# subportal

xdg-desktop-portal, but the sandbox boundary is an SSH connection.

`subportal` bridges a headless SSH server to the user's local desktop. Server-side
commands like `xdg-open` and `notify-send` transparently forward requests
through an SSH tunnel to a client daemon, which handles them using the local
desktop environment.

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

Unix domain socket over SSH reverse socket forwarding. Default socket path:
`$XDG_RUNTIME_DIR/subportal/subportal.sock`.

Client `~/.ssh/config`:

```
Host myserver
    RemoteForward /run/user/1000/subportal/subportal.sock /run/user/1000/subportal/subportal.sock
```

Server-side commands connect to `$SUBPORTAL_SOCKET` (default
`$XDG_RUNTIME_DIR/subportal/subportal.sock`). If nothing is listening,
subportal is unavailable.

No server-side SSH config changes required.

Unix-to-Unix socket forwarding requires OpenSSH 6.7+ (released 2014).

## Protocol

Varlink over Unix socket. Each connection is one method call.

```
interface io.subportal

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
error io.subportal.UserDenied ()
error io.subportal.NotSupported (capability: string)
error io.subportal.FileTooLarge (max_bytes: int)
error io.subportal.NoClient ()
```

File content in `OpenFile` is base64-encoded. 5MB cap for v1.

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
- If subportal unavailable -> silently fail (notifications are best-effort)

### Explicit CLI

```bash
subportal status          # ping, show capabilities + latency
subportal open <target>   # explicit open
subportal notify ...      # explicit notify
```

## Client Daemon -- `subportald`

Runs on the user's desktop machine. Listens on
`$XDG_RUNTIME_DIR/subportal/subportal.sock`.

### Peer Identity

When accepting a connection, `subportald` uses `SO_PEERCRED` to obtain the
PID of the connecting process. It then walks the process tree via
`/proc/<pid>/cmdline` to find an `sshd` parent process and extract the SSH
remote host (e.g. `sshd: user@1.2.3.4`). This information is logged with
each request and can be used for per-server trust policies.

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

## Capability Handshake

On first connection (or via `subportal status`), server calls `Ping`. Client
responds with supported capabilities:

```json
{"capabilities": ["OpenURI", "OpenFile", "Notify"], "version": "1.0"}
```

Server caches this for the session. If a command tries an unsupported
capability, it gets `io.subportal.NotSupported` without a round-trip.

## Security Model

- **Transport**: Encrypted by SSH. No additional encryption needed.
- **Access control**: Unix socket permissions restrict access to the owning
  user. Only the user who owns the socket can connect, unlike TCP localhost
  which is accessible by any local user.
- **Server identity**: `SO_PEERCRED` on the Unix socket provides the PID of
  the connecting process. The daemon resolves this to the SSH remote host via
  `/proc/<pid>/cmdline`, enabling per-server logging and trust policies.
- **OpenURI/OpenFile**: User confirmation required before opening.
- **Notify**: No confirmation (passive, low risk).

### Trust configuration

Optional `~/.config/subportal/trust.toml`:

```toml
[servers.myserver]
auto_open_urls = false    # still confirm
auto_open_files = false   # still confirm
```

## Components

| Component     | Runs on         | Language | Description                                          |
| ------------- | --------------- | -------- | ---------------------------------------------------- |
| `subportald`     | Client desktop  | Rust     | Daemon, listens on Unix socket, talks to xdg-desktop-portal and D-Bus  |
| `xdg-open`    | Server          | Rust     | Drop-in replacement, connects to subportal              |
| `notify-send` | Server          | Rust     | Drop-in replacement                                  |
| `subportal`   | Server          | Rust     | Explicit CLI for all capabilities + status             |

## NixOS / home-manager / system-manager

The flake exports packages and modules for all three systems.

### Packages

| Flake output | Contents |
| --- | --- |
| `packages.<system>.subportald` | Client daemon binary |
| `packages.<system>.subportal` | Server-side CLI tools (`subportal`, `xdg-open`, `notify-send`) |

### Modules

| Flake output | Type | Description |
| --- | --- | --- |
| `nixosModules.subportald` | NixOS | systemd user service for the client daemon |
| `nixosModules.subportal` | NixOS | Server-side CLI tools in `environment.systemPackages` |
| `homeModules.subportald` | home-manager | systemd user service for the client daemon |
| `homeModules.subportal` | home-manager | Server-side CLI tools in `home.packages` |
| `modules.system-manager.subportald` | system-manager | systemd system service for the client daemon |
| `modules.system-manager.subportal` | system-manager | Server-side CLI tools in `environment.systemPackages` |

### Client setup (desktop machine)

NixOS:

```nix
{ inputs, ... }:
{
  imports = [ inputs.subportal.nixosModules.subportald ];
  services.subportald.enable = true;
  # services.subportald.socketPath = "%t/subportal/subportal.sock";  # default
  # services.subportald.sshHosts = [ "myserver" ];      # auto-configure RemoteForward
}
```

home-manager:

```nix
{ inputs, ... }:
{
  imports = [ inputs.subportal.homeModules.subportald ];
  services.subportald.enable = true;
  # services.subportald.sshHosts = [ "myserver" ];      # auto-configure RemoteForward
}
```

### Server setup (remote SSH host)

NixOS:

```nix
{ inputs, ... }:
{
  imports = [ inputs.subportal.nixosModules.subportal ];
  programs.subportal.enable = true;
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
- Multiple server connections (subportald manages several tunnels)
