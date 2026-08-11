use std::collections::HashMap;
use std::fs::{self, File, OpenOptions};
use std::io::{Error, ErrorKind, Write};
use std::path::PathBuf;

use crate::protocol::{ResourceInfo, message::types::ResourceId};

pub struct ActiveDownload {
    pub info: ResourceInfo,
    pub received_bytes: u64,
    pub temp_file: std::io::BufWriter<File>,
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
        // Prevent duplicate downloads silently overwriting each other
        if self.downloads.contains_key(&info.id) {
            return Err(Error::new(ErrorKind::AlreadyExists, "Download already in progress for this resource"));
        }

        let temp_path = self.downloads_dir.join(format!(".tmp_{:?}", info.id));
        let temp_file = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(&temp_path)?;

        let download = ActiveDownload {
            info: info.clone(),
            received_bytes: 0,
            // BufWriter with 256KB capacity — chunks are written sequentially,
            // no seek() is ever called, so the buffer is always used correctly.
            temp_file: std::io::BufWriter::with_capacity(256 * 1024, temp_file),
            temp_path,
        };

        self.downloads.insert(info.id.clone(), download);
        Ok(())
    }

    /// Process an incoming chunk. The sender is always sequential so we write
    /// directly without any seeking. `received_bytes` is always advanced.
    pub fn process_chunk(&mut self, id: &ResourceId, data: &[u8]) -> Result<bool, Error> {
        if let Some(download) = self.downloads.get_mut(id) {
            download.temp_file.write_all(data)?;
            download.received_bytes += data.len() as u64;
            return Ok(download.received_bytes >= download.info.size);
        }
        Err(Error::new(ErrorKind::NotFound, "Download not found"))
    }

    pub fn complete_download(&mut self, id: &ResourceId) -> Result<(), Error> {
        if let Some(mut download) = self.downloads.remove(id) {
            download.temp_file.flush()?;
            let final_path = self.downloads_dir.join(&download.info.name);
            fs::rename(download.temp_path, final_path)?;
            Ok(())
        } else {
            Err(Error::new(ErrorKind::NotFound, "Download not found"))
        }
    }

    /// Cancel a download mid-flight (e.g. peer disconnected) and delete the temp file.
    pub fn cancel_download(&mut self, id: &ResourceId) -> Result<(), Error> {
        if let Some(download) = self.downloads.remove(id) {
            // Best-effort cleanup; ignore errors (temp file may already be gone)
            let _ = fs::remove_file(&download.temp_path);
            Ok(())
        } else {
            Err(Error::new(ErrorKind::NotFound, "Download not found"))
        }
    }
}
