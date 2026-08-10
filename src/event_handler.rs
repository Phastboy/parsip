use crate::peer::{Peer, PeerEvent};
use crate::protocol::Message;
use crate::fs_dir::downloads_dir;

pub fn handle_event(peer: &Peer, store: &crate::resource::LocalResourceStore, event: PeerEvent) {
    match event {
        PeerEvent::Discovered(peer_id, addr) => {
            if !peer.is_connected(&peer_id) {
                println!("[Discovery] Found peer {:?} at {}, connecting...", peer_id, addr);
                let _ = peer.connect(addr);
            }
        }
        PeerEvent::NewConnection(peer_id) => {
            println!("[Event] New connection established with {:?}", peer_id);
            let _ = peer.send(&peer_id, &Message::ListResources);
        }
        PeerEvent::Disconnected(peer_id) => {
            println!("[Event] Peer {:?} disconnected", peer_id);
        }
        PeerEvent::Message(peer_id, msg) => handle_message(peer, store, peer_id, msg),
    }
}

fn handle_message(peer: &Peer, store: &crate::resource::LocalResourceStore, peer_id: crate::identity::PeerId, msg: Message) {
    match msg {
        Message::ListResources => {
            println!("[Protocol] Peer {:?} requested ListResources", peer_id);
            let resources = store.list_resources();
            let _ = peer.send(&peer_id, &Message::ResourceList { resources });
        }
        Message::ResourceList { resources } => {
            println!("[Protocol] Received resources from {:?}:", peer_id);
            for res in &resources {
                println!(" - {} ({} bytes, id: {:?})", res.name, res.size, res.id);
            }
            if let Some(res) = resources.first() {
                println!("[Protocol] Automatically downloading {}...", res.name);
                let _ = peer.send(&peer_id, &Message::GetChunk { 
                    id: res.id.clone(), 
                    offset: 0, 
                    // Safely request up to 1MB or the file size
                    length: (res.size as u32).min(1024 * 1024) 
                });
            }
        }
        Message::GetChunk { id, offset, length } => {
            println!("[Protocol] Peer {:?} requested GetChunk: {:?} offset {} len {}", peer_id, id, offset, length);
            if let Some(file_path) = store.get_path(&id) {
                use std::io::{Seek, SeekFrom, Read};
                if let Ok(mut file) = std::fs::File::open(file_path) {
                    if file.seek(SeekFrom::Start(offset)).is_ok() {
                        let mut buffer = vec![0u8; length as usize];
                        if let Ok(bytes_read) = file.read(&mut buffer) {
                            buffer.truncate(bytes_read);
                            let _ = peer.send(&peer_id, &Message::ResourceChunk { 
                                id, 
                                offset, 
                                data: buffer 
                            });
                        }
                    }
                }
            } else {
                eprintln!("[Protocol] Resource {:?} not found", id);
            }
        }
        Message::ResourceChunk { id, offset, data } => {
            println!("[Protocol] Received chunk for {:?} offset {} ({} bytes)", id, offset, data.len());
            // For now, just save it as the chunk (full file download assembly will come next!)
            let file_path = downloads_dir().join(format!("{:?}.chunk", id));
            if let Err(e) = std::fs::write(&file_path, data) {
                eprintln!("Failed to save downloaded chunk: {}", e);
            } else {
                println!("Saved chunk to {:?}", file_path);
            }
        }
        _ => {}
    }
}
