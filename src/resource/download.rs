use std::collections::HashMap;
use std::fs::{File, OpenOptions};
use std::io::{Error, ErrorKind, Seek, SeekFrom, Write};
use std::path::PathBuf;

use crate::protocol::{ResourceInfo, message::types::ResourceId};

pub struct ActiveDownload {
    pub info: ResourceInfo,
    pub received_bytes: u64,
    pub temp_file: File,
    pub temp_path: PathBuf,
}

pub struct DownloadManager {
    downloads: HashMap<ResourceId, ActiveDownload>,
    downloads_dir: PathBuf,
}

impl DownloadManager {
    pub fn new(downloads_dir: PathBuf) -> Self {
        Self {
            downloads: HashMap::new(),
            downloads_dir,
        }
    }

    pub fn start_download(&mut self, info: &ResourceInfo) -> Result<(), Error> {
        let temp_path = self.downloads_dir.join(format!(".tmp_{:?}", info.id));
        let temp_file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(true)
            .open(&temp_path)?;
            
        // Pre-allocate the file size
        temp_file.set_len(info.size)?;

        let download = ActiveDownload {
            info: info.clone(),
            received_bytes: 0,
            temp_file,
            temp_path,
        };

        self.downloads.insert(info.id.clone(), download);
        Ok(())
    }

    pub fn process_chunk(&mut self, id: &ResourceId, offset: u64, data: &[u8]) -> Result<bool, Error> {
        if let Some(download) = self.downloads.get_mut(id) {
            if offset != download.received_bytes {
                // Out of order chunk, for sequential pipelining we reject or ignore it for now
                // but let's just write it if we get it, though we only track received_bytes sequentially
                download.temp_file.seek(SeekFrom::Start(offset))?;
                download.temp_file.write_all(data)?;
                
                // Assuming strict sequential for received_bytes progress
                if offset == download.received_bytes {
                    download.received_bytes += data.len() as u64;
                }
            } else {
                download.temp_file.seek(SeekFrom::Start(offset))?;
                download.temp_file.write_all(data)?;
                download.received_bytes += data.len() as u64;
            }

            return Ok(download.received_bytes >= download.info.size);
        }
        Err(Error::new(ErrorKind::NotFound, "Download not found"))
    }


    pub fn complete_download(&mut self, id: &ResourceId) -> Result<(), Error> {
        if let Some(download) = self.downloads.remove(id) {
            let final_path = self.downloads_dir.join(&download.info.name);
            std::fs::rename(&download.temp_path, &final_path)?;
            Ok(())
        } else {
            Err(Error::new(ErrorKind::NotFound, "Download not found"))
        }
    }
}
