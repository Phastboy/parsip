use std::io::Error;
use std::net::{SocketAddr, TcpListener, TcpStream};

#[derive(Debug, Clone)]
pub struct ParsipServer<'a> {
    address: &'a str,
}

impl<'a> ParsipServer<'a> {
    pub fn new(address: &'a str) -> Self {
        Self { address }
    }

    pub fn listen(&self) -> Result<TcpListener, Error> {
        let listener = TcpListener::bind(self.address)?;
        Ok(listener)
    }
}

pub fn accept(listener: &TcpListener) -> Result<(TcpStream, SocketAddr), Error> {
    let (stream, addr) = listener.accept()?;
    Ok((stream, addr))
}
