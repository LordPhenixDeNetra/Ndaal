//! Non-cryptographic randomness (jitter, sampling, simulation choices).
//!
//! Keys and tokens never come from here: cryptographic randomness goes through
//! `ring` (ADR-020).

/// A source of random numbers.
pub trait Rng {
    /// Next 64 random bits.
    fn next_u64(&mut self) -> u64;

    /// Fills `out` with random bytes.
    fn fill_bytes(&mut self, out: &mut [u8]);
}

/// xoshiro256++ (Blackman and Vigna, 2019), seeded through SplitMix64.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Xoshiro256PlusPlus {
    state: [u64; 4],
}

impl Xoshiro256PlusPlus {
    /// Deterministic generator for `seed`.
    pub fn from_seed(seed: u64) -> Self {
        let mut sm = seed;
        let state = [(); 4].map(|()| splitmix64(&mut sm));
        Self { state }
    }
}

/// SplitMix64 step: expands one seed into well-mixed words (never all zero).
fn splitmix64(state: &mut u64) -> u64 {
    *state = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
    let mut z = *state;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

impl Rng for Xoshiro256PlusPlus {
    fn next_u64(&mut self) -> u64 {
        let s = &mut self.state;
        let result = s[0].wrapping_add(s[3]).rotate_left(23).wrapping_add(s[0]);
        let t = s[1] << 17;
        s[2] ^= s[0];
        s[3] ^= s[1];
        s[1] ^= s[2];
        s[0] ^= s[3];
        s[2] ^= t;
        s[3] = s[3].rotate_left(45);
        result
    }

    fn fill_bytes(&mut self, out: &mut [u8]) {
        for chunk in out.chunks_mut(8) {
            let word = self.next_u64().to_le_bytes();
            chunk.copy_from_slice(&word[..chunk.len()]);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Expected values computed with an independent Python transcription of the
    // published algorithms; SplitMix64(0) first output 0xe220a8397b1dcdaf matches
    // the published reference.
    #[test]
    fn reference_vectors() {
        let mut rng = Xoshiro256PlusPlus::from_seed(0);
        let out: Vec<u64> = (0..4).map(|_| rng.next_u64()).collect();
        assert_eq!(
            out,
            [
                0x53175d61490b23df,
                0x61da6f3dc380d507,
                0x5c0fdf91ec9a7bfc,
                0x02eebf8c3bbe5e1a
            ]
        );
    }

    #[test]
    fn same_seed_same_sequence_different_seed_different_sequence() {
        let a: Vec<u64> = {
            let mut r = Xoshiro256PlusPlus::from_seed(42);
            (0..8).map(|_| r.next_u64()).collect()
        };
        let mut r = Xoshiro256PlusPlus::from_seed(42);
        assert_eq!(a, (0..8).map(|_| r.next_u64()).collect::<Vec<_>>());
        let mut r = Xoshiro256PlusPlus::from_seed(43);
        assert_ne!(a, (0..8).map(|_| r.next_u64()).collect::<Vec<_>>());
    }

    #[test]
    fn fill_bytes_uses_little_endian_words_and_handles_tails() {
        let mut a = Xoshiro256PlusPlus::from_seed(0);
        let mut bytes = [0u8; 11];
        a.fill_bytes(&mut bytes);
        let mut b = Xoshiro256PlusPlus::from_seed(0);
        let first = b.next_u64().to_le_bytes();
        let second = b.next_u64().to_le_bytes();
        assert_eq!(&bytes[..8], &first);
        assert_eq!(&bytes[8..], &second[..3]);
    }
}
