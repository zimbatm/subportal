# Architecture

## Overview

subportal bridges a headless server to the user's local desktop. The core
idea is borrowed from xdg-desktop-portal: applications talk to a daemon over
a Unix socket, and the daemon handles the interaction with the desktop
environment. The difference is that the boundary is a network connection
(via iroh, peer-to-peer QUIC) instead of a Flatpak/container sandbox.

```
Server (headless)                         Client (desktop)
                                          +------------------+
+-------------+                           | subportal-desktop|
|  xdg-open   |--+                        |                  |
+-------------+  |  +------------------+  |  +--------+      |
| notify-send |--+--| subportal agent  |====|  portal  |     |
+-------------+  |  |  (Unix socket    |  |  |  D-Bus  |     |
|  subportal  |--+  |   + iroh QUIC)   |  |  +--------+      |
+-------------+     +------------------+  +------------------+
```

## Components

The project has five crates, split into two installable packages:

### subportal-server (server-side package)

Contains three binaries, all installed on the remote server:

- **subportal** -- CLI with `status`, `open`, `notify` subcommands for
  user-facing operations, plus `agent`, `ticket`, `clients`, and `revoke`
  subcommands for managing the agent daemon and enrolled clients. The
  `agent` subcommand starts the daemon that bridges local tools to remote
  desktop clients. It listens on a Unix socket for local requests and on an
  iroh endpoint for client connections.

- **xdg-open** -- drop-in replacement for the standard `xdg-open`. Placed
  earlier in `$PATH` so that existing tools and scripts work transparently.

- **notify-send** -- drop-in replacement for the standard `notify-send`.
  Same `$PATH` shadowing strategy. Parses the standard flags for
  compatibility.

All CLI tools use the shared library to connect to the agent.

### subportal-desktop (client-side package)

A single daemon binary that runs on the user's desktop machine. It:

1. Connects to enrolled agents via iroh (peer-to-peer QUIC)
2. Accepts requests forwarded by the agent
3. Dispatches requests to the appropriate desktop interface
4. Reports focus state (active/idle) back to the agent

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

### subportal-iroh (shared library)

Not distributed as a separate package. Contains iroh-specific code shared
between the agent and client daemon:

- **transport** -- Varlink-over-QUIC request/response I/O
- **control** -- Control channel messages (focus updates, dismiss notifications)
- **peers** -- Client and server registries (persistent enrollment data)
- **keypair** -- iroh keypair generation and loading
- **ticket** -- Enrollment ticket serialization

## Data flow

A typical request follows this path:

1. **Server-side tool** (e.g. `xdg-open https://example.com`) creates a
   `Request::OpenURI`.

2. The **client library** connects to the Unix socket at the configured path,
   serializes the request as NUL-delimited JSON, and sends it.

3. The **subportal agent** receives the request, determines which connected
   client(s) should handle it (routing), and forwards it over iroh QUIC.

4. **subportal-desktop** receives the request on its iroh connection and validates it.

5. The **handler** dispatches the request to the appropriate **portal**
   function.

6. The **portal** module calls the xdg-desktop-portal D-Bus API (for
   OpenURI/OpenFile) or the `org.freedesktop.Notifications` D-Bus interface
   (for Notify).

7. The desktop environment shows a confirmation dialog or notification.

8. The response travels back through the same path.

## Connection model

Server-side CLI tools use a one-shot connection model: each request opens a
new Unix socket connection, sends one request, receives one response, and
closes the connection.

The agent maintains persistent iroh connections to all enrolled desktop
clients. Requests received on the Unix socket are routed to the appropriate
client(s) over these persistent connections.

## Routing

The agent chooses a strategy per request type. Candidate clients are ranked by
a deterministic total order -- active focus first, then most-recently-active,
then a stable id tiebreak -- so with several clients connected the choice is
never arbitrary.

- **Failover** (OpenURI, OpenFile) -- sent to the single best client, failing
  over to the next *only* if a device is unreachable. A user decision (approve
  or deny) is final and returned as-is, not retried elsewhere.
- **Race** (Confirm) -- sent to every capable client at once; the first user
  *decision* wins, so you can approve from whichever device you're actually at.
  A transport failure doesn't count as a decision. (Losing dialogs currently
  clear on their own client-side timeout; a dedicated cancel message is a
  follow-up.)
- **FanOut** (Notify) -- sent to all connected clients with the capability.
- **Direct** (Ping, GenerateTicket, RevokeClient) -- handled by the agent
  itself without forwarding to clients.

## Enrollment

Desktop clients are enrolled with the agent using one-time tickets:

1. The agent generates a ticket containing its iroh endpoint address and a
   one-time token
2. The ticket is transferred to the desktop (e.g. via `ssh myserver
   subportal ticket | subportal-desktop enroll`)
3. The desktop client connects to the agent, presents the token, and is
   enrolled in the persistent registry
4. On subsequent starts, the client reconnects automatically using the
   stored endpoint address

## Desktop integration

subportal-desktop uses two separate D-Bus interfaces:

### xdg-desktop-portal (for OpenURI and OpenFile)

The [xdg-desktop-portal](https://flatpak.github.io/xdg-desktop-portal/)
provides a desktop-neutral API for file and URI opening. It handles
confirmation dialogs natively -- on GNOME you get GNOME dialogs, on KDE you
get KDE dialogs.

subportal-desktop uses the [ashpd](https://crates.io/crates/ashpd) Rust library to
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
