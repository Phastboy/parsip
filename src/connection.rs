use std::io::Error;
use std::net::{SocketAddr, TcpStream};
use std::thread;

use crate::protocol::{Decoder, Encoder, Frame, LengthPrefixCodec, MessageType};
use crate::identity::PeerId;

pub struct Connection {
    remote_addr: SocketAddr,
    pub remote_peer_id: Option<PeerId>,
    stream: TcpStream,
}

impl Connection {
    pub fn new(stream: TcpStream, remote_addr: SocketAddr) -> Self {
        Self { remote_addr, remote_peer_id: None, stream }
    }

    pub fn handshake(&mut self, my_id: PeerId) -> Result<PeerId, Error> {
        let mut codec = LengthPrefixCodec;
        
        // 1. Send my ID
        let payload = my_id.to_bytes().to_vec();
        let frame = Frame::new(MessageType::Hello, payload);
        codec.encode(&frame, &mut self.stream)?;
        
        // 2. Read their ID
        match codec.decode(&mut self.stream)? {
            Some(frame) => {
                if frame.message_type == MessageType::Hello && frame.payload.len() == 32 {
                    let mut bytes = [0u8; 32];
                    bytes.copy_from_slice(&frame.payload);
                    let their_id = PeerId::from_bytes(bytes);
                    self.remote_peer_id = Some(their_id.clone());
                    Ok(their_id)
                } else {
                    Err(Error::new(std::io::ErrorKind::InvalidData, "Invalid handshake frame"))
                }
            }
            None => Err(Error::new(std::io::ErrorKind::ConnectionAborted, "Peer disconnected during handshake"))
        }
    }

    pub fn start_read_loop(&self) -> Result<(), Error> {
        let mut read_stream = self.stream.try_clone()?;
        let peer_addr = self.remote_addr;

        thread::spawn(move || {
            let mut codec = LengthPrefixCodec;

            loop {
                match codec.decode(&mut read_stream) {
                    Ok(Some(frame)) => {
                        let text = String::from_utf8_lossy(&frame.payload);
                        println!("Received frame:");
                        println!("    type: {:?}", frame.message_type);
                        println!("    payload: {}", text);
                    }
                    Ok(None) => {
                        println!("Peer {} disconnected cleanly.", peer_addr);
                        break;
                    }
                    Err(e) => {
                        eprintln!("Error reading frame from {}: {}", peer_addr, e);
                        break;
                    }
                }
            }
        });

        Ok(())
    }
}
