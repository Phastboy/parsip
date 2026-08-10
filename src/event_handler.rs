use std::fs;
use crate::peer::{Peer, PeerEvent};
use crate::protocol::{Message, ResourceInfo};
use crate::fs_dir::{shared_dir, downloads_dir};

pub fn handle_event(peer: &Peer, event: PeerEvent) {
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
        PeerEvent::Message(peer_id, msg) => handle_message(peer, peer_id, msg),
    }
}

fn handle_message(peer: &Peer, peer_id: crate::identity::PeerId, msg: Message) {
    match msg {
        Message::ListResources => {
            println!("[Protocol] Peer {:?} requested ListResources", peer_id);
            let mut resources = Vec::new();
            if let Ok(entries) = fs::read_dir(shared_dir()) {
                for entry in entries.flatten() {
                    if let Ok(metadata) = entry.metadata() {
                        if metadata.is_file() {
                            resources.push(ResourceInfo {
                                name: entry.file_name().to_string_lossy().to_string(),
                                size: metadata.len(),
                            });
                        }
                    }
                }
            }
            let _ = peer.send(&peer_id, &Message::ResourceList { resources });
        }
        Message::ResourceList { resources } => {
            println!("[Protocol] Received resources from {:?}:", peer_id);
            for res in &resources {
                println!(" - {} ({} bytes)", res.name, res.size);
            }
            if let Some(res) = resources.first() {
                println!("[Protocol] Automatically downloading {}...", res.name);
                let _ = peer.send(&peer_id, &Message::GetResource { name: res.name.clone() });
            }
        }
        Message::GetResource { name } => {
            println!("[Protocol] Peer {:?} requested GetResource: {}", peer_id, name);
            if !name.contains('/') && !name.contains('\\') {
                let file_path = shared_dir().join(&name);
                if let Ok(data) = fs::read(file_path) {
                    let _ = peer.send(&peer_id, &Message::ResourceData { name, data });
                } else {
                    eprintln!("[Protocol] File {} not found or unreadable", name);
                }
            }
        }
        Message::ResourceData { name, data } => {
            println!("[Protocol] Received file {} from {:?} ({} bytes)", name, peer_id, data.len());
            let file_path = downloads_dir().join(&name);
            if let Err(e) = fs::write(&file_path, data) {
                eprintln!("Failed to save downloaded file: {}", e);
            } else {
                println!("Saved to {:?}", file_path);
            }
        }
        _ => {}
    }
}
