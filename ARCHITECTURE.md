# Parsip Architecture

Parsip is a decentralized, peer-to-peer resource-sharing protocol. This document outlines its architectural foundations and the boundaries of its core abstractions.

## 1. Core Abstractions & Responsibilities

To avoid accumulating technical debt as the protocol expands, Parsip strictly separates the concerns of identity, transport, and routing.

### The `Peer` (The Node)
The `Peer` represents the **logical participant** in the network. A Peer is *not* a server or a client—it is an independent node that can simultaneously listen, connect, request, and serve.

**Responsibilities:**
- **Identity Ownership**: Holds the cryptographic `PeerId` that permanently identifies this node across network and IP changes.
- **Connection Lifecycle Management**: Tracks all active relationships in a `HashMap<PeerId, Connection>`. Handles peer deduplication, disconnections, and stale-cleanup races.
- **Routing & Orchestration**: Decides *what* to send and to *whom*. (e.g., `send(PeerB, Frame)`).
- **Resource Management**: (Future) Owns the resource store and handles incoming `ListResources` and `GetResource` requests.

### The `Connection` (The Transport)
The `Connection` represents a **single bidirectional TCP link** between this node and one specific remote peer.

**Responsibilities:**
- **I/O Mechanics**: Wraps the raw `TcpStream`. Exposes clean read/write primitives to the `Peer`.
- **Concurrency**: Manages the split between the background reading thread and the foreground writing mechanisms.
- **Handshaking**: Executes the synchronous cryptographic handshake upon establishment to prove identity before any application data is exchanged.
- **Framing**: Uses the codec layer to translate between raw byte streams and structured protocol frames.

### The `Protocol` (The Semantics)
The `Protocol` dictates the **meaning** of the bytes traversing a `Connection`. 

**Responsibilities:**
- Defines the `[4B length][1B type][N bytes payload]` Option B framing structure.
- Distinctly separates **Control Messages** (e.g., `Hello`, `Ping`, `Pong`) from **Resource Messages** (e.g., `ListResources`, `GetResource`).

---

## 2. The Identity Model

A network endpoint (`SocketAddr` like `192.168.0.2:9000`) is transient. A Parsip identity (`PeerId`) is permanent.

- **Storage**: Identities are persisted locally (e.g., `~/.parsip/identity`) so a peer remains the same logical participant across restarts, IP changes, and NATs.
- **Authentication**: A `PeerId` is derived deterministically from an Ed25519 public key (`SHA-256(pubkey)`). During the `Connection` handshake, peers exchange public keys and sign random nonces (challenge-response) to cryptographically prove ownership of their `PeerId` before any application data is exchanged.

---

## 3. Peer Discovery

Parsip nodes discover each other autonomously without relying on central coordination servers.

- **Local Discovery (UDP Broadcast)**: Nodes on the same local network (e.g., the same WiFi) find each other seamlessly. A background broadcaster routinely shouts its `PeerId` and TCP listen port via UDP to `255.255.255.255:9090`. A background listener catches these broadcasts and pipes `PeerEvent::Discovered` events into the `Peer`'s event loop, which autonomously initiates a TCP connection if one doesn't already exist.

---

## 4. The Protocol Lifecycle Progression

Parsip is being built according to the following evolutionary stages:

### Stage 1: Identity
Establishing the `PeerId` structure, cryptographic ed25519 keypairs, and local persistence.

### Stage 2: Connection Identity
Performing synchronous challenge-response handshakes to cryptographically prove identity upon TCP connection, resulting in a secure `HashMap<PeerId, Connection>`.

### Stage 3: Peer Lifecycle
Handling the complex realities of networking: disconnects, reconnects, duplicate connections, and avoiding stale-cleanup races when managing the connection pool.

### Stage 4: Peer Discovery & Routing
Implementing the Bidirectional Message Dispatcher (an event loop cleanly detached from TCP) and autonomous UDP broadcast discovery so peers can mesh automatically.

### Stage 5: Resource Protocol
Finally layering on the actual semantic purpose of Parsip: sharing resources via `LIST_RESOURCES`, `GET_RESOURCE`, and `RESOURCE_DATA`.
