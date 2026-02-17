notify-send(1)

# NAME

notify-send - send a desktop notification via subportal

# SYNOPSIS

notify-send [_OPTIONS_] _TITLE_ [_BODY_]

# DESCRIPTION

This is the subportal drop-in replacement for notify-send(1). When placed
earlier in *$PATH* than the real notify-send, it transparently forwards
notifications through an SSH tunnel to the subportal client daemon
(*subportald*), which displays them on the local desktop using
xdg-desktop-portal.

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

The connection to the daemon goes through an SSH reverse tunnel using Unix
socket forwarding. Configure it in *~/.ssh/config*:

```
Host myserver
    RemoteForward /run/user/1000/subportal.sock /run/user/1000/subportal.sock
```

Or on the command line:

```
ssh -R /run/user/1000/subportal.sock:/run/user/1000/subportal.sock myserver
```

Unix-to-Unix socket forwarding requires OpenSSH 6.7+ (released 2014).

# SEE ALSO

*subportal*(1), *xdg-open*(1), *subportald*(1)

# AUTHORS

subportal contributors
