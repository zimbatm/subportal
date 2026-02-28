use anyhow::{bail, Context};
use iroh::{EndpointAddr, EndpointId, TransportAddr};
use serde::{Deserialize, Serialize};
use std::io::{Cursor, Read, Write};
use std::net::{Ipv4Addr, Ipv6Addr, SocketAddr};

/// An enrollment ticket exchanged between agent and client.
///
/// The agent generates this and prints it as JSON to stdout. The client reads
/// it from stdin during `subportald enroll`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Ticket {
    /// The agent's endpoint ID (public key).
    pub endpoint_id: String,
    /// Direct addresses of the agent.
    pub addrs: Vec<String>,
    /// Relay URL, if available.
    pub relay_url: Option<String>,
    /// One-time enrollment token.
    pub token: String,
    /// Hostname of the server running the agent.
    pub hostname: String,
}

impl Ticket {
    /// Serialize the ticket to JSON.
    pub fn to_json(&self) -> String {
        serde_json::to_string(self).expect("ticket serialization cannot fail")
    }

    /// Parse a ticket from JSON.
    pub fn from_json(s: &str) -> anyhow::Result<Self> {
        serde_json::from_str(s).context("failed to parse enrollment ticket")
    }

    /// Compact binary prefix.
    const COMPACT_PREFIX: &str = "SP1:";

    /// Encode the ticket into a compact binary format, base45-encoded with
    /// an `SP1:` prefix. Designed for efficient QR code rendering using
    /// alphanumeric mode.
    pub fn to_compact(&self) -> anyhow::Result<String> {
        let mut buf = Vec::new();

        // version
        buf.write_all(&[0x01])?;

        // endpoint_id: 32 bytes from hex
        let eid_bytes =
            hex::decode(&self.endpoint_id).context("endpoint_id is not valid hex")?;
        anyhow::ensure!(eid_bytes.len() == 32, "endpoint_id must be 32 bytes (64 hex chars)");
        buf.write_all(&eid_bytes)?;

        // token: 16 bytes from hex
        let token_bytes = hex::decode(&self.token).context("token is not valid hex")?;
        anyhow::ensure!(token_bytes.len() == 16, "token must be 16 bytes (32 hex chars)");
        buf.write_all(&token_bytes)?;

        // addrs
        let parsed_addrs: Vec<SocketAddr> = self
            .addrs
            .iter()
            .map(|a| a.parse::<SocketAddr>().context("invalid socket address"))
            .collect::<anyhow::Result<Vec<_>>>()?;
        anyhow::ensure!(parsed_addrs.len() <= 255, "too many addresses");
        buf.write_all(&[parsed_addrs.len() as u8])?;

        for addr in &parsed_addrs {
            match addr {
                SocketAddr::V4(v4) => {
                    buf.write_all(&[4])?;
                    buf.write_all(&v4.ip().octets())?;
                    buf.write_all(&v4.port().to_be_bytes())?;
                }
                SocketAddr::V6(v6) => {
                    buf.write_all(&[6])?;
                    buf.write_all(&v6.ip().octets())?;
                    buf.write_all(&v6.port().to_be_bytes())?;
                }
            }
        }

        // relay_url
        match &self.relay_url {
            Some(url) => {
                buf.write_all(&[1])?;
                let url_bytes = url.as_bytes();
                anyhow::ensure!(url_bytes.len() <= u16::MAX as usize, "relay URL too long");
                buf.write_all(&(url_bytes.len() as u16).to_be_bytes())?;
                buf.write_all(url_bytes)?;
            }
            None => {
                buf.write_all(&[0])?;
            }
        }

        // hostname
        let host_bytes = self.hostname.as_bytes();
        anyhow::ensure!(host_bytes.len() <= 255, "hostname too long");
        buf.write_all(&[host_bytes.len() as u8])?;
        buf.write_all(host_bytes)?;

        let encoded = base45::encode(&buf);
        Ok(format!("{}{}", Self::COMPACT_PREFIX, encoded))
    }

    /// Decode a compact-encoded ticket (base45 with `SP1:` prefix).
    pub fn from_compact(s: &str) -> anyhow::Result<Self> {
        let payload = s
            .strip_prefix(Self::COMPACT_PREFIX)
            .context("missing SP1: prefix")?;

        let bin = base45::decode(payload).context("invalid base45 encoding")?;
        let mut cur = Cursor::new(&bin);

        // version
        let mut version = [0u8; 1];
        cur.read_exact(&mut version)
            .context("truncated: missing version byte")?;
        anyhow::ensure!(version[0] == 0x01, "unsupported compact ticket version {}", version[0]);

        // endpoint_id
        let mut eid = [0u8; 32];
        cur.read_exact(&mut eid)
            .context("truncated: missing endpoint_id")?;
        let endpoint_id = hex::encode(eid);

        // token
        let mut token_buf = [0u8; 16];
        cur.read_exact(&mut token_buf)
            .context("truncated: missing token")?;
        let token = hex::encode(token_buf);

        // addrs
        let mut addr_count = [0u8; 1];
        cur.read_exact(&mut addr_count)
            .context("truncated: missing addr_count")?;
        let mut addrs = Vec::with_capacity(addr_count[0] as usize);
        for _ in 0..addr_count[0] {
            let mut atype = [0u8; 1];
            cur.read_exact(&mut atype)
                .context("truncated: missing addr type")?;
            let addr: SocketAddr = match atype[0] {
                4 => {
                    let mut ip = [0u8; 4];
                    cur.read_exact(&mut ip).context("truncated: missing IPv4")?;
                    let mut port = [0u8; 2];
                    cur.read_exact(&mut port)
                        .context("truncated: missing port")?;
                    SocketAddr::new(
                        Ipv4Addr::from(ip).into(),
                        u16::from_be_bytes(port),
                    )
                }
                6 => {
                    let mut ip = [0u8; 16];
                    cur.read_exact(&mut ip)
                        .context("truncated: missing IPv6")?;
                    let mut port = [0u8; 2];
                    cur.read_exact(&mut port)
                        .context("truncated: missing port")?;
                    SocketAddr::new(
                        Ipv6Addr::from(ip).into(),
                        u16::from_be_bytes(port),
                    )
                }
                other => bail!("invalid address type: {other}"),
            };
            addrs.push(addr.to_string());
        }

        // relay_url
        let mut relay_present = [0u8; 1];
        cur.read_exact(&mut relay_present)
            .context("truncated: missing relay_url_present")?;
        let relay_url = if relay_present[0] == 1 {
            let mut len_buf = [0u8; 2];
            cur.read_exact(&mut len_buf)
                .context("truncated: missing relay URL length")?;
            let len = u16::from_be_bytes(len_buf) as usize;
            let mut url_buf = vec![0u8; len];
            cur.read_exact(&mut url_buf)
                .context("truncated: missing relay URL")?;
            Some(String::from_utf8(url_buf).context("relay URL is not valid UTF-8")?)
        } else {
            None
        };

        // hostname
        let mut host_len = [0u8; 1];
        cur.read_exact(&mut host_len)
            .context("truncated: missing hostname length")?;
        let mut host_buf = vec![0u8; host_len[0] as usize];
        cur.read_exact(&mut host_buf)
            .context("truncated: missing hostname")?;
        let hostname = String::from_utf8(host_buf).context("hostname is not valid UTF-8")?;

        Ok(Ticket {
            endpoint_id,
            addrs,
            relay_url,
            token,
            hostname,
        })
    }

    /// Parse a ticket from either compact (`SP1:...`) or JSON format,
    /// auto-detecting based on the prefix.
    pub fn parse(s: &str) -> anyhow::Result<Self> {
        if s.starts_with(Self::COMPACT_PREFIX) {
            Self::from_compact(s)
        } else {
            Self::from_json(s)
        }
    }

    /// Build an iroh `EndpointAddr` from the ticket fields.
    pub fn to_endpoint_addr(&self) -> anyhow::Result<EndpointAddr> {
        let id: EndpointId = self
            .endpoint_id
            .parse()
            .context("invalid endpoint ID in ticket")?;
        let mut addrs = std::collections::BTreeSet::new();
        if let Some(ref relay) = self.relay_url {
            let url = relay.parse().context("invalid relay URL in ticket")?;
            addrs.insert(TransportAddr::Relay(url));
        }
        for addr_str in &self.addrs {
            if let Ok(sock) = addr_str.parse() {
                addrs.insert(TransportAddr::Ip(sock));
            }
        }
        Ok(EndpointAddr { id, addrs })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ticket_json_round_trip() {
        let ticket = Ticket {
            endpoint_id: "abc123".into(),
            addrs: vec!["127.0.0.1:1234".into()],
            relay_url: Some("https://relay.example.com".into()),
            token: "deadbeef".into(),
            hostname: "myserver".into(),
        };
        let json = ticket.to_json();
        let back = Ticket::from_json(&json).unwrap();
        assert_eq!(back.endpoint_id, ticket.endpoint_id);
        assert_eq!(back.token, ticket.token);
        assert_eq!(back.hostname, ticket.hostname);
        assert_eq!(back.addrs, ticket.addrs);
        assert_eq!(back.relay_url, ticket.relay_url);
    }

    #[test]
    fn ticket_from_invalid_json() {
        assert!(Ticket::from_json("not json").is_err());
    }

    /// Helper to create a realistic ticket with valid 32-byte endpoint_id
    /// and 16-byte token (both hex-encoded).
    fn make_ticket(
        addrs: Vec<&str>,
        relay_url: Option<&str>,
    ) -> Ticket {
        Ticket {
            // 32 bytes = 64 hex chars
            endpoint_id: "a1b2c3d4e5f6a7b8c9d0e1f2a3b4c5d6a7b8c9d0e1f2a3b4c5d6a7b8c9d0e1f2"
                .into(),
            addrs: addrs.into_iter().map(String::from).collect(),
            relay_url: relay_url.map(String::from),
            // 16 bytes = 32 hex chars
            token: "deadbeefcafebabe1234567890abcdef".into(),
            hostname: "myserver".into(),
        }
    }

    #[test]
    fn compact_round_trip_ipv4_with_relay() {
        let ticket = make_ticket(
            vec!["127.0.0.1:1234", "192.168.1.1:5678"],
            Some("https://relay.example.com"),
        );
        let compact = ticket.to_compact().unwrap();
        assert!(compact.starts_with("SP1:"));
        let back = Ticket::from_compact(&compact).unwrap();
        assert_eq!(back.endpoint_id, ticket.endpoint_id);
        assert_eq!(back.token, ticket.token);
        assert_eq!(back.hostname, ticket.hostname);
        assert_eq!(back.addrs, ticket.addrs);
        assert_eq!(back.relay_url, ticket.relay_url);
    }

    #[test]
    fn compact_round_trip_ipv6() {
        let ticket = make_ticket(
            vec!["[::1]:4433", "[2001:db8::1]:8080"],
            None,
        );
        let compact = ticket.to_compact().unwrap();
        let back = Ticket::from_compact(&compact).unwrap();
        assert_eq!(back.addrs, ticket.addrs);
        assert_eq!(back.relay_url, None);
    }

    #[test]
    fn compact_round_trip_mixed_addrs() {
        let ticket = make_ticket(
            vec!["127.0.0.1:1234", "[::1]:5678"],
            Some("https://relay.example.com"),
        );
        let compact = ticket.to_compact().unwrap();
        let back = Ticket::from_compact(&compact).unwrap();
        assert_eq!(back.addrs, ticket.addrs);
    }

    #[test]
    fn compact_round_trip_no_relay() {
        let ticket = make_ticket(vec!["10.0.0.1:9999"], None);
        let compact = ticket.to_compact().unwrap();
        let back = Ticket::from_compact(&compact).unwrap();
        assert_eq!(back.relay_url, None);
    }

    #[test]
    fn compact_bad_prefix() {
        assert!(Ticket::from_compact("XX:AAAA").is_err());
    }

    #[test]
    fn compact_bad_version() {
        // Encode version 0x02 instead of 0x01
        let mut buf = vec![0x02];
        buf.extend_from_slice(&[0u8; 32]); // endpoint_id
        buf.extend_from_slice(&[0u8; 16]); // token
        buf.push(0); // addr_count
        buf.push(0); // relay_url_present
        buf.push(0); // hostname_len
        let encoded = format!("SP1:{}", base45::encode(&buf));
        let err = Ticket::from_compact(&encoded).unwrap_err();
        assert!(
            format!("{err}").contains("unsupported compact ticket version"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn compact_truncated() {
        // Just the prefix with a few valid base45 chars
        let encoded = format!("SP1:{}", base45::encode(&[0x01]));
        assert!(Ticket::from_compact(&encoded).is_err());
    }

    #[test]
    fn compact_invalid_base45() {
        assert!(Ticket::from_compact("SP1:!!!invalid!!!").is_err());
    }

    #[test]
    fn parse_auto_detects_compact() {
        let ticket = make_ticket(vec!["127.0.0.1:1234"], None);
        let compact = ticket.to_compact().unwrap();
        let back = Ticket::parse(&compact).unwrap();
        assert_eq!(back.endpoint_id, ticket.endpoint_id);
    }

    #[test]
    fn parse_auto_detects_json() {
        let ticket = make_ticket(vec!["127.0.0.1:1234"], None);
        let json = ticket.to_json();
        let back = Ticket::parse(&json).unwrap();
        assert_eq!(back.endpoint_id, ticket.endpoint_id);
    }

    #[test]
    fn compact_output_is_qr_alphanumeric() {
        let ticket = make_ticket(
            vec!["127.0.0.1:1234"],
            Some("https://relay.example.com"),
        );
        let compact = ticket.to_compact().unwrap();
        // QR alphanumeric charset: 0-9 A-Z space $ % * + - . / :
        let qr_alpha = "0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZ $%*+-./:";
        for ch in compact.chars() {
            assert!(
                qr_alpha.contains(ch),
                "non-QR-alphanumeric character found: '{ch}'"
            );
        }
    }
}
