//! Round trip between two cores (T02.03 done criterion: < 1 us on the
//! reference machine, D-05). Core 0 sends requests that core 1 echoes;
//! the bench thread asks core 0 for N round trips and gets their time.

use std::sync::{Arc, Mutex, mpsc};
use std::time::Duration;

use criterion::{Criterion, criterion_group, criterion_main};
use ndaal_io::Clock;
use ndaal_io::real::SystemClock;
use ndaal_observe::metrics::Registry;
use ndaal_runtime::layout::{Class, CoreSpec, Layout};
use ndaal_runtime::mailbox::{CoreId, Incoming};
use ndaal_runtime::{Runtime, RuntimeOptions, allowed_cpus};
use tokio::sync::mpsc::unbounded_channel;

fn round_trip(c: &mut Criterion) {
    let cpus = allowed_cpus();
    let layout = Layout {
        cores: (0..2)
            .map(|i| CoreSpec {
                cpu: cpus[i % cpus.len()],
                primary: Class::Shard,
                shared: Vec::new(),
            })
            .collect(),
    };
    let options = RuntimeOptions {
        pin: true,
        idle_spin: Duration::from_millis(1),
        capacity: 64,
        registry: Registry::default(),
    };
    let (orders, order_rx) = unbounded_channel::<u64>();
    let order_rx = Arc::new(Mutex::new(Some(order_rx)));
    let (times_tx, times) = mpsc::channel::<Duration>();
    let runtime = Runtime::start::<u64, u64, _, _>(&layout, options, move |ctx| {
        // Core 0 measures, core 1 echoes: by id, not by whoever comes
        // first (core 1 requesting itself would wait for its own reply).
        let order_rx = if ctx.id == CoreId(0) {
            order_rx.lock().map(|mut rx| rx.take()).unwrap_or(None)
        } else {
            None
        };
        let times_tx = times_tx.clone();
        async move {
            match order_rx {
                Some(mut orders) => {
                    let clock = SystemClock::new();
                    while let Some(rounds) = orders.recv().await {
                        let start = clock.monotonic();
                        for i in 0..rounds {
                            let pending = ctx.mailbox.request(CoreId(1), i).expect("one in flight");
                            assert_eq!(pending.await, Ok(i));
                        }
                        times_tx
                            .send(clock.monotonic() - start)
                            .expect("bench alive");
                    }
                }
                None => {
                    while let Some(incoming) = ctx.mailbox.recv().await {
                        if let Incoming::Request { msg, replier, .. } = incoming {
                            replier.reply(msg);
                        }
                    }
                }
            }
        }
    })
    .expect("runtime start");
    c.bench_function("runtime/round_trip", |b| {
        b.iter_custom(|rounds| {
            orders.send(rounds).expect("core 0 alive");
            times.recv().expect("core 0 answers")
        });
    });
    drop(orders);
    runtime.shutdown().expect("cores stop");
}

criterion_group!(benches, round_trip);
criterion_main!(benches);
