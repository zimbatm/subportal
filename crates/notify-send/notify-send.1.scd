notify-send(1)

# NAME

notify-send - send a desktop notification via subportal

# SYNOPSIS

notify-send [_OPTIONS_] _TITLE_ [_BODY_]

# DESCRIPTION

This is the subportal drop-in replacement for notify-send(1). When placed
earlier in *$PATH* than the real notify-send, it transparently forwards
notifications to a running *subportal-agent*(1), which routes them via iroh
(peer-to-peer QUIC) to enrolled desktop clients running *subportald*(1).
Notifications are displayed via the standard _org.freedesktop.Notifications_
D-Bus interface.

Notifications are delivered without user confirmation (passive, low risk).

# OPTIONS

*-u, --urgency* _LEVEL_
	Urgency level: *low*, *normal*, or *critical*. Default: normal.

*-i, --icon* _ICON_
	Icon name from the icon theme (e.g. _dialog-information_).

*-h, --help*
	Show usage information.

*--version*
	Show the subportal version.

## ACCEPTED BUT UNSUPPORTED

The following flags from the standard notify-send(1) are accepted for
compatibility but have no effect. The xdg-desktop-portal notification API
does not support these features.

*-t, --expire-time* _MS_
	Notification timeout in milliseconds.

*-a, --app-name* _NAME_
	Application name for the notification.

*-c, --category* _TYPE_
	Notification category.

*-A, --action* _[NAME=]TEXT_
	Action buttons.

*-r, --replace-id* _ID_
	Replace an existing notification.

*--hint* _TYPE:NAME:VALUE_
	Extra hint data.

*-p, --print-id*
	Print the notification ID.

*-w, --wait*
	Wait for the notification to be closed.

*-e, --transient*
	Show a transient notification.

# ENVIRONMENT

*SUBPORTAL_SOCKET*
	Override the Unix socket path used to connect to the daemon. Default:
	_$XDG_RUNTIME_DIR/subportal.sock_.

# TRANSPORT

Server-side tools connect to the *subportal-agent*(1) via a Unix domain
socket at _$XDG_RUNTIME_DIR/subportal.sock_. The agent forwards requests to
enrolled desktop clients over iroh (peer-to-peer QUIC). No SSH tunnel or
port forwarding is required.

# SEE ALSO

*subportal*(1), *xdg-open*(1), *subportald*(1)

# AUTHORS

subportal contributors
