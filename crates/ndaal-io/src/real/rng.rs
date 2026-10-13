//! Randomness seeded by the operating system.

use std::io;

use crate::{Rng, Xoshiro256PlusPlus};

/// [`Xoshiro256PlusPlus`] seeded from the operating system. Not cryptographic.
#[derive(Debug, Clone)]
pub struct OsRng {
    inner: Xoshiro256PlusPlus,
}

impl OsRng {
    /// A generator with a fresh seed from the operating system.
    pub fn new() -> io::Result<Self> {
        let seed = getrandom::u64().map_err(|e| io::Error::other(format!("os randomness: {e}")))?;
        Ok(Self {
            inner: Xoshiro256PlusPlus::from_seed(seed),
        })
    }
}

impl Rng for OsRng {
    fn next_u64(&mut self) -> u64 {
        self.inner.next_u64()
    }

    fn fill_bytes(&mut self, out: &mut [u8]) {
        self.inner.fill_bytes(out);
    }
}
