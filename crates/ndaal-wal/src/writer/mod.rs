//! The log writer of one shard (T02.08, T02.09, ARCHITECTURE.md section 6.3).
//!
//! It lives on the shard's core (`Rc`, no lock, ADR-003). [`WalWriter::append`]
//! encodes into the pending buffer and never waits; the [`Flusher`], a task
//! on the same core, keeps one flush in flight: one write-and-sync job of
//! the `Critical` class, at most `MAX_FLUSH` bytes, ending on a frame
//! (ADR-033). A record is durable once the sync covering it succeeded.

mod file_buf;
mod files;
mod flusher;
mod metrics;
mod state;

use std::cell::{Cell, RefCell};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::pin::pin;
use std::rc::Rc;
use std::time::Duration;

use ndaal_common::ids::NodeId;
use ndaal_io::{Clock, Disk};
use ndaal_observe::metrics::Registry;
use tokio::sync::Notify;

use files::{Owner, io_error, open_log, release_paths};
pub use flusher::Flusher;
use metrics::Metrics;
pub use state::Position;
use state::State;

use crate::{MAX_FLUSH, Record, WalError};

/// Settings of a writer (from `[wal]` and `[durability]`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WriterConfig {
    /// Bytes reserved for each log file (a multiple of 4 KiB).
    pub file_size: u64,
    /// Pending bytes that start a flush at once.
    pub group_commit_max_bytes: u64,
    /// Longest wait of an awaited record for others to share its flush.
    pub group_commit_max_delay: Duration,
    /// Longest delay before a record nobody awaits is on disk.
    pub async_flush_interval: Duration,
}

/// When a write is acknowledged (ARCHITECTURE.md section 6.4, ENF-DUR-04).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Durability {
    /// Nothing is logged: lost unless a checkpoint covers it.
    None,
    /// Logged and acknowledged at once; on disk within the async flush
    /// interval.
    Async,
    /// Acknowledged once the sync covering it succeeded.
    Sync,
}

/// Pending bytes past which `append` answers busy (P8): two flushes.
const PENDING_LIMIT: usize = 2 * MAX_FLUSH;

/// What the writer and its flusher share.
struct Shared<D: Disk, C> {
    disk: D,
    dir: PathBuf,
    owner: Owner,
    clock: C,
    config: WriterConfig,
    /// Open log files by number: those with data still to flush, and the
    /// next one once prepared.
    files: RefCell<BTreeMap<u64, Rc<D::File>>>,
    state: RefCell<State>,
    /// Wakes the flusher: new records, an awaited record, a writer gone.
    work: Notify,
    /// Wakes those awaiting durability: a flush ended or failed.
    progress: Notify,
    writers: Cell<usize>,
    metrics: Metrics,
}

/// The appending end of a shard's log. Clones share the log; the flusher
/// ends once every clone is dropped and everything is flushed.
pub struct WalWriter<D: Disk, C> {
    shared: Rc<Shared<D, C>>,
}

impl<D: Disk, C: Clock> WalWriter<D, C>
where
    D::File: 'static,
{
    /// Opens the log of `shard` under `dir`: resumes after the last valid
    /// frame, or creates the first file. Returns the writer and its
    /// flusher, for the caller to spawn on the shard's core. A corrupt log
    /// is refused (`WalError::Corrupt`).
    pub async fn open(
        disk: D,
        dir: &Path,
        shard: u16,
        node: NodeId,
        config: WriterConfig,
        clock: C,
        registry: &Registry,
    ) -> Result<(WalWriter<D, C>, Flusher<D, C>), WalError> {
        let dir = dir.join(format!("shard-{shard:02}"));
        let owner = Owner { shard, node };
        let (state, files) = open_log(&disk, &dir, owner, &clock, config.file_size).await?;
        let metrics =
            Metrics::new(shard, registry).map_err(|_| WalError::Io(std::io::ErrorKind::Other))?;
        let shared = Rc::new(Shared {
            disk,
            dir,
            owner,
            clock,
            config,
            files: RefCell::new(files.into_iter().map(|(n, f)| (n, Rc::new(f))).collect()),
            state: RefCell::new(state),
            work: Notify::new(),
            progress: Notify::new(),
            writers: Cell::new(1),
            metrics,
        });
        let flusher = Flusher::new(Rc::clone(&shared));
        Ok((WalWriter { shared }, flusher))
    }

    /// Appends `record`; returns where it ends. Never waits: answers
    /// `Busy` when too much is pending, `Failed` after an I/O error.
    pub fn append(&self, record: &Record<'_>) -> Result<Position, WalError> {
        let shared = &self.shared;
        let mut st = shared.state.borrow_mut();
        if st.failed {
            return Err(WalError::Failed);
        }
        // A record larger than the bound alone is still accepted when nothing
        // else waits: `Busy` means "retry later", which would never succeed.
        let pending = st.pending_len();
        if pending > 0 && pending + crate::encoded_len(record.payload.len()) > PENDING_LIMIT {
            shared.metrics.busy.increment(1);
            return Err(WalError::Busy);
        }
        let at = st.push(record, shared.clock.monotonic());
        drop(st);
        shared.work.notify_one();
        Ok(at)
    }

    /// Completes once the record ending at `at` is durable.
    pub async fn durable(&self, at: Position) -> Result<(), WalError> {
        let shared = &self.shared;
        loop {
            let mut progress = pin!(shared.progress.notified());
            progress.as_mut().enable();
            {
                let mut st = shared.state.borrow_mut();
                if st.durable.is_some_and(|d| d >= at) {
                    return Ok(());
                }
                if st.failed {
                    return Err(WalError::Failed);
                }
                st.urgent = true;
            }
            shared.work.notify_one();
            progress.await;
        }
    }

    /// Logs `record` as `mode` asks and answers when the mode allows:
    /// `None` logs nothing, `Async` answers once appended, `Sync` once
    /// durable. Returns where the record ends, if it was logged.
    pub async fn submit(
        &self,
        record: &Record<'_>,
        mode: Durability,
    ) -> Result<Option<Position>, WalError> {
        if mode == Durability::None {
            return Ok(None);
        }
        let at = self.append(record)?;
        if mode == Durability::Sync {
            self.durable(at).await?;
        }
        Ok(Some(at))
    }

    /// The end of what is durable, if anything was flushed.
    pub fn durable_position(&self) -> Option<Position> {
        self.shared.state.borrow().durable
    }

    /// Releases log file `file_no`, whose records are all covered by
    /// checkpoints (T02.19): it becomes a free file, recycled by a later
    /// rollover. Only files older than every file still in memory qualify.
    pub async fn release(&self, file_no: u64) -> Result<(), WalError> {
        let shared = &self.shared;
        if file_no >= shared.state.borrow().oldest_file() {
            return Err(WalError::Io(std::io::ErrorKind::InvalidInput));
        }
        let (numbered, free) = release_paths(&shared.dir, file_no);
        shared
            .disk
            .rename(&numbered, &free)
            .await
            .map_err(|e| io_error(&e))?;
        shared
            .disk
            .sync_dir(&shared.dir)
            .await
            .map_err(|e| io_error(&e))?;
        if shared.state.borrow_mut().release(file_no) {
            Ok(())
        } else {
            Err(WalError::Io(std::io::ErrorKind::InvalidInput))
        }
    }
}

impl<D: Disk, C> Clone for WalWriter<D, C> {
    fn clone(&self) -> Self {
        self.shared.writers.set(self.shared.writers.get() + 1);
        WalWriter {
            shared: Rc::clone(&self.shared),
        }
    }
}

impl<D: Disk, C> Drop for WalWriter<D, C> {
    fn drop(&mut self) {
        self.shared.writers.set(self.shared.writers.get() - 1);
        self.shared.work.notify_one();
    }
}
