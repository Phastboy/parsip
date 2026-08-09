use std::io::Error;
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::thread;
use std::sync::{Arc, Mutex};
use std::collections::HashMap;

use crate::connection::Connection;
use crate::identity::PeerId;

#[derive(Clone)]
pub struct Peer {
    pub id: PeerId,
    address: SocketAddr,
    connections: Arc<Mutex<HashMap<PeerId, Connection>>>,
}

impl Peer {
    pub fn new(id: PeerId, address: SocketAddr) -> Self {
        Self {
            id,
            address,
            connections: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn address(&self) -> SocketAddr {
        self.address
    }

    pub fn listen(&self) -> Result<TcpListener, Error> {
        TcpListener::bind(self.address)
    }

    pub fn start_accept_loop(&self, listener: TcpListener) {
        let peer_clone = self.clone();
        thread::spawn(move || {
            loop {
                match listener.accept() {
                    Ok((stream, peer_address)) => {
                        println!("Accepted incoming connection from {}", peer_address);
                        
                        let mut connection = Connection::new(stream, peer_address);
                        
                        // 1. Synchronous Handshake
                        let their_id = match connection.handshake(peer_clone.id.clone()) {
                            Ok(id) => id,
                            Err(e) => {
                                eprintln!("Handshake failed with {}: {}", peer_address, e);
                                continue;
                            }
                        };
                        
                        println!("Handshake successful! Remote peer is {:?}", their_id);

                        // 2. Start background reader
                        if let Err(e) = connection.start_read_loop() {
                            eprintln!("Failed to start read loop for {:?}: {}", their_id, e);
                            continue;
                        }

                        // 3. Register the connection by PeerId
                        if let Ok(mut conns) = peer_clone.connections.lock() {
                            conns.insert(their_id, connection);
                        }
                    }
                    Err(e) => {
                        eprintln!("Failed to accept connection: {}", e);
                    }
                }
            }
        });
    }

    pub fn connect(&self, target: SocketAddr) -> Result<(), Error> {
        println!("Initiating connection to remote peer: {}", target);
        let stream = TcpStream::connect(target)?;

        let mut connection = Connection::new(stream, target);

        // 1. Synchronous Handshake
        let their_id = connection.handshake(self.id.clone())?;
        println!("Handshake successful! Remote peer is {:?}", their_id);

        // 2. Start background reader
        connection.start_read_loop()?;

        // 3. Register the connection by PeerId
        if let Ok(mut conns) = self.connections.lock() {
            conns.insert(their_id, connection);
        }

        Ok(())
    }
}
