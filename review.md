# Parsip Architecture Review & Roadmap

## 2.74 MB/s — What does this mean?

This is excellent. 2.4 GHz 802.11n practical ceiling is ~20-25 Mbps = **~3 MB/s**.
You're at **91% of the physical ceiling**. The software is no longer the bottleneck — the radio is.

Connecting both devices to the **5 GHz band** would give you ~150 Mbps practical = ~18 MB/s.

---

## Do we need Tokio?

**No.**

Here's the honest breakdown:

| Scenario                      | Tokio wins                       | Threads win                   |
| ----------------------------- | -------------------------------- | ----------------------------- |
| 10,000 concurrent connections | ✅                               | ❌ (10k threads = 80GB stack) |
| 10–100 peers (parsip)         | ❌ (overhead)                    | ✅                            |
| Blocking disk I/O             | ❌ (`spawn_blocking` complexity) | ✅ (trivial)                  |
| Simple reasoning about code   | ❌                               | ✅                            |
| `async` virality              | ❌ (infects everything)          | ✅                            |

Parsip will realistically handle tens of peers. At that scale, threads are simpler, faster to reason about, and have zero runtime overhead. Every `async fn` in Tokio has a state machine cost; every `await` is a potential suspension point you must think about. We'd be rewriting the entire codebase for zero real gain.

**Stay with threads.**

---

## Do we need LDAP?

**No.** LDAP is an enterprise directory protocol for centralised user identity (Active Directory, corporate logins). Parsip is decentralised and already has its own identity via ed25519 keypairs. LDAP would be the opposite of what this project is.

---

## Daemon + CLI — is this right?

**Yes, and it's the right foundation.** Here's why:

The daemon model means parsip keeps running, accepting connections, and serving files even when you're not actively typing commands. The CLI is just one frontend. The daemon's control API (`127.0.0.1:9091` right now, but should move to a Unix socket) can serve any frontend:

```
parsip daemon (always running)
       │
       ├── CLI (parsip scan / get / list)
       ├── TUI (future: rich terminal interface)
       └── Native app (future: Android/iOS via REST or gRPC)
```

**Android native terminal** (the built-in one in Android 14+) is real but very restricted — no package manager, no background services, no TCP sockets in some cases. Termux is still the way to go for serious usage. The daemon model works perfectly on Termux.

---

## The architecture diagram — should you pursue it?

```
                Parsip Core
                     │
          ┌──────────┴──────────┐
          │                     │
       Protocol              Resource
          │                     │
          └──────────┬──────────┘
                     │
                 Node API
                     │
          ┌──────────┴──────────┐
          │                     │
        Desktop              Mobile
```

**Yes — and you're already closer to this than you think.**

| Layer    | Current state                                    |
| -------- | ------------------------------------------------ |
| Protocol | `src/protocol/` — framing, messages, codec ✅    |
| Resource | `src/resource/` — store, download, transfer ✅   |
| Node API | `src/daemon/control.rs` — partial, TCP socket ⚠️ |
| Desktop  | `src/cli/` — bare CLI ✅                         |
| Mobile   | Nothing yet ❌                                   |

The evolution is to formally separate these layers and make the **Node API** a proper, documented interface. Right now it's ad-hoc JSON over a TCP socket. That's actually fine — but the API surface needs to grow and stabilise before building native UIs on top of it.

---

## Multi-peer, Multi-transfer — Do we need changes?

**Mostly works already. One bottleneck remains.**

### What already works:

- **Download from peer A while peer B downloads from you**: ✅ Separate reader/writer threads per connection
- **Download from peer A and peer B simultaneously**: ✅ Each `ResourceChunk` has a `request_id`, each download has its own writer thread, the event loop routes them independently in microseconds
- **Multiple peers uploading to you at the same time**: ✅ Each sender has its own reader thread

### What has a bottleneck:

**Sending to the same peer from multiple threads** (e.g. uploading two files simultaneously to one peer):

```
Upload thread A: wants to send chunk → locks Connection mutex → writes 128KB → releases
Upload thread B: wants to send chunk → BLOCKED waiting for mutex
```

This creates head-of-line blocking. It doesn't corrupt data, but it serialises all sends to any given peer.

### The fix: per-connection writer queue

```rust
// Instead of Arc<Mutex<Connection>>, each connection gets:
Connection {
    writer_tx: Sender<Frame>,  // send frames here, non-blocking
}

// A writer thread per connection drains frames in order:
writer thread: loop { frame = rx.recv(); write to TcpStream }
```

This means all threads can send frames to any peer instantly (non-blocking channel send). The writer thread serialises them onto the wire in FIFO order. No mutex contention. True non-blocking bidirectional communication.

---

## What's missing from the full vision

### 🔴 Must-have for reliability

- [ ] **SHA256 verification after download** — currently you trust the bytes are correct; never verified
- [ ] **Resume interrupted downloads** — if a 1 GB transfer cuts at 900 MB, start from where it left off
- [ ] **Per-connection writer thread** — eliminate send-side mutex contention (see above)
- [ ] **Unix socket for control API** — replace `127.0.0.1:9091` with `~/.parsip/daemon.sock` (faster, no port conflict, no security surface)

### 🟡 Important for UX

- [ ] **Transfer queue** — `parsip get p1 r1 r2 r3` queues all three
- [ ] **Auto-reconnect** — if a known peer appears, reconnect automatically
- [ ] **Watch folder** — any file added to `~/.parsip/shared/` is automatically available
- [ ] **Transfer history** — log of what was sent/received
- [ ] **TUI** — replace the bare CLI with a live terminal UI (progress bars, peer list)

### 🟡 Important for scalability

- [ ] **mDNS/Bonjour discovery** — more reliable than UDP broadcast, works across subnets
- [ ] **Connection pooling** — reconnect automatically if a peer drops and comes back
- [ ] **Bandwidth throttling** — limit upload speed so sharing doesn't saturate your connection

### 🟢 Longer term

- [ ] **End-to-end encryption** — currently authenticated (ed25519) but data is plaintext on the wire
- [ ] **NAT traversal** — hole punching for internet transfers (not just LAN)
- [ ] **Multi-source download** — download the same file from multiple peers simultaneously (BitTorrent-style)
- [ ] **Native mobile app** — Kotlin/Flutter talking to the daemon's Node API
- [ ] **Resource announcements** — when you connect to a peer, auto-push your resource list

### What NOT to build

- ❌ **LDAP** — centralised directory, opposite of decentralised
- ❌ **Tokio** — wrong scale, wrong complexity tradeoff
- ❌ **DHT (Distributed Hash Table)** — massive complexity for LAN use case (maybe later for internet)

---

## Recommended next steps (in order)

1. **Per-connection writer thread** — unblocks true simultaneous multi-transfer
2. **Unix socket for control API** — cleaner, faster, no port conflicts
3. **SHA256 verification** — data integrity
4. **Resume downloads** — resilience
5. **Transfer queue** — UX
6. **TUI** — UX
