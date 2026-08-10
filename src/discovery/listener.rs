use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4};
use std::sync::mpsc::Sender;
use std::thread;
use socket2::{Domain, Protocol, Socket, Type};
use std::io::Error;

use crate::identity::PeerId;
use crate::peer::PeerEvent;
use crate::discovery::{DISCOVERY_PORT, MAGIC};

pub fn spawn(my_id: PeerId, event_tx: Sender<PeerEvent>) -> Result<(), Error> {
    let listen_addr = SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::UNSPECIFIED, DISCOVERY_PORT));

    let sock = Socket::new(Domain::IPV4, Type::DGRAM, Some(Protocol::UDP))?;
    sock.set_reuse_address(true)?;
    #[cfg(target_os = "linux")]
    sock.set_reuse_port(true)?;
    sock.bind(&listen_addr.into())?;
    let listener = std::net::UdpSocket::from(sock);

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
                Err(e) => eprintln!("Discovery receive error: {}", e),
            }
        }
    });

    Ok(())
}
