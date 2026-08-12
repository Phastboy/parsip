use crate::protocol::frame::Frame;
use crate::protocol::message::types::Message;
use std::io::{Error, ErrorKind};

fn err(msg: &str) -> Error {
    Error::new(ErrorKind::InvalidData, msg)
}

impl TryFrom<Frame> for Message {
    type Error = Error;

    fn try_from(frame: Frame) -> Result<Self, Self::Error> {
        let p = &frame.payload;
        match frame.message_type {
            1 => {
                if p.len() != 64 {
                    return Err(err("Invalid Hello"));
                }
                let mut pk = [0u8; 32];
                let mut nonce = [0u8; 32];
                pk.copy_from_slice(&p[0..32]);
                nonce.copy_from_slice(&p[32..64]);
                Ok(Message::Hello {
                    public_key: pk,
                    nonce,
                })
            }
            2 => {
                if p.len() != 64 {
                    return Err(err("Invalid HelloProof"));
                }
                let mut sig = [0u8; 64];
                sig.copy_from_slice(p);
                Ok(Message::HelloProof { signature: sig })
            }
            3 => parse_send_resource_request(p),
            4 => parse_send_resource_accept(p),
            5 => parse_resource_chunk(frame.payload),
            6 => parse_resource_end(p),
            _ => Err(err("Unknown msg type")),
        }
    }
}

fn parse_send_resource_request(p: &[u8]) -> Result<Message, Error> {
    if p.len() < 16 {
        return Err(err("Bad SendResourceRequest"));
    }
    let request_id = u32::from_be_bytes(p[0..4].try_into().unwrap());
    let size = u64::from_be_bytes(p[4..12].try_into().unwrap());
    let name_len = u32::from_be_bytes(p[12..16].try_into().unwrap()) as usize;
    if p.len() < 16 + name_len {
        return Err(err("Bad SendResourceRequest Name Length"));
    }
    let name = String::from_utf8(p[16..16 + name_len].to_vec()).map_err(|_| err("Bad UTF8"))?;
    Ok(Message::SendResourceRequest {
        request_id,
        name,
        size,
    })
}

fn parse_send_resource_accept(p: &[u8]) -> Result<Message, Error> {
    if p.len() != 4 {
        return Err(err("Bad SendResourceAccept"));
    }
    let request_id = u32::from_be_bytes(p[0..4].try_into().unwrap());
    Ok(Message::SendResourceAccept { request_id })
}

fn parse_resource_chunk(mut payload: Vec<u8>) -> Result<Message, Error> {
    if payload.len() < 12 {
        return Err(err("Bad ResourceChunk"));
    }
    let request_id = u32::from_be_bytes(payload[0..4].try_into().unwrap());
    let offset = u64::from_be_bytes(payload[4..12].try_into().unwrap());
    
    payload.drain(0..12);
    let data = payload;
    Ok(Message::ResourceChunk {
        request_id,
        offset,
        data,
    })
}

fn parse_resource_end(p: &[u8]) -> Result<Message, Error> {
    if p.len() != 4 {
        return Err(err("Bad ResourceEnd"));
    }
    let request_id = u32::from_be_bytes(p[0..4].try_into().unwrap());
    Ok(Message::ResourceEnd { request_id })
}
