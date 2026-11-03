//! Writing a segment in one sequential pass, with bounded memory.

use std::io;
use std::path::Path;

use ndaal_io::{Disk, File, OpenOptions};
use ndaal_wal::seeded_crc_with;

use crate::segment::directory::encode_directory;
use crate::segment::{
    BlockHandle, Compression, FORMAT, Footer, HEADER_LEN, MAX_DIRECTORY_LEN, SECTION_ALIGN,
    Section, SegmentError, SegmentHeader, block_seed, encode_block,
};

/// Bytes buffered before a write to the file: 1 MiB.
const WRITE_BUFFER: usize = 1 << 20;

/// Writes a segment: sections of blocks, then [`SegmentWriter::finish`].
/// Nothing is readable before `finish`; a segment is published only once
/// a manifest names it (T02.16).
pub struct SegmentWriter<F> {
    file: F,
    header: SegmentHeader,
    /// Bytes not written yet; `buf[0]` goes at `buf_at` in the file.
    buf: Vec<u8>,
    buf_at: u64,
    sections: Vec<Section>,
    open: Option<Section>,
}

impl<F: File> SegmentWriter<F> {
    /// Creates the file at `path`; fails with `AlreadyExists` rather than
    /// overwrite a file, and with `Misuse` for a header the reader refuses.
    pub async fn create<D: Disk<File = F>>(
        disk: &D,
        path: &Path,
        header: SegmentHeader,
    ) -> Result<Self, SegmentError> {
        // Never write, sync and publish a segment its reader refuses.
        if header.min_lsn > header.max_lsn {
            return Err(SegmentError::Misuse("min_lsn above max_lsn"));
        }
        // The trait has no create_new: the shard's core owns its segment
        // names, so nothing else creates the file between probe and create.
        let read_only = OpenOptions {
            read_only: true,
            ..OpenOptions::default()
        };
        match disk.open(path, read_only).await {
            Ok(_) => return Err(SegmentError::Io(io::ErrorKind::AlreadyExists.into())),
            Err(e) if e.kind() == io::ErrorKind::NotFound => {}
            Err(e) => return Err(SegmentError::Io(e)),
        }
        let create = OpenOptions {
            create: true,
            truncate: true,
            ..OpenOptions::default()
        };
        let file = disk.open(path, create).await.map_err(SegmentError::Io)?;
        Ok(SegmentWriter {
            file,
            header,
            buf: Vec::with_capacity(WRITE_BUFFER),
            buf_at: HEADER_LEN as u64,
            sections: Vec::new(),
            open: None,
        })
    }

    /// Starts a section of type `kind`, at the next multiple of 4 096.
    pub fn begin_section(&mut self, kind: u32) -> Result<(), SegmentError> {
        if self.open.is_some() {
            return Err(SegmentError::Misuse("section already open"));
        }
        self.pad();
        self.open = Some(Section {
            kind,
            offset: self.position(),
            len: 0,
            blocks: 0,
            properties: Vec::new(),
        });
        Ok(())
    }

    /// Appends a block to the open section; the engine keeps its handle.
    pub async fn add_block(
        &mut self,
        data: &[u8],
        compression: Compression,
    ) -> Result<BlockHandle, SegmentError> {
        if self.open.is_none() {
            return Err(SegmentError::Misuse("block outside a section"));
        }
        let offset = self.position();
        let seed = block_seed(self.header.segment_id, offset);
        encode_block(data, compression, &seed, &mut self.buf)?;
        let len = self.position() - offset;
        let section = self.open.as_mut().expect("checked above");
        section.len += len;
        section.blocks += 1;
        if self.buf.len() >= WRITE_BUFFER {
            self.flush().await?;
        }
        let len = u32::try_from(len).expect("a block is at most MAX_BLOCK_LEN plus its trailer");
        Ok(BlockHandle { offset, len })
    }

    /// Closes the open section with the engine's `properties`.
    pub fn end_section(&mut self, properties: Vec<u8>) -> Result<(), SegmentError> {
        match self.open.take() {
            Some(section) if section.blocks > 0 => {
                self.sections.push(Section {
                    properties,
                    ..section
                });
                Ok(())
            }
            Some(_) => Err(SegmentError::Misuse("empty section")),
            None => Err(SegmentError::Misuse("no open section")),
        }
    }

    /// Writes the directory, the footer and the header, then syncs the
    /// file; returns its size. Renaming and syncing the directory belong
    /// to the manifest (T02.16).
    pub async fn finish(mut self) -> Result<u64, SegmentError> {
        if self.open.is_some() {
            return Err(SegmentError::Misuse("section still open"));
        }
        let directory_offset = self.position();
        let directory = encode_directory(&self.sections);
        if directory.len() > MAX_DIRECTORY_LEN {
            return Err(SegmentError::Misuse("directory over MAX_DIRECTORY_LEN"));
        }
        let footer = Footer {
            directory_offset,
            directory_len: u32::try_from(directory.len()).expect("checked against the maximum"),
            directory_crc: seeded_crc_with(
                &block_seed(self.header.segment_id, directory_offset),
                &directory,
            ),
            format: FORMAT,
        };
        self.buf.extend_from_slice(&directory);
        self.pad();
        self.buf.extend_from_slice(&footer.encode());
        self.flush().await?;
        let io = SegmentError::Io;
        self.file
            .write_all_at(&self.header.encode(), 0)
            .await
            .map_err(io)?;
        self.file.sync_data().await.map_err(io)?;
        Ok(self.buf_at)
    }

    fn position(&self) -> u64 {
        self.buf_at + self.buf.len() as u64
    }

    /// Zeros up to the next multiple of 4 096.
    fn pad(&mut self) {
        let gap = self.position().next_multiple_of(SECTION_ALIGN) - self.position();
        self.buf.resize(self.buf.len() + gap as usize, 0);
    }

    async fn flush(&mut self) -> Result<(), SegmentError> {
        self.file
            .write_all_at(&self.buf, self.buf_at)
            .await
            .map_err(SegmentError::Io)?;
        self.buf_at += self.buf.len() as u64;
        self.buf.clear();
        Ok(())
    }
}
