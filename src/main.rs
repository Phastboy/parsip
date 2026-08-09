use std::io::Error;
use std::env;
use std::thread;
use std::net::TcpStream;

mod protocol;
mod listener;
mod transport;

use listener::{ParsipServer, Server};
use protocol::{Encoder, Frame, LengthPrefixCodec, MessageType};

fn main() -> Result<(), Error> {
    let args: Vec<String> = env::args().collect();
    let listen_port = args.get(1).map(|s| s.as_str()).unwrap_or("9000");
    let address = format!("127.0.0.1:{}", listen_port);

    // 1. Start Listener in a background thread
    let server = ParsipServer::new(address.clone());
    let listener = server.listen()?;

    println!("Peer listening on {}", server.address());

    thread::spawn(move || {
        loop {
            match server.accept(&listener) {
                Ok((stream, peer_address)) => {
                    println!("Accepted incoming connection from {}", peer_address);
                    transport::handle_connection(stream, peer_address);
                }
                Err(e) => {
                    eprintln!("Failed to accept connection: {}", e);
                }
            }
        }
    });

    // 2. Connect to remote peer if provided
    if let Some(target) = args.get(2) {
        println!("Initiating connection to remote peer: {}", target);
        let mut stream = TcpStream::connect(target)?;

        let mut codec = LengthPrefixCodec;
        let payload = format!("hello from peer on {}", listen_port).into_bytes();
        let frame = Frame::new(MessageType::Hello, payload);
        
        codec.encode(&frame, &mut stream)?;
        println!("Sent Hello frame to {}", target);

        // Handle incoming messages on this outgoing connection
        let peer_addr = stream.peer_addr()?;
        transport::handle_connection(stream, peer_addr);
    } else {
        // Block main thread forever if we are just listening
        loop {
            thread::park();
        }
    }

    Ok(())
}
