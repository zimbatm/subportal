use anyhow::Context;
use iroh::{EndpointAddr, EndpointId, TransportAddr};
use serde::{Deserialize, Serialize};

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
}
