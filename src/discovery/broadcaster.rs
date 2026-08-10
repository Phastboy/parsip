use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4};
use std::thread;
use std::time::Duration;
use socket2::{Domain, Protocol, Socket, Type};
use std::io::Error;

use crate::identity::PeerId;
use crate::discovery::{DISCOVERY_PORT, MAGIC};

pub fn spawn(tcp_listen_port: u16, my_id: &PeerId) -> Result<(), Error> {
    let broadcast_addr = SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::BROADCAST, DISCOVERY_PORT));

    let sock = Socket::new(Domain::IPV4, Type::DGRAM, Some(Protocol::UDP))?;
    sock.set_broadcast(true)?;
    let broadcaster = std::net::UdpSocket::from(sock);

    let mut payload = Vec::with_capacity(40);
    payload.extend_from_slice(MAGIC);
    payload.extend_from_slice(&tcp_listen_port.to_be_bytes());
    payload.extend_from_slice(&my_id.to_bytes());

    thread::spawn(move || loop {
        if let Err(e) = broadcaster.send_to(&payload, broadcast_addr) {
            eprintln!("Discovery broadcast failed: {}", e);
        }
        thread::sleep(Duration::from_secs(3));
    });

    Ok(())
}
