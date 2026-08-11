use std::fs;
use std::path::PathBuf;

use crate::random::random_bytes_32;
use ed25519_dalek::{Signature, Signer, SigningKey};

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

        if path.exists()
            && let Ok(content) = fs::read(&path)
        {
            if content.len() == 32 {
                let mut seed = [0u8; 32];
                seed.copy_from_slice(&content);
                let signing_key = SigningKey::from_bytes(&seed);
                let peer_id = PeerId::from_public_key(&signing_key.verifying_key());
                println!("Loaded existing identity from {:?}", path);
                return Self {
                    signing_key,
                    peer_id,
                };
            }
            eprintln!(
                "Warning: identity file at {:?} is malformed, regenerating",
                path
            );
        }

        let seed = random_bytes_32();
        let signing_key = SigningKey::from_bytes(&seed);
        let peer_id = PeerId::from_public_key(&signing_key.verifying_key());

        if let Some(parent) = path.parent()
            && let Err(e) = fs::create_dir_all(parent)
        {
            eprintln!(
                "Warning: could not create identity directory {:?}: {}",
                parent, e
            );
        }
        if let Err(e) = fs::write(&path, signing_key.to_bytes()) {
            eprintln!("Warning: could not save identity to {:?}: {}", path, e);
        } else {
            println!("Generated and saved new identity at {:?}", path);
        }

        Self {
            signing_key,
            peer_id,
        }
    }

    pub fn public_key_bytes(&self) -> [u8; 32] {
        self.signing_key.verifying_key().to_bytes()
    }

    pub fn sign(&self, message: &[u8]) -> Signature {
        self.signing_key.sign(message)
    }

    /// Generate an in-memory identity without touching the filesystem (useful for tests)
    #[cfg(test)]
    pub fn generate_ephemeral() -> Self {
        let seed = random_bytes_32();
        let signing_key = SigningKey::from_bytes(&seed);
        let peer_id = PeerId::from_public_key(&signing_key.verifying_key());
        Self {
            signing_key,
            peer_id,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ephemeral_identity() {
        let id1 = Identity::generate_ephemeral();
        let id2 = Identity::generate_ephemeral();
        assert_ne!(id1.peer_id.0, id2.peer_id.0); // very unlikely to collide
    }

    #[test]
    fn test_signing() {
        use ed25519_dalek::Verifier;
        let id = Identity::generate_ephemeral();
        let msg = b"test message";
        let sig = id.sign(msg);

        let vk = id.signing_key.verifying_key();
        assert!(vk.verify(msg, &sig).is_ok());
    }
}
