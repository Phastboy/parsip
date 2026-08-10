use std::net::TcpStream;
use std::io::{BufRead, BufReader, Write, Error, ErrorKind};
use crate::daemon::control::{ControlMessage, ControlResponse};

pub fn run(args: &[String]) -> Result<(), Error> {
    if args.len() < 2 {
        println!("Usage: parsip <command> [args]");
        return Ok(());
    }

    let cmd_str = args[1].as_str();

    if cmd_str == "config" {
        if args.len() < 3 {
            println!("Usage: parsip config <show|set> [key] [value]");
            return Ok(());
        }
        let mut config = crate::config::Config::load();
        
        match args[2].as_str() {
            "show" => {
                println!("Nickname: {}", config.nickname);
                println!("Shared Dir: {}", config.shared_dir.display());
                println!("Downloads Dir: {}", config.downloads_dir.display());
                println!("Listen Port: {}", config.listen_port);
                println!("Log File: {}", config.log_file.display());
            }
            "set" => {
                if args.len() < 5 {
                    println!("Usage: parsip config set <key> <value>");
                    return Ok(());
                }
                let key = args[3].as_str();
                let val = args[4].clone();
                match key {
                    "nickname" => config.nickname = val,
                    "shared" | "shared_dir" => config.shared_dir = std::path::PathBuf::from(val),
                    "downloads" | "downloads_dir" => config.downloads_dir = std::path::PathBuf::from(val),
                    "port" | "listen_port" => {
                        if let Ok(p) = val.parse::<u16>() {
                            config.listen_port = p;
                        } else {
                            println!("Invalid port number.");
                            return Ok(());
                        }
                    }
                    _ => {
                        println!("Unknown config key: {}", key);
                        return Ok(());
                    }
                }
                if let Err(e) = config.save() {
                    eprintln!("Failed to save config: {}", e);
                } else {
                    println!("Configuration updated.");
                }
            }
            _ => {
                println!("Usage: parsip config <show|set> [key] [value]");
            }
        }
        return Ok(());
    }
    let msg = match cmd_str {
        "scan" => ControlMessage::Scan,
        "peers" => ControlMessage::ListPeers,
        "connect" => {
            if args.len() < 3 {
                println!("Usage: parsip connect <alias>");
                return Ok(());
            }
            ControlMessage::Connect { alias: args[2].clone() }
        }
        "list" => {
            if args.len() < 3 {
                println!("Usage: parsip list <peer_alias>");
                return Ok(());
            }
            ControlMessage::ListResources { peer_alias: args[2].clone() }
        }
        "get" => {
            if args.len() < 4 {
                println!("Usage: parsip get <peer_alias> <resource_alias>");
                return Ok(());
            }
            ControlMessage::GetResource { 
                peer_alias: args[2].clone(), 
                resource_alias: args[3].clone() 
            }
        }
        "add" => {
            if args.len() < 3 {
                println!("Usage: parsip add <file_path>");
                return Ok(());
            }
            ControlMessage::AddResource { path: args[2].clone() }
        }
        _ => {
            println!("Unknown command: {}", cmd_str);
            return Ok(());
        }
    };

    let mut stream = match TcpStream::connect("127.0.0.1:9091") {
        Ok(s) => s,
        Err(_) => {
            eprintln!("Failed to connect to daemon. Is 'parsip daemon' running?");
            return Ok(());
        }
    };

    let req_json = serde_json::to_string(&msg).unwrap();
    stream.write_all(format!("{}\n", req_json).as_bytes())?;

    let mut reader = BufReader::new(stream);
    let mut resp_line = String::new();
    reader.read_line(&mut resp_line)?;

    if resp_line.is_empty() {
        return Err(Error::new(ErrorKind::ConnectionAborted, "Daemon closed connection"));
    }

    let resp: ControlResponse = serde_json::from_str(&resp_line)
        .map_err(|e| Error::new(ErrorKind::InvalidData, e))?;

    match resp {
        ControlResponse::Ok => println!("Success"),
        ControlResponse::Error(err) => eprintln!("Error: {}", err),
        ControlResponse::ScanResults(results) => {
            if results.is_empty() {
                println!("No peers discovered.");
            } else {
                for r in results {
                    println!("  {}: {} ({})", r.alias, r.nickname, r.address);
                }
            }
        }
        ControlResponse::PeersList(peers) => {
            if peers.is_empty() {
                println!("No connected peers.");
            } else {
                println!("Connected Peers:");
                for p in peers {
                    println!("  {} -> {:?}", p.alias, p.peer_id);
                }
            }
        }
        ControlResponse::ResourceList(resources) => {
            if resources.is_empty() {
                println!("No resources available.");
            } else {
                for r in resources {
                    println!("  {:<4} {:<30} {}", r.alias, r.name, format_size(r.size));
                }
            }
        }
        ControlResponse::ResourceAdded { alias, id } => {
            println!("Added resource -> {:?} (alias: {})", id, alias);
        }
    }

    Ok(())
}

fn format_size(bytes: u64) -> String {
    let kb = 1024_f64;
    let mb = kb * 1024_f64;
    let gb = mb * 1024_f64;

    let b = bytes as f64;
    if b >= gb {
        format!("{:.2} GB", b / gb)
    } else if b >= mb {
        format!("{:.2} MB", b / mb)
    } else if b >= kb {
        format!("{:.2} KB", b / kb)
    } else {
        format!("{} bytes", bytes)
    }
}
