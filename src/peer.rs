use std::io::{Error, ErrorKind};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::thread;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver};

use crate::connection::Connection;
use crate::connection_manager::ConnectionManager;
use crate::identity::PeerId;

#[derive(Clone)]
pub struct Peer {
    pub id: PeerId,
    address: SocketAddr,
    manager: ConnectionManager,
}

impl Peer {
    pub fn new(id: PeerId, address: SocketAddr) -> Self {
        Self {
            id,
            address,
            manager: ConnectionManager::new(),
        }
    }

    pub fn address(&self) -> SocketAddr {
        self.address
    }

    pub fn listen(&self) -> Result<TcpListener, Error> {
        TcpListener::bind(self.address)
    }

    /// Shared handshake + registration path for both inbound and outbound connections.
    /// Peer owns identity/orchestration; ConnectionManager owns the registry itself.
    fn register_connection(&self, stream: TcpStream, remote_addr: SocketAddr) -> Result<PeerId, Error> {
        let mut connection = Connection::new(stream, remote_addr);

        let their_id = connection.handshake(self.id.clone())?;

        if their_id == self.id {
            return Err(Error::new(ErrorKind::InvalidData, "Rejected self-connection"));
        }

        println!("Handshake successful! Remote peer is {:?}", their_id);

        let conn_id = self.manager.reserve_id();
        let died_before_insert = Arc::new(AtomicBool::new(false));

        let manager_for_cleanup = self.manager.clone();
        let cleanup_id = their_id.clone();
        let died_flag = died_before_insert.clone();

        connection.start_read_loop(move || {
            died_flag.store(true, Ordering::SeqCst);
            manager_for_cleanup.remove_if_current(&cleanup_id, conn_id);
        })?;

        self.manager.insert_if_alive(their_id.clone(), conn_id, connection, &died_before_insert)?;

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

            match peer_clone.register_connection(stream, target) {
                Ok(peer_id) => {
                    let _ = tx.send(Ok(peer_id));
                }
                Err(e) => {
                    eprintln!("Failed to register outgoing connection to {}: {}", target, e);
                    let _ = tx.send(Err(e));
                }
            }
        });

        rx
    }
}
