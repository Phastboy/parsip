pub mod handshake;
pub mod reader;

use std::net::{SocketAddr, TcpStream};
use std::time::Duration;
use std::io::Error;

use crate::identity::PeerId;
use crate::protocol::{Encoder, Frame, LengthPrefixCodec};

pub struct Connection {
    pub(crate) remote_addr: SocketAddr,
    pub remote_peer_id: Option<PeerId>,
    pub(crate) stream: TcpStream,
}

impl Connection {
    pub fn new(stream: TcpStream, remote_addr: SocketAddr) -> Self {
        let _ = stream.set_read_timeout(Some(Duration::from_secs(30)));
        let _ = stream.set_write_timeout(Some(Duration::from_secs(30)));
        Self { remote_addr, remote_peer_id: None, stream }
    }

    pub fn send(&mut self, frame: &Frame) -> Result<(), Error> {
        let mut codec = LengthPrefixCodec;
        codec.encode(frame, &mut self.stream)
    }
}
