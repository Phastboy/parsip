use std::io::{Error, ErrorKind};
use std::net::{SocketAddr, TcpListener, TcpStream};

use std::thread;

use crate::connection::{Connection, Direction};
use crate::identity::PeerId;
use crate::peer::{Peer, PeerEvent};

impl Peer {
    pub(crate) fn register_connection(
        &self,
        mut stream: TcpStream,
        remote_addr: SocketAddr,
        direction: Direction,
    ) -> Result<PeerId, Error> {
        let their_id =
            crate::connection::handshake::perform_handshake(&mut stream, &self.identity)?;

        if their_id == self.id {
            return Err(Error::new(
                ErrorKind::InvalidData,
                "Rejected self-connection",
            ));
        }

        let (connection, reader) =
            Connection::new(stream, remote_addr, their_id.clone(), direction)?;

        let conn_id = self.manager.insert_connection(connection, &self.id)?;

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
