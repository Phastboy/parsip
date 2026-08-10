use std::fs;
use std::path::PathBuf;
use crate::protocol::ResourceInfo;

#[derive(Clone)]
pub struct LocalResourceStore {
    shared_dir: PathBuf,
}

impl LocalResourceStore {
    pub fn new(shared_dir: PathBuf) -> Self {
        Self { shared_dir }
    }

    pub fn list_resources(&self) -> Vec<ResourceInfo> {
        let mut resources = Vec::new();
        if let Ok(entries) = fs::read_dir(&self.shared_dir) {
            for entry in entries.flatten() {
                if let Ok(metadata) = entry.metadata() {
                    if metadata.is_file() {
                        resources.push(ResourceInfo {
                            name: entry.file_name().to_string_lossy().to_string(),
                            size: metadata.len(),
                        });
                    }
                }
            }
        }
        resources
    }
}
