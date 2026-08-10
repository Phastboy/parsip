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
mod daemon;
mod cli;

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

    if args.len() > 1 && args[1] == "daemon" {
        let mut i = 2;
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
                arg if i == 2 => {
                    if let Ok(p) = arg.parse::<u16>() {
                        config.listen_port = p;
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

        println!("Starting parsip daemon...");
        if let Err(e) = daemon::run(config) {
            error!("Daemon crashed: {}", e);
            eprintln!("Daemon crashed: {}", e);
        }
        return Ok(());
    }

    if args.len() > 1 && args[1] == "start" {
        // Phase 7: spawn daemon
        println!("parsip start not implemented yet (Phase 7)");
        return Ok(());
    }

    // Otherwise, route to CLI client
    if let Err(e) = cli::run(&args) {
        eprintln!("CLI Error: {}", e);
    }
    
    Ok(())
}
