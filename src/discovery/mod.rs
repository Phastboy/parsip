use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4};
use std::sync::mpsc::Sender;
use std::thread;
use socket2::{Domain, Protocol, Socket, Type};
use std::io::Error;

use crate::identity::PeerId;
use crate::peer::PeerEvent;

pub const DISCOVERY_PORT: u16 = 9090;
pub const MAGIC_REQUEST: &[u8; 6] = b"PARREQ";
pub const MAGIC_RESPONSE: &[u8; 6] = b"PARRES";

pub struct Discovery;

impl Discovery {
    pub fn start(tcp_listen_port: u16, my_id: PeerId, my_nickname: String, event_tx: Sender<PeerEvent>) -> Result<(), Error> {
        let listen_addr = SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::UNSPECIFIED, DISCOVERY_PORT));

        let sock = Socket::new(Domain::IPV4, Type::DGRAM, Some(Protocol::UDP))?;
        sock.set_reuse_address(true)?;
        #[cfg(target_os = "linux")]
        sock.set_reuse_port(true)?;
        sock.bind(&listen_addr.into())?;
        sock.set_broadcast(true)?;
        let socket = std::net::UdpSocket::from(sock);

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
                                        // Reply with our info
                                        let mut payload = Vec::with_capacity(40 + my_nickname.len());
                                        payload.extend_from_slice(MAGIC_RESPONSE);
                                        payload.extend_from_slice(&tcp_listen_port.to_be_bytes());
                                        payload.extend_from_slice(&my_id.to_bytes());
                                        payload.extend_from_slice(my_nickname.as_bytes());
                                        let _ = socket_clone.send_to(&payload, src);
                                    }

                                    let mut target_addr = src;
                                    target_addr.set_port(remote_tcp_port);
                                    let _ = event_tx.send(PeerEvent::Discovered(remote_peer_id, target_addr, nickname));
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

    pub fn broadcast_scan(tcp_listen_port: u16, my_id: &PeerId, my_nickname: &str) -> Result<(), Error> {
        let broadcast_addr = SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::BROADCAST, DISCOVERY_PORT));

        let sock = Socket::new(Domain::IPV4, Type::DGRAM, Some(Protocol::UDP))?;
        sock.set_broadcast(true)?;
        let broadcaster = std::net::UdpSocket::from(sock);

        let mut payload = Vec::with_capacity(40 + my_nickname.len());
        payload.extend_from_slice(MAGIC_REQUEST);
        payload.extend_from_slice(&tcp_listen_port.to_be_bytes());
        payload.extend_from_slice(&my_id.to_bytes());
        payload.extend_from_slice(my_nickname.as_bytes());

        broadcaster.send_to(&payload, broadcast_addr)?;
        Ok(())
    }
}
