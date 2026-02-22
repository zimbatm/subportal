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

All commands connect to the subportal agent (*subportal-agent*) through a
Unix domain socket.

# COMMANDS

*status*
	Ping the agent and display its version, round-trip latency, and
	supported capabilities. Exits with code 1 if the agent is not
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
	Override the Unix socket path used to connect to the agent. Default:
	_$XDG_RUNTIME_DIR/subportal.sock_.

*RUST_LOG*
	Control log verbosity (e.g. _RUST_LOG=debug_).

# SEE ALSO

*subportal-agent*(1), *subportald*(1), *xdg-open*(1), *notify-send*(1)

# AUTHORS

subportal contributors
