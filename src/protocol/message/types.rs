#[derive(Debug, Clone)]
pub struct ResourceInfo {
    pub name: String,
    pub size: u64,
}

#[derive(Debug, Clone)]
pub enum Message {
    Hello { public_key: [u8; 32], nonce: [u8; 32] },
    HelloProof { signature: [u8; 64] },
    ListResources,
    ResourceList { resources: Vec<ResourceInfo> },
    GetResource { name: String },
    ResourceData { name: String, data: Vec<u8> },
}

impl Message {
    pub(crate) fn message_type(&self) -> u8 {
        match self {
            Message::Hello { .. } => 1,
            Message::HelloProof { .. } => 2,
            Message::ListResources => 3,
            Message::ResourceList { .. } => 4,
            Message::GetResource { .. } => 5,
            Message::ResourceData { .. } => 6,
        }
    }
}
