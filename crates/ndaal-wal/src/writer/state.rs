//! The writer's buffers, file by file. Pure: no I/O, no clock.

use std::collections::{BTreeMap, VecDeque};
use std::time::Duration;

use ndaal_common::ids::{Lsn, PartitionId};

use super::file_buf::{FileBuf, Flush};
use crate::{
    FRAME_HEADER_LEN, HEADER_LEN, MAX_FRAGMENT, Record, RecordType, encode_fragment, fragment_count,
};

/// Where a record ends in the shard's log: the file and the offset just
/// past its last frame. Ordered by file, then offset.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Position {
    /// Number of the log file.
    pub file_no: u64,
    /// Offset in that file.
    pub offset: u64,
}

/// A file to make ready on disk before data goes into it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct Prepare {
    pub file_no: u64,
    pub epoch: u64,
    /// The free file to recycle, or `None` to create one.
    pub source: Option<u64>,
    pub size: u64,
}

const FRAME_HEADER: u64 = FRAME_HEADER_LEN as u64;

#[derive(Debug)]
pub(super) struct State {
    file_size: u64,
    /// Oldest first; the back takes the appends. A sealed file leaves once
    /// all its bytes are on disk.
    files: VecDeque<FileBuf>,
    /// The file after the current one, decided ahead of the rollover.
    next: Option<FileBuf>,
    /// Released files to recycle: `(number, recycle epoch)`.
    free: VecDeque<(u64, u64)>,
    /// Recycle epoch of every numbered file still in the log.
    epochs: BTreeMap<u64, u64>,
    next_file_no: u64,
    pub durable: Option<Position>,
    pub failed: bool,
    /// Someone waits for durability: flush with the group commit rules.
    pub urgent: bool,
    /// Records appended so far: a yield that adds none means the core idles.
    pub appended: u64,
    /// When the oldest pending record was appended.
    pub first_pending_at: Option<Duration>,
}

impl State {
    /// The log continues in `current`. File numbers below `next_file_no`
    /// are taken; `epochs` holds the numbered files on disk, `free` the
    /// released ones.
    pub fn new(
        file_size: u64,
        current: FileBuf,
        next_file_no: u64,
        epochs: BTreeMap<u64, u64>,
        free: VecDeque<(u64, u64)>,
    ) -> State {
        State {
            file_size,
            files: VecDeque::from([current]),
            next: None,
            free,
            epochs,
            next_file_no,
            durable: None,
            failed: false,
            urgent: false,
            appended: 0,
            first_pending_at: None,
        }
    }

    /// The file after the current one is already on disk (found at open).
    pub fn with_next(mut self, next: FileBuf) -> State {
        self.next = Some(next);
        self
    }

    fn current(&mut self) -> &mut FileBuf {
        let last = self.files.len() - 1;
        &mut self.files[last]
    }

    pub fn pending_len(&self) -> usize {
        self.files.iter().map(FileBuf::pending_len).sum()
    }

    pub fn has_pending(&self) -> bool {
        self.files.iter().any(|f| f.pending_len() > 0)
    }

    /// The oldest file still in memory: files below it may be released.
    pub fn oldest_file(&self) -> u64 {
        self.files.front().map_or(0, |f| f.file_no)
    }

    /// Appends the frames of `record`, rolling over to the next file when
    /// a frame does not fit; returns where the record ends.
    pub fn push(&mut self, record: &Record<'_>, now: Duration) -> Position {
        let mut end = Position {
            file_no: 0,
            offset: 0,
        };
        for i in 0..fragment_count(record.payload.len()) {
            let chunk = (record.payload.len() - i * MAX_FRAGMENT).min(MAX_FRAGMENT);
            self.make_room(FRAME_HEADER + chunk as u64);
            let file = self.current();
            let (file_no, epoch) = (file.file_no, file.epoch);
            let offset = file.push_with(|out| encode_fragment(record, i, epoch, file_no, out));
            end = Position { file_no, offset };
        }
        self.appended += 1;
        self.first_pending_at.get_or_insert(now);
        end
    }

    /// Makes the current file fit a frame of `len` bytes. If it does not,
    /// or would keep 1 to 43 bytes no frame can fill, its rest is padded
    /// and the next file becomes current.
    fn make_room(&mut self, len: u64) {
        let rest = self.current().room();
        if len <= rest && (rest == len || rest - len >= FRAME_HEADER) {
            return;
        }
        self.seal_current();
        let next = match self.next.take() {
            Some(next) => next,
            None => self.decide_next(),
        };
        self.files.push_back(next);
    }

    /// Fills the current file's rest with padding frames and seals it.
    fn seal_current(&mut self) {
        let file = self.current();
        let (file_no, epoch) = (file.file_no, file.epoch);
        let mut rest = file.room();
        // Less than a frame header can only be left by a damaged file: seal
        // as is rather than write a frame that cannot fit.
        while rest >= FRAME_HEADER {
            let mut take = rest.min(MAX_FRAGMENT as u64 + FRAME_HEADER);
            if (1..FRAME_HEADER).contains(&(rest - take)) {
                // Leave room for one last, empty padding frame.
                take = rest - FRAME_HEADER;
            }
            let padding = Record {
                kind: RecordType::Padding,
                partition: PartitionId(0),
                lsn: Lsn(0),
                term: 0,
                timestamp: 0,
                payload: vec![0; (take - FRAME_HEADER) as usize].into(),
            };
            self.current()
                .push_with(|out| encode_fragment(&padding, 0, epoch, file_no, out));
            rest -= take;
        }
        self.current().seal();
    }

    /// Chooses the next file: a released one to recycle, or a new one.
    fn decide_next(&mut self) -> FileBuf {
        let file_no = self.next_file_no;
        self.next_file_no += 1;
        let (epoch, source) = match self.free.pop_front() {
            Some((free_no, free_epoch)) => (free_epoch.wrapping_add(1), Some(free_no)),
            None => (0, None),
        };
        self.epochs.insert(file_no, epoch);
        FileBuf::fresh(file_no, epoch, source, self.file_size, HEADER_LEN as u64)
    }

    /// The file to make ready on disk now, if any: the oldest one holding
    /// data that is not ready, or the next file once the current one is
    /// half full (so the rollover never waits).
    pub fn next_to_prepare(&mut self) -> Option<Prepare> {
        if let Some(file) = self.files.iter().find(|f| !f.prepared()) {
            return Some(file.prepare());
        }
        let current = self.files.back()?;
        if self.next.is_none() && current.used() * 2 >= current.size() {
            let next = self.decide_next();
            self.next = Some(next);
        }
        self.next
            .as_ref()
            .filter(|f| !f.prepared())
            .map(FileBuf::prepare)
    }

    /// File `file_no` is ready on disk.
    pub fn prepared(&mut self, file_no: u64) {
        for file in self.files.iter_mut().chain(self.next.iter_mut()) {
            if file.file_no == file_no {
                file.set_prepared();
            }
        }
    }

    /// Takes the next flush from the oldest file with pending bytes; drops
    /// sealed files that are fully on disk.
    pub fn take_flush(&mut self) -> Option<Flush> {
        loop {
            let front = self.files.front_mut()?;
            if !front.prepared() {
                return None;
            }
            if let Some(flush) = front.take_flush() {
                return Some(flush);
            }
            if front.done() && self.files.len() > 1 {
                self.files.pop_front();
                continue;
            }
            return None;
        }
    }

    /// Drops the sealed files at the front that are fully on disk.
    pub fn take_flush_cleanup(&mut self) {
        while self.files.len() > 1 && self.files.front().is_some_and(FileBuf::done) {
            self.files.pop_front();
        }
    }

    /// `flush` ended on disk.
    pub fn flushed(&mut self, flush: &Flush, now: Duration) {
        if let Some(file) = self.files.iter_mut().find(|f| f.file_no == flush.file_no) {
            file.flushed(flush);
        }
        self.durable = Some(Position {
            file_no: flush.file_no,
            offset: flush.end,
        });
        self.first_pending_at = self.has_pending().then_some(now);
        self.urgent = false;
    }

    /// Takes file `file_no` out of the log so it can be recycled; returns
    /// `false` if it is not an old, numbered file of the log.
    pub fn release(&mut self, file_no: u64) -> bool {
        if file_no >= self.oldest_file() {
            return false;
        }
        match self.epochs.remove(&file_no) {
            Some(epoch) => {
                self.free.push_back((file_no, epoch));
                true
            }
            None => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SIZE: u64 = 4 << 20;

    fn state() -> State {
        let mut current = FileBuf::fresh(0, 0, None, SIZE, HEADER_LEN as u64);
        current.set_prepared();
        State::new(SIZE, current, 1, BTreeMap::from([(0, 0)]), VecDeque::new())
    }

    fn record(size: usize) -> Record<'static> {
        Record {
            kind: RecordType::KvEffects,
            partition: PartitionId(1),
            lsn: Lsn(1),
            term: 0,
            timestamp: 0,
            payload: vec![1; size].into(),
        }
    }

    #[test]
    fn flush_rewrites_the_partial_page() {
        let mut st = state();
        let first = st.push(&record(100), Duration::ZERO);
        let flush = st.take_flush().expect("flush");
        assert_eq!((flush.start, flush.end), (4096, first.offset));
        assert_eq!(flush.buf.len(), 4096);
        st.flushed(&flush, Duration::ZERO);
        st.push(&record(100), Duration::ZERO);
        let next = st.take_flush().expect("flush");
        assert_eq!(next.start, 4096, "same page, rewritten");
        assert_eq!(next.buf[..144], flush.buf[..144]);
    }

    #[test]
    fn rollover_never_leaves_an_unfillable_rest() {
        for size in [100, 1000, 40_000, 999_999, MAX_FRAGMENT, MAX_FRAGMENT + 5] {
            let mut st = state();
            let mut last = Position {
                file_no: 0,
                offset: 0,
            };
            while last.file_no == 0 {
                last = st.push(&record(size), Duration::ZERO);
            }
            let sealed = &st.files[0];
            assert_eq!(sealed.room(), 0, "{size}: file 0 padded to its end");
            assert!(st.files[1].room() < SIZE);
        }
    }

    #[test]
    fn released_file_is_recycled_with_the_next_epoch() {
        let mut st = state();
        let mut last = Position {
            file_no: 0,
            offset: 0,
        };
        while last.file_no < 1 {
            last = st.push(&record(500_000), Duration::ZERO);
        }
        st.files.pop_front();
        assert!(st.release(0));
        assert!(!st.release(1), "the current file stays");
        let next = st.decide_next();
        assert_eq!(next.prepare().source, Some(0));
        assert_eq!(next.epoch, 1);
    }
}
