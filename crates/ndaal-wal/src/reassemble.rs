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
}

fn fragment_error(kind: FragmentError) -> WalError {
    WalError::Fragment(kind)
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
    pub fn push<'a>(&mut self, frame: Frame<'a>) -> Result<Option<Record<'a>>, WalError> {
        match (self.pending.take(), frame.first, frame.last) {
            (None, true, true) => Ok(Some(Record {
                kind: frame.kind,
                partition: frame.partition,
                lsn: frame.lsn,
                term: frame.term,
                timestamp: frame.timestamp,
                payload: Cow::Borrowed(frame.payload),
            })),
            (None, true, false) => {
                self.pending = Some(Record {
                    kind: frame.kind,
                    partition: frame.partition,
                    lsn: frame.lsn,
                    term: frame.term,
                    timestamp: frame.timestamp,
                    payload: Cow::Owned(frame.payload.to_vec()),
                });
                Ok(None)
            }
            (None, false, _) | (Some(_), true, _) => Err(fragment_error(FragmentError::Unexpected)),
            (Some(mut record), false, last) => {
                if !same_record(&record, &frame) {
                    return Err(fragment_error(FragmentError::Mismatch));
                }
                record.payload.to_mut().extend_from_slice(frame.payload);
                if last {
                    Ok(Some(record))
                } else {
                    self.pending = Some(record);
                    Ok(None)
                }
            }
        }
    }

    /// Whether a record was started and not finished: at the end of the
    /// log, that record was torn and is dropped.
    pub fn pending(&self) -> bool {
        self.pending.is_some()
    }
}
