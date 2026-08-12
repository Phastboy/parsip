pub mod handshake;
pub mod reader;

use std::io::{Error, Write};
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
    pub sender: std::sync::mpsc::SyncSender<Frame>,
}

pub struct ConnectionReader {
    pub(crate) stream: TcpStream,
    pub(crate) remote_addr: SocketAddr,
}

impl Connection {
    pub fn new(
        stream: TcpStream,
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

        let _ = stream.set_write_timeout(Some(Duration::from_secs(60)));
        let (tx, rx) = std::sync::mpsc::sync_channel::<Frame>(256);

        let remote_addr_clone = remote_addr;
        std::thread::spawn(move || {
            // BufWriter coalesces multiple frame encodes into large kernel writes.
            // Without this every rx.recv() → encode → write_vectored triggers a
            // separate syscall per thread wakeup, which at 128KB/frame on a LAN
            // produces ~6 frames/second due to scheduling latency alone.
            let mut writer = std::io::BufWriter::with_capacity(256 * 1024, stream);
            let mut codec = LengthPrefixCodec;
            while let Ok(frame) = rx.recv() {
                if let Err(e) = codec.encode(&frame, &mut writer) {
                    eprintln!(
                        "Writer thread for {} exited due to error: {}",
                        remote_addr_clone, e
                    );
                    let _ = writer.into_inner().map(|s| s.shutdown(std::net::Shutdown::Both));
                    return;
                }
                // Drain any immediately available frames from the channel to coalesce
                // them into the same buffer before flushing to the kernel.
                while let Ok(frame) = rx.try_recv() {
                    if let Err(e) = codec.encode(&frame, &mut writer) {
                        eprintln!(
                            "Writer thread for {} exited due to error: {}",
                            remote_addr_clone, e
                        );
                        let _ = writer.into_inner().map(|s| s.shutdown(std::net::Shutdown::Both));
                        return;
                    }
                }

                // The channel is now transiently empty (burst is done). Flush the
                // BufWriter to the kernel. This keeps latency low for control
                // messages while still batching bulk data frames.
                if let Err(e) = writer.flush() {
                    eprintln!(
                        "Writer thread for {} flush error: {}",
                        remote_addr_clone, e
                    );
                    let _ = writer.into_inner().map(|s| s.shutdown(std::net::Shutdown::Both));
                    return;
                }
            }
            // Channel closed cleanly — flush remaining buffered data before exit
            let _ = writer.flush();
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
