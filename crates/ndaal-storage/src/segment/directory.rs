//! Section directory (FORMATS_DE_DONNEES.md section 3.3): protobuf
//! (ADR-015), with messages derived by hand: no `protoc` (C-01).

use prost::Message;

use crate::segment::format::{HEADER_LEN, corrupt};
use crate::segment::{SECTION_ALIGN, SegmentError};

/// Largest directory: 16 MiB. Bounds what a corrupt footer can make the
/// reader allocate.
pub const MAX_DIRECTORY_LEN: usize = 16 << 20;

/// `message SectionDirectory { repeated SectionEntry sections = 1; }`
#[derive(Clone, PartialEq, Message)]
struct SectionDirectory {
    #[prost(message, repeated, tag = "1")]
    sections: Vec<SectionEntry>,
}

/// `message SectionEntry` (FORMATS_DE_DONNEES.md section 3.3).
#[derive(Clone, PartialEq, Message)]
struct SectionEntry {
    #[prost(uint32, tag = "1")]
    kind: u32,
    #[prost(uint64, tag = "2")]
    offset: u64,
    #[prost(uint64, tag = "3")]
    len: u64,
    #[prost(uint32, tag = "4")]
    blocks: u32,
    #[prost(bytes = "vec", tag = "5")]
    properties: Vec<u8>,
}

/// One section of a segment: a run of blocks the engine interprets.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Section {
    /// Section type, defined by the segment's kind.
    pub kind: u32,
    /// Offset of its first block, a multiple of 4 096.
    pub offset: u64,
    /// Bytes of blocks, padding excluded.
    pub len: u64,
    /// Number of blocks.
    pub blocks: u32,
    /// The engine's own properties, opaque to the envelope.
    pub properties: Vec<u8>,
}

pub(crate) fn encode_directory(sections: &[Section]) -> Vec<u8> {
    SectionDirectory {
        sections: sections
            .iter()
            .map(|s| SectionEntry {
                kind: s.kind,
                offset: s.offset,
                len: s.len,
                blocks: s.blocks,
                properties: s.properties.clone(),
            })
            .collect(),
    }
    .encode_to_vec()
}

/// The sections of a directory read at `directory_offset`, once they are
/// checked: sorted, aligned, disjoint, between the header and the
/// directory, none empty. Its size and CRC are the caller's to check.
pub fn decode_directory(bytes: &[u8], directory_offset: u64) -> Result<Vec<Section>, SegmentError> {
    let at = |what| corrupt(what, directory_offset);
    let directory = SectionDirectory::decode(bytes).map_err(|_| at("directory protobuf"))?;
    let mut end = HEADER_LEN as u64;
    let mut sections = Vec::with_capacity(directory.sections.len());
    for entry in directory.sections {
        if entry.len == 0 || entry.blocks == 0 {
            return Err(at("empty section"));
        }
        if entry.offset < end || entry.offset % SECTION_ALIGN != 0 {
            return Err(at("section offset"));
        }
        end = entry
            .offset
            .checked_add(entry.len)
            .filter(|&e| e <= directory_offset)
            .ok_or(at("section end"))?;
        sections.push(Section {
            kind: entry.kind,
            offset: entry.offset,
            len: entry.len,
            blocks: entry.blocks,
            properties: entry.properties,
        });
    }
    Ok(sections)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::segment::SegmentError;

    fn section(offset: u64, len: u64) -> Section {
        Section {
            kind: 1,
            offset,
            len,
            blocks: 1,
            properties: Vec::new(),
        }
    }

    #[test]
    fn directory_round_trip() {
        let sections = vec![
            Section {
                properties: b"props".to_vec(),
                ..section(4096, 5000)
            },
            Section {
                kind: 7,
                blocks: 3,
                ..section(12_288, 10)
            },
        ];
        let bytes = encode_directory(&sections);
        assert_eq!(decode_directory(&bytes, 16_384).expect("valid"), sections);
        assert_eq!(
            decode_directory(&encode_directory(&[]), 4096).expect("empty"),
            vec![]
        );
    }

    #[test]
    fn wire_tags_match_the_proto() {
        // SectionDirectory { sections (1): SectionEntry { kind (1) = 1,
        // offset (2) = 4096, len (3) = 5, blocks (4) = 1 } }.
        let expected = [
            0x0a, 0x09, 0x08, 0x01, 0x10, 0x80, 0x20, 0x18, 0x05, 0x20, 0x01,
        ];
        assert_eq!(encode_directory(&[section(4096, 5)]), expected);
    }

    #[test]
    fn invalid_sections_are_refused() {
        let cases: [(&str, Vec<Section>, u64); 8] = [
            (
                "overlap",
                vec![section(4096, 5000), section(8192, 10)],
                16_384,
            ),
            ("order", vec![section(8192, 10), section(4096, 10)], 16_384),
            ("alignment", vec![section(4097, 10)], 16_384),
            ("before the header", vec![section(0, 10)], 16_384),
            ("past the directory", vec![section(4096, 5000)], 8192),
            ("overflow", vec![section(4096, u64::MAX)], 16_384),
            ("empty", vec![section(4096, 0)], 16_384),
            (
                "no block",
                vec![Section {
                    blocks: 0,
                    ..section(4096, 10)
                }],
                16_384,
            ),
        ];
        for (name, sections, directory_offset) in cases {
            let bytes = encode_directory(&sections);
            let err = decode_directory(&bytes, directory_offset).expect_err(name);
            assert!(
                matches!(err, SegmentError::Corrupt { .. }),
                "{name}: {err:?}"
            );
        }
    }

    #[test]
    fn garbage_is_refused() {
        for bytes in [&[0xff, 0xff, 0xff][..], &[0x0a, 0x7f], &[0x0b]] {
            assert!(decode_directory(bytes, 8192).is_err(), "{bytes:?}");
        }
    }
}
