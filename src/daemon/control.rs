use crate::identity::PeerId;
use serde::{Deserialize, Serialize};

/// Represents a message sent from the CLI Client to the Daemon over the local TCP control socket.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ControlMessage {
    Scan,
    Connect {
        alias: String,
    },
    ListPeers,
    SendResource {
        peer_alias: String,
        file_path: String,
    },
}

/// Represents a response sent from the Daemon back to the CLI Client.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ControlResponse {
    Ok,
    Error(String),
    ScanResults(Vec<DiscoveredPeerInfo>),
    PeersList(Vec<ConnectedPeerInfo>),
    TransferInitiated,
    TransferProgress { bytes: u64, total: u64, mbps: f64 },
    TransferComplete { bytes: u64, elapsed_secs: f64 },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscoveredPeerInfo {
    pub alias: String,
    pub nickname: String,
    pub address: std::net::SocketAddr,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectedPeerInfo {
    pub alias: String,
    pub peer_id: PeerId,
}
