use serde::{Deserialize, Serialize};
use crate::identity::PeerId;
use crate::protocol::message::types::ResourceId;

/// Represents a message sent from the CLI Client to the Daemon over the local TCP control socket.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ControlMessage {
    Scan,
    Connect { alias: String },
    ListPeers,
    ListResources { peer_alias: String },
    GetResource { peer_alias: String, resource_alias: String },
    AddResource { path: String },
}

/// Represents a response sent from the Daemon back to the CLI Client.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ControlResponse {
    Ok,
    Error(String),
    ScanResults(Vec<DiscoveredPeerInfo>),
    PeersList(Vec<ConnectedPeerInfo>),
    ResourceList(Vec<ResourceInfo>),
    ResourceAdded { alias: String, id: ResourceId },
    DownloadComplete { bytes: u64, elapsed_secs: f64 },
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceInfo {
    pub alias: String,
    pub id: ResourceId,
    pub name: String,
    pub size: u64,
}
