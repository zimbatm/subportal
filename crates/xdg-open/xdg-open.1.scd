xdg-open(1)

# NAME

xdg-open - open a file or URL on the client desktop via subportal

# SYNOPSIS

xdg-open {_file_ | _URL_}

xdg-open {*--help* | *--version*}

# DESCRIPTION

This is the subportal drop-in replacement for xdg-open(1). When placed
earlier in *$PATH* than the real xdg-open, it transparently forwards open
requests through an SSH tunnel to the subportal client daemon (*subportald*),
which opens them on the local desktop using xdg-desktop-portal.

If the target is a URL (contains a _scheme://_), it is sent as an *OpenURI*
request. The client shows a confirmation dialog before opening the URL in the
user's preferred browser.

If the target is a file path, the file is read, base64-encoded, and sent as
an *OpenFile* request. The client shows a confirmation dialog with the file
name, size, and MIME type before opening it in the preferred application.
Files are limited to 5 MB.

# OPTIONS

*--help*
	Show usage information.

*--version*
	Show the subportal version.

# EXIT CODES

Exit codes follow the xdg-open convention:

*0*
	Success.

*1*
	Error in command line syntax.

*2*
	File not found or not readable.

*3*
	Required tool could not be found (daemon not reachable).

*4*
	The action failed (user denied, file too large, or other error).

# ENVIRONMENT

*SUBPORTAL_SOCKET*
	Override the Unix socket path used to connect to the daemon. Default:
	_$XDG_RUNTIME_DIR/subportal/subportal.sock_.

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

*subportal*(1), *notify-send*(1), *subportald*(1)

# AUTHORS

subportal contributors
