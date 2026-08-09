use std::io::Error;
use std::env;
use std::thread;
use std::net::{Ipv4Addr, SocketAddr};

mod protocol;
mod peer;
mod transport;

use peer::Peer;
use protocol::{Encoder, Frame, LengthPrefixCodec, MessageType};

fn main() -> Result<(), Error> {
    let args: Vec<String> = env::args().collect();
    
    // 1. Setup Peer
    let listen_port: u16 = args.get(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(9000);
        
    let address = SocketAddr::from((Ipv4Addr::UNSPECIFIED, listen_port));
    let peer = Peer::new(address);
    let listener = peer.listen()?;

    println!("Peer listening on {}", peer.address());

    // 2. Accept Incoming Connections (Background Thread)
    let peer_clone = peer.clone();
    thread::spawn(move || {
        loop {
            match peer_clone.accept(&listener) {
                Ok((mut stream, peer_address)) => {
                    println!("Accepted incoming connection from {}", peer_address);
                    
                    // Split stream: one handle for reading, one for writing
                    let mut read_stream = stream.try_clone().expect("Failed to clone stream");
                    
                    // Spawn a thread to handle reading
                    thread::spawn(move || {
                        transport::handle_connection(read_stream, peer_address);
                    });
                    
                    // Immediately write a HELLO frame back to prove bidirectional equality
                    let mut codec = LengthPrefixCodec;
                    let payload = format!("hello from acceptor on {}", listen_port).into_bytes();
                    let frame = Frame::new(MessageType::Hello, payload);
                    if let Err(e) = codec.encode(&frame, &mut stream) {
                        eprintln!("Failed to write to peer: {}", e);
                    } else {
                        println!("Sent Hello frame to {}", peer_address);
                    }
                }
                Err(e) => {
                    eprintln!("Failed to accept connection: {}", e);
                }
            }
        }
    });

    // 3. Initiate Outgoing Connection (Main Thread)
    if let Some(target_str) = args.get(2) {
        if let Ok(target) = target_str.parse::<SocketAddr>() {
            println!("Initiating connection to remote peer: {}", target);
            let mut stream = peer.connect(target)?;

            // Split stream: one handle for reading, one for writing
            let mut read_stream = stream.try_clone().expect("Failed to clone stream");
            
            // Spawn a thread to handle reading
            thread::spawn(move || {
                transport::handle_connection(read_stream, target);
            });

            // Immediately write a HELLO frame to prove bidirectional equality
            let mut codec = LengthPrefixCodec;
            let payload = format!("hello from initiator on {}", listen_port).into_bytes();
            let frame = Frame::new(MessageType::Hello, payload);
            
            codec.encode(&frame, &mut stream)?;
            println!("Sent Hello frame to {}", target);
        } else {
            eprintln!("Invalid target address format. Expected IP:PORT");
        }
    }

    // 4. Keep process alive
    loop {
        thread::park();
    }
}
