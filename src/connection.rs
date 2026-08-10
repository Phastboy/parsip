use std::io::{Error, ErrorKind};
use std::net::{SocketAddr, TcpStream};
use std::thread;
use std::time::Duration;

use ed25519_dalek::{Signature, Verifier, VerifyingKey};

use crate::identity::{Identity, PeerId};
use crate::protocol::{Decoder, Encoder, Frame, LengthPrefixCodec, MessageType};
use crate::random::random_bytes_32;

pub struct Connection {
    remote_addr: SocketAddr,
    pub remote_peer_id: Option<PeerId>,
    stream: TcpStream,
}

impl Connection {
    pub fn new(stream: TcpStream, remote_addr: SocketAddr) -> Self {
        let _ = stream.set_read_timeout(Some(Duration::from_secs(30)));
        let _ = stream.set_write_timeout(Some(Duration::from_secs(30)));
        Self { remote_addr, remote_peer_id: None, stream }
    }

    /// Authenticated handshake (see PROTOCOL.md):
    ///
    /// 1. Both sides send Hello { public_key, nonce } and read the peer's Hello.
    /// 2. Both sides sign the nonce THEY RECEIVED and send it as HelloProof.
    /// 3. Both sides verify the incoming HelloProof against the peer's public
    ///    key and the nonce THEY SENT.
    ///
    /// A verification failure is currently treated the same as any other
    /// handshake error (connection closed, logged) — this is a natural
    /// extension point later for a distinguished "security-relevant" event
    /// (e.g. blocklisting), but that policy decision is deliberately out of
    /// scope for now.
    pub fn handshake(&mut self, identity: &Identity) -> Result<PeerId, Error> {
        let mut codec = LengthPrefixCodec;

        // --- Step 1: exchange Hello { public_key(32) || nonce(32) } ---
        let my_nonce = random_bytes_32();

        let mut hello_payload = Vec::with_capacity(64);
        hello_payload.extend_from_slice(&identity.public_key_bytes());
        hello_payload.extend_from_slice(&my_nonce);
        codec.encode(&Frame::new(MessageType::Hello, hello_payload), &mut self.stream)?;

        let their_hello = match codec.decode(&mut self.stream)? {
            Some(frame) if frame.message_type == MessageType::Hello && frame.payload.len() == 64 => frame,
            Some(_) => return Err(Error::new(ErrorKind::InvalidData, "Invalid Hello frame")),
            None => return Err(Error::new(ErrorKind::ConnectionAborted, "Peer disconnected during handshake")),
        };

        let mut their_pubkey_bytes = [0u8; 32];
        their_pubkey_bytes.copy_from_slice(&their_hello.payload[0..32]);
        let mut their_nonce = [0u8; 32];
        their_nonce.copy_from_slice(&their_hello.payload[32..64]);

        let their_verifying_key = VerifyingKey::from_bytes(&their_pubkey_bytes)
            .map_err(|_| Error::new(ErrorKind::InvalidData, "Invalid public key in Hello"))?;
        let their_id = PeerId::from_public_key(&their_verifying_key);

        // --- Step 2: sign the nonce we just received, exchange HelloProof ---
        let my_signature = identity.sign(&their_nonce);
        codec.encode(
            &Frame::new(MessageType::HelloProof, my_signature.to_bytes().to_vec()),
            &mut self.stream,
        )?;

        let their_proof = match codec.decode(&mut self.stream)? {
            Some(frame) if frame.message_type == MessageType::HelloProof && frame.payload.len() == 64 => frame,
            Some(_) => return Err(Error::new(ErrorKind::InvalidData, "Invalid HelloProof frame")),
            None => return Err(Error::new(ErrorKind::ConnectionAborted, "Peer disconnected during proof exchange")),
        };

        let mut sig_bytes = [0u8; 64];
        sig_bytes.copy_from_slice(&their_proof.payload);
        let their_signature = Signature::from_bytes(&sig_bytes);

        // --- Step 3: verify their signature over the nonce WE sent ---
        their_verifying_key
            .verify(&my_nonce, &their_signature)
            .map_err(|_| Error::new(ErrorKind::InvalidData, "Handshake signature verification failed"))?;

        self.remote_peer_id = Some(their_id.clone());
        Ok(their_id)
    }

    pub fn send(&mut self, frame: &Frame) -> Result<(), Error> {
        let mut codec = LengthPrefixCodec;
        codec.encode(frame, &mut self.stream)
    }

    pub fn start_read_loop<F>(&self, event_tx: std::sync::mpsc::Sender<crate::peer::PeerEvent>, peer_id: PeerId, on_disconnect: F) -> Result<(), Error>
    where
        F: FnOnce() + Send + 'static,
    {
        let mut read_stream = self.stream.try_clone()?;
        let peer_addr = self.remote_addr;

        thread::spawn(move || {
            let mut codec = LengthPrefixCodec;

            loop {
                match codec.decode(&mut read_stream) {
                    Ok(Some(frame)) => {
                        if frame.message_type == MessageType::Hello
                            || frame.message_type == MessageType::HelloProof
                        {
                            eprintln!(
                                "Protocol violation from {}: unexpected handshake message in Established phase, closing",
                                peer_addr
                            );
                            break;
                        }

                        // Dispatch the frame!
                        let _ = event_tx.send(crate::peer::PeerEvent::Message(peer_id.clone(), frame));
                    }
                    Ok(None) => {
                        break;
                    }
                    Err(e) if e.kind() == ErrorKind::WouldBlock || e.kind() == ErrorKind::TimedOut => {
                        continue;
                    }
                    Err(e) => {
                        eprintln!("Error reading frame from {}: {}", peer_addr, e);
                        break;
                    }
                }
            }

            let _ = event_tx.send(crate::peer::PeerEvent::Disconnected(peer_id));
            on_disconnect();
        });

        Ok(())
    }
}
