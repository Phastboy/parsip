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
    discovered_peers: std::collections::HashSet<PeerId>,
    next_peer_id: u32,
    next_resource_id: u32,
}

impl AliasRegistry {
    pub fn new() -> Self {
        Self {
            peer_aliases: HashMap::new(),
            resource_aliases: HashMap::new(),
            resource_info: HashMap::new(),
            discovered_peers: std::collections::HashSet::new(),
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
    req_tracker: &mut crate::request_tracker::RequestTracker,
    event: PeerEvent
) {
    match event {
        PeerEvent::Discovered(peer_id, addr) => {
            if aliases.discovered_peers.insert(peer_id.clone()) {
                if !peer.is_connected(&peer_id) {
                    println!("[Discovery] Found peer {:?} at {} (not auto-connecting)", peer_id, addr);
                }
            }
        }
        PeerEvent::NewConnection(peer_id) => {
            let alias = aliases.add_peer(peer_id.clone());
            println!("[Event] New connection established with {:?} (alias: {})", peer_id, alias);
        }
        PeerEvent::Disconnected(peer_id) => {
            println!("[Event] Peer {:?} disconnected", peer_id);
        }
        PeerEvent::Message(peer_id, msg) => handle_message(peer, store, download_mgr, aliases, req_tracker, peer_id, msg),
        PeerEvent::ControlRequest(cmd, sender) => handle_control(peer, store, download_mgr, aliases, req_tracker, cmd, sender),
    }
}

use std::sync::mpsc::Sender;
use crate::daemon::control::{ControlMessage, ControlResponse, DiscoveredPeerInfo, ConnectedPeerInfo, ResourceInfo as CtrlResourceInfo};

fn handle_control(
    peer: &Peer, 
    store: &mut crate::resource::LocalResourceStore, 
    download_mgr: &mut crate::resource::DownloadManager, 
    aliases: &mut AliasRegistry,
    req_tracker: &mut crate::request_tracker::RequestTracker,
    cmd: ControlMessage,
    sender: Sender<ControlResponse>
) {
    match cmd {
        ControlMessage::Scan => {
            // For now, we return empty until JIT Discovery is implemented in Phase 6
            let _ = sender.send(ControlResponse::ScanResults(vec![]));
        }
        ControlMessage::Connect { alias } => {
            // we don't have a persistent discovery registry yet, so this will fail for now unless we implement JIT discovery.
            let _ = sender.send(ControlResponse::Error("Connect requires JIT discovery (Phase 6)".to_string()));
        }
        ControlMessage::ListPeers => {
            let mut connected = Vec::new();
            for (alias, id) in &aliases.peer_aliases {
                if peer.is_connected(id) {
                    connected.push(ConnectedPeerInfo { alias: alias.clone(), peer_id: id.clone() });
                }
            }
            let _ = sender.send(ControlResponse::PeersList(connected));
        }
        ControlMessage::ListResources { peer_alias } => {
            if let Some(peer_id) = aliases.get_peer(&peer_alias) {
                let req_id = req_tracker.next_id();
                req_tracker.register(req_id, sender);
                let _ = peer.send(&peer_id, &Message::ListResources { request_id: req_id });
            } else {
                let _ = sender.send(ControlResponse::Error(format!("Unknown peer alias: {}", peer_alias)));
            }
        }
        ControlMessage::GetResource { peer_alias, resource_alias } => {
            if let Some(peer_id) = aliases.get_peer(&peer_alias) {
                if let Some(res_id) = aliases.get_resource(&resource_alias) {
                    if let Some(info) = aliases.get_info(&res_id) {
                        let chunk_size = 32 * 1024;
                        if let Ok(()) = download_mgr.start_download(&info, chunk_size) {
                            if let Some((offset, length)) = download_mgr.get_next_request(&res_id) {
                                // RequestTracker for GetResource? 
                                // GetChunk returns a single ResourceChunk. The download manager handles it.
                                // We probably should just return Ok immediately to the CLI, and let background handle it.
                                let _ = peer.send(&peer_id, &Message::GetChunk { request_id: 0, id: res_id.clone(), offset, length });
                                let _ = sender.send(ControlResponse::Ok);
                            } else {
                                let _ = sender.send(ControlResponse::Error("Already downloaded".to_string()));
                            }
                        } else {
                            let _ = sender.send(ControlResponse::Error("Failed to start download".to_string()));
                        }
                    } else {
                        let _ = sender.send(ControlResponse::Error(format!("Resource metadata missing. Try 'list {}' first.", peer_alias)));
                    }
                } else {
                    let _ = sender.send(ControlResponse::Error(format!("Unknown resource alias: {}", resource_alias)));
                }
            } else {
                let _ = sender.send(ControlResponse::Error(format!("Unknown peer alias: {}", peer_alias)));
            }
        }
        ControlMessage::AddResource { path } => {
            let expanded_path = if path.starts_with("~/") {
                let home = std::env::var("HOME").unwrap_or_else(|_| String::from("."));
                path.replacen("~", &home, 1)
            } else {
                path
            };
            let p = PathBuf::from(expanded_path);
            match store.add_resource(p) {
                Ok((id, info)) => {
                    let alias = aliases.add_resource(id.clone());
                    let _ = sender.send(ControlResponse::ResourceAdded { alias, id });
                }
                Err(e) => {
                    let _ = sender.send(ControlResponse::Error(e.to_string()));
                }
            }
        }
    }
}

fn handle_message(
    peer: &Peer, 
    store: &mut crate::resource::LocalResourceStore, 
    download_mgr: &mut crate::resource::DownloadManager, 
    aliases: &mut AliasRegistry,
    req_tracker: &mut crate::request_tracker::RequestTracker,
    peer_id: crate::identity::PeerId, 
    msg: Message
) {
    match msg {
        Message::ListResources { request_id } => {
            println!("[Protocol] Peer {:?} requested ListResources (req_id: {})", peer_id, request_id);
            let resources = store.list_resources();
            let _ = peer.send(&peer_id, &Message::ResourceList { request_id, resources });
        }
        Message::ResourceList { request_id, resources } => {
            let alias = aliases.add_peer(peer_id.clone());
            println!("[Protocol] Received resources from {}:", alias);
            
            let mut ctrl_resources = Vec::new();
            for res in &resources {
                let r_alias = aliases.add_resource(res.id.clone());
                aliases.cache_info(res.clone());
                ctrl_resources.push(CtrlResourceInfo {
                    alias: r_alias,
                    id: res.id.clone(),
                    name: res.name.clone(),
                    size: res.size,
                });
            }

            if let Some(sender) = req_tracker.complete(request_id) {
                let _ = sender.send(ControlResponse::ResourceList(ctrl_resources));
            }
        }
        Message::GetChunk { request_id, id, offset, length } => {
            if let Some(file_path) = store.get_path(&id) {
                use std::io::{Seek, SeekFrom, Read};
                if let Ok(mut file) = std::fs::File::open(file_path) {
                    if file.seek(SeekFrom::Start(offset)).is_ok() {
                        let mut buffer = vec![0u8; length as usize];
                        if let Ok(bytes_read) = file.read(&mut buffer) {
                            buffer.truncate(bytes_read);
                            let _ = peer.send(&peer_id, &Message::ResourceChunk { 
                                request_id,
                                id, 
                                offset, 
                                data: buffer 
                            });
                        }
                    }
                }
            }
        }
        Message::ResourceChunk { request_id, id, offset, data } => {
            match download_mgr.process_chunk(&id, offset, &data) {
                Ok(true) => {
                    if let Ok(()) = download_mgr.complete_download(&id) {
                        println!("[Protocol] Download complete for {:?}", id);
                    }
                }
                Ok(false) => {
                    if let Some((next_offset, length)) = download_mgr.get_next_request(&id) {
                        let _ = peer.send(&peer_id, &Message::GetChunk { request_id, id, offset: next_offset, length });
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
