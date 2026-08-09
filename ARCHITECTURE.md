# Parsip Architecture

Parsip is a decentralized, peer-to-peer resource-sharing protocol. This document outlines its architectural foundations.

## 1. Unified Peer Architecture
Unlike traditional client/server web architectures, Parsip does not rely on a central server. Every Parsip instance is a **Peer**. 

A Peer acts as both:
- **Acceptor**: Listens for incoming connections from other peers on a background thread.
- **Initiator**: Connects out to other peers.

By running the listener on a separate `std::thread`, a single Parsip binary can concurrently handle an infinite number of incoming connections while independently making outgoing connections.

## 2. Framing and Protocol Boundaries
TCP provides a continuous byte stream, not discrete messages. Parsip implements a strict framing protocol to break this stream into manageable chunks.

### The Wire Format
Every frame sent over the network follows the **Option B** design (Length = Type + Payload):
```text
┌────────────┬──────────┬──────────────────┐
│ 4B length  │ 1B type  │ N bytes payload  │
└────────────┴──────────┴──────────────────┘
```

**Why Option B?**
By making the 4-byte header describe the *entire* remaining frame body (type + payload), the low-level framing codec (`LengthPrefixCodec`) can remain completely agnostic to the application protocol. It simply reads `N` bytes and passes an opaque byte array upwards.

### Codec Layer
The network boundary is guarded by traits:
- `Encoder`: Takes a `Frame` and writes the exact bytes to the stream.
- `Decoder`: Uses `read_exact` to read the 4-byte header, allocates the exact buffer needed, and blocks until the full frame arrives. Any mid-frame disconnect automatically results in an `UnexpectedEof` error, preventing corrupted or partial state.

### Message Types
The single byte immediately following the length header denotes the message semantics. 
```rust
pub enum MessageType {
    Hello = 1,
    ListResources = 2,
    GetResource = 3,
}
```
This enum strictly bounds the application logic, ensuring that any unrecognized byte (e.g., `255`) is rejected as a protocol error immediately by `TryFrom<u8>`.
