subportal(1)

# NAME

subportal - server-side CLI for the subportal protocol

# SYNOPSIS

subportal status

subportal open _TARGET_

subportal notify [_OPTIONS_] _TITLE_ [_BODY_]

subportal agent

subportal ticket [*--ttl* _SECS_]

subportal clients

subportal revoke _NAME-OR-ID_

# DESCRIPTION

*subportal* is the server-side CLI for the subportal protocol. It provides
both the user-facing commands (*status*, *open*, *notify*) and the agent
daemon plus its management commands (*agent*, *ticket*, *clients*, *revoke*).

All commands except *agent* connect to the running agent through a Unix
domain socket.

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

*agent*
	Start the agent daemon. The agent binds a Unix socket at
	_$XDG_RUNTIME_DIR/subportal.sock_ and starts listening for iroh
	connections from enrolled clients.

*ticket* [*--ttl* _SECS_]
	Generate a one-time enrollment ticket and print it as JSON to stdout.
	The ticket contains the agent's iroh endpoint address and a token
	that expires after _SECS_ seconds (default: 600, i.e. 10 minutes).
	Pipe this to *subportal-desktop enroll* on the desktop machine to enroll.
	The agent must be running for this command to work (it connects via
	Unix socket).

*clients*
	List all enrolled client devices with their name, endpoint ID,
	enrollment date, last seen time, and capabilities.

*revoke* _NAME-OR-ID_
	Remove an enrolled client by name or endpoint ID. The client is
	removed from the persistent registry and, if currently connected,
	is disconnected immediately. The agent must be running for this
	command to work (it connects via Unix socket).

# ENROLLMENT

To connect a desktop client to the agent:

. Start the agent on the server: *subportal agent*
. Generate a ticket: *subportal ticket*
. Pipe the ticket to the desktop client: *subportal-desktop enroll*

Example using SSH as a transport for the ticket:

```
ssh myserver subportal ticket | subportal-desktop enroll
```

After enrollment, *subportal-desktop* connects directly to the agent via iroh
without needing SSH.

# ENVIRONMENT

*SUBPORTAL_SOCKET*
	Override the Unix socket path used to connect to the agent. Default:
	_$XDG_RUNTIME_DIR/subportal.sock_.

*XDG_RUNTIME_DIR*
	Base directory for the Unix socket. Falls back to
	_/run/user/<uid>_ if unset.

*XDG_DATA_HOME*
	Base directory for persistent state (keypair, client registry).
	Falls back to _~/.local/share_ if unset.

*RUST_LOG*
	Control log verbosity (e.g. _RUST_LOG=debug_).

# FILES

_$XDG_DATA_HOME/subportal/keypair_
	The agent's iroh secret key (32 bytes, mode 0600).

_$XDG_DATA_HOME/subportal/clients.json_
	Registry of enrolled client devices.

# SYSTEMD

*subportal agent* is typically managed as a systemd user service. On NixOS:

```
programs.subportal.agent.enable = true;
```

# SEE ALSO

*subportal-desktop*(1), *xdg-open*(1), *notify-send*(1)

# AUTHORS

subportal contributors
