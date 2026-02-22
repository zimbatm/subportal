# How to set up subportal with Nix modules

subportal provides NixOS, home-manager, and system-manager modules for
declarative configuration. There are two modules to configure: one for
the client daemon (your desktop) and one for the server-side tools (the
remote server).

## Adding the flake input

Add subportal to your flake inputs:

```nix
{
  inputs.subportal.url = "git+https://git.ntd.one/zimbatm/subportal";
}
```

## Client setup (desktop machine)

The client daemon (`subportald`) runs on your desktop and handles incoming
requests from remote agents.

### NixOS

```nix
{ inputs, ... }:
{
  imports = [ inputs.subportal.nixosModules.subportald ];

  services.subportald.enable = true;
}
```

This creates a systemd user service that starts with your graphical session.

### home-manager

```nix
{ inputs, ... }:
{
  imports = [ inputs.subportal.homeModules.subportald ];

  services.subportald.enable = true;
}
```

### system-manager

system-manager runs as a system service rather than a user service, so it
requires specifying the user:

```nix
{ inputs, ... }:
{
  imports = [ inputs.subportal.modules.system-manager.subportald ];

  services.subportald = {
    enable = true;
    user = "myuser";
    socketPath = "/run/subportal.sock";
  };
}
```

## Server setup (remote host)

The server-side package provides `subportal`, `subportal-agent`, `xdg-open`,
and `notify-send`.

### NixOS

```nix
{ inputs, ... }:
{
  imports = [ inputs.subportal.nixosModules.subportal ];

  programs.subportal = {
    enable = true;
    agent.enable = true;  # start the agent as a systemd user service
    # xdg-open = true;      # install drop-in (default: true)
    # notify-send = true;    # install drop-in (default: true)
  };
}
```

### home-manager

```nix
{ inputs, ... }:
{
  imports = [ inputs.subportal.homeModules.subportal ];

  programs.subportal = {
    enable = true;
    # xdg-open = true;
    # notify-send = true;
  };
}
```

### system-manager

```nix
{ inputs, ... }:
{
  imports = [ inputs.subportal.modules.system-manager.subportal ];

  programs.subportal = {
    enable = true;
  };
}
```

## Enrollment

After both the client and server are running, enroll the desktop client:

```sh
ssh myserver subportal-agent ticket | subportald enroll
```

See [enrollment](enrollment.md) for full details.

## Disabling drop-in replacements

If you do not want subportal's `xdg-open` or `notify-send` to shadow the
system versions, disable them individually:

```nix
programs.subportal = {
  enable = true;
  xdg-open = false;      # do not install xdg-open drop-in
  notify-send = false;    # do not install notify-send drop-in
};
```

You can still use `subportal open` and `subportal notify` explicitly.

## Module options reference

See the [Nix modules reference](../reference/nix-modules.md) for a full list
of options.
