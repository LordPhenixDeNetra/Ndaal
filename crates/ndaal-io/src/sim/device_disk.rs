//! A [`SimDisk`] behind a [`SimDevice`]: the same files, faults and
//! crashes, with every operation scheduled and timed by the device.

use std::io;
use std::path::{Path, PathBuf};

use super::{SimDevice, SimDisk, SimFile};
use crate::sched::{IoClass, UNSIZED_COST};
use crate::{Disk, File, OpenOptions};

/// A simulated disk whose operations go through a device. Directory
/// operations (rename, remove, list, sync) run as `Critical`: they are
/// rare, and the log and the manifest depend on them.
#[derive(Clone)]
pub struct DeviceDisk {
    disk: SimDisk,
    device: SimDevice,
    class: IoClass,
}

/// A file of a [`DeviceDisk`]; its operations carry the class it was opened
/// with.
pub struct DeviceFile {
    file: SimFile,
    device: SimDevice,
    class: IoClass,
}

/// Bytes an operation on a buffer of `len` bytes moves.
fn cost(len: usize) -> u64 {
    u64::try_from(len).unwrap_or(u64::MAX)
}

impl DeviceDisk {
    /// `disk` behind `device`; [`Disk::open`] opens files of `class`.
    pub fn new(disk: SimDisk, device: SimDevice, class: IoClass) -> Self {
        Self {
            disk,
            device,
            class,
        }
    }

    /// Opens a file whose operations run as `class`.
    pub async fn open_class(
        &self,
        path: &Path,
        options: OpenOptions,
        class: IoClass,
    ) -> io::Result<DeviceFile> {
        let file = self
            .device
            .run(class, UNSIZED_COST, self.disk.open(path, options))
            .await?;
        Ok(DeviceFile {
            file,
            device: self.device.clone(),
            class,
        })
    }

    async fn critical<T>(&self, work: impl Future<Output = T>) -> T {
        self.device.run(IoClass::Critical, UNSIZED_COST, work).await
    }
}

impl Disk for DeviceDisk {
    type File = DeviceFile;

    async fn open(&self, path: &Path, options: OpenOptions) -> io::Result<DeviceFile> {
        self.open_class(path, options, self.class).await
    }

    async fn remove(&self, path: &Path) -> io::Result<()> {
        self.critical(self.disk.remove(path)).await
    }

    async fn rename(&self, from: &Path, to: &Path) -> io::Result<()> {
        self.critical(self.disk.rename(from, to)).await
    }

    async fn create_dir_all(&self, path: &Path) -> io::Result<()> {
        self.critical(self.disk.create_dir_all(path)).await
    }

    async fn list_dir(&self, path: &Path) -> io::Result<Vec<PathBuf>> {
        self.critical(self.disk.list_dir(path)).await
    }

    async fn sync_dir(&self, dir: &Path) -> io::Result<()> {
        self.critical(self.disk.sync_dir(dir)).await
    }
}

impl File for DeviceFile {
    async fn read_at(&self, buf: &mut [u8], offset: u64) -> io::Result<usize> {
        let size = cost(buf.len());
        self.device
            .run(self.class, size, self.file.read_at(buf, offset))
            .await
    }

    async fn write_all_at(&self, buf: &[u8], offset: u64) -> io::Result<()> {
        self.device
            .run(
                self.class,
                cost(buf.len()),
                self.file.write_all_at(buf, offset),
            )
            .await
    }

    async fn sync_data(&self) -> io::Result<()> {
        self.device
            .run(self.class, UNSIZED_COST, self.file.sync_data())
            .await
    }

    async fn len(&self) -> io::Result<u64> {
        // Metadata from the page cache: no device time.
        self.file.len().await
    }

    async fn set_len(&self, len: u64) -> io::Result<()> {
        self.device
            .run(self.class, UNSIZED_COST, self.file.set_len(len))
            .await
    }

    async fn allocate(&self, len: u64) -> io::Result<()> {
        self.device
            .run(self.class, UNSIZED_COST, self.file.allocate(len))
            .await
    }
}
