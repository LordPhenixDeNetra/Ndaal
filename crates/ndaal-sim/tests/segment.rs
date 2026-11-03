//! The generic segment envelope (T02.13, ENF-DUR-03): writing, reading,
//! refusing every corruption.

use std::io;
use std::path::Path;
use std::pin::pin;
use std::task::{Context, Poll, Waker};

use ndaal_common::ids::{Lsn, PartitionId, SegmentId};
use ndaal_io::sim::{CrashMode, SimDisk};
use ndaal_io::{Disk, File, OpenOptions};
use ndaal_storage::segment::{
    BlockHandle, Compression, Footer, SECTION_ALIGN, SegmentError, SegmentHeader, SegmentKind,
    SegmentReader, SegmentWriter,
};

const PATH: &str = "/segment-1";

/// Runs a future that never waits: the simulated disk answers at once.
fn now<F: Future>(future: F) -> F::Output {
    let mut future = pin!(future);
    match future
        .as_mut()
        .poll(&mut Context::from_waker(Waker::noop()))
    {
        Poll::Ready(output) => output,
        Poll::Pending => panic!("a simulated disk operation waited"),
    }
}

fn header() -> SegmentHeader {
    SegmentHeader {
        kind: SegmentKind::Sst,
        kind_format: 1,
        segment_id: SegmentId {
            partition: PartitionId(3),
            lsn: Lsn(40),
            counter: 1,
        },
        partition: PartitionId(3),
        min_lsn: Lsn(1),
        max_lsn: Lsn(40),
    }
}

fn data(seed: u8, len: usize) -> Vec<u8> {
    (0..len)
        .map(|i| seed.wrapping_add((i % 13) as u8))
        .collect()
}

/// A written segment: its blocks as `(section, handle, data)`.
type Written = Vec<(usize, BlockHandle, Vec<u8>)>;

/// Writes `sections` (each a list of `(data, compression)`) to `PATH`.
fn write(disk: &SimDisk, sections: &[Vec<(Vec<u8>, Compression)>]) -> Written {
    let mut writer = now(SegmentWriter::create(disk, Path::new(PATH), header())).expect("create");
    let mut written = Vec::new();
    for (i, blocks) in sections.iter().enumerate() {
        writer.begin_section(10 + i as u32).expect("begin");
        for (bytes, compression) in blocks {
            let handle = now(writer.add_block(bytes, *compression)).expect("add");
            written.push((i, handle, bytes.clone()));
        }
        writer.end_section(vec![i as u8]).expect("end");
    }
    now(writer.finish()).expect("finish");
    written
}

fn three_sections() -> Vec<Vec<(Vec<u8>, Compression)>> {
    vec![
        vec![
            (data(1, 100), Compression::None),
            (data(2, 5000), Compression::None),
        ],
        vec![(data(3, 20_000), Compression::Lz4)],
        vec![
            (data(4, 1), Compression::Lz4),
            (data(5, 70_000), Compression::Lz4),
        ],
    ]
}

fn open(disk: &SimDisk) -> Result<SegmentReader<<SimDisk as Disk>::File>, SegmentError> {
    now(SegmentReader::open(disk, Path::new(PATH), SegmentKind::Sst))
}

fn file_len(disk: &SimDisk) -> u64 {
    let file = now(disk.open(Path::new(PATH), OpenOptions::default())).expect("open");
    now(file.len()).expect("len")
}

#[test]
fn write_then_read() {
    let disk = SimDisk::new(1);
    let written = write(&disk, &three_sections());
    let reader = open(&disk).expect("open");
    assert_eq!(*reader.header(), header());
    let sections = reader.sections();
    assert_eq!(sections.len(), 3);
    for (i, section) in sections.iter().enumerate() {
        assert_eq!(section.kind, 10 + i as u32);
        assert_eq!(section.properties, vec![i as u8]);
    }
    assert_eq!(sections[0].blocks, 2);
    for (_, handle, bytes) in &written {
        assert_eq!(&now(reader.read_block(*handle)).expect("read"), bytes);
    }
}

#[test]
fn sections_are_aligned_and_size_is_a_multiple() {
    let disk = SimDisk::new(2);
    write(&disk, &three_sections());
    let reader = open(&disk).expect("open");
    for section in reader.sections() {
        assert_eq!(section.offset % SECTION_ALIGN, 0, "{section:?}");
    }
    assert_eq!(reader.sections()[0].offset, 4096);
    assert_eq!(file_len(&disk) % 4096, 0);
}

#[test]
fn misuse_is_an_error() {
    let disk = SimDisk::new(3);
    let create = |path: &str| now(SegmentWriter::create(&disk, Path::new(path), header()));
    let is_misuse = |r: Result<(), SegmentError>| matches!(r, Err(SegmentError::Misuse(_)));

    let mut writer = create("/a").expect("create");
    let outside = now(writer.add_block(b"x", Compression::None)).map(|_| ());
    assert!(is_misuse(outside));

    let mut writer = create("/b").expect("create");
    writer.begin_section(1).expect("begin");
    assert!(is_misuse(writer.begin_section(2)));

    let mut writer = create("/c").expect("create");
    writer.begin_section(1).expect("begin");
    now(writer.add_block(b"x", Compression::None)).expect("add");
    assert!(is_misuse(now(writer.finish()).map(|_| ())));

    let mut writer = create("/d").expect("create");
    writer.begin_section(1).expect("begin");
    assert!(is_misuse(writer.end_section(Vec::new())));
}

#[test]
fn create_never_overwrites() {
    let disk = SimDisk::new(4);
    let written = write(&disk, &three_sections());
    let again = now(SegmentWriter::create(&disk, Path::new(PATH), header()));
    assert!(
        matches!(again, Err(SegmentError::Io(ref e)) if e.kind() == io::ErrorKind::AlreadyExists)
    );
    let reader = open(&disk).expect("still valid");
    assert_eq!(
        now(reader.read_block(written[0].1)).expect("read"),
        written[0].2
    );
}

#[test]
fn handles_outside_sections_are_refused() {
    let disk = SimDisk::new(5);
    let written = write(&disk, &three_sections());
    let reader = open(&disk).expect("open");
    let first = reader.sections()[0].clone();
    let second = reader.sections()[1].clone();
    let bad = [
        BlockHandle {
            offset: first.offset + first.len,
            len: 10,
        },
        BlockHandle {
            offset: second.offset - 3,
            len: 10,
        },
        BlockHandle {
            offset: written[0].1.offset,
            len: 4,
        },
        BlockHandle {
            offset: u64::MAX - 2,
            len: 10,
        },
        BlockHandle {
            offset: 0,
            len: 4096,
        },
    ];
    // Armed failure: a handle refused before any read leaves it armed.
    disk.fail_next(Path::new(PATH), io::ErrorKind::Other);
    for handle in bad {
        let err = now(reader.read_block(handle)).expect_err("outside");
        assert!(
            matches!(err, SegmentError::Corrupt { .. }),
            "{handle:?}: {err:?}"
        );
    }
    let valid = now(reader.read_block(written[0].1));
    assert!(matches!(valid, Err(SegmentError::Io(_))), "{valid:?}");
}

#[test]
fn odd_file_sizes_are_refused() {
    let disk = SimDisk::new(6);
    write(&disk, &three_sections());
    let len = file_len(&disk);
    let file = now(disk.open(Path::new(PATH), OpenOptions::default())).expect("open");
    now(file.write_all_at(&[0], len)).expect("grow");
    assert!(matches!(open(&disk), Err(SegmentError::Corrupt { .. })));

    let disk = SimDisk::new(7);
    write(&disk, &three_sections());
    let len = file_len(&disk);
    let file = now(disk.open(Path::new(PATH), OpenOptions::default())).expect("open");
    let mut bytes = vec![0; usize::try_from(len - 4096).expect("small")];
    now(file.read_at(&mut bytes, 0)).expect("read");
    let short = now(disk.open(
        Path::new("/short"),
        OpenOptions {
            create: true,
            ..OpenOptions::default()
        },
    ))
    .expect("create");
    now(short.write_all_at(&bytes, 0)).expect("write");
    let err = now(SegmentReader::open(
        &disk,
        Path::new("/short"),
        SegmentKind::Sst,
    ));
    assert!(matches!(err, Err(SegmentError::Corrupt { .. })));
}

mod property {
    use proptest::prelude::*;

    use super::*;

    fn compression() -> impl Strategy<Value = Compression> {
        prop_oneof![Just(Compression::None), Just(Compression::Lz4)]
    }

    fn block() -> impl Strategy<Value = (Vec<u8>, Compression)> {
        (1usize..40_000, any::<u8>(), 1u8..20, compression()).prop_map(|(len, seed, period, c)| {
            (
                (0..len)
                    .map(|i| seed.wrapping_add((i % usize::from(period)) as u8))
                    .collect(),
                c,
            )
        })
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(32))]

        #[test]
        fn random_segments_round_trip(
            sections in prop::collection::vec(prop::collection::vec(block(), 1..20), 1..5),
        ) {
            let disk = SimDisk::new(8);
            let written = write(&disk, &sections);
            let reader = open(&disk).expect("open");
            prop_assert_eq!(reader.sections().len(), sections.len());
            for (i, section) in reader.sections().iter().enumerate() {
                prop_assert_eq!(section.offset % SECTION_ALIGN, 0);
                prop_assert_eq!(section.blocks as usize, sections[i].len());
            }
            for (_, handle, bytes) in &written {
                prop_assert_eq!(&now(reader.read_block(*handle)).expect("read"), bytes);
            }
            prop_assert_eq!(file_len(&disk) % 4096, 0);
        }
    }
}

/// Offsets `[start, end)` of the file that no reader ever looks at: the
/// zero padding before each section, after the directory.
fn padding(disk: &SimDisk) -> Vec<(u64, u64)> {
    let reader = open(disk).expect("open");
    let len = file_len(disk);
    let file = now(disk.open(Path::new(PATH), OpenOptions::default())).expect("open");
    let mut bytes = vec![0; 4096];
    now(file.read_at(&mut bytes, len - 4096)).expect("footer");
    let footer = Footer::decode(&bytes).expect("footer");
    let mut gaps = Vec::new();
    let mut end = 4096;
    for section in reader.sections() {
        gaps.push((end, section.offset));
        end = section.offset + section.len;
    }
    gaps.push((end, footer.directory_offset));
    gaps.push((
        footer.directory_offset + u64::from(footer.directory_len),
        len - 4096,
    ));
    gaps
}

/// Opens the segment and reads every block of `written`; equal to the
/// original, or an error.
fn read_all(disk: &SimDisk, written: &Written) -> Result<(), SegmentError> {
    let reader = open(disk)?;
    for (_, handle, bytes) in written {
        let back = now(reader.read_block(*handle))?;
        assert_eq!(&back, bytes, "a corrupt block read back as valid data");
    }
    Ok(())
}

#[test]
fn every_corrupted_bit_is_detected() {
    let disk = SimDisk::new(9);
    let written = write(
        &disk,
        &[
            vec![
                (data(1, 700), Compression::None),
                (data(2, 900), Compression::Lz4),
            ],
            vec![
                (data(3, 300), Compression::Lz4),
                (data(4, 50), Compression::None),
            ],
        ],
    );
    let gaps = padding(&disk);
    let path = Path::new(PATH);
    for offset in 0..file_len(&disk) {
        disk.corrupt(path, offset).expect("flip");
        let result = read_all(&disk, &written);
        disk.corrupt(path, offset).expect("flip back");
        if gaps
            .iter()
            .any(|&(start, end)| (start..end).contains(&offset))
        {
            assert!(result.is_ok(), "padding at {offset}: {result:?}");
        } else {
            assert!(result.is_err(), "corruption at {offset} not detected");
        }
    }
}

#[test]
fn crash_before_finish_is_refused() {
    for seed in 0..20 {
        let disk = SimDisk::new(seed);
        let path = Path::new(PATH);
        let mut writer = now(SegmentWriter::create(&disk, path, header())).expect("create");
        now(disk.sync_dir(Path::new("/"))).expect("name durable");
        writer.begin_section(1).expect("begin");
        for i in 0..3 {
            // Three 1 MiB blocks: the writer flushes, the file has data.
            now(writer.add_block(&data(i, 1 << 20), Compression::None)).expect("add");
        }
        disk.crash(if seed % 2 == 0 {
            CrashMode::LoseAllUnsynced
        } else {
            CrashMode::Random
        });
        assert!(
            open(&disk).is_err(),
            "seed {seed}: unfinished segment opened"
        );
    }
}

#[test]
fn crash_after_finish_keeps_all_or_nothing() {
    for seed in 0..20 {
        let disk = SimDisk::new(seed);
        let written = write(&disk, &three_sections());
        disk.crash(CrashMode::Random);
        // The name is not durable (no sync_dir: the manifest's job), the
        // data is: the segment is gone or whole.
        match open(&disk) {
            Err(SegmentError::Io(e)) => assert_eq!(e.kind(), io::ErrorKind::NotFound),
            Err(other) => panic!("seed {seed}: {other:?}"),
            Ok(_) => read_all(&disk, &written).expect("whole"),
        }
    }
}

#[test]
fn io_errors_surface() {
    let disk = SimDisk::new(10);
    let path = Path::new(PATH);
    let mut writer = now(SegmentWriter::create(&disk, path, header())).expect("create");
    writer.begin_section(1).expect("begin");
    now(writer.add_block(b"block", Compression::None)).expect("add");
    writer.end_section(Vec::new()).expect("end");
    disk.fail_next(path, io::ErrorKind::Other);
    assert!(matches!(now(writer.finish()), Err(SegmentError::Io(_))));
}

#[test]
fn header_the_reader_refuses_is_refused_at_create() {
    let disk = SimDisk::new(11);
    let mut swapped = header();
    swapped.min_lsn = Lsn(41);
    let created = now(SegmentWriter::create(&disk, Path::new(PATH), swapped));
    assert!(
        matches!(created, Err(SegmentError::Misuse(_))),
        "{:?}",
        created.err()
    );
}

#[test]
fn directory_bounds_are_checked_before_any_read() {
    let disk = SimDisk::new(12);
    write(&disk, &three_sections());
    let len = file_len(&disk);
    let footer_at = len - 4096;
    let file = now(disk.open(Path::new(PATH), OpenOptions::default())).expect("open");
    let mut bytes = vec![0; 4096];
    now(file.read_at(&mut bytes, footer_at)).expect("footer");
    let good = Footer::decode(&bytes).expect("footer");
    let bad = [
        Footer {
            directory_offset: u64::MAX - 2,
            ..good
        },
        Footer {
            directory_offset: 100,
            ..good
        },
        Footer {
            directory_offset: footer_at,
            ..good
        },
        Footer {
            directory_len: u32::MAX,
            ..good
        },
        Footer {
            directory_len: (16 << 20) + 1,
            ..good
        },
    ];
    for footer in bad {
        now(file.write_all_at(&footer.encode(), footer_at)).expect("rewrite footer");
        // Refused by the bounds, before reading or allocating anything; a
        // later check (short read, CRC) would mean the bounds were skipped.
        let err = open(&disk).err();
        assert!(
            matches!(
                err,
                Some(SegmentError::Corrupt {
                    what: "directory bounds",
                    ..
                })
            ),
            "{footer:?}: {err:?}"
        );
    }
}
