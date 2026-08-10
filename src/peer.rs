use std::io::{Error, ErrorKind};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::thread;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};

use crate::connection::Connection;
use crate::connection_manager::ConnectionManager;
use crate::identity::{Identity, PeerId};
use crate::protocol::Frame;

pub enum PeerEvent {
    NewConnection(PeerId),
    Disconnected(PeerId),
    Message(PeerId, Frame),
    Discovered(PeerId, SocketAddr),
}

#[derive(Clone)]
pub struct Peer {
    identity: Arc<Identity>,
    pub id: PeerId,
    address: SocketAddr,
    manager: ConnectionManager,
    pub event_tx: Sender<PeerEvent>,
}

impl Peer {
    pub fn new(identity: Identity, address: SocketAddr) -> (Self, Receiver<PeerEvent>) {
        let id = identity.peer_id.clone();
        let (tx, rx) = mpsc::channel();
        let peer = Self {
            identity: Arc::new(identity),
            id,
            address,
            manager: ConnectionManager::new(),
            event_tx: tx,
        };
        (peer, rx)
    }

    pub fn address(&self) -> SocketAddr {
        self.address
    }

    pub fn is_connected(&self, peer_id: &PeerId) -> bool {
        self.manager.is_connected(peer_id)
    }

    pub fn send(&self, target: &PeerId, frame: &Frame) -> Result<(), Error> {
        self.manager.send(target, frame)
    }

    pub fn listen(&self) -> Result<TcpListener, Error> {
        TcpListener::bind(self.address)
    }

    fn register_connection(&self, stream: TcpStream, remote_addr: SocketAddr) -> Result<PeerId, Error> {
        let mut connection = Connection::new(stream, remote_addr);

        let their_id = connection.handshake(&self.identity)?;

        if their_id == self.id {
            return Err(Error::new(ErrorKind::InvalidData, "Rejected self-connection"));
        }

        let _ = self.event_tx.send(PeerEvent::NewConnection(their_id.clone()));

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
