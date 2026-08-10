use crate::protocol::frame::Frame;
use crate::protocol::message::types::Message;

impl Into<Frame> for &Message {
    fn into(self) -> Frame {
        let msg_type = self.message_type();
        
        let payload = match self {
            Message::Hello { public_key, nonce } => {
                let mut p = Vec::with_capacity(64);
                p.extend_from_slice(public_key);
                p.extend_from_slice(nonce);
                p
            }
            Message::HelloProof { signature } => signature.to_vec(),
            Message::ListResources => vec![],
            Message::ResourceList { resources } => {
                let mut p = Vec::new();
                p.extend_from_slice(&(resources.len() as u32).to_be_bytes());
                for r in resources {
                    p.extend_from_slice(&r.id.0); // 32 bytes
                    let name_bytes = r.name.as_bytes();
                    p.extend_from_slice(&(name_bytes.len() as u32).to_be_bytes());
                    p.extend_from_slice(name_bytes);
                    p.extend_from_slice(&r.size.to_be_bytes());
                }
                p
            }
            Message::GetChunk { id, offset, length } => {
                let mut p = Vec::with_capacity(32 + 8 + 4);
                p.extend_from_slice(&id.0);
                p.extend_from_slice(&offset.to_be_bytes());
                p.extend_from_slice(&length.to_be_bytes());
                p
            }
            Message::ResourceChunk { id, offset, data } => {
                let mut p = Vec::with_capacity(32 + 8 + data.len());
                p.extend_from_slice(&id.0);
                p.extend_from_slice(&offset.to_be_bytes());
                p.extend_from_slice(data);
                p
            }
        };

        Frame::new(msg_type, payload)
    }
}
