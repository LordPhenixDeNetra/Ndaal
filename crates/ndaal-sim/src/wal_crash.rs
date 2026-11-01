//! A power cut while the log is written in its three durability modes
//! (T02.11, ENF-DUR-01, ENF-DUR-04). Writers log records as `Sync`,
//! `Async` and `None`, a few of them larger than a fragment; another task
//! releases old files, so files roll over and get recycled; the disk
//! crashes at an instant drawn from the seed, losing or tearing what was
//! not synced; the log is reopened and read.
//!
//! Allowed losses: no acknowledged `Sync` record; no `Async` record older
//! than the async flush interval (plus one flush) at the cut; every `None`
//! record (none is ever in the log). Records of released files are gone by
//! design and not counted.

use std::cell::RefCell;
use std::collections::BTreeSet;
use std::future::Future;
use std::path::Path;
use std::pin::pin;
use std::rc::Rc;
use std::task::{Context, Poll, Waker};
use std::time::Duration;

use ndaal_common::ids::{Lsn, NodeId, PartitionId};
use ndaal_io::sched::{IoClass, SchedulerConfig};
use ndaal_io::sim::{CrashMode, DeviceDisk, DeviceModel, Policy, SimClock, SimDevice, SimDisk};
use ndaal_io::{Clock, Disk, File, OpenOptions, Rng};
use ndaal_observe::metrics::Registry;
use ndaal_wal::{
    Durability, FileReader, Flusher, Reassembler, Record, RecordType, Step, WalError, WalWriter,
    WriterConfig,
};

use crate::sweep::SeedResult;
use crate::{Limits, Outcome, Sim};

const LIMITS: Limits = Limits {
    max_steps: 5_000_000,
    max_virtual_time: Duration::from_secs(60),
};
const DIR: &str = "/wal";
/// The async flush interval the checks hold the writer to.
const ASYNC_INTERVAL: Duration = Duration::from_millis(20);
const CONFIG: WriterConfig = WriterConfig {
    // Small files: cheap to copy at each cut, and rollovers happen; still
    // larger than a fragment and its header.
    file_size: 2 << 20,
    group_commit_max_bytes: 256 * 1024,
    group_commit_max_delay: Duration::from_micros(200),
    async_flush_interval: ASYNC_INTERVAL,
};
/// Time an `Async` record may still need after the interval: the flush in
/// flight when it was appended, then its own. On the device below a flush
/// of at most 2 MiB takes up to 11.3 ms (2 MiB at 200 MiB/s, 0.3 ms of
/// latency, +10 % of variation): two of them, rounded up.
const ASYNC_SLACK: Duration = Duration::from_millis(25);
const MODEL: DeviceModel = DeviceModel {
    latency: Duration::from_micros(300),
    bytes_per_second: 200 << 20,
};
/// The cut comes between these times after the start: late enough that
/// `Async` records older than the window exist.
const CRASH_FROM_MICROS: u64 = 10_000;
const CRASH_WITHIN_MICROS: u64 = 120_000;
/// Records logged in each mode, at most.
const RECORDS: u64 = 1_000;
/// One record in this many is larger than a fragment.
const LARGE_EVERY: u64 = 200;
const LARGE: usize = 1_200_000;
/// How often an old file is released.
const RELEASE_EVERY: Duration = Duration::from_millis(15);
/// Files released at most: enough to recycle under a cut, few enough that
/// most acknowledged records stay checked.
const RELEASES: u64 = 2;

type Writer = WalWriter<DeviceDisk, SimClock>;

/// A deliberately wrong writer, to show the checks bite.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Control {
    /// The real writer.
    Honest,
    /// `Sync` answers once appended, without waiting for the sync.
    LyingSync,
    /// `Async` records may wait 10 s instead of the interval, and up to
    /// 2 MiB pending (no `Sync` writer or large record forces flushes).
    LazyAsync,
}

/// What happened during one run of [`wal_crash`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WalCrashReport {
    /// How the run before the cut and the reopening ended.
    pub outcome: Outcome,
    /// `Sync` records acknowledged before the cut.
    pub acked_sync: u32,
    /// Of those, the ones missing after the restart: must be 0.
    pub lost_sync: u32,
    /// `Async` records older than the window at the cut: the ones checked.
    pub checked_async: u32,
    /// Of those, the ones missing: must be 0.
    pub lost_async_beyond_window: u32,
    /// Files released (recycled later) before the cut.
    pub released: u32,
    /// The log could not be read back (a fragment error, a bad file).
    pub unreadable: bool,
    /// Trace fingerprint.
    pub fingerprint: u64,
}

/// Partition of the records of each mode, to tell them apart on reading.
fn partition(mode: Durability) -> PartitionId {
    PartitionId(match mode {
        Durability::Sync => 1,
        Durability::Async => 2,
        Durability::None => 3,
    })
}

fn record(mode: Durability, lsn: u64, size: usize) -> Record<'static> {
    Record {
        kind: RecordType::KvEffects,
        partition: partition(mode),
        lsn: Lsn(lsn),
        term: 0,
        timestamp: lsn,
        payload: vec![(lsn % 251) as u8; size].into(),
    }
}

/// Runs a future that never waits: the simulated disk answers at once.
fn now<F: Future>(future: F) -> F::Output {
    let mut future = pin!(future);
    match future
        .as_mut()
        .poll(&mut Context::from_waker(Waker::noop()))
    {
        Poll::Ready(output) => output,
        Poll::Pending => panic!("a simulated disk operation waited"),
    }
}

fn open(
    sim: &Sim,
    disk: &SimDisk,
    config: WriterConfig,
) -> Result<(Writer, Flusher<DeviceDisk, SimClock>), WalError> {
    let scheduler = SchedulerConfig {
        depth: 4,
        background_share: 0.3,
    };
    let device = SimDevice::new(
        sim.clock(),
        sim.rng("device"),
        MODEL,
        scheduler,
        Policy::Priority,
    );
    let disk = DeviceDisk::new(disk.clone(), device, IoClass::Critical);
    let opened = Rc::new(RefCell::new(None));
    let (o, clock) = (Rc::clone(&opened), sim.clock());
    sim.spawn(async move {
        let result = WalWriter::open(
            disk,
            Path::new(DIR),
            0,
            NodeId([1; 16]),
            config,
            clock,
            &Registry::default(),
        )
        .await;
        *o.borrow_mut() = Some(result);
    });
    sim.run(LIMITS);
    opened.borrow_mut().take().unwrap_or(Err(WalError::Failed))
}

/// What the writers saw acknowledged before the cut: `(lsn, file)` and, for
/// `Async`, when.
#[derive(Default)]
struct Acks {
    sync: Vec<(u64, u64)>,
    asynchronous: Vec<(u64, u64, Duration)>,
    released: BTreeSet<u64>,
}

/// Logs records in `mode` until the log fails.
async fn log(
    writer: Writer,
    sim: Sim,
    mode: Durability,
    control: Control,
    acks: Rc<RefCell<Acks>>,
) {
    let clock = sim.clock();
    let mut rng = sim.rng(&format!("wal-crash-{mode:?}"));
    for lsn in 1..=RECORDS {
        // Large records start flushes by size: not in the lazy control.
        let size = if lsn % LARGE_EVERY == 0 && control != Control::LazyAsync {
            LARGE
        } else {
            50 + (rng.next_u64() % 3000) as usize
        };
        let rec = record(mode, lsn, size);
        let acked = if control == Control::LyingSync && mode == Durability::Sync {
            writer.append(&rec).map(Some)
        } else {
            writer.submit(&rec, mode).await
        };
        match acked {
            Ok(at) => {
                sim.trace(format!("{mode:?} {lsn}"));
                let mut a = acks.borrow_mut();
                match (mode, at) {
                    (Durability::Sync, Some(at)) => a.sync.push((lsn, at.file_no)),
                    (Durability::Async, Some(at)) => {
                        a.asynchronous.push((lsn, at.file_no, clock.monotonic()));
                    }
                    _ => {}
                }
            }
            Err(WalError::Busy) => {}
            Err(_) => return,
        }
        clock
            .sleep(Duration::from_micros(rng.next_u64() % 300))
            .await;
    }
}

/// Releases old files in order while the log runs, as checkpoints would.
async fn release(writer: Writer, sim: Sim, count: u64, acks: Rc<RefCell<Acks>>) {
    let clock = sim.clock();
    let mut next = 0;
    for _ in 0..10 {
        clock.sleep(RELEASE_EVERY).await;
        if next < count && writer.release(next).await.is_ok() {
            acks.borrow_mut().released.insert(next);
            sim.trace(format!("release {next}"));
            next += 1;
        }
    }
}

/// `(partition, lsn)` of every record in the log; `None` if a file or the
/// fragments cannot be read.
fn read_log(disk: &SimDisk) -> Option<BTreeSet<(u32, u64)>> {
    let dir = Path::new(DIR).join("shard-00");
    let mut names = now(disk.list_dir(&dir)).ok()?;
    names.retain(|p| {
        p.file_name()
            .and_then(|n| n.to_str())
            .is_some_and(|n| n.ends_with(".wal") && !n.starts_with("free-"))
    });
    let mut glue = Reassembler::default();
    let mut out = BTreeSet::new();
    for path in names {
        let file = now(disk.open(&path, OpenOptions::default())).ok()?;
        let len = now(file.len()).ok()?;
        let mut bytes = vec![0; usize::try_from(len).ok()?];
        now(file.read_at(&mut bytes, 0)).ok()?;
        let mut reader = FileReader::new(&bytes).ok()?;
        loop {
            match reader.next_step().ok()? {
                Step::Frame(frame) => {
                    if let Some(r) = glue.push(frame).ok()? {
                        out.insert((r.partition.0, r.lsn.0));
                    }
                }
                Step::End { .. } => break,
                Step::Corrupt { .. } => return None,
            }
        }
    }
    Some(out)
}

/// Runs the scenario with the writer `control` picks.
pub fn wal_crash_with(seed: u64, control: Control) -> WalCrashReport {
    let sim = Sim::new(seed);
    let disk = sim.disk("wal");
    let config = match control {
        // Lazy for real: no time limit, and a size threshold the writers
        // take long to reach.
        Control::LazyAsync => WriterConfig {
            async_flush_interval: Duration::from_secs(10),
            group_commit_max_bytes: 2 << 20,
            ..CONFIG
        },
        _ => CONFIG,
    };
    let Ok((writer, flusher)) = open(&sim, &disk, config) else {
        return failed(&sim);
    };
    sim.spawn(async move {
        let _ = flusher.run().await;
    });
    let acks = Rc::new(RefCell::new(Acks::default()));
    let modes: &[Durability] = match control {
        Control::LazyAsync => &[Durability::Async, Durability::None],
        _ => &[Durability::Sync, Durability::Async, Durability::None],
    };
    for &mode in modes {
        let (w, s, a) = (writer.clone(), sim.clone(), Rc::clone(&acks));
        sim.spawn(log(w, s, mode, control, a));
    }
    // Odd seeds recycle files under the cut; even ones keep every file, so
    // the oldest records, those the async window checks, stay counted.
    let releases = if seed % 2 == 1 { RELEASES } else { 0 };
    sim.spawn(release(
        writer.clone(),
        sim.clone(),
        releases,
        Rc::clone(&acks),
    ));
    drop(writer);
    let crash_at = Rc::new(RefCell::new(Duration::ZERO));
    let (s, d, c) = (sim.clone(), disk.clone(), Rc::clone(&crash_at));
    sim.spawn(async move {
        let mut rng = s.rng("wal-crash-cut");
        let clock = s.clock();
        let wait = CRASH_FROM_MICROS + rng.next_u64() % (CRASH_WITHIN_MICROS - CRASH_FROM_MICROS);
        clock.sleep(Duration::from_micros(wait)).await;
        let mode = if rng.next_u64().is_multiple_of(2) {
            CrashMode::Random
        } else {
            CrashMode::LoseAllUnsynced
        };
        *c.borrow_mut() = clock.monotonic();
        s.trace(format!("crash {mode:?}"));
        d.crash(mode);
    });
    let before = sim.run(LIMITS);
    // Reopening resumes the log (and erases a torn tail), as a restart does.
    let reopened = open(&sim, &disk, CONFIG).map(drop);
    let outcome = match (before, reopened) {
        (Outcome::Finished, Ok(())) => Outcome::Finished,
        (Outcome::Finished, Err(_)) => Outcome::Deadlock { blocked: 0 },
        (other, _) => other,
    };
    check(&sim, &disk, outcome, &acks.borrow(), *crash_at.borrow())
}

fn check(
    sim: &Sim,
    disk: &SimDisk,
    outcome: Outcome,
    acks: &Acks,
    cut: Duration,
) -> WalCrashReport {
    let log = read_log(disk);
    let in_log = |mode, lsn| {
        log.as_ref()
            .is_some_and(|l| l.contains(&(partition(mode).0, lsn)))
    };
    let kept = |file: &u64| !acks.released.contains(file);
    let lost_sync = acks
        .sync
        .iter()
        .filter(|(lsn, file)| kept(file) && !in_log(Durability::Sync, *lsn))
        .count();
    let window = ASYNC_INTERVAL + ASYNC_SLACK;
    let checked: Vec<_> = acks
        .asynchronous
        .iter()
        .filter(|(_, file, at)| kept(file) && *at + window <= cut)
        .collect();
    let lost_async = checked
        .iter()
        .filter(|(lsn, _, _)| !in_log(Durability::Async, *lsn))
        .count();
    let count = |n: usize| u32::try_from(n).unwrap_or(u32::MAX);
    WalCrashReport {
        outcome,
        acked_sync: count(acks.sync.len()),
        lost_sync: count(lost_sync),
        checked_async: count(checked.len()),
        lost_async_beyond_window: count(lost_async),
        released: count(acks.released.len()),
        unreadable: log.is_none(),
        fingerprint: sim.fingerprint(),
    }
}

fn failed(sim: &Sim) -> WalCrashReport {
    WalCrashReport {
        outcome: Outcome::Deadlock { blocked: 0 },
        acked_sync: 0,
        lost_sync: 0,
        checked_async: 0,
        lost_async_beyond_window: 0,
        released: 0,
        unreadable: true,
        fingerprint: sim.fingerprint(),
    }
}

/// [`wal_crash_with`] with the real writer.
pub fn wal_crash(seed: u64) -> WalCrashReport {
    wal_crash_with(seed, Control::Honest)
}

/// The nightly form: the run ends, the log reopens and reads back, and only
/// the losses each mode allows happened.
pub fn wal_crash_result(seed: u64) -> SeedResult {
    let report = wal_crash(seed);
    let finished = report.outcome == Outcome::Finished
        && !report.unreadable
        && report.lost_sync == 0
        && report.lost_async_beyond_window == 0;
    let detail = if finished {
        String::new()
    } else {
        format!(
            "outcome {:?}, unreadable {}, lost {} of {} acked sync records, lost {} of {} async records past the window",
            report.outcome,
            report.unreadable,
            report.lost_sync,
            report.acked_sync,
            report.lost_async_beyond_window,
            report.checked_async
        )
    };
    SeedResult {
        finished,
        fingerprint: report.fingerprint,
        detail,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wal_crash_loses_only_what_modes_allow() {
        for seed in 0..50 {
            let result = wal_crash_result(seed);
            assert!(result.finished, "seed {seed}: {}", result.detail);
        }
    }

    #[test]
    fn wal_crash_is_reproducible() {
        assert_eq!(wal_crash(7), wal_crash(7));
        assert_ne!(wal_crash(7).fingerprint, wal_crash(8).fingerprint);
    }

    #[test]
    fn wal_crash_detects_a_lying_sync() {
        let caught = (0..50).any(|seed| wal_crash_with(seed, Control::LyingSync).lost_sync > 0);
        assert!(
            caught,
            "a sync acknowledged before its sync was never caught"
        );
    }

    #[test]
    fn wal_crash_detects_a_lazy_async_flush() {
        let caught = (0..50)
            .any(|seed| wal_crash_with(seed, Control::LazyAsync).lost_async_beyond_window > 0);
        assert!(
            caught,
            "async records kept past the window were never caught"
        );
    }

    #[test]
    fn the_scenario_exercises_what_it_checks() {
        let reports: Vec<WalCrashReport> = (0..20).map(wal_crash).collect();
        let sum = |f: fn(&WalCrashReport) -> u32| reports.iter().map(f).sum::<u32>();
        assert!(sum(|r| r.acked_sync) > 200, "few sync records");
        assert!(
            sum(|r| r.checked_async) > 200,
            "the async window is never checked"
        );
        assert!(sum(|r| r.released) > 5, "no file is ever released");
    }
}
