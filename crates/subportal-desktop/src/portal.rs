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

/// How long a confirmation prompt waits for the user before giving up.
const CONFIRM_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(120);

/// `NotificationClosed` reason code for "dismissed by the user" — the only
/// close that is an actual answer. 1 = expired, 3 = CloseNotification call.
/// https://specifications.freedesktop.org/notification-spec/latest/protocol.html#signal-notification-closed
const REASON_DISMISSED: u32 = 2;

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

/// The outcome of a confirmation prompt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfirmDecision {
    Approved,
    Denied,
    /// Prompt ended without a user answer (expired, superseded, timed out).
    /// Must not veto an answer from another device.
    NoDecision,
}

#[derive(Debug)]
enum ConfirmEvent {
    Invoked { id: u32, action: String },
    Closed { id: u32, reason: u32 },
}

/// Resolve a confirmation from a single ordered stream of notification events.
///
/// A click emits ActionInvoked then NotificationClosed, and D-Bus preserves
/// per-sender order — but only within one stream. So a close seen before any
/// action means nobody clicked. Two separate signal subscriptions lose that
/// ordering; do not go back to them.
async fn await_decision<S>(mut events: S, id: u32) -> ConfirmDecision
where
    S: futures_util::Stream<Item = ConfirmEvent> + Unpin,
{
    use futures_util::StreamExt;

    while let Some(ev) = events.next().await {
        match ev {
            ConfirmEvent::Invoked { id: eid, action } if eid == id => {
                return if action == "approve" {
                    ConfirmDecision::Approved
                } else {
                    ConfirmDecision::Denied
                };
            }
            ConfirmEvent::Closed { id: eid, reason } if eid == id => {
                // Swipe-away is a "no"; expiry or programmatic close is not.
                return if reason == REASON_DISMISSED {
                    ConfirmDecision::Denied
                } else {
                    ConfirmDecision::NoDecision
                };
            }
            _ => {} // another notification's event
        }
    }
    ConfirmDecision::NoDecision // bus connection died
}

/// Show a yes/no confirmation on the client and block until the user answers.
///
/// Built on `org.freedesktop.Notifications` actions; see [`await_decision`]
/// for why the signals must come through a single stream.
pub async fn confirm(
    message: &str,
    title: Option<&str>,
    host: Option<&str>,
) -> anyhow::Result<ConfirmDecision> {
    use futures_util::StreamExt;

    let connection = Connection::session()
        .await
        .context("failed to connect to session D-Bus")?;
    let proxy = NotificationsProxy::new(&connection)
        .await
        .context("failed to build Notifications proxy")?;

    // Subscribe BEFORE sending the notification, so a fast click can't land
    // before we start listening. One rule, one stream: bus-delivery order.
    let rule = zbus::MatchRule::builder()
        .msg_type(zbus::message::Type::Signal)
        .interface("org.freedesktop.Notifications")
        .context("bad interface in match rule")?
        .path("/org/freedesktop/Notifications")
        .context("bad path in match rule")?
        .build();
    let messages = zbus::MessageStream::for_match_rule(rule, &connection, Some(64))
        .await
        .context("failed to subscribe to notification signals")?;

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

    let events = std::pin::pin!(messages.filter_map(|msg| async move {
        let msg = msg.ok()?;
        let header = msg.header();
        match header.member()?.as_str() {
            "ActionInvoked" => {
                let (id, action): (u32, String) = msg.body().deserialize().ok()?;
                Some(ConfirmEvent::Invoked { id, action })
            }
            "NotificationClosed" => {
                let (id, reason): (u32, u32) = msg.body().deserialize().ok()?;
                Some(ConfirmEvent::Closed { id, reason })
            }
            _ => None,
        }
    }));

    let decision = tokio::time::timeout(CONFIRM_TIMEOUT, await_decision(events, id)).await;

    match decision {
        Ok(decision) => Ok(decision),
        Err(_) => {
            // Timed out: clear the stale prompt.
            let _ = proxy.close_notification(id).await;
            Ok(ConfirmDecision::NoDecision)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures_util::stream;
    use ConfirmDecision::{Approved, Denied, NoDecision};

    const REASON_EXPIRED: u32 = 1;
    const REASON_CLOSED_BY_CALL: u32 = 3;

    fn invoked(id: u32, action: &str) -> ConfirmEvent {
        ConfirmEvent::Invoked {
            id,
            action: action.into(),
        }
    }

    fn closed(id: u32, reason: u32) -> ConfirmEvent {
        ConfirmEvent::Closed { id, reason }
    }

    async fn decide(events: Vec<ConfirmEvent>) -> ConfirmDecision {
        await_decision(stream::iter(events), 1).await
    }

    #[tokio::test]
    async fn approve_action_approves() {
        assert_eq!(decide(vec![invoked(1, "approve")]).await, Approved);
    }

    #[tokio::test]
    async fn deny_action_denies() {
        assert_eq!(decide(vec![invoked(1, "deny")]).await, Denied);
    }

    /// The original bug: a click's own close event must never outrun the click.
    #[tokio::test]
    async fn action_wins_over_its_trailing_close() {
        let d = decide(vec![invoked(1, "approve"), closed(1, REASON_DISMISSED)]).await;
        assert_eq!(d, Approved);
    }

    #[tokio::test]
    async fn user_dismissal_denies() {
        assert_eq!(decide(vec![closed(1, REASON_DISMISSED)]).await, Denied);
    }

    #[tokio::test]
    async fn expiry_is_no_decision() {
        assert_eq!(decide(vec![closed(1, REASON_EXPIRED)]).await, NoDecision);
    }

    #[tokio::test]
    async fn programmatic_close_is_no_decision() {
        assert_eq!(decide(vec![closed(1, REASON_CLOSED_BY_CALL)]).await, NoDecision);
    }

    #[tokio::test]
    async fn other_notification_events_are_ignored() {
        let d = decide(vec![
            invoked(2, "approve"),
            closed(2, REASON_DISMISSED),
            invoked(1, "deny"),
        ])
        .await;
        assert_eq!(d, Denied);
    }

    #[tokio::test]
    async fn stream_end_is_no_decision() {
        assert_eq!(decide(vec![]).await, NoDecision);
    }
}
