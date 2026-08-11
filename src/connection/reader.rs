use std::io::ErrorKind;
use std::thread;

use crate::connection::ConnectionReader;
use crate::identity::PeerId;
use crate::protocol::{Decoder, LengthPrefixCodec, Message};

impl ConnectionReader {
    pub fn start_read_loop<F>(
        self,
        event_tx: std::sync::mpsc::Sender<crate::peer::PeerEvent>,
        peer_id: PeerId,
        on_disconnect: F,
    ) where
        F: FnOnce() + Send + 'static,
    {
        let peer_addr = self.remote_addr;

        thread::spawn(move || {
            let mut codec = LengthPrefixCodec;
            let mut buf_reader = std::io::BufReader::with_capacity(64 * 1024, self.stream);

            loop {
                match codec.decode(&mut buf_reader) {
                    Ok(Some(frame)) => match Message::try_from(frame) {
                        Ok(msg) => match msg {
                            Message::Hello { .. } | Message::HelloProof { .. } => {
                                eprintln!(
                                    "Protocol violation from {}: unexpected handshake message in Established phase, closing",
                                    peer_addr
                                );
                                break;
                            }
                            _ => {
                                let _ = event_tx
                                    .send(crate::peer::PeerEvent::Message(peer_id.clone(), msg));
                            }
                        },
                        Err(e) => {
                            eprintln!("Failed to parse message from {}: {}", peer_addr, e);
                            break;
                        }
                    },
                    Ok(None) => break,
                    // WouldBlock and TimedOut are treated as fatal connection errors.
                    // Because read_exact is not resumable, hitting a timeout mid-frame
                    // means the stream is permanently desynchronized. We intentionally break
                    // the loop to tear down the connection and drop the corrupted peer state.
                    Err(e)
                        if e.kind() == ErrorKind::WouldBlock || e.kind() == ErrorKind::TimedOut =>
                    {
                        eprintln!(
                            "Connection to {} stalled (timeout/wouldblock), closing to prevent desync",
                            peer_addr
                        );
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
