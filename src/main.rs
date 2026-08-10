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
mod fs_dir; // Note: 'fs' is a std mod, so I named the module fs_dir
mod event_handler;
mod resource;

use peer::Peer;
use identity::Identity;
use discovery::Discovery;
use fs_dir::{default_shared_dir, default_downloads_dir};
use event_handler::handle_event;
use std::path::PathBuf;

fn main() -> Result<(), Error> {
    let args: Vec<String> = env::args().collect();

    let mut listen_port: u16 = 9000;
    let mut connect_target: Option<SocketAddr> = None;
    let mut shared_dir_path = default_shared_dir();
    let mut downloads_dir_path = default_downloads_dir();

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--shared" => {
                if i + 1 < args.len() {
                    shared_dir_path = PathBuf::from(&args[i + 1]);
                    i += 1;
                }
            }
            "--downloads" => {
                if i + 1 < args.len() {
                    downloads_dir_path = PathBuf::from(&args[i + 1]);
                    i += 1;
                }
            }
            arg if i == 1 => {
                if let Ok(p) = arg.parse::<u16>() {
                    listen_port = p;
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

    let address = SocketAddr::from((Ipv4Addr::UNSPECIFIED, listen_port));
    let identity = Identity::load_or_generate();
    let (peer, event_rx) = Peer::new(identity, address);
    let listener = peer.listen()?;

    let _ = fs::create_dir_all(&shared_dir_path);
    let _ = fs::create_dir_all(&downloads_dir_path);

    println!("Peer listening on {}", peer.address());

    peer.start_accept_loop(listener);

    if let Err(e) = Discovery::start(listen_port, peer.id.clone(), peer.event_tx.clone()) {
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

    let mut resource_store = resource::LocalResourceStore::new(shared_dir_path);
    let mut download_mgr = resource::DownloadManager::new(downloads_dir_path);
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
