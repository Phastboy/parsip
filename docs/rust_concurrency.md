# Practical Rust Concurrency

Rust's defining feature is "Fearless Concurrency." In languages like C or C++, sharing data between threads is a minefield of race conditions, segfaults, and deadlocks. Rust's compiler guarantees memory safety at compile time. 

Here is how Parsip leverages Rust's concurrency model.

## 1. Message Passing (`std::sync::mpsc`)

*"Do not communicate by sharing memory; instead, share memory by communicating."*

This philosophy is central to Parsip. We use `mpsc` (Multi-Producer, Single-Consumer) channels heavily.

### Example: The Central Event Loop
Parsip has many things happening at once:
1.  TCP Reader threads are receiving data from peers.
2.  UDP Listener threads are discovering new peers.
3.  The CLI API is receiving commands from the user.

Instead of all these threads trying to lock and mutate shared state (like a global list of peers), they all **send messages** to a single central channel.

```rust
pub enum PeerEvent {
    NewConnection(PeerId),
    Message(PeerId, Message),
    ControlRequest(ControlMessage, Sender<ControlResponse>),
}
```

The main thread sits in a loop, pulling events out of this channel one by one. Because only the main thread processes these events, it can safely mutate the application state without needing any `Mutex` locks! 

## 2. Ownership Across Threads (The `Send` Trait)

When we moved disk I/O off the event loop, we needed to pass chunks of downloaded data (byte arrays) to the background writer thread.

```rust
// Event loop receives data from the network
let data: &[u8] = network_buffer;

// Clone the data to create an owned Vec<u8>
let owned_data = data.to_vec();

// Send ownership across the channel to the writer thread
download.writer_tx.send(Some(owned_data));
```

Notice the `.to_vec()` clone. 
The event loop only has a temporary reference (`&[u8]`) to the data sitting in the network buffer. If we tried to send that reference to another thread, the compiler would stop us: *What if the event loop overwrites the network buffer before the writer thread finishes writing it to disk?*

By cloning it into a `Vec<u8>`, we create a brand new allocation. We then pass **ownership** of that allocation into the channel. The writer thread receives the `Vec<u8>`, writes it to disk, and when the variable goes out of scope, the memory is safely freed. Rust guarantees no two threads can mutate it simultaneously.

## 3. Shared State (`Arc<Mutex<T>>`)

Sometimes message passing isn't enough, and you *must* share state.

In Parsip, when the Event Loop wants to send a message *out* to a peer, it needs access to the TCP connection. But wait—we said earlier that there is a dedicated Reader Thread constantly reading from that same TCP connection.

How do two threads share one TCP connection?

```rust
let connection = Arc::new(Mutex::new(tcp_connection));
```

*   **`Mutex` (Mutual Exclusion):** Ensures only one thread can access the connection at a time. If the Event Loop wants to write, it locks the mutex. If the Reader Thread tries to read at the same moment, it must wait its turn.
*   **`Arc` (Atomic Reference Counted):** A smart pointer that allows multiple threads to "own" the Mutex. It keeps a count. When the Event Loop drops its `Arc`, the count goes down. When the Reader Thread drops its `Arc`, the count goes to zero, and the connection is safely closed and deallocated.

> [!WARNING]
> **The Mutex Bottleneck**
> While safe, `Mutex` contention is a performance killer. If you are uploading two files to the same peer simultaneously, Thread A and Thread B will fight over the Mutex lock for the connection.
> **Future Architecture Upgrade:** We plan to remove this Mutex entirely. Instead, each connection will have a dedicated *Writer Thread* with its own `mpsc` channel. All other threads will just drop frames into the channel, completely eliminating lock contention.
