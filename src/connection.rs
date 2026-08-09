use std::io::Error;
use std::net::{SocketAddr, TcpStream};
use std::thread;

use crate::protocol::{Decoder, Encoder, Frame, LengthPrefixCodec, MessageType};

pub struct Connection {
    remote_addr: SocketAddr,
    stream: TcpStream,
}

impl Connection {
    pub fn new(stream: TcpStream, remote_addr: SocketAddr) -> Self {
        Self { remote_addr, stream }
    }

    pub fn send_hello(&mut self, payload: Vec<u8>) -> Result<(), Error> {
        let mut codec = LengthPrefixCodec;
        let frame = Frame::new(MessageType::Hello, payload);
        codec.encode(&frame, &mut self.stream)?;
        Ok(())
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
