use std::collections::HashMap;
use std::path::PathBuf;

use crate::peer::{Peer, PeerEvent};
use crate::protocol::{ResourceInfo, Message, message::types::ResourceId};
use crate::identity::PeerId;

pub struct AliasRegistry {
    pub peer_aliases: HashMap<String, PeerId>,
    resource_aliases: HashMap<String, ResourceId>,
    resource_info: HashMap<ResourceId, ResourceInfo>,
    pub discovered_peers: HashMap<PeerId, crate::daemon::control::DiscoveredPeerInfo>,
    next_peer_id: u32,
    next_resource_id: u32,
}

impl AliasRegistry {
    pub fn new() -> Self {
        Self {
            peer_aliases: HashMap::new(),
            resource_aliases: HashMap::new(),
            resource_info: HashMap::new(),
            discovered_peers: HashMap::new(),
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
    config: &crate::config::Config,
    peer: &Peer, 
    store: &mut crate::resource::LocalResourceStore, 
    download_mgr: &mut crate::resource::DownloadManager, 
    transfer_mgr: &mut crate::resource::TransferManager,
    aliases: &mut AliasRegistry,
    req_tracker: &mut crate::request_tracker::RequestTracker,
    event: PeerEvent
) {
    match event {
        PeerEvent::Discovered(peer_id, addr, nickname) => {
            let alias = aliases.add_peer(peer_id.clone());
            if !aliases.discovered_peers.contains_key(&peer_id) {
                aliases.discovered_peers.insert(peer_id.clone(), crate::daemon::control::DiscoveredPeerInfo {
                    alias: alias.clone(),
                    nickname: nickname.clone(),
                    address: addr,
                });
                if !peer.is_connected(&peer_id) {
                    println!("[Discovery] Found peer {:?} at {} (alias: {}, nickname: {})", peer_id, addr, alias, nickname);
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
        PeerEvent::Message(peer_id, msg) => handle_message(peer, store, download_mgr, transfer_mgr, aliases, req_tracker, peer_id, msg),
        PeerEvent::ControlRequest(cmd, sender) => handle_control(config, peer, store, download_mgr, transfer_mgr, aliases, req_tracker, cmd, sender),
        PeerEvent::ScanTimeout(req_id) => {
            if let Some(sender) = req_tracker.complete(req_id) {
                let mut results = Vec::new();
                for (_, info) in &aliases.discovered_peers {
                    results.push(info.clone());
                }
                let _ = sender.send(ControlResponse::ScanResults(results));
            }
        }
    }
}

use std::sync::mpsc::Sender;
use crate::daemon::control::{ControlMessage, ControlResponse, ConnectedPeerInfo, ResourceInfo as CtrlResourceInfo};

fn handle_control(
    config: &crate::config::Config,
    peer: &Peer, 
    store: &mut crate::resource::LocalResourceStore, 
    download_mgr: &mut crate::resource::DownloadManager, 
    transfer_mgr: &mut crate::resource::TransferManager,
    aliases: &mut AliasRegistry,
    req_tracker: &mut crate::request_tracker::RequestTracker,
    cmd: ControlMessage,
    sender: Sender<ControlResponse>
) {
    match cmd {
        ControlMessage::Scan => {
            aliases.discovered_peers.clear();
            let req_id = req_tracker.next_id();
            req_tracker.register(req_id, sender);
            
            if let Err(e) = crate::discovery::Discovery::broadcast_scan(config.listen_port, &peer.id, &config.nickname) {
                if let Some(s) = req_tracker.complete(req_id) {
                    let _ = s.send(ControlResponse::Error(format!("Failed to broadcast: {}", e)));
                }
                return;
            }

            let event_tx = peer.event_tx.clone();
            std::thread::spawn(move || {
                std::thread::sleep(std::time::Duration::from_millis(500));
                let _ = event_tx.send(PeerEvent::ScanTimeout(req_id));
            });
        }
        ControlMessage::Connect { alias } => {
            // Find peer by alias in discovered_peers
            if let Some(info) = aliases.discovered_peers.values().find(|info| info.alias == alias) {
                let addr = info.address;
                let _ = peer.connect(addr);
                let _ = sender.send(ControlResponse::Ok);
            } else {
                let _ = sender.send(ControlResponse::Error(format!("Unknown peer alias: {}", alias)));
            }
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
                        if let Ok(()) = download_mgr.start_download(&info) {
                            let request_id = req_tracker.next_id();
                            req_tracker.register(request_id, sender);
                            transfer_mgr.register(crate::resource::Transfer {
                                request_id,
                                peer_id: peer_id.clone(),
                                resource_id: res_id.clone(),
                                is_download: true,
                                bytes_transferred: 0,
                                bytes_total: info.size,
                                start_time: std::time::Instant::now(),
                                last_report_time: std::time::Instant::now(),
                                last_report_bytes: 0,
                            });
                            let _ = peer.send(&peer_id, &Message::DownloadResource { request_id, id: res_id.clone() });
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
                Ok((id, _info)) => {
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
    transfer_mgr: &mut crate::resource::TransferManager,
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
        Message::DownloadResource { request_id, id } => {
            if let Some(file_path) = store.get_path(&id) {
                let peer_clone = peer.clone();
                let peer_id_clone = peer_id.clone();
                let id_clone = id.clone();
                std::thread::spawn(move || {
                    use std::io::Read;
                    if let Ok(mut file) = std::fs::File::open(file_path) {
                        let mut buffer = vec![0u8; 128 * 1024];
                        let mut offset = 0u64;
                        loop {
                            match file.read(&mut buffer) {
                                Ok(0) => break, // EOF
                                Ok(bytes_read) => {
                                    let data = buffer[..bytes_read].to_vec();
                                    let chunk_msg = Message::ResourceChunk {
                                        request_id,
                                        id: id_clone.clone(),
                                        offset,
                                        data,
                                    };
                                    if peer_clone.send(&peer_id_clone, &chunk_msg).is_err() {
                                        eprintln!("[Protocol] Connection lost during file stream");
                                        return;
                                    }
                                    offset += bytes_read as u64;
                                }
                                Err(e) => {
                                    eprintln!("[Protocol] Error reading file: {}", e);
                                    return;
                                }
                            }
                        }
                        // Send End
                        let _ = peer_clone.send(&peer_id_clone, &Message::ResourceEnd { request_id, id: id_clone });
                    }
                });
            }
        }
        Message::ResourceChunk { request_id, id, offset, data } => {
            if let Ok(_) = download_mgr.process_chunk(&id, offset, &data) {
                if let Some((bytes, total, mbps)) = transfer_mgr.update_progress(request_id, data.len() as u64) {
                    if let Some(sender) = req_tracker.get(request_id) {
                        let _ = sender.send(ControlResponse::DownloadProgress { bytes, total, mbps });
                    }
                }
            }
        }
        Message::ResourceEnd { request_id, id } => {
            if let Ok(()) = download_mgr.complete_download(&id) {
                if let Some(t) = transfer_mgr.complete(request_id) {
                    let elapsed_secs = t.start_time.elapsed().as_secs_f64();
                    println!("[Protocol] Download complete for {:?} ({} bytes)", id, t.bytes_transferred);
                    
                    if let Some(sender) = req_tracker.complete(request_id) {
                        let _ = sender.send(ControlResponse::DownloadComplete { 
                            bytes: t.bytes_transferred, 
                            elapsed_secs 
                        });
                    }
                } else {
                    println!("[Protocol] Download complete for {:?}", id);
                }
            }
        }
        _ => {}
    }
}
