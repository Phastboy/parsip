use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4};
use std::sync::mpsc::Sender;
use std::thread;
use std::time::Duration;
use socket2::{Domain, Protocol, Socket, Type};
use std::io::Error;

use crate::identity::PeerId;
use crate::peer::PeerEvent;

const DISCOVERY_PORT: u16 = 9090;
const MAGIC: &[u8; 6] = b"PARSIP";

pub struct Discovery;

impl Discovery {
    pub fn start(tcp_listen_port: u16, my_id: PeerId, event_tx: Sender<PeerEvent>) -> Result<(), Error> {
        let broadcast_addr = SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::BROADCAST, DISCOVERY_PORT));
        let listen_addr = SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::UNSPECIFIED, DISCOVERY_PORT));

        // Broadcaster socket
        let broadcaster = Socket::new(Domain::IPV4, Type::DGRAM, Some(Protocol::UDP))?;
        broadcaster.set_broadcast(true)?;
        let broadcaster = std::net::UdpSocket::from(broadcaster);

        // Listener socket with reuse_port (Linux) and reuse_address
        let listener = Socket::new(Domain::IPV4, Type::DGRAM, Some(Protocol::UDP))?;
        listener.set_reuse_address(true)?;
        #[cfg(target_os = "linux")]
        listener.set_reuse_port(true)?;
        listener.bind(&listen_addr.into())?;
        let listener = std::net::UdpSocket::from(listener);

        // Build Payload
        let mut payload = Vec::with_capacity(40);
        payload.extend_from_slice(MAGIC);
        payload.extend_from_slice(&tcp_listen_port.to_be_bytes());
        payload.extend_from_slice(&my_id.to_bytes());

        // Broadcaster Thread
        thread::spawn(move || {
            loop {
                if let Err(e) = broadcaster.send_to(&payload, broadcast_addr) {
                    eprintln!("Discovery broadcast failed: {}", e);
                }
                thread::sleep(Duration::from_secs(3));
            }
        });

        // Listener Thread
        thread::spawn(move || {
            let mut buf = [0u8; 1024];
            loop {
                match listener.recv_from(&mut buf) {
                    Ok((amt, src)) => {
                        if amt == 40 && &buf[0..6] == MAGIC {
                            let mut port_bytes = [0u8; 2];
                            port_bytes.copy_from_slice(&buf[6..8]);
                            let remote_tcp_port = u16::from_be_bytes(port_bytes);

                            let mut peer_id_bytes = [0u8; 32];
                            peer_id_bytes.copy_from_slice(&buf[8..40]);
                            let remote_peer_id = PeerId::from_bytes(peer_id_bytes);

                            if remote_peer_id != my_id {
                                let mut target_addr = src;
                                target_addr.set_port(remote_tcp_port);
                                let _ = event_tx.send(PeerEvent::Discovered(remote_peer_id, target_addr));
                            }
                        }
                    }
                    Err(e) => {
                        eprintln!("Discovery receive error: {}", e);
                    }
                }
            }
        });

        Ok(())
    }
}
