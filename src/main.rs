use std::io::Error;
use std::env;
use std::thread;
use std::net::{Ipv4Addr, SocketAddr};

mod protocol;
mod peer;
mod connection;
mod identity;

use peer::Peer;
use identity::PeerId;

fn main() -> Result<(), Error> {
    let args: Vec<String> = env::args().collect();
    
    // 1. Parse listen port
    let listen_port: u16 = args.get(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(9000);
        
    // 2. Setup Peer
    let address = SocketAddr::from((Ipv4Addr::UNSPECIFIED, listen_port));
    let peer_id = PeerId::load_or_generate(listen_port);
    let peer = Peer::new(peer_id, address);
    let listener = peer.listen()?;

    println!("Peer listening on {}", peer.address());

    // 3. Start background acceptor
    peer.start_accept_loop(listener);

    // 4. Connect to remote peer if provided
    if let Some(target_str) = args.get(2) {
        if let Ok(target) = target_str.parse::<SocketAddr>() {
            peer.connect(target)?;
        } else {
            eprintln!("Invalid target address format. Expected IP:PORT");
        }
    }

    // 5. Keep process alive
    loop {
        thread::park();
    }
}
