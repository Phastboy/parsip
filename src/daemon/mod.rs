pub mod control;

use std::net::{Ipv4Addr, SocketAddr, TcpListener};
use std::io::{Error, BufReader, BufRead, Write};
use std::thread;

use crate::config::Config;
use crate::peer::Peer;
use crate::identity::Identity;
use crate::event_handler::{handle_event, AliasRegistry};
use crate::resource::{LocalResourceStore, DownloadManager};
use crate::request_tracker::RequestTracker;
use log::{info, error};

pub fn run(config: Config) -> Result<(), Error> {
    let address = SocketAddr::from((Ipv4Addr::UNSPECIFIED, config.listen_port));
    let identity = Identity::load_or_generate();
    let (peer, event_rx) = Peer::new(identity, address);
    let listener = peer.listen()?;

    info!("Daemon node listening on {}", peer.address());
    peer.start_accept_loop(listener);

    // Initialize RequestTracker
    let mut request_tracker = RequestTracker::new();

    let mut resource_store = LocalResourceStore::new(config.shared_dir.clone());
    let mut download_mgr = DownloadManager::new(config.downloads_dir.clone());
    let mut aliases = AliasRegistry::new();

    // Start Control TCP Server (127.0.0.1:9091)
    let control_listener = TcpListener::bind("127.0.0.1:9091")?;
    info!("Control API listening on 127.0.0.1:9091");

    if let Err(e) = crate::discovery::Discovery::start(config.listen_port, peer.id.clone(), config.nickname.clone(), peer.event_tx.clone()) {
        error!("Failed to start local discovery listener: {}", e);
    }

    let tx_clone = peer.event_tx.clone();
    thread::spawn(move || {
        for stream in control_listener.incoming() {
            if let Ok(mut stream) = stream {
                let tx = tx_clone.clone();
                thread::spawn(move || {
                    let reader = BufReader::new(stream.try_clone().unwrap());
                    for line in reader.lines() {
                        if let Ok(line) = line {
                            if let Ok(cmd) = serde_json::from_str::<control::ControlMessage>(&line) {
                                let (res_tx, res_rx) = std::sync::mpsc::channel();
                                let _ = tx.send(crate::peer::PeerEvent::ControlRequest(cmd, res_tx));
                                if let Ok(resp) = res_rx.recv() {
                                    if let Ok(resp_json) = serde_json::to_string(&resp) {
                                        let _ = stream.write_all(format!("{}\n", resp_json).as_bytes());
                                    }
                                }
                            } else {
                                let _ = stream.write_all(b"{\"Error\":\"Invalid JSON\"}\n");
                            }
                        }
                    }
                });
            }
        }
    });

    for event in event_rx.iter() {
        handle_event(&config, &peer, &mut resource_store, &mut download_mgr, &mut aliases, &mut request_tracker, event);
    }
    
    Ok(())
}
