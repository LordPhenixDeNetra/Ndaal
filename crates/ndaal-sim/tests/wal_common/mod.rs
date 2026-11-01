//! Shared by the log writer tests: a writer on a simulated device, a disk
//! that records every flush, and reading the log back.

#![allow(dead_code)] // each test binary uses a subset

use std::borrow::Cow;
use std::cell::{Cell, RefCell};
use std::future::Future;
use std::io;
use std::path::{Path, PathBuf};
use std::pin::pin;
use std::rc::Rc;
use std::task::{Context, Poll, Waker};
use std::time::Duration;

use ndaal_common::ids::{Lsn, NodeId, PartitionId};
use ndaal_io::sched::{IoClass, SchedulerConfig};
use ndaal_io::sim::{DeviceDisk, DeviceFile, DeviceModel, Policy, SimClock, SimDevice, SimDisk};
use ndaal_io::{Disk, File, OpenOptions};
use ndaal_observe::metrics::Registry;
use ndaal_sim::{Limits, Outcome, Sim};
use ndaal_wal::{
    FileReader, Position, Reassembler, Record, RecordType, Step, WalError, WalWriter, WriterConfig,
};

pub const LIMITS: Limits = Limits {
    max_steps: 5_000_000,
    max_virtual_time: Duration::from_secs(60),
};

/// Where the log lives on the simulated disk.
pub const DIR: &str = "/wal";
/// Small files, so tests roll over quickly.
pub const FILE_SIZE: u64 = 4 << 20;

pub fn config() -> WriterConfig {
    WriterConfig {
        file_size: FILE_SIZE,
        group_commit_max_bytes: 256 * 1024,
        group_commit_max_delay: Duration::from_micros(200),
        async_flush_interval: Duration::from_millis(100),
    }
}

pub fn record(lsn: u64, size: usize) -> Record<'static> {
    Record {
        kind: RecordType::KvEffects,
        partition: PartitionId(1),
        lsn: Lsn(lsn),
        term: 0,
        timestamp: lsn,
        payload: Cow::Owned(vec![(lsn % 251) as u8; size]),
    }
}

/// Runs a future that never waits: the simulated disk answers at once.
pub fn now<F: Future>(future: F) -> F::Output {
    let mut future = pin!(future);
    match future
        .as_mut()
        .poll(&mut Context::from_waker(Waker::noop()))
    {
        Poll::Ready(output) => output,
        Poll::Pending => panic!("a simulated disk operation waited"),
    }
}

/// One flush seen by the device.
#[derive(Debug, Clone)]
pub struct Write {
    pub path: PathBuf,
    pub offset: u64,
    pub len: u64,
    pub done: bool,
}

/// Records every write-and-sync, and checks there is never two at once.
#[derive(Debug, Default, Clone)]
pub struct Recorder {
    writes: Rc<RefCell<Vec<Write>>>,
    inflight: Rc<Cell<u32>>,
    fail_next: Rc<Cell<Option<io::ErrorKind>>>,
    /// Cuts the power on this disk just before a free file is renamed to
    /// its number (after its header was written and synced); the name of
    /// that free file is kept.
    crash_before_rename: Rc<RefCell<Option<SimDisk>>>,
    pub crashed_on: Rc<RefCell<Option<String>>>,
}

impl Recorder {
    /// Every write: page-aligned, at most `max` bytes.
    pub fn check_writes(&self, max: usize) {
        for w in self.writes.borrow().iter() {
            assert_eq!(w.offset % 4096, 0, "{w:?}");
            assert_eq!(w.len % 4096, 0, "{w:?}");
            assert!(w.len as usize <= max, "{w:?}");
        }
    }

    /// `at` was covered by a write that finished.
    pub fn check_durable(&self, at: Position) {
        let covered = self.writes.borrow().iter().any(|w| {
            // Files are opened under their free name, then renamed.
            let name = w.path.file_name().map(|n| n.to_string_lossy().into_owned());
            let name = name
                .as_deref()
                .map(|n| n.trim_start_matches("free-").to_owned());
            w.done
                && name == Some(format!("{:016x}.wal", at.file_no))
                && w.offset + w.len >= at.offset
        });
        assert!(covered, "{at:?} reported durable without a finished sync");
    }
}

/// A device disk that records flushes and can fail the next one.
#[derive(Clone)]
pub struct TestDisk {
    inner: DeviceDisk,
    recorder: Recorder,
}

pub struct TestFile {
    inner: DeviceFile,
    path: PathBuf,
    recorder: Recorder,
}

impl Disk for TestDisk {
    type File = TestFile;

    async fn open(&self, path: &Path, options: OpenOptions) -> io::Result<TestFile> {
        Ok(TestFile {
            inner: self.inner.open(path, options).await?,
            path: path.to_owned(),
            recorder: self.recorder.clone(),
        })
    }
    async fn remove(&self, path: &Path) -> io::Result<()> {
        self.inner.remove(path).await
    }
    async fn rename(&self, from: &Path, to: &Path) -> io::Result<()> {
        let name = from.file_name().map(|n| n.to_string_lossy().into_owned());
        let free = name.as_deref().is_some_and(|n| n.starts_with("free-"));
        let armed = if free {
            self.recorder.crash_before_rename.borrow_mut().take()
        } else {
            None
        };
        if let Some(disk) = armed {
            disk.crash(ndaal_io::sim::CrashMode::LoseAllUnsynced);
            *self.recorder.crashed_on.borrow_mut() = name;
            return Err(io::ErrorKind::Other.into());
        }
        self.inner.rename(from, to).await
    }
    async fn create_dir_all(&self, path: &Path) -> io::Result<()> {
        self.inner.create_dir_all(path).await
    }
    async fn list_dir(&self, path: &Path) -> io::Result<Vec<PathBuf>> {
        self.inner.list_dir(path).await
    }
    async fn sync_dir(&self, dir: &Path) -> io::Result<()> {
        self.inner.sync_dir(dir).await
    }
}

impl File for TestFile {
    async fn read_at(&self, buf: &mut [u8], offset: u64) -> io::Result<usize> {
        self.inner.read_at(buf, offset).await
    }
    async fn write_all_at(&self, buf: &[u8], offset: u64) -> io::Result<()> {
        self.inner.write_all_at(buf, offset).await
    }
    async fn sync_data(&self) -> io::Result<()> {
        self.inner.sync_data().await
    }
    async fn len(&self) -> io::Result<u64> {
        self.inner.len().await
    }
    async fn set_len(&self, len: u64) -> io::Result<()> {
        self.inner.set_len(len).await
    }
    async fn allocate(&self, len: u64) -> io::Result<()> {
        self.inner.allocate(len).await
    }
    async fn write_all_sync_at(&self, buf: &[u8], offset: u64) -> io::Result<()> {
        let rec = &self.recorder;
        assert_eq!(rec.inflight.get(), 0, "two flushes at once");
        rec.inflight.set(1);
        let index = {
            let mut writes = rec.writes.borrow_mut();
            writes.push(Write {
                path: self.path.clone(),
                offset,
                len: buf.len() as u64,
                done: false,
            });
            writes.len() - 1
        };
        let result = match rec.fail_next.take() {
            Some(kind) => Err(io::Error::from(kind)),
            None => self.inner.write_all_sync_at(buf, offset).await,
        };
        rec.inflight.set(0);
        if result.is_ok() {
            rec.writes.borrow_mut()[index].done = true;
        }
        result
    }
}

/// A writer on a simulated device, its flusher spawned on `sim`.
pub struct Setup {
    writer: RefCell<Option<WalWriter<TestDisk, SimClock>>>,
    pub disk: SimDisk,
    pub recorder: Recorder,
    /// Metrics of the writer.
    pub registry: Registry,
}

impl Setup {
    pub fn new(sim: &Sim, latency: Duration) -> Setup {
        Setup::recording(sim, latency, Recorder::default())
    }

    pub fn recording(sim: &Sim, latency: Duration, recorder: Recorder) -> Setup {
        let disk = sim.disk("wal");
        Setup::on_disk(sim, &disk, latency, recorder).expect("open the log")
    }

    /// A new writer on the log already on `disk` (after a crash, say).
    pub fn reopen(sim: &Sim, disk: &SimDisk, latency: Duration) -> Setup {
        Setup::on_disk(sim, disk, latency, Recorder::default()).expect("reopen the log")
    }

    /// Opening the log on `disk`, only for its outcome.
    pub fn try_reopen(sim: &Sim, disk: &SimDisk) -> Result<(), WalError> {
        Setup::on_disk(sim, disk, Duration::from_micros(100), Recorder::default()).map(drop)
    }

    fn on_disk(
        sim: &Sim,
        disk: &SimDisk,
        latency: Duration,
        recorder: Recorder,
    ) -> Result<Setup, WalError> {
        let model = DeviceModel {
            latency,
            bytes_per_second: 500 << 20,
        };
        let scheduler = SchedulerConfig {
            depth: 4,
            background_share: 0.3,
        };
        let device = SimDevice::new(
            sim.clock(),
            sim.rng("device"),
            model,
            scheduler,
            Policy::Priority,
        );
        let test_disk = TestDisk {
            inner: DeviceDisk::new(disk.clone(), device, IoClass::Critical),
            recorder: recorder.clone(),
        };
        let opened = Rc::new(RefCell::new(None));
        let registry = Registry::default();
        let (o, clock, reg) = (Rc::clone(&opened), sim.clock(), registry.clone());
        sim.spawn(async move {
            let result = WalWriter::open(
                test_disk,
                Path::new(DIR),
                0,
                NodeId([9; 16]),
                config(),
                clock,
                &reg,
            )
            .await;
            *o.borrow_mut() = Some(result);
        });
        assert_eq!(sim.run(LIMITS), Outcome::Finished);
        let (writer, flusher) = opened.borrow_mut().take().expect("opened")?;
        sim.spawn(flusher.run());
        Ok(Setup {
            writer: RefCell::new(Some(writer)),
            disk: disk.clone(),
            recorder,
            registry,
        })
    }

    /// The writer, once: when the test drops it, the flusher ends.
    pub fn writer(&self) -> WalWriter<TestDisk, SimClock> {
        self.writer.borrow_mut().take().expect("writer taken once")
    }

    /// A handle that arms the power cut before the next rename of a free
    /// file to its number; usable from inside a simulated task.
    pub fn crash_hook(&self) -> CrashHook {
        CrashHook {
            slot: Rc::clone(&self.recorder.crash_before_rename),
            disk: self.disk.clone(),
        }
    }

    pub fn fail_next_write(&self, kind: io::ErrorKind) {
        self.recorder.fail_next.set(Some(kind));
    }
}

/// `(lsn, payload length)` of every record in the log, files in order.
pub fn read_back(disk: &SimDisk) -> Vec<(u64, usize)> {
    let dir = Path::new(DIR).join("shard-00");
    let files = now(disk.list_dir(&dir)).expect("list");
    let mut glue = Reassembler::default();
    let mut out = Vec::new();
    for path in files
        .iter()
        .filter(|p| p.extension().is_some_and(|e| e == "wal"))
    {
        if path
            .file_name()
            .is_some_and(|n| n.to_string_lossy().starts_with("free-"))
        {
            continue;
        }
        let file = now(disk.open(path, OpenOptions::default())).expect("open");
        let len = now(file.len()).expect("len");
        let mut bytes = vec![0; len as usize];
        now(file.read_at(&mut bytes, 0)).expect("read");
        let mut reader = FileReader::new(&bytes).expect("header");
        while let Ok(Step::Frame(frame)) = reader.next_step() {
            if let Some(r) = glue.push(frame).expect("glue") {
                out.push((r.lsn.0, r.payload.len()));
            }
        }
    }
    out
}

/// Appends `record`; when the writer is busy, waits for what is pending to
/// be durable, then retries (what a shard does on `Busy`).
pub async fn append_retrying(
    writer: &WalWriter<TestDisk, SimClock>,
    record: &Record<'_>,
    last: Option<Position>,
) -> Result<Position, WalError> {
    loop {
        match writer.append(record) {
            Err(WalError::Busy) => {
                let at = last.expect("busy with nothing appended");
                writer.durable(at).await?;
            }
            other => return other,
        }
    }
}

/// See [`Setup::crash_hook`].
pub struct CrashHook {
    slot: Rc<RefCell<Option<SimDisk>>>,
    disk: SimDisk,
}

impl CrashHook {
    pub fn arm(&self) {
        *self.slot.borrow_mut() = Some(self.disk.clone());
    }
}
