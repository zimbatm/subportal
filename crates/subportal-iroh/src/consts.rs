use std::path::PathBuf;

/// ALPN protocol identifier for subportal over iroh.
pub const ALPN: &[u8] = b"subportal/0";

/// File name for the persisted keypair.
pub const KEYPAIR_FILE: &str = "keypair";

/// File name for the server-side client registry.
pub const CLIENTS_FILE: &str = "clients.json";

/// File name for the desktop-side server list.
pub const SERVERS_FILE: &str = "servers.json";

/// Default enrollment token TTL in seconds (10 minutes).
pub const DEFAULT_TOKEN_TTL_SECS: u64 = 600;

/// Return the data directory: `$XDG_DATA_HOME/subportal` or `~/.local/share/subportal`.
pub fn data_dir() -> PathBuf {
    match std::env::var("XDG_DATA_HOME") {
        Ok(dir) => PathBuf::from(dir).join("subportal"),
        Err(_) => {
            let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
            PathBuf::from(home).join(".local/share/subportal")
        }
    }
}

/// Return the config directory: `$XDG_CONFIG_HOME/subportal` or `~/.config/subportal`.
pub fn config_dir() -> PathBuf {
    match std::env::var("XDG_CONFIG_HOME") {
        Ok(dir) => PathBuf::from(dir).join("subportal"),
        Err(_) => {
            let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
            PathBuf::from(home).join(".config/subportal")
        }
    }
}
