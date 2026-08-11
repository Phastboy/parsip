use log::{LevelFilter, error};
use simplelog::{Config as LogConfig, WriteLogger};
use std::env;
use std::fs;
use std::fs::File;
use std::io::Error;
use std::path::PathBuf;

mod cli;
mod config;
mod connection;
mod connection_manager;
mod daemon;
mod discovery;
mod event_handler;
mod identity;
mod peer;
mod protocol;
mod random;
mod request_tracker;
mod resource;

fn main() -> Result<(), Error> {
    let mut config = config::Config::load();
    let args: Vec<String> = env::args().collect();

    if args.len() > 1 && args[1] == "daemon" {
        let cmd = if args.len() > 2 { args[2].as_str() } else { "start" };
        let mut i = 3;
        
        while i < args.len() {
            match args[i].as_str() {
                "--nickname" => {
                    if i + 1 < args.len() {
                        config.nickname = args[i + 1].clone();
                        i += 1;
                    } else {
                        eprintln!("Error: --nickname requires a value");
                        std::process::exit(1);
                    }
                }
                "--shared" => {
                    if i + 1 < args.len() {
                        config.shared_dir = PathBuf::from(&args[i + 1]);
                        i += 1;
                    } else {
                        eprintln!("Error: --shared requires a value");
                        std::process::exit(1);
                    }
                }
                "--downloads" => {
                    if i + 1 < args.len() {
                        config.downloads_dir = PathBuf::from(&args[i + 1]);
                        i += 1;
                    } else {
                        eprintln!("Error: --downloads requires a value");
                        std::process::exit(1);
                    }
                }
                arg => {
                    if let Ok(p) = arg.parse::<u16>() {
                        config.listen_port = p;
                    }
                }
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
        if let Ok(log_file) = File::options()
            .create(true)
            .append(true)
            .open(&config.log_file)
        {
            let _ = WriteLogger::init(LevelFilter::Info, LogConfig::default(), log_file);
        }

        let base = std::env::var("HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("."));
        let parsip_dir = base.join(".parsip");
        let pid_file = parsip_dir.join("parsip.pid");
        let log_file_path = parsip_dir.join("daemon_out.log");
        let err_file_path = parsip_dir.join("daemon_err.log");

        if let Err(e) = fs::create_dir_all(&parsip_dir) {
            eprintln!("Error creating {}: {}", parsip_dir.display(), e);
            std::process::exit(1);
        }

        match cmd {
            "start" => {
                let socket_path = config::Config::control_socket_path();
                if socket_path.exists() {
                    if std::os::unix::net::UnixStream::connect(&socket_path).is_ok() {
                        eprintln!("Daemon is already running.");
                        std::process::exit(1);
                    }
                }

                let stdout = File::create(&log_file_path).unwrap_or_else(|e| {
                    eprintln!("Error creating {}: {}", log_file_path.display(), e);
                    std::process::exit(1);
                });
                let stderr = File::create(&err_file_path).unwrap_or_else(|e| {
                    eprintln!("Error creating {}: {}", err_file_path.display(), e);
                    std::process::exit(1);
                });

                let daemonize = daemonize::Daemonize::new()
                    .pid_file(&pid_file)
                    .chown_pid_file(true)
                    .working_directory(base)
                    .stdout(stdout)
                    .stderr(stderr);

                println!("Starting parsip daemon in background...");
                match daemonize.start() {
                    Ok(_) => {
                        if let Err(e) = daemon::run(config) {
                            error!("Daemon crashed: {}", e);
                            std::process::exit(1);
                        }
                    }
                    Err(e) => eprintln!("Error, {}", e),
                }
            }
            "stop" => {
                if let Ok(pid_str) = std::fs::read_to_string(&pid_file) {
                    if let Ok(pid) = pid_str.trim().parse::<i32>() {
                        let _ = std::process::Command::new("kill")
                            .arg(pid.to_string())
                            .status();
                        println!("Stopped parsip daemon (PID: {})", pid);
                        let _ = std::fs::remove_file(pid_file);
                    } else {
                        eprintln!("Invalid PID file contents.");
                    }
                } else {
                    println!("Daemon doesn't seem to be running (no parsip.pid found).");
                }
            }
            "restart" => {
                if let Ok(pid_str) = std::fs::read_to_string(&pid_file)
                    && let Ok(pid) = pid_str.trim().parse::<i32>()
                {
                    let _ = std::process::Command::new("kill")
                        .arg(pid.to_string())
                        .status();
                    println!("Stopped parsip daemon (PID: {})", pid);
                    let _ = std::fs::remove_file(&pid_file);
                    std::thread::sleep(std::time::Duration::from_millis(500));
                }

                let socket_path = config::Config::control_socket_path();
                if socket_path.exists() {
                    if std::os::unix::net::UnixStream::connect(&socket_path).is_ok() {
                        eprintln!("Daemon is already running.");
                        std::process::exit(1);
                    }
                }

                let stdout = File::create(&log_file_path).unwrap_or_else(|e| {
                    eprintln!("Error creating {}: {}", log_file_path.display(), e);
                    std::process::exit(1);
                });
                let stderr = File::create(&err_file_path).unwrap_or_else(|e| {
                    eprintln!("Error creating {}: {}", err_file_path.display(), e);
                    std::process::exit(1);
                });

                let daemonize = daemonize::Daemonize::new()
                    .pid_file(&pid_file)
                    .chown_pid_file(true)
                    .working_directory(base)
                    .stdout(stdout)
                    .stderr(stderr);

                println!("Starting parsip daemon in background...");
                match daemonize.start() {
                    Ok(_) => {
                        if let Err(e) = daemon::run(config) {
                            error!("Daemon crashed: {}", e);
                            std::process::exit(1);
                        }
                    }
                    Err(e) => eprintln!("Error, {}", e),
                }
            }
            _ => {
                eprintln!("Unknown daemon command: {}", cmd);
                std::process::exit(1);
            }
        }
        return Ok(());
    }

    // Otherwise, route to CLI client
    if let Err(e) = cli::run(&args) {
        eprintln!("CLI Error: {}", e);
    }

    Ok(())
}
