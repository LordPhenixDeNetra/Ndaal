//! Hash maps and sets with a fixed hasher (P-31, ADR-017).
//!
//! `std::collections::HashMap` seeds its hasher from the operating system's
//! randomness: its iteration order changes from one run to the next, which
//! breaks deterministic simulation. Clippy forbids it outside this module.
//! Prefer `BTreeMap`/`BTreeSet` when the order matters; use these aliases
//! when a hash map is faster and the order does not leak into behavior.
//! The order is stable across runs for a given Rust version, not across
//! versions.
//!
//! Never key these maps with data a client chooses (keys, names, headers):
//! the hash keys are fixed and public, so a client can send colliding keys
//! and make every operation linear (hash flooding). Such maps need a hasher
//! seeded from `ndaal_io::Rng` (P-33).

#![allow(
    clippy::disallowed_types,
    reason = "the only place that names the std hash map, with a fixed hasher"
)]

use std::collections::{HashMap, HashSet};
use std::hash::{BuildHasherDefault, DefaultHasher};

/// Hasher with fixed keys: same hashes in every run.
pub type DetHasher = BuildHasherDefault<DefaultHasher>;

/// Hash map whose iteration order is the same in every run.
pub type DetMap<K, V> = HashMap<K, V, DetHasher>;

/// Hash set whose iteration order is the same in every run.
pub type DetSet<T> = HashSet<T, DetHasher>;

#[cfg(test)]
mod tests {
    use super::*;

    fn build() -> Vec<u32> {
        let mut map = DetMap::default();
        for key in 0..200_u32 {
            map.insert(key.wrapping_mul(2_654_435_761), key);
        }
        map.into_values().collect()
    }

    #[test]
    fn iteration_order_is_the_same_for_every_map() {
        assert_eq!(build(), build());
        let a: DetSet<u64> = (0..200).collect();
        let b: DetSet<u64> = (0..200).collect();
        assert!(a.iter().eq(b.iter()));
    }
}
