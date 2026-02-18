# subportal - xdg-open on your server

Did you ever try to go through the oauth flow on your server and had to
copy-paste the URL back into your local browser? Or wanted to open a file back
in your local $EDITOR for a quick peek? Now you can.

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
(`$XDG_RUNTIME_DIR/subportal.sock`), which SSH reverse-forwards to
the client daemon (`subportald`). The daemon uses
[xdg-desktop-portal](https://flatpak.github.io/xdg-desktop-portal/) D-Bus
APIs to show native dialogs and notifications on whatever desktop environment
you run (GNOME, KDE, Sway, ...).

## Quick start

See the [getting started tutorial](docs/tutorials/getting-started.md) for a
complete walkthrough.

## Documentation

The [documentation](docs/index.md) is organized using the
[Diataxis](https://diataxis.fr/) framework:

- **[Tutorials](docs/tutorials/getting-started.md)** -- learn by doing
- **How-to guides** -- solve specific problems
  - [SSH setup](docs/howto/ssh-setup.md)
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
