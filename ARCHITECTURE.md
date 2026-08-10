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

- **Storage**: Identities are persisted locally (e.g., `~/.parsip/identity.key`) so a peer remains the same logical participant across restarts, IP changes, and NATs.
- **Authentication**: While currently a randomly generated `[u8; 32]`, the `PeerId` is designed to evolve into a public key hash (`hash(public_key)`), allowing the `Connection` handshake to cryptographically authenticate peers.

---

## 3. The Protocol Lifecycle Progression

Parsip is being built according to the following evolutionary stages:

### Stage 1: Identity
Establishing the `PeerId` structure and persistence.

### Stage 2: Connection Identity
Performing synchronous handshakes to prove identity upon TCP connection, resulting in a `HashMap<PeerId, Connection>`.

### Stage 3: Peer Lifecycle
Handling the complex realities of networking: disconnects, reconnects, duplicate connections, and avoiding stale-cleanup races when managing the connection pool.

### Stage 4: Peer-to-Peer Messaging
Exposing clean primitives on the `Peer` to route frames to specific `PeerId`s asynchronously.

### Stage 5: Resource Protocol
Finally layering on the actual semantic purpose of Parsip: sharing resources via `LIST_RESOURCES`, `GET_RESOURCE`, and `RESOURCE_DATA`.
