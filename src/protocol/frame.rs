#[derive(Debug, Clone)]
pub struct Frame {
    pub message_type: u8,
    pub payload: Vec<u8>,
}

impl Frame {
    pub fn new(message_type: u8, payload: Vec<u8>) -> Self {
        Self {
            message_type,
            payload,
        }
    }
}
