use crate::identity::PeerId;
use crate::protocol::message::types::ResourceId;
use std::collections::HashMap;

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

    pub fn update_progress(
        &mut self,
        request_id: u32,
        bytes_added: u64,
    ) -> Option<(u64, u64, f64)> {
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

    /// Drain all active transfers for a peer that just disconnected.
    /// Returns the list so the caller can cancel their pending CLI requests.
    pub fn cancel_for_peer(&mut self, peer_id: &PeerId) -> Vec<Transfer> {
        let ids: Vec<u32> = self
            .transfers
            .values()
            .filter(|t| &t.peer_id == peer_id)
            .map(|t| t.request_id)
            .collect();
        ids.into_iter()
            .filter_map(|id| self.transfers.remove(&id))
            .collect()
    }

    #[allow(dead_code)]
    pub fn list(&self) -> Vec<Transfer> {
        self.transfers.values().cloned().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_transfer_progress() {
        let mut mgr = TransferManager::new();
        let t = Transfer {
            request_id: 1,
            peer_id: PeerId([0; 32]),
            resource_id: ResourceId([1; 32]),
            is_download: true,
            bytes_transferred: 0,
            bytes_total: 1000,
            start_time: std::time::Instant::now(),
            last_report_time: std::time::Instant::now() - std::time::Duration::from_millis(250), // force update
            last_report_bytes: 0,
        };
        mgr.register(t);

        let progress = mgr.update_progress(1, 100);
        assert!(progress.is_some());
        let (transferred, total, mbps) = progress.unwrap();
        assert_eq!(transferred, 100);
        assert_eq!(total, 1000);
        assert!(mbps > 0.0);
    }

    #[test]
    fn test_cancel_for_peer() {
        let mut mgr = TransferManager::new();
        let p1 = PeerId([1; 32]);
        let p2 = PeerId([2; 32]);

        let mut t1 = Transfer {
            request_id: 1,
            peer_id: p1.clone(),
            resource_id: ResourceId([1; 32]),
            is_download: true,
            bytes_transferred: 0,
            bytes_total: 100,
            start_time: std::time::Instant::now(),
            last_report_time: std::time::Instant::now(),
            last_report_bytes: 0,
        };
        let mut t2 = t1.clone();
        t2.request_id = 2;
        t2.peer_id = p2.clone(); // different peer

        mgr.register(t1);
        mgr.register(t2);

        let cancelled = mgr.cancel_for_peer(&p1);
        assert_eq!(cancelled.len(), 1);
        assert_eq!(cancelled[0].request_id, 1);

        assert!(mgr.complete(1).is_none());
        assert!(mgr.complete(2).is_some()); // t2 should still be there
    }
}
