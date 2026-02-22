# How to enroll a desktop client

subportal uses iroh (peer-to-peer QUIC) to connect desktop clients to
server-side agents. This guide covers the enrollment process.

## Prerequisites

- The agent (`subportal-agent`) must be running on the server
- The client daemon (`subportald`) must be installed on your desktop

## Enrollment steps

### 1. Generate a ticket on the server

On the server (where the agent is running):

```sh
subportal-agent ticket
```

This prints a JSON ticket to stdout. The ticket contains the agent's iroh
endpoint address and a one-time token that expires after 10 minutes
(configurable with `--ttl`).

### 2. Pipe the ticket to the client

The easiest way is to use SSH as a one-time transport for the ticket:

```sh
ssh myserver subportal-agent ticket | subportald enroll
```

This generates the ticket on the server and feeds it directly to the client
for enrollment. After this step, the client connects to the agent directly
via iroh -- no SSH tunnel or port forwarding is needed.

### 3. Verify the connection

On the server, check that the client is connected:

```sh
subportal-agent clients
```

You should see your desktop listed with its hostname, enrollment date, and
capabilities.

Then test with:

```sh
subportal status
```

## Multiple servers

Repeat the enrollment process for each server. The client daemon connects
to all enrolled agents on startup.

## Revoking a client

To remove an enrolled client from the server:

```sh
subportal-agent revoke <name-or-id>
```

This removes the client from the persistent registry and disconnects it
immediately if currently connected. The agent must be running.

## Forgetting a server

To remove an enrolled server from the client:

```sh
subportald forget <name-or-id>
```

The client will no longer connect to that server on subsequent runs.

## Troubleshooting enrollment

### "could not connect to running agent"

The `subportal-agent ticket` and `subportal-agent revoke` commands require
the agent to be running, since they communicate via Unix socket. Start the
agent with:

```sh
subportal-agent run
```

### Token expired

Tickets expire after 10 minutes by default. Generate a new one if needed,
or use `--ttl` to increase the timeout:

```sh
subportal-agent ticket --ttl 3600
```

### Client cannot reach agent

If the client enrolls but cannot connect, check:

1. Both machines have internet connectivity
2. No firewall is blocking QUIC (UDP) traffic
3. If both are behind NAT, iroh's relay servers handle NAT traversal
   automatically
