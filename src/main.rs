mod listener;
use listener::{ParsipServer, accept};

fn main() -> Result<(), std::io::Error> {
    let address = "0.0.0.0:9000";
    let server = ParsipServer::new(address);
    let listener = server.listen()?;

    println!("Listening on {}", address);
    println!("Waiting for connection...");

    let (_stream, peer_address) = accept(&listener)?;

    println!("Connection accepted from {}", peer_address);

    Ok(())
}
