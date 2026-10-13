//! Clock::sleep: virtual timers (SimClock) and real time (SystemClock).

use std::future::Future;
use std::pin::pin;
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll, Wake, Waker};
use std::time::{Duration, Instant};

use ndaal_io::Clock;
use ndaal_io::real::SystemClock;
use ndaal_io::sim::SimClock;

/// Records the order in which wakers fire.
struct Recorder {
    id: usize,
    log: Arc<Mutex<Vec<usize>>>,
}

impl Wake for Recorder {
    fn wake(self: Arc<Self>) {
        self.log.lock().expect("log").push(self.id);
    }
}

fn recorder(id: usize, log: &Arc<Mutex<Vec<usize>>>) -> Waker {
    Waker::from(Arc::new(Recorder {
        id,
        log: Arc::clone(log),
    }))
}

fn poll_once<F: Future>(future: std::pin::Pin<&mut F>, waker: &Waker) -> Poll<F::Output> {
    future.poll(&mut Context::from_waker(waker))
}

#[test]
fn zero_sleep_is_ready_at_once() {
    let clock = SimClock::new(0);
    let sleep = pin!(clock.sleep(Duration::ZERO));
    assert!(poll_once(sleep, Waker::noop()).is_ready());
}

#[test]
fn sleep_waits_for_its_deadline() {
    let clock = SimClock::new(0);
    let log = Arc::new(Mutex::new(Vec::new()));
    let waker = recorder(1, &log);
    let mut sleep = pin!(clock.sleep(Duration::from_millis(10)));
    assert!(poll_once(sleep.as_mut(), &waker).is_pending());
    assert_eq!(clock.next_deadline(), Some(Duration::from_millis(10)));
    clock.advance_to(Duration::from_millis(9));
    assert!(log.lock().expect("log").is_empty(), "woken too early");
    assert!(poll_once(sleep.as_mut(), &waker).is_pending());
    clock.advance_to(Duration::from_millis(10));
    assert_eq!(*log.lock().expect("log"), [1]);
    assert!(poll_once(sleep.as_mut(), &waker).is_ready());
    assert_eq!(clock.next_deadline(), None);
}

#[test]
fn equal_deadlines_fire_in_registration_order() {
    let clock = SimClock::new(0);
    let log = Arc::new(Mutex::new(Vec::new()));
    for id in [3, 1, 2] {
        clock.wake_at(Duration::from_millis(5), recorder(id, &log));
    }
    clock.wake_at(Duration::from_millis(1), recorder(9, &log));
    assert_eq!(clock.next_deadline(), Some(Duration::from_millis(1)));
    clock.advance_to(Duration::from_millis(5));
    assert_eq!(*log.lock().expect("log"), [9, 3, 1, 2]);
}

#[test]
fn time_never_goes_backwards() {
    let clock = SimClock::new(0);
    clock.advance_to(Duration::from_millis(5));
    clock.advance_to(Duration::from_millis(2));
    assert_eq!(clock.monotonic(), Duration::from_millis(5));
}

#[tokio::test(flavor = "current_thread")]
async fn real_sleep_takes_real_time() {
    let start = Instant::now();
    SystemClock::new().sleep(Duration::from_millis(20)).await;
    assert!(start.elapsed() >= Duration::from_millis(20));
}
