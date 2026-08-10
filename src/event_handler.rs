use crate::peer::{Peer, PeerEvent};
use crate::protocol::Message;

pub fn handle_event(
    peer: &Peer, 
    store: &crate::resource::LocalResourceStore, 
    download_mgr: &mut crate::resource::DownloadManager, 
    event: PeerEvent
) {
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
        PeerEvent::Message(peer_id, msg) => handle_message(peer, store, download_mgr, peer_id, msg),
    }
}

fn handle_message(
    peer: &Peer, 
    store: &crate::resource::LocalResourceStore, 
    download_mgr: &mut crate::resource::DownloadManager, 
    peer_id: crate::identity::PeerId, 
    msg: Message
) {
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
                let chunk_size = 32 * 1024; // 32KB
                if let Ok(()) = download_mgr.start_download(res, chunk_size) {
                    if let Some((offset, length)) = download_mgr.get_next_request(&res.id) {
                        let _ = peer.send(&peer_id, &Message::GetChunk { id: res.id.clone(), offset, length });
                    }
                } else {
                    eprintln!("[Protocol] Failed to start download for {}", res.name);
                }
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
            match download_mgr.process_chunk(&id, offset, &data) {
                Ok(true) => {
                    if let Ok(()) = download_mgr.complete_download(&id) {
                        println!("[Protocol] Download complete for {:?}", id);
                    } else {
                        eprintln!("[Protocol] Failed to complete download for {:?}", id);
                    }
                }
                Ok(false) => {
                    // Request next chunk
                    if let Some((next_offset, length)) = download_mgr.get_next_request(&id) {
                        let _ = peer.send(&peer_id, &Message::GetChunk { id, offset: next_offset, length });
                    }
                }
                Err(e) => {
                    eprintln!("[Protocol] Error processing chunk: {}", e);
                }
            }
        }
        _ => {}
    }
}
