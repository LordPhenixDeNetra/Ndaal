//! Real TCP byte streams (used by the HTTP frontend) and platform defaults.

use std::path::PathBuf;

use ndaal_io::platform::default_data_dir;
use ndaal_io::real::{ByteStream, TcpByteListener};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

const LOCALHOST: &str = "127.0.0.1:0";

#[tokio::test(flavor = "current_thread")]
async fn bound_listener_echoes_bytes() {
    let listener = TcpByteListener::bind(LOCALHOST.parse().expect("addr"))
        .await
        .expect("bind");
    let addr = listener.local_addr().expect("local addr");
    let server = async {
        let (mut stream, _) = listener.accept().await.expect("accept");
        let mut buf = vec![0u8; 8192];
        loop {
            let n = stream.read(&mut buf).await.expect("server read");
            if n == 0 {
                break;
            }
            stream.write_all(&buf[..n]).await.expect("server write");
        }
    };
    let client = async {
        let mut stream = ByteStream::connect(addr).await.expect("connect");
        let data: Vec<u8> = (0..100_000u32).map(|i| (i % 251) as u8).collect();
        let (mut reader, mut writer) = tokio::io::split(&mut stream);
        let write = async {
            writer.write_all(&data).await.expect("client write");
            writer.shutdown().await.expect("shutdown");
        };
        let read = async {
            let mut echoed = Vec::new();
            reader.read_to_end(&mut echoed).await.expect("client read");
            echoed
        };
        let ((), echoed) = tokio::join!(write, read);
        assert_eq!(echoed, data);
    };
    tokio::join!(server, client);
}

#[tokio::test(flavor = "current_thread")]
async fn connect_to_a_closed_port_fails() {
    let listener = TcpByteListener::bind(LOCALHOST.parse().expect("addr"))
        .await
        .expect("bind");
    let addr = listener.local_addr().expect("local addr");
    drop(listener);
    assert!(ByteStream::connect(addr).await.is_err());
}

fn vars(list: &'static [(&'static str, &'static str)]) -> impl Fn(&str) -> Option<String> {
    move |name| {
        list.iter()
            .find(|(k, _)| *k == name)
            .map(|(_, v)| (*v).to_owned())
    }
}

#[test]
fn default_data_dir_per_system() {
    let with = default_data_dir(vars(&[
        ("HOME", "/home/u"),
        ("LOCALAPPDATA", "C:\\Users\\u\\AppData\\Local"),
    ]));
    let without = default_data_dir(vars(&[]));
    if cfg!(target_os = "windows") {
        let expected = PathBuf::from("C:\\Users\\u\\AppData\\Local")
            .join("ndaal")
            .join("data");
        assert_eq!(with, expected);
        assert_eq!(without, PathBuf::from("ndaal-data"));
    } else if cfg!(target_os = "macos") {
        let expected = PathBuf::from("/home/u")
            .join("Library")
            .join("Application Support")
            .join("ndaal");
        assert_eq!(with, expected);
        assert_eq!(without, PathBuf::from("ndaal-data"));
    } else {
        assert_eq!(with, PathBuf::from("/var/lib/ndaal"));
        assert_eq!(without, PathBuf::from("/var/lib/ndaal"));
    }
}
