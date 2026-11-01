//! The buffer of one log file: what is flushed, in flight and pending.

use std::collections::VecDeque;

use super::state::Prepare;
use crate::MAX_FLUSH;

/// Size of a disk page: flushes start and end on one (FORMATS section 2.3).
const PAGE: u64 = 4096;

fn page_floor(at: u64) -> u64 {
    at - at % PAGE
}

fn page_ceil(at: u64) -> u64 {
    at.div_ceil(PAGE) * PAGE
}

/// A flush to perform: write `buf` at `start` in the file, then sync.
#[derive(Debug)]
pub(super) struct Flush {
    pub file_no: u64,
    pub start: u64,
    pub buf: Vec<u8>,
    pub end: u64,
}

#[derive(Debug)]
pub(super) struct FileBuf {
    pub file_no: u64,
    pub epoch: u64,
    /// The free file to recycle into this one, or `None` to create it.
    source: Option<u64>,
    size: u64,
    prepared: bool,
    sealed: bool,
    /// End of what is written and synced.
    flushed_end: u64,
    /// Bytes from the page holding `flushed_end` up to it: rewritten, the
    /// same, by the next flush.
    carry: Vec<u8>,
    inflight_end: Option<u64>,
    /// Frames not flushed yet, starting where the flush in flight ends.
    pending: Vec<u8>,
    /// End offset of each frame of `pending`, in order.
    pending_ends: VecDeque<u64>,
}

impl FileBuf {
    /// A file still to create or recycle; data starts at `start`.
    pub fn fresh(file_no: u64, epoch: u64, source: Option<u64>, size: u64, start: u64) -> FileBuf {
        FileBuf {
            file_no,
            epoch,
            source,
            size,
            prepared: false,
            sealed: false,
            flushed_end: start,
            carry: Vec::new(),
            inflight_end: None,
            pending: Vec::new(),
            pending_ends: VecDeque::new(),
        }
    }

    /// A file on disk whose valid bytes end at `end`; `carry` holds the
    /// bytes of the page that contains `end`, up to it.
    pub fn resumed(file_no: u64, epoch: u64, size: u64, end: u64, carry: Vec<u8>) -> FileBuf {
        FileBuf {
            prepared: true,
            carry,
            ..FileBuf::fresh(file_no, epoch, None, size, end)
        }
    }

    pub fn prepare(&self) -> Prepare {
        Prepare {
            file_no: self.file_no,
            epoch: self.epoch,
            source: self.source,
            size: self.size,
        }
    }

    pub fn prepared(&self) -> bool {
        self.prepared
    }

    pub fn set_prepared(&mut self) {
        self.prepared = true;
    }

    pub fn seal(&mut self) {
        self.sealed = true;
    }

    pub fn size(&self) -> u64 {
        self.size
    }

    pub fn pending_len(&self) -> usize {
        self.pending.len()
    }

    /// Where the next frame goes.
    fn next_offset(&self) -> u64 {
        self.inflight_end.unwrap_or(self.flushed_end) + self.pending.len() as u64
    }

    /// Bytes used, flushed or not.
    pub fn used(&self) -> u64 {
        self.next_offset()
    }

    /// Bytes left for frames.
    pub fn room(&self) -> u64 {
        self.size.saturating_sub(self.next_offset())
    }

    /// Sealed, and everything is on disk.
    pub fn done(&self) -> bool {
        self.sealed && self.pending.is_empty() && self.inflight_end.is_none()
    }

    /// Appends one frame written by `encode`; returns where it ends.
    pub fn push_with(&mut self, encode: impl FnOnce(&mut Vec<u8>)) -> u64 {
        encode(&mut self.pending);
        let end = self.next_offset();
        self.pending_ends.push_back(end);
        end
    }

    /// Takes the next flush: the carried page, then whole frames, zeros to
    /// the next page; at most [`MAX_FLUSH`] bytes in all (ADR-033).
    pub fn take_flush(&mut self) -> Option<Flush> {
        if self.inflight_end.is_some() || self.pending.is_empty() {
            return None;
        }
        let start = page_floor(self.flushed_end);
        let limit = start + MAX_FLUSH as u64;
        let mut end = self.flushed_end;
        while let Some(&next) = self.pending_ends.front() {
            if page_ceil(next) > limit && end > self.flushed_end {
                break;
            }
            end = next;
            self.pending_ends.pop_front();
        }
        let taken = (end - self.flushed_end) as usize;
        let rest = self.pending.split_off(taken);
        let frames = std::mem::replace(&mut self.pending, rest);
        let total = (page_ceil(end) - start) as usize;
        let mut buf = Vec::with_capacity(total);
        buf.extend_from_slice(&self.carry);
        buf.extend_from_slice(&frames);
        buf.resize(total, 0);
        self.inflight_end = Some(end);
        Some(Flush {
            file_no: self.file_no,
            start,
            buf,
            end,
        })
    }

    /// The flush in flight ended on disk.
    pub fn flushed(&mut self, flush: &Flush) {
        let carried = (page_floor(flush.end) - flush.start) as usize;
        let kept = (flush.end - flush.start) as usize;
        self.carry = flush.buf[carried..kept].to_vec();
        self.flushed_end = flush.end;
        self.inflight_end = None;
    }
}
