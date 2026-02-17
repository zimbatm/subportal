# portal

xdg-desktop-portal, but the sandbox boundary is an SSH connection.

`portal` bridges a headless SSH server to the user's local desktop. Server-side
commands like `xdg-open` and `notify-send` transparently forward requests
through an SSH tunnel to a client daemon, which handles them using the local
desktop environment.

## V1 Scope

| Capability   | Direction        | Description                                    |
| ------------ | ---------------- | ---------------------------------------------- |
| **OpenURI**  | server → client  | Open a URL in the client's browser             |
| **OpenFile** | server → client  | Transfer a file (<5MB) and open it on client   |
| **Notify**   | server → client  | Show a desktop notification                    |
| **Ping**     | server → client  | Check connectivity, discover capabilities      |

Deferred to v2: secrets/keychain, clipboard, FileChooser (client → server),
large file chunked transfer.

## Transport

TCP over SSH reverse port forwarding. Default port `19494`.

Client `~/.ssh/config`:

```
Host myserver
    RemoteForward 127.0.0.1:19494 127.0.0.1:19494
```

Server-side commands connect to `localhost:$PORTAL_PORT` (default `19494`). If
nothing is listening, portal is unavailable.

No server-side SSH config changes required.

## Protocol

Varlink over TCP. Each connection is one method call.

```
interface io.portal

method Ping() -> (capabilities: []string, version: string)

method OpenURI(uri: string) -> ()

method OpenFile(
    name: string,
    mime: string,
    content: string
) -> ()

method Notify(
    title: string,
    body: ?string,
    urgency: ?string,
    icon: ?string
) -> ()

```

Errors follow Varlink convention:

```
error io.portal.UserDenied ()
error io.portal.NotSupported (capability: string)
error io.portal.FileTooLarge (max_bytes: int)
error io.portal.NoClient ()
```

File content in `OpenFile` is base64-encoded. 5MB cap for v1.

## Server-Side Commands

### Drop-in replacements

These are placed higher priority in `$PATH` so existing tools work
transparently.

#### `xdg-open <target>`

- If `target` is a URL → `OpenURI`
- If `target` is a file → read file, `OpenFile` with detected MIME type
- If portal unavailable → fall back to real `xdg-open`, then error

#### `notify-send [options] <title> [body]`

- Parses standard `notify-send` flags (`-u`, `-i`, etc.)
- Forwards via `Notify`
- If portal unavailable → fall back to real `notify-send`, then silently fail
  (notifications are best-effort)

### Explicit CLI

```bash
portal status          # ping, show capabilities + latency
portal drain           # process queued requests
portal open <target>   # explicit open
portal notify ...      # explicit notify
```

## Client Daemon — `portald`

Runs on the user's desktop machine. Listens on `127.0.0.1:19494`.

### Request handling

| Method       | Behavior                                                                                     |
| ------------ | -------------------------------------------------------------------------------------------- |
| `Ping`       | Return supported capabilities + version                                                      |
| `OpenURI`    | Show confirmation via xdg-desktop-portal `OpenURI` → open in browser                        |
| `OpenFile`   | Show confirmation (name, size, MIME) → save to `$XDG_RUNTIME_DIR/portal/<name>` → open it   |
| `Notify`     | Forward to xdg-desktop-portal `AddNotification` (no confirmation needed)                     |

Confirmation dialogs use the local xdg-desktop-portal D-Bus interface. On
Gnome, you get native Gnome dialogs. On KDE, native KDE dialogs.

### Lifecycle

Started via systemd user unit or XDG autostart. Runs persistently.

## Queue (Server-Side)

When portal can't reach `localhost:$PORTAL_PORT`:

- **`notify-send`**: Queue to `~/.local/share/portal/queue/`. Silent.
- **`xdg-open`**: Queue and print "Queued — will open when portal connects."

`portal drain` replays the queue. Client shows a summary notification:
"3 queued items from myserver" with an action to review them.

## Capability Handshake

On first connection (or via `portal status`), server calls `Ping`. Client
responds with supported capabilities:

```json
{"capabilities": ["OpenURI", "OpenFile", "Notify"], "version": "1.0"}
```

Server caches this for the session. If a command tries an unsupported
capability, it gets `io.portal.NotSupported` without a round-trip.

## Security Model

- **Transport**: Encrypted by SSH. No additional encryption needed.
- **OpenURI/OpenFile**: User confirmation required before opening.
- **Notify**: No confirmation (passive, low risk).

### Trust configuration

Optional `~/.config/portal/trust.toml`:

```toml
[servers.myserver]
auto_open_urls = false    # still confirm
auto_open_files = false   # still confirm
```

## Components

| Component     | Runs on         | Language | Description                                          |
| ------------- | --------------- | -------- | ---------------------------------------------------- |
| `portald`     | Client desktop  | Python   | Daemon, listens on TCP, talks to xdg-desktop-portal  |
| `xdg-open`    | Server          | Python   | Drop-in replacement, connects to portal              |
| `notify-send` | Server          | Python   | Drop-in replacement                                  |
| `portal`      | Server          | Python   | Explicit CLI for all capabilities + status/drain      |

## V2 Roadmap

- Secrets/keychain forwarding (secret-tool drop-in)
- Clipboard forwarding
- FileChooser (client → server file picker)
- Chunked file transfer (large files)
- Multiple server connections (portald manages several tunnels)
