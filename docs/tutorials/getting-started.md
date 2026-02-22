# Getting started with subportal

This tutorial walks you through setting up subportal so that commands like
`xdg-open` and `notify-send` on a remote server transparently work on
your local desktop.

By the end, you will have:

- A running `subportal-desktop` daemon on your desktop
- A running `subportal agent` on your server
- Your desktop enrolled with the agent
- Verified the connection with `subportal status`

## What you need

- A Linux desktop with xdg-desktop-portal (GNOME, KDE, Sway, Hyprland, ...)
- A remote Linux server

## Step 1: Install the client daemon

The client daemon (`subportal-desktop`) runs on your desktop machine -- the one with
the monitor, browser, and notification area.

### With Nix flakes

Add subportal as a flake input and enable the module:

```nix
# NixOS (configuration.nix or equivalent)
{ inputs, ... }:
{
  imports = [ inputs.subportal.nixosModules.subportal-desktop ];
  services.subportal-desktop.enable = true;
}
```

Or with home-manager:

```nix
{ inputs, ... }:
{
  imports = [ inputs.subportal.homeModules.subportal-desktop ];
  services.subportal-desktop.enable = true;
}
```

### Without Nix

Build from source (requires `cargo`, `pkg-config`, and `libdbus` development
headers):

```sh
git clone https://git.ntd.one/zimbatm/subportal.git
cd subportal
cargo build --release --bin subportal-desktop
```

Copy the binary somewhere in your `$PATH`:

```sh
cp target/release/subportal-desktop ~/.local/bin/
```

Start it:

```sh
subportal-desktop &
```

## Step 2: Install server-side tools

On the remote server, install the server-side package. This provides three
binaries: `subportal` (the CLI and agent daemon), `xdg-open` (drop-in
replacement), and `notify-send` (drop-in replacement).

### With Nix flakes

```nix
# NixOS (on the server)
{ inputs, ... }:
{
  imports = [ inputs.subportal.nixosModules.subportal ];
  programs.subportal.enable = true;
  programs.subportal.agent.enable = true;
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

Start the agent:

```sh
subportal agent &
```

## Step 3: Enroll your desktop

The easiest way to enroll is to pipe a ticket from the server to the client
using SSH as a one-time transport:

```sh
ssh myserver subportal ticket | subportal-desktop enroll
```

This generates an enrollment ticket on the server and feeds it to the client.
After enrollment, the client connects directly to the agent via iroh
(peer-to-peer QUIC) -- no ongoing SSH tunnel is needed.

> **Tip:** If you use the NixOS or home-manager module, the agent runs as a
> systemd service. You can generate tickets from any session on the server.

## Step 4: Test the connection

On the server, check connectivity:

```sh
subportal status
```

You should see output like:

```
subportal v0.2.0
latency: 12.3ms
capabilities: OpenURI, OpenFile, Notify
```

If this works, the agent is reachable and your desktop client is connected.

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

- [Enrollment](../howto/enrollment.md) -- manage enrolled clients, revoke
  access, enroll additional servers
- [Troubleshooting](../howto/troubleshooting.md) -- what to do when things
  go wrong
- [Architecture](../explanation/architecture.md) -- understand how the
  components work together
