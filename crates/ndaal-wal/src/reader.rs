//! Reading one log file held in memory (FORMATS_DE_DONNEES.md section 2.3,
//! ADR-033).
//!
//! The first invalid frame ends the readable log. It is a torn write when
//! no valid frame follows at least [`MAX_FLUSH`] bytes further: the writer
//! keeps one flush of at most that size in flight and acknowledges only
//! after it is synced, so whatever a power cut tore was never acknowledged.
//! A valid frame further away means acknowledged data follows a damaged
//! frame: a corruption.

use ndaal_common::ids::{Lsn, PartitionId};

use crate::record::{FLAG_COMPRESSED, FLAG_ENCRYPTED, FLAG_FIRST, FLAG_LAST, FLAGS_RESERVED};
use crate::{
    FRAME_HEADER_LEN, FileHeader, Frame, HEADER_LEN, MAX_FLUSH, MAX_FRAGMENT, RecordType, WalError,
    masked_crc,
};

/// What [`FileReader::next_step`] found.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Step<'a> {
    /// The next frame.
    Frame(Frame<'a>),
    /// The readable log ends at `valid_len`; anything after it was torn or
    /// never written.
    End {
        /// Bytes of the file, header included, that hold valid frames.
        valid_len: u64,
    },
    /// The frame at `offset` is damaged and valid frames follow it.
    Corrupt {
        /// Where the damaged frame starts.
        offset: u64,
    },
}

/// Reads the frames of one log file.
#[derive(Debug)]
pub struct FileReader<'a> {
    file: &'a [u8],
    header: FileHeader,
    epoch: u16,
    at: usize,
    done: Option<Step<'a>>,
}

fn u32_at(b: &[u8], at: usize) -> u32 {
    u32::from_le_bytes([b[at], b[at + 1], b[at + 2], b[at + 3]])
}

fn u64_at(b: &[u8], at: usize) -> u64 {
    let mut x = [0; 8];
    x.copy_from_slice(&b[at..at + 8]);
    u64::from_le_bytes(x)
}

fn offset(at: usize) -> u64 {
    // A file held in memory is far smaller than 2^64 bytes.
    u64::try_from(at).unwrap_or(u64::MAX)
}

impl<'a> FileReader<'a> {
    /// A reader over the whole contents of a log file; checks its header.
    pub fn new(file: &'a [u8]) -> Result<FileReader<'a>, WalError> {
        let header = FileHeader::decode(file)?;
        Ok(FileReader {
            file,
            header,
            // Frames keep the low 16 bits of the epoch.
            epoch: (header.recycle_epoch & 0xFFFF) as u16,
            at: HEADER_LEN,
            done: None,
        })
    }

    /// The file's header.
    pub fn header(&self) -> &FileHeader {
        &self.header
    }

    /// The next frame, or how the log ends. Once it ended, every call
    /// returns the same end.
    pub fn next_step(&mut self) -> Result<Step<'a>, WalError> {
        if let Some(done) = self.done {
            return Ok(done);
        }
        loop {
            let Some((frame, flags, len)) = self.frame_at(self.at) else {
                let step = self.end_at(self.at);
                self.done = Some(step);
                return Ok(step);
            };
            if flags & FLAG_COMPRESSED != 0 {
                return Err(WalError::Unsupported("compressed log record"));
            }
            if flags & FLAG_ENCRYPTED != 0 {
                return Err(WalError::Unsupported("encrypted log record"));
            }
            self.at += len;
            if frame.kind != RecordType::Padding {
                return Ok(Step::Frame(frame));
            }
        }
    }

    /// How the log ends when the frame at `at` is invalid. Acknowledged
    /// frames after it would be contiguous, so one of them would start at
    /// most one whole frame past `at + MAX_FLUSH`: the scan stops there,
    /// which bounds its cost on hostile bytes and ignores a lone stray
    /// frame further away.
    fn end_at(&self, at: usize) -> Step<'a> {
        let from = at.saturating_add(MAX_FLUSH);
        let last = self
            .file
            .len()
            .saturating_sub(FRAME_HEADER_LEN)
            .min(from.saturating_add(MAX_FRAGMENT + FRAME_HEADER_LEN));
        if (from..=last).any(|p| self.frame_at(p).is_some()) {
            Step::Corrupt { offset: offset(at) }
        } else {
            Step::End {
                valid_len: offset(at),
            }
        }
    }

    /// The valid frame at `at`, its flags and its total length. Cheap
    /// checks come first: scanning zeros never computes a CRC.
    fn frame_at(&self, at: usize) -> Option<(Frame<'a>, u8, usize)> {
        let b = self.file;
        let header = b.get(at..at.checked_add(FRAME_HEADER_LEN)?)?;
        let len = usize::try_from(u32_at(header, 4)).ok()?;
        if len > MAX_FRAGMENT {
            return None;
        }
        let end = at + FRAME_HEADER_LEN + len;
        let frame = b.get(at..end)?;
        if u16::from_le_bytes([header[10], header[11]]) != self.epoch {
            return None;
        }
        let kind = RecordType::from_code(header[8])?;
        let flags = header[9];
        if flags & FLAGS_RESERVED != 0 {
            return None;
        }
        if masked_crc(&frame[4..]) != u32_at(header, 0) {
            return None;
        }
        let parsed = Frame {
            kind,
            partition: PartitionId(u32_at(header, 12)),
            lsn: Lsn(u64_at(header, 16)),
            term: u64_at(header, 24),
            timestamp: u64_at(header, 32),
            first: flags & FLAG_FIRST != 0,
            last: flags & FLAG_LAST != 0,
            payload: &frame[FRAME_HEADER_LEN..],
        };
        Some((parsed, flags, FRAME_HEADER_LEN + len))
    }
}
