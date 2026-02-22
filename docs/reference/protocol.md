# Protocol reference

subportal uses the [Varlink](https://varlink.org/) protocol over a Unix domain
socket. Each connection carries exactly one request-response exchange.

## Wire format

Messages are JSON objects delimited by a NUL byte (`0x00`). A connection
consists of:

1. Client sends a JSON request followed by NUL
2. Server sends a JSON response followed by NUL
3. Connection is closed

Maximum message size is 8 MB (enough for a base64-encoded 5 MB file plus JSON
overhead).

## Interface

The interface is named `io.subportal`. All methods accept an optional `host`
parameter (string) that identifies the originating server's hostname.
Server-side tools set this automatically via `gethostname(2)`. The `host`
field is a transport-level concern: the client library injects it into the
JSON before sending, and the server extracts it before deserializing into the
typed request. It does not appear in the typed `Request` enum.

```
interface io.subportal

method Ping(host: ?string) -> (capabilities: []string, version: string, clients: []string, endpoint_id: string)

method OpenURI(uri: string, host: ?string) -> ()

method OpenFile(
    name: string,
    mime: string,
    content: string,
    host: ?string
) -> ()

method Notify(
    title: string,
    body: ?string,
    urgency: ?string,
    icon: ?string,
    host: ?string
) -> (id: string)

method NotifyDismiss(id: string) -> ()

error io.subportal.UserDenied ()
error io.subportal.NotSupported (capability: string)
error io.subportal.FileTooLarge (max_bytes: int)
error io.subportal.NoClient ()
error io.subportal.NotFound (what: string)
```

## Methods

### Ping

Check connectivity and discover daemon capabilities.

**Parameters:**

| Name   | Type    | Required | Description                    |
| ------ | ------- | -------- | ------------------------------ |
| `host` | string  | no       | Hostname of the originating server |

**Returns:**

| Name           | Type      | Description                          |
| -------------- | --------- | ------------------------------------ |
| `capabilities` | []string  | List of supported capabilities       |
| `version`      | string    | Daemon protocol version              |
| `clients`      | []string  | Names of connected desktop clients   |
| `endpoint_id`  | string    | Agent's iroh endpoint ID             |

V1 capabilities: `OpenURI`, `OpenFile`, `Notify`.

### OpenURI

Open a URI in the client's default application. A confirmation dialog is
shown to the user before opening.

**Parameters:**

| Name   | Type    | Required | Description                    |
| ------ | ------- | -------- | ------------------------------ |
| `uri`  | string  | yes      | The URI to open                |
| `host` | string  | no       | Hostname of the originating server |

**Returns:** empty object on success.

**Errors:** `UserDenied` if the user rejects the confirmation dialog.

### OpenFile

Transfer a file and open it on the client. The file content is base64-encoded
in the request. A confirmation dialog shows the file name, size, and MIME type.

**Parameters:**

| Name      | Type    | Required | Description                              |
| --------- | ------- | -------- | ---------------------------------------- |
| `name`    | string  | yes      | File name (used in confirmation dialog)  |
| `mime`    | string  | yes      | MIME type (e.g. `application/pdf`)       |
| `content` | string  | yes      | Base64-encoded file content              |
| `host`    | string  | no       | Hostname of the originating server       |

**Returns:** empty object on success.

**Errors:**
- `UserDenied` if the user rejects the confirmation dialog
- `FileTooLarge` if the decoded content exceeds 5 MB

The daemon writes received files to `$XDG_RUNTIME_DIR/subportal/<name>` before
opening them.

### Notify

Show a desktop notification. No confirmation is required.

**Parameters:**

| Name      | Type    | Required | Description                              |
| --------- | ------- | -------- | ---------------------------------------- |
| `title`   | string  | yes      | Notification title                       |
| `body`    | string  | no       | Notification body text                   |
| `urgency` | string  | no       | `low`, `normal`, or `critical`           |
| `icon`    | string  | no       | Icon name (e.g. `dialog-information`)    |
| `host`    | string  | no       | Hostname of the originating server       |

**Returns:**

| Name | Type   | Description                                      |
| ---- | ------ | ------------------------------------------------ |
| `id` | string | Agent-assigned notification ID for dismiss tracking |

The daemon sets the notification app name to `subportal@<host>` when a host
is provided, or `subportal` otherwise.

When the agent forwards a `Notify` request to clients (fan-out routing), it
injects an additional `notification_id` field into the wire parameters. Clients
use this ID to map their local notification IDs back to the agent-level ID for
cross-device dismiss tracking via `NotifyDismiss`.

### NotifyDismiss

Dismiss a notification across all connected clients. When a notification is
dismissed on one device, the agent broadcasts the dismissal to all other
devices that received it.

**Parameters:**

| Name | Type   | Required | Description                              |
| ---- | ------ | -------- | ---------------------------------------- |
| `id` | string | yes      | The notification ID returned by `Notify` |

**Returns:** empty object on success.

## Errors

### io.subportal.UserDenied

The user declined the confirmation dialog (OpenURI/OpenFile only).

**Parameters:** none.

### io.subportal.NotSupported

The daemon does not support the requested capability.

**Parameters:**

| Name         | Type   | Description                              |
| ------------ | ------ | ---------------------------------------- |
| `capability` | string | The unsupported capability name          |

### io.subportal.FileTooLarge

The file exceeds the maximum allowed size.

**Parameters:**

| Name        | Type | Description                                |
| ----------- | ---- | ------------------------------------------ |
| `max_bytes` | int  | Maximum allowed file size in bytes         |

### io.subportal.NoClient

The daemon is not reachable. This error is generated client-side by the
library when the socket connection fails; it is never sent over the wire.

**Parameters:** none.

### io.subportal.NotFound

The referenced resource (e.g. a client name or endpoint ID) was not found.

**Parameters:**

| Name   | Type   | Description                              |
| ------ | ------ | ---------------------------------------- |
| `what` | string | Description of what was not found        |

## Wire examples

### Ping

Request:
```json
{"method":"io.subportal.Ping","parameters":{"host":"myserver"}}
```

Response:
```json
{"parameters":{"capabilities":["OpenURI","OpenFile","Notify"],"version":"0.2.0","clients":["laptop"],"endpoint_id":"abc123"}}
```

### OpenURI

Request:
```json
{"method":"io.subportal.OpenURI","parameters":{"uri":"https://example.com","host":"myserver"}}
```

Success response:
```json
{"parameters":{}}
```

### Error response

```json
{"error":"io.subportal.UserDenied","parameters":{}}
```

## Limits

| Limit              | Value   |
| ------------------- | ------- |
| Max file size       | 5 MB (5,242,880 bytes) |
| Max message size    | 8 MB (8,388,608 bytes) |
| Connections per request | 1 (one-shot) |
