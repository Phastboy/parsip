use std::io::Error;
use std::env;
use std::thread;
use std::net::{Ipv4Addr, SocketAddr};
use std::fs;

mod protocol;
mod peer;
mod connection;
mod connection_manager;
mod identity;
mod random;
mod discovery;
mod config;
mod event_handler;
mod resource;
mod request_tracker;

use peer::Peer;
use identity::Identity;
use discovery::Discovery;
use event_handler::handle_event;
use std::path::PathBuf;
use log::{info, error, LevelFilter};
use simplelog::{WriteLogger, Config as LogConfig};
use std::fs::File;

fn main() -> Result<(), Error> {
    let mut config = config::Config::load();
    let args: Vec<String> = env::args().collect();

    let mut connect_target: Option<SocketAddr> = None;

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--shared" => {
                if i + 1 < args.len() {
                    config.shared_dir = PathBuf::from(&args[i + 1]);
                    i += 1;
                }
            }
            "--downloads" => {
                if i + 1 < args.len() {
                    config.downloads_dir = PathBuf::from(&args[i + 1]);
                    i += 1;
                }
            }
            arg if i == 1 => {
                if let Ok(p) = arg.parse::<u16>() {
                    config.listen_port = p;
                }
            }
            arg if i == 2 => {
                if let Ok(addr) = arg.parse::<SocketAddr>() {
                    connect_target = Some(addr);
                }
            }
            _ => {}
        }
        i += 1;
    }

    // Ensure directories exist
    let _ = fs::create_dir_all(&config.shared_dir);
    let _ = fs::create_dir_all(&config.downloads_dir);
    if let Some(parent) = config.log_file.parent() {
        let _ = fs::create_dir_all(parent);
    }

    // Initialize File Logger
    if let Ok(log_file) = File::options().create(true).append(true).open(&config.log_file) {
        let _ = WriteLogger::init(LevelFilter::Info, LogConfig::default(), log_file);
    }

    let address = SocketAddr::from((Ipv4Addr::UNSPECIFIED, config.listen_port));
    let identity = Identity::load_or_generate();
    let (peer, event_rx) = Peer::new(identity, address);
    let listener = peer.listen()?;

    println!("Peer listening on {}", peer.address());
    info!("Peer started listening on {}", peer.address());

    peer.start_accept_loop(listener);

    if let Err(e) = Discovery::start(config.listen_port, peer.id.clone(), peer.event_tx.clone()) {
        error!("Failed to start local discovery: {}", e);
        eprintln!("Warning: Failed to start local discovery: {}", e);
    }

    if let Some(target) = connect_target {
        let rx = peer.connect(target);
        thread::spawn(move || {
            match rx.recv() {
                Ok(Ok(peer_id)) => println!("Connected to {:?}", peer_id),
                Ok(Err(e)) => eprintln!("Connection to {} failed: {}", target, e),
                Err(_) => eprintln!("Connection attempt to {} was dropped unexpectedly", target),
            }
        });
    }

    let mut resource_store = resource::LocalResourceStore::new(config.shared_dir.clone());
    let mut download_mgr = resource::DownloadManager::new(config.downloads_dir.clone());
    let mut aliases = event_handler::AliasRegistry::new();

    let tx_clone = peer.event_tx.clone();
    thread::spawn(move || {
        let stdin = std::io::stdin();
        loop {
            let mut line = String::new();
            if stdin.read_line(&mut line).is_ok() {
                let cmd = line.trim().to_string();
                if !cmd.is_empty() {
                    let _ = tx_clone.send(peer::PeerEvent::Command(cmd));
                }
            } else {
                break;
            }
        }
    });

    println!("Starting interactive event loop. Type 'connect', 'peers', 'list', 'get', 'add'...");
    for event in event_rx.iter() {
        handle_event(&peer, &mut resource_store, &mut download_mgr, &mut aliases, event);
    }
    
    Ok(())
}
