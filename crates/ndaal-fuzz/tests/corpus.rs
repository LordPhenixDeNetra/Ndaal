//! Replays the versioned fuzz corpus on stable, on every system.

use std::path::{Path, PathBuf};

use ndaal_fuzz::{TARGETS, common_codec};
use ndaal_io::real::RealDisk;
use ndaal_io::{Disk, File, OpenOptions};

fn corpus_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("fuzz")
        .join("corpus")
}

fn block_on<T>(future: impl Future<Output = T>) -> T {
    tokio::runtime::Builder::new_current_thread()
        .build()
        .expect("runtime")
        .block_on(future)
}

async fn read(path: &Path) -> Vec<u8> {
    let options = OpenOptions {
        read_only: true,
        ..OpenOptions::default()
    };
    let file = RealDisk
        .open(path, options)
        .await
        .expect("open corpus file");
    let len = file.len().await.expect("len");
    let mut bytes = vec![0; usize::try_from(len).expect("corpus file fits in memory")];
    let mut done = 0;
    while done < bytes.len() {
        let offset = u64::try_from(done).expect("offset");
        let n = file
            .read_at(&mut bytes[done..], offset)
            .await
            .expect("read");
        assert!(n > 0, "{} shrank while read", path.display());
        done += n;
    }
    bytes
}

fn name(path: &Path) -> String {
    path.file_name()
        .expect("file name")
        .to_string_lossy()
        .into_owned()
}

#[test]
fn every_target_has_a_corpus() {
    let mut dirs: Vec<String> = block_on(RealDisk.list_dir(&corpus_root()))
        .expect("list corpus")
        .iter()
        .filter(|p| name(p) != "README.md")
        .map(|p| name(p))
        .collect();
    dirs.sort();
    let mut targets: Vec<String> = TARGETS.iter().map(|(n, _)| (*n).to_owned()).collect();
    targets.sort();
    assert_eq!(dirs, targets, "one corpus directory per target");
    for target in targets {
        let files = block_on(RealDisk.list_dir(&corpus_root().join(&target))).expect("list");
        assert!(!files.is_empty(), "empty corpus for {target}");
    }
}

#[test]
fn corpus_replays_without_panic() {
    for (target, run) in TARGETS {
        let dir = corpus_root().join(target);
        for path in block_on(RealDisk.list_dir(&dir)).expect("list") {
            // Printed before the run: a panic's output names the input.
            eprintln!("replaying {target}: {}", path.display());
            run(&block_on(read(&path)));
        }
    }
}

#[test]
fn common_codec_rejects_without_panicking() {
    let cases: &[&[u8]] = &[
        b"",
        &[0xFF; 11],
        &[0x80, 0x00],
        &[0x7F, b'a'],
        &[0x02, 0xC3, 0x28],
        b"7ZZZZZZZZZZZZZZZZZZZZZZZZU",
        b"8ZZZZZZZZZZZZZZZZZZZZZZZZZ",
        b"01ARZ3NDEKTSV4RRFFQ69G5FAV",
    ];
    for case in cases {
        common_codec(case);
    }
}
