# Architectural Decisions

When building software, every major decision is a trade-off. This document explains *why* Parsip is architected the way it is.

## 1. The Daemon + CLI Split

Parsip is split into two distinct pieces:
1.  **The Daemon (`parsip daemon`):** A long-running background process. It holds the TCP sockets open, listens for UDP broadcasts, and reads/writes files to disk.
2.  **The CLI (`parsip get`, `parsip list`):** A transient tool that briefly connects to the daemon, issues a command, and prints the result.

### Why not just one program?
If Parsip was a single program, closing your terminal would sever all your connections and kill active downloads. By running a daemon, Parsip acts like a true network service. You can close your laptop lid, reopen it, and the daemon is still there, ready to serve files to peers.

Furthermore, this architecture allows us to build **different frontends** later. We can build a graphical UI, a web interface, or an Android app that all talk to the exact same Core Daemon API.

## 2. Why OS Threads instead of `tokio` (Async Rust)?

In the modern Rust ecosystem, `tokio` (an asynchronous runtime) is the default choice for networking. We explicitly chose **not** to use it.

### The "10k Connection" Myth
Async programming (like `tokio` or Node.js) was invented to solve the C10k problem: how to handle 10,000 simultaneous connections. If you spawn 10,000 OS threads, the memory overhead (stack size) and context-switching cost will crash your server. Async uses a single thread (or a small pool) to juggle thousands of connections efficiently.

**Parsip is a LAN tool.** A single node will connect to 5, 10, maybe 50 peers at most. 

At this scale:
*   OS threads have near-zero overhead.
*   Threads are vastly simpler to reason about.
*   We don't have to deal with the viral nature of `async fn` (where making one function async forces every calling function to also become async).

### The Disk I/O Trap
Async runtimes are terrible at blocking operations like reading/writing from a hard drive. If you block an async worker thread while waiting for a disk spin, you freeze hundreds of other network connections. You have to use special `spawn_blocking` workarounds.

With OS threads, if a thread blocks on a slow disk write, the OS simply schedules a different thread. It just works.

## 3. Decentralized Identity (Why not LDAP?)

LDAP (Lightweight Directory Access Protocol) is a way to centrally manage users (like corporate logins). 

Parsip is fundamentally **decentralized**. There is no central server, no internet requirement, and no admin.

Instead of LDAP, Parsip uses **Public-Key Cryptography (ed25519)**. 
When you start the daemon for the first time, it generates a unique, permanent private/public keypair. Your public key *is* your identity (`PeerId(c9d4..6764)`). 
When you connect to someone, you prove you own that identity using cryptographic signatures. This ensures nobody on the LAN can spoof your identity, all without a central server.

## 4. Unblocking the Event Loop (The I/O Bottleneck)

Early versions of Parsip suffered from a massive performance bottleneck.

When downloading a file, the main Event Loop would receive a chunk of data from the network and immediately write it to disk:

```text
[Network Data Arrives] -> [Event Loop] -> [Write to Disk] -> [Wait for Disk to Finish] -> [Process Next Event]
```

If the disk was slow, or the OS decided to flush its buffers, the entire Event Loop froze. This meant we stopped reading from the network, TCP buffers filled up, and the sender's OS slowed down the transfer rate.

### The Fix: Channel-Driven Writer Threads
We decoupled the network from the disk using `mpsc` (Multi-Producer, Single-Consumer) channels.

Now, every active download spawns a dedicated **Writer Thread**.
When the Event Loop receives data, it simply drops the data into a channel and instantly moves on to the next network packet. The Writer Thread independently pulls data from the channel and writes it to disk at its own pace.

```text
[Event Loop] -> (Instant Channel Send) -> [Process Next Event]
                      |
                      v
              [Writer Thread] -> [Write to Disk]
```
This single architectural change dramatically increased download speeds.
