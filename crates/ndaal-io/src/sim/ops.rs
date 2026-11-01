//! Pending operations of the simulated disk, and what a crash makes of them.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::{Rng, Xoshiro256PlusPlus};

/// A torn write keeps whole sectors, in any order: a device may persist
/// the sectors of one write out of order.
pub(super) const SECTOR: usize = 512;

#[derive(Debug, Clone)]
pub(super) enum DataOp {
    Write { offset: usize, data: Vec<u8> },
    SetLen(usize),
}

#[derive(Debug, Clone)]
pub(super) enum NameOp {
    Create { path: PathBuf, inode: u64 },
    Remove { path: PathBuf },
    Rename { from: PathBuf, to: PathBuf },
}

impl NameOp {
    /// Whether the operation only involves entries of `dir`: a snapshot of
    /// `dir` makes it fully durable. A rename across directories needs both.
    pub(super) fn only_in(&self, dir: &Path) -> bool {
        let in_dir = |p: &PathBuf| p.parent() == Some(dir);
        match self {
            NameOp::Create { path, .. } | NameOp::Remove { path } => in_dir(path),
            NameOp::Rename { from, to } => in_dir(from) && in_dir(to),
        }
    }

    pub(super) fn apply(&self, names: &mut BTreeMap<PathBuf, u64>) {
        match self {
            NameOp::Create { path, inode } => {
                names.insert(path.clone(), *inode);
            }
            NameOp::Remove { path } => {
                names.remove(path);
            }
            NameOp::Rename { from, to } => {
                if let Some(inode) = names.remove(from) {
                    names.insert(to.clone(), inode);
                }
            }
        }
    }
}

impl DataOp {
    pub(super) fn apply(&self, content: &mut Vec<u8>) {
        match self {
            DataOp::Write { offset, data } => write_into(content, *offset, data),
            DataOp::SetLen(len) => content.resize(*len, 0),
        }
    }
}

pub(super) fn write_into(content: &mut Vec<u8>, offset: usize, data: &[u8]) {
    if data.is_empty() {
        return;
    }
    let end = offset + data.len();
    if content.len() < end {
        content.resize(end, 0);
    }
    content[offset..end].copy_from_slice(data);
}

/// Random crash outcome of one pending data operation.
pub(super) fn survive(rng: &mut Xoshiro256PlusPlus, op: &DataOp, durable: &mut Vec<u8>) {
    match (rng.next_u64() % 3, op) {
        (0, _) => {}
        (1, _) | (_, DataOp::SetLen(_)) => op.apply(durable),
        (_, DataOp::Write { offset, data }) => {
            // Each sector is kept or lost on its own; the last, partial
            // sector of a write counts as one.
            for (i, sector) in data.chunks(SECTOR).enumerate() {
                if rng.next_u64().is_multiple_of(2) {
                    write_into(durable, *offset + i * SECTOR, sector);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A random crash keeps none, all, or any subset of the sectors of a
    /// write, not only a prefix: a device may persist them out of order.
    #[test]
    fn torn_writes_keep_any_subset_of_sectors() {
        let data: Vec<u8> = (0..8 * SECTOR).map(|i| (i / SECTOR) as u8 + 1).collect();
        let op = DataOp::Write {
            offset: 0,
            data: data.clone(),
        };
        let mut gap_before_a_kept_sector = false;
        for seed in 0..200 {
            let mut rng = Xoshiro256PlusPlus::from_seed(seed);
            let mut durable = vec![0u8; data.len()];
            survive(&mut rng, &op, &mut durable);
            let kept: Vec<bool> = durable.chunks(SECTOR).map(|s| s[0] != 0).collect();
            for (i, sector) in durable.chunks(SECTOR).enumerate() {
                assert!(
                    sector.iter().all(|&b| b == 0) || sector == &data[i * SECTOR..(i + 1) * SECTOR],
                    "seed {seed}: sector {i} half written"
                );
            }
            if kept.windows(2).any(|w| !w[0] && w[1]) {
                gap_before_a_kept_sector = true;
            }
        }
        assert!(gap_before_a_kept_sector, "only prefixes were kept");
    }
}
