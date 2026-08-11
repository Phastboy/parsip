# Parsip Documentation: The Entrypoint

Welcome to the Parsip developer documentation! 

Parsip is a decentralized, peer-to-peer file-sharing application designed for local area networks (LANs). Building a reliable P2P system requires navigating complex intersections of low-level networking, concurrent programming, and system architecture.

This documentation is designed to be **highly educational**. Whether you are here to understand *how* Parsip works, learn the fundamentals of network programming, or see practical Rust concurrency in action, this is your starting point.

## Table of Contents

Navigate through the documentation based on what you want to learn:

### 1. [Networking Fundamentals](./networking.md)
If you are new to network programming, start here. This guide explains the core concepts Parsip relies on, stripping away the magic of the internet.
*   **TCP vs. UDP:** Why we use both and when.
*   **Framing:** How to extract meaningful messages from an endless stream of bytes.
*   **Network Byte Order (Endianness):** Speaking the same language across different CPU architectures.
*   **TCP Keepalives:** How to detect when a mobile phone goes to sleep.
*   **UDP Broadcasts:** How peers find each other without a central server.

### 2. [Architectural Decisions](./architecture.md)
Learn *why* Parsip is built the way it is. We break down the trade-offs and decisions that shape the codebase.
*   **The Daemon + CLI Model:** Why separating the background service from the user interface is crucial.
*   **Threads vs. Async (Tokio):** Why we explicitly rejected the popular `tokio` runtime for standard OS threads.
*   **Decentralized Identity:** Why we use cryptographic keys instead of systems like LDAP.
*   **The Event Loop vs. Disk I/O:** How we prevent slow hard drives from killing network speeds.

### 3. [Practical Rust Concurrency](./rust_concurrency.md)
A bonus guide on how Rust's safety guarantees shape our concurrent design.
*   **Message Passing (`mpsc`):** The backbone of Parsip's internal communication.
*   **Shared State (`Arc<Mutex<T>>`):** When to use it, and why it becomes a bottleneck.
*   **Fearless Concurrency:** How the compiler prevents data races when managing multiple active downloads.

---

> [!TIP]
> **How to use these docs:** Start with the Networking Fundamentals if you are fuzzy on how data actually moves across wires. If you want to understand the codebase structure immediately, jump into the Architectural Decisions.
