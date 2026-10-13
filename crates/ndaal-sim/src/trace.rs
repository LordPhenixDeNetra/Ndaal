//! Trace fingerprint: FNV-1a 64 bits, stable across Rust versions (unlike
//! `DefaultHasher`), so a fingerprint means the same thing on every machine.

const OFFSET_BASIS: u64 = 0xcbf2_9ce4_8422_2325;
const PRIME: u64 = 0x0000_0100_0000_01b3;

/// Running FNV-1a hash of trace events.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Fnv(u64);

impl Default for Fnv {
    fn default() -> Self {
        Fnv(OFFSET_BASIS)
    }
}

impl Fnv {
    /// Hashes `bytes` alone.
    pub fn of(bytes: &[u8]) -> u64 {
        let mut fnv = Fnv::default();
        fnv.write(bytes);
        fnv.0
    }

    /// Adds one event, length-prefixed so that ("ab", "c") differs from ("a", "bc").
    pub fn event(&mut self, bytes: &[u8]) {
        self.write(&(bytes.len() as u64).to_le_bytes());
        self.write(bytes);
    }

    /// The fingerprint so far.
    pub fn value(self) -> u64 {
        self.0
    }

    fn write(&mut self, bytes: &[u8]) {
        for &byte in bytes {
            self.0 = (self.0 ^ u64::from(byte)).wrapping_mul(PRIME);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Published FNV-1a 64 test vectors.
    #[test]
    fn reference_vectors() {
        assert_eq!(Fnv::of(b""), 0xcbf2_9ce4_8422_2325);
        assert_eq!(Fnv::of(b"a"), 0xaf63_dc4c_8601_ec8c);
        assert_eq!(Fnv::of(b"foobar"), 0x8594_4171_f739_67e8);
    }

    #[test]
    fn events_are_delimited() {
        let mut a = Fnv::default();
        a.event(b"ab");
        a.event(b"c");
        let mut b = Fnv::default();
        b.event(b"a");
        b.event(b"bc");
        assert_ne!(a.value(), b.value());
    }
}
