//! `ndaal server` and `ndaal health` on the real binary (T01.22, P-27).

use std::io::{BufRead, BufReader};
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Output, Stdio};

use ndaal_io::real::{RealDisk, TcpByteListener};
use ndaal_io::{Disk, File, OpenOptions};

fn command(args: &[&str]) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_ndaal"));
    command.args(args);
    for (name, _) in std::env::vars() {
        if name.starts_with("NDAAL_") {
            command.env_remove(name);
        }
    }
    command.env("NDAAL_LANG", "en");
    command
}

fn run(args: &[&str]) -> Output {
    command(args).output().expect("run ndaal")
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

fn runtime() -> tokio::runtime::Runtime {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("runtime")
}

fn text(path: &Path) -> &str {
    path.to_str().expect("utf-8 temp path")
}

/// A server killed when the test ends, even on failure.
struct Server(Child);

impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

/// Starts `ndaal server` on a free port and returns it with its address,
/// read from the "http listening" log event.
fn start_server(data_dir: &Path) -> (Server, SocketAddr) {
    let args = [
        "server",
        "--set",
        "http.listen=127.0.0.1:0",
        "--set",
        "log.format=json",
        "--data-dir",
        text(data_dir),
    ];
    let mut child = command(&args)
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn server");
    let stdout = child.stdout.take().expect("stdout");
    let server = Server(child);
    for line in BufReader::new(stdout).lines() {
        let line = line.expect("log line");
        if !line.contains("http listening") {
            continue;
        }
        let start = line.find("\"addr\":\"").expect("addr field") + "\"addr\":\"".len();
        let end = start + line[start..].find('"').expect("end of addr");
        return (server, line[start..end].parse().expect("socket address"));
    }
    panic!("the server stopped before listening");
}

fn listen_arg(addr: SocketAddr) -> String {
    format!("http.listen={addr}")
}

#[test]
fn server_becomes_ready_and_health_succeeds() {
    let dir = tempfile::tempdir().expect("tempdir");
    let data_dir = dir.path().join("d");
    let (_server, addr) = start_server(&data_dir);
    let output = run(&["health", "--set", &listen_arg(addr)]);
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    let listed = runtime()
        .block_on(RealDisk.list_dir(dir.path()))
        .expect("list");
    assert_eq!(listed, vec![data_dir]);
}

#[test]
fn health_fails_fast_on_closed_port() {
    let addr = runtime().block_on(async {
        let listener = TcpByteListener::bind("127.0.0.1:0".parse().expect("addr"))
            .await
            .expect("bind");
        listener.local_addr().expect("addr")
    });
    let output = run(&["health", "--set", &listen_arg(addr)]);
    assert_eq!(output.status.code(), Some(1));
    assert!(stderr(&output).contains("not ready"), "{}", stderr(&output));
}

#[test]
fn health_times_out_on_silent_server() {
    let runtime = runtime();
    // Bound but never accepted: the connection opens, nothing answers.
    let listener = runtime
        .block_on(TcpByteListener::bind("127.0.0.1:0".parse().expect("addr")))
        .expect("bind");
    let addr = listener.local_addr().expect("addr");
    let output = run(&["health", "--set", &listen_arg(addr)]);
    assert_eq!(output.status.code(), Some(1));
    assert!(stderr(&output).contains("no answer"), "{}", stderr(&output));
    drop(listener);
}

#[test]
fn server_fails_when_data_dir_is_a_file() {
    let dir = tempfile::tempdir().expect("tempdir");
    let file: PathBuf = dir.path().join("file");
    runtime().block_on(async {
        let create = OpenOptions {
            create: true,
            ..OpenOptions::default()
        };
        let handle = RealDisk.open(&file, create).await.expect("create");
        handle.write_all_at(b"x", 0).await.expect("write");
    });
    let output = run(&[
        "server",
        "--set",
        "http.listen=127.0.0.1:0",
        "--data-dir",
        text(&file),
    ]);
    assert_eq!(output.status.code(), Some(1), "{}", stderr(&output));
    assert!(
        stderr(&output).contains("data directory"),
        "{}",
        stderr(&output)
    );
}

#[test]
fn public_listen_is_refused_by_server_and_health() {
    for command in ["server", "health"] {
        let output = run(&[command, "--set", "http.listen=0.0.0.0:0"]);
        assert_eq!(output.status.code(), Some(2), "{command}");
    }
}
