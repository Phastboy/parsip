use std::net::{TcpListener, TcpStream, SocketAddr};
use std::thread;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::io::{Error, ErrorKind};

use crate::connection::Connection;
use crate::identity::PeerId;
use crate::peer::{Peer, PeerEvent};

impl Peer {
    pub(crate) fn register_connection(&self, stream: TcpStream, remote_addr: SocketAddr) -> Result<PeerId, Error> {
        let mut connection = Connection::new(stream, remote_addr);

        let their_id = connection.handshake(&self.identity)?;

        if their_id == self.id {
            return Err(Error::new(ErrorKind::InvalidData, "Rejected self-connection"));
        }

        let conn_id = self.manager.reserve_id();
        let died_before_insert = Arc::new(AtomicBool::new(false));

        let manager_for_cleanup = self.manager.clone();
        let cleanup_id = their_id.clone();
        let died_flag = died_before_insert.clone();
        
        let tx_clone = self.event_tx.clone();
        let peer_id_for_loop = their_id.clone();

        connection.start_read_loop(tx_clone, peer_id_for_loop, move || {
            died_flag.store(true, Ordering::SeqCst);
            manager_for_cleanup.remove_if_current(&cleanup_id, conn_id);
        })?;

        self.manager.insert_if_alive(their_id.clone(), conn_id, connection, &died_before_insert)?;

        let _ = self.event_tx.send(PeerEvent::NewConnection(their_id.clone()));

        Ok(their_id)
    }

    pub fn start_accept_loop(&self, listener: TcpListener) {
        let peer_clone = self.clone();
        thread::spawn(move || {
            loop {
                match listener.accept() {
                    Ok((stream, peer_address)) => {
                        println!("Accepted incoming connection from {}", peer_address);
                        let peer_clone2 = peer_clone.clone();

                        thread::spawn(move || {
                            if let Err(e) = peer_clone2.register_connection(stream, peer_address) {
                                eprintln!("Failed to register incoming connection from {}: {}", peer_address, e);
                            }
                        });
                    }
                    Err(e) => {
                        eprintln!("Failed to accept connection: {}", e);
                    }
                }
            }
        });
    }
}
