use std::collections::HashMap;
use std::net::SocketAddr;
use std::path::PathBuf;

use crate::peer::{Peer, PeerEvent};
use crate::protocol::{ResourceInfo, Message, message::types::ResourceId};
use crate::identity::PeerId;

pub struct AliasRegistry {
    peer_aliases: HashMap<String, PeerId>,
    resource_aliases: HashMap<String, ResourceId>,
    resource_info: HashMap<ResourceId, ResourceInfo>,
    next_peer_id: u32,
    next_resource_id: u32,
}

impl AliasRegistry {
    pub fn new() -> Self {
        Self {
            peer_aliases: HashMap::new(),
            resource_aliases: HashMap::new(),
            resource_info: HashMap::new(),
            next_peer_id: 1,
            next_resource_id: 1,
        }
    }

    pub fn add_peer(&mut self, peer_id: PeerId) -> String {
        // If already aliased, return existing
        if let Some((alias, _)) = self.peer_aliases.iter().find(|(_, id)| **id == peer_id) {
            return alias.clone();
        }
        let alias = format!("p{}", self.next_peer_id);
        self.next_peer_id += 1;
        self.peer_aliases.insert(alias.clone(), peer_id);
        alias
    }

    pub fn add_resource(&mut self, resource_id: ResourceId) -> String {
        if let Some((alias, _)) = self.resource_aliases.iter().find(|(_, id)| **id == resource_id) {
            return alias.clone();
        }
        let alias = format!("r{}", self.next_resource_id);
        self.next_resource_id += 1;
        self.resource_aliases.insert(alias.clone(), resource_id);
        alias
    }

    pub fn cache_info(&mut self, info: ResourceInfo) {
        self.resource_info.insert(info.id.clone(), info);
    }

    pub fn get_info(&self, id: &ResourceId) -> Option<ResourceInfo> {
        self.resource_info.get(id).cloned()
    }

    pub fn get_peer(&self, alias: &str) -> Option<PeerId> {
        self.peer_aliases.get(alias).cloned()
    }

    pub fn get_resource(&self, alias: &str) -> Option<ResourceId> {
        self.resource_aliases.get(alias).cloned()
    }
}

pub fn handle_event(
    peer: &Peer, 
    store: &mut crate::resource::LocalResourceStore, 
    download_mgr: &mut crate::resource::DownloadManager, 
    aliases: &mut AliasRegistry,
    event: PeerEvent
) {
    match event {
        PeerEvent::Discovered(peer_id, addr) => {
            if !peer.is_connected(&peer_id) {
                println!("[Discovery] Found peer {:?} at {} (not auto-connecting)", peer_id, addr);
            }
        }
        PeerEvent::NewConnection(peer_id) => {
            let alias = aliases.add_peer(peer_id.clone());
            println!("[Event] New connection established with {:?} (alias: {})", peer_id, alias);
        }
        PeerEvent::Disconnected(peer_id) => {
            println!("[Event] Peer {:?} disconnected", peer_id);
        }
        PeerEvent::Message(peer_id, msg) => handle_message(peer, store, download_mgr, aliases, peer_id, msg),
        PeerEvent::Command(cmd) => handle_command(peer, store, download_mgr, aliases, cmd),
    }
}

fn handle_command(
    peer: &Peer, 
    store: &mut crate::resource::LocalResourceStore, 
    download_mgr: &mut crate::resource::DownloadManager, 
    aliases: &mut AliasRegistry,
    cmd: String
) {
    let parts: Vec<&str> = cmd.split_whitespace().collect();
    if parts.is_empty() { return; }

    match parts[0] {
        "connect" => {
            if parts.len() < 2 {
                println!("Usage: connect <ip:port>");
                return;
            }
            if let Ok(addr) = parts[1].parse::<SocketAddr>() {
                println!("Connecting to {}...", addr);
                let _ = peer.connect(addr);
            } else {
                println!("Invalid address format");
            }
        }
        "peers" => {
            println!("Connected Peers:");
            for (alias, id) in &aliases.peer_aliases {
                if peer.is_connected(id) {
                    println!("  {} -> {:?}", alias, id);
                }
            }
        }
        "list" => {
            if parts.len() < 2 {
                println!("Usage: list <peer_alias>");
                return;
            }
            if let Some(peer_id) = aliases.get_peer(parts[1]) {
                let _ = peer.send(&peer_id, &Message::ListResources);
                println!("Requested resource list from {}", parts[1]);
            } else {
                println!("Unknown peer alias: {}", parts[1]);
            }
        }
        "get" => {
            if parts.len() < 3 {
                println!("Usage: get <peer_alias> <resource_alias>");
                return;
            }
            if let Some(peer_id) = aliases.get_peer(parts[1]) {
                if let Some(res_id) = aliases.get_resource(parts[2]) {
                    if let Some(info) = aliases.get_info(&res_id) {
                        let chunk_size = 32 * 1024;
                        if let Ok(()) = download_mgr.start_download(&info, chunk_size) {
                            if let Some((offset, length)) = download_mgr.get_next_request(&res_id) {
                                let _ = peer.send(&peer_id, &Message::GetChunk { id: res_id.clone(), offset, length });
                                println!("Started downloading {} from {}", parts[2], parts[1]);
                            }
                        } else {
                            println!("Failed to start download");
                        }
                    } else {
                        println!("Resource metadata missing. Try 'list {}' first.", parts[1]);
                    }
                } else {
                    println!("Unknown resource alias: {}", parts[2]);
                }
            } else {
                println!("Unknown peer alias: {}", parts[1]);
            }
        }
        "add" => {
            if parts.len() < 2 {
                println!("Usage: add <file_path>");
                return;
            }
            let path = PathBuf::from(parts[1]);
            match store.add_resource(path) {
                Ok((id, info)) => {
                    let alias = aliases.add_resource(id.clone());
                    println!("Added resource {} -> {:?} (alias: {})", info.name, id, alias);
                }
                Err(e) => {
                    println!("Failed to add resource: {}", e);
                }
            }
        }
        _ => {
            println!("Unknown command. Available: connect, peers, list, get, add");
        }
    }
}

fn handle_message(
    peer: &Peer, 
    store: &mut crate::resource::LocalResourceStore, 
    download_mgr: &mut crate::resource::DownloadManager, 
    aliases: &mut AliasRegistry,
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
            let alias = aliases.add_peer(peer_id.clone());
            println!("[Protocol] Received resources from {}:", alias);
            for res in &resources {
                let r_alias = aliases.add_resource(res.id.clone());
                // We should cache the info here so `get` can use it!
                aliases.cache_info(res.clone());
                println!(" - {} ({} bytes, id: {:?}, alias: {})", res.name, res.size, res.id, r_alias);
            }
        }
        Message::GetChunk { id, offset, length } => {
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
            }
        }
        Message::ResourceChunk { id, offset, data } => {
            match download_mgr.process_chunk(&id, offset, &data) {
                Ok(true) => {
                    if let Ok(()) = download_mgr.complete_download(&id) {
                        println!("[Protocol] Download complete for {:?}", id);
                    }
                }
                Ok(false) => {
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
