# CLI reference

Each binary has a man page (installed with the package) that serves as the
authoritative reference. The source files are in scdoc format and are
readable directly.

| Binary             | Side   | Description                              | Man page source                                                                    |
| ------------------ | ------ | ---------------------------------------- | ---------------------------------------------------------------------------------- |
| `subportal-desktop`| client | Daemon -- connects to agents via iroh    | [subportal-desktop.1.scd](../../crates/subportal-desktop/subportal-desktop.1.scd)  |
| `subportal`        | server | CLI + agent: `status`, `open`, `notify`, `agent`, `ticket`, `clients`, `revoke` | [subportal.1.scd](../../crates/subportal/subportal.1.scd)                |
| `xdg-open`         | server | Drop-in replacement for `xdg-open`       | [xdg-open.1.scd](../../crates/xdg-open/xdg-open.1.scd)                            |
| `notify-send`      | server | Drop-in replacement for `notify-send`    | [notify-send.1.scd](../../crates/notify-send/notify-send.1.scd)                    |

To read man pages after installation:

```sh
man subportal-desktop
man subportal
man xdg-open
man notify-send
```

## Environment variables

These apply to all binaries.

| Variable            | Default                              | Description                    |
| ------------------- | ------------------------------------ | ------------------------------ |
| `SUBPORTAL_SOCKET`  | `$XDG_RUNTIME_DIR/subportal.sock`    | Override the Unix socket path  |
| `XDG_RUNTIME_DIR`   | (set by system)                      | Base directory for the default socket path |
| `RUST_LOG`          | (unset)                              | Control log verbosity (e.g. `debug`, `info`) |

When `XDG_RUNTIME_DIR` is not set, the socket path falls back to
`/run/user/<uid>/subportal.sock`. This assumes a systemd-based system.
