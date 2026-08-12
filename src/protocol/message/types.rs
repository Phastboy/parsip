



#[derive(Debug, Clone)]
pub enum Message {
    Hello {
        public_key: [u8; 32],
        nonce: [u8; 32],
    },
    HelloProof {
        signature: [u8; 64],
    },
    SendResourceRequest {
        request_id: u32,
        name: String,
        size: u64,
    },
    SendResourceAccept {
        request_id: u32,
    },
    ResourceChunk {
        request_id: u32,
        offset: u64,
        data: Vec<u8>,
    },
    ResourceEnd {
        request_id: u32,
    },
}

impl Message {
    pub(crate) fn message_type(&self) -> u8 {
        match self {
            Message::Hello { .. } => 1,
            Message::HelloProof { .. } => 2,
            Message::SendResourceRequest { .. } => 3,
            Message::SendResourceAccept { .. } => 4,
            Message::ResourceChunk { .. } => 5,
            Message::ResourceEnd { .. } => 6,
        }
    }
}
