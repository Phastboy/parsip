use std::io::{Error, ErrorKind};
use crate::protocol::frame::Frame;
use crate::protocol::message::types::{Message, ResourceInfo};

fn err(msg: &str) -> Error { Error::new(ErrorKind::InvalidData, msg) }

impl TryFrom<Frame> for Message {
    type Error = Error;

    fn try_from(frame: Frame) -> Result<Self, Self::Error> {
        let p = &frame.payload;
        match frame.message_type {
            1 => {
                if p.len() != 64 { return Err(err("Invalid Hello")); }
                let mut pk = [0u8; 32];
                let mut nonce = [0u8; 32];
                pk.copy_from_slice(&p[0..32]);
                nonce.copy_from_slice(&p[32..64]);
                Ok(Message::Hello { public_key: pk, nonce })
            }
            2 => {
                if p.len() != 64 { return Err(err("Invalid HelloProof")); }
                let mut sig = [0u8; 64];
                sig.copy_from_slice(p);
                Ok(Message::HelloProof { signature: sig })
            }
            3 => Ok(Message::ListResources),
            4 => parse_resource_list(p),
            5 => {
                let name = String::from_utf8(p.to_vec()).map_err(|_| err("Bad UTF8"))?;
                Ok(Message::GetResource { name })
            }
            6 => parse_resource_data(p),
            _ => Err(err("Unknown msg type")),
        }
    }
}

fn parse_resource_list(p: &[u8]) -> Result<Message, Error> {
    let mut off = 0;
    if off + 4 > p.len() { return Err(err("Bad List")); }
    let count = u32::from_be_bytes(p[off..off+4].try_into().unwrap());
    off += 4;
    
    let mut resources = Vec::with_capacity(count as usize);
    for _ in 0..count {
        if off + 4 > p.len() { return Err(err("Bad List")); }
        let nl = u32::from_be_bytes(p[off..off+4].try_into().unwrap()) as usize;
        off += 4;
        
        if off + nl > p.len() { return Err(err("Bad List")); }
        let name = String::from_utf8(p[off..off+nl].to_vec()).map_err(|_| err("Bad UTF8"))?;
        off += nl;
        
        if off + 8 > p.len() { return Err(err("Bad List")); }
        let size = u64::from_be_bytes(p[off..off+8].try_into().unwrap());
        off += 8;
        
        resources.push(ResourceInfo { name, size });
    }
    Ok(Message::ResourceList { resources })
}

fn parse_resource_data(p: &[u8]) -> Result<Message, Error> {
    if p.len() < 4 { return Err(err("Bad Data")); }
    let nl = u32::from_be_bytes(p[0..4].try_into().unwrap()) as usize;
    if p.len() < 4 + nl { return Err(err("Bad Data")); }
    let name = String::from_utf8(p[4..4+nl].to_vec()).map_err(|_| err("Bad UTF8"))?;
    Ok(Message::ResourceData { name, data: p[4+nl..].to_vec() })
}
