//! Log files of the writer (J02-C2, T02.09): rollover with padding,
//! recycling, resuming after a crash, refusing a corrupt log.

mod wal_common;

use std::cell::RefCell;
use std::path::Path;
use std::rc::Rc;
use std::time::Duration;

use ndaal_io::sim::CrashMode;
use ndaal_io::{Disk, File, OpenOptions};
use ndaal_sim::{Outcome, Sim};
use ndaal_wal::{FileReader, Position, Step, WalError};
use wal_common::{DIR, FILE_SIZE, LIMITS, Setup, append_retrying, now, read_back, record};

/// A payload whose frame fills one 512-byte sector exactly (44-byte
/// header): frames line up with sectors, so a torn flush can lose one frame
/// and keep the next.
const SECTOR_PAYLOAD: usize = 512 - 44;

fn shard_dir() -> std::path::PathBuf {
    Path::new(DIR).join("shard-00")
}

/// Names of the files of the log directory.
fn names(sim_disk: &ndaal_io::sim::SimDisk) -> Vec<String> {
    now(sim_disk.list_dir(&shard_dir()))
        .expect("list")
        .iter()
        .map(|p| p.file_name().expect("name").to_string_lossy().into_owned())
        .collect()
}

/// The end the reader finds in file `name`, and its length.
fn end_of(sim_disk: &ndaal_io::sim::SimDisk, name: &str) -> (Step<'static>, u64) {
    let file = now(sim_disk.open(&shard_dir().join(name), OpenOptions::default())).expect("open");
    let len = now(file.len()).expect("len");
    let mut bytes = vec![0; len as usize];
    now(file.read_at(&mut bytes, 0)).expect("read");
    let bytes: &'static [u8] = Box::leak(bytes.into_boxed_slice());
    let mut reader = FileReader::new(bytes).expect("header");
    loop {
        match reader.next_step().expect("readable") {
            Step::Frame(_) => {}
            other => return (other, len),
        }
    }
}

/// Appends `count` records of `size` bytes and waits for the last one.
fn write(sim: &Sim, setup: &Setup, first: u64, count: u64, size: usize) {
    let writer = setup.writer();
    sim.spawn(async move {
        let mut last = None;
        for lsn in first..first + count {
            let at = append_retrying(&writer, &record(lsn, size), last).await;
            last = Some(at.expect("append"));
        }
        writer.durable(last.expect("one")).await.expect("durable");
    });
    assert_eq!(sim.run(LIMITS), Outcome::Finished);
}

#[test]
fn rolls_over_with_padding() {
    let sim = Sim::new(1);
    let setup = Setup::new(&sim, Duration::from_micros(100));
    write(&sim, &setup, 1, 100, 100_000);
    let lsns: Vec<u64> = read_back(&setup.disk).iter().map(|r| r.0).collect();
    assert_eq!(lsns, (1..=100).collect::<Vec<_>>());
    let mut files: Vec<String> = names(&setup.disk)
        .into_iter()
        .filter(|n| !n.starts_with("free-"))
        .collect();
    files.sort();
    assert!(files.len() >= 3, "{files:?}");
    for name in &files[..files.len() - 1] {
        let (end, len) = end_of(&setup.disk, name);
        assert_eq!(len, FILE_SIZE, "{name}");
        assert_eq!(
            end,
            Step::End {
                valid_len: FILE_SIZE
            },
            "{name} ends early"
        );
    }
}

#[test]
fn fragmented_record_across_files() {
    let sim = Sim::new(2);
    let setup = Setup::new(&sim, Duration::from_micros(100));
    let writer = setup.writer();
    sim.spawn(async move {
        let first = writer.append(&record(1, 3_000_000)).expect("append");
        let at = append_retrying(&writer, &record(2, 3_000_000), Some(first))
            .await
            .expect("append");
        writer.durable(at).await.expect("durable");
    });
    assert_eq!(sim.run(LIMITS), Outcome::Finished);
    assert_eq!(read_back(&setup.disk), [(1, 3_000_000), (2, 3_000_000)]);
}

#[test]
fn recycled_file_never_replays_old_records() {
    let sim = Sim::new(3);
    let setup = Setup::new(&sim, Duration::from_micros(100));
    let writer = setup.writer();
    sim.spawn(async move {
        let mut last = Position {
            file_no: 0,
            offset: 0,
        };
        let mut lsn = 0;
        // Fill file 0 and start file 1.
        while last.file_no < 1 {
            lsn += 1;
            last = append_retrying(&writer, &record(lsn, 100_000), Some(last))
                .await
                .expect("append");
        }
        writer.durable(last).await.expect("durable");
        writer.release(0).await.expect("release");
        // Write until a file made from file 0 is in use.
        while last.file_no < 4 {
            lsn += 1;
            last = append_retrying(&writer, &record(lsn, 100_000), Some(last))
                .await
                .expect("append");
        }
        writer.durable(last).await.expect("durable");
    });
    assert_eq!(sim.run(LIMITS), Outcome::Finished);
    let names = names(&setup.disk);
    assert!(
        !names.iter().any(|n| n.starts_with("free-")),
        "file 0 reused: {names:?}"
    );
    // Records of file 0 were released: what reads back starts in file 1, in
    // order, without any record of the old life of file 0 after them.
    let lsns: Vec<u64> = read_back(&setup.disk).iter().map(|r| r.0).collect();
    assert!(lsns.windows(2).all(|w| w[1] == w[0] + 1), "{lsns:?}");
}

#[test]
fn crash_mid_recycle_is_safe() {
    let sim = Sim::new(4);
    let setup = Setup::new(&sim, Duration::from_micros(100));
    write(&sim, &setup, 1, 10, 1000);
    // A free file whose header rewrite was cut short: garbage header.
    let garbage = shard_dir().join("free-00000000000000aa.wal");
    let create = OpenOptions {
        create: true,
        ..OpenOptions::default()
    };
    let file = now(setup.disk.open(&garbage, create)).expect("create");
    now(file.write_all_at(&[0xEE; 8192], 0)).expect("garbage");
    now(file.sync_data()).expect("sync");
    now(setup.disk.sync_dir(&shard_dir())).expect("sync dir");
    let again = Setup::reopen(&sim, &setup.disk, Duration::from_micros(100));
    write(&sim, &again, 11, 200, 100_000);
    let lsns: Vec<u64> = read_back(&setup.disk).iter().map(|r| r.0).collect();
    assert_eq!(lsns, (1..=210).collect::<Vec<_>>());
}

#[test]
fn resume_never_resurrects_a_torn_flush() {
    for seed in 0..12 {
        let sim = Sim::new(seed);
        let setup = Setup::new(&sim, Duration::from_millis(1));
        let writer = setup.writer();
        let disk = setup.disk.clone();
        sim.spawn(async move {
            let mut last = None;
            for lsn in 1..=1500 {
                match writer.append(&record(lsn, SECTOR_PAYLOAD)) {
                    Ok(at) => last = Some(at),
                    Err(_) => return,
                }
            }
            // Fails if the crash comes first; ends either way.
            if let Some(at) = last {
                let _ = writer.durable(at).await;
            }
        });
        // Crash in the middle of the flushes.
        let clock = sim.clock();
        let crash_disk = disk.clone();
        sim.spawn(async move {
            ndaal_io::Clock::sleep(&clock, Duration::from_micros(500 + seed * 97)).await;
            crash_disk.crash(CrashMode::Random);
        });
        sim.run(LIMITS);
        let before: Vec<u64> = read_back(&disk).iter().map(|r| r.0).collect();
        let again = Setup::reopen(&sim, &disk, Duration::from_micros(100));
        // A new record that starts at the resume point and ends exactly on
        // the next page boundary: the flush pads nothing with zeros, so an
        // old frame that survived the tear right after it would read back.
        let resume_at = 4096 + before.len() as u64 * 512;
        let page_end = (resume_at + 1).div_ceil(4096) * 4096;
        let new_len = (page_end - resume_at - 44) as usize;
        write(&sim, &again, 9000, 1, new_len);
        let after: Vec<u64> = read_back(&disk).iter().map(|r| r.0).collect();
        let mut expected = before.clone();
        expected.push(9000);
        assert_eq!(after, expected, "seed {seed}: a torn record came back");
    }
}

#[test]
fn open_refuses_a_corrupt_log() {
    let sim = Sim::new(5);
    let setup = Setup::new(&sim, Duration::from_micros(100));
    write(&sim, &setup, 1, 30, 100_000);
    // Damage a frame early in the file holding the data (the next file,
    // prepared ahead, is still empty), with valid frames well past it.
    let first = shard_dir().join("0000000000000000.wal");
    setup.disk.corrupt(&first, 4096 + 60).expect("corrupt");
    let result = Rc::new(RefCell::new(None));
    let r = Rc::clone(&result);
    let reopened = Setup::try_reopen(&sim, &setup.disk);
    *r.borrow_mut() = Some(reopened);
    let reopened = result.borrow_mut().take().expect("ran");
    assert!(
        matches!(reopened, Err(WalError::Corrupt { .. })),
        "{reopened:?}"
    );
}

/// A log file cut by someone else to a length that is not a whole number of
/// pages is refused at open, never a panic.
#[test]
fn truncated_file_is_refused_without_panic() {
    let sim = Sim::new(6);
    let setup = Setup::new(&sim, Duration::from_micros(100));
    write(&sim, &setup, 1, 1, 1);
    let path = shard_dir().join("0000000000000000.wal");
    let file = now(setup.disk.open(&path, OpenOptions::default())).expect("open");
    now(file.set_len(4096 + 50)).expect("truncate");
    now(file.sync_data()).expect("sync");
    let reopened = Setup::try_reopen(&sim, &setup.disk);
    assert!(
        matches!(reopened, Err(WalError::Corrupt { .. })),
        "{reopened:?}"
    );
}

/// A power cut after a recycled file's header was rewritten and synced, but
/// before it took its number: the free file stays free and reusable, the
/// log reopens and goes on, nothing acknowledged is lost.
#[test]
fn power_cut_mid_recycle_keeps_a_reusable_free_file() {
    let sim = Sim::new(8);
    let setup = Setup::new(&sim, Duration::from_micros(100));
    let (writer, hook) = (setup.writer(), setup.crash_hook());
    let acked = Rc::new(RefCell::new(Vec::new()));
    let a = Rc::clone(&acked);
    sim.spawn(async move {
        let mut last = Position {
            file_no: 0,
            offset: 0,
        };
        let mut lsn = 0;
        while last.file_no < 1 {
            lsn += 1;
            last = append_retrying(&writer, &record(lsn, 100_000), Some(last))
                .await
                .expect("append");
        }
        writer.durable(last).await.expect("durable");
        a.borrow_mut().extend(1..=lsn);
        writer.release(0).await.expect("release");
        // The next file made recycles file 0: cut the power before it takes
        // its number.
        hook.arm();
        loop {
            lsn += 1;
            let Ok(at) = append_retrying(&writer, &record(lsn, 100_000), Some(last)).await else {
                return;
            };
            last = at;
            if writer.durable(at).await.is_err() {
                return;
            }
            a.borrow_mut().push(lsn);
        }
    });
    assert_eq!(sim.run(LIMITS), Outcome::Finished);
    assert_eq!(
        setup.recorder.crashed_on.borrow().as_deref(),
        Some("free-0000000000000000.wal"),
        "the cut hit the recycling of file 0"
    );
    let again = Setup::reopen(&sim, &setup.disk, Duration::from_micros(100));
    write(&sim, &again, 9000, 30, 100_000);
    let lsns: Vec<u64> = read_back(&setup.disk).iter().map(|r| r.0).collect();
    let acked = acked.borrow();
    // File 0 was released: its records are gone; every later acknowledged
    // record is there, then the new ones.
    for lsn in acked.iter().filter(|&&l| lsns.contains(&l) || l > lsns[0]) {
        assert!(lsns.contains(lsn), "acknowledged record {lsn} lost");
    }
    assert!(
        lsns.ends_with(&(9000..9030).collect::<Vec<_>>()),
        "{lsns:?}"
    );
}
