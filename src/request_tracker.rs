use std::collections::HashMap;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::mpsc::Sender;
use crate::daemon::control::ControlResponse;

pub struct RequestTracker {
    next_id: AtomicU32,
    pending_requests: HashMap<u32, Sender<ControlResponse>>,
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

    pub fn register(&mut self, request_id: u32, sender: Sender<ControlResponse>) {
        self.pending_requests.insert(request_id, sender);
    }

    pub fn complete(&mut self, request_id: u32) -> Option<Sender<ControlResponse>> {
        self.pending_requests.remove(&request_id)
    }
}
