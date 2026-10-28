//! Disk and file access.
//!
//! Durability follows POSIX: file contents are durable after
//! [`File::sync_data`]; creating, renaming or removing a file is durable after
//! [`Disk::sync_dir`] on its parent directory.

use std::io;
use std::path::{Path, PathBuf};

/// How to open a file: for reading and writing, or for reading only.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct OpenOptions {
    /// Create the file if it does not exist.
    pub create: bool,
    /// Truncate the file to zero length.
    pub truncate: bool,
    /// Open for reading only: writes fail, and the file may be read-only on
    /// disk (a configuration file). `create` and `truncate` are then invalid.
    pub read_only: bool,
}

/// A file system.
pub trait Disk {
    /// Files of this disk.
    type File: File;

    /// Opens (and optionally creates or truncates) a file.
    async fn open(&self, path: &Path, options: OpenOptions) -> io::Result<Self::File>;

    /// Removes a file.
    async fn remove(&self, path: &Path) -> io::Result<()>;

    /// Renames a file, replacing `to` if it exists.
    async fn rename(&self, from: &Path, to: &Path) -> io::Result<()>;

    /// Creates a directory and its missing parents.
    async fn create_dir_all(&self, path: &Path) -> io::Result<()>;

    /// Entries of a directory, sorted.
    async fn list_dir(&self, path: &Path) -> io::Result<Vec<PathBuf>>;

    /// Makes create, rename and remove operations in `dir` durable.
    async fn sync_dir(&self, dir: &Path) -> io::Result<()>;
}

/// An open file, accessed by position.
pub trait File {
    /// Reads at `offset` until `buf` is full or the end of the file is reached;
    /// returns the number of bytes read (short only at the end of the file).
    async fn read_at(&self, buf: &mut [u8], offset: u64) -> io::Result<usize>;

    /// Writes all of `buf` at `offset`; writing past the end leaves a zero-filled hole.
    async fn write_all_at(&self, buf: &[u8], offset: u64) -> io::Result<()>;

    /// Makes the file contents durable.
    async fn sync_data(&self) -> io::Result<()>;

    /// Current length in bytes.
    async fn len(&self) -> io::Result<u64>;

    /// Whether the file is empty.
    async fn is_empty(&self) -> io::Result<bool> {
        Ok(self.len().await? == 0)
    }

    /// Truncates or extends (with zeros) the file.
    async fn set_len(&self, len: u64) -> io::Result<()>;

    /// Reserves disk space up to `len` bytes without changing the contents:
    /// the length becomes `max(len, current length)`, never smaller. Like
    /// [`File::set_len`], the new length is durable after
    /// [`File::sync_data`].
    async fn allocate(&self, len: u64) -> io::Result<()>;
}
