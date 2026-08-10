use std::io::{Error, ErrorKind, Read, Write};

const MAX_FRAME_SIZE: usize = 16 * 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum MessageType {
    Hello = 1,
    ListResources = 2,
    GetResource = 3,
}

impl TryFrom<u8> for MessageType {
    type Error = Error;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            1 => Ok(MessageType::Hello),
            2 => Ok(MessageType::ListResources),
            3 => Ok(MessageType::GetResource),
            _ => Err(Error::new(ErrorKind::InvalidData, format!("Unknown message type: {}", value))),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Frame {
    pub message_type: MessageType,
    pub payload: Vec<u8>,
}

impl Frame {
    pub fn new(message_type: MessageType, payload: Vec<u8>) -> Self {
        Self { message_type, payload }
    }
}

pub trait Encoder<Item> {
    fn encode(&mut self, item: &Item, stream: &mut impl Write) -> Result<(), Error>;
}

pub trait Decoder {
    type Item;
    fn decode(&mut self, stream: &mut impl Read) -> Result<Option<Self::Item>, Error>;
}

pub struct LengthPrefixCodec;

impl Encoder<Frame> for LengthPrefixCodec {
    fn encode(&mut self, item: &Frame, stream: &mut impl Write) -> Result<(), Error> {
        let length = (1 + item.payload.len()) as u32; // 1 byte for type + payload length
        stream.write_all(&length.to_be_bytes())?;
        stream.write_all(&[item.message_type as u8])?;
        stream.write_all(&item.payload)?;
        Ok(())
    }
}

impl Decoder for LengthPrefixCodec {
    type Item = Frame;

    fn decode(&mut self, stream: &mut impl Read) -> Result<Option<Self::Item>, Error> {
        let mut length_buffer = [0u8; 4];

        if let Err(e) = stream.read_exact(&mut length_buffer) {
            if e.kind() == ErrorKind::UnexpectedEof {
                return Ok(None);
            }
            return Err(e);
        }

        let frame_length = u32::from_be_bytes(length_buffer) as usize;

        if frame_length == 0 {
            return Err(Error::new(ErrorKind::InvalidData, "Frame length must be at least 1 byte (for the type)"));
        }
        if frame_length > MAX_FRAME_SIZE {
            return Err(Error::new(ErrorKind::InvalidData, format!("Frame too large: {} bytes", frame_length)));
        }

        let mut body = vec![0u8; frame_length];
        stream.read_exact(&mut body)?;

        let message_type = MessageType::try_from(body[0])?;
        let payload = body[1..].to_vec();

        Ok(Some(Frame { message_type, payload }))
    }
}
