# Nix module options reference

subportal provides modules for NixOS, home-manager, and system-manager. Each
system has two modules: one for the client daemon (`subportal-desktop`) and one for
the server-side tools (`subportal`).

## Flake outputs

| Output                                      | Type           | Side   |
| ------------------------------------------- | -------------- | ------ |
| `nixosModules.subportal-desktop`            | NixOS          | client |
| `nixosModules.subportal`                    | NixOS          | server |
| `homeModules.subportal-desktop`             | home-manager   | client |
| `homeModules.subportal`                     | home-manager   | server |
| `modules.system-manager.subportal-desktop`  | system-manager | client |
| `modules.system-manager.subportal`          | system-manager | server |
| `packages.<system>.subportal-desktop`       | package        | client |
| `packages.<system>.subportal-server`        | package        | server |

## Client daemon options (subportal-desktop)

Available in NixOS, home-manager, and system-manager modules.

### services.subportal-desktop.enable

Whether to enable the subportal client daemon.

- **Type:** boolean
- **Default:** `false`

### services.subportal-desktop.package

The subportal-desktop package to use.

- **Type:** package
- **Default:** `flake.packages.<system>.subportal-desktop`

### services.subportal-desktop.user

*system-manager only.* User account to run subportal-desktop as.

- **Type:** string
- **Required:** yes

## Server-side options (subportal)

Available in NixOS, home-manager, and system-manager modules.

### programs.subportal.enable

Whether to install the subportal server-side CLI tools.

- **Type:** boolean
- **Default:** `false`

### programs.subportal.package

The subportal-server package to use.

- **Type:** package
- **Default:** `flake.packages.<system>.subportal-server`

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

The NixOS and home-manager `subportal-desktop` modules create a systemd user
service that:

- Starts after `graphical-session.target`
- Is part of `graphical-session.target`
- Restarts on failure with a 5-second delay

The system-manager module creates a system service instead (not a user
service) and requires the `user` option.
