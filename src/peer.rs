use std::io::{Error, ErrorKind};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::thread;
use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver};
use std::collections::HashMap;

use crate::connection::Connection;
use crate::identity::PeerId;

#[derive(Clone)]
pub struct Peer {
    pub id: PeerId,
    address: SocketAddr,
    connections: Arc<Mutex<HashMap<PeerId, (u64, Connection)>>>,
    next_conn_id: Arc<AtomicU64>,
}

impl Peer {
    pub fn new(id: PeerId, address: SocketAddr) -> Self {
        Self {
            id,
            address,
            connections: Arc::new(Mutex::new(HashMap::new())),
            next_conn_id: Arc::new(AtomicU64::new(1)),
        }
    }

    pub fn address(&self) -> SocketAddr {
        self.address
    }

    pub fn listen(&self) -> Result<TcpListener, Error> {
        TcpListener::bind(self.address)
    }

    /// Shared handshake + registration path for both inbound and outbound connections.
    fn register_connection(&self, stream: TcpStream, remote_addr: SocketAddr) -> Result<PeerId, Error> {
        let mut connection = Connection::new(stream, remote_addr);

        let their_id = connection.handshake(self.id.clone())?;

        if their_id == self.id {
            return Err(Error::new(ErrorKind::InvalidData, "Rejected self-connection"));
        }

        println!("Handshake successful! Remote peer is {:?}", their_id);

        let conn_id = self.next_conn_id.fetch_add(1, Ordering::SeqCst);
        let died_before_insert = Arc::new(AtomicBool::new(false));

        let connections = self.connections.clone();
        let cleanup_id = their_id.clone();
        let died_flag = died_before_insert.clone();

        connection.start_read_loop(move || {
            // Mark first, in case this fires before registration below completes.
            died_flag.store(true, Ordering::SeqCst);
            if let Ok(mut conns) = connections.lock() {
                // Only remove if the entry currently registered is THIS connection —
                // otherwise a newer connection for the same peer would be wiped out.
                let should_remove = matches!(conns.get(&cleanup_id), Some((id, _)) if *id == conn_id);
                if should_remove {
                    conns.remove(&cleanup_id);
                    println!("Removed dead connection for {:?}", cleanup_id);
                }
            }
        })?;

        let mut conns = self.connections.lock()
            .map_err(|_| Error::new(ErrorKind::Other, "connections lock poisoned"))?;

        if died_before_insert.load(Ordering::SeqCst) {
            // Connection died between starting the read loop and reaching here —
            // don't insert an already-dead connection that nothing will ever clean up.
            drop(conns);
            return Err(Error::new(ErrorKind::ConnectionAborted, "Connection died before registration completed"));
        }

        if conns.contains_key(&their_id) {
            println!("Replacing existing connection for {:?}", their_id);
        }
        conns.insert(their_id.clone(), (conn_id, connection));

        Ok(their_id)
    }

    /// Accepts inbound connections on a background thread. Each accepted
    /// connection is handed off to its own thread so a slow/silent peer
    /// can't stall accepting further connections.
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

    /// Non-blocking: dials and registers the connection on a background thread.
    /// Returns a Receiver the caller can optionally check for the outcome —
    /// Ok(PeerId) on success, Err(Error) on dial or handshake failure.
    /// Dropping the receiver without reading it is fine; the send simply no-ops.
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
