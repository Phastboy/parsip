#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct ResourceId(pub [u8; 32]);

#[derive(Debug, Clone)]
pub struct ResourceInfo {
    pub id: ResourceId,
    pub name: String,
    pub size: u64,
}

#[derive(Debug, Clone)]
pub enum Message {
    Hello { public_key: [u8; 32], nonce: [u8; 32] },
    HelloProof { signature: [u8; 64] },
    ListResources,
    ResourceList { resources: Vec<ResourceInfo> },
    GetChunk { id: ResourceId, offset: u64, length: u32 },
    ResourceChunk { id: ResourceId, offset: u64, data: Vec<u8> },
}

impl Message {
    pub(crate) fn message_type(&self) -> u8 {
        match self {
            Message::Hello { .. } => 1,
            Message::HelloProof { .. } => 2,
            Message::ListResources => 3,
            Message::ResourceList { .. } => 4,
            Message::GetChunk { .. } => 5,
            Message::ResourceChunk { .. } => 6,
        }
    }
}
