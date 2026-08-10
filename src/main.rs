use std::io::Error;
use std::env;
use std::path::PathBuf;
use log::{error, LevelFilter};
use simplelog::{WriteLogger, Config as LogConfig};
use std::fs;
use std::fs::File;

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
        let base = std::env::var("HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("."));
        let parsip_dir = base.join(".parsip");
        let pid_file = parsip_dir.join("parsip.pid");
        let log_file = parsip_dir.join("daemon_out.log");
        let err_file = parsip_dir.join("daemon_err.log");

        let stdout = File::create(log_file).unwrap();
        let stderr = File::create(err_file).unwrap();

        let daemonize = daemonize::Daemonize::new()
            .pid_file(&pid_file)
            .chown_pid_file(true)
            .working_directory(base)
            .stdout(stdout)
            .stderr(stderr);

        match daemonize.start() {
            Ok(_) => {
                println!("Starting parsip daemon in background...");
                if let Err(e) = daemon::run(config) {
                    error!("Daemon crashed: {}", e);
                    std::process::exit(1);
                }
            }
            Err(e) => eprintln!("Error, {}", e),
        }
        return Ok(());
    }

    if args.len() > 1 && args[1] == "stop" {
        let base = std::env::var("HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("."));
        let pid_file = base.join(".parsip").join("parsip.pid");
        
        if let Ok(pid_str) = std::fs::read_to_string(&pid_file) {
            if let Ok(pid) = pid_str.trim().parse::<i32>() {
                // Send SIGTERM to the pid using kill command (libc::kill is better but standard `kill` works)
                let _ = std::process::Command::new("kill").arg(pid.to_string()).status();
                println!("Stopped parsip daemon (PID: {})", pid);
                let _ = std::fs::remove_file(pid_file);
            } else {
                eprintln!("Invalid PID file contents.");
            }
        } else {
            println!("Daemon doesn't seem to be running (no parsip.pid found).");
        }
        return Ok(());
    }

    // Otherwise, route to CLI client
    if let Err(e) = cli::run(&args) {
        eprintln!("CLI Error: {}", e);
    }
    
    Ok(())
}
