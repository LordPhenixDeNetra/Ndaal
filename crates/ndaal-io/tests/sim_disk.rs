//! Simulated disk and clock (T01.08).

mod common;

use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::time::Duration;

use ndaal_io::sim::{CrashMode, SimClock, SimDisk};
use ndaal_io::{Clock, Disk, File, OpenOptions};
use proptest::prelude::*;

use common::CREATE;

fn run<F: std::future::Future>(future: F) -> F::Output {
    tokio::runtime::Builder::new_current_thread()
        .build()
        .expect("runtime")
        .block_on(future)
}

/// A disk with an empty, durable `/d` directory.
fn disk(seed: u64) -> SimDisk {
    let disk = SimDisk::new(seed);
    run(disk.create_dir_all(Path::new("/d"))).expect("mkdir");
    disk
}

async fn read_all(disk: &SimDisk, path: &Path) -> Option<Vec<u8>> {
    let file = disk.open(path, OpenOptions::default()).await.ok()?;
    let len = file.len().await.expect("len");
    let mut buf = vec![0u8; usize::try_from(len).expect("small file")];
    file.read_at(&mut buf, 0).await.expect("read");
    Some(buf)
}

#[test]
fn sim_disk_meets_the_contract() {
    let disk = disk(1);
    run(common::disk_contract(&disk, Path::new("/d")));
}

#[test]
fn unsynced_create_vanishes_after_crash() {
    let disk = disk(2);
    let path = Path::new("/d/new");
    run(async {
        let file = disk.open(path, CREATE).await.expect("create");
        file.write_all_at(b"data", 0).await.expect("write");
        file.sync_data()
            .await
            .expect("sync data, not the directory");
    });
    disk.crash(CrashMode::LoseAllUnsynced);
    assert_eq!(run(read_all(&disk, path)), None);
}

#[test]
fn synced_create_survives_crash() {
    let disk = disk(3);
    let path = Path::new("/d/kept");
    run(async {
        let file = disk.open(path, CREATE).await.expect("create");
        file.write_all_at(b"data", 0).await.expect("write");
        file.sync_data().await.expect("sync");
        disk.sync_dir(Path::new("/d")).await.expect("sync dir");
    });
    disk.crash(CrashMode::LoseAllUnsynced);
    assert_eq!(run(read_all(&disk, path)), Some(b"data".to_vec()));
}

#[test]
fn unsynced_rename_is_undone_by_crash() {
    let disk = disk(4);
    run(async {
        disk.open(Path::new("/d/old"), CREATE)
            .await
            .expect("create");
        disk.sync_dir(Path::new("/d")).await.expect("sync dir");
        disk.rename(Path::new("/d/old"), Path::new("/d/new"))
            .await
            .expect("rename");
    });
    disk.crash(CrashMode::LoseAllUnsynced);
    assert!(run(read_all(&disk, Path::new("/d/old"))).is_some());
    assert!(run(read_all(&disk, Path::new("/d/new"))).is_none());
}

#[test]
fn files_opened_before_a_crash_are_invalid() {
    let disk = disk(5);
    let file = run(disk.open(Path::new("/d/f"), CREATE)).expect("create");
    disk.crash(CrashMode::LoseAllUnsynced);
    let err = run(file.len()).expect_err("stale handle");
    assert_eq!(err.kind(), ErrorKind::Other);
}

#[test]
fn injected_faults() {
    let disk = disk(6);
    let path = Path::new("/d/f");
    run(async {
        let file = disk.open(path, CREATE).await.expect("create");
        disk.fail_next(path, ErrorKind::Other);
        assert_eq!(
            file.sync_data().await.map_err(|e| e.kind()),
            Err(ErrorKind::Other)
        );
        file.sync_data()
            .await
            .expect("only the next operation fails");

        disk.set_capacity(10);
        let full = file.write_all_at(&[1u8; 11], 0).await;
        assert_eq!(full.map_err(|e| e.kind()), Err(ErrorKind::StorageFull));
        file.write_all_at(&[1u8; 10], 0).await.expect("fits");
        file.sync_data().await.expect("sync");

        disk.corrupt(path, 3).expect("corrupt");
        let mut buf = [0u8; 10];
        file.read_at(&mut buf, 0).await.expect("read");
        assert_eq!(buf[3], 1 ^ 1, "bit 0 flipped");
        assert!(buf.iter().enumerate().all(|(i, &b)| i == 3 || b == 1));
    });
}

#[test]
fn sim_clock_moves_only_when_advanced() {
    let clock = SimClock::new(1_000_000);
    assert_eq!(
        (clock.monotonic(), clock.unix_micros()),
        (Duration::ZERO, 1_000_000)
    );
    clock.clone().advance(Duration::from_millis(5));
    assert_eq!(clock.monotonic(), Duration::from_millis(5));
    assert_eq!(clock.unix_micros(), 1_005_000);
}

/// One step of a random workload on files `/d/f0` to `/d/f2`.
#[derive(Debug, Clone)]
enum Op {
    Write {
        file: usize,
        offset: u64,
        data: Vec<u8>,
    },
    Sync {
        file: usize,
    },
}

fn op() -> impl Strategy<Value = Op> {
    prop_oneof![
        (
            0..3usize,
            0..4096u64,
            prop::collection::vec(any::<u8>(), 1..1500)
        )
            .prop_map(|(file, offset, data)| Op::Write { file, offset, data }),
        (0..3usize).prop_map(|file| Op::Sync { file }),
    ]
}

fn path(i: usize) -> PathBuf {
    PathBuf::from(format!("/d/f{i}"))
}

fn apply(content: &mut Vec<u8>, offset: u64, data: &[u8]) {
    let start = usize::try_from(offset).expect("small offset");
    if content.len() < start + data.len() {
        content.resize(start + data.len(), 0);
    }
    content[start..start + data.len()].copy_from_slice(data);
}

/// Runs `ops` on a fresh disk whose files are created and durable; returns the
/// disk, the durable model and the writes still pending per file.
/// Writes not yet synced, per file: (offset, data).
type Pending = Vec<Vec<(u64, Vec<u8>)>>;

fn workload(seed: u64, ops: &[Op]) -> (SimDisk, Vec<Vec<u8>>, Pending) {
    let disk = disk(seed);
    let mut durable = vec![Vec::new(); 3];
    let mut visible = vec![Vec::new(); 3];
    let mut pending = vec![Vec::new(); 3];
    run(async {
        let mut files = Vec::new();
        for i in 0..3 {
            files.push(disk.open(&path(i), CREATE).await.expect("create"));
        }
        disk.sync_dir(Path::new("/d")).await.expect("sync dir");
        for op in ops {
            match op {
                Op::Write { file, offset, data } => {
                    files[*file]
                        .write_all_at(data, *offset)
                        .await
                        .expect("write");
                    apply(&mut visible[*file], *offset, data);
                    pending[*file].push((*offset, data.clone()));
                }
                Op::Sync { file } => {
                    files[*file].sync_data().await.expect("sync");
                    durable[*file] = visible[*file].clone();
                    pending[*file].clear();
                }
            }
        }
    });
    (disk, durable, pending)
}

proptest! {
    // T01.08 done criterion: a crash loses exactly the unsynced writes.
    #[test]
    fn crash_keeps_exactly_the_synced_state(seed: u64, ops in prop::collection::vec(op(), 0..40)) {
        let (disk, durable, _) = workload(seed, &ops);
        disk.crash(CrashMode::LoseAllUnsynced);
        for (i, expected) in durable.iter().enumerate() {
            let after = run(read_all(&disk, &path(i)));
            prop_assert_eq!(after.as_ref(), Some(expected));
        }
    }

    // Random crash: every byte is either the synced byte (zero past the synced
    // end) or a byte of a pending write that covered it.
    #[test]
    fn random_crash_only_mixes_synced_and_pending_bytes(
        seed: u64, ops in prop::collection::vec(op(), 0..40)
    ) {
        let (disk, durable, pending) = workload(seed, &ops);
        disk.crash(CrashMode::Random);
        for i in 0..3 {
            let after = run(read_all(&disk, &path(i))).expect("file survives");
            let max_end = pending[i]
                .iter()
                .map(|(o, d)| usize::try_from(*o).expect("small") + d.len())
                .chain([durable[i].len()])
                .max()
                .unwrap_or(0);
            prop_assert!(after.len() >= durable[i].len() && after.len() <= max_end);
            for (p, &byte) in after.iter().enumerate() {
                let synced = durable[i].get(p).copied().unwrap_or(0);
                let from_pending = pending[i].iter().any(|(o, d)| {
                    let o = usize::try_from(*o).expect("small");
                    p >= o && p < o + d.len() && d[p - o] == byte
                });
                prop_assert!(byte == synced || from_pending, "file {} byte {}", i, p);
            }
        }
    }

    #[test]
    fn random_crash_is_deterministic(seed: u64, ops in prop::collection::vec(op(), 0..20)) {
        let contents = |s| {
            let (disk, _, _) = workload(s, &ops);
            disk.crash(CrashMode::Random);
            (0..3).map(|i| run(read_all(&disk, &path(i)))).collect::<Vec<_>>()
        };
        prop_assert_eq!(contents(seed), contents(seed));
    }
}

// ---- Regression tests from the lot 3 final review ----

async fn create_synced(disk: &SimDisk, path: &Path, byte: u8) {
    let file = disk.open(path, CREATE).await.expect("create");
    file.write_all_at(&[byte], 0).await.expect("write");
    file.sync_data().await.expect("sync");
}

#[test]
fn rename_into_place_with_both_dirs_synced_keeps_only_the_new_name() {
    let disk = disk(10);
    run(async {
        disk.create_dir_all(Path::new("/tmp")).await.expect("mkdir");
        create_synced(&disk, Path::new("/tmp/x"), 1).await;
        disk.rename(Path::new("/tmp/x"), Path::new("/d/x"))
            .await
            .expect("rename");
        disk.sync_dir(Path::new("/d")).await.expect("sync target");
        disk.sync_dir(Path::new("/tmp")).await.expect("sync source");
    });
    disk.crash(CrashMode::LoseAllUnsynced);
    assert_eq!(run(read_all(&disk, Path::new("/d/x"))), Some(vec![1]));
    assert_eq!(run(read_all(&disk, Path::new("/tmp/x"))), None);
}

#[test]
fn cross_dir_rename_is_not_durable_until_the_target_dir_is_synced() {
    let disk = disk(11);
    run(async {
        disk.create_dir_all(Path::new("/e")).await.expect("mkdir");
        create_synced(&disk, Path::new("/d/x"), 1).await;
        disk.sync_dir(Path::new("/d")).await.expect("sync");
        disk.rename(Path::new("/d/x"), Path::new("/e/x"))
            .await
            .expect("rename");
        disk.sync_dir(Path::new("/d"))
            .await
            .expect("sync source only");
    });
    disk.crash(CrashMode::LoseAllUnsynced);
    assert_eq!(run(read_all(&disk, Path::new("/e/x"))), None);
}

#[test]
fn failed_sync_data_loses_the_pending_writes_even_if_retried() {
    let disk = disk(12);
    let path = Path::new("/d/f");
    run(async {
        let file = disk.open(path, CREATE).await.expect("create");
        disk.sync_dir(Path::new("/d")).await.expect("sync dir");
        file.write_all_at(b"lost", 0).await.expect("write");
        disk.fail_next(path, ErrorKind::Other);
        assert!(file.sync_data().await.is_err());
        file.sync_data()
            .await
            .expect("retry reports success, like Linux");
    });
    disk.crash(CrashMode::LoseAllUnsynced);
    assert_eq!(run(read_all(&disk, path)), Some(Vec::new()));
}

#[test]
fn removing_a_file_frees_capacity() {
    let disk = disk(13);
    run(async {
        disk.set_capacity(1000);
        let file = disk
            .open(Path::new("/d/big"), CREATE)
            .await
            .expect("create");
        file.write_all_at(&[0u8; 900], 0).await.expect("write");
        drop(file);
        disk.remove(Path::new("/d/big")).await.expect("remove");
        let file = disk
            .open(Path::new("/d/next"), CREATE)
            .await
            .expect("create");
        file.write_all_at(&[0u8; 500], 0)
            .await
            .expect("space was freed");
    });
}

#[test]
fn huge_offsets_are_errors_not_panics() {
    let disk = disk(14);
    run(async {
        let file = disk.open(Path::new("/d/f"), CREATE).await.expect("create");
        assert!(file.write_all_at(b"x", u64::MAX).await.is_err());
        assert!(file.write_all_at(b"x", 1 << 40).await.is_err());
        assert!(file.set_len(1 << 40).await.is_err());
        assert_eq!(file.len().await.expect("len"), 0);
    });
}

/// Namespace workload over `/d` and `/e`: names `n0` to `n2` in each.
#[derive(Debug, Clone)]
enum NsOp {
    Create(usize, usize),
    Rename((usize, usize), (usize, usize)),
    Remove(usize, usize),
    SyncDir(usize),
}

fn ns_op() -> impl Strategy<Value = NsOp> {
    let name = (0..2usize, 0..3usize);
    prop_oneof![
        name.clone().prop_map(|(d, n)| NsOp::Create(d, n)),
        (name.clone(), name.clone()).prop_map(|(a, b)| NsOp::Rename(a, b)),
        name.prop_map(|(d, n)| NsOp::Remove(d, n)),
        (0..2usize).prop_map(NsOp::SyncDir),
    ]
}

fn ns_dir(d: usize) -> PathBuf {
    PathBuf::from(if d == 0 { "/d" } else { "/e" })
}

fn ns_path((d, n): (usize, usize)) -> PathBuf {
    ns_dir(d).join(format!("n{n}"))
}

proptest! {
    // After a crash, each directory holds exactly what it held at its last
    // sync_dir (per-directory snapshot), and each name keeps its file.
    #[test]
    fn crash_restores_each_directory_to_its_last_sync(
        seed: u64, ops in prop::collection::vec(ns_op(), 0..30)
    ) {
        use std::collections::BTreeMap;
        use std::collections::btree_map::Entry;
        let disk = disk(seed);
        let mut visible: BTreeMap<PathBuf, u8> = BTreeMap::new();
        let mut durable: BTreeMap<PathBuf, u8> = BTreeMap::new();
        let mut next_id = 0u8;
        run(async {
            disk.create_dir_all(Path::new("/e")).await.expect("mkdir");
            for op in &ops {
                match *op {
                    NsOp::Create(d, n) => {
                        let p = ns_path((d, n));
                        if let Entry::Vacant(slot) = visible.entry(p.clone()) {
                            create_synced(&disk, &p, next_id).await;
                            slot.insert(next_id);
                            next_id += 1;
                        }
                    }
                    NsOp::Rename(a, b) => {
                        let (from, to) = (ns_path(a), ns_path(b));
                        if from != to && let Some(id) = visible.remove(&from) {
                            disk.rename(&from, &to).await.expect("rename");
                            visible.insert(to, id);
                        }
                    }
                    NsOp::Remove(d, n) => {
                        let p = ns_path((d, n));
                        if visible.remove(&p).is_some() {
                            disk.remove(&p).await.expect("remove");
                        }
                    }
                    NsOp::SyncDir(d) => {
                        let dir = ns_dir(d);
                        disk.sync_dir(&dir).await.expect("sync dir");
                        durable.retain(|p, _| p.parent() != Some(dir.as_path()));
                        for (p, id) in &visible {
                            if p.parent() == Some(dir.as_path()) {
                                durable.insert(p.clone(), *id);
                            }
                        }
                    }
                }
            }
        });
        disk.crash(CrashMode::LoseAllUnsynced);
        for d in 0..2 {
            for n in 0..3 {
                let p = ns_path((d, n));
                let expected = durable.get(&p).map(|id| vec![*id]);
                prop_assert_eq!(run(read_all(&disk, &p)), expected, "{:?}", p);
            }
        }
    }
}
