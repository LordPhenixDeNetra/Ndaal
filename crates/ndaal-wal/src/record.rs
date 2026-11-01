//! Records and their frames (FORMATS_DE_DONNEES.md sections 2.3 and 2.4).

use std::borrow::Cow;

use ndaal_common::ids::{Lsn, PartitionId};

use crate::masked_crc;

/// Bytes of a frame before its payload.
pub const FRAME_HEADER_LEN: usize = 40;
/// Largest payload of one frame; larger records are fragmented (ADR-033).
pub const MAX_FRAGMENT: usize = 1 << 20;
/// Largest flush of the writer, hence the largest zone a power cut can
/// tear (ADR-033).
pub const MAX_FLUSH: usize = 2 << 20;

/// Frame flag: payload compressed (not supported yet).
pub(crate) const FLAG_COMPRESSED: u8 = 1;
/// Frame flag: payload encrypted (not supported yet).
pub(crate) const FLAG_ENCRYPTED: u8 = 1 << 1;
/// Frame flag: last fragment of a record.
pub(crate) const FLAG_LAST: u8 = 1 << 2;
/// Frame flag: first fragment of a record.
pub(crate) const FLAG_FIRST: u8 = 1 << 3;
/// Flag bits that must be zero.
pub(crate) const FLAGS_RESERVED: u8 = 0xF0;

/// The type of a record and its code (FORMATS_DE_DONNEES.md section 2.4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RecordType {
    /// Key-value effects.
    KvEffects,
    /// Vector effects.
    VecEffects,
    /// Object metadata effects.
    ObjEffects,
    /// A transaction mixing the three models.
    Tx,
    /// Start of a partition checkpoint.
    CheckpointBegin,
    /// End of a partition checkpoint.
    CheckpointEnd,
    /// Change of the partition's local catalog.
    Schema,
    /// Partition split.
    Split,
    /// Partition merge.
    Merge,
    /// Change of the Raft group members.
    RaftMembership,
    /// Empty entry of a new Raft leader.
    RaftBlank,
    /// Deletions by expiry.
    ExpireDel,
    /// Mark of the blob garbage collector.
    GcMark,
    /// Padding, skipped by readers.
    Padding,
}

impl RecordType {
    /// Every type.
    pub const ALL: [RecordType; 14] = [
        RecordType::KvEffects,
        RecordType::VecEffects,
        RecordType::ObjEffects,
        RecordType::Tx,
        RecordType::CheckpointBegin,
        RecordType::CheckpointEnd,
        RecordType::Schema,
        RecordType::Split,
        RecordType::Merge,
        RecordType::RaftMembership,
        RecordType::RaftBlank,
        RecordType::ExpireDel,
        RecordType::GcMark,
        RecordType::Padding,
    ];

    /// The code written in the frame.
    pub fn code(self) -> u8 {
        match self {
            RecordType::KvEffects => 0x01,
            RecordType::VecEffects => 0x02,
            RecordType::ObjEffects => 0x03,
            RecordType::Tx => 0x04,
            RecordType::CheckpointBegin => 0x10,
            RecordType::CheckpointEnd => 0x11,
            RecordType::Schema => 0x20,
            RecordType::Split => 0x21,
            RecordType::Merge => 0x22,
            RecordType::RaftMembership => 0x30,
            RecordType::RaftBlank => 0x31,
            RecordType::ExpireDel => 0x40,
            RecordType::GcMark => 0x41,
            RecordType::Padding => 0xF0,
        }
    }

    /// The type of `code`, if it is one.
    pub fn from_code(code: u8) -> Option<RecordType> {
        RecordType::ALL.into_iter().find(|t| t.code() == code)
    }
}

/// A log record: one change of one partition.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Record<'a> {
    /// What the payload holds.
    pub kind: RecordType,
    /// The partition changed.
    pub partition: PartitionId,
    /// Its LSN in that partition.
    pub lsn: Lsn,
    /// Raft term, 0 outside a cluster.
    pub term: u64,
    /// Leader clock, microseconds since the Unix epoch.
    pub timestamp: u64,
    /// The change, opaque to the log.
    pub payload: Cow<'a, [u8]>,
}

/// One frame read from a file: a whole record or one of its fragments.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Frame<'a> {
    /// Record type.
    pub kind: RecordType,
    /// Partition.
    pub partition: PartitionId,
    /// LSN in the partition.
    pub lsn: Lsn,
    /// Raft term.
    pub term: u64,
    /// Leader clock.
    pub timestamp: u64,
    /// First fragment of its record (both flags: a whole record).
    pub first: bool,
    /// Last fragment of its record.
    pub last: bool,
    /// Payload bytes, borrowed from the file.
    pub payload: &'a [u8],
}

/// Bytes [`encode`] writes for a payload of `payload_len` bytes.
pub fn encoded_len(payload_len: usize) -> usize {
    let frames = payload_len.div_ceil(MAX_FRAGMENT).max(1);
    frames * FRAME_HEADER_LEN + payload_len
}

/// Appends `record` to `out` as one frame, or as fragments of at most
/// [`MAX_FRAGMENT`] bytes, stamped with the low 16 bits of `epoch`.
pub fn encode(record: &Record<'_>, epoch: u64, out: &mut Vec<u8>) {
    let payload: &[u8] = &record.payload;
    let count = payload.len().div_ceil(MAX_FRAGMENT).max(1);
    out.reserve(encoded_len(payload.len()));
    for i in 0..count {
        let end = ((i + 1) * MAX_FRAGMENT).min(payload.len());
        let chunk = &payload[(i * MAX_FRAGMENT).min(end)..end];
        let mut flags = 0;
        if i == 0 {
            flags |= FLAG_FIRST;
        }
        if i + 1 == count {
            flags |= FLAG_LAST;
        }
        put_frame(record, flags, epoch, chunk, out);
    }
}

fn put_frame(record: &Record<'_>, flags: u8, epoch: u64, chunk: &[u8], out: &mut Vec<u8>) {
    let start = out.len();
    // `chunk` holds at most MAX_FRAGMENT bytes: it fits a u32.
    let len = u32::try_from(chunk.len()).unwrap_or(u32::MAX);
    out.extend_from_slice(&[0; 4]);
    out.extend_from_slice(&len.to_le_bytes());
    out.push(record.kind.code());
    out.push(flags);
    // The frame keeps the low 16 bits of the epoch (FORMATS section 2.3).
    out.extend_from_slice(&((epoch & 0xFFFF) as u16).to_le_bytes());
    out.extend_from_slice(&record.partition.0.to_le_bytes());
    out.extend_from_slice(&record.lsn.0.to_le_bytes());
    out.extend_from_slice(&record.term.to_le_bytes());
    out.extend_from_slice(&record.timestamp.to_le_bytes());
    out.extend_from_slice(chunk);
    let crc = masked_crc(&out[start + 4..]);
    out[start..start + 4].copy_from_slice(&crc.to_le_bytes());
}
