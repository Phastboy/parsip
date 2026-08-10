use std::io::{Error, ErrorKind};
use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicU64, Ordering};
use std::collections::HashMap;

use crate::connection::{Connection, Direction};
use crate::identity::PeerId;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ConnectionState {
    Handshaking,
    Established,
    Closing,
}

pub struct ConnectionEntry {
    pub id: u64,
    pub peer_id: Option<PeerId>,
    pub direction: Direction,
    pub connection: Arc<Mutex<Connection>>,
    pub state: ConnectionState,
}

#[derive(Clone)]
pub struct ConnectionManager {
    entries: Arc<Mutex<HashMap<u64, ConnectionEntry>>>,
    peer_index: Arc<Mutex<HashMap<PeerId, u64>>>,
    next_conn_id: Arc<AtomicU64>,
}

impl ConnectionManager {
    pub fn new() -> Self {
        Self {
            entries: Arc::new(Mutex::new(HashMap::new())),
            peer_index: Arc::new(Mutex::new(HashMap::new())),
            next_conn_id: Arc::new(AtomicU64::new(1)),
        }
    }

    pub fn insert_pending(&self, connection: Arc<Mutex<Connection>>, direction: Direction) -> Result<u64, Error> {
        let conn_id = self.reserve_id();
        let entry = ConnectionEntry {
            id: conn_id,
            peer_id: None,
            direction,
            connection,
            state: ConnectionState::Handshaking,
        };
        
        let mut entries = self.entries.lock()
            .map_err(|_| Error::new(ErrorKind::Other, "entries lock poisoned"))?;
            
        entries.insert(conn_id, entry);
        Ok(conn_id)
    }

    pub fn reserve_id(&self) -> u64 {
        self.next_conn_id.fetch_add(1, Ordering::SeqCst)
    }

    pub fn remove(&self, conn_id: u64) {
        if let Ok(mut entries) = self.entries.lock() {
            if let Some(entry) = entries.remove(&conn_id) {
                if let Some(peer_id) = entry.peer_id {
                    if let Ok(mut index) = self.peer_index.lock() {
                        if matches!(index.get(&peer_id), Some(&id) if id == conn_id) {
                            index.remove(&peer_id);
                        }
                    }
                }
            }
        }
    }

    pub fn promote_to_established(
        &self,
        conn_id: u64,
        my_id: &PeerId,
        their_id: PeerId,
    ) -> Result<(), Error> {
        let mut entries = self.entries.lock()
            .map_err(|_| Error::new(ErrorKind::Other, "entries lock poisoned"))?;
        let mut index = self.peer_index.lock()
            .map_err(|_| Error::new(ErrorKind::Other, "peer_index lock poisoned"))?;

        let (is_local_initiator, entry_direction) = {
            let entry = entries.get(&conn_id).ok_or_else(|| {
                Error::new(ErrorKind::NotFound, "Pending connection not found")
            })?;
            (entry.direction == Direction::Outgoing, entry.direction)
        };

        if let Some(&old_conn_id) = index.get(&their_id) {
            if old_conn_id == conn_id {
                return Ok(());
            }

            let old_entry_exists = entries.contains_key(&old_conn_id);
            if old_entry_exists {
                let local_wins_tie = my_id.to_bytes() > their_id.to_bytes();
                
                let keep_new = if is_local_initiator {
                    local_wins_tie
                } else {
                    !local_wins_tie
                };

                if keep_new {
                    println!("Deduplication: keeping NEW {:?} connection to {:?}", entry_direction, their_id);
                    if let Some(removed_old) = entries.remove(&old_conn_id) {
                        if let Ok(conn) = removed_old.connection.lock() {
                            let _ = conn.stream.shutdown(std::net::Shutdown::Both);
                        }
                    }
                    // Insert new into index
                    index.insert(their_id.clone(), conn_id);
                    
                    if let Some(entry) = entries.get_mut(&conn_id) {
                        entry.peer_id = Some(their_id);
                        entry.state = ConnectionState::Established;
                    }
                    Ok(())
                } else {
                    println!("Deduplication: dropping NEW {:?} connection to {:?}", entry_direction, their_id);
                    entries.remove(&conn_id); // Drop ourselves
                    Err(Error::new(ErrorKind::AlreadyExists, "Deduplication tie-breaker dropped connection"))
                }
            } else {
                // Stale index? Overwrite it.
                index.insert(their_id.clone(), conn_id);
                if let Some(entry) = entries.get_mut(&conn_id) {
                    entry.peer_id = Some(their_id);
                    entry.state = ConnectionState::Established;
                }
                Ok(())
            }
        } else {
            index.insert(their_id.clone(), conn_id);
            if let Some(entry) = entries.get_mut(&conn_id) {
                entry.peer_id = Some(their_id);
                entry.state = ConnectionState::Established;
            }
            Ok(())
        }
    }

    pub fn send(&self, peer_id: &PeerId, message: &crate::protocol::Message) -> Result<(), Error> {
        let conn_id = {
            let index = self.peer_index.lock()
                .map_err(|_| Error::new(ErrorKind::Other, "peer_index lock poisoned"))?;
            index.get(peer_id).copied()
        };

        if let Some(id) = conn_id {
            if let Ok(mut entries) = self.entries.lock() {
                if let Some(entry) = entries.get_mut(&id) {
                    if let Ok(mut conn) = entry.connection.lock() {
                        let frame: crate::protocol::Frame = message.into();
                        return conn.send(&frame);
                    }
                }
            }
        }
        Err(Error::new(ErrorKind::NotConnected, "Peer not connected"))
    }

    pub fn is_connected(&self, peer_id: &PeerId) -> bool {
        if let Ok(index) = self.peer_index.lock() {
            index.contains_key(peer_id)
        } else {
            false
        }
    }
}
