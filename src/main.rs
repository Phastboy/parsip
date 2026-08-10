use std::io::Error;
use std::env;
use std::thread;
use std::net::{Ipv4Addr, SocketAddr};
use std::fs;

mod protocol;
mod peer;
mod connection;
mod connection_manager;
mod identity;
mod random;
mod discovery;
mod fs_dir; // Note: 'fs' is a std mod, so I named the module fs_dir
mod event_handler;

use peer::Peer;
use identity::Identity;
use discovery::Discovery;
use protocol::Message;
use fs_dir::{shared_dir, downloads_dir};
use event_handler::handle_event;

fn main() -> Result<(), Error> {
    let args: Vec<String> = env::args().collect();

    let listen_port: u16 = args.get(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(9000);

    let address = SocketAddr::from((Ipv4Addr::UNSPECIFIED, listen_port));
    let identity = Identity::load_or_generate();
    let (peer, event_rx) = Peer::new(identity, address);
    let listener = peer.listen()?;

    let _ = fs::create_dir_all(shared_dir());
    let _ = fs::create_dir_all(downloads_dir());

    println!("Peer listening on {}", peer.address());

    peer.start_accept_loop(listener);

    if let Err(e) = Discovery::start(listen_port, peer.id.clone(), peer.event_tx.clone()) {
        eprintln!("Warning: Failed to start local discovery: {}", e);
    }

    if let Some(target_str) = args.get(2) {
        match target_str.parse::<SocketAddr>() {
            Ok(target) => {
                let rx = peer.connect(target);
                let peer_clone = peer.clone();
                thread::spawn(move || {
                    match rx.recv() {
                        Ok(Ok(peer_id)) => {
                            println!("Connected to {:?}", peer_id);
                            let msg = Message::ListResources;
                            if let Err(e) = peer_clone.send(&peer_id, &msg) {
                                eprintln!("Failed to send ListResources to {:?}: {}", peer_id, e);
                            }
                        }
                        Ok(Err(e)) => eprintln!("Connection to {} failed: {}", target, e),
                        Err(_) => eprintln!("Connection attempt to {} was dropped unexpectedly", target),
                    }
                });
            }
            Err(_) => eprintln!("Invalid target address format. Expected IP:PORT"),
        }
    }

    println!("Starting event loop...");
    for event in event_rx.iter() {
        handle_event(&peer, event);
    }
    
    Ok(())
}
