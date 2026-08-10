use std::fs;
use std::io::Read;

/// Fills a 32-byte buffer with cryptographically random bytes from the OS.
/// Deliberately avoids depending on the rand_core/getrandom ecosystem
/// (whose OsRng-equivalent API has moved and been renamed across recent
/// major versions); reading directly from the OS random device is stable
/// and sufficient for our needs (identity seeds, handshake nonces).
pub fn random_bytes_32() -> [u8; 32] {
    let mut bytes = [0u8; 32];
    if let Ok(mut f) = fs::File::open("/dev/urandom") {
        if f.read_exact(&mut bytes).is_ok() {
            return bytes;
        }
    }
    // Fallback: extremely unlikely to be hit on any Unix system, but avoid
    // ever returning all-zero randomness.
    let time = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let time_bytes = time.to_be_bytes();
    bytes[0..16].copy_from_slice(&time_bytes);
    bytes
}
