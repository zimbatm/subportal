# How to install subportal manually

This guide covers building subportal from source and installing it without
Nix.

## Prerequisites

- Rust toolchain (`cargo`, `rustc`)
- `pkg-config`
- `libdbus` development headers (for the client daemon)
- `scdoc` (optional, for building man pages)

On Debian/Ubuntu:

```sh
sudo apt install pkg-config libdbus-1-dev
```

On Fedora:

```sh
sudo dnf install pkg-config dbus-devel
```

On Arch:

```sh
sudo pacman -S pkgconf dbus
```

## Building

Clone the repository and build in release mode:

```sh
git clone https://git.ntd.one/zimbatm/subportal.git
cd subportal
cargo build --release
```

Or build with Nix:

```sh
nix build .#subportald   # client daemon
nix build .#subportal    # server-side tools
```

## Installing the client daemon (desktop machine)

Copy the daemon binary:

```sh
cp target/release/subportald ~/.local/bin/
```

### Starting manually

```sh
subportald &
```

### Autostart with systemd

Create `~/.config/systemd/user/subportald.service`:

```ini
[Unit]
Description=subportal client daemon
After=graphical-session.target
PartOf=graphical-session.target

[Service]
ExecStart=%h/.local/bin/subportald
Restart=on-failure
RestartSec=5

[Install]
WantedBy=graphical-session.target
```

Enable and start it:

```sh
systemctl --user daemon-reload
systemctl --user enable --now subportald
```

### Autostart with XDG

Copy the desktop file to autostart:

```sh
cp crates/subportald/subportald.desktop ~/.config/autostart/
```

Edit `Exec=` to point to your binary location.

## Installing server-side tools (remote server)

Copy the four server-side binaries:

```sh
scp target/release/subportal myserver:~/.local/bin/
scp target/release/subportal-agent myserver:~/.local/bin/
scp target/release/xdg-open myserver:~/.local/bin/
scp target/release/notify-send myserver:~/.local/bin/
```

The drop-in replacements (`xdg-open`, `notify-send`) must appear in `$PATH`
*before* the system versions. If `~/.local/bin` is already at the front of
your `$PATH`, this works automatically. Otherwise, adjust your shell profile:

```sh
export PATH="$HOME/.local/bin:$PATH"
```

Verify with:

```sh
which xdg-open
# should show ~/.local/bin/xdg-open, not /usr/bin/xdg-open
```

## Building man pages

Man pages are written in `scdoc` format. If you have `scdoc` installed:

```sh
scdoc < crates/subportald/subportald.1.scd > subportald.1
scdoc < crates/subportal/subportal.1.scd > subportal.1
scdoc < crates/xdg-open/xdg-open.1.scd > xdg-open.1
scdoc < crates/notify-send/notify-send.1.scd > notify-send.1
```

Install them:

```sh
install -Dm644 subportald.1 ~/.local/share/man/man1/subportald.1
install -Dm644 subportal.1 ~/.local/share/man/man1/subportal.1
install -Dm644 xdg-open.1 ~/.local/share/man/man1/xdg-open.1
install -Dm644 notify-send.1 ~/.local/share/man/man1/notify-send.1
```

## Uninstalling

Remove the binaries and service files:

```sh
rm ~/.local/bin/{subportald,subportal,xdg-open,notify-send}
systemctl --user disable --now subportald
rm ~/.config/systemd/user/subportald.service
systemctl --user daemon-reload
```
