//! Internal keys (FORMATS_DE_DONNEES.md section 9, T02.14): byte order is
//! logical order.

use std::borrow::Cow;
use std::cmp::Ordering;

use ndaal_common::ids::NamespaceId;
use ndaal_common::keys::{
    KeyType, get_escaped, get_prefix, get_u32_be, get_u64_be, inverted, put_escaped, put_prefix,
    put_u32_be, put_u64_be,
};
use proptest::prelude::*;

const TYPES: [KeyType; 8] = [
    KeyType::Kv,
    KeyType::Expiry,
    KeyType::ObjectMeta,
    KeyType::Multipart,
    KeyType::Staged,
    KeyType::VectorPayload,
    KeyType::LinkIndex,
    KeyType::System,
];

fn escaped(key: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    put_escaped(&mut out, key);
    out
}

#[test]
fn escape_round_trip() {
    for key in [&b""[..], b"a", b"a\0", b"\0\0", b"\0\xff"] {
        let mut bytes = escaped(key);
        bytes.extend_from_slice(b"rest");
        let mut input = &bytes[..];
        assert_eq!(get_escaped(&mut input).expect("valid").as_ref(), key);
        assert_eq!(input, b"rest");
    }
    assert_eq!(escaped(b""), [0, 0]);
    assert_eq!(escaped(b"a\0"), [b'a', 0, 0xff, 0, 0]);
}

#[test]
fn plain_key_is_borrowed() {
    let bytes = escaped(b"abc");
    let mut input = &bytes[..];
    assert!(matches!(get_escaped(&mut input), Ok(Cow::Borrowed(b"abc"))));
}

#[test]
fn invalid_escapes_are_refused() {
    let cases: [&[u8]; 5] = [
        &[0x00, 0x01],
        &[0x61],
        &[0x00],
        &[0x00, 0xff],
        &[0x61, 0x00, 0xff, 0x00],
    ];
    for bytes in cases {
        let mut input = bytes;
        assert!(get_escaped(&mut input).is_err(), "{bytes:?}");
        assert_eq!(input, bytes, "input moved on error");
    }
}

#[test]
fn prefix_round_trip_and_unknown_type() {
    for kind in TYPES {
        let mut bytes = Vec::new();
        put_prefix(&mut bytes, kind, NamespaceId(0x0102_0304));
        assert_eq!(bytes, [kind as u8, 1, 2, 3, 4]);
        let mut input = &bytes[..];
        assert_eq!(
            get_prefix(&mut input).expect("valid"),
            (kind, NamespaceId(0x0102_0304))
        );
        assert!(input.is_empty());
    }
    for bytes in [&[0x03, 0, 0, 0, 1][..], &[0x01, 0, 0, 0]] {
        let mut input = bytes;
        assert!(get_prefix(&mut input).is_err(), "{bytes:?}");
        assert_eq!(input, bytes);
    }
}

#[test]
fn be_round_trip() {
    let mut bytes = Vec::new();
    put_u32_be(&mut bytes, 0x0102_0304);
    put_u64_be(&mut bytes, 0x0506_0708_090a_0b0c);
    assert_eq!(bytes[..4], [1, 2, 3, 4]);
    let mut input = &bytes[..];
    assert_eq!(get_u32_be(&mut input), Ok(0x0102_0304));
    assert_eq!(get_u64_be(&mut input), Ok(0x0506_0708_090a_0b0c));
    assert!(input.is_empty());
    let mut short = &[1u8, 2, 3][..];
    assert!(get_u32_be(&mut short).is_err());
    assert_eq!(short.len(), 3);
    assert_eq!(inverted(0), u64::MAX);
}

/// Keys rich in 0x00 and 0xff, the bytes the escaping is about.
fn key() -> impl Strategy<Value = Vec<u8>> {
    prop::collection::vec(
        prop_oneof![Just(0u8), Just(0xff), Just(1), any::<u8>()],
        0..8,
    )
}

fn composed(t: &(usize, u32, u8, Vec<u8>, u64)) -> Vec<u8> {
    let mut out = Vec::new();
    put_prefix(&mut out, TYPES[t.0], NamespaceId(t.1));
    out.push(t.2);
    put_escaped(&mut out, &t.3);
    put_u64_be(&mut out, t.4);
    out
}

proptest! {
    #[test]
    fn escaped_order_is_key_order(a in key(), b in key()) {
        prop_assert_eq!(escaped(&a).cmp(&escaped(&b)), a.cmp(&b));
    }

    #[test]
    fn suffix_keeps_order(a in key(), x in key(), b in key(), y in key()) {
        let mut ea = escaped(&a);
        ea.extend_from_slice(&x);
        let mut eb = escaped(&b);
        eb.extend_from_slice(&y);
        prop_assert_eq!(ea.cmp(&eb), (&a, &x).cmp(&(&b, &y)));
    }

    #[test]
    fn be_order_is_numeric_order(a: u64, b: u64, c: u32, d: u32) {
        let (mut ea, mut eb) = (Vec::new(), Vec::new());
        put_u64_be(&mut ea, a);
        put_u64_be(&mut eb, b);
        prop_assert_eq!(ea.cmp(&eb), a.cmp(&b));
        let (mut ec, mut ed) = (Vec::new(), Vec::new());
        put_u32_be(&mut ec, c);
        put_u32_be(&mut ed, d);
        prop_assert_eq!(ec.cmp(&ed), c.cmp(&d));
    }

    #[test]
    fn inverted_reverses_order(a: u64, b: u64) {
        prop_assert_eq!(inverted(a).cmp(&inverted(b)), b.cmp(&a));
    }

    #[test]
    fn composed_key_order_is_tuple_order(
        a in (0usize..8, any::<u32>(), any::<u8>(), key(), any::<u64>()),
        b in (0usize..8, any::<u32>(), any::<u8>(), key(), any::<u64>()),
    ) {
        let tuple = |t: &(usize, u32, u8, Vec<u8>, u64)| {
            (TYPES[t.0] as u8, t.1, t.2, t.3.clone(), t.4)
        };
        let expected: Ordering = tuple(&a).cmp(&tuple(&b));
        prop_assert_eq!(composed(&a).cmp(&composed(&b)), expected);
    }
}

#[test]
fn errors_tell_what_is_left() {
    use ndaal_common::bytes::DecodeError;
    let mut input = &b"abc"[..];
    assert_eq!(
        get_escaped(&mut input),
        Err(DecodeError::UnexpectedEnd {
            needed: 2,
            remaining: 3
        })
    );
    let mut input = &b"ab\0"[..];
    assert_eq!(
        get_escaped(&mut input),
        Err(DecodeError::UnexpectedEnd {
            needed: 1,
            remaining: 3
        })
    );
}
