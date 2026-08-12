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

/// Internal state protected by a single mutex.
/// Previously split across two `Arc<Mutex<_>>` (entries + peer_index),
/// requiring two sequential lock acquisitions per `send` call and
/// creating a TOCTOU window between them. A single lock eliminates
/// both the extra contention and the window.
struct Inner {
    entries: HashMap<u64, ConnectionEntry>,
    peer_index: HashMap<PeerId, u64>,
}

#[derive(Clone)]
pub struct ConnectionManager {
    inner: Arc<Mutex<Inner>>,
    next_conn_id: Arc<AtomicU64>,
}

impl ConnectionManager {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(Inner {
                entries: HashMap::new(),
                peer_index: HashMap::new(),
            })),
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

        let mut g = self
            .inner
            .lock()
            .map_err(|_| Error::other("connection manager lock poisoned"))?;

        if let Some(&old_conn_id) = g.peer_index.get(&their_id) {
            let old_entry_exists = g.entries.contains_key(&old_conn_id);
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
                    g.entries.remove(&old_conn_id);
                    g.peer_index.insert(their_id.clone(), conn_id);
                    g.entries.insert(conn_id, entry);
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
                g.peer_index.insert(their_id.clone(), conn_id);
                g.entries.insert(conn_id, entry);
                Ok(conn_id)
            }
        } else {
            g.peer_index.insert(their_id.clone(), conn_id);
            g.entries.insert(conn_id, entry);
            Ok(conn_id)
        }
    }

    pub fn reserve_id(&self) -> u64 {
        self.next_conn_id.fetch_add(1, Ordering::SeqCst)
    }

    pub fn remove(&self, conn_id: u64) {
        if let Ok(mut g) = self.inner.lock()
            && let Some(mut entry) = g.entries.remove(&conn_id)
        {
            entry.state = ConnectionState::Closing;
            let peer_id = entry.connection.remote_peer_id.clone();
            if matches!(g.peer_index.get(&peer_id), Some(&id) if id == conn_id) {
                g.peer_index.remove(&peer_id);
            }
        }
    }

    pub fn send(&self, peer_id: &PeerId, message: &crate::protocol::Message) -> Result<(), Error> {
        let g = self
            .inner
            .lock()
            .map_err(|_| Error::other("connection manager lock poisoned"))?;

        let conn_id = match g.peer_index.get(peer_id).copied() {
            Some(id) => id,
            None => return Err(Error::new(ErrorKind::NotConnected, "Peer not connected")),
        };

        let sender = match g.entries.get(&conn_id) {
            Some(entry) => entry.connection.sender.clone(),
            None => return Err(Error::new(ErrorKind::NotConnected, "Peer not connected")),
        };

        // Drop the lock before the potentially-blocking channel send.
        drop(g);

        let frame: crate::protocol::Frame = message.into();
        sender
            .send(frame)
            .map_err(|_| Error::new(ErrorKind::BrokenPipe, "Writer thread closed"))
    }

    pub fn is_connected(&self, peer_id: &PeerId) -> bool {
        self.inner
            .lock()
            .map(|g| g.peer_index.contains_key(peer_id))
            .unwrap_or(false)
    }
}
