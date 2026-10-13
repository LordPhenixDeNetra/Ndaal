//! Simulated implementations for the deterministic simulator (ADR-017).
//!
//! Single-threaded by design: state is shared through `Rc`, and every
//! operation completes immediately. Randomness comes from
//! [`crate::Xoshiro256PlusPlus`] seeded by the simulation seed.

mod clock;
mod disk;
mod net;
mod ops;

pub use clock::SimClock;
pub use disk::{CrashMode, SimDisk, SimFile};
pub use net::{SimHost, SimListener, SimNetwork, SimRx, SimTx};
