//! Generic segment envelope (FORMATS_DE_DONNEES.md section 3, T02.13):
//! header, sections of checksummed blocks, directory, footer. Every
//! immutable segment (SST, vector, object metadata) is written in it.

mod block;
mod directory;
mod format;
mod reader;
mod writer;

pub use block::{Compression, MAX_BLOCK_LEN, TRAILER_LEN, block_seed, decode_block, encode_block};
pub use directory::{MAX_DIRECTORY_LEN, Section, decode_directory};
pub use format::{FOOTER_LEN, FORMAT, Footer, HEADER_LEN, SegmentHeader, SegmentKind};
pub use reader::{BlockHandle, SegmentReader};
pub use writer::SegmentWriter;

/// Every section starts at a multiple of this (spec decision 2): a
/// section can be read with direct I/O or mapped without a copy (T05.10).
pub const SECTION_ALIGN: u64 = 4096;

/// What can go wrong reading or writing a segment.
#[derive(Debug, thiserror::Error)]
pub enum SegmentError {
    /// The disk failed.
    #[error("segment i/o failed")]
    Io(#[source] std::io::Error),
    /// Bytes read from the segment are not what was written.
    #[error("corrupt segment: {what} at offset {offset}")]
    Corrupt {
        /// The check that failed.
        what: &'static str,
        /// Offset in the file of the part that failed it.
        offset: u64,
    },
    /// The envelope format is unknown to this version.
    #[error("unsupported segment format {found}")]
    UnsupportedFormat {
        /// The format read.
        found: u16,
    },
    /// The segment is of another kind than the caller expects.
    #[error("segment of kind {found} where {expected:?} was expected")]
    WrongKind {
        /// The kind the caller asked for.
        expected: SegmentKind,
        /// The kind read.
        found: u16,
    },
    /// The segment is encrypted; encryption comes with T11.07.
    #[error("encrypted segments are not supported yet")]
    Encrypted,
    /// The writer was used out of order.
    #[error("segment writer misuse: {0}")]
    Misuse(&'static str),
}
