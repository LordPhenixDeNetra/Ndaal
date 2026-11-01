//! The file header of the log and its masked CRC (FORMATS_DE_DONNEES.md
//! sections 1 and 2.2, T02.07).

use ndaal_common::ids::NodeId;
use ndaal_wal::{FileHeader, HEADER_LEN, WalError, masked_crc};
use proptest::prelude::*;

fn header() -> FileHeader {
    FileHeader {
        shard: 3,
        file_no: 0xa2,
        node: NodeId([7; 16]),
        created_at: 1_767_225_600_000_000,
        recycle_epoch: 5,
        recycled: false,
    }
}

#[test]
fn masked_crc_known_value() {
    // CRC32C("") = 0 and CRC32C("123456789") = 0xe3069283, masked by
    // ((crc >> 15) | (crc << 17)) + 0xa282ead8, computed by hand.
    assert_eq!(masked_crc(b""), 0xa282_ead8);
    assert_eq!(masked_crc(b"123456789"), 0xc78a_b0e5);
}

proptest! {
    #[test]
    fn header_round_trip(
        shard in any::<u16>(),
        file_no in any::<u64>(),
        node in any::<[u8; 16]>(),
        created_at in any::<u64>(),
        recycle_epoch in any::<u64>(),
        recycled in any::<bool>(),
    ) {
        let h = FileHeader { shard, file_no, node: NodeId(node), created_at, recycle_epoch, recycled };
        let bytes = h.encode();
        prop_assert_eq!(bytes.len(), HEADER_LEN);
        prop_assert_eq!(&bytes[..8], b"NDLWAL01");
        prop_assert_eq!(FileHeader::decode(&bytes).expect("decode"), h);
    }
}

#[test]
fn header_rejects() {
    let good = header().encode();
    assert_eq!(FileHeader::decode(&[]), Err(WalError::ShortHeader));
    assert_eq!(
        FileHeader::decode(&good[..HEADER_LEN - 1]),
        Err(WalError::ShortHeader)
    );

    let mut magic = good;
    magic[0] = b'X';
    assert_eq!(FileHeader::decode(&magic), Err(WalError::BadMagic));

    // A newer format, with a valid CRC: refused by version, not by CRC.
    let mut newer = good;
    newer[8..10].copy_from_slice(&2u16.to_le_bytes());
    let crc = masked_crc(&newer[..HEADER_LEN - 4]);
    newer[HEADER_LEN - 4..].copy_from_slice(&crc.to_le_bytes());
    assert_eq!(FileHeader::decode(&newer), Err(WalError::UnknownFormat(2)));

    let mut encrypted = good;
    encrypted[12] |= 1;
    let crc = masked_crc(&encrypted[..HEADER_LEN - 4]);
    encrypted[HEADER_LEN - 4..].copy_from_slice(&crc.to_le_bytes());
    assert!(matches!(
        FileHeader::decode(&encrypted),
        Err(WalError::Unsupported(_))
    ));

    for byte in 8..HEADER_LEN {
        let mut flipped = good;
        flipped[byte] ^= 0x10;
        assert!(FileHeader::decode(&flipped).is_err(), "byte {byte}");
    }
}
