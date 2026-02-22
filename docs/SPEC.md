# subportal specification

> For user-facing documentation, see the [docs index](index.md).
> This file is the design specification for the subportal protocol and
> components.

xdg-desktop-portal, but the sandbox boundary is a network connection.

`subportal` bridges a headless server to the user's local desktop. Server-side
commands like `xdg-open` and `notify-send` transparently forward requests
via iroh (peer-to-peer QUIC) to a client daemon, which handles them using
the local desktop environment.

## V1 Scope

| Capability   | Direction        | Description                                    |
| ------------ | ---------------- | ---------------------------------------------- |
| **OpenURI**  | server -> client  | Open a URL in the client's browser             |
| **OpenFile** | server -> client  | Transfer a file (<5MB) and open it on client   |
| **Notify**   | server -> client  | Show a desktop notification                    |
| **Ping**     | server -> client  | Check connectivity, discover capabilities      |

Deferred to v2: secrets/keychain, clipboard, FileChooser (client -> server),
large file chunked transfer.

## Transport

The agent listens on a Unix domain socket for local tools and on an iroh
endpoint for desktop clients. Default socket path:
`$XDG_RUNTIME_DIR/subportal.sock`.

Server-side commands connect to `$SUBPORTAL_SOCKET` (default
`$XDG_RUNTIME_DIR/subportal.sock`). If the agent is not running,
subportal is unavailable.

Desktop clients connect to the agent via iroh (peer-to-peer QUIC). The
connection is authenticated by endpoint ID (public key) and established
automatically after enrollment.

## Protocol

Varlink over Unix socket (local tools to agent) and Varlink over QUIC
(agent to clients). Each connection is one method call.

All methods accept an optional `host` parameter (string) that identifies the
originating server's hostname. Server-side tools set this automatically via
`gethostname(2)`. The daemon uses it to annotate notifications and logs (e.g.
`subportal@myserver` as the notification app name). In the implementation,
`host` is a transport-level field: the client library injects it into the
JSON before sending, and the server extracts it before deserializing into the
typed request. It is not part of the typed `Request` enum.

```
interface io.subportal

method Ping(host: ?string) -> (capabilities: []string, version: string, clients: []string, endpoint_id: string)

method OpenURI(uri: string, host: ?string) -> ()

method OpenFile(
    name: string,
    mime: string,
    content: string,
    host: ?string
) -> ()

method Notify(
    title: string,
    body: ?string,
    urgency: ?string,
    icon: ?string,
    host: ?string
) -> (id: string)

method NotifyDismiss(id: string) -> ()

method GenerateTicket(ttl: int) -> (ticket_json: string)

method RevokeClient(name_or_id: string) -> ()

```

Errors follow Varlink convention:

```
error io.subportal.UserDenied ()
error io.subportal.NotSupported (capability: string)
error io.subportal.FileTooLarge (max_bytes: int)
error io.subportal.NoClient ()
error io.subportal.NotFound (what: string)
```

File content in `OpenFile` is base64-encoded. 5MB cap for v1.

When the agent forwards a `Notify` request to clients (fan-out), it injects
an additional `notification_id` field into the wire parameters. Clients use
this ID to map their local notification IDs back to the agent-level ID for
cross-device dismiss tracking via `NotifyDismiss`.

## Server-Side Commands

### Drop-in replacements

These are placed higher priority in `$PATH` so existing tools work
transparently.

#### `xdg-open <target>`

- If `target` is a URL -> `OpenURI`
- If `target` is a file -> read file, `OpenFile` with detected MIME type
- If subportal unavailable -> exit with error

#### `notify-send [options] <title> [body]`

- Parses standard `notify-send` flags (`-u`, `-i`, etc.)
- Forwards via `Notify`
- If subportal unavailable -> exit with error

### Explicit CLI

```bash
subportal status          # ping, show capabilities + latency
subportal open <target>   # explicit open
subportal notify ...      # explicit notify
```

### Agent CLI

```bash
subportal agent             # start the agent daemon
subportal ticket [--ttl]    # generate enrollment ticket
subportal clients           # list enrolled clients
subportal revoke <id>       # revoke an enrolled client
```

## Client Daemon -- `subportal-desktop`

Runs on the user's desktop machine. Connects to enrolled agents via iroh.

### Peer Identity

Server-side tools include their hostname in every request via the `host`
parameter (set automatically from `gethostname(2)`). The daemon uses this
to annotate notifications (e.g. `subportal@myserver`) and for logging.

The agent uses `SO_PEERCRED` to obtain the UID of the connecting
process on the Unix socket and rejects connections from UIDs that don't
match its own (except root).

### Request handling

| Method       | Behavior                                                                                     |
| ------------ | -------------------------------------------------------------------------------------------- |
| `Ping`       | Return supported capabilities + version                                                      |
| `OpenURI`    | Show confirmation via xdg-desktop-portal `OpenURI` -> open in browser                        |
| `OpenFile`   | Show confirmation (name, size, MIME) -> save to `$XDG_RUNTIME_DIR/subportal/<name>` -> open it   |
| `Notify`     | Forward to `org.freedesktop.Notifications` D-Bus interface (no confirmation needed)          |

Confirmation dialogs use the local xdg-desktop-portal D-Bus interface. On
Gnome, you get native Gnome dialogs. On KDE, native KDE dialogs.

Notifications use the standard `org.freedesktop.Notifications` D-Bus interface
directly (the same one used by `libnotify`/`notify-send`), rather than the
portal, because the portal requires a discoverable `.desktop` file and
pidfd-based caller identification that is unreliable for non-sandboxed apps.

### Lifecycle

Started via systemd user unit or XDG autostart. Runs persistently.
Connects to all enrolled agents on startup and reconnects automatically.

## Capability Handshake

Desktop clients advertise their supported capabilities in the `ClientHello`
message when they connect to the agent. The agent caches these capabilities
and uses them for routing decisions: if no connected client supports the
requested capability, the agent returns `io.subportal.NoClient` without
forwarding the request.

The `Ping` method also returns the union of all connected clients'
capabilities:

```json
{"capabilities": ["OpenURI", "OpenFile", "Notify"], "version": "0.2.0", "clients": ["laptop"], "endpoint_id": "abc123"}
```

## Security Model

- **Transport**: Encrypted by iroh (QUIC with TLS 1.3). Each endpoint is
  authenticated by its public key.
- **Access control**: Unix socket permissions restrict access to the owning
  user. Only the user who owns the socket can connect, unlike TCP localhost
  which is accessible by any local user.
- **Enrollment**: Desktop clients must present a one-time token to be
  enrolled. After enrollment, they are authenticated by endpoint ID.
- **Server identity**: Server-side tools self-report their hostname via the
  `host` request parameter. `SO_PEERCRED` provides UID-based access control
  on the Unix socket.
- **OpenURI/OpenFile**: User confirmation required before opening.
- **Notify**: No confirmation (passive, low risk).

## Components

| Component           | Runs on         | Language | Description                                          |
| ------------------- | --------------- | -------- | ---------------------------------------------------- |
| `subportal`         | Server          | Rust     | CLI + agent daemon, Unix socket + iroh, routes requests |
| `subportal-desktop` | Client desktop  | Rust     | Client daemon, iroh + xdg-desktop-portal + D-Bus    |
| `xdg-open`          | Server          | Rust     | Drop-in replacement, connects to agent               |
| `notify-send`       | Server          | Rust     | Drop-in replacement                                  |

## NixOS / home-manager / system-manager

The flake exports packages and modules for all three systems.

### Packages

| Flake output | Contents |
| --- | --- |
| `packages.<system>.subportal-desktop` | Client daemon binary |
| `packages.<system>.subportal-server` | Server-side CLI tools (`subportal`, `xdg-open`, `notify-send`) |

### Modules

| Flake output | Type | Description |
| --- | --- | --- |
| `nixosModules.subportal-desktop` | NixOS | systemd user service for the client daemon |
| `nixosModules.subportal` | NixOS | Server-side CLI tools in `environment.systemPackages` |
| `homeModules.subportal-desktop` | home-manager | systemd user service for the client daemon |
| `homeModules.subportal` | home-manager | Server-side CLI tools in `home.packages` |
| `modules.system-manager.subportal-desktop` | system-manager | systemd system service for the client daemon |
| `modules.system-manager.subportal` | system-manager | Server-side CLI tools in `environment.systemPackages` |

### Client setup (desktop machine)

NixOS:

```nix
{ inputs, ... }:
{
  imports = [ inputs.subportal.nixosModules.subportal-desktop ];
  services.subportal-desktop.enable = true;
}
```

home-manager:

```nix
{ inputs, ... }:
{
  imports = [ inputs.subportal.homeModules.subportal-desktop ];
  services.subportal-desktop.enable = true;
}
```

### Server setup (remote host)

NixOS:

```nix
{ inputs, ... }:
{
  imports = [ inputs.subportal.nixosModules.subportal ];
  programs.subportal.enable = true;
  programs.subportal.agent.enable = true;
  # programs.subportal.xdg-open = true;     # install xdg-open drop-in
  # programs.subportal.notify-send = true;   # install notify-send drop-in
}
```

home-manager:

```nix
{ inputs, ... }:
{
  imports = [ inputs.subportal.homeModules.subportal ];
  programs.subportal.enable = true;
}
```

The `xdg-open` and `notify-send` drop-in replacements are installed by default.
Set `programs.subportal.xdg-open = false` or `programs.subportal.notify-send = false`
to disable them.

## V2 Roadmap

- Secrets/keychain forwarding (secret-tool drop-in)
- Clipboard forwarding
- FileChooser (client -> server file picker)
- Chunked file transfer (large files)
- Multiple agent connections (subportal-desktop manages several agents)
