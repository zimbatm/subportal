# How to configure SSH for subportal

subportal uses SSH reverse forwarding to tunnel a Unix domain socket from the
remote server back to your desktop. This guide covers the SSH configuration
needed on both sides.

## Basic setup

Add a `RemoteForward` directive to your SSH client config (`~/.ssh/config` on
your desktop):

```
Host myserver
    RemoteForward /run/user/1000/subportal.sock /run/user/1000/subportal.sock
```

The format is:

```
RemoteForward <remote-socket-path> <local-socket-path>
```

Both paths default to `$XDG_RUNTIME_DIR/subportal.sock`, which is typically
`/run/user/<uid>/subportal.sock`.

Replace `1000` with your actual UID. Run `id -u` to check.

## Command-line usage

Instead of editing `~/.ssh/config`, you can pass the forwarding on the command
line:

```sh
ssh -R /run/user/1000/subportal.sock:/run/user/1000/subportal.sock myserver
```

## Server-side sshd configuration

The remote server's `sshd_config` (usually `/etc/ssh/sshd_config`) must
include:

```
StreamLocalBindUnlink yes
```

This tells `sshd` to remove an existing socket file before binding a new one.
Without it, if a previous SSH session was interrupted (network drop, crash,
etc.), the stale socket remains and the new session fails with "address
already in use."

After changing `sshd_config`, reload the SSH daemon:

```sh
sudo systemctl reload sshd
```

The NixOS and system-manager subportal modules set this automatically.

## Different UIDs on client and server

If your UID differs between the desktop and the server (e.g., UID 1000 locally,
UID 1001 on the server), adjust the remote path:

```
Host myserver
    RemoteForward /run/user/1001/subportal.sock /run/user/1000/subportal.sock
```

With the NixOS/home-manager module, use the `remoteUid` option:

```nix
services.subportald.sshHosts."myserver" = { remoteUid = 1001; };
```

## Multiple servers

Add a `RemoteForward` entry for each server:

```
Host server-a
    RemoteForward /run/user/1000/subportal.sock /run/user/1000/subportal.sock

Host server-b
    RemoteForward /run/user/1000/subportal.sock /run/user/1000/subportal.sock
```

Each SSH connection forwards independently to the same local daemon.

## Wildcard configuration

To enable subportal for all SSH connections:

```
Host *
    RemoteForward /run/user/1000/subportal.sock /run/user/1000/subportal.sock
```

This is convenient but means every SSH session attempts the forward, which
produces a warning if the remote `$XDG_RUNTIME_DIR` does not exist or `sshd`
does not allow it. Errors in socket forwarding do not prevent the SSH session
from connecting.

## Verifying the tunnel

After connecting, run on the server:

```sh
ls -la /run/user/1000/subportal.sock
```

You should see a socket file. Then test with:

```sh
subportal status
```

If the socket exists but `subportal status` fails, the daemon may not be
running on your desktop. See [troubleshooting](troubleshooting.md).

## OpenSSH version requirements

Unix-to-Unix socket forwarding requires OpenSSH 6.7 or later (released
October 2014). Both the client and server must support it. Check with:

```sh
ssh -V
```
