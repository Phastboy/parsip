use std::net::TcpStream;
use std::io::{BufRead, BufReader, Write, Error, ErrorKind};
use crate::daemon::control::{ControlMessage, ControlResponse};

pub fn run(args: &[String]) -> Result<(), Error> {
    if args.len() < 2 {
        println!("Usage: parsip <command> [args]");
        return Ok(());
    }

    let cmd_str = args[1].as_str();
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
                    println!(" - {} ({} bytes, id: {:?}, alias: {})", r.name, r.size, r.id, r.alias);
                }
            }
        }
        ControlResponse::ResourceAdded { alias, id } => {
            println!("Added resource -> {:?} (alias: {})", id, alias);
        }
    }

    Ok(())
}
