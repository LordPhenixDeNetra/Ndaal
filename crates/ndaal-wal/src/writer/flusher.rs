//! The flush task of a writer: one flush in flight, group commit.

use std::pin::pin;
use std::rc::Rc;
use std::time::Duration;

use ndaal_io::{Clock, Disk, File};

use super::Shared;
use super::files::prepare;
use crate::WalError;

/// The flush task of a [`WalWriter`](super::WalWriter); run it on the
/// shard's core with [`Flusher::run`].
pub struct Flusher<D: Disk, C> {
    shared: Rc<Shared<D, C>>,
}

impl<D: Disk, C: Clock> Flusher<D, C>
where
    D::File: 'static,
{
    pub(super) fn new(shared: Rc<Shared<D, C>>) -> Self {
        Flusher { shared }
    }

    /// Flushes until every writer is dropped and nothing is pending, or
    /// until an I/O error leaves the log `Failed`.
    pub async fn run(self) {
        while self.wait_for_work().await {
            if self.prepare_files().await.is_err() {
                return;
            }
            self.batch().await;
            if self.flush_one().await.is_err() {
                return;
            }
            // The next file, once the current one is half full: the
            // rollover then never waits for a file to be made.
            if self.prepare_files().await.is_err() {
                return;
            }
        }
    }

    /// Waits for pending records; `false` when the flusher should end.
    async fn wait_for_work(&self) -> bool {
        let shared = &self.shared;
        loop {
            let mut work = pin!(shared.work.notified());
            work.as_mut().enable();
            {
                let st = shared.state.borrow();
                if st.failed {
                    return false;
                }
                if st.has_pending() {
                    return true;
                }
            }
            if shared.writers.get() == 0 {
                return false;
            }
            work.await;
        }
    }

    /// Lets more records join the flush. Awaited records follow the group
    /// commit rules; when nobody awaits, records may wait up to the async
    /// flush interval to share one sync.
    async fn batch(&self) {
        let shared = &self.shared;
        let config = shared.config;
        loop {
            let (urgent, appended, pending, first) = {
                let st = shared.state.borrow();
                (
                    st.urgent,
                    st.appended,
                    st.pending_len(),
                    st.first_pending_at,
                )
            };
            let waited = shared
                .clock
                .monotonic()
                .saturating_sub(first.unwrap_or_default());
            if pending as u64 >= config.group_commit_max_bytes || shared.writers.get() == 0 {
                return;
            }
            if urgent {
                if waited >= config.group_commit_max_delay {
                    return;
                }
                tokio::task::yield_now().await;
                // No record joined during the yield: the core idles.
                if shared.state.borrow().appended == appended {
                    return;
                }
                continue;
            }
            let left = config.async_flush_interval.saturating_sub(waited);
            if left == Duration::ZERO {
                return;
            }
            let mut work = pin!(shared.work.notified());
            work.as_mut().enable();
            tokio::select! {
                () = &mut work => {}
                () = shared.clock.sleep(left) => {}
            }
        }
    }

    /// Makes ready on disk every file the state asks for.
    async fn prepare_files(&self) -> Result<(), WalError> {
        let shared = &self.shared;
        loop {
            let Some(spec) = shared.state.borrow_mut().next_to_prepare() else {
                return Ok(());
            };
            let prepared =
                prepare(&shared.disk, &shared.dir, shared.owner, &shared.clock, spec).await;
            match prepared {
                Ok(file) => {
                    shared
                        .files
                        .borrow_mut()
                        .insert(spec.file_no, Rc::new(file));
                    shared.state.borrow_mut().prepared(spec.file_no);
                }
                Err(error) => {
                    if let WalError::Io(kind) = error {
                        shared.metrics.failed("file preparation", kind);
                    }
                    self.fail();
                    return Err(error);
                }
            }
        }
    }

    fn fail(&self) {
        self.shared.state.borrow_mut().failed = true;
        self.shared.progress.notify_waiters();
    }

    async fn flush_one(&self) -> Result<(), WalError> {
        let shared = &self.shared;
        let Some(flush) = shared.state.borrow_mut().take_flush() else {
            return Ok(());
        };
        let Some(file) = shared.files.borrow().get(&flush.file_no).cloned() else {
            self.fail();
            return Err(WalError::Failed);
        };
        let started = shared.clock.monotonic();
        let result = file.write_all_sync_at(&flush.buf, flush.start).await;
        let now = shared.clock.monotonic();
        shared
            .metrics
            .flushed(flush.buf.len(), now.saturating_sub(started));
        if let Err(error) = result {
            // Never retried: after a failed sync the kernel may have dropped
            // the pages; the node restarts from the log.
            shared.metrics.failed("flush", error.kind());
            self.fail();
            return Err(WalError::Failed);
        }
        let oldest = {
            let mut st = shared.state.borrow_mut();
            st.flushed(&flush, now);
            st.take_flush_cleanup();
            st.oldest_file()
        };
        shared.metrics.durable(flush.end);
        // Files fully on disk are closed.
        shared.files.borrow_mut().retain(|&n, _| n >= oldest);
        shared.progress.notify_waiters();
        Ok(())
    }
}
