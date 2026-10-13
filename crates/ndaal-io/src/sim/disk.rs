//! Simulated disk modeling the page cache (STRATEGIE_DE_TESTS.md section 4.1).
//!
//! Each file has a durable content and a list of pending writes; reads see
//! both. `sync_data` makes a file's pending writes durable; when it fails, the
//! pending writes are dropped and a retry succeeds without them, like Linux
//! after a writeback error ("fsyncgate"). Creating, renaming and removing are
//! pending until `sync_dir`, which makes the directory's current entries
//! durable (a per-directory snapshot): a rename across directories is durable
//! in each directory only once that directory is synced.
//! Directories themselves are durable as soon as they are created (deliberate
//! simplification: the engine creates its directories once, at startup).
//!
//! A crash keeps only durable state ([`CrashMode::LoseAllUnsynced`]), or keeps a
//! random subset of the pending operations, some writes torn on a sector
//! boundary ([`CrashMode::Random`]). Both are deterministic for a given seed.

use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::io;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use super::ops::{DataOp, NameOp, survive, write_into};
use crate::{Disk, File, OpenOptions, Rng, Xoshiro256PlusPlus};

/// What a crash keeps of the operations not yet durable.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CrashMode {
    /// Everything not synced is lost.
    LoseAllUnsynced,
    /// Each pending operation is lost, kept, or (writes only) torn, chosen by the seed.
    Random,
}

/// A simulated disk; clones share the same state.
#[derive(Debug, Clone)]
pub struct SimDisk {
    state: Rc<RefCell<State>>,
}

/// A file of a [`SimDisk`]; invalid after a crash.
#[derive(Debug, Clone)]
pub struct SimFile {
    disk: SimDisk,
    inode: u64,
    path: PathBuf,
    epoch: u64,
    read_only: bool,
}

#[derive(Debug)]
struct State {
    rng: Xoshiro256PlusPlus,
    /// Incremented by each crash; files opened in an older epoch are invalid.
    epoch: u64,
    dirs: BTreeSet<PathBuf>,
    names: BTreeMap<PathBuf, u64>,
    durable_names: BTreeMap<PathBuf, u64>,
    pending_names: Vec<NameOp>,
    /// BTreeMap, not HashMap: crashes iterate it and must be deterministic.
    inodes: BTreeMap<u64, Inode>,
    next_inode: u64,
    capacity: Option<u64>,
    fail_next: HashMap<PathBuf, io::ErrorKind>,
}

#[derive(Debug, Default)]
struct Inode {
    durable: Vec<u8>,
    visible: Vec<u8>,
    pending: Vec<DataOp>,
}

/// Largest simulated file: the simulator keeps contents in memory.
const MAX_FILE_SIZE: u64 = 1 << 30;

/// End of a write of `len` bytes at `offset`; past [`MAX_FILE_SIZE`] (or
/// overflowing) is an error, as a real file system returns `EFBIG`.
fn check_end(offset: u64, len: usize) -> io::Result<usize> {
    match offset.checked_add(len as u64) {
        Some(end) if end <= MAX_FILE_SIZE => to_usize(end),
        _ => Err(io::ErrorKind::FileTooLarge.into()),
    }
}

fn to_usize(v: u64) -> io::Result<usize> {
    usize::try_from(v).map_err(|_| io::Error::from(io::ErrorKind::InvalidInput))
}

impl SimDisk {
    /// An empty disk containing only `/`; `seed` drives random crashes.
    pub fn new(seed: u64) -> Self {
        let state = State {
            rng: Xoshiro256PlusPlus::from_seed(seed),
            epoch: 0,
            dirs: BTreeSet::from([PathBuf::from("/")]),
            names: BTreeMap::new(),
            durable_names: BTreeMap::new(),
            pending_names: Vec::new(),
            inodes: BTreeMap::new(),
            next_inode: 0,
            capacity: None,
            fail_next: HashMap::new(),
        };
        Self {
            state: Rc::new(RefCell::new(state)),
        }
    }

    /// Simulates a power loss; every open [`SimFile`] becomes invalid.
    pub fn crash(&self, mode: CrashMode) {
        let mut st = self.state.borrow_mut();
        let st = &mut *st;
        for op in std::mem::take(&mut st.pending_names) {
            if mode == CrashMode::Random && st.rng.next_u64() % 2 == 1 {
                op.apply(&mut st.durable_names);
            }
        }
        st.names = st.durable_names.clone();
        for inode in st.inodes.values_mut() {
            for op in std::mem::take(&mut inode.pending) {
                if mode == CrashMode::Random {
                    survive(&mut st.rng, &op, &mut inode.durable);
                }
            }
            inode.visible = inode.durable.clone();
        }
        let live: BTreeSet<u64> = st.names.values().copied().collect();
        st.inodes.retain(|inode, _| live.contains(inode));
        st.epoch += 1;
    }

    /// The next operation on `path` (disk or file) fails with `kind`.
    pub fn fail_next(&self, path: &Path, kind: io::ErrorKind) {
        self.state
            .borrow_mut()
            .fail_next
            .insert(path.to_owned(), kind);
    }

    /// Total bytes the files may hold; writes beyond fail with `StorageFull`.
    pub fn set_capacity(&self, bytes: u64) {
        self.state.borrow_mut().capacity = Some(bytes);
    }

    /// Flips bit 0 of a durable byte: silent corruption, for checksum tests.
    pub fn corrupt(&self, path: &Path, offset: u64) -> io::Result<()> {
        let mut st = self.state.borrow_mut();
        let inode = *st.names.get(path).ok_or(io::ErrorKind::NotFound)?;
        let offset = to_usize(offset)?;
        let inode = st.inodes.get_mut(&inode).ok_or(io::ErrorKind::NotFound)?;
        let byte = inode
            .durable
            .get_mut(offset)
            .ok_or(io::ErrorKind::InvalidInput)?;
        *byte ^= 1;
        if let Some(byte) = inode.visible.get_mut(offset) {
            *byte ^= 1;
        }
        Ok(())
    }
}

impl State {
    fn take_fault(&mut self, path: &Path) -> io::Result<()> {
        match self.fail_next.remove(path) {
            Some(kind) => Err(io::Error::new(kind, "injected fault")),
            None => Ok(()),
        }
    }

    fn require_dir(&self, dir: &Path) -> io::Result<()> {
        if self.dirs.contains(dir) {
            Ok(())
        } else {
            Err(io::ErrorKind::NotFound.into())
        }
    }

    /// Rejects growth of `inode` to `new_len` beyond the capacity. Only files
    /// that still have a name count: a removed file frees its space at once.
    fn reserve(&self, inode: u64, new_len: usize) -> io::Result<()> {
        let Some(capacity) = self.capacity else {
            return Ok(());
        };
        let named: BTreeSet<u64> = self.names.values().copied().collect();
        let others: usize = self
            .inodes
            .iter()
            .filter(|(id, _)| **id != inode && named.contains(id))
            .map(|(_, file)| file.visible.len())
            .sum();
        let current = self.inodes.get(&inode).map_or(0, |file| file.visible.len());
        if (others + new_len.max(current)) as u64 > capacity {
            return Err(io::ErrorKind::StorageFull.into());
        }
        Ok(())
    }
}

impl Disk for SimDisk {
    type File = SimFile;

    async fn open(&self, path: &Path, options: OpenOptions) -> io::Result<SimFile> {
        if options.read_only && (options.create || options.truncate) {
            return Err(io::ErrorKind::InvalidInput.into());
        }
        let mut st = self.state.borrow_mut();
        st.take_fault(path)?;
        let inode = match st.names.get(path) {
            Some(&inode) => inode,
            None if options.create => {
                st.require_dir(path.parent().ok_or(io::ErrorKind::InvalidInput)?)?;
                let inode = st.next_inode;
                st.next_inode += 1;
                st.inodes.insert(inode, Inode::default());
                st.names.insert(path.to_owned(), inode);
                st.pending_names.push(NameOp::Create {
                    path: path.to_owned(),
                    inode,
                });
                inode
            }
            None => return Err(io::ErrorKind::NotFound.into()),
        };
        if options.truncate {
            let file = st.inodes.get_mut(&inode).ok_or(io::ErrorKind::NotFound)?;
            file.visible.clear();
            file.pending.push(DataOp::SetLen(0));
        }
        Ok(SimFile {
            disk: self.clone(),
            inode,
            path: path.to_owned(),
            epoch: st.epoch,
            read_only: options.read_only,
        })
    }

    async fn remove(&self, path: &Path) -> io::Result<()> {
        let mut st = self.state.borrow_mut();
        st.take_fault(path)?;
        st.names.remove(path).ok_or(io::ErrorKind::NotFound)?;
        st.pending_names.push(NameOp::Remove {
            path: path.to_owned(),
        });
        Ok(())
    }

    async fn rename(&self, from: &Path, to: &Path) -> io::Result<()> {
        let mut st = self.state.borrow_mut();
        st.take_fault(from)?;
        st.require_dir(to.parent().ok_or(io::ErrorKind::InvalidInput)?)?;
        let inode = st.names.remove(from).ok_or(io::ErrorKind::NotFound)?;
        st.names.insert(to.to_owned(), inode);
        st.pending_names.push(NameOp::Rename {
            from: from.to_owned(),
            to: to.to_owned(),
        });
        Ok(())
    }

    async fn create_dir_all(&self, path: &Path) -> io::Result<()> {
        let mut st = self.state.borrow_mut();
        st.take_fault(path)?;
        for dir in path.ancestors() {
            st.dirs.insert(dir.to_owned());
        }
        Ok(())
    }

    async fn list_dir(&self, path: &Path) -> io::Result<Vec<PathBuf>> {
        let mut st = self.state.borrow_mut();
        st.take_fault(path)?;
        st.require_dir(path)?;
        let in_dir = |p: &&PathBuf| p.parent() == Some(path);
        let mut entries: Vec<PathBuf> = st.names.keys().filter(in_dir).cloned().collect();
        entries.extend(st.dirs.iter().filter(in_dir).cloned());
        entries.sort();
        Ok(entries)
    }

    async fn sync_dir(&self, dir: &Path) -> io::Result<()> {
        let mut st = self.state.borrow_mut();
        let st = &mut *st;
        st.take_fault(dir)?;
        st.require_dir(dir)?;
        let in_dir = |p: &PathBuf| p.parent() == Some(dir);
        st.durable_names.retain(|p, _| !in_dir(p));
        for (path, inode) in st.names.iter().filter(|(p, _)| in_dir(p)) {
            st.durable_names.insert(path.clone(), *inode);
        }
        st.pending_names.retain(|op| !op.only_in(dir));
        Ok(())
    }
}

impl SimFile {
    /// Runs `f` on this file's inode, after the crash and fault checks.
    fn with<T>(&self, f: impl FnOnce(&mut State, u64) -> io::Result<T>) -> io::Result<T> {
        let mut st = self.disk.state.borrow_mut();
        if st.epoch != self.epoch {
            return Err(io::Error::other("disk crashed"));
        }
        st.take_fault(&self.path)?;
        f(&mut st, self.inode)
    }

    /// Like [`Self::with`], for operations that change the file.
    fn with_write<T>(&self, f: impl FnOnce(&mut State, u64) -> io::Result<T>) -> io::Result<T> {
        if self.read_only {
            return Err(io::ErrorKind::PermissionDenied.into());
        }
        self.with(f)
    }
}

fn inode(st: &mut State, inode: u64) -> io::Result<&mut Inode> {
    st.inodes
        .get_mut(&inode)
        .ok_or_else(|| io::ErrorKind::NotFound.into())
}

impl File for SimFile {
    async fn read_at(&self, buf: &mut [u8], offset: u64) -> io::Result<usize> {
        self.with(|st, ino| {
            let visible = &inode(st, ino)?.visible;
            let start = to_usize(offset)?.min(visible.len());
            let n = buf.len().min(visible.len() - start);
            buf[..n].copy_from_slice(&visible[start..start + n]);
            Ok(n)
        })
    }

    async fn write_all_at(&self, buf: &[u8], offset: u64) -> io::Result<()> {
        self.with_write(|st, ino| {
            let end = check_end(offset, buf.len())?;
            st.reserve(ino, end)?;
            let offset = end - buf.len();
            let file = inode(st, ino)?;
            write_into(&mut file.visible, offset, buf);
            file.pending.push(DataOp::Write {
                offset,
                data: buf.to_vec(),
            });
            Ok(())
        })
    }

    async fn sync_data(&self) -> io::Result<()> {
        let mut st = self.disk.state.borrow_mut();
        if st.epoch != self.epoch {
            return Err(io::Error::other("disk crashed"));
        }
        let fault = st.take_fault(&self.path);
        let file = inode(&mut st, self.inode)?;
        if let Err(e) = fault {
            // Linux marks the pages clean after a failed writeback: the data
            // leaves the durable path and a retry will not bring it back.
            file.pending.clear();
            return Err(e);
        }
        for op in std::mem::take(&mut file.pending) {
            op.apply(&mut file.durable);
        }
        Ok(())
    }

    async fn len(&self) -> io::Result<u64> {
        self.with(|st, ino| Ok(inode(st, ino)?.visible.len() as u64))
    }

    async fn set_len(&self, len: u64) -> io::Result<()> {
        self.with_write(|st, ino| {
            let len = check_end(len, 0)?;
            st.reserve(ino, len)?;
            let file = inode(st, ino)?;
            file.visible.resize(len, 0);
            file.pending.push(DataOp::SetLen(len));
            Ok(())
        })
    }
}
