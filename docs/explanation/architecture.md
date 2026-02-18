# Architecture

## Overview

subportal bridges a headless SSH server to the user's local desktop. The core
idea is borrowed from xdg-desktop-portal: applications talk to a daemon over
a Unix socket, and the daemon handles the interaction with the desktop
environment. The difference is that the sandbox boundary is an SSH connection
instead of a Flatpak/container sandbox.

```
Server (headless)                         Client (desktop)
                                          ┌─────────────┐
┌────────────┐                            │  subportald  │
│  xdg-open  │──┐                         │              │
├────────────┤  │  ┌──────────────────┐   │  ┌────────┐  │
│ notify-send│──┼──│ subportal.sock   │───│──│ portal  │  │
├────────────┤  │  │  (Unix socket)   │   │  │ D-Bus   │  │
│  subportal │──┘  └──────────────────┘   │  └────────┘  │
└────────────┘     forwarded by SSH       └─────────────┘
```

## Components

The project has five crates, split into two installable packages:

### subportal (server-side package)

Contains three binaries, all installed on the remote SSH host:

- **subportal** -- explicit CLI with `status`, `open`, and `notify`
  subcommands. Used when you want explicit control or diagnostic output
  (latency, capabilities).

- **xdg-open** -- drop-in replacement for the standard `xdg-open`. Placed
  earlier in `$PATH` so that existing tools and scripts work transparently.

- **notify-send** -- drop-in replacement for the standard `notify-send`.
  Same `$PATH` shadowing strategy. Parses the standard flags for
  compatibility.

All three use the shared library to connect to the daemon.

### subportald (client-side package)

A single daemon binary that runs on the user's desktop machine. It:

1. Binds to a Unix domain socket
2. Accepts one connection at a time (though connections are handled
   concurrently via tokio tasks)
3. Validates the peer UID via `SO_PEERCRED`
4. Dispatches requests to the appropriate desktop interface

### subportal-lib (shared library)

Not distributed as a separate package. Contains:

- **protocol** -- Varlink request/response types, wire format I/O
  (NUL-delimited JSON), error types, and conversions between typed and wire
  representations.

- **client** -- Unix socket client. Creates a new connection for each
  request (one-shot model). Resolves the socket path from
  `$SUBPORTAL_SOCKET` or `$XDG_RUNTIME_DIR/subportal.sock`. Injects the
  local hostname into every request.

- **server** -- Unix socket server. Handles socket binding, stale socket
  cleanup, connection acceptance, `SO_PEERCRED` extraction, and host
  parameter extraction from requests.

- **consts** -- Protocol version, size limits, socket path defaults.

## Data flow

A typical request follows this path:

1. **Server-side tool** (e.g. `xdg-open https://example.com`) creates a
   `Request::OpenURI`.

2. The **client library** connects to the Unix socket at the configured path,
   serializes the request as NUL-delimited JSON, and sends it.

3. The socket is actually an **SSH reverse forward**. SSH transports the
   bytes over the encrypted SSH connection to the desktop machine.

4. **subportald** accepts the connection, reads the request, and validates
   the peer UID.

5. The **handler** dispatches the request to the appropriate **portal**
   function.

6. The **portal** module calls the xdg-desktop-portal D-Bus API (for
   OpenURI/OpenFile) or the `org.freedesktop.Notifications` D-Bus interface
   (for Notify).

7. The desktop environment shows a confirmation dialog or notification.

8. The response travels back through the same path.

## Connection model

subportal uses a one-shot connection model: each request opens a new Unix
socket connection, sends one request, receives one response, and closes the
connection. This is simple and avoids state management, connection pooling,
or multiplexing.

The overhead is minimal because Unix sockets are local (or tunneled through
an already-established SSH connection).

## Desktop integration

subportald uses two separate D-Bus interfaces:

### xdg-desktop-portal (for OpenURI and OpenFile)

The [xdg-desktop-portal](https://flatpak.github.io/xdg-desktop-portal/)
provides a desktop-neutral API for file and URI opening. It handles
confirmation dialogs natively -- on GNOME you get GNOME dialogs, on KDE you
get KDE dialogs.

subportald uses the [ashpd](https://crates.io/crates/ashpd) Rust library to
talk to the portal.

### org.freedesktop.Notifications (for Notify)

Notifications use the standard
[Desktop Notifications Specification](https://specifications.freedesktop.org/notification-spec/)
D-Bus interface directly, rather than going through xdg-desktop-portal.

The portal's notification API requires a `.desktop` file and pidfd-based
caller identification, which is unreliable for non-sandboxed applications.
The direct D-Bus interface works everywhere.

## Hostname identification

Server-side tools call `gethostname(2)` and include the result as the `host`
parameter in every request. The daemon uses this to:

- Set the notification app name to `subportal@<hostname>` so the user can
  see which server a notification came from
- Log the originating server for debugging

This is a self-reported value. It is not cryptographically verified. See the
[security model](security.md) for the trust implications.
