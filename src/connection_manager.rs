use std::io::{Error, ErrorKind};
use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::collections::HashMap;

use crate::connection::Connection;
use crate::identity::PeerId;

#[derive(Clone)]
pub struct ConnectionManager {
    connections: Arc<Mutex<HashMap<PeerId, (u64, Connection)>>>,
    next_conn_id: Arc<AtomicU64>,
}

impl ConnectionManager {
    pub fn new() -> Self {
        Self {
            connections: Arc::new(Mutex::new(HashMap::new())),
            next_conn_id: Arc::new(AtomicU64::new(1)),
        }
    }

    pub fn reserve_id(&self) -> u64 {
        self.next_conn_id.fetch_add(1, Ordering::SeqCst)
    }

    pub fn remove_if_current(&self, peer_id: &PeerId, conn_id: u64) {
        if let Ok(mut conns) = self.connections.lock() {
            let should_remove = matches!(conns.get(peer_id), Some((id, _)) if *id == conn_id);
            if should_remove {
                conns.remove(peer_id);
                println!("Removed dead connection for {:?}", peer_id);
            }
        }
    }

    pub fn insert_if_alive(
        &self,
        peer_id: PeerId,
        conn_id: u64,
        connection: Connection,
        died_flag: &AtomicBool,
    ) -> Result<(), Error> {
        let mut conns = self.connections.lock()
            .map_err(|_| Error::new(ErrorKind::Other, "connections lock poisoned"))?;

        if died_flag.load(Ordering::SeqCst) {
            return Err(Error::new(
                ErrorKind::ConnectionAborted,
                "Connection died before registration completed",
            ));
        }

        if conns.contains_key(&peer_id) {
            println!("Replacing existing connection for {:?}", peer_id);
        }
        conns.insert(peer_id, (conn_id, connection));
        Ok(())
    }
}
