use std::collections::HashMap;
use std::io::{Error, ErrorKind};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use crate::connection::{Connection, Direction};
use crate::identity::PeerId;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ConnectionState {
    Established,
    Closing,
}

pub struct ConnectionEntry {
    pub connection: Connection,
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

    pub fn insert_connection(&self, connection: Connection, my_id: &PeerId) -> Result<u64, Error> {
        let conn_id = self.reserve_id();
        let their_id = connection.remote_peer_id.clone();
        let entry_direction = connection.direction;
        let is_local_initiator = entry_direction == Direction::Outgoing;

        let entry = ConnectionEntry {
            connection,
            state: ConnectionState::Established,
        };

        let mut entries = self
            .entries
            .lock()
            .map_err(|_| Error::other("entries lock poisoned"))?;
        let mut index = self
            .peer_index
            .lock()
            .map_err(|_| Error::other("peer_index lock poisoned"))?;

        if let Some(&old_conn_id) = index.get(&their_id) {
            let old_entry_exists = entries.contains_key(&old_conn_id);
            if old_entry_exists {
                let local_wins_tie = my_id.to_bytes() > their_id.to_bytes();

                let keep_new = if is_local_initiator {
                    local_wins_tie
                } else {
                    !local_wins_tie
                };

                if keep_new {
                    println!(
                        "Deduplication: keeping NEW {:?} connection to {:?}",
                        entry_direction, their_id
                    );
                    entries.remove(&old_conn_id);
                    // Insert new into index
                    index.insert(their_id.clone(), conn_id);
                    entries.insert(conn_id, entry);
                    Ok(conn_id)
                } else {
                    println!(
                        "Deduplication: dropping NEW {:?} connection to {:?}",
                        entry_direction, their_id
                    );
                    Err(Error::new(
                        ErrorKind::AlreadyExists,
                        "Deduplication tie-breaker dropped connection",
                    ))
                }
            } else {
                index.insert(their_id.clone(), conn_id);
                entries.insert(conn_id, entry);
                Ok(conn_id)
            }
        } else {
            index.insert(their_id.clone(), conn_id);
            entries.insert(conn_id, entry);
            Ok(conn_id)
        }
    }

    pub fn reserve_id(&self) -> u64 {
        self.next_conn_id.fetch_add(1, Ordering::SeqCst)
    }

    pub fn remove(&self, conn_id: u64) {
        if let Ok(mut entries) = self.entries.lock()
            && let Some(mut entry) = entries.remove(&conn_id)
        {
            entry.state = ConnectionState::Closing;
            let peer_id = entry.connection.remote_peer_id.clone();
            if let Ok(mut index) = self.peer_index.lock()
                && matches!(index.get(&peer_id), Some(&id) if id == conn_id)
            {
                index.remove(&peer_id);
            }
        }
    }

    pub fn send(&self, peer_id: &PeerId, message: &crate::protocol::Message) -> Result<(), Error> {
        let conn_id = {
            let index = self
                .peer_index
                .lock()
                .map_err(|_| Error::other("peer_index lock poisoned"))?;
            index.get(peer_id).copied()
        };

        let conn_sender = if let Some(id) = conn_id {
            let mut entries = self
                .entries
                .lock()
                .map_err(|_| Error::other("entries lock poisoned"))?;
            if let Some(entry) = entries.get_mut(&id) {
                Some(entry.connection.sender.clone())
            } else {
                None
            }
        } else {
            None
        };

        if let Some(sender) = conn_sender {
            let frame: crate::protocol::Frame = message.into();
            return sender
                .send(frame)
                .map_err(|_| Error::new(ErrorKind::BrokenPipe, "Writer thread closed"));
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
