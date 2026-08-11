use crate::protocol::frame::Frame;
use std::io::{Error, ErrorKind, Read, Write};

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
                return Err(Error::new(
                    ErrorKind::WriteZero,
                    "failed to write whole frame",
                ));
            }
            total_written += n;

            if total_written < total_len {
                // Adjust IoSlices for remaining data
                let mut advanced = n;
                for slice in &mut iov {
                    if advanced == 0 {
                        break;
                    }
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
            return Err(Error::new(
                ErrorKind::InvalidData,
                "Frame length must be at least 1 byte (for the type)",
            ));
        }
        if frame_length > MAX_FRAME_SIZE {
            return Err(Error::new(
                ErrorKind::InvalidData,
                format!("Frame too large: {} bytes", frame_length),
            ));
        }

        let mut body = vec![0u8; frame_length];
        stream.read_exact(&mut body)?;

        let message_type = body[0];
        let payload = body[1..].to_vec();

        Ok(Some(Frame {
            message_type,
            payload,
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn test_encode_decode_basic() {
        let mut codec = LengthPrefixCodec;
        let original_frame = Frame {
            message_type: 42,
            payload: vec![1, 2, 3, 4, 5],
        };

        let mut buffer = Vec::new();
        codec.encode(&original_frame, &mut buffer).unwrap();

        // 4 bytes length + 1 byte type + 5 bytes payload = 10 bytes total
        assert_eq!(buffer.len(), 10);
        // length is payload (5) + type (1) = 6. Big endian 6 is [0, 0, 0, 6]
        assert_eq!(&buffer[0..4], &[0, 0, 0, 6]);
        assert_eq!(buffer[4], 42);
        assert_eq!(&buffer[5..], &[1, 2, 3, 4, 5]);

        let mut cursor = Cursor::new(buffer);
        let decoded_frame = codec.decode(&mut cursor).unwrap().unwrap();

        assert_eq!(decoded_frame.message_type, original_frame.message_type);
        assert_eq!(decoded_frame.payload, original_frame.payload);
    }

    #[test]
    fn test_decode_unexpected_eof() {
        let mut codec = LengthPrefixCodec;
        let buffer = vec![0, 0, 0]; // Missing one byte for length
        let mut cursor = Cursor::new(buffer);
        let result = codec.decode(&mut cursor);
        // read_exact on 3 bytes when expecting 4 returns UnexpectedEof
        // The decoder maps UnexpectedEof -> Ok(None) to signal clean stream end
        assert!(result.unwrap().is_none());
    }

    #[test]
    fn test_decode_too_large() {
        let mut codec = LengthPrefixCodec;
        let length: u32 = (MAX_FRAME_SIZE + 1) as u32;
        let buffer = length.to_be_bytes().to_vec();
        let mut cursor = Cursor::new(buffer);
        let result = codec.decode(&mut cursor);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().kind(), ErrorKind::InvalidData);
    }

    #[test]
    fn test_decode_zero_length() {
        let mut codec = LengthPrefixCodec;
        let buffer = vec![0, 0, 0, 0];
        let mut cursor = Cursor::new(buffer);
        let result = codec.decode(&mut cursor);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().kind(), ErrorKind::InvalidData);
    }
}
