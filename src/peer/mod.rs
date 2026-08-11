pub mod acceptor;
pub mod connector;
pub mod event;

use std::io::Error;
use std::net::{SocketAddr, TcpListener};
use std::sync::Arc;
use std::sync::mpsc::{self, Receiver, Sender};

use crate::connection_manager::ConnectionManager;
use crate::identity::{Identity, PeerId};
use crate::protocol::Message;
pub use event::PeerEvent;

#[derive(Clone)]
pub struct Peer {
    pub(crate) identity: Arc<Identity>,
    pub id: PeerId,
    address: SocketAddr,
    pub(crate) manager: ConnectionManager,
    pub event_tx: Sender<PeerEvent>,
}

impl Peer {
    pub fn new(identity: Identity, address: SocketAddr) -> (Self, Receiver<PeerEvent>) {
        let id = identity.peer_id.clone();
        let (tx, rx) = mpsc::channel();
        let peer = Self {
            identity: Arc::new(identity),
            id,
            address,
            manager: ConnectionManager::new(),
            event_tx: tx,
        };
        (peer, rx)
    }

    pub fn address(&self) -> SocketAddr {
        self.address
    }

    pub fn is_connected(&self, peer_id: &PeerId) -> bool {
        self.manager.is_connected(peer_id)
    }

    pub fn send(&self, target: &PeerId, message: &Message) -> Result<(), Error> {
        self.manager.send(target, message)
    }

    pub fn listen(&self) -> Result<TcpListener, Error> {
        TcpListener::bind(self.address)
    }
}
