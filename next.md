# Prioritized Frame Queue for Connections

Currently, each connection uses an unbounded `mpsc::channel` to send frames to the writer thread. When a peer requests a file download, our upload worker loops infinitely, reading the file from disk and instantly flooding this unbounded channel with the entire file's worth of `ResourceChunk` frames.

Because the channel is purely FIFO, any subsequent control messages (like a `DownloadResource` request initiated by the user for a simultaneous transfer) get stuck at the back of the queue, behind hundreds of megabytes of chunks. This creates severe **Head-Of-Line Blocking** and makes simultaneous transfers appear queued/sequential.

## Proposed Changes

We will replace the `mpsc::channel` with a custom priority queue using `std::sync::Mutex` and `std::sync::Condvar`.

### `src/connection/mod.rs`

#### [MODIFY] [mod.rs](file:///home/tgenericx/dev/github.com/tgenericx/networking/parsip/src/connection/mod.rs)

- Remove `std::sync::mpsc::Sender`.
- Introduce a `ConnectionQueue` containing two `VecDeque<Frame>`: one for control frames and one for data frames.
- Implement a bounded capacity for the data queue (e.g., 16 frames).
- Add a `send` method that blocks (via Condvar) if a data frame is pushed and the data queue is full, but bypasses the bound for control frames.
- Update the writer thread to pull from the control queue first, falling back to the data queue, and notifying the Condvar after a pop to unblock the upload worker.

### `src/connection_manager.rs`

#### [MODIFY] [connection_manager.rs](file:///home/tgenericx/dev/github.com/tgenericx/networking/parsip/src/connection_manager.rs)

- Update `ConnectionManager::send` to use the new priority queue interface instead of `tx.send`.
- Introduce a helper to distinguish `Message::ResourceChunk` (data) from other messages (control).

## Verification Plan

### Manual Verification

- Execute simultaneous bidirectional downloads (PC downloading from Phone, Phone downloading from PC).
- Verify that both transfers proceed concurrently rather than sequentially.
