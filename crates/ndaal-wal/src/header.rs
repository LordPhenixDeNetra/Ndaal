//! The 4 KiB header of a log file (FORMATS_DE_DONNEES.md section 2.2).

use ndaal_common::ids::NodeId;

use crate::{WalError, masked_crc};

/// Size of the header: one page, so records start aligned.
pub const HEADER_LEN: usize = 4096;
/// Version of the log format this build writes and reads.
pub const FORMAT: u16 = 1;

const MAGIC: &[u8; 8] = b"NDLWAL01";
/// Header flag: the file is encrypted (not supported yet).
const FLAG_ENCRYPTED: u32 = 1;
/// Header flag: the file was recycled.
const FLAG_RECYCLED: u32 = 1 << 1;
/// Where the CRC of the header lives: its last 4 bytes.
const CRC_AT: usize = HEADER_LEN - 4;

/// What a log file says about itself.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FileHeader {
    /// Shard whose log this is.
    pub shard: u16,
    /// Number of the file in the shard's log.
    pub file_no: u64,
    /// Node that wrote it.
    pub node: NodeId,
    /// Creation time, microseconds since the Unix epoch.
    pub created_at: u64,
    /// Raised at each recycling; records carry its low 16 bits.
    pub recycle_epoch: u64,
    /// Whether the file was recycled.
    pub recycled: bool,
}

fn u16_at(bytes: &[u8], at: usize) -> u16 {
    u16::from_le_bytes([bytes[at], bytes[at + 1]])
}

fn u32_at(bytes: &[u8], at: usize) -> u32 {
    let mut b = [0; 4];
    b.copy_from_slice(&bytes[at..at + 4]);
    u32::from_le_bytes(b)
}

fn u64_at(bytes: &[u8], at: usize) -> u64 {
    let mut b = [0; 8];
    b.copy_from_slice(&bytes[at..at + 8]);
    u64::from_le_bytes(b)
}

impl FileHeader {
    /// The header block, CRC included.
    pub fn encode(&self) -> [u8; HEADER_LEN] {
        let mut out = [0u8; HEADER_LEN];
        let flags = if self.recycled { FLAG_RECYCLED } else { 0 };
        out[..8].copy_from_slice(MAGIC);
        out[8..10].copy_from_slice(&FORMAT.to_le_bytes());
        out[10..12].copy_from_slice(&self.shard.to_le_bytes());
        out[12..16].copy_from_slice(&flags.to_le_bytes());
        out[16..24].copy_from_slice(&self.file_no.to_le_bytes());
        out[24..40].copy_from_slice(&self.node.0);
        out[40..48].copy_from_slice(&self.created_at.to_le_bytes());
        out[48..56].copy_from_slice(&self.recycle_epoch.to_le_bytes());
        // 56..72: key id of an encrypted file; zeros until encryption.
        let crc = masked_crc(&out[..CRC_AT]);
        out[CRC_AT..].copy_from_slice(&crc.to_le_bytes());
        out
    }

    /// Reads the header at the start of `bytes`.
    pub fn decode(bytes: &[u8]) -> Result<FileHeader, WalError> {
        let bytes = bytes.get(..HEADER_LEN).ok_or(WalError::ShortHeader)?;
        if &bytes[..8] != MAGIC {
            return Err(WalError::BadMagic);
        }
        if masked_crc(&bytes[..CRC_AT]) != u32_at(bytes, CRC_AT) {
            return Err(WalError::BadHeaderCrc);
        }
        let format = u16_at(bytes, 8);
        if format != FORMAT {
            return Err(WalError::UnknownFormat(format));
        }
        let flags = u32_at(bytes, 12);
        if flags & FLAG_ENCRYPTED != 0 {
            return Err(WalError::Unsupported("encrypted log file"));
        }
        if flags & !FLAG_RECYCLED != 0 {
            return Err(WalError::Unsupported("unknown log file flags"));
        }
        let mut node = [0; 16];
        node.copy_from_slice(&bytes[24..40]);
        Ok(FileHeader {
            shard: u16_at(bytes, 10),
            file_no: u64_at(bytes, 16),
            node: NodeId(node),
            created_at: u64_at(bytes, 40),
            recycle_epoch: u64_at(bytes, 48),
            recycled: flags & FLAG_RECYCLED != 0,
        })
    }
}
