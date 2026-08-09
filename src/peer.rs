use std::io::Error;
use std::net::{SocketAddr, TcpListener, TcpStream};

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

    pub fn accept(
        &self,
        listener: &TcpListener,
    ) -> Result<(TcpStream, SocketAddr), Error> {
        listener.accept()
    }

    pub fn connect(&self, address: SocketAddr) -> Result<TcpStream, Error> {
        TcpStream::connect(address)
    }
}
