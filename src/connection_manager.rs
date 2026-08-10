use std::io::{Error, ErrorKind};
use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::collections::HashMap;

use crate::connection::{Connection, Direction};
use crate::identity::PeerId;

#[derive(Clone)]
pub struct ConnectionManager {
    connections: Arc<Mutex<HashMap<PeerId, (u64, Connection, Direction)>>>,
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
            let should_remove = matches!(conns.get(peer_id), Some((id, _, _)) if *id == conn_id);
            if should_remove {
                conns.remove(peer_id);
                println!("Removed dead connection for {:?}", peer_id);
            }
        }
    }

    pub fn insert_deduplicated(
        &self,
        my_id: PeerId,
        their_id: PeerId,
        conn_id: u64,
        connection: Connection,
        direction: Direction,
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

        if let Some((_, _, _old_dir)) = conns.get(&their_id) {
            let is_local_initiator = direction == Direction::Outgoing;
            let local_wins_tie = my_id.to_bytes() > their_id.to_bytes();
            
            let keep_new = if is_local_initiator {
                local_wins_tie
            } else {
                !local_wins_tie
            };

            if keep_new {
                println!("Deduplication: keeping NEW {:?} connection to {:?}", direction, their_id);
                if let Some((_, old_conn, _)) = conns.remove(&their_id) {
                    let _ = old_conn.stream.shutdown(std::net::Shutdown::Both);
                }
                conns.insert(their_id, (conn_id, connection, direction));
                Ok(())
            } else {
                println!("Deduplication: dropping NEW {:?} connection to {:?}", direction, their_id);
                Err(Error::new(ErrorKind::AlreadyExists, "Deduplication tie-breaker dropped connection"))
            }
        } else {
            conns.insert(their_id, (conn_id, connection, direction));
            Ok(())
        }
    }

    pub fn send(&self, peer_id: &PeerId, message: &crate::protocol::Message) -> Result<(), Error> {
        if let Ok(mut conns) = self.connections.lock() {
            if let Some((_, connection, _)) = conns.get_mut(peer_id) {
                let frame: crate::protocol::Frame = message.into();
                return connection.send(&frame);
            }
        }
        Err(Error::new(ErrorKind::NotConnected, "Peer not connected"))
    }

    pub fn is_connected(&self, peer_id: &PeerId) -> bool {
        if let Ok(conns) = self.connections.lock() {
            conns.contains_key(peer_id)
        } else {
            false
        }
    }
}
