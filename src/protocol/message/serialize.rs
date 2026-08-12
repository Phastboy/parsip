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
            Message::SendResourceRequest {
                request_id,
                name,
                size,
            } => {
                let name_bytes = name.as_bytes();
                let mut p = Vec::with_capacity(4 + 8 + 4 + name_bytes.len());
                p.extend_from_slice(&request_id.to_be_bytes());
                p.extend_from_slice(&size.to_be_bytes());
                p.extend_from_slice(&(name_bytes.len() as u32).to_be_bytes());
                p.extend_from_slice(name_bytes);
                p
            }
            Message::SendResourceAccept { request_id } => {
                let mut p = Vec::with_capacity(4);
                p.extend_from_slice(&request_id.to_be_bytes());
                p
            }
            Message::ResourceChunk {
                request_id,
                offset,
                data,
            } => {
                let mut p = Vec::with_capacity(4 + 8 + data.len());
                p.extend_from_slice(&request_id.to_be_bytes());
                p.extend_from_slice(&offset.to_be_bytes());
                p.extend_from_slice(data);
                p
            }
            Message::ResourceEnd { request_id } => {
                let mut p = Vec::with_capacity(4);
                p.extend_from_slice(&request_id.to_be_bytes());
                p
            }
        };

        Frame::new(msg_type, payload)
    }
}
