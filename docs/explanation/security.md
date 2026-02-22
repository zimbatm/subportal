# Security model

## Threat model

subportal forwards desktop actions (opening URLs, opening files, showing
notifications) from a remote server to a local desktop. The primary threats
are:

1. **Unauthorized access** -- a process on the server that should not be
   able to trigger desktop actions does so
2. **Malicious content** -- a legitimate request carries a harmful URL or
   file
3. **Spoofed identity** -- the user cannot tell which server a request came
   from

## Transport encryption

All traffic between the agent and client daemon travels over iroh, which
uses QUIC with TLS 1.3. Each endpoint has a persistent keypair, and
connections are authenticated by endpoint ID (public key). No additional
encryption is needed.

On the server side, communication between CLI tools and the agent uses a
local Unix domain socket, which is not network-accessible.

## Access control

### Unix socket permissions

The agent's Unix socket is a Unix domain socket, which has inherently
stronger access control than TCP localhost:

- **Unix socket**: only accessible by the file owner (restricted by
  filesystem permissions and umask)
- **TCP localhost**: accessible by any process on the machine, regardless of
  user

This is a deliberate choice. TCP localhost is a common source of privilege
escalation in local-service architectures. Unix sockets avoid this entirely.

### SO_PEERCRED validation

When a process connects to the agent's Unix socket, the agent calls
`getsockopt` with `SO_PEERCRED` to obtain the peer's UID and PID. It then:

- **Rejects** connections from UIDs that do not match the agent's own UID
- **Allows** connections from root (UID 0), since root can access any
  socket anyway and systemd services may run as root

### Enrollment-based client authentication

Desktop clients must be enrolled with the agent before they can receive
requests. Enrollment uses a one-time token:

1. The agent generates a time-limited token
2. The token is transferred out-of-band (e.g. piped over SSH)
3. The client presents the token on first connection
4. After enrollment, the client's iroh endpoint ID (public key) is stored
   in the agent's persistent registry
5. On reconnection, the client is authenticated by its endpoint ID

Only enrolled clients can connect. Unknown endpoint IDs are rejected.

## User confirmation

### OpenURI and OpenFile

Every URL and file open request triggers a confirmation dialog via
xdg-desktop-portal. The user must explicitly approve each request. The
dialog shows:

- For URLs: the URL to be opened
- For files: the file name, size, and MIME type

This is the primary defense against malicious content. Even if an attacker
gains code execution on the server, they cannot silently open arbitrary URLs
or install files on the desktop without the user's explicit consent.

### Notify

Notifications are delivered without confirmation. This is a deliberate
trade-off:

- Notifications are passive (they display text, they do not execute code or
  open applications)
- Requiring confirmation for notifications would defeat their purpose
- The worst case is unwanted notification spam, which is annoying but not a
  security compromise

## Server identity

Server-side tools include their hostname (from `gethostname(2)`) in every
request. The daemon displays this in notifications as `subportal@<hostname>`
and uses it for logging.

This hostname is **self-reported and not cryptographically verified**. A
compromised server could report any hostname. However:

- The iroh connection authenticates the agent by its endpoint ID
- The user explicitly enrolled with a specific agent
- The hostname is informational, not used for access control decisions
- In practice, the user knows which server they enrolled

## Trust boundaries

```
Untrusted                          Trusted
(server)          iroh QUIC        (desktop)
+--------------+  ===========  +--------------+
| xdg-open     |               | subportald   |
| notify-send  | ---------->   |   |          |
| subportal    |               |   v          |
|              |               | confirmation |
| other tools  |               | dialog       |
+--------------+               +--------------+
```

The trust boundary is at the daemon. Everything coming from the server is
treated as untrusted:

- URLs and files require explicit user confirmation
- Notifications are shown but cannot execute code
- The daemon never sends data back to the server (except success/error
  responses)
- The daemon never executes commands on behalf of the server

## Limitations

- **File size**: 5 MB limit prevents large file exfiltration but also
  limits legitimate use. Large file transfer is deferred to v2.
- **No rate limiting**: a compromised server could spam notifications or
  confirmation dialogs. This is annoying but not exploitable.
- **Hostname spoofing**: as noted, the hostname is self-reported. A
  compromised server could impersonate another server's hostname in
  notifications.
