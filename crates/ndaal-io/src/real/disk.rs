//! `std::fs`, run on tokio's blocking thread pool (ADR-016, thread-pool backend).
//!
//! Buffers are copied into and out of the blocking task: the io_uring backend
//! (T07.01) will remove the copies.

use std::io;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::{Disk, File, OpenOptions};

/// The real file system.
#[derive(Debug, Clone, Copy, Default)]
pub struct RealDisk;

/// A real open file.
#[derive(Debug, Clone)]
pub struct RealFile {
    file: Arc<std::fs::File>,
}

/// Runs blocking file-system work off the calling thread.
async fn blocking<T, F>(work: F) -> io::Result<T>
where
    F: FnOnce() -> io::Result<T> + Send + 'static,
    T: Send + 'static,
{
    tokio::task::spawn_blocking(work)
        .await
        .map_err(|e| io::Error::other(format!("blocking task failed: {e}")))?
}

impl Disk for RealDisk {
    type File = RealFile;

    async fn open(&self, path: &Path, options: OpenOptions) -> io::Result<RealFile> {
        let path = path.to_owned();
        let file = blocking(move || {
            std::fs::OpenOptions::new()
                .read(true)
                .write(true)
                .create(options.create)
                .truncate(options.truncate)
                .open(path)
        })
        .await?;
        Ok(RealFile {
            file: Arc::new(file),
        })
    }

    async fn remove(&self, path: &Path) -> io::Result<()> {
        let path = path.to_owned();
        blocking(move || std::fs::remove_file(path)).await
    }

    async fn rename(&self, from: &Path, to: &Path) -> io::Result<()> {
        let (from, to) = (from.to_owned(), to.to_owned());
        blocking(move || std::fs::rename(from, to)).await
    }

    async fn create_dir_all(&self, path: &Path) -> io::Result<()> {
        let path = path.to_owned();
        blocking(move || std::fs::create_dir_all(path)).await
    }

    async fn list_dir(&self, path: &Path) -> io::Result<Vec<PathBuf>> {
        let path = path.to_owned();
        blocking(move || {
            let mut entries = std::fs::read_dir(path)?
                .map(|entry| entry.map(|e| e.path()))
                .collect::<io::Result<Vec<_>>>()?;
            entries.sort();
            Ok(entries)
        })
        .await
    }

    async fn sync_dir(&self, dir: &Path) -> io::Result<()> {
        let dir = dir.to_owned();
        blocking(move || sync_dir_blocking(&dir)).await
    }
}

#[cfg(unix)]
fn sync_dir_blocking(dir: &Path) -> io::Result<()> {
    std::fs::File::open(dir)?.sync_all()
}

/// Windows has no directory fsync; NTFS journals metadata itself.
#[cfg(not(unix))]
fn sync_dir_blocking(_dir: &Path) -> io::Result<()> {
    Ok(())
}

impl File for RealFile {
    async fn read_at(&self, buf: &mut [u8], offset: u64) -> io::Result<usize> {
        let file = Arc::clone(&self.file);
        let len = buf.len();
        let data = blocking(move || {
            let mut data = vec![0u8; len];
            let n = read_full_at(&file, &mut data, offset)?;
            data.truncate(n);
            Ok(data)
        })
        .await?;
        buf[..data.len()].copy_from_slice(&data);
        Ok(data.len())
    }

    async fn write_all_at(&self, buf: &[u8], offset: u64) -> io::Result<()> {
        let file = Arc::clone(&self.file);
        let data = buf.to_vec();
        blocking(move || write_full_at(&file, &data, offset)).await
    }

    async fn sync_data(&self) -> io::Result<()> {
        let file = Arc::clone(&self.file);
        blocking(move || file.sync_data()).await
    }

    async fn len(&self) -> io::Result<u64> {
        let file = Arc::clone(&self.file);
        blocking(move || Ok(file.metadata()?.len())).await
    }

    async fn set_len(&self, len: u64) -> io::Result<()> {
        let file = Arc::clone(&self.file);
        blocking(move || file.set_len(len)).await
    }
}

/// Reads until `buf` is full or end of file; returns the bytes read.
fn read_full_at(file: &std::fs::File, buf: &mut [u8], offset: u64) -> io::Result<usize> {
    let mut done = 0;
    while done < buf.len() {
        match read_once_at(file, &mut buf[done..], offset + done as u64) {
            Ok(0) => break,
            Ok(n) => done += n,
            Err(e) if e.kind() == io::ErrorKind::Interrupted => {}
            Err(e) => return Err(e),
        }
    }
    Ok(done)
}

fn write_full_at(file: &std::fs::File, buf: &[u8], offset: u64) -> io::Result<()> {
    let mut done = 0;
    while done < buf.len() {
        match write_once_at(file, &buf[done..], offset + done as u64) {
            Ok(0) => return Err(io::Error::from(io::ErrorKind::WriteZero)),
            Ok(n) => done += n,
            Err(e) if e.kind() == io::ErrorKind::Interrupted => {}
            Err(e) => return Err(e),
        }
    }
    Ok(())
}

#[cfg(unix)]
fn read_once_at(file: &std::fs::File, buf: &mut [u8], offset: u64) -> io::Result<usize> {
    std::os::unix::fs::FileExt::read_at(file, buf, offset)
}

#[cfg(unix)]
fn write_once_at(file: &std::fs::File, buf: &[u8], offset: u64) -> io::Result<usize> {
    std::os::unix::fs::FileExt::write_at(file, buf, offset)
}

#[cfg(windows)]
fn read_once_at(file: &std::fs::File, buf: &mut [u8], offset: u64) -> io::Result<usize> {
    std::os::windows::fs::FileExt::seek_read(file, buf, offset)
}

#[cfg(windows)]
fn write_once_at(file: &std::fs::File, buf: &[u8], offset: u64) -> io::Result<usize> {
    std::os::windows::fs::FileExt::seek_write(file, buf, offset)
}
