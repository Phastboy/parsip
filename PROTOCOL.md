# Parsip Wire Protocol

## Framing

Every message on the wire is a length-prefixed frame:

    +----------------+----------+-----------------+
    | length (u32 BE)| type(u8) | payload (N bytes)|
    +----------------+----------+-----------------+

- `length` = 1 (for the type byte) + payload length, big-endian u32.
- `type` = one of the `MessageType` values below.
- `length` of 0 is invalid (type byte is always present).
- Frames larger than `MAX_FRAME_SIZE` (16MB) are rejected before allocation.

## Connection lifecycle

A connection has exactly two phases: **Handshake** and **Established**.

### Handshake phase

Immediately upon TCP connection (inbound or outbound), both sides MUST:

1. Send exactly one `Hello` frame, payload = 32-byte PeerId.
2. Read exactly one `Hello` frame from the peer.

This exchange is symmetric and relies on TCP being full-duplex — both
sides send before reading, so there is no ordering dependency between
the initiator and the acceptor. Neither side is a "client" or "server"
at the protocol level after this point.

Handshake fails (and the connection MUST be closed) if:
- The first frame received is not `Hello`.
- The `Hello` payload is not exactly 32 bytes.
- The connection closes before a `Hello` is received.
- The received PeerId equals the local PeerId (self-connection).

### Established phase

Once both `Hello` frames have been exchanged successfully, the
connection transitions to Established. From this point:

- A `Hello` frame received in the Established phase is a **protocol
  violation** — a peer only identifies itself once, at the start of
  the connection. The receiving side MUST close the connection.
- All other defined message types are only valid in this phase.

## Message types

| Value | Name           | Phase       | Payload                    |
|-------|----------------|-------------|-----------------------------|
| 1     | Hello          | Handshake   | 32-byte PeerId              |
| 2     | ListResources  | Established | (reserved, not yet defined) |
| 3     | GetResource    | Established | (reserved, not yet defined) |

This table will grow as the resource protocol is designed; see the
project's next milestone notes for that work.
