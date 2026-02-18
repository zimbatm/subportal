# Troubleshooting subportal

This guide covers common problems and how to diagnose them.

## Quick diagnosis

Run `subportal status` on the remote server. It checks the full path from
server-side tool through the SSH tunnel to the client daemon.

```sh
subportal status
```

Healthy output:

```
subportald v0.1.0
latency: 12.3ms
capabilities: OpenURI, OpenFile, Notify
```

## "Connection refused" or "No such file or directory"

**Symptom:** `subportal status` fails immediately.

**The socket file does not exist on the server:**

```sh
ls -la $XDG_RUNTIME_DIR/subportal.sock
```

If the file is missing, the SSH tunnel is not active.

Causes:

1. You did not SSH with `RemoteForward` configured. Check your
   `~/.ssh/config` or add `-R` to the ssh command. See
   [SSH setup](ssh-setup.md).
2. You connected before the daemon was running. SSH creates the remote socket
   at connection time. Disconnect and reconnect after starting `subportald`.
3. The remote `$XDG_RUNTIME_DIR` does not exist. Verify with `echo
   $XDG_RUNTIME_DIR` on the server. It is typically `/run/user/<uid>` and
   requires an active login session (e.g., via `loginctl enable-linger`).

**The socket file exists but nothing responds:**

The daemon may not be running on your desktop.

```sh
# On your desktop:
systemctl --user status subportald
```

Or check if the process is running:

```sh
pgrep subportald
```

## "Address already in use"

**Symptom:** SSH logs `Warning: remote port forwarding failed for listen path`
when connecting.

A stale socket from a previous session was not cleaned up.

**Fix:** Ensure the server's `sshd_config` includes:

```
StreamLocalBindUnlink yes
```

Then reload sshd:

```sh
sudo systemctl reload sshd
```

Alternatively, remove the stale socket manually on the server:

```sh
rm $XDG_RUNTIME_DIR/subportal.sock
```

Then reconnect via SSH.

## "Permission denied"

**Symptom:** `subportal status` returns a permission error.

The daemon uses `SO_PEERCRED` to verify that the connecting process runs as
the same UID. This fails if:

1. The socket is owned by a different user.
2. The daemon was started as a different user than the one SSH is
   forwarding to.

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
3. Check `subportald` logs for errors:
   ```sh
   journalctl --user -u subportald -f
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

`subportal status` reports the round-trip latency through the SSH tunnel. High
latency reflects the SSH connection quality. There is no subportal-specific
tuning -- improve your SSH connection (compression, KeepAlive settings, or a
closer server).

## Enabling debug logging

Set the `RUST_LOG` environment variable for verbose output:

```sh
# Server-side:
RUST_LOG=debug subportal status

# Client daemon (restart with):
RUST_LOG=debug subportald
```

Or if using systemd:

```sh
systemctl --user stop subportald
RUST_LOG=debug subportald
```

Then reproduce the issue and examine the output.

## Checking the SUBPORTAL_SOCKET path

If you use a non-default socket path, make sure both sides agree:

```sh
# What the server-side tools are using:
echo ${SUBPORTAL_SOCKET:-$XDG_RUNTIME_DIR/subportal.sock}

# What the daemon is listening on (check the process arguments):
ps aux | grep subportald
```

The SSH `RemoteForward` must map the server path to the client path.
