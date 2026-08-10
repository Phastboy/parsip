use std::collections::HashMap;
use crate::identity::PeerId;
use crate::protocol::message::types::ResourceId;

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct Transfer {
    pub request_id: u32,
    pub peer_id: PeerId,
    pub resource_id: ResourceId,
    pub is_download: bool,
    pub bytes_transferred: u64,
    pub bytes_total: u64,
    pub start_time: std::time::Instant,
}

pub struct TransferManager {
    transfers: HashMap<u32, Transfer>,
}

impl TransferManager {
    pub fn new() -> Self {
        Self {
            transfers: HashMap::new(),
        }
    }

    pub fn register(&mut self, transfer: Transfer) {
        self.transfers.insert(transfer.request_id, transfer);
    }

    pub fn update_progress(&mut self, request_id: u32, bytes_added: u64) {
        if let Some(t) = self.transfers.get_mut(&request_id) {
            t.bytes_transferred += bytes_added;
        }
    }

    pub fn complete(&mut self, request_id: u32) -> Option<Transfer> {
        self.transfers.remove(&request_id)
    }

    #[allow(dead_code)]
    pub fn list(&self) -> Vec<Transfer> {
        self.transfers.values().cloned().collect()
    }
}
