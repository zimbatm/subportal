subportald(1)

# NAME

subportald - subportal client daemon

# SYNOPSIS

subportald [*--socket* _PATH_]

# DESCRIPTION

*subportald* runs on the user's desktop machine and listens for
Varlink requests on a Unix domain socket from server-side subportal tools.
Each request is dispatched to the local xdg-desktop-portal D-Bus interface
to open URLs, transfer files, or show desktop notifications.

The daemon listens on a Unix socket (by default
_$XDG_RUNTIME_DIR/subportal.sock_). Server-side tools reach it
through an SSH reverse socket forward.

# OPTIONS

*-s, --socket* _PATH_
	Unix socket path to listen on. Default:
	_$XDG_RUNTIME_DIR/subportal.sock_.

# CAPABILITIES

*OpenURI*
	Open a URL in the user's preferred application. A confirmation dialog
	is shown before opening.

*OpenFile*
	Receive a base64-encoded file (up to 5 MB), write it to
	_$XDG_RUNTIME_DIR/subportal/_, and open it in the preferred
	application. A confirmation dialog is shown with the file name, size,
	and MIME type.

*Notify*
	Display a desktop notification. No confirmation is required.

# ENVIRONMENT

*SUBPORTAL_SOCKET*
	Override the Unix socket path (same as *--socket*).

*RUST_LOG*
	Control log verbosity (e.g. _RUST_LOG=debug_).

*XDG_RUNTIME_DIR*
	Base directory for the default socket path and temporary file storage.
	Falls back to _/tmp_ if unset.

# TRANSPORT

Server-side tools connect through an SSH reverse tunnel. On the client, the
SSH connection must be configured to forward the subportal Unix socket:

```
Host myserver
    RemoteForward /run/user/1000/subportal.sock /run/user/1000/subportal.sock
```

Or on the command line:

```
ssh -R /run/user/1000/subportal.sock:/run/user/1000/subportal.sock myserver
```

Unix-to-Unix socket forwarding requires OpenSSH 6.7+ (released 2014).

# PEER IDENTITY

Server-side tools include their hostname in every request via the _host_
parameter (set automatically from gethostname(2)). The daemon uses this to
annotate notifications (e.g. _subportal@myserver_) and for logging.

*subportald* also uses _SO_PEERCRED_ to obtain the UID of the connecting
process and rejects connections from UIDs that don't match its own (except
root).

# SYSTEMD

*subportald* is typically managed as a systemd user service, started after
*graphical-session.target*. On NixOS or home-manager:

```
services.subportald.enable = true;
```

# SEE ALSO

*subportal*(1), *xdg-open*(1), *notify-send*(1)

# AUTHORS

subportal contributors
