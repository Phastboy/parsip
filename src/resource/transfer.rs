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
    pub last_report_time: std::time::Instant,
    pub last_report_bytes: u64,
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

    pub fn update_progress(&mut self, request_id: u32, bytes_added: u64) -> Option<(u64, u64, f64)> {
        if let Some(t) = self.transfers.get_mut(&request_id) {
            t.bytes_transferred += bytes_added;
            let elapsed_ms = t.last_report_time.elapsed().as_millis();
            if elapsed_ms > 200 {
                let bytes_since_last = t.bytes_transferred - t.last_report_bytes;
                let mb_since_last = bytes_since_last as f64 / 1_000_000.0;
                let secs_since_last = elapsed_ms as f64 / 1000.0;
                let mbps = mb_since_last / secs_since_last;

                t.last_report_time = std::time::Instant::now();
                t.last_report_bytes = t.bytes_transferred;

                return Some((t.bytes_transferred, t.bytes_total, mbps));
            }
        }
        None
    }

    pub fn complete(&mut self, request_id: u32) -> Option<Transfer> {
        self.transfers.remove(&request_id)
    }

    #[allow(dead_code)]
    pub fn list(&self) -> Vec<Transfer> {
        self.transfers.values().cloned().collect()
    }
}
