# subportal

xdg-desktop-portal, but the sandbox boundary is an SSH connection.

`subportal` bridges a headless SSH server to your local desktop. Server-side
commands like `xdg-open` and `notify-send` transparently forward requests
through an SSH tunnel to a client daemon, which handles them using the local
desktop environment.

Open a URL on a remote server and it appears in your local browser. Send a
notification and it pops up on your desktop. All through your existing SSH
connection.

## How it works

```
Server (headless)                       Client (your desktop)
                                        subportald (daemon)
xdg-open https://example.com  ─────>     -> confirmation dialog
notify-send "Build done"      ─(unix)─>  -> desktop notification
subportal open ./report.pdf   ─(sock)─>  -> opens in PDF viewer
```

The server-side tools connect to a Unix domain socket
(`$XDG_RUNTIME_DIR/subportal/subportal.sock`), which SSH reverse-forwards to
the client daemon (`subportald`). The daemon uses
[xdg-desktop-portal](https://flatpak.github.io/xdg-desktop-portal/) D-Bus
APIs to show native dialogs and notifications on whatever desktop environment
you run (GNOME, KDE, Sway, ...).

The wire protocol is [Varlink](https://varlink.org/) over a Unix socket -- one
JSON message per connection, NUL-delimited.

## Prerequisites

- A Linux desktop with xdg-desktop-portal (GNOME, KDE, Sway, etc.)
- An SSH connection to the remote server (OpenSSH 6.7+ for Unix socket forwarding)
- Rust toolchain (for building from source)

## Building

With Nix:

```sh
nix develop  # enter dev shell
cargo build --release
```

Without Nix, make sure you have `cargo`, `pkg-config`, and `libdbus` development
headers installed, then:

```sh
cargo build --release
```

Binaries are placed in `target/release/`:

| Binary | Description |
|---|---|
| `subportald` | Client daemon -- runs on your desktop |
| `subportal` | CLI with explicit `status`, `open`, and `notify` commands |
| `xdg-open` | Drop-in replacement for the standard `xdg-open` |
| `notify-send` | Drop-in replacement for the standard `notify-send` |

## SSH setup

Configure SSH to reverse-forward the subportal Unix socket from the server
back to your desktop. Add this to your `~/.ssh/config`:

```
Host myserver
    RemoteForward /run/user/1000/subportal/subportal.sock /run/user/1000/subportal/subportal.sock
```

Or pass it on the command line:

```sh
ssh -R /run/user/1000/subportal/subportal.sock:/run/user/1000/subportal/subportal.sock myserver
```

Replace `1000` with your actual UID on both machines, or use
`$XDG_RUNTIME_DIR/subportal/subportal.sock` if your shell expands it.

No server-side SSH configuration changes are required.

## Usage

### Client side

Start the daemon on your desktop machine:

```sh
subportald
```

It listens on `$XDG_RUNTIME_DIR/subportal/subportal.sock` by default. Use
`--socket` to override.

### Server side

Install the server-side binaries (`subportal`, `xdg-open`, `notify-send`)
somewhere in your `$PATH` on the remote server. Place the drop-in replacements
earlier in `$PATH` than the real `xdg-open`/`notify-send` so they take
precedence.

**Open a URL:**

```sh
xdg-open https://example.com
# or
subportal open https://example.com
```

A confirmation dialog appears on your desktop before the browser opens.

**Open a file:**

```sh
xdg-open ./report.pdf
# or
subportal open ./report.pdf
```

The file is read, base64-encoded, and sent to the client (5 MB limit). A
confirmation dialog shows the file name, size, and MIME type before opening.

**Send a notification:**

```sh
notify-send "Build finished" "All 42 tests passed"
# or
subportal notify "Build finished" "All 42 tests passed" -u normal
```

Notifications are delivered without confirmation (passive, low risk).

**Check connectivity:**

```sh
subportal status
```

Shows the daemon version, round-trip latency, and supported capabilities.

### Environment variables

| Variable | Default | Description |
|---|---|---|
| `SUBPORTAL_SOCKET` | `$XDG_RUNTIME_DIR/subportal/subportal.sock` | Override the Unix socket path used by server-side tools |
| `RUST_LOG` | -- | Control log verbosity (e.g. `RUST_LOG=debug`) |

## V1 capabilities

| Capability | Direction | Description |
|---|---|---|
| **OpenURI** | server -> client | Open a URL in the client's browser |
| **OpenFile** | server -> client | Transfer a file (<5 MB) and open it on the client |
| **Notify** | server -> client | Show a desktop notification |
| **Ping** | server -> client | Check connectivity, discover capabilities |

## Security

- **Transport**: All traffic is encrypted by SSH. No additional encryption is
  needed since the Unix socket connection only traverses the SSH tunnel.
- **Access control**: Unix socket permissions restrict access to the owning
  user. Only the socket owner can connect, unlike TCP localhost which is
  accessible by any local user.
- **Server identity**: The daemon uses `SO_PEERCRED` to identify the PID of
  the connecting process and resolves it to the SSH remote host via
  `/proc/<pid>/cmdline`. This enables per-server logging and trust policies.
- **OpenURI / OpenFile**: The client shows a confirmation dialog before
  opening anything. The user must explicitly approve each request.
- **Notify**: No confirmation required (passive, low risk).

## Architecture

The project is structured as a Cargo workspace with five crates:

```
crates/
  subportal-lib/    Shared library: protocol types, client, server
  subportald/       Client daemon (desktop side)
  subportal/        Explicit CLI (server side)
  xdg-open/         Drop-in xdg-open replacement (server side)
  notify-send/      Drop-in notify-send replacement (server side)
```

See [docs/SPEC.md](docs/SPEC.md) for the full protocol specification,
including the Varlink interface definition, error types, and the v2 roadmap.

## Development

Enter the dev shell:

```sh
nix develop
```

Build and run:

```sh
cargo build
cargo run --bin subportald
cargo run --bin subportal -- status
```

Run clippy:

```sh
cargo clippy --workspace
```

Generate library documentation:

```sh
cargo doc --workspace --open
```

## License

TODO
