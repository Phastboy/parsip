use serde::{Deserialize, Serialize};

#[derive(Clone, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
pub struct ResourceId(pub [u8; 32]);

#[derive(Debug, Clone)]
pub struct ResourceInfo {
    pub id: ResourceId,
    pub name: String,
    pub size: u64,
}

#[derive(Debug, Clone)]
pub enum Message {
    Hello {
        public_key: [u8; 32],
        nonce: [u8; 32],
    },
    HelloProof {
        signature: [u8; 64],
    },
    ListResources {
        request_id: u32,
    },
    ResourceList {
        request_id: u32,
        resources: Vec<ResourceInfo>,
    },
    DownloadResource {
        request_id: u32,
        id: ResourceId,
    },
    ResourceChunk {
        request_id: u32,
        id: ResourceId,
        offset: u64,
        data: Vec<u8>,
    },
    ResourceEnd {
        request_id: u32,
        id: ResourceId,
    },
}

impl Message {
    pub(crate) fn message_type(&self) -> u8 {
        match self {
            Message::Hello { .. } => 1,
            Message::HelloProof { .. } => 2,
            Message::ListResources { .. } => 3,
            Message::ResourceList { .. } => 4,
            Message::DownloadResource { .. } => 5,
            Message::ResourceChunk { .. } => 6,
            Message::ResourceEnd { .. } => 7,
        }
    }
}
