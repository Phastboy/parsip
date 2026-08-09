use std::io::Error;
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::thread;

use crate::connection::Connection;

#[derive(Debug, Clone)]
pub struct Peer {
    address: SocketAddr,
}

impl Peer {
    pub fn new(address: SocketAddr) -> Self {
        Self { address }
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
                        
                        // Start reading on a background thread
                        if let Err(e) = connection.start_read_loop() {
                            eprintln!("Failed to start read loop: {}", e);
                            continue;
                        }

                        // Immediately write a HELLO frame
                        let local_port = peer_clone.address().port();
                        let payload = format!("hello from acceptor on {}", local_port).into_bytes();
                        if let Err(e) = connection.send_hello(payload) {
                            eprintln!("Failed to send hello to peer: {}", e);
                        } else {
                            println!("Sent Hello frame to {}", peer_address);
                        }
                    }
                    Err(e) => {
                        eprintln!("Failed to accept connection: {}", e);
                    }
                }
            }
        });
    }

    pub fn initiate_connection(&self, target: SocketAddr) -> Result<(), Error> {
        println!("Initiating connection to remote peer: {}", target);
        let stream = TcpStream::connect(target)?;

        let mut connection = Connection::new(stream, target);

        // Start reading on a background thread
        connection.start_read_loop()?;

        // Immediately write a HELLO frame
        let local_port = self.address().port();
        let payload = format!("hello from initiator on {}", local_port).into_bytes();
        connection.send_hello(payload)?;
        println!("Sent Hello frame to {}", target);

        Ok(())
    }
}
