use std::io::{Error, ErrorKind};
use std::thread;

use crate::identity::PeerId;
use crate::protocol::{Decoder, LengthPrefixCodec, Message};
use crate::connection::Connection;

impl Connection {
    pub fn start_read_loop<F>(&self, event_tx: std::sync::mpsc::Sender<crate::peer::PeerEvent>, peer_id: PeerId, on_disconnect: F) -> Result<(), Error>
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
                        match Message::try_from(frame) {
                            Ok(msg) => match msg {
                                Message::Hello { .. } | Message::HelloProof { .. } => {
                                    eprintln!("Protocol violation from {}: unexpected handshake message in Established phase, closing", peer_addr);
                                    break;
                                }
                                _ => {
                                    let _ = event_tx.send(crate::peer::PeerEvent::Message(peer_id.clone(), msg));
                                }
                            },
                            Err(e) => {
                                eprintln!("Failed to parse message from {}: {}", peer_addr, e);
                            }
                        }
                    }
                    Ok(None) => break,
                    Err(e) if e.kind() == ErrorKind::WouldBlock || e.kind() == ErrorKind::TimedOut => continue,
                    Err(e) => {
                        eprintln!("Error reading frame from {}: {}", peer_addr, e);
                        break;
                    }
                }
            }

            let _ = event_tx.send(crate::peer::PeerEvent::Disconnected(peer_id));
            on_disconnect();
        });

        Ok(())
    }
}
