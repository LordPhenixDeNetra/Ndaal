//! Gluing fragments back into records (FORMATS_DE_DONNEES.md section 2.3).
//!
//! Fragments of a record are consecutive in the log but may straddle two
//! files: the state is kept from one [`FileReader`](crate::FileReader) to
//! the next.

use std::borrow::Cow;

use crate::{FragmentError, Frame, Record, WalError};

/// Turns frames into records.
#[derive(Debug, Default)]
pub struct Reassembler {
    /// The record being glued: its first fragment, payload copied.
    pending: Option<Record<'static>>,
    /// Records abandoned because a new one started before their end.
    abandoned: u64,
}

fn fragment_error(kind: FragmentError) -> WalError {
    WalError::Fragment(kind)
}

/// The record `frame` starts, with `payload`.
fn record_of<'b>(frame: &Frame<'_>, payload: Cow<'b, [u8]>) -> Record<'b> {
    Record {
        kind: frame.kind,
        partition: frame.partition,
        lsn: frame.lsn,
        term: frame.term,
        timestamp: frame.timestamp,
        payload,
    }
}

/// Whether `frame` is a fragment of `record`.
fn same_record(record: &Record<'_>, frame: &Frame<'_>) -> bool {
    record.kind == frame.kind
        && record.partition == frame.partition
        && record.lsn == frame.lsn
        && record.term == frame.term
        && record.timestamp == frame.timestamp
}

impl Reassembler {
    /// Takes the next frame; returns a record when it completes one. A
    /// whole frame keeps its payload borrowed; a fragmented record is
    /// copied.
    ///
    /// A first fragment while a record is pending abandons that record: the
    /// writer never starts a record before finishing the previous one, so
    /// the pending record was torn by a crash, then the writer resumed. It
    /// was never acknowledged: the sync of its last fragment acknowledges it.
    pub fn push<'a>(&mut self, frame: Frame<'a>) -> Result<Option<Record<'a>>, WalError> {
        if frame.first {
            if self.pending.take().is_some() {
                self.abandoned += 1;
            }
            if frame.last {
                return Ok(Some(record_of(&frame, Cow::Borrowed(frame.payload))));
            }
            self.pending = Some(record_of(&frame, Cow::Owned(frame.payload.to_vec())));
            return Ok(None);
        }
        let Some(mut pending) = self.pending.take() else {
            return Err(fragment_error(FragmentError::Unexpected));
        };
        if !same_record(&pending, &frame) {
            return Err(fragment_error(FragmentError::Mismatch));
        }
        pending.payload.to_mut().extend_from_slice(frame.payload);
        if frame.last {
            return Ok(Some(pending));
        }
        self.pending = Some(pending);
        Ok(None)
    }

    /// Whether a record was started and not finished: at the end of the
    /// log, that record was torn and is dropped.
    pub fn pending(&self) -> bool {
        self.pending.is_some()
    }

    /// Records abandoned so far because a new record started before their
    /// last fragment (torn by a crash).
    pub fn abandoned(&self) -> u64 {
        self.abandoned
    }
}
