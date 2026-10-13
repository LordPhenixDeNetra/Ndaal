//! Byte encodings of FORMATS_DE_DONNEES.md section 1: LEB128 `varint`, `zigzag`,
//! length-prefixed byte strings and little-endian fixed-size integers.
//!
//! Decoders read from `&mut &[u8]` and advance the slice only on success: on
//! error the input is left untouched. They never panic on hostile input.

/// Error while decoding bytes.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum DecodeError {
    /// The input ended before the value was complete.
    #[error("unexpected end of input: needed {needed} bytes, {remaining} remaining")]
    UnexpectedEnd {
        /// Bytes needed to complete the value.
        needed: usize,
        /// Bytes left in the input.
        remaining: usize,
    },
    /// A varint longer than the 10 bytes a `u64` can need.
    #[error("varint longer than 10 bytes")]
    VarintTooLong,
    /// A varint whose value does not fit in a `u64`.
    #[error("varint overflows u64")]
    VarintOverflow,
    /// A varint with a superfluous trailing zero group (more than one encoding per value).
    #[error("varint is not minimally encoded")]
    VarintNotMinimal,
    /// Bytes that are not valid UTF-8 where a string is expected.
    #[error("invalid UTF-8")]
    InvalidUtf8,
    /// A well-formed value outside its allowed range or syntax.
    #[error("invalid value: {0}")]
    InvalidValue(&'static str),
}

/// Longest varint for a `u64`: ceil(64 / 7) groups.
const MAX_VARINT_LEN: usize = 10;

/// Appends `v` as an unsigned LEB128 varint (1 to 10 bytes).
pub fn put_varint(out: &mut Vec<u8>, mut v: u64) {
    while v >= 0x80 {
        // Truncation intended: the low 7 bits plus the continuation bit.
        out.push((v as u8) | 0x80);
        v >>= 7;
    }
    out.push(v as u8);
}

/// Reads an unsigned LEB128 varint.
pub fn get_varint(input: &mut &[u8]) -> Result<u64, DecodeError> {
    let mut value: u64 = 0;
    for (i, &byte) in input.iter().enumerate() {
        if i == MAX_VARINT_LEN - 1 {
            // Tenth group: only bit 63 is left, and no continuation is possible.
            if byte & 0x80 != 0 {
                return Err(DecodeError::VarintTooLong);
            }
            if byte > 1 {
                return Err(DecodeError::VarintOverflow);
            }
        }
        value |= u64::from(byte & 0x7F) << (7 * i);
        if byte & 0x80 == 0 {
            if byte == 0 && i > 0 {
                return Err(DecodeError::VarintNotMinimal);
            }
            *input = &input[i + 1..];
            return Ok(value);
        }
    }
    Err(DecodeError::UnexpectedEnd {
        needed: input.len() + 1,
        remaining: input.len(),
    })
}

/// Maps signed to unsigned so that small magnitudes stay small: 0, -1, 1, -2 -> 0, 1, 2, 3.
pub fn zigzag_encode(v: i64) -> u64 {
    // Bit reinterpretation intended (same width).
    ((v << 1) ^ (v >> 63)) as u64
}

/// Inverse of [`zigzag_encode`].
pub fn zigzag_decode(v: u64) -> i64 {
    // Bit reinterpretation intended (same width).
    ((v >> 1) as i64) ^ -((v & 1) as i64)
}

/// Appends a varint length followed by the bytes.
pub fn put_bytes(out: &mut Vec<u8>, b: &[u8]) {
    put_varint(out, b.len() as u64);
    out.extend_from_slice(b);
}

/// Reads a varint length followed by that many bytes, without copying.
pub fn get_bytes<'a>(input: &mut &'a [u8]) -> Result<&'a [u8], DecodeError> {
    let mut rest = *input;
    let len = get_varint(&mut rest)?;
    // Compare as u64: a huge announced length must not be cast or allocated.
    if len > rest.len() as u64 {
        return Err(DecodeError::UnexpectedEnd {
            needed: usize::try_from(len).unwrap_or(usize::MAX),
            remaining: rest.len(),
        });
    }
    let (bytes, tail) = rest.split_at(len as usize);
    *input = tail;
    Ok(bytes)
}

/// Takes exactly `N` bytes, or reports how many were missing.
fn take<const N: usize>(input: &mut &[u8]) -> Result<[u8; N], DecodeError> {
    let Some((head, tail)) = input.split_first_chunk::<N>() else {
        return Err(DecodeError::UnexpectedEnd {
            needed: N,
            remaining: input.len(),
        });
    };
    let head = *head;
    *input = tail;
    Ok(head)
}

/// Appends a UTF-8 string as length-prefixed bytes.
pub fn put_str(out: &mut Vec<u8>, s: &str) {
    put_bytes(out, s.as_bytes());
}

/// Reads a length-prefixed UTF-8 string, without copying.
pub fn get_str<'a>(input: &mut &'a [u8]) -> Result<&'a str, DecodeError> {
    let mut rest = *input;
    let bytes = get_bytes(&mut rest)?;
    let s = std::str::from_utf8(bytes).map_err(|_| DecodeError::InvalidUtf8)?;
    *input = rest;
    Ok(s)
}

/// Appends a little-endian `u16`.
pub fn put_u16_le(out: &mut Vec<u8>, v: u16) {
    out.extend_from_slice(&v.to_le_bytes());
}

/// Appends a little-endian `u32`.
pub fn put_u32_le(out: &mut Vec<u8>, v: u32) {
    out.extend_from_slice(&v.to_le_bytes());
}

/// Appends a little-endian `u64`.
pub fn put_u64_le(out: &mut Vec<u8>, v: u64) {
    out.extend_from_slice(&v.to_le_bytes());
}

/// Reads a little-endian `u16`.
pub fn get_u16_le(input: &mut &[u8]) -> Result<u16, DecodeError> {
    take::<2>(input).map(u16::from_le_bytes)
}

/// Reads a little-endian `u32`.
pub fn get_u32_le(input: &mut &[u8]) -> Result<u32, DecodeError> {
    take::<4>(input).map(u32::from_le_bytes)
}

/// Reads a little-endian `u64`.
pub fn get_u64_le(input: &mut &[u8]) -> Result<u64, DecodeError> {
    take::<8>(input).map(u64::from_le_bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    fn varint(v: u64) -> Vec<u8> {
        let mut out = Vec::new();
        put_varint(&mut out, v);
        out
    }

    #[test]
    fn varint_known_encodings() {
        assert_eq!(varint(0), [0x00]);
        assert_eq!(varint(127), [0x7F]);
        assert_eq!(varint(128), [0x80, 0x01]);
        assert_eq!(varint(300), [0xAC, 0x02]);
        assert_eq!(varint(u64::MAX).len(), 10);
    }

    #[test]
    fn zigzag_known_values() {
        let encoded: Vec<u64> = [0, -1, 1, -2].into_iter().map(zigzag_encode).collect();
        assert_eq!(encoded, [0, 1, 2, 3]);
        assert_eq!(zigzag_encode(i64::MIN), u64::MAX);
    }

    fn decode_varint(bytes: &[u8]) -> (Result<u64, DecodeError>, usize) {
        let mut input = bytes;
        let result = get_varint(&mut input);
        (result, input.len())
    }

    #[test]
    fn varint_errors_leave_input_untouched() {
        let cases: [(&[u8], DecodeError); 4] = [
            (
                &[],
                DecodeError::UnexpectedEnd {
                    needed: 1,
                    remaining: 0,
                },
            ),
            (&[0x80; 11], DecodeError::VarintTooLong),
            (
                &[0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0x7F],
                DecodeError::VarintOverflow,
            ),
            (&[0x80, 0x00], DecodeError::VarintNotMinimal),
        ];
        for (bytes, expected) in cases {
            assert_eq!(
                decode_varint(bytes),
                (Err(expected), bytes.len()),
                "{bytes:?}"
            );
        }
    }

    #[test]
    fn varint_truncated_continuation_is_unexpected_end() {
        let (result, remaining) = decode_varint(&[0x80, 0x80]);
        assert!(matches!(result, Err(DecodeError::UnexpectedEnd { .. })));
        assert_eq!(remaining, 2);
    }

    #[test]
    fn huge_length_prefix_is_unexpected_end_without_allocation() {
        let mut bytes = varint(u64::MAX);
        bytes.extend_from_slice(&[1, 2, 3]);
        let mut input = bytes.as_slice();
        assert!(matches!(
            get_bytes(&mut input),
            Err(DecodeError::UnexpectedEnd { .. })
        ));
        assert_eq!(input.len(), bytes.len());
    }

    #[test]
    fn invalid_utf8_is_rejected() {
        let mut input: &[u8] = &[0x01, 0xFF];
        assert_eq!(get_str(&mut input), Err(DecodeError::InvalidUtf8));
        assert_eq!(input.len(), 2);
    }

    #[test]
    fn short_fixed_integers_are_unexpected_end() {
        let mut input: &[u8] = &[1, 2, 3];
        assert_eq!(
            get_u32_le(&mut input),
            Err(DecodeError::UnexpectedEnd {
                needed: 4,
                remaining: 3
            })
        );
        assert_eq!(input.len(), 3);
    }

    proptest! {
        #[test]
        fn varint_round_trip(v: u64) {
            let bytes = varint(v);
            let mut input = bytes.as_slice();
            prop_assert_eq!(get_varint(&mut input), Ok(v));
            prop_assert!(input.is_empty());
        }

        #[test]
        fn zigzag_round_trip(v: i64) {
            prop_assert_eq!(zigzag_decode(zigzag_encode(v)), v);
        }

        #[test]
        fn mixed_sequence_round_trip(
            a: u64, b: Vec<u8>, c: String, d: u16, e: u32, f: u64, g: i64
        ) {
            let mut out = Vec::new();
            put_varint(&mut out, a);
            put_bytes(&mut out, &b);
            put_str(&mut out, &c);
            put_u16_le(&mut out, d);
            put_u32_le(&mut out, e);
            put_u64_le(&mut out, f);
            put_varint(&mut out, zigzag_encode(g));
            let mut input = out.as_slice();
            prop_assert_eq!(get_varint(&mut input), Ok(a));
            prop_assert_eq!(get_bytes(&mut input), Ok(b.as_slice()));
            prop_assert_eq!(get_str(&mut input), Ok(c.as_str()));
            prop_assert_eq!(get_u16_le(&mut input), Ok(d));
            prop_assert_eq!(get_u32_le(&mut input), Ok(e));
            prop_assert_eq!(get_u64_le(&mut input), Ok(f));
            prop_assert_eq!(get_varint(&mut input).map(zigzag_decode), Ok(g));
            prop_assert!(input.is_empty());
        }

        #[test]
        fn decoders_never_panic(bytes: Vec<u8>) {
            let mut input = bytes.as_slice();
            let _ = get_varint(&mut input);
            let mut input = bytes.as_slice();
            let _ = get_bytes(&mut input);
            let mut input = bytes.as_slice();
            let _ = get_str(&mut input);
            let mut input = bytes.as_slice();
            let _ = get_u64_le(&mut input);
        }
    }
}
