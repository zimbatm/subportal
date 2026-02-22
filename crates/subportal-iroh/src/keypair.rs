use std::path::Path;

use anyhow::Context;
use iroh::SecretKey;

/// Load a secret key from the given file, or generate a new one and save it.
///
/// The file stores the raw 32-byte secret key with 0o600 permissions.
pub async fn load_or_generate_keypair(path: &Path) -> anyhow::Result<SecretKey> {
    if path.exists() {
        let data = tokio::fs::read(path)
            .await
            .context("failed to read keypair file")?;
        let bytes: [u8; 32] = data
            .try_into()
            .map_err(|_| anyhow::anyhow!("keypair file has invalid length"))?;
        Ok(SecretKey::from_bytes(&bytes))
    } else {
        let key = SecretKey::generate(&mut rand::rng());
        if let Some(parent) = path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }
        tokio::fs::write(path, key.to_bytes()).await?;
        // Set permissions to 0o600
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let perms = std::fs::Permissions::from_mode(0o600);
            tokio::fs::set_permissions(path, perms).await?;
        }
        Ok(key)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn keypair_round_trip() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("keypair");

        // Generate and save
        let key1 = load_or_generate_keypair(&path).await.unwrap();
        assert!(path.exists());

        // Reload
        let key2 = load_or_generate_keypair(&path).await.unwrap();
        assert_eq!(key1.to_bytes(), key2.to_bytes());
    }

    #[tokio::test]
    async fn keypair_permissions() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("keypair");
        let _key = load_or_generate_keypair(&path).await.unwrap();

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let meta = std::fs::metadata(&path).unwrap();
            assert_eq!(meta.permissions().mode() & 0o777, 0o600);
        }
    }
}
