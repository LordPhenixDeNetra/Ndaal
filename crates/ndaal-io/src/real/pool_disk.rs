//! The real file system through a device's [`IoPool`]: every operation is
//! scheduled by class (T02.06). [`RealDisk`](super::RealDisk) stays on
//! tokio's blocking threads for the CLI and the tools.

use std::io;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::Arc;

use super::IoSubmitter;
use super::disk::{
    allocate_blocking, list_dir_blocking, open_blocking, read_full_at, sync_dir_blocking,
    write_full_at,
};
use super::pool_metrics::IoKind;
use crate::sched::{IoClass, UNSIZED_COST};
use crate::{Disk, File, OpenOptions};

/// A real disk whose operations go through a pool. It lives on one core
/// (not `Send`); directory operations run as `Critical`.
#[derive(Clone)]
pub struct PoolDisk {
    submitter: Rc<IoSubmitter>,
    class: IoClass,
}

/// A real file of a [`PoolDisk`], with the class it was opened with.
#[derive(Clone)]
pub struct PoolFile {
    file: Arc<std::fs::File>,
    submitter: Rc<IoSubmitter>,
    class: IoClass,
}

/// Bytes an operation on a buffer of `len` bytes moves.
fn cost(len: usize) -> u64 {
    u64::try_from(len).unwrap_or(u64::MAX)
}

async fn run<T, F>(
    submitter: &IoSubmitter,
    class: IoClass,
    kind: IoKind,
    cost: u64,
    work: F,
) -> io::Result<T>
where
    T: Send + 'static,
    F: FnOnce() -> io::Result<T> + Send + 'static,
{
    submitter.submit(class, kind, cost, work)?.await
}

impl PoolDisk {
    /// A disk on `submitter`'s pool; [`Disk::open`] opens files of `class`.
    pub fn new(submitter: IoSubmitter, class: IoClass) -> Self {
        Self {
            submitter: Rc::new(submitter),
            class,
        }
    }

    /// Opens a file whose operations run as `class`.
    pub async fn open_class(
        &self,
        path: &Path,
        options: OpenOptions,
        class: IoClass,
    ) -> io::Result<PoolFile> {
        let path = path.to_owned();
        let work = move || open_blocking(&path, options);
        let file = run(&self.submitter, class, IoKind::Meta, UNSIZED_COST, work).await?;
        Ok(PoolFile {
            file: Arc::new(file),
            submitter: Rc::clone(&self.submitter),
            class,
        })
    }

    async fn critical<T, F>(&self, kind: IoKind, work: F) -> io::Result<T>
    where
        T: Send + 'static,
        F: FnOnce() -> io::Result<T> + Send + 'static,
    {
        run(&self.submitter, IoClass::Critical, kind, UNSIZED_COST, work).await
    }
}

impl Disk for PoolDisk {
    type File = PoolFile;

    async fn open(&self, path: &Path, options: OpenOptions) -> io::Result<PoolFile> {
        self.open_class(path, options, self.class).await
    }

    async fn remove(&self, path: &Path) -> io::Result<()> {
        let path = path.to_owned();
        self.critical(IoKind::Meta, move || std::fs::remove_file(path))
            .await
    }

    async fn rename(&self, from: &Path, to: &Path) -> io::Result<()> {
        let (from, to) = (from.to_owned(), to.to_owned());
        self.critical(IoKind::Meta, move || std::fs::rename(from, to))
            .await
    }

    async fn create_dir_all(&self, path: &Path) -> io::Result<()> {
        let path = path.to_owned();
        self.critical(IoKind::Meta, move || std::fs::create_dir_all(path))
            .await
    }

    async fn list_dir(&self, path: &Path) -> io::Result<Vec<PathBuf>> {
        let path = path.to_owned();
        self.critical(IoKind::Meta, move || list_dir_blocking(&path))
            .await
    }

    async fn sync_dir(&self, dir: &Path) -> io::Result<()> {
        let dir = dir.to_owned();
        self.critical(IoKind::Sync, move || sync_dir_blocking(&dir))
            .await
    }
}

impl PoolFile {
    async fn run<T, F>(&self, kind: IoKind, cost: u64, work: F) -> io::Result<T>
    where
        T: Send + 'static,
        F: FnOnce(&std::fs::File) -> io::Result<T> + Send + 'static,
    {
        let file = Arc::clone(&self.file);
        run(&self.submitter, self.class, kind, cost, move || work(&file)).await
    }
}

impl File for PoolFile {
    async fn read_at(&self, buf: &mut [u8], offset: u64) -> io::Result<usize> {
        let len = buf.len();
        let data = self
            .run(IoKind::Read, cost(len), move |file| {
                let mut data = vec![0u8; len];
                let n = read_full_at(file, &mut data, offset)?;
                data.truncate(n);
                Ok(data)
            })
            .await?;
        buf[..data.len()].copy_from_slice(&data);
        Ok(data.len())
    }

    async fn write_all_at(&self, buf: &[u8], offset: u64) -> io::Result<()> {
        let data = buf.to_vec();
        self.run(IoKind::Write, cost(data.len()), move |file| {
            write_full_at(file, &data, offset)
        })
        .await
    }

    async fn sync_data(&self) -> io::Result<()> {
        self.run(IoKind::Sync, UNSIZED_COST, std::fs::File::sync_data)
            .await
    }

    async fn len(&self) -> io::Result<u64> {
        self.run(IoKind::Meta, UNSIZED_COST, |file| {
            Ok(file.metadata()?.len())
        })
        .await
    }

    async fn set_len(&self, len: u64) -> io::Result<()> {
        self.run(IoKind::Meta, UNSIZED_COST, move |file| file.set_len(len))
            .await
    }

    async fn allocate(&self, len: u64) -> io::Result<()> {
        self.run(IoKind::Meta, UNSIZED_COST, move |file| {
            allocate_blocking(file, len)
        })
        .await
    }
}
