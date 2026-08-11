# Networking Fundamentals

To understand Parsip, you need to understand how computers talk to each other on a local network. This guide breaks down the networking concepts we rely on.

## 1. TCP vs. UDP: Choosing the Right Tool

The internet runs primarily on two transport protocols: TCP and UDP. Parsip uses **both**, but for entirely different reasons.

### TCP (Transmission Control Protocol)
*   **Use case in Parsip:** Transferring files (`parsip get`).
*   **Analogy:** A phone call. You dial, establish a connection, and both sides know the other is listening. If you say something and they don't hear it, they ask you to repeat it.
*   **Characteristics:**
    *   **Reliable:** Guarantees data arrives in the exact order it was sent. If a packet is lost, TCP automatically retransmits it.
    *   **Stream-oriented:** There are no "message boundaries." If you send 10 bytes and then 20 bytes, the receiver might read all 30 bytes at once, or 5 bytes then 25 bytes. It's just a continuous pipe of water.
    *   **Flow Control:** If the receiver's disk is slow, TCP automatically tells the sender to slow down (backpressure).

### UDP (User Datagram Protocol)
*   **Use case in Parsip:** Peer Discovery (finding who is on the network).
*   **Analogy:** Shouting in a crowded room. You yell a message, hoping someone hears it. You don't know who received it, and you get no confirmation.
*   **Characteristics:**
    *   **Unreliable:** Packets can be dropped, duplicated, or arrive out of order.
    *   **Datagram-oriented:** Message boundaries are preserved. If you send a 50-byte packet, the receiver gets exactly one 50-byte packet.
    *   **Broadcast Capable:** This is the killer feature. With TCP, you must know the exact IP address of the target. With UDP, you can send a packet to a special "broadcast" address, and the router delivers it to *everyone* on the network.

## 2. Peer Discovery: How to find friends

When you start Parsip, it doesn't know who else is online. It uses **UDP Broadcasting** to find out.

Instead of asking a central server "who is online?" (which requires internet access and centralized infrastructure), Parsip yells to the local network: *"I am Parsip node X, I am listening on port 9000!"*

> [!IMPORTANT]
> **Subnet Broadcast vs. Global Broadcast**
> Initially, Parsip broadcasted to `255.255.255.255`. This means "literally everyone." However, modern routers often block this to prevent network storms.
> We updated Parsip to use **Subnet Broadcasts**. If your IP is `192.168.0.50` with a subnet mask of `255.255.255.0`, your subnet broadcast address is `192.168.0.255`. This tells the router: *"Send this only to devices on my specific local network slice."* Routers allow this.

## 3. Framing: Making sense of the TCP stream

Remember that TCP is just a continuous stream of bytes. 

If Node A sends two messages:
1. `"HELLO"` (5 bytes)
2. `"WORLD"` (5 bytes)

Node B's TCP reader might wake up and read `"HELL"`, then later read `"OWORLD"`. How does Node B know where one message ends and the next begins?

This is called **Framing**.

Parsip uses a **Length-Prefix Codec**. Before sending any data, we send a 4-byte integer representing the exact size of the upcoming message.

```text
[ 0 0 0 5 ] [ H E L L O ] [ 0 0 0 5 ] [ W O R L D ]
  ^-- 4 bytes              ^-- 4 bytes
```
Node B reads exactly 4 bytes to find the length (5), then reads exactly 5 bytes to get the payload.

## 4. Endianness (Network Byte Order)

When sending that 4-byte integer (e.g., the number `5`), how do we represent it in bytes?
*   **Big Endian:** `[0, 0, 0, 5]` (Most significant byte first).
*   **Little Endian:** `[5, 0, 0, 0]` (Least significant byte first).

Most modern CPUs (Intel, AMD, ARM) are Little Endian. However, networking protocols universally agreed decades ago to use **Big Endian** (also known as Network Byte Order). 

If a Windows PC (Little Endian) sends a length to a Mac (also Little Endian), they both must convert the number to Big Endian before sending, and convert it back upon receiving. Parsip does this automatically in its Codec.

## 5. TCP Keepalives: Dealing with sleepy phones

In a perfect world, when a computer disconnects, it sends a TCP `FIN` (Finish) or `RST` (Reset) packet, politely telling the other side to close the connection.

In the real world (especially with Android phones), devices just vanish. Mobile OSes aggressively put WiFi chips to sleep to save battery. When this happens, no `FIN` packet is sent. 

If Parsip is downloading a file when the phone sleeps, the laptop's TCP socket just sits there waiting for the next byte... forever.

To fix this, we enable **TCP Keepalives**.
We tell the Operating System: *"If this connection is completely silent for 10 seconds, send a tiny, invisible 'Are you there?' probe. If you don't get a reply after 3 attempts, kill the connection."*

This allows Parsip to quickly detect dead peers, cancel the frozen download, and unblock the user interface within ~25 seconds, rather than hanging indefinitely.
