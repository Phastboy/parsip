use std::net::SocketAddr;
use crate::identity::PeerId;
use crate::protocol::Message;

pub enum PeerEvent {
    NewConnection(PeerId),
    Disconnected(PeerId),
    Message(PeerId, Message),
    Discovered(PeerId, SocketAddr),
    Command(String),
}
