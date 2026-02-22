# Troubleshooting subportal

This guide covers common problems and how to diagnose them.

## Quick diagnosis

Run `subportal status` on the remote server. It checks the full path from
server-side tool through the agent to the client daemon.

```sh
subportal status
```

Healthy output:

```
subportal v0.2.0
latency: 12.3ms
capabilities: OpenURI, OpenFile, Notify
```

## "Connection refused" or "No such file or directory"

**Symptom:** `subportal status` fails immediately.

**The socket file does not exist on the server:**

```sh
ls -la $XDG_RUNTIME_DIR/subportal.sock
```

If the file is missing, the agent is not running.

Causes:

1. The agent is not running. Start it with:
   ```sh
   subportal agent
   ```
   Or enable the systemd service.
2. The `$XDG_RUNTIME_DIR` does not exist. Verify with `echo
   $XDG_RUNTIME_DIR` on the server. It is typically `/run/user/<uid>` and
   requires an active login session (e.g., via `loginctl enable-linger`).

**The socket file exists but nothing responds:**

The agent may have crashed. Check its logs:

```sh
journalctl --user -u subportal-agent -f
```

Or if running manually, check the terminal output.

## No clients connected

**Symptom:** `subportal status` reports "no client daemon reachable".

```sh
subportal clients
```

If no clients are listed, you need to enroll a desktop client. See
[enrollment](enrollment.md).

If clients are listed but disconnected, check:

1. The client daemon (`subportal-desktop`) is running on your desktop:
   ```sh
   systemctl --user status subportal-desktop
   ```
2. Both machines have internet connectivity
3. No firewall is blocking QUIC (UDP) traffic

## "Permission denied"

**Symptom:** `subportal status` returns a permission error.

The agent uses `SO_PEERCRED` to verify that the connecting process runs as
the same UID. This fails if:

1. The socket is owned by a different user.
2. The agent was started as a different user.

Verify UIDs match:

```sh
# On the server:
id -u
stat -c '%U' $XDG_RUNTIME_DIR/subportal.sock
```

## Notifications do not appear

**Symptom:** `notify-send` exits successfully but nothing shows on screen.

1. Check that your desktop environment's notification daemon is running (e.g.,
   `mako`, `dunst`, `gnome-shell`).
2. Check D-Bus:
   ```sh
   # On your desktop:
   dbus-send --session --dest=org.freedesktop.Notifications \
     --type=method_call --print-reply \
     /org/freedesktop/Notifications \
     org.freedesktop.Notifications.GetServerInformation
   ```
3. Check `subportal-desktop` logs for errors:
   ```sh
   journalctl --user -u subportal-desktop -f
   ```

## xdg-open confirmation is never shown

**Symptom:** `xdg-open` hangs or times out without showing a dialog.

xdg-desktop-portal must be running on your desktop. Check:

```sh
# On your desktop:
systemctl --user status xdg-desktop-portal
```

If it is not running, install and enable it. Your desktop environment should
provide a portal backend (e.g., `xdg-desktop-portal-gtk`,
`xdg-desktop-portal-kde`, `xdg-desktop-portal-wlr`).

## File transfer fails with "file too large"

subportal has a 5 MB file size limit in v1. Files larger than this are
rejected with `io.subportal.FileTooLarge`.

Workaround: use `scp`, `rsync`, or similar to transfer large files.

## High latency

`subportal status` reports the round-trip latency through the iroh
connection. High latency may reflect:

- Geographic distance between client and server
- Network congestion
- Relay usage (if direct connection cannot be established)

iroh attempts direct peer-to-peer connections but falls back to relay
servers when both endpoints are behind NAT.

## Enabling debug logging

Set the `RUST_LOG` environment variable for verbose output:

```sh
# Server-side:
RUST_LOG=debug subportal status

# Agent:
RUST_LOG=debug subportal agent

# Client daemon (restart with):
RUST_LOG=debug subportal-desktop
```

Or if using systemd:

```sh
systemctl --user stop subportal-desktop
RUST_LOG=debug subportal-desktop
```

Then reproduce the issue and examine the output.

## Checking the SUBPORTAL_SOCKET path

If you use a non-default socket path, make sure both the agent and CLI
tools agree:

```sh
# What the server-side tools are using:
echo ${SUBPORTAL_SOCKET:-$XDG_RUNTIME_DIR/subportal.sock}

# What the agent is listening on (check the process arguments):
ps aux | grep 'subportal agent'
```
