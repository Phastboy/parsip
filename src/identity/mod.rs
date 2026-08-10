use std::fs;
use std::path::PathBuf;

use ed25519_dalek::{Signature, Signer, SigningKey};
use crate::random::random_bytes_32;

pub mod peer_id;
pub use peer_id::PeerId;

pub struct Identity {
    signing_key: SigningKey,
    pub peer_id: PeerId,
}

impl Identity {
    fn identity_path() -> PathBuf {
        let base = std::env::var("HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("."));
        base.join(".parsip").join("identity")
    }

    pub fn load_or_generate() -> Self {
        let path = Self::identity_path();

        if path.exists() {
            if let Ok(content) = fs::read(&path) {
                if content.len() == 32 {
                    let mut seed = [0u8; 32];
                    seed.copy_from_slice(&content);
                    let signing_key = SigningKey::from_bytes(&seed);
                    let peer_id = PeerId::from_public_key(&signing_key.verifying_key());
                    println!("Loaded existing identity from {:?}", path);
                    return Self { signing_key, peer_id };
                }
                eprintln!("Warning: identity file at {:?} is malformed, regenerating", path);
            }
        }

        let seed = random_bytes_32();
        let signing_key = SigningKey::from_bytes(&seed);
        let peer_id = PeerId::from_public_key(&signing_key.verifying_key());

        if let Some(parent) = path.parent() {
            if let Err(e) = fs::create_dir_all(parent) {
                eprintln!("Warning: could not create identity directory {:?}: {}", parent, e);
            }
        }
        if let Err(e) = fs::write(&path, signing_key.to_bytes()) {
            eprintln!("Warning: could not save identity to {:?}: {}", path, e);
        } else {
            println!("Generated and saved new identity at {:?}", path);
        }

        Self { signing_key, peer_id }
    }

    pub fn public_key_bytes(&self) -> [u8; 32] {
        self.signing_key.verifying_key().to_bytes()
    }

    pub fn sign(&self, message: &[u8]) -> Signature {
        self.signing_key.sign(message)
    }
}
