# Getting started with subportal

This tutorial walks you through setting up subportal so that commands like
`xdg-open` and `notify-send` on a remote SSH server transparently work on
your local desktop.

By the end, you will have:

- A running `subportald` daemon on your desktop
- SSH configured to forward the subportal socket
- Server-side tools installed on your remote machine
- Verified the connection with `subportal status`

## What you need

- A Linux desktop with xdg-desktop-portal (GNOME, KDE, Sway, Hyprland, ...)
- A remote Linux server you connect to via SSH
- OpenSSH 6.7+ on both machines (for Unix socket forwarding)

## Step 1: Install the client daemon

The client daemon (`subportald`) runs on your desktop machine -- the one with
the monitor, browser, and notification area.

### With Nix flakes

Add subportal as a flake input and enable the module:

```nix
# NixOS (configuration.nix or equivalent)
{ inputs, ... }:
{
  imports = [ inputs.subportal.nixosModules.subportald ];
  services.subportald.enable = true;
}
```

Or with home-manager:

```nix
{ inputs, ... }:
{
  imports = [ inputs.subportal.homeModules.subportald ];
  services.subportald.enable = true;
}
```

### Without Nix

Build from source (requires `cargo`, `pkg-config`, and `libdbus` development
headers):

```sh
git clone https://git.ntd.one/zimbatm/subportal.git
cd subportal
cargo build --release --bin subportald
```

Copy the binary somewhere in your `$PATH`:

```sh
cp target/release/subportald ~/.local/bin/
```

Start it:

```sh
subportald &
```

The daemon listens on `$XDG_RUNTIME_DIR/subportal.sock` (typically
`/run/user/1000/subportal.sock`).

## Step 2: Configure SSH

SSH needs to reverse-forward the subportal socket from the remote server back
to your desktop. Add this to `~/.ssh/config` on your desktop machine:

```
Host myserver
    RemoteForward /run/user/1000/subportal.sock /run/user/1000/subportal.sock
```

Replace `myserver` with your SSH host alias and `1000` with your UID (run
`id -u` to check).

> **Tip:** If you use the NixOS or home-manager module, you can configure this
> declaratively instead:
>
> ```nix
> services.subportald.sshHosts."myserver" = {};
> ```

The remote server's `sshd_config` must include:

```
StreamLocalBindUnlink yes
```

This lets SSH clean up stale sockets from previous sessions. Without it,
reconnecting will fail with "address already in use." NixOS and system-manager
modules set this automatically.

## Step 3: Install server-side tools

On the remote server, install the server-side package. This provides three
binaries: `subportal` (the explicit CLI), `xdg-open` (drop-in replacement),
and `notify-send` (drop-in replacement).

### With Nix flakes

```nix
# NixOS (on the server)
{ inputs, ... }:
{
  imports = [ inputs.subportal.nixosModules.subportal ];
  programs.subportal.enable = true;
}
```

Or with home-manager:

```nix
{ inputs, ... }:
{
  imports = [ inputs.subportal.homeModules.subportal ];
  programs.subportal.enable = true;
}
```

### Without Nix

Build the server-side tools:

```sh
cargo build --release -p subportal -p xdg-open -p notify-send
```

Install them somewhere in your `$PATH`, making sure the drop-in replacements
appear *before* the system `xdg-open` and `notify-send`:

```sh
cp target/release/subportal ~/.local/bin/
cp target/release/xdg-open ~/.local/bin/
cp target/release/notify-send ~/.local/bin/
```

## Step 4: Test the connection

SSH into your server (this activates the socket forwarding):

```sh
ssh myserver
```

On the server, check connectivity:

```sh
subportal status
```

You should see output like:

```
subportald v0.1.0
latency: 12.3ms
capabilities: OpenURI, OpenFile, Notify
```

If this works, the tunnel is up and the daemon is reachable.

## Step 5: Try it out

### Open a URL

```sh
xdg-open https://example.com
```

A confirmation dialog appears on your desktop. Approve it, and the URL opens
in your local browser.

### Send a notification

```sh
notify-send "Hello from $(hostname)" "subportal is working"
```

A desktop notification pops up, tagged with the server's hostname.

### Open a file

```sh
echo "Hello, world" > /tmp/test.txt
xdg-open /tmp/test.txt
```

The file is transferred to your desktop (up to 5 MB) and opened in your
default text editor after you confirm.

## What's next

- [SSH setup](../howto/ssh-setup.md) -- advanced SSH configuration (different
  UIDs, multiple servers, command-line usage)
- [Troubleshooting](../howto/troubleshooting.md) -- what to do when things
  go wrong
- [Architecture](../explanation/architecture.md) -- understand how the
  components work together
