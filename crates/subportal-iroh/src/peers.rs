use std::path::{Path, PathBuf};

use anyhow::Context;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::consts::{CLIENTS_FILE, DEFAULT_TOKEN_TTL_SECS, SERVERS_FILE};

// ---------------------------------------------------------------------------
// Client registry (server/agent side)
// ---------------------------------------------------------------------------

/// An enrolled client device.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientEntry {
    /// The client's iroh endpoint ID (public key hex).
    pub endpoint_id: String,
    /// Human-readable name for this client.
    pub name: String,
    /// When this client was enrolled.
    pub enrolled_at: DateTime<Utc>,
    /// Last time this client connected.
    pub last_seen: Option<DateTime<Utc>>,
    /// Capabilities this client supports.
    pub capabilities: Vec<String>,
}

/// Persistent registry of enrolled clients, stored as JSON.
#[derive(Debug)]
pub struct ClientRegistry {
    path: PathBuf,
    clients: Vec<ClientEntry>,
}

impl ClientRegistry {
    /// Load the client registry from the given directory.
    pub async fn load(dir: &Path) -> anyhow::Result<Self> {
        let path = dir.join(CLIENTS_FILE);
        let clients = if path.exists() {
            let data = tokio::fs::read_to_string(&path)
                .await
                .context("failed to read clients file")?;
            serde_json::from_str(&data).context("failed to parse clients file")?
        } else {
            Vec::new()
        };
        Ok(Self { path, clients })
    }

    /// Save the registry to disk.
    pub async fn save(&self) -> anyhow::Result<()> {
        if let Some(parent) = self.path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }
        let data = serde_json::to_string_pretty(&self.clients)?;
        tokio::fs::write(&self.path, data).await?;
        Ok(())
    }

    /// Add a new client entry and persist.
    pub async fn add(&mut self, entry: ClientEntry) -> anyhow::Result<()> {
        // Remove any existing entry with the same endpoint_id
        self.clients.retain(|c| c.endpoint_id != entry.endpoint_id);
        self.clients.push(entry);
        self.save().await
    }

    /// Remove a client by endpoint ID or name.
    pub async fn remove(&mut self, id_or_name: &str) -> anyhow::Result<bool> {
        let before = self.clients.len();
        self.clients
            .retain(|c| c.endpoint_id != id_or_name && c.name != id_or_name);
        let removed = self.clients.len() < before;
        if removed {
            self.save().await?;
        }
        Ok(removed)
    }

    /// Find a client by endpoint ID.
    pub fn find_by_id(&self, endpoint_id: &str) -> Option<&ClientEntry> {
        self.clients.iter().find(|c| c.endpoint_id == endpoint_id)
    }

    /// List all enrolled clients.
    pub fn list(&self) -> &[ClientEntry] {
        &self.clients
    }

    /// Update the last_seen timestamp for a client.
    pub async fn touch(&mut self, endpoint_id: &str) -> anyhow::Result<()> {
        if let Some(c) = self
            .clients
            .iter_mut()
            .find(|c| c.endpoint_id == endpoint_id)
        {
            c.last_seen = Some(Utc::now());
            self.save().await?;
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Pending enrollment tokens (agent side, in-memory)
// ---------------------------------------------------------------------------

/// A one-time enrollment token.
#[derive(Debug, Clone)]
pub struct PendingToken {
    /// The token string (hex-encoded random bytes).
    pub token: String,
    /// When the token was created.
    pub created_at: DateTime<Utc>,
    /// When the token expires.
    pub expires_at: DateTime<Utc>,
}

impl PendingToken {
    /// Generate a new enrollment token.
    pub fn generate(ttl_secs: u64) -> Self {
        let mut bytes = [0u8; 16];
        rand::fill(&mut bytes);
        let token = hex::encode(bytes);
        let now = Utc::now();
        let expires_at = now + chrono::Duration::seconds(ttl_secs as i64);
        Self {
            token,
            created_at: now,
            expires_at,
        }
    }

    /// Generate a token with the default TTL.
    pub fn generate_default() -> Self {
        Self::generate(DEFAULT_TOKEN_TTL_SECS)
    }

    /// Check if the token has expired.
    pub fn is_expired(&self) -> bool {
        Utc::now() > self.expires_at
    }
}

// ---------------------------------------------------------------------------
// Server registry (desktop/client side)
// ---------------------------------------------------------------------------

/// An enrolled server that the desktop client connects to.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerEntry {
    /// The server agent's iroh endpoint ID (public key hex).
    pub endpoint_id: String,
    /// Human-readable name for this server.
    pub name: String,
    /// Direct addresses for the server.
    pub addrs: Vec<String>,
    /// Relay URL for the server.
    pub relay_url: Option<String>,
    /// When this server was enrolled.
    pub enrolled_at: DateTime<Utc>,
}

/// Persistent registry of enrolled servers, stored as JSON.
#[derive(Debug)]
pub struct ServerRegistry {
    path: PathBuf,
    servers: Vec<ServerEntry>,
}

impl ServerRegistry {
    /// Load the server registry from the given directory.
    pub async fn load(dir: &Path) -> anyhow::Result<Self> {
        let path = dir.join(SERVERS_FILE);
        let servers = if path.exists() {
            let data = tokio::fs::read_to_string(&path)
                .await
                .context("failed to read servers file")?;
            serde_json::from_str(&data).context("failed to parse servers file")?
        } else {
            Vec::new()
        };
        Ok(Self { path, servers })
    }

    /// Save the registry to disk.
    pub async fn save(&self) -> anyhow::Result<()> {
        if let Some(parent) = self.path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }
        let data = serde_json::to_string_pretty(&self.servers)?;
        tokio::fs::write(&self.path, data).await?;
        Ok(())
    }

    /// Add a new server entry and persist.
    pub async fn add(&mut self, entry: ServerEntry) -> anyhow::Result<()> {
        self.servers.retain(|s| s.endpoint_id != entry.endpoint_id);
        self.servers.push(entry);
        self.save().await
    }

    /// Remove a server by endpoint ID or name.
    pub async fn remove(&mut self, id_or_name: &str) -> anyhow::Result<bool> {
        let before = self.servers.len();
        self.servers
            .retain(|s| s.endpoint_id != id_or_name && s.name != id_or_name);
        let removed = self.servers.len() < before;
        if removed {
            self.save().await?;
        }
        Ok(removed)
    }

    /// List all enrolled servers.
    pub fn list(&self) -> &[ServerEntry] {
        &self.servers
    }

    /// Find a server by endpoint ID.
    pub fn find_by_id(&self, endpoint_id: &str) -> Option<&ServerEntry> {
        self.servers.iter().find(|s| s.endpoint_id == endpoint_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn client_registry_round_trip() {
        let tmp = tempfile::tempdir().unwrap();
        let mut reg = ClientRegistry::load(tmp.path()).await.unwrap();
        assert!(reg.list().is_empty());

        let entry = ClientEntry {
            endpoint_id: "abc123".into(),
            name: "laptop".into(),
            enrolled_at: Utc::now(),
            last_seen: None,
            capabilities: vec!["Notify".into()],
        };
        reg.add(entry).await.unwrap();
        assert_eq!(reg.list().len(), 1);

        // Reload from disk
        let reg2 = ClientRegistry::load(tmp.path()).await.unwrap();
        assert_eq!(reg2.list().len(), 1);
        assert_eq!(reg2.list()[0].name, "laptop");
    }

    #[tokio::test]
    async fn client_registry_remove() {
        let tmp = tempfile::tempdir().unwrap();
        let mut reg = ClientRegistry::load(tmp.path()).await.unwrap();
        let entry = ClientEntry {
            endpoint_id: "abc".into(),
            name: "laptop".into(),
            enrolled_at: Utc::now(),
            last_seen: None,
            capabilities: vec![],
        };
        reg.add(entry).await.unwrap();
        assert!(reg.remove("laptop").await.unwrap());
        assert!(reg.list().is_empty());
    }

    #[tokio::test]
    async fn server_registry_round_trip() {
        let tmp = tempfile::tempdir().unwrap();
        let mut reg = ServerRegistry::load(tmp.path()).await.unwrap();
        assert!(reg.list().is_empty());

        let entry = ServerEntry {
            endpoint_id: "xyz789".into(),
            name: "myserver".into(),
            addrs: vec!["1.2.3.4:5678".into()],
            relay_url: None,
            enrolled_at: Utc::now(),
        };
        reg.add(entry).await.unwrap();

        let reg2 = ServerRegistry::load(tmp.path()).await.unwrap();
        assert_eq!(reg2.list().len(), 1);
        assert_eq!(reg2.list()[0].name, "myserver");
    }

    #[test]
    fn pending_token_generation() {
        let token = PendingToken::generate(60);
        assert_eq!(token.token.len(), 32); // 16 bytes = 32 hex chars
        assert!(!token.is_expired());
    }

    #[test]
    fn pending_token_expired() {
        let token = PendingToken {
            token: "test".into(),
            created_at: Utc::now() - chrono::Duration::seconds(120),
            expires_at: Utc::now() - chrono::Duration::seconds(60),
        };
        assert!(token.is_expired());
    }
}
