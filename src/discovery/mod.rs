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
    /// plus a directed subnet broadcast for every local IPv4 interface, derived from
    /// the actual subnet mask in /proc/net/route.
    fn collect_broadcast_addrs() -> Vec<Ipv4Addr> {
        let mut addrs = vec![Ipv4Addr::BROADCAST]; // 255.255.255.255

        // /proc/net/route columns (whitespace-separated, all numbers are little-endian hex):
        //   Iface Destination Gateway Flags RefCnt Use Metric Mask MTU Window IRTT
        // We need columns 1 (Destination) and 7 (Mask).
        // RTF_UP flag (0x1) filters out disabled routes; dest == 0 is the default gateway.
        if let Ok(content) = std::fs::read_to_string("/proc/net/route") {
            for line in content.lines().skip(1) {
                // skip header
                let fields: Vec<&str> = line.split_ascii_whitespace().collect();
                if fields.len() < 8 {
                    continue;
                }
                let Ok(dest_le) = u32::from_str_radix(fields[1], 16) else {
                    continue;
                };
                let Ok(mask_le) = u32::from_str_radix(fields[7], 16) else {
                    continue;
                };
                let Ok(flags) = u32::from_str_radix(fields[3], 16) else {
                    continue;
                };

                // Skip down routes and the default gateway (dest == 0)
                if flags & 0x1 == 0 || dest_le == 0 {
                    continue;
                }

                // Values are little-endian; swap to get the canonical u32 for Ipv4Addr
                let dest_be = dest_le.swap_bytes();
                let mask_be = mask_le.swap_bytes();

                let dest_ip = Ipv4Addr::from(dest_be);
                if dest_ip.is_loopback() {
                    continue;
                }

                // Broadcast = network_addr | ~mask  (both in big-endian u32)
                let broadcast = Ipv4Addr::from(dest_be | !mask_be);
                if !addrs.contains(&broadcast) {
                    addrs.push(broadcast);
                }
            }
        }

        // Fallback: use a dummy UDP connect to discover the local IP and derive /24.
        // Only used when /proc/net/route parsing fails or is unavailable.
        if addrs.len() == 1
            && let Ok(sock) = UdpSocket::bind("0.0.0.0:0")
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

        addrs
    }
}
