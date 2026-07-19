//! Desktop integration via D-Bus.
//!
//! Uses [ashpd](https://docs.rs/ashpd) for portal-based file/URI opening and
//! the standard `org.freedesktop.Notifications` interface for notifications.

use std::collections::HashMap;
use std::os::fd::AsFd;
use std::path::PathBuf;

use anyhow::Context;
use ashpd::desktop::open_uri::OpenFileRequest;
use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine;
use zbus::zvariant::Value;
use zbus::Connection;

/// How long a confirmation prompt waits for the user before resolving as denied.
const CONFIRM_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(120);

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

    // Write to $XDG_RUNTIME_DIR/subportal/<name>
    let runtime_dir = std::env::var("XDG_RUNTIME_DIR").unwrap_or_else(|_| {
        let uid = unsafe { libc::getuid() };
        format!("/run/user/{uid}")
    });
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
    host: Option<&str>,
) -> anyhow::Result<u32> {
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

    let app_name = match host {
        Some(h) => format!("subportal@{h}"),
        None => "subportal".to_string(),
    };

    let reply_id: u32 = connection
        .call_method(
            Some("org.freedesktop.Notifications"),
            "/org/freedesktop/Notifications",
            Some("org.freedesktop.Notifications"),
            "Notify",
            &(
                app_name.as_str(),  // app_name
                0u32,               // replaces_id
                icon.unwrap_or(""), // app_icon
                title,              // summary
                body.unwrap_or(""), // body
                Vec::<&str>::new(), // actions
                &hints,             // hints
                -1i32,              // expire_timeout (-1 = server default)
            ),
        )
        .await
        .context("failed to send notification")?
        .body()
        .deserialize()
        .context("failed to parse notification reply")?;

    Ok(reply_id)
}

/// D-Bus proxy for the freedesktop Desktop Notifications interface.
///
/// This is the classic `org.freedesktop.Notifications` spec (which predates
/// xdg-desktop-portal and is what [`notify`] already uses). Its `actions` list
/// plus the `ActionInvoked` signal is how a notification becomes a yes/no
/// confirmation — there is no dedicated confirm dialog in either the
/// notifications spec or the desktop portal spec.
#[zbus::proxy(
    interface = "org.freedesktop.Notifications",
    default_service = "org.freedesktop.Notifications",
    default_path = "/org/freedesktop/Notifications"
)]
trait Notifications {
    #[allow(clippy::too_many_arguments)]
    fn notify(
        &self,
        app_name: &str,
        replaces_id: u32,
        app_icon: &str,
        summary: &str,
        body: &str,
        actions: Vec<&str>,
        hints: HashMap<&str, Value<'_>>,
        expire_timeout: i32,
    ) -> zbus::Result<u32>;

    fn close_notification(&self, id: u32) -> zbus::Result<()>;

    #[zbus(signal)]
    fn action_invoked(&self, id: u32, action_key: String) -> zbus::Result<()>;

    #[zbus(signal)]
    fn notification_closed(&self, id: u32, reason: u32) -> zbus::Result<()>;
}

/// Show a yes/no confirmation on the client and block until the user answers.
///
/// Returns `Ok(true)` when approved, and `Ok(false)` when denied, dismissed, or
/// the prompt times out (fail-safe: an unanswered prompt is a denial). Built on
/// `org.freedesktop.Notifications` actions + the `ActionInvoked` signal.
pub async fn confirm(
    message: &str,
    title: Option<&str>,
    host: Option<&str>,
) -> anyhow::Result<bool> {
    use futures_util::StreamExt;

    let connection = Connection::session()
        .await
        .context("failed to connect to session D-Bus")?;
    let proxy = NotificationsProxy::new(&connection)
        .await
        .context("failed to build Notifications proxy")?;

    // Subscribe to the signals BEFORE sending the notification, so a fast click
    // can't land before we start listening.
    let mut invoked = proxy
        .receive_action_invoked()
        .await
        .context("failed to subscribe to ActionInvoked")?;
    let mut closed = proxy
        .receive_notification_closed()
        .await
        .context("failed to subscribe to NotificationClosed")?;

    let app_name = match host {
        Some(h) => format!("subportal@{h}"),
        None => "subportal".to_string(),
    };
    let summary = title.unwrap_or("Approval required");

    let mut hints: HashMap<&str, Value<'_>> = HashMap::new();
    hints.insert("urgency", Value::from(2u8)); // critical

    // Action list is [key, label, key, label, ...].
    let actions = vec!["approve", "Approve", "deny", "Deny"];

    let id = proxy
        .notify(
            &app_name,
            0,                 // replaces_id
            "dialog-question", // app_icon
            summary,
            message,
            actions,
            hints,
            0, // expire_timeout: 0 = never expire; wait for the user
        )
        .await
        .context("failed to show confirmation notification")?;

    // Wait for the matching action or close, bounded so a forgotten prompt
    // eventually resolves as a denial.
    let decision = tokio::time::timeout(CONFIRM_TIMEOUT, async {
        loop {
            tokio::select! {
                Some(sig) = invoked.next() => {
                    if let Ok(args) = sig.args() {
                        if args.id == id {
                            return args.action_key == "approve";
                        }
                    }
                }
                Some(sig) = closed.next() => {
                    if let Ok(args) = sig.args() {
                        if args.id == id {
                            return false;
                        }
                    }
                }
                else => return false,
            }
        }
    })
    .await;

    match decision {
        Ok(approved) => Ok(approved),
        Err(_) => {
            // Timed out: dismiss the stale prompt and treat as denied.
            let _ = proxy.close_notification(id).await;
            Ok(false)
        }
    }
}
