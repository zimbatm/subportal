subportald(1)

# NAME

subportald - subportal client daemon

# SYNOPSIS

subportald [*--port* _PORT_]

# DESCRIPTION

*subportald* runs on the user's desktop machine and listens for
Varlink-over-TCP requests from server-side subportal tools. Each request is
dispatched to the local xdg-desktop-portal D-Bus interface to open URLs,
transfer files, or show desktop notifications.

The daemon binds to *127.0.0.1* only. Server-side tools reach it through an
SSH reverse port forward.

# OPTIONS

*-p, --port* _PORT_
	TCP port to listen on. Default: 19494.

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

*RUST_LOG*
	Control log verbosity (e.g. _RUST_LOG=debug_).

*XDG_RUNTIME_DIR*
	Directory for temporary file storage. Falls back to _/tmp_ if unset.

# TRANSPORT

Server-side tools connect through an SSH reverse tunnel. On the client, the
SSH connection must be configured to forward the subportal port:

```
Host myserver
    RemoteForward 127.0.0.1:19494 127.0.0.1:19494
```

Or on the command line:

```
ssh -R 127.0.0.1:19494:127.0.0.1:19494 myserver
```

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
