//! Core threads: start, pinning, stop (J02-A, T02.01).

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::time::Duration;

use ndaal_observe::metrics::Registry;
use ndaal_runtime::layout::{Class, CoreSpec, Layout};
use ndaal_runtime::mailbox::{CoreId, Incoming, ReplyError};
use ndaal_runtime::{Runtime, RuntimeOptions, ShutdownError, allowed_cpus};

/// `n` shard cores on the allowed CPUs, reused when there are fewer.
fn layout(n: usize) -> Layout {
    let cpus = allowed_cpus();
    Layout {
        cores: (0..n)
            .map(|i| CoreSpec {
                cpu: cpus[i % cpus.len()],
                primary: Class::Shard,
                shared: Vec::new(),
            })
            .collect(),
    }
}

fn options(registry: &Registry) -> RuntimeOptions {
    RuntimeOptions {
        pin: true,
        idle_spin: Duration::from_micros(50),
        capacity: 4,
        registry: registry.clone(),
    }
}

#[test]
fn starts_and_stops_four_cores() {
    let (tx, rx) = mpsc::channel();
    let registry = Registry::default();
    let runtime = Runtime::start::<u32, u32, _, _>(&layout(4), options(&registry), move |ctx| {
        let tx = tx.clone();
        async move {
            tx.send(ctx.id).expect("test alive");
            while ctx.mailbox.recv().await.is_some() {}
        }
    })
    .expect("start");
    let mut started: Vec<u16> = (0..4).map(|_| rx.recv().expect("core started").0).collect();
    started.sort_unstable();
    assert_eq!(started, vec![0, 1, 2, 3]);
    assert_eq!(runtime.shutdown(), Ok(()));
}

#[cfg(target_os = "linux")]
#[test]
fn pinned_on_linux() {
    let (tx, rx) = mpsc::channel();
    let registry = Registry::default();
    let layout = layout(2);
    let runtime = Runtime::start::<u32, u32, _, _>(&layout, options(&registry), move |ctx| {
        let tx = tx.clone();
        async move {
            let ids: Vec<usize> = core_affinity::get_core_ids()
                .unwrap_or_default()
                .into_iter()
                .map(|c| c.id)
                .collect();
            tx.send((ctx.spec.cpu, ids)).expect("test alive");
            while ctx.mailbox.recv().await.is_some() {}
        }
    })
    .expect("start");
    for _ in 0..2 {
        let (cpu, ids) = rx.recv().expect("core");
        assert_eq!(ids, vec![cpu]);
    }
    assert_eq!(runtime.shutdown(), Ok(()));
}

#[cfg(target_os = "macos")]
#[test]
fn macos_runs_unpinned() {
    let registry = Registry::default();
    let runtime =
        Runtime::start::<u32, u32, _, _>(&layout(2), options(&registry), |ctx| async move {
            while ctx.mailbox.recv().await.is_some() {}
        })
        .expect("pin = true is not an error on macOS");
    assert_eq!(runtime.shutdown(), Ok(()));
}

/// Core 1 never answers; shutdown fails core 0's request and stops both.
#[test]
fn shutdown_resolves_pending_requests() {
    let (tx, rx) = mpsc::channel();
    let registry = Registry::default();
    let runtime = Runtime::start::<u32, u32, _, _>(&layout(2), options(&registry), move |ctx| {
        let tx = tx.clone();
        async move {
            if ctx.id == CoreId(0) {
                let pending = ctx.mailbox.request(CoreId(1), 1).expect("room");
                tx.send(None).expect("test alive");
                tx.send(Some(pending.await)).expect("test alive");
            } else {
                let mut held = Vec::new();
                while let Some(incoming) = ctx.mailbox.recv().await {
                    if let Incoming::Request { replier, .. } = incoming {
                        held.push(replier);
                    }
                }
            }
        }
    })
    .expect("start");
    assert_eq!(rx.recv().expect("sent"), None);
    assert_eq!(runtime.shutdown(), Ok(()));
    let answer = rx.recv().expect("answer");
    assert_eq!(answer, Some(Err(ReplyError::ShuttingDown)));
}

/// A core that ignores shutdown is reported, after the stop timeout.
#[test]
fn stuck_core_is_reported() {
    let release = Arc::new(AtomicBool::new(false));
    let stuck = Arc::clone(&release);
    let registry = Registry::default();
    let runtime = Runtime::start::<u32, u32, _, _>(&layout(2), options(&registry), move |ctx| {
        let stuck = Arc::clone(&stuck);
        async move {
            if ctx.id == CoreId(1) {
                while !stuck.load(Ordering::SeqCst) {
                    tokio::task::yield_now().await;
                }
            } else {
                while ctx.mailbox.recv().await.is_some() {}
            }
        }
    })
    .expect("start");
    let result = runtime.shutdown();
    release.store(true, Ordering::SeqCst);
    assert_eq!(
        result,
        Err(ShutdownError {
            stuck: vec![CoreId(1)]
        })
    );
}

#[test]
fn cores_gauge() {
    let registry = Registry::default();
    let layout = layout(3);
    let runtime = Runtime::start::<u32, u32, _, _>(&layout, options(&registry), |ctx| async move {
        while ctx.mailbox.recv().await.is_some() {}
    })
    .expect("start");
    let text = registry.render();
    assert!(
        text.contains("ndaal_runtime_cores{class=\"shard\"} 3"),
        "{text}"
    );
    assert!(
        text.contains("ndaal_runtime_cores{class=\"io\"} 0"),
        "{text}"
    );
    assert_eq!(runtime.shutdown(), Ok(()));
}
