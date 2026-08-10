use std::io::Error;
use std::env;
use std::thread;
use std::net::{Ipv4Addr, SocketAddr};

mod protocol;
mod peer;
mod connection;
mod connection_manager;
mod identity;
mod random;

use peer::Peer;
use identity::Identity;

fn main() -> Result<(), Error> {
    let args: Vec<String> = env::args().collect();

    let listen_port: u16 = args.get(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(9000);

    let address = SocketAddr::from((Ipv4Addr::UNSPECIFIED, listen_port));
    let identity = Identity::load_or_generate();
    let peer = Peer::new(identity, address);
    let listener = peer.listen()?;

    println!("Peer listening on {}", peer.address());

    peer.start_accept_loop(listener);

    if let Some(target_str) = args.get(2) {
        match target_str.parse::<SocketAddr>() {
            Ok(target) => {
                let rx = peer.connect(target);
                thread::spawn(move || {
                    match rx.recv() {
                        Ok(Ok(peer_id)) => println!("Connected to {:?}", peer_id),
                        Ok(Err(e)) => eprintln!("Connection to {} failed: {}", target, e),
                        Err(_) => eprintln!("Connection attempt to {} was dropped unexpectedly", target),
                    }
                });
            }
            Err(_) => eprintln!("Invalid target address format. Expected IP:PORT"),
        }
    }

    loop {
        thread::park();
    }
}
