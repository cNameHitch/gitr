use crate::HashError;

/// Lookup table: ASCII byte → nibble value (255 = invalid).
const HEX_DECODE: [u8; 256] = {
    let mut table = [255u8; 256];
    let mut i = 0u8;
    loop {
        match i {
            b'0'..=b'9' => table[i as usize] = i - b'0',
            b'a'..=b'f' => table[i as usize] = i - b'a' + 10,
            b'A'..=b'F' => table[i as usize] = i - b'A' + 10,
            _ => {}
        }
        if i == 255 {
            break;
        }
        i += 1;
    }
    table
};

const HEX_ENCODE: &[u8; 16] = b"0123456789abcdef";

/// Hex-encode `bytes` into `buf`. `buf` must be at least `bytes.len() * 2` bytes.
///
/// # Panics
///
/// Panics if `buf` is too short.
pub fn hex_encode(bytes: &[u8], buf: &mut [u8]) {
    assert!(
        buf.len() >= bytes.len() * 2,
        "hex_encode: buffer too short"
    );
    for (i, &b) in bytes.iter().enumerate() {
        buf[i * 2] = HEX_ENCODE[(b >> 4) as usize];
        buf[i * 2 + 1] = HEX_ENCODE[(b & 0x0f) as usize];
    }
}

/// Hex-encode `bytes` to a new `String`.
pub fn hex_to_string(bytes: &[u8]) -> String {
    let mut buf = vec![0u8; bytes.len() * 2];
    hex_encode(bytes, &mut buf);
    // SAFETY: hex_encode only writes ASCII hex digits.
    unsafe { String::from_utf8_unchecked(buf) }
}

/// Decode a hex string into `buf`. The hex string length must be exactly `buf.len() * 2`.
pub fn hex_decode(hex: &str, buf: &mut [u8]) -> Result<(), HashError> {
    let hex = hex.as_bytes();
    if hex.len() != buf.len() * 2 {
        return Err(HashError::InvalidHexLength {
            expected: buf.len() * 2,
            actual: hex.len(),
        });
    }
    for i in 0..buf.len() {
        let hi = HEX_DECODE[hex[i * 2] as usize];
        let lo = HEX_DECODE[hex[i * 2 + 1] as usize];
        if hi == 255 {
            return Err(HashError::InvalidHex {
                position: i * 2,
                character: hex[i * 2] as char,
            });
        }
        if lo == 255 {
            return Err(HashError::InvalidHex {
                position: i * 2 + 1,
                character: hex[i * 2 + 1] as char,
            });
        }
        buf[i] = (hi << 4) | lo;
    }
    Ok(())
}

/// Decode a hex string to a new `Vec<u8>`.
pub fn hex_to_bytes(hex: &str) -> Result<Vec<u8>, HashError> {
    if hex.len() % 2 != 0 {
        return Err(HashError::InvalidHexLength {
            expected: hex.len() + 1, // nearest even
            actual: hex.len(),
        });
    }
    let mut buf = vec![0u8; hex.len() / 2];
    hex_decode(hex, &mut buf)?;
    Ok(buf)
}

/// Check if a string is valid hexadecimal (even length, all hex chars).
pub fn is_valid_hex(s: &str) -> bool {
    s.len() % 2 == 0 && s.bytes().all(|b| HEX_DECODE[b as usize] != 255)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encode_decode_roundtrip() {
        let bytes = [0xde, 0xad, 0xbe, 0xef, 0x00, 0xff];
        let hex = hex_to_string(&bytes);
        assert_eq!(hex, "deadbeef00ff");
        let decoded = hex_to_bytes(&hex).unwrap();
        assert_eq!(decoded, bytes);
    }

    #[test]
    fn decode_uppercase() {
        let decoded = hex_to_bytes("DEADBEEF").unwrap();
        assert_eq!(decoded, [0xde, 0xad, 0xbe, 0xef]);
    }

    #[test]
    fn decode_mixed_case() {
        let decoded = hex_to_bytes("DeAdBeEf").unwrap();
        assert_eq!(decoded, [0xde, 0xad, 0xbe, 0xef]);
    }

    #[test]
    fn decode_invalid_char() {
        let err = hex_to_bytes("deadgoof").unwrap_err();
        match err {
            HashError::InvalidHex {
                position: 4,
                character: 'g',
            } => {}
            other => panic!("unexpected error: {other}"),
        }
    }

    #[test]
    fn decode_odd_length() {
        let err = hex_to_bytes("abc").unwrap_err();
        assert!(matches!(err, HashError::InvalidHexLength { .. }));
    }

    #[test]
    fn is_valid_hex_checks() {
        assert!(is_valid_hex("deadbeef"));
        assert!(is_valid_hex("DEADBEEF"));
        assert!(is_valid_hex("0123456789abcdef"));
        assert!(!is_valid_hex("xyz"));
        assert!(!is_valid_hex("abc")); // odd length
        assert!(is_valid_hex("")); // empty is valid
    }

    #[test]
    fn encode_to_buffer() {
        let bytes = [0x01, 0x23, 0x45];
        let mut buf = [0u8; 6];
        hex_encode(&bytes, &mut buf);
        assert_eq!(&buf, b"012345");
    }

    #[test]
    fn all_byte_values_roundtrip() {
        let bytes: Vec<u8> = (0..=255).collect();
        let hex = hex_to_string(&bytes);
        let decoded = hex_to_bytes(&hex).unwrap();
        assert_eq!(decoded, bytes);
    }
}
