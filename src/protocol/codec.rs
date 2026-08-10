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
        let len_bytes = length.to_be_bytes();
        let type_byte = [item.message_type];
        
        use std::io::IoSlice;
        let mut iov = [
            IoSlice::new(&len_bytes),
            IoSlice::new(&type_byte),
            IoSlice::new(&item.payload),
        ];
        
        let mut total_written = 0;
        let total_len = 4 + 1 + item.payload.len();
        
        while total_written < total_len {
            let n = stream.write_vectored(&iov)?;
            if n == 0 {
                return Err(Error::new(ErrorKind::WriteZero, "failed to write whole frame"));
            }
            total_written += n;
            
            if total_written < total_len {
                // Adjust IoSlices for remaining data
                let mut advanced = n;
                for slice in &mut iov {
                    if advanced == 0 { break; }
                    let len = slice.len();
                    if len > advanced {
                        *slice = IoSlice::new(unsafe { 
                            std::slice::from_raw_parts(slice.as_ptr().add(advanced), len - advanced)
                        });
                        advanced = 0;
                    } else {
                        *slice = IoSlice::new(&[]);
                        advanced -= len;
                    }
                }
            }
        }
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
