use std::io::Error;
use std::net::{SocketAddr, TcpStream};
use std::sync::mpsc::{self, Receiver};
use std::thread;

use crate::connection::Direction;
use crate::identity::PeerId;
use crate::peer::Peer;

impl Peer {
    pub fn connect(&self, target: SocketAddr) -> Receiver<Result<PeerId, Error>> {
        let (tx, rx) = mpsc::channel();
        let peer_clone = self.clone();

        thread::spawn(move || {
            println!("Initiating connection to remote peer: {}", target);

            let stream = match TcpStream::connect(target) {
                Ok(s) => s,
                Err(e) => {
                    eprintln!("Failed to connect to {}: {}", target, e);
                    let _ = tx.send(Err(e));
                    return;
                }
            };

            match peer_clone.register_connection(stream, target, Direction::Outgoing) {
                Ok(peer_id) => {
                    let _ = tx.send(Ok(peer_id));
                }
                Err(e) => {
                    eprintln!(
                        "Failed to register outgoing connection to {}: {}",
                        target, e
                    );
                    let _ = tx.send(Err(e));
                }
            }
        });

        rx
    }
}
