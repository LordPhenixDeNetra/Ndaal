//! One use of each item disallowed by clippy.toml; scripts/check-lint.sh
//! checks that clippy reports every one. Never run: only linted.

#![forbid(unsafe_code)]
#![allow(dead_code, unused_must_use, unused_variables, missing_docs)]

use std::path::Path;
use std::time::{Duration, Instant, SystemTime};

fn clock(i: Instant, s: SystemTime, rx: &std::sync::mpsc::Receiver<u8>) {
    Instant::now();
    i.elapsed();
    SystemTime::now();
    s.elapsed();
    std::thread::sleep(Duration::ZERO);
    rx.recv_timeout(Duration::ZERO);
}

async fn tokio_clock(i: tokio::time::Instant) {
    tokio::time::sleep(Duration::ZERO).await;
    tokio::time::sleep_until(i).await;
    tokio::time::timeout(Duration::ZERO, async {}).await;
    tokio::time::timeout_at(i, async {}).await;
    tokio::time::interval(Duration::from_secs(1));
    tokio::time::interval_at(i, Duration::from_secs(1));
    tokio::time::Instant::now();
}

fn disk(p: &Path) {
    std::fs::read(p);
    std::fs::read_to_string(p);
    std::fs::write(p, b"");
    std::fs::read_dir(p);
    std::fs::create_dir(p);
    std::fs::create_dir_all(p);
    std::fs::remove_file(p);
    std::fs::remove_dir(p);
    std::fs::remove_dir_all(p);
    std::fs::rename(p, p);
    std::fs::copy(p, p);
    std::fs::metadata(p);
    std::fs::symlink_metadata(p);
    std::fs::exists(p);
    std::fs::canonicalize(p);
    std::fs::read_link(p);
    std::fs::hard_link(p, p);
}

fn disk_permissions(p: &Path, perm: std::fs::Permissions) {
    std::fs::set_permissions(p, perm);
}

fn path_methods(p: &Path) {
    p.exists();
    p.try_exists();
    p.is_file();
    p.is_dir();
    p.metadata();
    p.symlink_metadata();
    p.read_dir();
    p.canonicalize();
    p.read_link();
}

async fn tokio_disk(p: &Path) {
    tokio::fs::read(p).await;
    tokio::fs::read_to_string(p).await;
    tokio::fs::write(p, b"").await;
    tokio::fs::read_dir(p).await;
    tokio::fs::create_dir(p).await;
    tokio::fs::create_dir_all(p).await;
    tokio::fs::remove_file(p).await;
    tokio::fs::remove_dir(p).await;
    tokio::fs::remove_dir_all(p).await;
    tokio::fs::rename(p, p).await;
    tokio::fs::copy(p, p).await;
    tokio::fs::metadata(p).await;
}

fn disk_types(
    _: Option<std::fs::File>,
    _: Option<std::fs::OpenOptions>,
    _: Option<std::fs::DirBuilder>,
    _: Option<tokio::fs::File>,
    _: Option<tokio::fs::OpenOptions>,
) {
}

async fn network() {
    use std::net::ToSocketAddrs;
    "localhost:1".to_socket_addrs();
    tokio::net::lookup_host("localhost:1").await;
}

fn network_types(
    _: Option<std::net::TcpStream>,
    _: Option<std::net::TcpListener>,
    _: Option<std::net::UdpSocket>,
    _: Option<tokio::net::TcpStream>,
    _: Option<tokio::net::TcpListener>,
    _: Option<tokio::net::TcpSocket>,
    _: Option<tokio::net::UdpSocket>,
) {
}

#[cfg(unix)]
fn unix_types(
    _: Option<std::os::unix::net::UnixStream>,
    _: Option<std::os::unix::net::UnixListener>,
    _: Option<std::os::unix::net::UnixDatagram>,
    _: Option<tokio::net::UnixStream>,
    _: Option<tokio::net::UnixListener>,
    _: Option<tokio::net::UnixDatagram>,
) {
}

fn hash_types(
    _: Option<std::collections::HashMap<u8, u8>>,
    _: Option<std::collections::HashSet<u8>>,
    _: Option<std::hash::RandomState>,
) {
}

fn randomness(buf: &mut [u8], uninit: &mut [std::mem::MaybeUninit<u8>]) {
    getrandom::fill(buf);
    getrandom::fill_uninit(uninit);
    getrandom::u32();
    getrandom::u64();
    rand::rng();
    rand::make_rng::<rand::rngs::StdRng>();
    rand::random::<u8>();
    rand::random_iter::<u8>();
    rand::random_range(0..2);
    rand::random_bool(0.5);
    rand::random_ratio(1, 2);
    rand::fill(buf);
}
