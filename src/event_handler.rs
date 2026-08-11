use std::collections::HashMap;
use std::path::PathBuf;

use crate::identity::PeerId;
use crate::peer::{Peer, PeerEvent};
use crate::protocol::{Message, ResourceInfo, message::types::ResourceId};

pub struct AliasRegistry {
    pub peer_aliases: HashMap<String, PeerId>,
    peer_id_to_alias: HashMap<PeerId, String>, // O(1) reverse lookup
    resource_aliases: HashMap<String, ResourceId>,
    resource_id_to_alias: HashMap<ResourceId, String>, // O(1) reverse lookup
    resource_info: HashMap<ResourceId, ResourceInfo>,
    pub discovered_peers: HashMap<PeerId, crate::daemon::control::DiscoveredPeerInfo>,
    next_peer_id: u32,
    next_resource_id: u32,
}

impl AliasRegistry {
    pub fn new() -> Self {
        Self {
            peer_aliases: HashMap::new(),
            peer_id_to_alias: HashMap::new(),
            resource_aliases: HashMap::new(),
            resource_id_to_alias: HashMap::new(),
            resource_info: HashMap::new(),
            discovered_peers: HashMap::new(),
            next_peer_id: 1,
            next_resource_id: 1,
        }
    }

    pub fn add_peer(&mut self, peer_id: PeerId) -> String {
        // O(1) reverse lookup instead of O(n) linear scan
        if let Some(alias) = self.peer_id_to_alias.get(&peer_id) {
            return alias.clone();
        }
        let alias = format!("p{}", self.next_peer_id);
        self.next_peer_id += 1;
        self.peer_id_to_alias.insert(peer_id.clone(), alias.clone());
        self.peer_aliases.insert(alias.clone(), peer_id);
        alias
    }

    pub fn add_resource(&mut self, resource_id: ResourceId) -> String {
        // O(1) reverse lookup instead of O(n) linear scan
        if let Some(alias) = self.resource_id_to_alias.get(&resource_id) {
            return alias.clone();
        }
        let alias = format!("r{}", self.next_resource_id);
        self.next_resource_id += 1;
        self.resource_id_to_alias
            .insert(resource_id.clone(), alias.clone());
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

pub struct DaemonContext<'a> {
    pub config: &'a crate::config::Config,
    pub peer: &'a Peer,
    pub store: &'a mut crate::resource::LocalResourceStore,
    pub download_mgr: &'a mut crate::resource::DownloadManager,
    pub transfer_mgr: &'a mut crate::resource::TransferManager,
    pub aliases: &'a mut AliasRegistry,
    pub req_tracker: &'a mut crate::request_tracker::RequestTracker,
}

pub fn handle_event(ctx: &mut DaemonContext, event: PeerEvent) {
    match event {
        PeerEvent::Discovered(peer_id, addr, nickname) => {
            let alias = ctx.aliases.add_peer(peer_id.clone());
            if !ctx.aliases.discovered_peers.contains_key(&peer_id) {
                ctx.aliases.discovered_peers.insert(
                    peer_id.clone(),
                    crate::daemon::control::DiscoveredPeerInfo {
                        alias: alias.clone(),
                        nickname: nickname.clone(),
                        address: addr,
                    },
                );
                if !ctx.peer.is_connected(&peer_id) {
                    println!(
                        "[Discovery] Found peer {:?} at {} (alias: {}, nickname: {})",
                        peer_id, addr, alias, nickname
                    );
                }
            }
        }
        PeerEvent::NewConnection(peer_id) => {
            let alias = ctx.aliases.add_peer(peer_id.clone());
            println!(
                "[Event] New connection established with {:?} (alias: {})",
                peer_id, alias
            );
        }
        PeerEvent::Disconnected(peer_id) => {
            let alias = ctx
                .aliases
                .peer_aliases
                .iter()
                .find(|(_, id)| *id == &peer_id)
                .map(|(a, _)| a.clone())
                .unwrap_or_else(|| format!("{:?}", peer_id));
            println!("[Event] Peer {} disconnected", alias);

            // Cancel all in-flight transfers for this peer and unblock any waiting CLI
            // commands. Without this, `parsip get` would hang forever after a disconnect.
            let cancelled = ctx.transfer_mgr.cancel_for_peer(&peer_id);
            for t in cancelled {
                // Remove the partial temp file from the download manager
                let _ = ctx.download_mgr.cancel_download(&t.resource_id);
                // Unblock the CLI with an error response
                if let Some(sender) = ctx.req_tracker.complete(t.request_id) {
                    let _ = sender.send(crate::daemon::control::ControlResponse::Error(
                        format!("Transfer failed: peer {} disconnected mid-transfer ({} / {} bytes received)",
                            alias, t.bytes_transferred, t.bytes_total)
                    ));
                }
            }
        }
        PeerEvent::Message(peer_id, msg) => handle_message(ctx, peer_id, msg),
        PeerEvent::ControlRequest(cmd, sender) => handle_control(ctx, cmd, sender),
        PeerEvent::ScanTimeout(req_id) => {
            if let Some(sender) = ctx.req_tracker.complete(req_id) {
                let mut results = Vec::new();
                for info in ctx.aliases.discovered_peers.values() {
                    results.push(info.clone());
                }
                let _ = sender.send(crate::daemon::control::ControlResponse::ScanResults(
                    results,
                ));
            }
        }
        PeerEvent::ConnectResult(req_id, result) => {
            if let Some(sender) = ctx.req_tracker.complete(req_id) {
                match result {
                    Ok(_) => {
                        let _ = sender.send(crate::daemon::control::ControlResponse::Ok);
                    }
                    Err(e) => {
                        let _ = sender.send(crate::daemon::control::ControlResponse::Error(e));
                    }
                }
            }
        }
    }
}

use crate::daemon::control::{
    ConnectedPeerInfo, ControlMessage, ControlResponse, ResourceInfo as CtrlResourceInfo,
};
use std::sync::mpsc::Sender;

fn handle_control(ctx: &mut DaemonContext, cmd: ControlMessage, sender: Sender<ControlResponse>) {
    match cmd {
        ControlMessage::Scan => {
            ctx.aliases.discovered_peers.clear();
            let req_id = ctx.req_tracker.next_id();
            ctx.req_tracker.register(req_id, sender);

            if let Err(e) = crate::discovery::Discovery::broadcast_scan(
                ctx.config.listen_port,
                &ctx.peer.id,
                &ctx.config.nickname,
            ) {
                if let Some(s) = ctx.req_tracker.complete(req_id) {
                    let _ = s.send(ControlResponse::Error(format!(
                        "Failed to broadcast: {}",
                        e
                    )));
                }
                return;
            }

            let event_tx = ctx.peer.event_tx.clone();
            std::thread::spawn(move || {
                std::thread::sleep(std::time::Duration::from_millis(500));
                let _ = event_tx.send(PeerEvent::ScanTimeout(req_id));
            });
        }
        ControlMessage::Connect { alias } => {
            if let Some(info) = ctx
                .aliases
                .discovered_peers
                .values()
                .find(|info| info.alias == alias)
            {
                let addr = info.address;
                let rx = ctx.peer.connect(addr);
                let req_id = ctx.req_tracker.next_id();
                ctx.req_tracker.register(req_id, sender);
                let event_tx = ctx.peer.event_tx.clone();

                std::thread::spawn(move || {
                    let result = match rx.recv() {
                        Ok(Ok(peer_id)) => Ok(peer_id),
                        Ok(Err(e)) => Err(format!("Connection failed: {}", e)),
                        Err(_) => Err("Connection thread panicked".to_string()),
                    };
                    let _ = event_tx.send(PeerEvent::ConnectResult(req_id, result));
                });
            } else {
                let _ = sender.send(ControlResponse::Error(format!(
                    "Unknown peer alias: {}",
                    alias
                )));
            }
        }
        ControlMessage::ListPeers => {
            let mut connected = Vec::new();
            for (alias, id) in &ctx.aliases.peer_aliases {
                if ctx.peer.is_connected(id) {
                    connected.push(ConnectedPeerInfo {
                        alias: alias.clone(),
                        peer_id: id.clone(),
                    });
                }
            }
            let _ = sender.send(ControlResponse::PeersList(connected));
        }
        ControlMessage::ListResources { peer_alias } => {
            if let Some(peer_id) = ctx.aliases.get_peer(&peer_alias) {
                let req_id = ctx.req_tracker.next_id();
                ctx.req_tracker.register(req_id, sender);
                let _ = ctx
                    .peer
                    .send(&peer_id, &Message::ListResources { request_id: req_id });
            } else {
                let _ = sender.send(ControlResponse::Error(format!(
                    "Unknown peer alias: {}",
                    peer_alias
                )));
            }
        }
        ControlMessage::GetResource {
            peer_alias,
            resource_alias,
        } => {
            if let Some(peer_id) = ctx.aliases.get_peer(&peer_alias) {
                if let Some(res_id) = ctx.aliases.get_resource(&resource_alias) {
                    if let Some(info) = ctx.aliases.get_info(&res_id) {
                        if let Ok(()) = ctx.download_mgr.start_download(&info) {
                            let request_id = ctx.req_tracker.next_id();
                            ctx.req_tracker.register(request_id, sender);
                            ctx.transfer_mgr.register(crate::resource::Transfer {
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
                            let _ = ctx.peer.send(
                                &peer_id,
                                &Message::DownloadResource {
                                    request_id,
                                    id: res_id.clone(),
                                },
                            );
                        } else {
                            let _ = sender.send(ControlResponse::Error(
                                "Failed to start download".to_string(),
                            ));
                        }
                    } else {
                        let _ = sender.send(ControlResponse::Error(format!(
                            "Resource metadata missing. Try 'list {}' first.",
                            peer_alias
                        )));
                    }
                } else {
                    let _ = sender.send(ControlResponse::Error(format!(
                        "Unknown resource alias: {}",
                        resource_alias
                    )));
                }
            } else {
                let _ = sender.send(ControlResponse::Error(format!(
                    "Unknown peer alias: {}",
                    peer_alias
                )));
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
            match ctx.store.add_resource(p) {
                Ok((id, _info)) => {
                    let alias = ctx.aliases.add_resource(id.clone());
                    let _ = sender.send(ControlResponse::ResourceAdded { alias, id });
                }
                Err(e) => {
                    let _ = sender.send(ControlResponse::Error(e.to_string()));
                }
            }
        }
    }
}

fn handle_message(ctx: &mut DaemonContext, peer_id: crate::identity::PeerId, msg: Message) {
    match msg {
        Message::ListResources { request_id } => {
            println!(
                "[Protocol] Peer {:?} requested ListResources (req_id: {})",
                peer_id, request_id
            );
            let resources = ctx.store.list_resources();
            let _ = ctx.peer.send(
                &peer_id,
                &Message::ResourceList {
                    request_id,
                    resources,
                },
            );
        }
        Message::ResourceList {
            request_id,
            resources,
        } => {
            let alias = ctx.aliases.add_peer(peer_id.clone());
            println!("[Protocol] Received resources from {}:", alias);

            let mut ctrl_resources = Vec::new();
            for res in &resources {
                let r_alias = ctx.aliases.add_resource(res.id.clone());
                ctx.aliases.cache_info(res.clone());
                ctrl_resources.push(CtrlResourceInfo {
                    alias: r_alias,
                    id: res.id.clone(),
                    name: res.name.clone(),
                    size: res.size,
                });
            }

            if let Some(sender) = ctx.req_tracker.complete(request_id) {
                let _ = sender.send(ControlResponse::ResourceList(ctrl_resources));
            }
        }
        Message::DownloadResource { request_id, id } => {
            if let Some(file_path) = ctx.store.get_path(&id) {
                let peer_clone = ctx.peer.clone();
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
                        let _ = peer_clone.send(
                            &peer_id_clone,
                            &Message::ResourceEnd {
                                request_id,
                                id: id_clone,
                            },
                        );
                    }
                });
            }
        }
        Message::ResourceChunk {
            request_id,
            id,
            data,
            ..
        } => {
            if ctx.download_mgr.process_chunk(&id, &data).is_ok()
                && let Some((bytes, total, mbps)) = ctx
                    .transfer_mgr
                    .update_progress(request_id, data.len() as u64)
                && let Some(sender) = ctx.req_tracker.get(request_id)
            {
                let _ = sender.send(ControlResponse::DownloadProgress { bytes, total, mbps });
            }
        }
        Message::ResourceEnd { request_id, id } => {
            if let Ok(()) = ctx.download_mgr.complete_download(&id) {
                if let Some(t) = ctx.transfer_mgr.complete(request_id) {
                    let elapsed_secs = t.start_time.elapsed().as_secs_f64();
                    println!(
                        "[Protocol] Download complete for {:?} ({} bytes)",
                        id, t.bytes_transferred
                    );

                    if let Some(sender) = ctx.req_tracker.complete(request_id) {
                        let _ = sender.send(ControlResponse::DownloadComplete {
                            bytes: t.bytes_transferred,
                            elapsed_secs,
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
