pub mod handshake;
pub mod reader;

use std::net::{SocketAddr, TcpStream};
use std::time::Duration;
use std::io::Error;

use crate::identity::PeerId;
use crate::protocol::{Encoder, Frame, LengthPrefixCodec};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Direction {
    Incoming,
    Outgoing,
}

pub struct Connection {
    pub remote_peer_id: Option<PeerId>,
    pub(crate) stream: TcpStream,
}

pub struct ConnectionReader {
    pub(crate) stream: TcpStream,
    pub(crate) remote_addr: SocketAddr,
}

impl Connection {
    pub fn new(stream: TcpStream, remote_addr: SocketAddr) -> Result<(Self, ConnectionReader), Error> {
        let _ = stream.set_nonblocking(false);
        let _ = stream.set_nodelay(true);
        // Read timeout: detect dead peers that stop sending (e.g. crash without FIN).
        // No write timeout: TCP backpressure naturally handles slow receivers.
        // A write timeout would kill large file transfers on slow networks.
        let _ = stream.set_read_timeout(Some(Duration::from_secs(120)));
        
        let read_stream = stream.try_clone()?;
        
        let writer = Self { remote_peer_id: None, stream };
        let reader = ConnectionReader { stream: read_stream, remote_addr };
        
        Ok((writer, reader))
    }

    pub fn send(&mut self, frame: &Frame) -> Result<(), Error> {
        let mut codec = LengthPrefixCodec;
        codec.encode(frame, &mut self.stream)
    }
}
