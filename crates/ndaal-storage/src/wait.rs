//! The durable LSN of a partition and waiting for an LSN (EF-TX-09).

use std::pin::pin;
use std::time::Duration;

use ndaal_common::ids::Lsn;
use ndaal_io::{Clock, Disk};

use crate::{Apply, Partition, PartitionError};

impl<D: Disk, C: Clock, E: Apply> Partition<D, C, E>
where
    D::File: 'static,
{
    /// Last LSN known durable: every logged record up to it is on disk.
    /// Records of mode `None` are not logged and do not hold it back; it
    /// never passes the applied LSN and never goes back.
    pub fn durable_lsn(&self) -> Option<Lsn> {
        let on_disk = self.writer.durable_position();
        let mut st = self.state.borrow_mut();
        while let (Some(&(_, position)), Some(on_disk)) = (st.logged.front(), on_disk) {
            if position > on_disk {
                break;
            }
            st.logged.pop_front();
        }
        let durable = match st.logged.front() {
            // Everything up to the first record not on disk, exclusive.
            Some(&(lsn, _)) => lsn.0.checked_sub(1).filter(|&l| l > 0).map(Lsn),
            None => st.applied,
        };
        let durable = durable.max(st.durable);
        if durable != st.durable {
            st.durable = durable;
            if let Some(lsn) = durable {
                self.metrics.durable(lsn);
            }
        }
        durable
    }

    /// Completes once `lsn` is applied, even if it was not given yet;
    /// `Timeout` after `timeout`, `Failed` if the partition fails meanwhile.
    pub async fn wait_applied(&self, lsn: Lsn, timeout: Duration) -> Result<(), PartitionError> {
        let deadline = self.clock.monotonic().saturating_add(timeout);
        loop {
            let mut progress = pin!(self.progress.notified());
            progress.as_mut().enable();
            {
                let st = self.state.borrow();
                if st.applied.is_some_and(|a| a >= lsn) {
                    return Ok(());
                }
                if st.failed {
                    return Err(PartitionError::Failed);
                }
            }
            let left = deadline.saturating_sub(self.clock.monotonic());
            if left == Duration::ZERO {
                return Err(PartitionError::Timeout);
            }
            tokio::select! {
                () = &mut progress => {}
                () = self.clock.sleep(left) => {}
            }
        }
    }

    /// Completes once `lsn` is applied and durable; `Timeout` after
    /// `timeout` in all, `Failed` if the partition fails meanwhile.
    pub async fn wait_durable(&self, lsn: Lsn, timeout: Duration) -> Result<(), PartitionError> {
        let deadline = self.clock.monotonic().saturating_add(timeout);
        self.wait_applied(lsn, timeout).await?;
        loop {
            if self.durable_lsn().is_some_and(|d| d >= lsn) {
                return Ok(());
            }
            // The last logged record at or before `lsn`: once it is on
            // disk, so is everything logged before it.
            let position = {
                let st = self.state.borrow();
                st.logged
                    .iter()
                    .take_while(|(l, _)| *l <= lsn)
                    .last()
                    .map(|&(_, p)| p)
            };
            let Some(position) = position else {
                return Ok(());
            };
            let left = deadline.saturating_sub(self.clock.monotonic());
            if left == Duration::ZERO {
                return Err(PartitionError::Timeout);
            }
            tokio::select! {
                result = self.writer.durable(position) => {
                    result.map_err(|_| self.fail())?;
                }
                () = self.clock.sleep(left) => return Err(PartitionError::Timeout),
            }
        }
    }
}
