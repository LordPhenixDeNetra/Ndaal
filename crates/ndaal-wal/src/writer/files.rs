//! Log files on disk (T02.09, FORMATS section 2.1): opening and resuming
//! the log, making a file ready (created or recycled), reading a file.
//!
//! A file only takes its number once its header is written and synced:
//! it is prepared under a `free-` name, then renamed. A crash in between
//! leaves a free file, never a numbered file with a bad header.

use std::collections::{BTreeMap, VecDeque};
use std::path::{Path, PathBuf};

use ndaal_common::ids::NodeId;
use ndaal_io::{Clock, Disk, File, OpenOptions};

use super::file_buf::FileBuf;
use super::state::{Prepare, State};
use crate::{FileHeader, FileReader, HEADER_LEN, MAX_FLUSH, Step, WalError};

const PAGE: u64 = 4096;

/// Name of log file `file_no`.
pub(super) fn file_name(file_no: u64) -> String {
    format!("{file_no:016x}.wal")
}

/// Name of a free file, by its original number.
pub(super) fn free_name(file_no: u64) -> String {
    format!("free-{file_no:016x}.wal")
}

/// `(free, number)` of a log directory entry, if it is one.
fn parse(path: &Path) -> Option<(bool, u64)> {
    let name = path.file_name()?.to_str()?;
    let stem = name.strip_suffix(".wal")?;
    let (free, digits) = match stem.strip_prefix("free-") {
        Some(digits) => (true, digits),
        None => (false, stem),
    };
    if digits.len() != 16 {
        return None;
    }
    u64::from_str_radix(digits, 16).ok().map(|n| (free, n))
}

pub(super) fn io_error(error: &std::io::Error) -> WalError {
    WalError::Io(error.kind())
}

/// Who writes headers: the shard and node of the log.
#[derive(Debug, Clone, Copy)]
pub(super) struct Owner {
    pub shard: u16,
    pub node: NodeId,
}

/// The whole contents of `file`.
async fn read_all<F: File>(file: &F) -> Result<Vec<u8>, WalError> {
    let len = file.len().await.map_err(|e| io_error(&e))?;
    let len = usize::try_from(len).map_err(|_| WalError::Io(std::io::ErrorKind::OutOfMemory))?;
    let mut bytes = vec![0; len];
    file.read_at(&mut bytes, 0)
        .await
        .map_err(|e| io_error(&e))?;
    Ok(bytes)
}

/// The recycle epoch in the header of `path`; 0 if the header is unreadable
/// (a free file whose rewrite was cut short: its old frames are seeded by
/// another file number, so they never read back).
async fn epoch_of<D: Disk>(disk: &D, path: &Path) -> u64 {
    let Ok(file) = disk.open(path, OpenOptions::default()).await else {
        return 0;
    };
    let mut header = vec![0; HEADER_LEN];
    match file.read_at(&mut header, 0).await {
        Ok(n) if n == HEADER_LEN => FileHeader::decode(&header).map_or(0, |h| h.recycle_epoch),
        _ => 0,
    }
}

/// Makes a file ready: created (or a free one reused), space reserved,
/// header written and synced, then renamed to its number.
pub(super) async fn prepare<D: Disk, C: Clock>(
    disk: &D,
    dir: &Path,
    owner: Owner,
    clock: &C,
    spec: Prepare,
) -> Result<D::File, WalError> {
    let temp = dir.join(free_name(spec.source.unwrap_or(spec.file_no)));
    let options = OpenOptions {
        create: spec.source.is_none(),
        ..OpenOptions::default()
    };
    let file = disk.open(&temp, options).await.map_err(|e| io_error(&e))?;
    file.allocate(spec.size).await.map_err(|e| io_error(&e))?;
    let header = FileHeader {
        shard: owner.shard,
        file_no: spec.file_no,
        node: owner.node,
        created_at: clock.unix_micros(),
        recycle_epoch: spec.epoch,
        recycled: spec.source.is_some(),
    };
    file.write_all_sync_at(&header.encode(), 0)
        .await
        .map_err(|e| io_error(&e))?;
    disk.rename(&temp, &dir.join(file_name(spec.file_no)))
        .await
        .map_err(|e| io_error(&e))?;
    disk.sync_dir(dir).await.map_err(|e| io_error(&e))?;
    Ok(file)
}

/// Opens the log in `dir`: resumes after the last valid frame of the last
/// file, or creates the first file. Returns the state and the open file.
pub(super) async fn open_log<D: Disk, C: Clock>(
    disk: &D,
    dir: &Path,
    owner: Owner,
    clock: &C,
    file_size: u64,
) -> Result<(State, Vec<(u64, D::File)>), WalError> {
    disk.create_dir_all(dir).await.map_err(|e| io_error(&e))?;
    let entries = disk.list_dir(dir).await.map_err(|e| io_error(&e))?;
    let mut numbered = BTreeMap::new();
    let mut free = Vec::new();
    for path in entries {
        match parse(&path) {
            Some((true, n)) => free.push((n, epoch_of(disk, &path).await)),
            Some((false, n)) => {
                numbered.insert(n, path);
            }
            None => {}
        }
    }
    free.sort_unstable();
    let max = numbered
        .keys()
        .copied()
        .chain(free.iter().map(|f| f.0))
        .max();
    let next_file_no = max.map_or(0, |n| n + 1);
    let mut free: VecDeque<(u64, u64)> = free.into();
    let Some((&last, last_path)) = numbered.iter().next_back() else {
        let (state, no, file) =
            create_first(disk, dir, owner, clock, file_size, next_file_no, free).await?;
        return Ok((state, vec![(no, file)]));
    };
    let mut epochs = BTreeMap::new();
    for (&n, path) in &numbered {
        epochs.insert(n, epoch_of(disk, path).await);
    }
    let file = disk
        .open(last_path, OpenOptions::default())
        .await
        .map_err(|e| io_error(&e))?;
    let (current, epoch) = resume(&file, last).await?;
    free.retain(|f| f.0 != last);
    // The last file may be the next one, prepared ahead and still empty:
    // the log continues in the file before it, and this one stays next.
    let previous = numbered.range(..last).next_back();
    if let (true, Some((&prev, prev_path))) = (current.used() == HEADER_LEN as u64, previous) {
        let prev_file = disk
            .open(prev_path, OpenOptions::default())
            .await
            .map_err(|e| io_error(&e))?;
        let (prev_current, _) = resume(&prev_file, prev).await?;
        let mut next = FileBuf::fresh(last, epoch, None, current.size(), HEADER_LEN as u64);
        next.set_prepared();
        let state = State::new(file_size, prev_current, next_file_no, epochs, free).with_next(next);
        return Ok((state, vec![(prev, prev_file), (last, file)]));
    }
    let state = State::new(file_size, current, next_file_no, epochs, free);
    Ok((state, vec![(last, file)]))
}

async fn create_first<D: Disk, C: Clock>(
    disk: &D,
    dir: &Path,
    owner: Owner,
    clock: &C,
    file_size: u64,
    file_no: u64,
    mut free: VecDeque<(u64, u64)>,
) -> Result<(State, u64, D::File), WalError> {
    let (epoch, source) = match free.pop_front() {
        Some((n, e)) => (e.wrapping_add(1), Some(n)),
        None => (0, None),
    };
    let spec = Prepare {
        file_no,
        epoch,
        source,
        size: file_size,
    };
    let file = prepare(disk, dir, owner, clock, spec).await?;
    let mut current = FileBuf::fresh(file_no, epoch, None, file_size, HEADER_LEN as u64);
    current.set_prepared();
    let epochs = BTreeMap::from([(file_no, epoch)]);
    let state = State::new(file_size, current, file_no + 1, epochs, free);
    Ok((state, file_no, file))
}

/// Finds the end of the last file and erases the next `MAX_FLUSH` bytes, so
/// no frame of a torn flush can come back behind a new record (spec
/// section 5). Returns the file's buffer and its epoch.
async fn resume<F: File>(file: &F, file_no: u64) -> Result<(FileBuf, u64), WalError> {
    let bytes = read_all(file).await?;
    let mut reader = FileReader::new(&bytes)?;
    let epoch = reader.header().recycle_epoch;
    let end = loop {
        match reader.next_step()? {
            Step::Frame(_) => {}
            Step::End { valid_len } => break valid_len,
            Step::Corrupt { offset } => return Err(WalError::Corrupt { file_no, offset }),
        }
    };
    let size = bytes.len() as u64;
    // The writer only makes files of whole pages: another length means the
    // file was cut by someone else, and its end cannot be trusted.
    if !size.is_multiple_of(PAGE) {
        return Err(WalError::Corrupt {
            file_no,
            offset: end,
        });
    }
    let start = end - end % PAGE;
    let erase_to = (end + MAX_FLUSH as u64).div_ceil(PAGE) * PAGE;
    let erase_to = erase_to.min(size);
    let carry = bytes[start as usize..end as usize].to_vec();
    if erase_to > end {
        let mut buf = carry.clone();
        buf.resize((erase_to - start) as usize, 0);
        file.write_all_sync_at(&buf, start)
            .await
            .map_err(|e| io_error(&e))?;
    }
    Ok((FileBuf::resumed(file_no, epoch, size, end, carry), epoch))
}

/// The path of numbered file `file_no` and of its free name.
pub(super) fn release_paths(dir: &Path, file_no: u64) -> (PathBuf, PathBuf) {
    (dir.join(file_name(file_no)), dir.join(free_name(file_no)))
}
