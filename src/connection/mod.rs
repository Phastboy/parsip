pub mod handshake;
pub mod reader;

use std::io::Error;
use std::net::{SocketAddr, TcpStream};
use std::time::Duration;

use crate::identity::PeerId;
use crate::protocol::{Encoder, Frame, LengthPrefixCodec};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Direction {
    Incoming,
    Outgoing,
}

pub struct Connection {
    pub remote_peer_id: PeerId,
    pub direction: Direction,
    pub sender: std::sync::mpsc::Sender<Frame>,
}

pub struct ConnectionReader {
    pub(crate) stream: TcpStream,
    pub(crate) remote_addr: SocketAddr,
}

impl Connection {
    pub fn new(
        mut stream: TcpStream,
        remote_addr: SocketAddr,
        remote_peer_id: PeerId,
        direction: Direction,
    ) -> Result<(Self, ConnectionReader), Error> {
        let _ = stream.set_nonblocking(false);
        let _ = stream.set_nodelay(true);

        // Enable TCP keepalive so the OS actively probes the connection when idle.
        // Without this, a peer that disappears mid-transfer (WiFi sleep, crash) is
        // never detected — read_exact just blocks forever.
        //
        // Settings:
        //   idle     = 10s  → start probing after 10 seconds of no data
        //   interval = 5s   → probe every 5 seconds
        //   retries  = 3    → give up after 3 missed probes (15s of probing)
        //
        // Total detection time: ~25 seconds after a peer goes dark.
        {
            use socket2::{SockRef, TcpKeepalive};
            let keepalive = TcpKeepalive::new()
                .with_time(Duration::from_secs(10))
                .with_interval(Duration::from_secs(5))
                .with_retries(3);
            let sock_ref = SockRef::from(&stream);
            let _ = sock_ref.set_tcp_keepalive(&keepalive);
        }

        // Bounded frame-progress timeout. If an authenticated peer starts sending a frame
        // but stalls (e.g. hung process, slow loris), read_exact will block forever because
        // TCP keepalive only checks if the OS is reachable, not if the application is sending.
        // We set a 60s read timeout here. If this fires, the reader thread treats it as fatal
        // and drops the connection, preventing blocked threads.
        let _ = stream.set_read_timeout(Some(Duration::from_secs(60)));

        let read_stream = stream.try_clone()?;

        let (tx, rx) = std::sync::mpsc::channel::<Frame>();

        let remote_addr_clone = remote_addr;
        std::thread::spawn(move || {
            let mut codec = LengthPrefixCodec;
            while let Ok(frame) = rx.recv() {
                if let Err(e) = codec.encode(&frame, &mut stream) {
                    eprintln!(
                        "Writer thread for {} exited due to error: {}",
                        remote_addr_clone, e
                    );
                    let _ = stream.shutdown(std::net::Shutdown::Both);
                    break;
                }
            }
        });

        let writer = Self {
            remote_peer_id,
            direction,
            sender: tx,
        };

        let reader = ConnectionReader {
            stream: read_stream,
            remote_addr,
        };

        Ok((writer, reader))
    }
}
