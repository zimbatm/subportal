//! Unix socket server that accepts Varlink requests from subportal clients.
//!
//! The [`Server`] binds to a Unix domain socket and accepts one connection at a
//! time via [`Server::accept`]. Each accepted connection yields a parsed
//! [`Request`] and a [`Responder`] that the caller uses to send back either
//! a success [`Response`] or a [`SubportalError`].

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use tokio::io::BufReader;
use tokio::net::{UnixListener, UnixStream};
use tracing::{info, warn};

use crate::consts::default_socket_path;
use crate::protocol::{
    read_message, write_message, Request, Response, SubportalError, VarlinkRequest, VarlinkResponse,
};

/// Information about the peer process that connected.
#[derive(Debug, Clone, Default)]
pub struct PeerInfo {
    /// PID of the connecting process (from `SO_PEERCRED`).
    pub pid: Option<u32>,
    /// UID of the connecting process (from `SO_PEERCRED`).
    pub uid: Option<u32>,
    /// SSH remote host, resolved from `/proc/<pid>/cmdline` if the peer is an
    /// `sshd` or `ssh` process.
    pub ssh_host: Option<String>,
}

/// A Unix socket server that accepts Varlink requests.
pub struct Server {
    listener: UnixListener,
    path: PathBuf,
    /// Maps SSH ControlMaster socket paths to their SSH host names.
    /// Used to resolve the host when the ControlMaster has rewritten its cmdline.
    control_path_hosts: HashMap<PathBuf, String>,
}

/// Holds a parsed request and the stream, allowing the handler to send a response.
pub struct Responder {
    stream: UnixStream,
    /// Information about the connecting peer.
    pub peer: PeerInfo,
}

impl Server {
    /// Bind to the given Unix socket path.
    ///
    /// Creates the parent directory if needed and removes any stale socket file.
    pub async fn bind(path: impl AsRef<Path>) -> anyhow::Result<Self> {
        let path = path.as_ref().to_path_buf();

        // Create parent directory if it doesn't exist.
        if let Some(parent) = path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }

        // Remove stale socket file if present.
        if path.exists() {
            tokio::fs::remove_file(&path).await?;
        }

        let listener = UnixListener::bind(&path)?;
        info!("listening on {}", path.display());
        Ok(Self {
            listener,
            path,
            control_path_hosts: HashMap::new(),
        })
    }

    /// Return the socket path the server is bound to.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Set the ControlMaster control-path-to-hostname mapping.
    ///
    /// When SSH ControlMaster rewrites its cmdline to `ssh: <path> [mux]`,
    /// the original hostname is lost. This map lets us look it up from the
    /// control socket path.
    pub fn set_control_path_hosts(&mut self, map: HashMap<PathBuf, String>) {
        self.control_path_hosts = map;
    }

    /// Bind using the default socket path.
    pub async fn bind_default() -> anyhow::Result<Self> {
        Self::bind(default_socket_path()).await
    }

    /// Accept the next connection, read the request, and return it with a `Responder`.
    pub async fn accept(&self) -> anyhow::Result<(Request, Responder)> {
        let (mut stream, _addr) = self.listener.accept().await?;

        let peer = resolve_peer(&stream, &self.control_path_hosts);

        // Reject connections from different, non-root UIDs (matches OpenSSH behaviour).
        if let Some(uid) = peer.uid {
            let my_uid = unsafe { libc::getuid() };
            if uid != 0 && uid != my_uid {
                warn!(
                    peer_uid = uid,
                    server_uid = my_uid,
                    "rejecting connection: uid mismatch"
                );
                anyhow::bail!(
                    "uid mismatch: peer uid {} != server uid {}",
                    uid,
                    my_uid
                );
            }
        }

        if let Some(ref host) = peer.ssh_host {
            info!("connection from ssh host {host} (pid {:?})", peer.pid);
        } else if let Some(pid) = peer.pid {
            let cmdline = std::fs::read(format!("/proc/{pid}/cmdline"))
                .ok()
                .map(|b| {
                    b.split(|&c| c == 0)
                        .filter_map(|s| std::str::from_utf8(s).ok())
                        .filter(|s| !s.is_empty())
                        .collect::<Vec<_>>()
                        .join(" ")
                })
                .unwrap_or_default();
            info!("connection from pid {pid}, ssh host not resolved (cmdline: {cmdline})");
        } else {
            info!("connection from unknown peer");
        }

        let varlink_req: VarlinkRequest = read_message(&mut BufReader::new(&mut stream)).await?;
        let request = Request::from_varlink(&varlink_req)?;

        Ok((request, Responder { stream, peer }))
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        // Best-effort removal of the socket file.
        let _ = std::fs::remove_file(&self.path);
    }
}

impl Responder {
    /// Send a successful response.
    pub async fn send_ok(mut self, response: Response) -> anyhow::Result<()> {
        let varlink_resp = response.to_varlink();
        write_message(&mut self.stream, &varlink_resp).await
    }

    /// Send an error response.
    pub async fn send_error(mut self, error: SubportalError) -> anyhow::Result<()> {
        let varlink_resp: VarlinkResponse = error.to_varlink();
        write_message(&mut self.stream, &varlink_resp).await
    }
}

/// Resolve peer information from a Unix stream using `SO_PEERCRED`.
fn resolve_peer(
    stream: &UnixStream,
    control_path_hosts: &HashMap<PathBuf, String>,
) -> PeerInfo {
    let cred = stream.peer_cred().ok();

    let pid = cred.as_ref().and_then(|c| c.pid()).map(|p| p as u32);
    let uid = cred.as_ref().map(|c| c.uid());

    let ssh_host = pid.and_then(|p| resolve_ssh_host(p, control_path_hosts));

    PeerInfo { pid, uid, ssh_host }
}

/// Try to resolve the SSH host from `/proc/<pid>/cmdline`.
///
/// Walks the process tree upward looking for an `ssh` client process or an
/// `sshd` server process and extracts the remote host. Also checks for SSH
/// ControlMaster processes whose cmdline has been rewritten to
/// `ssh: <control_path> [mux]` and looks up the host via the control path map.
fn resolve_ssh_host(
    pid: u32,
    control_path_hosts: &HashMap<PathBuf, String>,
) -> Option<String> {
    let mut current_pid = pid;
    for _ in 0..10 {
        if let Some(host) = parse_ssh_host_from_cmdline(current_pid) {
            return Some(host);
        }
        if let Some(host) = resolve_mux_master_host(current_pid, control_path_hosts) {
            return Some(host);
        }
        match get_parent_pid(current_pid) {
            Some(ppid) if ppid > 1 && ppid != current_pid => current_pid = ppid,
            _ => break,
        }
    }
    None
}

/// Parse `/proc/<pid>/cmdline` looking for an SSH host pattern.
///
/// Handles two cases:
/// - **`ssh` client** (desktop side): cmdline is NUL-separated args like
///   `ssh\0-R\0...\0kit.ntd.one\0`. The destination is the first positional
///   argument.
/// - **`sshd` server** (remote side): cmdline is a process title like
///   `sshd: user@1.2.3.4`.
fn parse_ssh_host_from_cmdline(pid: u32) -> Option<String> {
    let cmdline = std::fs::read(format!("/proc/{pid}/cmdline")).ok()?;

    // Split into NUL-separated arguments, filtering empty trailing entries.
    let args: Vec<&str> = cmdline
        .split(|&b| b == 0)
        .filter_map(|s| {
            let s = std::str::from_utf8(s).ok()?;
            if s.is_empty() { None } else { Some(s) }
        })
        .collect();

    if args.is_empty() {
        return None;
    }

    // Check for sshd process title: "sshd: user@host"
    if args[0].starts_with("sshd: ") {
        return parse_sshd_host(args[0]);
    }

    // Check for ssh client binary.
    let binary = std::path::Path::new(args[0])
        .file_name()
        .and_then(|n| n.to_str())?;
    if binary == "ssh" {
        return parse_ssh_client_host(&args[1..]);
    }

    None
}

/// Extract host from an sshd process title like `sshd: user@1.2.3.4`.
fn parse_sshd_host(title: &str) -> Option<String> {
    let rest = title.strip_prefix("sshd: ")?;
    let at_pos = rest.find('@')?;
    let after_at = &rest[at_pos + 1..];
    let host = after_at
        .split(|c: char| c.is_whitespace() || c == '\0')
        .next()?;
    if host.is_empty() {
        return None;
    }
    Some(host.to_string())
}

/// Extract the destination host from ssh client arguments (excluding argv[0]).
///
/// Parses the argument list, skipping flags and their values, to find the
/// first positional argument (the destination). Handles `user@host` syntax.
fn parse_ssh_client_host(args: &[&str]) -> Option<String> {
    // ssh flags that consume the next argument as a value.
    const VALUE_FLAGS: &[char] = &[
        'B', 'b', 'c', 'D', 'E', 'e', 'F', 'I', 'i', 'J', 'L', 'l', 'm', 'O', 'o', 'p', 'Q',
        'R', 'S', 'W', 'w',
    ];

    let mut i = 0;
    while i < args.len() {
        let arg = args[i];

        if arg == "--" {
            i += 1;
            break;
        }

        if arg.starts_with('-') && arg.len() > 1 {
            // Check if the first flag character takes a value.
            let first_flag = arg.chars().nth(1)?;
            if VALUE_FLAGS.contains(&first_flag) {
                if arg.len() == 2 {
                    // Value is the next argument: `-p 22`
                    i += 2;
                } else {
                    // Value is attached: `-p22`, `-oFoo=bar`
                    i += 1;
                }
            } else {
                // Standalone flags, possibly combined: `-vvv`, `-NTf`
                i += 1;
            }
            continue;
        }

        // First positional argument is the destination.
        break;
    }

    if i >= args.len() {
        return None;
    }

    let dest = args[i];
    // Strip user@ prefix if present.
    match dest.find('@') {
        Some(at_pos) => Some(dest[at_pos + 1..].to_string()),
        None => Some(dest.to_string()),
    }
}

/// Resolve the SSH host from a ControlMaster process by matching its control
/// socket path against the known host map.
///
/// When SSH ControlMaster is active, it rewrites its cmdline via `setproctitle`
/// to `ssh: <control_path> [mux]`, losing the original hostname. We extract
/// the control path and look it up in the pre-built map.
fn resolve_mux_master_host(
    pid: u32,
    control_path_hosts: &HashMap<PathBuf, String>,
) -> Option<String> {
    if control_path_hosts.is_empty() {
        return None;
    }
    let cmdline = std::fs::read(format!("/proc/{pid}/cmdline")).ok()?;
    let title = std::str::from_utf8(&cmdline)
        .ok()?
        .trim_end_matches('\0');
    let path = parse_mux_master_path(title)?;
    control_path_hosts.get(Path::new(path)).cloned()
}

/// Extract the control socket path from an SSH ControlMaster process title.
///
/// The ControlMaster sets its process title to `ssh: <control_path> [mux]`.
/// Returns `None` if the title doesn't match this pattern.
fn parse_mux_master_path(title: &str) -> Option<&str> {
    let rest = title.strip_prefix("ssh: ")?;
    rest.strip_suffix(" [mux]").filter(|p| !p.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sshd_user_at_ip() {
        assert_eq!(
            parse_sshd_host("sshd: zimbatm@1.2.3.4"),
            Some("1.2.3.4".into())
        );
    }

    #[test]
    fn sshd_user_at_ip_with_suffix() {
        assert_eq!(
            parse_sshd_host("sshd: zimbatm@1.2.3.4 [priv]"),
            Some("1.2.3.4".into())
        );
    }

    #[test]
    fn sshd_no_at_sign() {
        assert_eq!(parse_sshd_host("sshd: zimbatm [priv]"), None);
    }

    #[test]
    fn sshd_not_sshd() {
        assert_eq!(parse_sshd_host("bash"), None);
    }

    #[test]
    fn ssh_simple_host() {
        assert_eq!(
            parse_ssh_client_host(&["kit.ntd.one"]),
            Some("kit.ntd.one".into())
        );
    }

    #[test]
    fn ssh_user_at_host() {
        assert_eq!(
            parse_ssh_client_host(&["zimbatm@kit.ntd.one"]),
            Some("kit.ntd.one".into())
        );
    }

    #[test]
    fn ssh_with_port() {
        assert_eq!(
            parse_ssh_client_host(&["-p", "2222", "kit.ntd.one"]),
            Some("kit.ntd.one".into())
        );
    }

    #[test]
    fn ssh_with_attached_port() {
        assert_eq!(
            parse_ssh_client_host(&["-p2222", "kit.ntd.one"]),
            Some("kit.ntd.one".into())
        );
    }

    #[test]
    fn ssh_with_options() {
        assert_eq!(
            parse_ssh_client_host(&["-o", "StrictHostKeyChecking=no", "-v", "kit.ntd.one"]),
            Some("kit.ntd.one".into())
        );
    }

    #[test]
    fn ssh_with_remote_forward() {
        assert_eq!(
            parse_ssh_client_host(&[
                "-R",
                "/run/user/1000/subportal.sock:/run/user/1000/subportal.sock",
                "kit.ntd.one"
            ]),
            Some("kit.ntd.one".into())
        );
    }

    #[test]
    fn ssh_combined_standalone_flags() {
        assert_eq!(
            parse_ssh_client_host(&["-NTf", "kit.ntd.one"]),
            Some("kit.ntd.one".into())
        );
    }

    #[test]
    fn ssh_double_dash() {
        assert_eq!(
            parse_ssh_client_host(&["-v", "--", "kit.ntd.one"]),
            Some("kit.ntd.one".into())
        );
    }

    #[test]
    fn ssh_no_args() {
        assert_eq!(parse_ssh_client_host(&[]), None);
    }

    #[test]
    fn ssh_only_flags() {
        assert_eq!(parse_ssh_client_host(&["-v", "-N"]), None);
    }

    #[test]
    fn mux_master_path() {
        assert_eq!(
            parse_mux_master_path("ssh: /home/user/.ssh/control-abc123 [mux]"),
            Some("/home/user/.ssh/control-abc123")
        );
    }

    #[test]
    fn mux_master_path_with_hash() {
        assert_eq!(
            parse_mux_master_path(
                "ssh: /home/zimbatm/.ssh/control-cd85c872044efab2587aaeb129ac8de9846a47ad [mux]"
            ),
            Some("/home/zimbatm/.ssh/control-cd85c872044efab2587aaeb129ac8de9846a47ad")
        );
    }

    #[test]
    fn mux_master_path_not_mux() {
        assert_eq!(parse_mux_master_path("ssh: [stopped mux]"), None);
    }

    #[test]
    fn mux_master_path_not_ssh() {
        assert_eq!(parse_mux_master_path("bash"), None);
    }

    #[test]
    fn mux_master_path_empty() {
        assert_eq!(parse_mux_master_path("ssh:  [mux]"), None);
    }
}

/// Get the parent PID of a process from `/proc/<pid>/stat`.
fn get_parent_pid(pid: u32) -> Option<u32> {
    let stat = std::fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
    // Format: "pid (comm) state ppid ..."
    // Find the closing paren to skip the comm field (which can contain spaces).
    let after_comm = stat.rfind(')')? + 2;
    let fields: Vec<&str> = stat[after_comm..].split_whitespace().collect();
    // fields[0] = state, fields[1] = ppid
    fields.get(1)?.parse().ok()
}
