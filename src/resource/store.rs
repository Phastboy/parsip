use std::fs::{self, File};
use std::io::Read;
use std::path::PathBuf;
use std::collections::HashMap;
use sha2::{Sha256, Digest};
use crate::protocol::{ResourceInfo, message::types::ResourceId};

#[derive(Clone)]
pub struct LocalResourceStore {
    shared_dir: PathBuf,
    index: HashMap<ResourceId, (PathBuf, ResourceInfo)>,
}

impl LocalResourceStore {
    pub fn new(shared_dir: PathBuf) -> Self {
        let mut index = HashMap::new();
        
        if let Ok(entries) = fs::read_dir(&shared_dir) {
            for entry in entries.flatten() {
                if let Ok(metadata) = entry.metadata() {
                    if metadata.is_file() {
                        let path = entry.path();
                        if let Ok(mut file) = File::open(&path) {
                            let mut hasher = Sha256::new();
                            let mut buffer = [0; 8192];
                            while let Ok(count) = file.read(&mut buffer) {
                                if count == 0 { break; }
                                hasher.update(&buffer[..count]);
                            }
                            let hash = hasher.finalize();
                            let mut id_bytes = [0u8; 32];
                            id_bytes.copy_from_slice(&hash);
                            let id = ResourceId(id_bytes);
                            
                            let info = ResourceInfo {
                                id: id.clone(),
                                name: entry.file_name().to_string_lossy().to_string(),
                                size: metadata.len(),
                            };
                            
                            index.insert(id, (path, info));
                        }
                    }
                }
            }
        }
        
        Self { shared_dir, index }
    }

    pub fn list_resources(&self) -> Vec<ResourceInfo> {
        self.index.values().map(|(_, info)| info.clone()).collect()
    }

    pub fn get_path(&self, id: &ResourceId) -> Option<PathBuf> {
        self.index.get(id).map(|(path, _)| path.clone())
    }
}
