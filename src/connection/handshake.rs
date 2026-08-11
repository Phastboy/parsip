use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use std::io::{Error, ErrorKind};


use crate::identity::{Identity, PeerId};
use crate::protocol::{Decoder, Encoder, LengthPrefixCodec, Message};
use crate::random::random_bytes_32;

pub fn perform_handshake(
    stream: &mut std::net::TcpStream,
    identity: &Identity,
) -> Result<PeerId, Error> {
    let mut codec = LengthPrefixCodec;

    let my_nonce = random_bytes_32();
    let mut my_pubkey_bytes = [0u8; 32];
    my_pubkey_bytes.copy_from_slice(&identity.public_key_bytes());

    let hello_msg = Message::Hello {
        public_key: my_pubkey_bytes,
        nonce: my_nonce,
    };
    codec.encode(&(&hello_msg).into(), stream)?;

    let their_hello_frame = match codec.decode(stream)? {
        Some(frame) => frame,
        None => {
            return Err(Error::new(
                ErrorKind::ConnectionAborted,
                "Peer disconnected during handshake",
            ));
        }
    };

    let their_hello = Message::try_from(their_hello_frame)?;
    let (their_pubkey_bytes, their_nonce) = match their_hello {
        Message::Hello { public_key, nonce } => (public_key, nonce),
        _ => return Err(Error::new(ErrorKind::InvalidData, "Expected Hello message")),
    };

    let their_verifying_key = VerifyingKey::from_bytes(&their_pubkey_bytes)
        .map_err(|_| Error::new(ErrorKind::InvalidData, "Invalid public key in Hello"))?;
    let their_id = PeerId::from_public_key(&their_verifying_key);

    let my_signature = identity.sign(&their_nonce);
    let mut sig_bytes = [0u8; 64];
    sig_bytes.copy_from_slice(&my_signature.to_bytes());

    let proof_msg = Message::HelloProof {
        signature: sig_bytes,
    };
    codec.encode(&(&proof_msg).into(), stream)?;

    let their_proof_frame = match codec.decode(stream)? {
        Some(frame) => frame,
        None => {
            return Err(Error::new(
                ErrorKind::ConnectionAborted,
                "Peer disconnected during proof exchange",
            ));
        }
    };

    let their_proof = Message::try_from(their_proof_frame)?;
    let their_signature_bytes = match their_proof {
        Message::HelloProof { signature } => signature,
        _ => {
            return Err(Error::new(
                ErrorKind::InvalidData,
                "Expected HelloProof message",
            ));
        }
    };

    let their_signature = Signature::from_bytes(&their_signature_bytes);

    their_verifying_key
        .verify(&my_nonce, &their_signature)
        .map_err(|_| {
            Error::new(
                ErrorKind::InvalidData,
                "Handshake signature verification failed",
            )
        })?;

    Ok(their_id)
}
