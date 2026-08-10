use std::collections::HashMap;
use std::sync::atomic::{AtomicU32, Ordering};

pub struct RequestTracker {
    next_id: AtomicU32,
    // We will later map these to TCP control response channels when building the Daemon RPC
    // For now, we just track the existence of the request.
    pending_requests: HashMap<u32, RequestType>,
}

pub enum RequestType {
    ListResources,
    GetChunk,
}

impl RequestTracker {
    pub fn new() -> Self {
        Self {
            next_id: AtomicU32::new(1),
            pending_requests: HashMap::new(),
        }
    }

    pub fn next_id(&self) -> u32 {
        self.next_id.fetch_add(1, Ordering::SeqCst)
    }

    pub fn register(&mut self, request_id: u32, req_type: RequestType) {
        self.pending_requests.insert(request_id, req_type);
    }

    pub fn complete(&mut self, request_id: u32) -> Option<RequestType> {
        self.pending_requests.remove(&request_id)
    }
}
