use std::collections::HashMap;
use std::path::PathBuf;

use crate::identity::PeerId;
use crate::peer::{Peer, PeerEvent};
use crate::protocol::Message;

pub struct AliasRegistry {
    pub peer_aliases: HashMap<String, PeerId>,
    peer_id_to_alias: HashMap<PeerId, String>, // O(1) reverse lookup
    pub discovered_peers: HashMap<PeerId, crate::daemon::control::DiscoveredPeerInfo>,
    next_peer_id: u32,
}

impl AliasRegistry {
    pub fn new() -> Self {
        Self {
            peer_aliases: HashMap::new(),
            peer_id_to_alias: HashMap::new(),
            discovered_peers: HashMap::new(),
            next_peer_id: 1,
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

    pub fn get_peer(&self, alias: &str) -> Option<PeerId> {
        self.peer_aliases.get(alias).cloned()
    }
}

pub struct DaemonContext<'a> {
    pub config: &'a crate::config::Config,
    pub peer: &'a Peer,
    pub pending_uploads: &'a mut HashMap<u32, String>,
    pub download_mgr: &'a mut crate::resource::DownloadManager,
    pub transfer_mgr: &'a mut crate::resource::TransferManager,
    pub aliases: &'a mut AliasRegistry,
    pub req_tracker: &'a mut crate::request_tracker::RequestTracker,
}

pub fn handle_event(ctx: &mut DaemonContext, event: PeerEvent) {
    match event {
        PeerEvent::Discovered(peer_id, addr, nickname) => {
            let alias = ctx.aliases.add_peer(peer_id.clone());
            // Always insert/update the latest address and nickname
            ctx.aliases.discovered_peers.insert(
                peer_id.clone(),
                crate::daemon::control::DiscoveredPeerInfo {
                    alias,
                    nickname: nickname.clone(),
                    address: addr,
                },
            );
        }
        PeerEvent::NewConnection(peer_id) => {
            let alias = ctx.aliases.add_peer(peer_id.clone());
            println!(
                "[Event] New connection established with {:?} (alias: {})",
                peer_id, alias
            );
        }
        PeerEvent::Disconnected(peer_id) => {
            println!("[Protocol] Peer {:?} disconnected", peer_id);
            if let Some(alias) = ctx.aliases.peer_id_to_alias.remove(&peer_id) {
                ctx.aliases.peer_aliases.remove(&alias);
            }
            let cancelled = ctx.transfer_mgr.cancel_for_peer(&peer_id);
            for t in cancelled {
                let _ = ctx.download_mgr.cancel_download(t.request_id);
                if let Some(sender) = ctx.req_tracker.complete(t.request_id) {
                    let _ = sender.send(crate::daemon::control::ControlResponse::Error(
                        "Peer disconnected".to_string(),
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
        PeerEvent::UploadComplete {
            request_id,
            bytes,
            elapsed_secs,
            error,
        } => {
            if let Some(sender) = ctx.req_tracker.complete(request_id) {
                if let Some(e) = error {
                    let _ = sender.send(crate::daemon::control::ControlResponse::Error(e));
                } else {
                    let _ =
                        sender.send(crate::daemon::control::ControlResponse::TransferComplete {
                            bytes,
                            elapsed_secs,
                        });
                }
            }
            // Also complete the local transfer state
            ctx.transfer_mgr.complete(request_id);
        }
    }
}

use crate::daemon::control::{ConnectedPeerInfo, ControlMessage, ControlResponse};
use crate::resource::PendingFinalization;
use std::sync::mpsc::Sender;

const SCAN_TIMEOUT_MS: u64 = 500;

/// Joins the disk writer thread, renames the temp file to its final name,
/// and sends the completion (or error) response to the CLI.
/// Must be called on a **background thread** — never on the event loop.
fn finalize_download(pending: PendingFinalization, bytes: u64, elapsed_secs: f64) {
    let mut dest = pending.final_path.clone();
    if dest.exists() {
        let mut i = 1;
        let ext = dest.extension().and_then(|s| s.to_str()).map(|s| format!(".{}", s)).unwrap_or_default();
        let stem = dest.file_stem().and_then(|s| s.to_str()).unwrap_or("file").to_string();
        let parent = dest.parent().unwrap_or(std::path::Path::new("")).to_path_buf();
        
        while dest.exists() {
            dest = parent.join(format!("{} ({}){}", stem, i, ext));
            i += 1;
        }
    }

    match pending.handle.join() {
        Ok(Ok(())) => {
            if let Err(e) = std::fs::rename(&pending.temp_path, &dest) {
                eprintln!("[Protocol] Failed to rename temp file: {}", e);
            } else {
                let mb = bytes as f64 / 1_000_000.0;
                let mb_per_sec = if elapsed_secs > 0.0 {
                    mb / elapsed_secs
                } else {
                    0.0
                };
                println!(
                    "[Protocol] Download completed: {:?} ({:.2} MB/s)",
                    dest, mb_per_sec
                );
            }
        }
        Ok(Err(e)) => eprintln!("[Protocol] Disk write error: {}", e),
        Err(_) => eprintln!("[Protocol] Writer thread panicked"),
    };
}

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
                std::thread::sleep(std::time::Duration::from_millis(SCAN_TIMEOUT_MS));
                let _ = event_tx.send(PeerEvent::ScanTimeout(req_id));
            });
        }
        ControlMessage::Connect { alias } => {
            if let Some(peer_id) = ctx.aliases.get_peer(&alias) {
                if let Some(info) = ctx.aliases.discovered_peers.get(&peer_id) {
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
                        "Peer alias {} found, but address is unknown",
                        alias
                    )));
                }
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
        ControlMessage::SendResource {
            peer_alias,
            file_path,
        } => {
            if let Some(peer_id) = ctx.aliases.get_peer(&peer_alias) {
                // Validate file exists and get size
                let path = PathBuf::from(&file_path);
                let metadata = match std::fs::metadata(&path) {
                    Ok(m) if m.is_file() => m,
                    _ => {
                        let _ = sender.send(ControlResponse::Error(format!(
                            "File not found or is a directory: {}",
                            file_path
                        )));
                        return;
                    }
                };

                let name = path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("unnamed_file")
                    .to_string();
                let size = metadata.len();

                let req_id = ctx.req_tracker.next_id();
                ctx.req_tracker.register(req_id, sender);

                // Track the pending upload
                ctx.pending_uploads.insert(req_id, file_path.clone());

                let _ = ctx.peer.send(
                    &peer_id,
                    &Message::SendResourceRequest {
                        request_id: req_id,
                        name,
                        size,
                    },
                );
            } else {
                let _ = sender.send(ControlResponse::Error(format!(
                    "Unknown peer alias: {}",
                    peer_alias
                )));
            }
        }
    }
}

fn handle_message(ctx: &mut DaemonContext, peer_id: crate::identity::PeerId, msg: Message) {
    match msg {
        Message::SendResourceRequest {
            request_id,
            name,
            size,
        } => {
            if size > 7 * 1024 * 1024 * 1024 {
                eprintln!(
                    "[Protocol] Rejected request {} (size {} exceeds 7GB cap)",
                    request_id, size
                );
                return;
            }
            let alias = ctx.aliases.add_peer(peer_id.clone());
            println!(
                "[Protocol] Peer {} wants to send '{}' (req_id: {})",
                alias, name, request_id
            );

            if let Ok(()) = ctx.download_mgr.start_download(request_id, &name, size) {
                // Register transfer for progress tracking
                ctx.transfer_mgr.register(crate::resource::Transfer {
                    request_id,
                    peer_id: peer_id.clone(),
                    bytes_transferred: 0,
                    bytes_total: size,
                    start_time: std::time::Instant::now(),
                    last_report_time: std::time::Instant::now(),
                    last_report_bytes: 0,
                });

                // Automatically accept the transfer
                let _ = ctx
                    .peer
                    .send(&peer_id, &Message::SendResourceAccept { request_id });
                println!("[Protocol] Accepted transfer {}", request_id);
            } else {
                eprintln!(
                    "[Protocol] Failed to start download for request {}",
                    request_id
                );
            }
        }
        Message::SendResourceAccept { request_id } => {
            println!("[Protocol] Peer accepted transfer {}", request_id);
            if let Some(file_path) = ctx.pending_uploads.remove(&request_id) {
                let peer_clone = ctx.peer.clone();
                let peer_id_clone = peer_id.clone();

                // Track transfer progress for upload
                let path_buf = PathBuf::from(&file_path);
                let size = std::fs::metadata(&path_buf).map(|m| m.len()).unwrap_or(0);

                ctx.transfer_mgr.register(crate::resource::Transfer {
                    request_id,
                    peer_id: peer_id.clone(),
                    bytes_transferred: 0,
                    bytes_total: size,
                    start_time: std::time::Instant::now(),
                    last_report_time: std::time::Instant::now(),
                    last_report_bytes: 0,
                });

                let mut cli_tx = None;
                if let Some(sender) = ctx.req_tracker.get(request_id) {
                    let _ = sender.send(ControlResponse::TransferInitiated);
                    cli_tx = Some(sender.clone());
                }

                let event_tx_clone = ctx.peer.event_tx.clone();
                let start_time = std::time::Instant::now();

                std::thread::spawn(move || {
                    use std::io::Read;
                    if let Ok(mut file) = std::fs::File::open(file_path) {
                        let mut buffer = vec![0u8; 128 * 1024];
                        let mut offset = 0u64;
                        let mut last_report_time = std::time::Instant::now();
                        let mut last_report_bytes = 0u64;
                        
                        loop {
                            match file.read(&mut buffer) {
                                Ok(0) => break, // EOF
                                Ok(bytes_read) => {
                                    let data = buffer[..bytes_read].to_vec();
                                    let chunk_msg = Message::ResourceChunk {
                                        request_id,
                                        offset,
                                        data,
                                    };
                                    if peer_clone.send(&peer_id_clone, &chunk_msg).is_err() {
                                        let _ = event_tx_clone.send(PeerEvent::UploadComplete {
                                            request_id,
                                            bytes: offset,
                                            elapsed_secs: start_time.elapsed().as_secs_f64(),
                                            error: Some(
                                                "Connection lost during file stream".to_string(),
                                            ),
                                        });
                                        return;
                                    }
                                    offset += bytes_read as u64;
                                    
                                    if let Some(tx) = &cli_tx {
                                        let elapsed_ms = last_report_time.elapsed().as_millis();
                                        if elapsed_ms > 200 {
                                            let bytes_since_last = offset - last_report_bytes;
                                            let mbps = (bytes_since_last as f64 / 1_000_000.0) / (elapsed_ms as f64 / 1000.0);
                                            let _ = tx.send(ControlResponse::TransferProgress {
                                                bytes: offset,
                                                total: size,
                                                mbps,
                                            });
                                            last_report_time = std::time::Instant::now();
                                            last_report_bytes = offset;
                                        }
                                    }
                                }
                                Err(e) => {
                                    let _ = event_tx_clone.send(PeerEvent::UploadComplete {
                                        request_id,
                                        bytes: offset,
                                        elapsed_secs: start_time.elapsed().as_secs_f64(),
                                        error: Some(format!("Error reading file: {}", e)),
                                    });
                                    return;
                                }
                            }
                        }
                        if peer_clone
                            .send(&peer_id_clone, &Message::ResourceEnd { request_id })
                            .is_err()
                        {
                            let _ = event_tx_clone.send(PeerEvent::UploadComplete {
                                request_id,
                                bytes: offset,
                                elapsed_secs: start_time.elapsed().as_secs_f64(),
                                error: Some(
                                    "Failed to send ResourceEnd: connection lost".to_string(),
                                ),
                            });
                        } else {
                            let _ = event_tx_clone.send(PeerEvent::UploadComplete {
                                request_id,
                                bytes: offset,
                                elapsed_secs: start_time.elapsed().as_secs_f64(),
                                error: None,
                            });
                        }
                    } else {
                        let _ = event_tx_clone.send(PeerEvent::UploadComplete {
                            request_id,
                            bytes: 0,
                            elapsed_secs: start_time.elapsed().as_secs_f64(),
                            error: Some("Failed to open file for reading".to_string()),
                        });
                    }
                });
            } else {
                eprintln!(
                    "[Protocol] Unknown or already active upload request {}",
                    request_id
                );
            }
        }
        Message::ResourceChunk {
            request_id, data, ..
        } => {
            let data_len = data.len() as u64;
            match ctx.download_mgr.process_chunk(request_id, data) {
                Ok(_) => {
                    let _ = ctx.transfer_mgr.update_progress(request_id, data_len);
                }
                Err(e) => {
                    eprintln!(
                        "[Protocol] Chunk write error for request {}: {}",
                        request_id, e
                    );
                }
            }
        }
        Message::ResourceEnd { request_id } => {
            match ctx.download_mgr.complete_download(request_id) {
                Ok(Some(pending)) => {
                    if let Some(t) = ctx.transfer_mgr.complete(request_id) {
                        let elapsed_secs = t.start_time.elapsed().as_secs_f64();
                        let bytes = t.bytes_transferred;
                        println!(
                            "[Protocol] Finalizing download for request {} ({} bytes)",
                            request_id, bytes
                        );
                        std::thread::spawn(move || {
                            finalize_download(pending, bytes, elapsed_secs);
                        });
                    }
                }
                Err(e) => {
                    eprintln!(
                        "[Protocol] Failed to complete download for request {}: {}",
                        request_id, e
                    );
                }
                Ok(None) => {}
            }
        }
        _ => {}
    }
}
