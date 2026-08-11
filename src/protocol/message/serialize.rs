use crate::protocol::frame::Frame;
use crate::protocol::message::types::Message;

impl From<&Message> for Frame {
    fn from(val: &Message) -> Self {
        let msg_type = val.message_type();

        let payload = match val {
            Message::Hello { public_key, nonce } => {
                let mut p = Vec::with_capacity(64);
                p.extend_from_slice(public_key);
                p.extend_from_slice(nonce);
                p
            }
            Message::HelloProof { signature } => signature.to_vec(),
            Message::ListResources { request_id } => {
                let mut p = Vec::with_capacity(4);
                p.extend_from_slice(&request_id.to_be_bytes());
                p
            }
            Message::ResourceList {
                request_id,
                resources,
            } => {
                let mut p = Vec::new();
                p.extend_from_slice(&request_id.to_be_bytes());
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
            Message::DownloadResource { request_id, id } => {
                let mut p = Vec::with_capacity(4 + 32);
                p.extend_from_slice(&request_id.to_be_bytes());
                p.extend_from_slice(&id.0);
                p
            }
            Message::ResourceChunk {
                request_id,
                id,
                offset,
                data,
            } => {
                let mut p = Vec::with_capacity(4 + 32 + 8 + data.len());
                p.extend_from_slice(&request_id.to_be_bytes());
                p.extend_from_slice(&id.0);
                p.extend_from_slice(&offset.to_be_bytes());
                p.extend_from_slice(data);
                p
            }
            Message::ResourceEnd { request_id, id } => {
                let mut p = Vec::with_capacity(4 + 32);
                p.extend_from_slice(&request_id.to_be_bytes());
                p.extend_from_slice(&id.0);
                p
            }
        };

        Frame::new(msg_type, payload)
    }
}
