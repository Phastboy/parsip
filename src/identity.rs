use std::fs;
use std::path::PathBuf;

use ed25519_dalek::{Signature, Signer, SigningKey, VerifyingKey};
use sha2::{Digest, Sha256};

use crate::random::random_bytes_32;

#[derive(Clone, PartialEq, Eq, Hash)]
pub struct PeerId([u8; 32]);

impl PeerId {
    /// Derives a PeerId deterministically from a public key: PeerId = SHA-256(pubkey).
    pub fn from_public_key(vk: &VerifyingKey) -> Self {
        let mut hasher = Sha256::new();
        hasher.update(vk.as_bytes());
        let digest = hasher.finalize();
        let mut bytes = [0u8; 32];
        bytes.copy_from_slice(&digest);
        Self(bytes)
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

/// A node's persistent cryptographic identity: a signing keypair plus the
/// PeerId derived from its public key. The private key never leaves this
/// struct; other code only ever sees the PeerId or asks Identity to sign.
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

    /// Loads the node's persistent identity (a 32-byte ed25519 seed) from
    /// disk, or generates and persists a new one. Independent of listen
    /// port/address.
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
