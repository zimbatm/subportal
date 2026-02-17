//! Desktop integration via D-Bus.
//!
//! Uses [ashpd](https://docs.rs/ashpd) for portal-based file/URI opening and
//! the standard `org.freedesktop.Notifications` interface for notifications.

use std::collections::HashMap;
use std::os::fd::AsFd;
use std::path::PathBuf;

use anyhow::Context;
use ashpd::desktop::open_uri::OpenFileRequest;
use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64;
use zbus::Connection;
use zbus::zvariant::Value;

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

/// Send a desktop notification via org.freedesktop.Notifications.
///
/// Uses the standard notification D-Bus interface directly rather than
/// xdg-desktop-portal, which requires a discoverable .desktop file and
/// pidfd-based caller identification that doesn't work with dbus-daemon.
pub async fn notify(
    title: &str,
    body: Option<&str>,
    urgency: Option<&str>,
    icon: Option<&str>,
) -> anyhow::Result<()> {
    let connection = Connection::session()
        .await
        .context("failed to connect to session D-Bus")?;

    let mut hints: HashMap<&str, Value<'_>> = HashMap::new();
    if let Some(u) = urgency {
        let level: u8 = match u {
            "low" => 0,
            "critical" | "urgent" => 2,
            _ => 1,
        };
        hints.insert("urgency", Value::from(level));
    }

    let _reply_id: u32 = connection
        .call_method(
            Some("org.freedesktop.Notifications"),
            "/org/freedesktop/Notifications",
            Some("org.freedesktop.Notifications"),
            "Notify",
            &(
                "subportal",                       // app_name
                0u32,                              // replaces_id
                icon.unwrap_or(""),                // app_icon
                title,                             // summary
                body.unwrap_or(""),                // body
                Vec::<&str>::new(),                // actions
                &hints,                            // hints
                -1i32,                             // expire_timeout (-1 = server default)
            ),
        )
        .await
        .context("failed to send notification")?
        .body()
        .deserialize()
        .context("failed to parse notification reply")?;

    Ok(())
}
