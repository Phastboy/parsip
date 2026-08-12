pub mod control;

use std::fs::{create_dir_all, remove_file, set_permissions, Permissions};
use std::io::{BufRead, BufReader, Error, Write};
use std::net::{Ipv4Addr, SocketAddr};
use std::os::unix::{fs::PermissionsExt, net::UnixListener};
use std::thread;

use crate::config::Config;
use crate::event_handler::{AliasRegistry, handle_event};
use crate::identity::Identity;
use crate::peer::Peer;
use crate::request_tracker::RequestTracker;
use crate::resource::{DownloadManager, LocalResourceStore};
use log::{error, info};

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
    let mut transfer_mgr = crate::resource::TransferManager::new();

    let socket_path = Config::control_socket_path();

    if socket_path.exists() && let Err(e) = remove_file(&socket_path) {
        error!("Failed to remove stale control socket: {}", e);
        return Err(e);
    }

    if let Some(parent) = socket_path.parent() {
        let _ = create_dir_all(parent);
        let _ = set_permissions(parent, Permissions::from_mode(0o700));
    }

    let control_listener = UnixListener::bind(&socket_path)?;
    set_permissions(&socket_path, Permissions::from_mode(0o600))?;

    info!("Control API listening on Unix socket {:?}", socket_path);

    if let Err(e) = crate::discovery::Discovery::start(
        config.listen_port,
        peer.id.clone(),
        config.nickname.clone(),
        peer.event_tx.clone(),
    ) {
        error!("Failed to start local discovery listener: {}", e);
    }

    let tx_clone = peer.event_tx.clone();
    thread::spawn(move || {
        for mut stream in control_listener.incoming().flatten() {
            let tx = tx_clone.clone();
            thread::spawn(move || {
                let reader = BufReader::new(stream.try_clone().unwrap());
                for line in reader.lines().map_while(Result::ok) {
                    if let Ok(cmd) = serde_json::from_str::<control::ControlMessage>(&line) {
                        let (res_tx, res_rx) = std::sync::mpsc::channel();
                        let _ = tx.send(crate::peer::PeerEvent::ControlRequest(cmd, res_tx));
                        while let Ok(resp) = res_rx.recv() {
                            let is_terminal = matches!(
                                resp,
                                control::ControlResponse::Ok
                                    | control::ControlResponse::Error(_)
                                    | control::ControlResponse::ScanResults(_)
                                    | control::ControlResponse::PeersList(_)
                                    | control::ControlResponse::ResourceList(_)
                                    | control::ControlResponse::ResourceAdded { .. }
                                    | control::ControlResponse::DownloadComplete { .. }
                            );
                            if let Ok(resp_json) = serde_json::to_string(&resp)
                                && stream
                                    .write_all(format!("{}\n", resp_json).as_bytes())
                                    .is_err()
                            {
                                break;
                            }
                            if is_terminal {
                                break;
                            }
                        }
                    } else {
                        let _ = stream.write_all(b"{\"Error\":\"Invalid JSON\"}\n");
                    }
                }
            });
        }
    });

    for event in event_rx.iter() {
        let mut ctx = crate::event_handler::DaemonContext {
            config: &config,
            peer: &peer,
            store: &mut resource_store,
            download_mgr: &mut download_mgr,
            transfer_mgr: &mut transfer_mgr,
            aliases: &mut aliases,
            req_tracker: &mut request_tracker,
        };
        handle_event(&mut ctx, event);
    }

    Ok(())
}
