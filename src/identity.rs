// identity.rs
use std::fs;
use std::io::Read;
use std::path::{PathBuf};

#[derive(Clone, PartialEq, Eq, Hash)]
pub struct PeerId([u8; 32]);

impl PeerId {
    pub fn generate() -> Self {
        let mut bytes = [0u8; 32];
        if let Ok(mut f) = fs::File::open("/dev/urandom") {
            let _ = f.read_exact(&mut bytes);
        } else {
            let time = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
            let time_bytes = time.to_be_bytes();
            bytes[0..16].copy_from_slice(&time_bytes);
        }
        Self(bytes)
    }

    fn identity_path() -> PathBuf {
        // Prefer ~/.parsip/identity; fall back to ./.parsip/identity if HOME is unset.
        let base = std::env::var("HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("."));
        base.join(".parsip").join("identity")
    }

    /// Loads the node's persistent identity, independent of listen port/address.
    pub fn load_or_generate() -> Self {
        let path = Self::identity_path();

        if path.exists() {
            if let Ok(content) = fs::read(&path) {
                if content.len() == 32 {
                    let mut bytes = [0u8; 32];
                    bytes.copy_from_slice(&content);
                    println!("Loaded existing identity from {:?}", path);
                    return Self(bytes);
                }
                eprintln!("Warning: identity file at {:?} is malformed, regenerating", path);
            }
        }

        let new_id = Self::generate();
        if let Some(parent) = path.parent() {
            if let Err(e) = fs::create_dir_all(parent) {
                eprintln!("Warning: could not create identity directory {:?}: {}", parent, e);
            }
        }
        if let Err(e) = fs::write(&path, new_id.0) {
            eprintln!("Warning: could not save identity to {:?}: {}", path, e);
        } else {
            println!("Generated and saved new identity at {:?}", path);
        }
        new_id
    }

    pub fn to_bytes(&self) -> [u8; 32] {
        self.0
    }

    pub fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }
}

impl std::fmt::Debug for PeerId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let hex_str = hex::encode(self.0);
        write!(f, "PeerId({}..{})", &hex_str[..4], &hex_str[60..])
    }
}
