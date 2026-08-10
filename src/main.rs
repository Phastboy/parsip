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
mod discovery;

use peer::Peer;
use identity::Identity;
use discovery::Discovery;

fn main() -> Result<(), Error> {
    let args: Vec<String> = env::args().collect();

    let listen_port: u16 = args.get(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(9000);

    let address = SocketAddr::from((Ipv4Addr::UNSPECIFIED, listen_port));
    let identity = Identity::load_or_generate();
    let (peer, event_rx) = Peer::new(identity, address);
    let listener = peer.listen()?;

    println!("Peer listening on {}", peer.address());

    peer.start_accept_loop(listener);

    // Start Local Discovery
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
                            // Send a test ListResources frame to demonstrate bidirectional dispatch
                            let frame = protocol::Frame::new(protocol::MessageType::ListResources, vec![]);
                            if let Err(e) = peer_clone.send(&peer_id, &frame) {
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
        match event {
            peer::PeerEvent::Discovered(peer_id, addr) => {
                if !peer.is_connected(&peer_id) {
                    println!("[Discovery] Found peer {:?} at {}, connecting...", peer_id, addr);
                    let _ = peer.connect(addr);
                }
            }
            peer::PeerEvent::NewConnection(peer_id) => {
                println!("[Event] New connection established with {:?}", peer_id);
            }
            peer::PeerEvent::Disconnected(peer_id) => {
                println!("[Event] Peer {:?} disconnected", peer_id);
            }
            peer::PeerEvent::Message(peer_id, frame) => {
                let text = String::from_utf8_lossy(&frame.payload);
                match frame.message_type {
                    protocol::MessageType::ListResources => {
                        println!("[Protocol] Peer {:?} requested ListResources", peer_id);
                        // In the future, we will construct a ResourceInfo response and send it back
                    }
                    protocol::MessageType::GetResource => {
                        println!("[Protocol] Peer {:?} requested GetResource: {}", peer_id, text);
                    }
                    _ => {
                        println!("[Protocol] Received unhandled frame from {:?}: {:?}", peer_id, frame.message_type);
                        println!("Payload: {}", text);
                    }
                }
            }
        }
    }
    
    Ok(())
}
