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

fn start_daemon(
    config: config::Config,
    parsip_dir: std::path::PathBuf,
    pid_file: std::path::PathBuf,
) {
    let log_file_path = parsip_dir.join("daemon_out.log");
    let err_file_path = parsip_dir.join("daemon_err.log");

    let stdout = fs::File::create(&log_file_path).unwrap_or_else(|e| {
        eprintln!("Error creating {}: {}", log_file_path.display(), e);
        std::process::exit(1);
    });
    let stderr = fs::File::create(&err_file_path).unwrap_or_else(|e| {
        eprintln!("Error creating {}: {}", err_file_path.display(), e);
        std::process::exit(1);
    });

    let daemonize = daemonize::Daemonize::new()
        .pid_file(&pid_file)
        .chown_pid_file(true)
        .working_directory(&parsip_dir)
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
        Err(e) => {
            let err_msg = e.to_string().to_lowercase();
            if err_msg.contains("unable to lock")
                || err_msg.contains("already running")
                || err_msg.contains("lock")
            {
                eprintln!("Daemon is already running.");
            } else {
                eprintln!("Error starting daemon: {}", e);
            }
            std::process::exit(1);
        }
    }
}

fn stop_daemon(pid_file: &std::path::Path) -> bool {
    let pid_str = match std::fs::read_to_string(pid_file) {
        Ok(s) => s,
        Err(_) => {
            println!("Daemon doesn't seem to be running (no parsip.pid found).");
            return true;
        }
    };

    let pid = match pid_str.trim().parse::<i32>() {
        Ok(p) => p,
        Err(_) => {
            eprintln!("Invalid PID file contents. Removing stale PID file.");
            let _ = fs::remove_file(pid_file);
            return true;
        }
    };

    if pid <= 1 {
        eprintln!("Invalid PID {}. Rejecting.", pid);
        return false;
    }

    let exe_path = fs::read_link(format!("/proc/{}/exe", pid)).unwrap_or_default();
    let is_parsip_exe = exe_path.file_name().map(|n| n == "parsip").unwrap_or(false);

    let cmdline = fs::read(format!("/proc/{}/cmdline", pid)).unwrap_or_default();
    let has_daemon_arg = cmdline.windows(7).any(|w| w == b"daemon\0");

    let is_parsip = is_parsip_exe && has_daemon_arg;
    let proc_exists = std::path::Path::new(&format!("/proc/{}", pid)).exists();

    if !is_parsip && proc_exists {
        eprintln!(
            "PID {} does not belong to parsip. Removing stale PID file.",
            pid
        );
        let _ = fs::remove_file(pid_file);
        return true;
    } else if !proc_exists {
        println!("Daemon doesn't seem to be running. Removing stale PID file.");
        let _ = fs::remove_file(pid_file);
        return true;
    }

    if std::process::Command::new("kill")
        .arg(pid.to_string())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
    {
        // bounded wait up to 5 seconds
        let mut exited = false;
        for _ in 0..50 {
            if !std::path::Path::new(&format!("/proc/{}", pid)).exists() {
                exited = true;
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(100));
        }

        if exited {
            println!("Stopped parsip daemon (PID: {})", pid);
            let _ = fs::remove_file(pid_file);
            true
        } else {
            eprintln!(
                "Failed to stop parsip daemon (PID: {}): timed out waiting for exit.",
                pid
            );
            false
        }
    } else {
        eprintln!("Failed to stop parsip daemon (PID: {}).", pid);
        false
    }
}

fn main() -> Result<(), Error> {
    let mut config = config::Config::load();
    let args: Vec<String> = env::args().collect();

    if args.len() > 1 && args[1] == "daemon" {
        let mut cmd = "start";
        let mut i = 2;

        if args.len() > 2 && matches!(args[2].as_str(), "start" | "stop" | "restart") {
            cmd = args[2].as_str();
            i = 3;
        }

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
                    } else {
                        eprintln!("Error: unrecognized argument or invalid port: {}", arg);
                        std::process::exit(1);
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

        let parsip_dir = config::Config::parsip_dir();
        let pid_file = parsip_dir.join("parsip.pid");

        if let Err(e) = fs::create_dir_all(&parsip_dir) {
            eprintln!("Error creating {}: {}", parsip_dir.display(), e);
            std::process::exit(1);
        }

        match cmd {
            "start" => {
                start_daemon(config, parsip_dir, pid_file);
            }
            "stop" => {
                stop_daemon(&pid_file);
            }
            "restart" => {
                if !stop_daemon(&pid_file) {
                    std::process::exit(1);
                }
                start_daemon(config, parsip_dir, pid_file);
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
