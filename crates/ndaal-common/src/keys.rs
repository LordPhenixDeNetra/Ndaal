//! Internal keys of FORMATS_DE_DONNEES.md section 9 (T02.14): every ordered
//! entry of a partition shares one key space sorted by bytes. Keys start
//! with `type (1) . namespace_id (4, big-endian)`; user keys are escaped
//! and integers big-endian, so byte order is logical order.
//!
//! Same conventions as [`crate::bytes`]: encoders append, decoders read
//! from `&mut &[u8]` and advance only on success.

use std::borrow::Cow;

use crate::bytes::{DecodeError, take};
use crate::ids::NamespaceId;

/// First byte of an internal key: what the entry is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum KeyType {
    /// Key-value entry: `db (1) . escaped user key`.
    Kv = 0x01,
    /// Expiry: `db (1) . expire_at (8) . key`.
    Expiry = 0x02,
    /// Object metadata: `bucket_id (4) . escaped object key . inverted version (8)`.
    ObjectMeta = 0x10,
    /// Multipart upload in progress.
    Multipart = 0x11,
    /// Staged object: `staged_id (16)`.
    Staged = 0x12,
    /// Vector payload of a mutable segment: `collection_id (4) . point_id`.
    VectorPayload = 0x20,
    /// Reverse index of links.
    LinkIndex = 0x30,
    /// The partition's own data.
    System = 0x7F,
}

impl KeyType {
    fn from_u8(value: u8) -> Option<Self> {
        Some(match value {
            0x01 => Self::Kv,
            0x02 => Self::Expiry,
            0x10 => Self::ObjectMeta,
            0x11 => Self::Multipart,
            0x12 => Self::Staged,
            0x20 => Self::VectorPayload,
            0x30 => Self::LinkIndex,
            0x7F => Self::System,
            _ => return None,
        })
    }
}

/// Appends `type . namespace_id`.
pub fn put_prefix(out: &mut Vec<u8>, kind: KeyType, namespace: NamespaceId) {
    out.push(kind as u8);
    put_u32_be(out, namespace.0);
}

/// Reads `type . namespace_id`; an unknown type is refused.
pub fn get_prefix(input: &mut &[u8]) -> Result<(KeyType, NamespaceId), DecodeError> {
    let bytes: [u8; 5] = {
        let mut peek = *input;
        take::<5>(&mut peek)?
    };
    let kind = KeyType::from_u8(bytes[0]).ok_or(DecodeError::InvalidValue("unknown key type"))?;
    let namespace = NamespaceId(u32::from_be_bytes([bytes[1], bytes[2], bytes[3], bytes[4]]));
    *input = &input[5..];
    Ok((kind, namespace))
}

/// Appends `key` escaped: each `0x00` becomes `0x00 0xff`, and `0x00 0x00`
/// ends it. Escaped keys sort as the keys, whatever follows them.
pub fn put_escaped(out: &mut Vec<u8>, key: &[u8]) {
    for &byte in key {
        out.push(byte);
        if byte == 0 {
            out.push(0xff);
        }
    }
    out.extend_from_slice(&[0, 0]);
}

/// Reads an escaped key: borrowed when it holds no `0x00`, copied
/// otherwise.
pub fn get_escaped<'a>(input: &mut &'a [u8]) -> Result<Cow<'a, [u8]>, DecodeError> {
    let bytes = *input;
    let mut owned: Option<Vec<u8>> = None;
    // `bytes[run..]` is not copied into `owned` yet.
    let mut run = 0;
    loop {
        let Some(zero) = bytes[run..].iter().position(|&b| b == 0).map(|p| run + p) else {
            return Err(DecodeError::UnexpectedEnd {
                needed: 2,
                remaining: bytes.len(),
            });
        };
        match bytes.get(zero + 1) {
            Some(0x00) => {
                let key = match owned {
                    None => Cow::Borrowed(&bytes[..zero]),
                    Some(mut key) => {
                        key.extend_from_slice(&bytes[run..zero]);
                        Cow::Owned(key)
                    }
                };
                *input = &bytes[zero + 2..];
                return Ok(key);
            }
            Some(0xff) => {
                // Keeps the 0x00, drops the 0xff.
                owned
                    .get_or_insert_with(Vec::new)
                    .extend_from_slice(&bytes[run..=zero]);
                run = zero + 2;
            }
            Some(_) => {
                return Err(DecodeError::InvalidValue(
                    "escaped key: 0x00 followed by neither 0x00 nor 0xff",
                ));
            }
            None => {
                return Err(DecodeError::UnexpectedEnd {
                    needed: 1,
                    remaining: bytes.len(),
                });
            }
        }
    }
}

/// Appends a big-endian `u32`: byte order is numeric order.
pub fn put_u32_be(out: &mut Vec<u8>, v: u32) {
    out.extend_from_slice(&v.to_be_bytes());
}

/// Appends a big-endian `u64`: byte order is numeric order.
pub fn put_u64_be(out: &mut Vec<u8>, v: u64) {
    out.extend_from_slice(&v.to_be_bytes());
}

/// Reads a big-endian `u32`.
pub fn get_u32_be(input: &mut &[u8]) -> Result<u32, DecodeError> {
    take::<4>(input).map(u32::from_be_bytes)
}

/// Reads a big-endian `u64`.
pub fn get_u64_be(input: &mut &[u8]) -> Result<u64, DecodeError> {
    take::<8>(input).map(u64::from_be_bytes)
}

/// `u64::MAX - version`: encoded big-endian, the newest version sorts first.
pub fn inverted(version: u64) -> u64 {
    u64::MAX - version
}
