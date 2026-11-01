//! Write-ahead log: format, group commit, reading, recovery.
//!
//! J02-C1: the format and the reader, pure functions over bytes
//! (FORMATS_DE_DONNEES.md section 2, ADR-033). The writer and the files
//! come with J02-C2.

// unsafe allowed (ARCHITECTURE.md section 3): aligned buffers for direct I/O.
// Every unsafe block carries a SAFETY comment.

mod crc;
mod error;
mod header;
mod reader;
mod reassemble;
mod record;
mod writer;

pub use crc::{masked_crc, seeded_crc};
pub use error::{FragmentError, WalError};
pub use header::{FORMAT, FileHeader, HEADER_LEN};
pub use reader::{FileReader, Step};
pub use reassemble::Reassembler;
pub use record::{
    FRAME_HEADER_LEN, Frame, MAX_FLUSH, MAX_FRAGMENT, Record, RecordType, encode, encode_fragment,
    encoded_len, fragment_count,
};
pub use writer::{Durability, Flusher, Position, WalWriter, WriterConfig};
