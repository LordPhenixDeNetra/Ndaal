//! Real implementations: system clock, OS-seeded randomness, `std::fs` on a
//! blocking thread pool (ADR-016, thread-pool backend).

mod clock;
mod disk;
mod net;
mod rng;
mod signal;
mod tcp;

pub use clock::SystemClock;
pub use disk::{RealDisk, RealFile};
pub use net::{RealListener, RealNetwork, RealRx, RealTx};
pub use rng::OsRng;
pub use signal::shutdown_signal;
pub use tcp::{ByteStream, TcpByteListener};
