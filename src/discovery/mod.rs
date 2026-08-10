use std::io::Error;
use std::sync::mpsc::Sender;

use crate::identity::PeerId;
use crate::peer::PeerEvent;

pub mod broadcaster;
pub mod listener;

pub const DISCOVERY_PORT: u16 = 9090;
pub const MAGIC: &[u8; 6] = b"PARSIP";

pub struct Discovery;

impl Discovery {
    pub fn start(tcp_listen_port: u16, my_id: PeerId, event_tx: Sender<PeerEvent>) -> Result<(), Error> {
        broadcaster::spawn(tcp_listen_port, &my_id)?;
        listener::spawn(my_id, event_tx)?;
        Ok(())
    }
}
