use std::net::{SocketAddr, TcpStream};
use crate::protocol::{Decoder, LengthPrefixCodec};

pub fn handle_connection(mut stream: TcpStream, peer_address: SocketAddr) {
    let mut codec = LengthPrefixCodec;

    loop {
        match codec.decode(&mut stream) {
            Ok(Some(frame)) => {
                let text = String::from_utf8_lossy(&frame.payload);
                println!("Received frame:");
                println!("    type: {:?}", frame.message_type);
                println!("    payload: {}", text);
            }
            Ok(None) => {
                println!("Client {} disconnected cleanly.", peer_address);
                break;
            }
            Err(e) => {
                eprintln!("Error reading frame from {}: {}", peer_address, e);
                break;
            }
        }
    }
}
