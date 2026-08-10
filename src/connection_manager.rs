use std::io::{Error, ErrorKind};
use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::collections::HashMap;

use crate::connection::Connection;
use crate::identity::PeerId;

/// Owns the registry of live connections, keyed by remote PeerId.
/// Responsible only for "which connection belongs to which peer" —
/// it knows nothing about handshakes, sockets, or framing.
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

    /// Reserves a unique id for a connection before it's registered, so the
    /// caller can wire up cleanup (which needs this id) before insertion.
    pub fn reserve_id(&self) -> u64 {
        self.next_conn_id.fetch_add(1, Ordering::SeqCst)
    }

    /// Removes the entry for `peer_id` only if it's still the connection
    /// identified by `conn_id` — prevents a dying old connection from
    /// evicting a newer, live one registered under the same PeerId.
    pub fn remove_if_current(&self, peer_id: &PeerId, conn_id: u64) {
        if let Ok(mut conns) = self.connections.lock() {
            let should_remove = matches!(conns.get(peer_id), Some((id, _)) if *id == conn_id);
            if should_remove {
                conns.remove(peer_id);
                println!("Removed dead connection for {:?}", peer_id);
            }
        }
    }

    /// Registers a connection under `peer_id`/`conn_id`, unless `died_flag`
    /// indicates it already died before reaching this point (in which case
    /// registering it would create an entry nothing will ever clean up).
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
