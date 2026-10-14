//! Fuzz target bodies (STRATEGIE_DE_TESTS.md section 5, ENF-SEC-03).
//!
//! Each target panics when a property is violated: that is how libFuzzer
//! reports a finding. The bodies live here, in the stable workspace, so the
//! versioned corpus is replayed by `cargo test` on every system; `fuzz/`
//! only wraps them for `cargo fuzz`.

#![forbid(unsafe_code)]

/// Every target, by name: the name of its `fuzz/fuzz_targets/` file and of
/// its corpus directory.
pub const TARGETS: &[(&str, Target)] = &[("common_codec", common_codec)];

/// A target body: panics when a property is violated.
pub type Target = fn(&[u8]);

use ndaal_common::base32::{decode_crockford, encode_crockford};
use ndaal_common::bytes::{self, DecodeError};

/// Decoders of `ndaal_common::bytes` and `ndaal_common::base32`: never panic,
/// and whatever decodes re-encodes to exactly the bytes it consumed (every
/// encoding is canonical: varints are minimal).
pub fn common_codec(data: &[u8]) {
    round_trip(data, bytes::get_varint, |out, v| bytes::put_varint(out, *v));
    round_trip(data, bytes::get_u16_le, |out, v| bytes::put_u16_le(out, *v));
    round_trip(data, bytes::get_u32_le, |out, v| bytes::put_u32_le(out, *v));
    round_trip(data, bytes::get_u64_le, |out, v| bytes::put_u64_le(out, *v));
    round_trip(data, bytes::get_bytes, |out, v| bytes::put_bytes(out, v));
    round_trip(data, bytes::get_str, |out, v| bytes::put_str(out, v));
    let mut input = data;
    if let Ok(v) = bytes::get_varint(&mut input) {
        assert_eq!(bytes::zigzag_encode(bytes::zigzag_decode(v)), v);
    }
    if let Ok(text) = std::str::from_utf8(data)
        && let Ok(value) = decode_crockford(text)
    {
        // Decoding accepts lower case and I/L/O aliases; encoding is canonical.
        let canonical = encode_crockford(&value);
        assert_eq!(decode_crockford(&canonical), Ok(value), "{text}");
    }
}

/// Decodes one value from `data`; on success, `encode` must rebuild the
/// consumed prefix exactly.
fn round_trip<'a, T>(
    data: &'a [u8],
    decode: impl Fn(&mut &'a [u8]) -> Result<T, DecodeError>,
    encode: impl Fn(&mut Vec<u8>, &T),
) {
    let mut input = data;
    if let Ok(value) = decode(&mut input) {
        let consumed = &data[..data.len() - input.len()];
        let mut out = Vec::new();
        encode(&mut out, &value);
        assert_eq!(out, consumed, "re-encoding differs from the input");
    }
}
