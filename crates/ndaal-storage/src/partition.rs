//! A partition: its LSNs, its engine, its place in the shard's log
//! (ARCHITECTURE.md sections 5, 6 and 10.2, T02.12).

use std::borrow::Cow;
use std::cell::{Ref, RefCell};
use std::collections::VecDeque;

use ndaal_common::ids::{Lsn, PartitionId};
use ndaal_io::{Clock, Disk};
use ndaal_observe::metrics::Registry;
use ndaal_wal::{Durability, Position, Record, RecordType, WalError, WalWriter};
use tokio::sync::Notify;

use crate::metrics::Metrics;
use crate::{Apply, PartitionError};

#[derive(Debug, Default)]
pub(crate) struct State {
    /// Last LSN given to a record.
    pub last: Option<Lsn>,
    /// Last LSN applied to the engine.
    pub applied: Option<Lsn>,
    /// Last LSN known durable.
    pub durable: Option<Lsn>,
    /// Logged records not known durable yet, in LSN order.
    pub logged: VecDeque<(Lsn, Position)>,
    pub failed: bool,
}

/// One partition, owned by its shard's core (not `Send`). It gives each
/// record the next LSN, logs it in the shard's shared log (ADR-004),
/// applies its effects to the engine `E`, and answers as the durability
/// mode asks.
pub struct Partition<D: Disk, C, E> {
    pub(crate) id: PartitionId,
    pub(crate) writer: WalWriter<D, C>,
    engine: RefCell<E>,
    pub(crate) clock: C,
    pub(crate) state: RefCell<State>,
    /// Wakes the waits: an LSN was applied, or the partition failed.
    pub(crate) progress: Notify,
    pub(crate) metrics: Metrics,
}

impl<D: Disk, C: Clock, E: Apply> Partition<D, C, E>
where
    D::File: 'static,
{
    /// A new partition, with no LSN given yet, writing to `writer` (a clone
    /// of its shard's log writer).
    pub fn new(
        id: PartitionId,
        writer: WalWriter<D, C>,
        engine: E,
        clock: C,
        registry: &Registry,
    ) -> Result<Self, PartitionError> {
        let metrics = Metrics::new(id.0, registry).map_err(PartitionError::Metrics)?;
        Ok(Partition {
            id,
            writer,
            engine: RefCell::new(engine),
            clock,
            state: RefCell::new(State::default()),
            progress: Notify::new(),
            metrics,
        })
    }

    /// Logs and applies a record whose effects the caller already computed
    /// and validated, then answers as `mode` asks: `None` and `Async` at
    /// once, `Sync` once the record is durable. A busy log refuses it
    /// before any LSN is given (`Busy`: nothing changed).
    pub async fn submit(
        &self,
        kind: RecordType,
        payload: &[u8],
        mode: Durability,
    ) -> Result<Lsn, PartitionError> {
        let lsn = {
            let st = self.state.borrow();
            if st.failed {
                return Err(PartitionError::Failed);
            }
            match st.last {
                None => Lsn::FIRST,
                Some(last) => match last.checked_next() {
                    Some(next) => next,
                    // The LSN comes from disk through `resume`: never wrap.
                    None => {
                        drop(st);
                        return Err(self.fail());
                    }
                },
            }
        };
        let position = if mode == Durability::None {
            None
        } else {
            let record = Record {
                kind,
                partition: self.id,
                lsn,
                term: 0,
                timestamp: self.clock.unix_micros(),
                payload: Cow::Borrowed(payload),
            };
            match self.writer.append(&record) {
                Ok(position) => Some(position),
                Err(WalError::Busy) => return Err(PartitionError::Busy),
                Err(WalError::Failed) => return Err(self.fail()),
                Err(error) => return Err(PartitionError::Wal(error)),
            }
        };
        {
            let mut st = self.state.borrow_mut();
            st.last = Some(lsn);
            if let Some(position) = position {
                st.logged.push_back((lsn, position));
            }
        }
        if let Err(error) = self.engine.borrow_mut().apply(lsn, kind, payload) {
            // Logged but not applied: never diverge from the log. Logged
            // here, once; the waiters only see `Failed`.
            tracing::error!(partition = self.id.0, lsn = lsn.0, ?kind, %error, "cannot apply a logged record; the partition takes no more writes");
            return Err(self.fail());
        }
        self.state.borrow_mut().applied = Some(lsn);
        self.metrics.applied(lsn);
        self.progress.notify_waiters();
        // No deadline here: a caller that needs one submits `Async`, then
        // waits with `wait_durable`.
        if let (Durability::Sync, Some(position)) = (mode, position) {
            self.writer
                .durable(position)
                .await
                .map_err(|_| self.fail())?;
        }
        // Keeps the durable LSN and its gauge current without a reader.
        self.durable_lsn();
        Ok(lsn)
    }

    /// Continues after `applied`, an LSN already applied and durable (from
    /// a checkpoint or a replay): the next record gets `applied + 1`.
    pub fn resume(&self, applied: Lsn) {
        let mut st = self.state.borrow_mut();
        st.last = Some(applied);
        st.applied = Some(applied);
        st.durable = Some(applied);
        st.logged.clear();
        drop(st);
        self.metrics.applied(applied);
        self.metrics.durable(applied);
        self.progress.notify_waiters();
    }

    /// Stops taking writes; wakes the waits so they end with `Failed`.
    pub(crate) fn fail(&self) -> PartitionError {
        let first = !std::mem::replace(&mut self.state.borrow_mut().failed, true);
        if first {
            self.metrics.failed.increment(1);
        }
        self.progress.notify_waiters();
        PartitionError::Failed
    }

    /// Last LSN given to a record, if any.
    pub fn last_lsn(&self) -> Option<Lsn> {
        self.state.borrow().last
    }

    /// Last LSN applied to the engine, if any.
    pub fn applied_lsn(&self) -> Option<Lsn> {
        self.state.borrow().applied
    }

    /// The partition's identifier.
    pub fn id(&self) -> PartitionId {
        self.id
    }

    /// The partition's clock (waits measure their deadline with it).
    pub fn clock(&self) -> &C {
        &self.clock
    }

    /// The engine, to read from (the protocol front ends).
    pub fn engine(&self) -> Ref<'_, E> {
        self.engine.borrow()
    }
}
