subportal(1)

# NAME

subportal - explicit CLI for the subportal protocol

# SYNOPSIS

subportal status

subportal open _TARGET_

subportal notify [_OPTIONS_] _TITLE_ [_BODY_]

# DESCRIPTION

*subportal* is the server-side CLI for the subportal protocol. Unlike the
drop-in replacements (*xdg-open*, *notify-send*), this binary is invoked
explicitly and provides richer output such as latency and capability
reporting.

All commands connect to the subportal client daemon (*subportald*) through an
SSH reverse tunnel via a Unix domain socket.

# COMMANDS

*status*
	Ping the daemon and display its version, round-trip latency, and
	supported capabilities. Exits with code 1 if the daemon is not
	reachable.

*open* _TARGET_
	Open a URL or file on the client desktop. If _TARGET_ contains a URI
	scheme (e.g. _https://_), it is sent as an *OpenURI* request. Otherwise
	it is treated as a file path: the file is read, base64-encoded, and
	sent as an *OpenFile* request. Files are limited to 5 MB. The client
	shows a confirmation dialog before opening.

*notify* _TITLE_ [_BODY_]
	Send a desktop notification. Notifications are delivered without user
	confirmation.

	Options:

	*-u, --urgency* _LEVEL_
		Urgency level: *low*, *normal*, or *critical*.

	*-i, --icon* _ICON_
		Icon name from the icon theme.

# ENVIRONMENT

*SUBPORTAL_SOCKET*
	Override the Unix socket path used to connect to the daemon. Default:
	_$XDG_RUNTIME_DIR/subportal/subportal.sock_.

*RUST_LOG*
	Control log verbosity (e.g. _RUST_LOG=debug_).

# TRANSPORT

The connection to the daemon goes through an SSH reverse tunnel using Unix
socket forwarding. Configure it in *~/.ssh/config*:

```
Host myserver
    RemoteForward /run/user/1000/subportal/subportal.sock /run/user/1000/subportal/subportal.sock
```

Or on the command line:

```
ssh -R /run/user/1000/subportal/subportal.sock:/run/user/1000/subportal/subportal.sock myserver
```

Unix-to-Unix socket forwarding requires OpenSSH 6.7+ (released 2014).

# SEE ALSO

*xdg-open*(1), *notify-send*(1), *subportald*(1)

# AUTHORS

subportal contributors
