# Parsip

Parsip is a decentralized, high-speed, peer-to-peer file sharing application designed for local area networks (LANs). It allows you to seamlessly share files across your devices (Laptops, Android via Termux, etc.) without relying on the internet, central servers, or cloud storage.

## Features
- **Decentralized Discovery**: Finds peers on your LAN automatically using UDP broadcasting.
- **Cryptographic Identity**: Ed25519 keypairs ensure that your devices recognize each other securely without centralized logins.
- **High Performance**: Custom background worker threads for disk I/O ensure your network transfer speeds are never bottlenecked by slow hard drives.
- **Daemon Architecture**: Runs silently in the background, allowing you to close your terminal without killing active file transfers.

## Installation

### Method 1: Pre-compiled Binaries (Recommended)
You do not need to install Rust to run Parsip. Simply head to the [Releases Page](../../releases) on GitHub and download the binary for your system:
* `parsip-linux-x86_64` (Standard Linux)
* `parsip-linux-aarch64` (ARM / Raspberry Pi / Android Termux)
* `parsip-macos-x86_64` & `parsip-macos-aarch64` (Apple)
* `parsip-windows-x86_64.exe` (Windows)

Make the file executable and move it to your path:
```bash
chmod +x parsip-linux-x86_64
mv parsip-linux-x86_64 /usr/local/bin/parsip
```

### Method 2: Build from Source
If you have `cargo` installed:
```bash
cargo install --path .
```
*(Note: A `cargo install parsip` via crates.io is coming soon!)*

## Usage

### 1. Start the Daemon
First, start the background daemon. You only need to do this once per device.
```bash
parsip daemon start --nickname mylaptop
```
This generates your cryptographic identity and begins broadcasting your presence to the local network.

### 2. Add Files to Share
Add a file to your local resource store so peers can download it.
```bash
parsip add ~/Downloads/movie.mp4
# Returns a resource alias like 'r1'
```

### 3. Discover and Connect
Scan the network for other Parsip nodes.
```bash
parsip scan
# Discovers peers like 'myphone' (alias 'p1')

parsip connect p1
```

### 4. Download Files
List the files available on the connected peer.
```bash
parsip list p1
# Shows available files with their resource aliases (e.g., 'r2')

parsip get p1 r2
```

## Documentation

For a deep dive into how Parsip works under the hood (including Networking fundamentals and Rust concurrency), check out the [`docs/` directory](./docs/).

## License
MIT OR Apache-2.0
