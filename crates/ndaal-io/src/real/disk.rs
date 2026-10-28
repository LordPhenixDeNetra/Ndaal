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
        let file = blocking(move || open_blocking(&path, options)).await?;
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
        blocking(move || list_dir_blocking(&path)).await
    }

    async fn sync_dir(&self, dir: &Path) -> io::Result<()> {
        let dir = dir.to_owned();
        blocking(move || sync_dir_blocking(&dir)).await
    }
}

pub(super) fn open_blocking(path: &Path, options: OpenOptions) -> io::Result<std::fs::File> {
    std::fs::OpenOptions::new()
        .read(true)
        .write(!options.read_only)
        .create(options.create)
        .truncate(options.truncate)
        .open(path)
}

/// Entries of `path`, sorted.
pub(super) fn list_dir_blocking(path: &Path) -> io::Result<Vec<PathBuf>> {
    let mut entries = std::fs::read_dir(path)?
        .map(|entry| entry.map(|e| e.path()))
        .collect::<io::Result<Vec<_>>>()?;
    entries.sort();
    Ok(entries)
}

#[cfg(unix)]
pub(super) fn sync_dir_blocking(dir: &Path) -> io::Result<()> {
    std::fs::File::open(dir)?.sync_all()
}

/// Windows has no directory fsync; NTFS journals metadata itself.
#[cfg(not(unix))]
pub(super) fn sync_dir_blocking(_dir: &Path) -> io::Result<()> {
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

    async fn allocate(&self, len: u64) -> io::Result<()> {
        let file = Arc::clone(&self.file);
        blocking(move || allocate_blocking(&file, len)).await
    }
}

/// `fallocate`: the blocks are reserved, so later writes cannot fail for
/// lack of space. Falls back to [`extend_to`] on file systems without it.
#[cfg(target_os = "linux")]
pub(super) fn allocate_blocking(file: &std::fs::File, len: u64) -> io::Result<()> {
    use std::os::fd::AsRawFd;
    if len == 0 {
        // fallocate refuses an empty range (EINVAL); there is nothing to do.
        return Ok(());
    }
    let len_off =
        libc::off_t::try_from(len).map_err(|_| io::Error::from(io::ErrorKind::InvalidInput))?;
    loop {
        // SAFETY: `file` owns the descriptor and outlives the call;
        // fallocate takes integers only and touches no memory of ours.
        let rc = unsafe { libc::fallocate(file.as_raw_fd(), 0, 0, len_off) };
        if rc == 0 {
            return Ok(());
        }
        let error = io::Error::last_os_error();
        match error.raw_os_error() {
            Some(libc::EINTR) => {}
            Some(libc::EOPNOTSUPP) => return extend_to(file, len),
            _ => return Err(error),
        }
    }
}

/// Other systems: extending the length is the closest portable equivalent.
#[cfg(not(target_os = "linux"))]
pub(super) fn allocate_blocking(file: &std::fs::File, len: u64) -> io::Result<()> {
    extend_to(file, len)
}

/// Extends the file with zeros up to `len`; never truncates.
fn extend_to(file: &std::fs::File, len: u64) -> io::Result<()> {
    if file.metadata()?.len() < len {
        file.set_len(len)?;
    }
    Ok(())
}

/// Reads until `buf` is full or end of file; returns the bytes read.
pub(super) fn read_full_at(file: &std::fs::File, buf: &mut [u8], offset: u64) -> io::Result<usize> {
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

pub(super) fn write_full_at(file: &std::fs::File, buf: &[u8], offset: u64) -> io::Result<()> {
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
