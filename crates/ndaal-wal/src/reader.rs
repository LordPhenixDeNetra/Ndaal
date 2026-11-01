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
    seeded_crc,
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
    epoch: u64,
    file_no: u64,
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
            epoch: header.recycle_epoch,
            file_no: header.file_no,
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
    /// The kind, flags and payload length of the frame at `at` if its
    /// header is valid: every cheap check, then the header CRC, which
    /// rejects a false candidate in 44 bytes before any payload is read
    /// (ADR-034).
    fn header_at(&self, at: usize) -> Option<(RecordType, u8, usize)> {
        let header = self.file.get(at..at.checked_add(FRAME_HEADER_LEN)?)?;
        let len = usize::try_from(u32_at(header, 8)).ok()?;
        if len > MAX_FRAGMENT || self.file.len() - at - FRAME_HEADER_LEN < len {
            return None;
        }
        // Frames keep the low 16 bits of the epoch: a cheap first filter.
        if u16::from_le_bytes([header[14], header[15]]) != (self.epoch & 0xFFFF) as u16 {
            return None;
        }
        let kind = RecordType::from_code(header[12])?;
        let flags = header[13];
        if flags & FLAGS_RESERVED != 0 {
            return None;
        }
        if seeded_crc(self.epoch, self.file_no, &header[8..]) != u32_at(header, 4) {
            return None;
        }
        Some((kind, flags, len))
    }

    /// The valid frame at `at`, its flags and its total length.
    fn frame_at(&self, at: usize) -> Option<(Frame<'a>, u8, usize)> {
        let (kind, flags, len) = self.header_at(at)?;
        let frame = &self.file[at..at + FRAME_HEADER_LEN + len];
        let header = &frame[..FRAME_HEADER_LEN];
        if seeded_crc(self.epoch, self.file_no, &frame[4..]) != u32_at(header, 0) {
            return None;
        }
        let parsed = Frame {
            kind,
            partition: PartitionId(u32_at(header, 16)),
            lsn: Lsn(u64_at(header, 20)),
            term: u64_at(header, 28),
            timestamp: u64_at(header, 36),
            first: flags & FLAG_FIRST != 0,
            last: flags & FLAG_LAST != 0,
            payload: &frame[FRAME_HEADER_LEN..],
        };
        Some((parsed, flags, FRAME_HEADER_LEN + len))
    }
}

#[cfg(test)]
mod tests {
    use std::borrow::Cow;

    use ndaal_common::ids::{Lsn, NodeId, PartitionId};

    use super::*;
    use crate::{Record, encode};

    fn file() -> Vec<u8> {
        let header = FileHeader {
            shard: 0,
            file_no: 3,
            node: NodeId([0; 16]),
            created_at: 0,
            recycle_epoch: 9,
            recycled: true,
        };
        let mut bytes = header.encode().to_vec();
        let record = Record {
            kind: RecordType::KvEffects,
            partition: PartitionId(1),
            lsn: Lsn(1),
            term: 0,
            timestamp: 0,
            payload: Cow::Borrowed(b"payload"),
        };
        encode(&record, 9, 3, &mut bytes);
        bytes
    }

    #[test]
    fn header_crc_rejects_before_payload_crc() {
        let mut damaged_payload = file();
        let last = damaged_payload.len() - 1;
        damaged_payload[last] ^= 1;
        let reader = FileReader::new(&damaged_payload).expect("header");
        assert!(reader.header_at(HEADER_LEN).is_some(), "header still valid");
        assert!(
            reader.frame_at(HEADER_LEN).is_none(),
            "payload CRC catches it"
        );

        let mut damaged_header = file();
        damaged_header[HEADER_LEN + 16] ^= 1;
        let reader = FileReader::new(&damaged_header).expect("header");
        assert!(
            reader.header_at(HEADER_LEN).is_none(),
            "header CRC catches it"
        );
    }
}
