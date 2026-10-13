//! Behavior every `Disk` implementation must share (real and simulated).

#![allow(dead_code)] // each test binary uses a subset

use std::io::ErrorKind;
use std::path::Path;

use ndaal_io::{Disk, File, OpenOptions};

pub const CREATE: OpenOptions = OpenOptions {
    create: true,
    truncate: false,
};

/// Runs every contract check under `root` (an empty, existing directory).
pub async fn disk_contract<D: Disk>(disk: &D, root: &Path) {
    write_then_read(disk, root).await;
    read_past_end_is_short(disk, root).await;
    sparse_write_leaves_zero_hole(disk, root).await;
    open_missing_without_create_is_not_found(disk, root).await;
    rename_remove_and_sorted_listing(disk, root).await;
    set_len_truncates_and_extends(disk, root).await;
    truncate_on_open(disk, root).await;
}

async fn write_then_read<D: Disk>(disk: &D, root: &Path) {
    let file = disk.open(&root.join("a"), CREATE).await.expect("open");
    file.write_all_at(b"hello world", 0).await.expect("write");
    file.write_all_at(b"W", 6).await.expect("overwrite");
    file.sync_data().await.expect("sync");
    let mut buf = [0u8; 11];
    assert_eq!(file.read_at(&mut buf, 0).await.expect("read"), 11);
    assert_eq!(&buf, b"hello World");
    assert_eq!(file.len().await.expect("len"), 11);
}

async fn read_past_end_is_short<D: Disk>(disk: &D, root: &Path) {
    let file = disk.open(&root.join("short"), CREATE).await.expect("open");
    file.write_all_at(b"abc", 0).await.expect("write");
    let mut buf = [0u8; 8];
    assert_eq!(file.read_at(&mut buf, 1).await.expect("read"), 2);
    assert_eq!(&buf[..2], b"bc");
    assert_eq!(file.read_at(&mut buf, 100).await.expect("read past end"), 0);
}

async fn sparse_write_leaves_zero_hole<D: Disk>(disk: &D, root: &Path) {
    let file = disk.open(&root.join("sparse"), CREATE).await.expect("open");
    let offset = 1 << 20;
    file.write_all_at(b"end", offset).await.expect("write");
    assert_eq!(file.len().await.expect("len"), offset + 3);
    let mut hole = vec![0xAAu8; 4096];
    assert_eq!(
        file.read_at(&mut hole, 4096).await.expect("read hole"),
        4096
    );
    assert!(hole.iter().all(|&b| b == 0));
}

async fn open_missing_without_create_is_not_found<D: Disk>(disk: &D, root: &Path) {
    let err = disk
        .open(&root.join("missing"), OpenOptions::default())
        .await;
    assert_eq!(err.err().map(|e| e.kind()), Some(ErrorKind::NotFound));
}

async fn rename_remove_and_sorted_listing<D: Disk>(disk: &D, root: &Path) {
    let dir = root.join("dir");
    disk.create_dir_all(&dir).await.expect("mkdir");
    for name in ["c", "a", "b"] {
        disk.open(&dir.join(name), CREATE).await.expect("create");
    }
    disk.rename(&dir.join("c"), &dir.join("d"))
        .await
        .expect("rename");
    disk.remove(&dir.join("a")).await.expect("remove");
    disk.sync_dir(&dir).await.expect("sync_dir");
    let names: Vec<String> = disk
        .list_dir(&dir)
        .await
        .expect("list")
        .iter()
        .map(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("?")
                .to_owned()
        })
        .collect();
    assert_eq!(names, ["b", "d"]);
}

async fn set_len_truncates_and_extends<D: Disk>(disk: &D, root: &Path) {
    let file = disk.open(&root.join("len"), CREATE).await.expect("open");
    file.write_all_at(b"abcdef", 0).await.expect("write");
    file.set_len(2).await.expect("truncate");
    file.set_len(4).await.expect("extend");
    let mut buf = [0xFFu8; 4];
    assert_eq!(file.read_at(&mut buf, 0).await.expect("read"), 4);
    assert_eq!(&buf, b"ab\0\0");
}

async fn truncate_on_open<D: Disk>(disk: &D, root: &Path) {
    let path = root.join("trunc");
    let file = disk.open(&path, CREATE).await.expect("open");
    file.write_all_at(b"data", 0).await.expect("write");
    let opts = OpenOptions {
        create: false,
        truncate: true,
    };
    let file = disk.open(&path, opts).await.expect("reopen");
    assert_eq!(file.len().await.expect("len"), 0);
}
