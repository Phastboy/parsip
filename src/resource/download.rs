use std::collections::HashMap;
use std::fs::{self, OpenOptions};
use std::io::{Error, ErrorKind, Write};
use std::path::PathBuf;
use std::sync::mpsc::{self, Sender};
use std::thread::JoinHandle;

use crate::protocol::{ResourceInfo, message::types::ResourceId};

/// A chunk of data or a termination signal for the writer thread.
type WriteCmd = Option<Vec<u8>>;

pub struct ActiveDownload {
    pub info: ResourceInfo,
    /// Bytes received so far (updated immediately when chunk arrives,
    /// before the writer thread has necessarily flushed them to disk).
    pub received_bytes: u64,
    pub temp_path: PathBuf,
    /// Send chunks here — the writer thread owns the file handle.
    writer_tx: Sender<WriteCmd>,
    /// The background thread that owns the BufWriter<File>.
    /// `None` after `complete_download` or `cancel_download` consumes it.
    writer_handle: Option<JoinHandle<Result<(), Error>>>,
}

/// Returned by `complete_download` so the caller can drive the final
/// join + rename **off** the event-loop thread.
pub struct PendingFinalization {
    pub handle: JoinHandle<Result<(), Error>>,
    pub temp_path: PathBuf,
    pub final_path: PathBuf,
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
            return Err(Error::new(
                ErrorKind::AlreadyExists,
                "Download already in progress for this resource",
            ));
        }

        let temp_path = self.downloads_dir.join(format!(".tmp_{:?}", info.id));
        let temp_file = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(&temp_path)?;

        // Unbounded channel: event loop sends chunk data here without ever blocking.
        // The writer thread drains it. In practice (disk >> network throughput)
        // the channel stays near-empty.
        let (tx, rx) = mpsc::channel::<WriteCmd>();

        let handle: JoinHandle<Result<(), Error>> = std::thread::spawn(move || {
            // 256KB write buffer: chunks arrive at 128KB; the buffer smooths two
            // chunks into a single OS write syscall most of the time.
            let mut writer = std::io::BufWriter::with_capacity(256 * 1024, temp_file);

            loop {
                match rx.recv() {
                    Ok(Some(data)) => writer.write_all(&data)?,
                    // Sender sent None (graceful EOF from complete_download)
                    Ok(None) => break,
                    // Sender dropped (cancel_download) — exit without flushing
                    Err(_) => return Ok(()),
                }
            }

            // Graceful shutdown: flush the BufWriter's memory buffer to disk
            writer.flush()?;
            Ok(())
        });

        let download = ActiveDownload {
            info: info.clone(),
            received_bytes: 0,
            temp_path,
            writer_tx: tx,
            writer_handle: Some(handle),
        };

        self.downloads.insert(info.id.clone(), download);
        Ok(())
    }

    /// Forward a chunk to the writer thread. Takes ownership of `data` to
    /// avoid a copy (the Vec is moved directly into the channel).
    /// Returns `Ok(true)` when all expected bytes have been received.
    pub fn process_chunk(&mut self, id: &ResourceId, data: Vec<u8>) -> Result<bool, Error> {
        if let Some(download) = self.downloads.get_mut(id) {
            let len = data.len() as u64;
            if download.writer_tx.send(Some(data)).is_err() {
                return Err(Error::new(
                    ErrorKind::BrokenPipe,
                    "Writer thread exited prematurely",
                ));
            }
            download.received_bytes += len;
            return Ok(download.received_bytes >= download.info.size);
        }
        Err(Error::new(ErrorKind::NotFound, "Download not found"))
    }

    /// Signal the writer thread to flush and exit, then return a
    /// `PendingFinalization` the caller must drive on a background thread.
    ///
    /// **This method never blocks.** The join + rename happen in the
    /// returned handle so the event-loop thread is never stalled.
    pub fn complete_download(&mut self, id: &ResourceId) -> Result<Option<PendingFinalization>, Error> {
        if let Some(mut download) = self.downloads.remove(id) {
            // Signal the writer to flush and exit
            let _ = download.writer_tx.send(None);

            let final_path = self.downloads_dir.join(&download.info.name);
            let pending = download.writer_handle.take().map(|handle| PendingFinalization {
                handle,
                temp_path: download.temp_path,
                final_path,
            });
            Ok(pending)
        } else {
            Err(Error::new(ErrorKind::NotFound, "Download not found"))
        }
    }

    /// Cancel a download mid-flight (e.g. peer disconnected) and delete the temp file.
    /// Does NOT join the writer thread — lets it exit naturally in the background.
    pub fn cancel_download(&mut self, id: &ResourceId) -> Result<(), Error> {
        if let Some(mut download) = self.downloads.remove(id) {
            // Drop the sender to close the channel. The writer thread sees
            // Err on recv() and exits. We do NOT join — it may have pending
            // chunks in its buffer; draining them would waste time on a cancel.
            drop(download.writer_tx);
            // Detach the writer thread (drop the JoinHandle)
            drop(download.writer_handle.take());
            // Best-effort cleanup of the partial temp file
            let _ = fs::remove_file(&download.temp_path);
            Ok(())
        } else {
            Err(Error::new(ErrorKind::NotFound, "Download not found"))
        }
    }
}
