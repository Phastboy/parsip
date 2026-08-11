use crate::daemon::control::{ControlMessage, ControlResponse};
use crate::identity::PeerId;
use crate::protocol::Message;
use std::net::SocketAddr;
use std::sync::mpsc::Sender;

pub enum PeerEvent {
    NewConnection(PeerId),
    Disconnected(PeerId),
    Message(PeerId, Message),
    Discovered(PeerId, SocketAddr, String), // Added nickname
    ControlRequest(ControlMessage, Sender<ControlResponse>),
    ScanTimeout(u32),
    ConnectResult(u32, Result<PeerId, String>),
}
