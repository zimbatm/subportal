//! xdg-desktop-portal integration via D-Bus.
//!
//! Uses the [ashpd](https://docs.rs/ashpd) crate to talk to the local
//! xdg-desktop-portal instance. Provides functions for opening URIs, opening
//! files (with a temp-file write step), and sending notifications.

use std::os::fd::AsFd;
use std::path::PathBuf;

use anyhow::Context;
use ashpd::desktop::notification::{Notification, NotificationProxy, Priority};
use ashpd::desktop::open_uri::OpenFileRequest;
use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64;

/// Open a URI in the user's default application, showing a confirmation dialog.
pub async fn open_uri(uri: &str) -> anyhow::Result<()> {
    let url = url::Url::parse(uri).context("invalid URI")?;
    OpenFileRequest::default()
        .ask(true)
        .send_uri(&url)
        .await
        .context("xdg-desktop-portal OpenURI failed")?;
    Ok(())
}

/// Decode a base64-encoded file, write it to a temp path, and open it via the portal.
pub async fn open_file(name: &str, _mime: &str, content_b64: &str) -> anyhow::Result<()> {
    let data = BASE64
        .decode(content_b64)
        .context("invalid base64 content")?;

    // Write to $XDG_RUNTIME_DIR/subportal/<name> (or /tmp/subportal/<name>)
    let runtime_dir = std::env::var("XDG_RUNTIME_DIR").unwrap_or_else(|_| "/tmp".into());
    let dir = PathBuf::from(runtime_dir).join("subportal");
    tokio::fs::create_dir_all(&dir).await?;

    let path = dir.join(name);
    tokio::fs::write(&path, &data).await?;

    let file = std::fs::File::open(&path).context("failed to open temp file")?;
    OpenFileRequest::default()
        .ask(true)
        .send_file(&file.as_fd())
        .await
        .context("xdg-desktop-portal OpenFile failed")?;

    Ok(())
}

/// Send a desktop notification.
pub async fn notify(
    title: &str,
    body: Option<&str>,
    urgency: Option<&str>,
    _icon: Option<&str>,
) -> anyhow::Result<()> {
    let proxy = NotificationProxy::new()
        .await
        .context("failed to connect to notification portal")?;

    let mut notification = Notification::new(title);

    if let Some(b) = body {
        notification = notification.body(b);
    }

    if let Some(u) = urgency {
        let priority = match u {
            "low" => Priority::Low,
            "critical" | "urgent" => Priority::Urgent,
            _ => Priority::Normal,
        };
        notification = notification.priority(priority);
    }

    // Use a unique ID so notifications don't replace each other
    let id = format!(
        "subportal-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis()
    );

    proxy
        .add_notification(&id, notification)
        .await
        .context("failed to send notification")?;

    Ok(())
}
