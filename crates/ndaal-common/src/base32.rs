//! Crockford base32 for 128-bit values (FORMATS_DE_DONNEES.md section 10).
//!
//! 128 bits are written as 26 characters: the first carries the top 3 bits
//! (so it is at most `7`), the next 25 carry 5 bits each. Encoding preserves
//! byte order: comparing two encodings as strings compares the values.

use crate::bytes::DecodeError;

/// Length of an encoded 128-bit value.
pub const ENCODED_LEN: usize = 26;

const ALPHABET: &[u8; 32] = b"0123456789ABCDEFGHJKMNPQRSTVWXYZ";

/// Encodes 16 bytes (big-endian value) as 26 uppercase Crockford characters.
pub fn encode_crockford(bytes: &[u8; 16]) -> String {
    let value = u128::from_be_bytes(*bytes);
    (0..ENCODED_LEN)
        .map(|i| {
            let shift = 5 * (ENCODED_LEN - 1 - i);
            // Masked to 5 bits: always a valid index.
            char::from(ALPHABET[((value >> shift) & 0x1F) as usize])
        })
        .collect()
}

/// Decodes 26 Crockford characters; case-insensitive, `I`/`L` read as 1, `O` as 0.
pub fn decode_crockford(s: &str) -> Result<[u8; 16], DecodeError> {
    if s.len() != ENCODED_LEN {
        return Err(DecodeError::InvalidValue(
            "base32 value must be 26 characters",
        ));
    }
    let mut value: u128 = 0;
    for (i, c) in s.bytes().enumerate() {
        let digit = digit_value(c).ok_or(DecodeError::InvalidValue("invalid base32 character"))?;
        if i == 0 && digit > 7 {
            return Err(DecodeError::InvalidValue("base32 value exceeds 128 bits"));
        }
        value = (value << 5) | u128::from(digit);
    }
    Ok(value.to_be_bytes())
}

fn digit_value(c: u8) -> Option<u8> {
    match c.to_ascii_uppercase() {
        b'O' => Some(0),
        b'I' | b'L' => Some(1),
        b'U' => None,
        upper => ALPHABET.iter().position(|&a| a == upper).map(|p| p as u8),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    #[test]
    fn known_values() {
        assert_eq!(encode_crockford(&[0; 16]), "0".repeat(26));
        assert_eq!(
            encode_crockford(&[0xFF; 16]),
            format!("7{}", "Z".repeat(25))
        );
        let mut one = [0u8; 16];
        one[15] = 1;
        assert_eq!(encode_crockford(&one), format!("{}1", "0".repeat(25)));
    }

    #[test]
    fn lenient_decoding() {
        let lower = "7zzzzzzzzzzzzzzzzzzzzzzzzz";
        assert_eq!(decode_crockford(lower), Ok([0xFF; 16]));
        let mut one = [0u8; 16];
        one[15] = 1;
        assert_eq!(decode_crockford(&format!("{}I", "O".repeat(25))), Ok(one));
        assert_eq!(decode_crockford(&format!("{}l", "o".repeat(25))), Ok(one));
    }

    #[test]
    fn invalid_inputs_are_rejected() {
        for s in [
            "",
            &"0".repeat(25),
            &"0".repeat(27),
            &format!("{}U", "0".repeat(25)),
            &format!("{}!", "0".repeat(25)),
            &format!("8{}", "0".repeat(25)),
            &format!("{}\u{e9}", "0".repeat(24)),
        ] {
            assert!(decode_crockford(s).is_err(), "{s:?}");
        }
    }

    proptest! {
        #[test]
        fn round_trip(bytes: [u8; 16]) {
            prop_assert_eq!(decode_crockford(&encode_crockford(&bytes)), Ok(bytes));
        }

        #[test]
        fn order_is_preserved(a: [u8; 16], b: [u8; 16]) {
            prop_assert_eq!(a.cmp(&b), encode_crockford(&a).cmp(&encode_crockford(&b)));
        }
    }
}
