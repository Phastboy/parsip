# TCP & Rust Networking Notes

This document captures the key networking and Rust concepts learned while building Parsip.

## 1. TCP Fundamentals
- **Stream of Bytes, Not Messages**: TCP provides an ordered, reliable stream of bytes. It does not promise that bytes sent together (e.g., `write("hello"); write("world");`) will be received together. The receiver might read them as `"helloworld"`, `"hel"` and `"loworld"`, or exactly as sent. 
- **The Listening Endpoint (`TcpListener`)**: Represents the server socket bound to a specific IP address and port (e.g., `0.0.0.0:9000`). Its sole responsibility is to listen for incoming connection requests.
- **The Peer Connection (`TcpStream`)**: Represents the actual 1-to-1 communication channel between the server and a specific client.
- **Accepting Connections**: Calling `listener.accept()` blocks until a client connects. It returns a tuple of `(TcpStream, SocketAddr)`, giving you both the communication channel and the address of the peer who connected.

## 2. Rust Idioms & Best Practices
- **Error Propagation (`Result` and `?`)**: Instead of panicking when an error occurs (like a port already being in use), idiomatic Rust returns a `Result<T, std::io::Error>`. The `?` operator allows for seamless error propagation up the call stack, letting the caller (or the OS, if bubbling out of `main`) decide how to handle it.
- **Zero-Cost Abstractions**: Rust's `std::io::Read` requires the caller to provide a mutable slice (`&mut [u8]`) rather than returning a dynamically allocated array. This prevents hidden heap allocations, prevents memory exhaustion from malicious clients, and maps cleanly to the underlying OS `read(2)` syscall.
- **Borrowing over Owning**: When storing configuration (like an address string) that doesn't need to be modified or dynamically allocated, using borrowed string slices (`&'a str`) inside structs is highly idiomatic. It avoids unnecessary heap allocations (like `.to_string()`) and enforces strict compile-time checks on the lifetime of the data. 

## 3. The Conceptual Separation
It's important to separate concerns structurally:
1. **The Server Config**: Owns the settings (e.g., the address to bind to).
2. **The Listener**: Owns the OS-level listening socket and accepts connections.
3. **The Stream**: Owns the bidirectional data flow with a specific peer.

## 4. Application Framing
- **TCP is Unaware of Messages**: Because TCP only streams bytes and doesn't preserve write boundaries, application protocols must implement **framing** to separate discrete messages.
- **Length-Prefix Framing**: A common technique is to precede every message with a fixed-size header that encodes the length of the upcoming payload.
- **Byte Order (Endianness)**: When sending integers (like a 4-byte `u32` length) over the network, it is standard practice to use **big-endian** (network byte order). In Rust, this is easily done via `u32::to_be_bytes()`.
- **Parsing the Frame**: The receiver's protocol is responsible for reading the exact number of header bytes (e.g., 4 bytes), decoding the integer, and then reading exactly that many payload bytes from the stream to reconstruct the message.
- **Option B Framing (Length = Type + Payload)**: A robust framing layer dictates that the `length` prefix describes the *entire* remaining frame body (type + payload). This ensures the generic codec layer can extract the frame without needing to know the application-specific byte-layout of the message inside.
- **Enums for Network Bytes**: When reading a `u8` from the network to represent a command (e.g., `1` for `Hello`), the application shouldn't juggle raw bytes. Idiomatic Rust parses that `u8` into an `enum` using `TryFrom<u8>`. This creates a strong typed boundary where invalid bytes (like `255`) are rejected as protocol errors immediately.
- **Traits for Codecs**: By abstracting encoding and decoding behind `Encoder` and `Decoder` traits, the application logic is decoupled from the wire format. A `LengthPrefixCodec` can be easily swapped for a `JsonCodec` or `LineCodec` without changing the core transport logic.
