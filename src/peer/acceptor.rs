use std::io::{Error, ErrorKind};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::Arc;
use std::thread;

use crate::connection::{Connection, Direction};
use crate::identity::PeerId;
use crate::peer::{Peer, PeerEvent};

impl Peer {
    pub(crate) fn register_connection(
        &self,
        stream: TcpStream,
        remote_addr: SocketAddr,
        direction: Direction,
    ) -> Result<PeerId, Error> {
        let (connection, reader) = Connection::new(stream, remote_addr)?;
        let connection_arc = Arc::new(std::sync::Mutex::new(connection));

        let conn_id = self
            .manager
            .insert_pending(connection_arc.clone(), direction)?;

        let handshake_result = {
            let mut conn = connection_arc.lock().unwrap();
            conn.handshake(&self.identity)
        };

        let their_id = match handshake_result {
            Ok(id) => id,
            Err(e) => {
                self.manager.remove(conn_id);
                return Err(e);
            }
        };

        if their_id == self.id {
            self.manager.remove(conn_id);
            return Err(Error::new(
                ErrorKind::InvalidData,
                "Rejected self-connection",
            ));
        }

        if let Err(e) = self
            .manager
            .promote_to_established(conn_id, &self.id, their_id.clone())
        {
            // manager already dropped our connection if it threw an error
            return Err(e);
        }

        // If we reach here, we survived deduplication and the connection is officially registered.
        let manager_for_cleanup = self.manager.clone();

        let tx_clone = self.event_tx.clone();
        let peer_id_for_loop = their_id.clone();

        reader.start_read_loop(tx_clone, peer_id_for_loop, move || {
            manager_for_cleanup.remove(conn_id);
        });

        let _ = self
            .event_tx
            .send(PeerEvent::NewConnection(their_id.clone()));

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
                            if let Err(e) = peer_clone2.register_connection(
                                stream,
                                peer_address,
                                Direction::Incoming,
                            ) {
                                eprintln!(
                                    "Failed to register incoming connection from {}: {}",
                                    peer_address, e
                                );
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
