subportal-agent(1)

# NAME

subportal-agent - subportal server-side agent

# SYNOPSIS

subportal-agent [*run*]

subportal-agent *ticket* [*--ttl* _SECS_]

subportal-agent *clients*

subportal-agent *revoke* _NAME-OR-ID_

# DESCRIPTION

*subportal-agent* runs on the server (headless machine) and bridges
server-side subportal tools to desktop clients via iroh (peer-to-peer QUIC).

The agent listens on both a Unix domain socket (for local tools like
*subportal*(1), *xdg-open*(1), and *notify-send*(1)) and an iroh endpoint
(for desktop clients running *subportald*(1)). Requests received on the Unix
socket are routed to connected desktop clients.

# COMMANDS

*run*
	Start the agent. This is the default command when none is specified.
	The agent binds a Unix socket at _$XDG_RUNTIME_DIR/subportal.sock_
	and starts listening for iroh connections from enrolled clients.

*ticket* [*--ttl* _SECS_]
	Generate a one-time enrollment ticket and print it as JSON to stdout.
	The ticket contains the agent's iroh endpoint address and a token
	that expires after _SECS_ seconds (default: 600, i.e. 10 minutes).
	Pipe this to *subportald enroll* on the desktop machine to enroll.

*clients*
	List all enrolled client devices with their name, endpoint ID,
	enrollment date, last seen time, and capabilities.

*revoke* _NAME-OR-ID_
	Remove an enrolled client by name or endpoint ID. The client will
	no longer be allowed to reconnect.

# ENROLLMENT

To connect a desktop client to the agent:

. Start the agent on the server: *subportal-agent*
. Generate a ticket: *subportal-agent ticket*
. Pipe the ticket to the desktop client: *subportald enroll*

Example using SSH as a transport for the ticket:

```
ssh myserver subportal-agent ticket | subportald enroll
```

After enrollment, *subportald* connects directly to the agent via iroh
without needing SSH.

# ENVIRONMENT

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

*subportal-agent* is typically managed as a systemd user service. On NixOS:

```
programs.subportal.agent.enable = true;
```

# SEE ALSO

*subportal*(1), *subportald*(1), *xdg-open*(1), *notify-send*(1)

# AUTHORS

subportal contributors
