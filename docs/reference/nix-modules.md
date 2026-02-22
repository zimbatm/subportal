# Nix module options reference

subportal provides modules for NixOS, home-manager, and system-manager. Each
system has two modules: one for the client daemon (`subportald`) and one for
the server-side tools (`subportal`).

## Flake outputs

| Output                                  | Type           | Side   |
| --------------------------------------- | -------------- | ------ |
| `nixosModules.subportald`               | NixOS          | client |
| `nixosModules.subportal`                | NixOS          | server |
| `homeModules.subportald`                | home-manager   | client |
| `homeModules.subportal`                 | home-manager   | server |
| `modules.system-manager.subportald`     | system-manager | client |
| `modules.system-manager.subportal`      | system-manager | server |
| `packages.<system>.subportald`          | package        | client |
| `packages.<system>.subportal`           | package        | server |

## Client daemon options (subportald)

Available in NixOS, home-manager, and system-manager modules.

### services.subportald.enable

Whether to enable the subportal client daemon.

- **Type:** boolean
- **Default:** `false`

### services.subportald.package

The subportald package to use.

- **Type:** package
- **Default:** `flake.packages.<system>.subportald`

### services.subportald.socketPath

Unix socket path for subportald to listen on.

- **Type:** string
- **Default (NixOS/home-manager):** `%t/subportal.sock` (systemd specifier,
  expands to `$XDG_RUNTIME_DIR/subportal.sock`)
- **Default (system-manager):** `/run/subportal.sock`

### services.subportald.user

*system-manager only.* User account to run subportald as.

- **Type:** string
- **Required:** yes

## Server-side options (subportal)

Available in NixOS, home-manager, and system-manager modules.

### programs.subportal.enable

Whether to install the subportal server-side CLI tools.

- **Type:** boolean
- **Default:** `false`

### programs.subportal.package

The subportal package to use.

- **Type:** package
- **Default:** `flake.packages.<system>.subportal`

### programs.subportal.xdg-open

Whether to install the `xdg-open` drop-in replacement.

- **Type:** boolean
- **Default:** `true`

### programs.subportal.notify-send

Whether to install the `notify-send` drop-in replacement.

- **Type:** boolean
- **Default:** `true`

## Side effects

### Systemd service

The NixOS and home-manager `subportald` modules create a systemd user
service that:

- Starts after `graphical-session.target`
- Is part of `graphical-session.target`
- Restarts on failure with a 5-second delay

The system-manager module creates a system service instead (not a user
service) and requires the `user` option.
