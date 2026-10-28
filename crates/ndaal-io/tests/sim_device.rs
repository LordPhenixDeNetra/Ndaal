//! The simulated device (J02-B1): a `SimDisk` behind the I/O scheduler
//! still meets the disk contract, and time moves only through the clock.

mod common;

use std::future::Future;
use std::path::Path;
use std::pin::pin;
use std::task::{Context, Poll, Waker};
use std::time::Duration;

use ndaal_io::sched::{IoClass, SchedulerConfig};
use ndaal_io::sim::{DeviceDisk, DeviceModel, Policy, SimClock, SimDevice, SimDisk};
use ndaal_io::{Clock, Disk, Xoshiro256PlusPlus};

const MODEL: DeviceModel = DeviceModel {
    latency: Duration::from_micros(100),
    bytes_per_second: 200 * 1024 * 1024,
};

/// Runs one future to completion on `clock`: when it waits, virtual time
/// jumps to the next timer.
fn drive<F: Future>(clock: &SimClock, future: F) -> F::Output {
    let mut future = pin!(future);
    let mut cx = Context::from_waker(Waker::noop());
    loop {
        if let Poll::Ready(output) = future.as_mut().poll(&mut cx) {
            return output;
        }
        let deadline = clock.next_deadline().expect("waiting without a timer");
        clock.advance_to(deadline);
    }
}

fn device(clock: &SimClock, policy: Policy) -> SimDevice {
    let config = SchedulerConfig {
        depth: 4,
        background_share: 0.3,
    };
    SimDevice::new(
        clock.clone(),
        Xoshiro256PlusPlus::from_seed(1),
        MODEL,
        config,
        policy,
    )
}

#[test]
fn device_disk_meets_the_contract() {
    for class in [IoClass::Critical, IoClass::High, IoClass::Background] {
        let clock = SimClock::new(0);
        let disk = DeviceDisk::new(SimDisk::new(1), device(&clock, Policy::Priority), class);
        drive(&clock, async {
            disk.create_dir_all(Path::new("/d")).await.expect("mkdir");
            common::disk_contract(&disk, Path::new("/d")).await;
        });
        assert!(clock.monotonic() > Duration::ZERO, "operations take time");
    }
}

#[test]
fn an_operation_takes_its_service_time() {
    let clock = SimClock::new(0);
    let dev = device(&clock, Policy::Priority);
    let cost = 1024 * 1024;
    drive(&clock, dev.run(IoClass::Low, cost, async {}));
    let elapsed = clock.monotonic();
    let nominal = MODEL.service(cost);
    assert!(elapsed >= nominal.mul_f64(0.9) && elapsed <= MODEL.max_service(cost));
}

#[test]
fn an_idle_device_registers_no_timer() {
    let clock = SimClock::new(0);
    let dev = device(&clock, Policy::Priority);
    drive(&clock, dev.run(IoClass::High, 4096, async {}));
    assert_eq!(clock.next_deadline(), None);
}

/// A task dropped while its operation waits or runs (a timeout, a crash)
/// gives its slot back: the device does not lock up.
#[test]
fn a_dropped_operation_frees_its_slot() {
    let clock = SimClock::new(0);
    let config = SchedulerConfig {
        depth: 1,
        background_share: 0.3,
    };
    let dev = SimDevice::new(
        clock.clone(),
        Xoshiro256PlusPlus::from_seed(1),
        MODEL,
        config,
        Policy::Priority,
    );
    let mut cx = Context::from_waker(Waker::noop());
    // One started (in service), one waiting behind it; both dropped.
    let mut started = Box::pin(dev.run(IoClass::Low, 1 << 20, async {}));
    let mut waiting = Box::pin(dev.run(IoClass::Low, 1 << 20, async {}));
    assert!(started.as_mut().poll(&mut cx).is_pending());
    assert!(waiting.as_mut().poll(&mut cx).is_pending());
    drop(started);
    drop(waiting);
    drive(&clock, dev.run(IoClass::High, 4096, async {}));
}
