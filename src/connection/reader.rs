use std::io::ErrorKind;
use std::thread;

use crate::identity::PeerId;
use crate::protocol::{Decoder, LengthPrefixCodec, Message};
use crate::connection::ConnectionReader;

impl ConnectionReader {
    pub fn start_read_loop<F>(
        self,
        event_tx: std::sync::mpsc::Sender<crate::peer::PeerEvent>,
        peer_id: PeerId,
        on_disconnect: F
    )
    where
        F: FnOnce() + Send + 'static,
    {
        let peer_addr = self.remote_addr;

        thread::spawn(move || {
            let mut codec = LengthPrefixCodec;
            let mut buf_reader = std::io::BufReader::with_capacity(64 * 1024, self.stream);

            loop {
                match codec.decode(&mut buf_reader) {
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
                    // WouldBlock: theoretically impossible on a blocking socket but guard it anyway.
                    Err(e) if e.kind() == ErrorKind::WouldBlock => continue,
                    // TimedOut: no data received within the read timeout window.
                    // Keepalive has already been probing the peer with no response by this point.
                    // Treat this as a dead connection — disconnect rather than looping forever.
                    Err(e) if e.kind() == ErrorKind::TimedOut => {
                        eprintln!("Connection to {} timed out (no data received), closing", peer_addr);
                        break;
                    }
                    Err(e) => {
                        eprintln!("Error reading frame from {}: {}", peer_addr, e);
                        break;
                    }
                }
            }

            let _ = event_tx.send(crate::peer::PeerEvent::Disconnected(peer_id));
            on_disconnect();
        });
    }
}
