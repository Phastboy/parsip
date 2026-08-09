use std::io::Error;
use std::net::{SocketAddr, TcpListener, TcpStream};

pub trait Server {
    fn listen(&self) -> Result<TcpListener, Error>;
    fn accept(&self, listener: &TcpListener) -> Result<(TcpStream, SocketAddr), Error>;
}

#[derive(Debug, Clone)]
pub struct Peer {
    address: String,
}

impl Peer {
    pub fn new(address: String) -> Self {
        Self { address }
    }

    pub fn address(&self) -> &str {
        &self.address
    }
}

impl Server for Peer {
    fn listen(&self) -> Result<TcpListener, Error> {
        let listener = TcpListener::bind(&self.address)?;
        Ok(listener)
    }

    fn accept(&self, listener: &TcpListener) -> Result<(TcpStream, SocketAddr), Error> {
        let (stream, addr) = listener.accept()?;
        Ok((stream, addr))
    }
}
