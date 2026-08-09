use std::fs;
use std::io::Read;
use std::path::Path;

#[derive(Clone, PartialEq, Eq, Hash)]
pub struct PeerId([u8; 32]);

impl PeerId {
    pub fn generate() -> Self {
        let mut bytes = [0u8; 32];
        if let Ok(mut f) = fs::File::open("/dev/urandom") {
            let _ = f.read_exact(&mut bytes);
        } else {
            // Fallback to basic random if /dev/urandom fails
            let time = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
            let time_bytes = time.to_be_bytes();
            bytes[0..16].copy_from_slice(&time_bytes);
        }
        Self(bytes)
    }

    pub fn load_or_generate(port: u16) -> Self {
        let filename = format!(".parsip_id_{}", port);
        let path = Path::new(&filename);
        
        if path.exists() {
            if let Ok(content) = fs::read(path) {
                if content.len() == 32 {
                    let mut bytes = [0u8; 32];
                    bytes.copy_from_slice(&content);
                    println!("Loaded existing identity for port {}", port);
                    return Self(bytes);
                }
            }
        }
        
        let new_id = Self::generate();
        if let Err(e) = fs::write(path, new_id.0) {
            eprintln!("Warning: could not save identity: {}", e);
        } else {
            println!("Generated and saved new identity for port {}", port);
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
