use std::io::{Error, ErrorKind};
use std::net::{SocketAddr, TcpStream};
use std::thread;
use std::time::Duration;

use crate::protocol::{Decoder, Encoder, Frame, LengthPrefixCodec, MessageType};
use crate::identity::PeerId;

pub struct Connection {
    remote_addr: SocketAddr,
    pub remote_peer_id: Option<PeerId>,
    stream: TcpStream,
}

impl Connection {
    pub fn new(stream: TcpStream, remote_addr: SocketAddr) -> Self {
        let _ = stream.set_read_timeout(Some(Duration::from_secs(30)));
        let _ = stream.set_write_timeout(Some(Duration::from_secs(30)));
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
                    Err(Error::new(ErrorKind::InvalidData, "Invalid handshake frame"))
                }
            }
            None => Err(Error::new(ErrorKind::ConnectionAborted, "Peer disconnected during handshake"))
        }
    }

    /// Starts the background read loop. `on_disconnect` is called exactly once,
    /// when the loop exits (clean disconnect or read error), so the caller can
    /// remove this connection from any shared registry.
    pub fn start_read_loop<F>(&self, on_disconnect: F) -> Result<(), Error>
    where
        F: FnOnce() + Send + 'static,
    {
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
                    Err(e) if e.kind() == ErrorKind::WouldBlock || e.kind() == ErrorKind::TimedOut => {
                        // No data within the read timeout window — not a real error, keep polling.
                        continue;
                    }
                    Err(e) => {
                        eprintln!("Error reading frame from {}: {}", peer_addr, e);
                        break;
                    }
                }
            }

            on_disconnect();
        });

        Ok(())
    }
}
