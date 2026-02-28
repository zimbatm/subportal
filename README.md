# subportal - xdg-open on your server

Did you ever try to go through the oauth flow on your server and had to
copy-paste the URL back into your local browser? Or wanted to open a file back
in your local $EDITOR for a quick peek? Now you can.

`subportal` bridges a headless server to your local desktop or phone.
Server-side commands like `xdg-open` and `notify-send` transparently
forward requests via iroh (peer-to-peer QUIC) to a client, which handles
them using the local desktop environment or Android system.

Open a URL on a remote server and it appears in your local browser. Send a
notification and it pops up on your desktop. No SSH tunnels or port
forwarding required -- just enroll once and it connects automatically.

## Clients

- **subportal-desktop** -- Linux daemon using
  [xdg-desktop-portal](https://flatpak.github.io/xdg-desktop-portal/) D-Bus
  APIs for native dialogs and notifications (GNOME, KDE, Sway, ...)
- **subportal-android** -- Android app (Jetpack Compose). Enroll by scanning
  an iroh ticket QR code. Receives URLs, files, and notifications as Android
  notifications.

## How it works

```
Server (headless)                       Client (desktop or phone)
                                        subportal-desktop / android app
xdg-open https://example.com  ─────>     -> opens in browser
notify-send "Build done"      ─(unix)─>  -> notification
subportal open ./report.pdf   ─(sock)─>  -> opens in PDF viewer
```

The server-side tools connect to `subportal agent` via a Unix domain
socket (`$XDG_RUNTIME_DIR/subportal.sock`). The agent routes requests to
enrolled clients over [iroh](https://iroh.computer/) (peer-to-peer QUIC).

On Linux, the desktop client uses xdg-desktop-portal D-Bus APIs. On
Android, the app runs as a foreground service with a persistent QUIC
connection and handles requests via system intents and notifications.

## Quick start

See the [getting started tutorial](docs/tutorials/getting-started.md) for a
complete walkthrough.

## Documentation

The [documentation](docs/index.md) is organized using the
[Diataxis](https://diataxis.fr/) framework:

- **[Tutorials](docs/tutorials/getting-started.md)** -- learn by doing
- **How-to guides** -- solve specific problems
  - [Enrollment](docs/howto/enrollment.md)
  - [NixOS / home-manager setup](docs/howto/nixos-setup.md)
  - [Manual installation](docs/howto/manual-install.md)
  - [Troubleshooting](docs/howto/troubleshooting.md)
- **Reference** -- technical details
  - [Protocol](docs/reference/protocol.md)
  - [CLI](docs/reference/cli.md)
  - [Nix modules](docs/reference/nix-modules.md)
- **Explanation** -- design and rationale
  - [Architecture](docs/explanation/architecture.md)
  - [Security model](docs/explanation/security.md)

## V1 capabilities

| Capability   | Direction        | Description                                    |
| ------------ | ---------------- | ---------------------------------------------- |
| **OpenURI**  | server -> client | Open a URL in the client's browser             |
| **OpenFile** | server -> client | Transfer a file (<5 MB) and open it on client  |
| **Notify**   | server -> client | Show a desktop notification                    |
| **Ping**     | server -> client | Check connectivity, discover capabilities      |

## Development

```sh
nix develop       # enter dev shell
cargo build       # build all crates
cargo test        # run tests
cargo clippy      # lint
```

See [docs/SPEC.md](docs/SPEC.md) for the protocol specification.

## License

[MIT](LICENSE)
