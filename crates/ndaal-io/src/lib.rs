//! Time, disk, network and randomness abstractions with real and simulated implementations.
//!
//! Every other crate reaches the clock, the disk and randomness through these
//! traits only (ADR-017), so the whole engine can run inside the deterministic
//! simulator.

// unsafe allowed (ARCHITECTURE.md section 3): system calls, aligned buffers and io_uring.
// Every unsafe block carries a SAFETY comment.

// Thread-per-core (ADR-003): futures stay on their core, so the traits
// deliberately do not require `Send` futures.
#![allow(async_fn_in_trait)]
// This crate is the one place that reaches the clock, the disk, the network
// and randomness (ADR-017); the rest of the workspace is linted against it.
#![allow(clippy::disallowed_methods, clippy::disallowed_types)]

mod clock;
mod disk;
mod net;
pub mod platform;
pub mod real;
mod rng;
pub mod sim;

pub use clock::Clock;
pub use disk::{Disk, File, OpenOptions};
pub use net::{FrameRx, FrameTx, Listener, MAX_FRAME_LEN, Network};
pub use rng::{Rng, Xoshiro256PlusPlus};
