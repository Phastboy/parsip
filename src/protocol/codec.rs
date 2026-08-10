use std::io::{Error, ErrorKind, Read, Write};
use crate::protocol::frame::Frame;

const MAX_FRAME_SIZE: usize = 16 * 1024 * 1024; // 16MB

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
        let length = (1 + item.payload.len()) as u32;
        stream.write_all(&length.to_be_bytes())?;
        stream.write_all(&[item.message_type])?;
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

        let message_type = body[0];
        let payload = body[1..].to_vec();

        Ok(Some(Frame { message_type, payload }))
    }
}
