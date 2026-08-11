use socket2::{Domain, Protocol, Socket, Type};
use std::io::Error;
use std::net::{IpAddr, Ipv4Addr, SocketAddr, SocketAddrV4, UdpSocket};
use std::sync::mpsc::Sender;
use std::thread;

use crate::identity::PeerId;
use crate::peer::PeerEvent;

pub const DISCOVERY_PORT: u16 = 9090;
pub const MAGIC_REQUEST: &[u8; 6] = b"PARREQ";
pub const MAGIC_RESPONSE: &[u8; 6] = b"PARRES";

pub struct Discovery;

impl Discovery {
    pub fn start(
        tcp_listen_port: u16,
        my_id: PeerId,
        my_nickname: String,
        event_tx: Sender<PeerEvent>,
    ) -> Result<(), Error> {
        let listen_addr = SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::UNSPECIFIED, DISCOVERY_PORT));

        let sock = Socket::new(Domain::IPV4, Type::DGRAM, Some(Protocol::UDP))?;
        sock.set_reuse_address(true)?;
        #[cfg(target_os = "linux")]
        sock.set_reuse_port(true)?;
        sock.bind(&listen_addr.into())?;
        sock.set_broadcast(true)?;
        let socket = UdpSocket::from(sock);

        let socket_clone = socket.try_clone()?;

        thread::spawn(move || {
            let mut buf = [0u8; 1024];
            loop {
                match socket.recv_from(&mut buf) {
                    Ok((amt, src)) => {
                        if amt >= 40 {
                            let is_request = &buf[0..6] == MAGIC_REQUEST;
                            let is_response = &buf[0..6] == MAGIC_RESPONSE;

                            if is_request || is_response {
                                let mut port_bytes = [0u8; 2];
                                port_bytes.copy_from_slice(&buf[6..8]);
                                let remote_tcp_port = u16::from_be_bytes(port_bytes);

                                let mut peer_id_bytes = [0u8; 32];
                                peer_id_bytes.copy_from_slice(&buf[8..40]);
                                let remote_peer_id = PeerId::from_bytes(peer_id_bytes);

                                let nickname = if amt > 40 {
                                    String::from_utf8_lossy(&buf[40..amt]).to_string()
                                } else {
                                    "Unknown".to_string()
                                };

                                if remote_peer_id != my_id {
                                    if is_request {
                                        let mut payload =
                                            Vec::with_capacity(40 + my_nickname.len());
                                        payload.extend_from_slice(MAGIC_RESPONSE);
                                        payload.extend_from_slice(&tcp_listen_port.to_be_bytes());
                                        payload.extend_from_slice(&my_id.to_bytes());
                                        payload.extend_from_slice(my_nickname.as_bytes());
                                        let mut reply_addr = src;
                                        reply_addr.set_port(DISCOVERY_PORT);
                                        let _ = socket_clone.send_to(&payload, reply_addr);
                                    }

                                    let mut target_addr = src;
                                    target_addr.set_port(remote_tcp_port);
                                    let _ = event_tx.send(PeerEvent::Discovered(
                                        remote_peer_id,
                                        target_addr,
                                        nickname,
                                    ));
                                }
                            }
                        }
                    }
                    Err(e) => eprintln!("Discovery receive error: {}", e),
                }
            }
        });

        Ok(())
    }

    pub fn broadcast_scan(
        tcp_listen_port: u16,
        my_id: &PeerId,
        my_nickname: &str,
    ) -> Result<(), Error> {
        let mut payload = Vec::with_capacity(40 + my_nickname.len());
        payload.extend_from_slice(MAGIC_REQUEST);
        payload.extend_from_slice(&tcp_listen_port.to_be_bytes());
        payload.extend_from_slice(&my_id.to_bytes());
        payload.extend_from_slice(my_nickname.as_bytes());

        // Send to both the limited broadcast AND each local subnet's directed broadcast.
        // Directed broadcast (e.g. 192.168.0.255) works on most home routers
        // where 255.255.255.255 is blocked. We do both to maximise discovery.
        let targets = Self::collect_broadcast_addrs();

        let sock = Socket::new(Domain::IPV4, Type::DGRAM, Some(Protocol::UDP))?;
        sock.set_broadcast(true)?;
        let broadcaster = UdpSocket::from(sock);

        for target in targets {
            let addr = SocketAddr::V4(SocketAddrV4::new(target, DISCOVERY_PORT));
            let _ = broadcaster.send_to(&payload, addr);
        }

        Ok(())
    }

    /// Collect all useful broadcast addresses: the limited broadcast (255.255.255.255)
    /// plus a directed subnet broadcast for every local IPv4 interface.
    fn collect_broadcast_addrs() -> Vec<Ipv4Addr> {
        let mut addrs = vec![Ipv4Addr::BROADCAST]; // 255.255.255.255

        // Try to enumerate local interfaces via /proc/net/if_inet6 alternative:
        // the simplest portable way is to try binding UDP sockets and reading
        // the local address. Instead, use a known-good fallback approach:
        // read /proc/net/fib_trie on Linux for interface prefixes.
        if let Ok(entries) = std::fs::read_to_string("/proc/net/fib_trie") {
            let mut current_local: Option<Ipv4Addr> = None;
            for line in entries.lines() {
                let trimmed = line.trim();
                // Lines like: "192.168.0.0/24" or "  |-- 192.168.0.100"
                if let Some(addr_str) = trimmed
                    .strip_prefix("|-- ")
                    .or_else(|| trimmed.strip_prefix("+-- "))
                    && let Ok(ip) = addr_str.trim().parse::<Ipv4Addr>()
                    && !ip.is_loopback()
                    && !ip.is_unspecified()
                {
                    current_local = Some(ip);
                }
                // LOCAL lines confirm this is a local address
                if trimmed == "LOCAL"
                    && let Some(local) = current_local.take()
                {
                    // Derive a /24 broadcast (most home networks use /24)
                    // A more accurate version would read the prefix length,
                    // but /24 covers the vast majority of home setups.
                    let octets = local.octets();
                    let broadcast = Ipv4Addr::new(octets[0], octets[1], octets[2], 255);
                    if !addrs.contains(&broadcast) {
                        addrs.push(broadcast);
                    }
                }
            }
        }

        // Fallback: also try common home network ranges directly
        // This is belt-and-suspenders in case /proc parsing fails
        if addrs.len() == 1 {
            // Try to determine our local IP by connecting a UDP socket (doesn't send anything)
            if let Ok(sock) = UdpSocket::bind("0.0.0.0:0")
                && sock.connect("8.8.8.8:80").is_ok()
                && let Ok(local) = sock.local_addr()
                && let IpAddr::V4(ip) = local.ip()
                && !ip.is_loopback()
            {
                let octets = ip.octets();
                let broadcast = Ipv4Addr::new(octets[0], octets[1], octets[2], 255);
                if !addrs.contains(&broadcast) {
                    addrs.push(broadcast);
                }
            }
        }

        addrs
    }
}
