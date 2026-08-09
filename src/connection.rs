use std::io::Error;
use std::net::{SocketAddr, TcpStream};
use std::thread;

use crate::protocol::{Encoder, Frame, LengthPrefixCodec, MessageType};
use crate::transport;

pub struct Connection {
    remote_addr: SocketAddr,
    stream: TcpStream,
}

impl Connection {
    pub fn new(stream: TcpStream, remote_addr: SocketAddr) -> Self {
        Self { remote_addr, stream }
    }

    pub fn send_hello(&mut self, payload: Vec<u8>) -> Result<(), Error> {
        let mut codec = LengthPrefixCodec;
        let frame = Frame::new(MessageType::Hello, payload);
        codec.encode(&frame, &mut self.stream)?;
        Ok(())
    }

    pub fn start_read_loop(&self) -> Result<(), Error> {
        let read_stream = self.stream.try_clone()?;
        let peer_addr = self.remote_addr;

        thread::spawn(move || {
            transport::handle_connection(read_stream, peer_addr);
        });

        Ok(())
    }
}
