//! Pending operations of the simulated disk, and what a crash makes of them.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::{Rng, Xoshiro256PlusPlus};

/// A torn write keeps a prefix that is a whole number of sectors.
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
            let sectors = data.len() / SECTOR;
            // Bounded by data.len(), so the cast back cannot truncate.
            let kept = (rng.next_u64() % (sectors as u64 + 1)) as usize * SECTOR;
            write_into(durable, *offset, &data[..kept]);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A random crash keeps 0, all, or a whole number of sectors of a write.
    #[test]
    fn torn_writes_keep_whole_sectors() {
        let data = vec![7u8; 3 * SECTOR + 100];
        let op = DataOp::Write {
            offset: 0,
            data: data.clone(),
        };
        for seed in 0..200 {
            let mut rng = Xoshiro256PlusPlus::from_seed(seed);
            let mut durable = Vec::new();
            survive(&mut rng, &op, &mut durable);
            let kept = durable.len();
            assert!(
                kept % SECTOR == 0 || kept == data.len(),
                "seed {seed}: kept {kept}"
            );
        }
    }
}
