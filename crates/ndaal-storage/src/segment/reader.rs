//! Opening a segment (every check of FORMATS_DE_DONNEES.md section 3.5)
//! and reading its blocks, each checked on every read.

use std::path::Path;

use ndaal_io::{Disk, File, OpenOptions};
use ndaal_wal::seeded_crc_with;

use crate::segment::format::corrupt;
use crate::segment::{
    FOOTER_LEN, Footer, HEADER_LEN, MAX_BLOCK_LEN, MAX_DIRECTORY_LEN, Section, SegmentError,
    SegmentHeader, SegmentKind, TRAILER_LEN, block_seed, decode_block, decode_directory,
};

/// Where a block is: what the engine keeps to read it back.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BlockHandle {
    /// Offset of the block in the file.
    pub offset: u64,
    /// Stored length, trailer included.
    pub len: u32,
}

/// An open segment whose footer, header and directory are valid.
pub struct SegmentReader<F> {
    file: F,
    header: SegmentHeader,
    sections: Vec<Section>,
}

impl<F: File> SegmentReader<F> {
    /// Opens the segment at `path`, of kind `expected`, and checks it.
    pub async fn open<D: Disk<File = F>>(
        disk: &D,
        path: &Path,
        expected: SegmentKind,
    ) -> Result<Self, SegmentError> {
        let options = OpenOptions {
            read_only: true,
            ..OpenOptions::default()
        };
        let file = disk.open(path, options).await.map_err(SegmentError::Io)?;
        let len = file.len().await.map_err(SegmentError::Io)?;
        let page = HEADER_LEN as u64;
        if len < 2 * page || len % page != 0 {
            return Err(corrupt("file size", len));
        }
        let footer_at = len - FOOTER_LEN as u64;
        let footer = read(&file, footer_at, FOOTER_LEN).await?;
        let footer = Footer::decode(&footer).map_err(|e| at(e, footer_at))?;
        let (header, format) = SegmentHeader::decode(&read(&file, 0, HEADER_LEN).await?, expected)?;
        if format != footer.format {
            return Err(corrupt("footer format differs from the header", footer_at));
        }
        let sections = directory(&file, &header, &footer, footer_at).await?;
        Ok(SegmentReader {
            file,
            header,
            sections,
        })
    }

    /// The header fields.
    pub fn header(&self) -> &SegmentHeader {
        &self.header
    }

    /// The sections, in file order.
    pub fn sections(&self) -> &[Section] {
        &self.sections
    }

    /// The data of the block at `handle`, its CRC and compression checked.
    /// A handle outside every section is refused before any read.
    pub async fn read_block(&self, handle: BlockHandle) -> Result<Vec<u8>, SegmentError> {
        let len = handle.len as usize;
        if len <= TRAILER_LEN || len > MAX_BLOCK_LEN + TRAILER_LEN {
            return Err(corrupt("block handle length", handle.offset));
        }
        let end = handle.offset.checked_add(u64::from(handle.len));
        let inside = self
            .sections
            .iter()
            .any(|s| s.offset <= handle.offset && end.is_some_and(|e| e <= s.offset + s.len));
        if !inside {
            return Err(corrupt("block handle outside the sections", handle.offset));
        }
        let stored = read(&self.file, handle.offset, len).await?;
        let seed = block_seed(self.header.segment_id, handle.offset);
        decode_block(&stored, &seed).map_err(|e| at(e, handle.offset))
    }
}

/// The checked sections of the directory the footer points to.
async fn directory<F: File>(
    file: &F,
    header: &SegmentHeader,
    footer: &Footer,
    footer_at: u64,
) -> Result<Vec<Section>, SegmentError> {
    let offset = footer.directory_offset;
    let len = footer.directory_len as usize;
    let fits = offset >= HEADER_LEN as u64
        && offset
            .checked_add(u64::from(footer.directory_len))
            .is_some_and(|e| e <= footer_at);
    if len > MAX_DIRECTORY_LEN || !fits {
        return Err(corrupt("directory bounds", footer_at));
    }
    let bytes = read(file, offset, len).await?;
    if seeded_crc_with(&block_seed(header.segment_id, offset), &bytes) != footer.directory_crc {
        return Err(corrupt("directory crc", offset));
    }
    decode_directory(&bytes, offset)
}

/// Exactly `len` bytes at `offset`; a short read is a corruption (the file
/// is shorter than its own structure says).
async fn read<F: File>(file: &F, offset: u64, len: usize) -> Result<Vec<u8>, SegmentError> {
    let mut buf = vec![0u8; len];
    let mut done = 0;
    while done < len {
        let n = file
            .read_at(&mut buf[done..], offset + done as u64)
            .await
            .map_err(SegmentError::Io)?;
        if n == 0 {
            return Err(corrupt("short read", offset));
        }
        done += n;
    }
    Ok(buf)
}

/// `error` with the offset in the file of the part that failed.
fn at(error: SegmentError, offset: u64) -> SegmentError {
    match error {
        SegmentError::Corrupt { what, .. } => SegmentError::Corrupt { what, offset },
        other => other,
    }
}
