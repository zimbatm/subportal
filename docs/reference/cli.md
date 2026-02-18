# CLI reference

Each binary has a man page (installed with the package) that serves as the
authoritative reference. The source files are in scdoc format and are
readable directly.

| Binary        | Side   | Description                              | Man page source                                                          |
| ------------- | ------ | ---------------------------------------- | ------------------------------------------------------------------------ |
| `subportald`  | client | Daemon -- listens for requests on a Unix socket | [subportald.1.scd](../../crates/subportald/subportald.1.scd)             |
| `subportal`   | server | Explicit CLI: `status`, `open`, `notify` | [subportal.1.scd](../../crates/subportal/subportal.1.scd)                |
| `xdg-open`    | server | Drop-in replacement for `xdg-open`       | [xdg-open.1.scd](../../crates/xdg-open/xdg-open.1.scd)                  |
| `notify-send` | server | Drop-in replacement for `notify-send`    | [notify-send.1.scd](../../crates/notify-send/notify-send.1.scd)          |

To read man pages after installation:

```sh
man subportald
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

When neither `SUBPORTAL_SOCKET` nor `XDG_RUNTIME_DIR` is set, the socket
path falls back to `/tmp/subportal-<uid>.sock`.
