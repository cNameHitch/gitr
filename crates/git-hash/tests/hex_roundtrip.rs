use git_hash::hex::{hex_decode, hex_encode, hex_to_bytes, hex_to_string, is_valid_hex};
use git_hash::{HashAlgorithm, ObjectId};
use proptest::prelude::*;

proptest! {
    #[test]
    fn hex_encode_decode_roundtrip(bytes in proptest::collection::vec(any::<u8>(), 0..128)) {
        let hex = hex_to_string(&bytes);
        let decoded = hex_to_bytes(&hex).unwrap();
        prop_assert_eq!(&decoded, &bytes);
    }

    #[test]
    fn hex_is_always_lowercase(bytes in proptest::collection::vec(any::<u8>(), 1..64)) {
        let hex = hex_to_string(&bytes);
        prop_assert!(hex.chars().all(|c| c.is_ascii_digit() || ('a'..='f').contains(&c)));
    }

    #[test]
    fn hex_length_is_double(bytes in proptest::collection::vec(any::<u8>(), 0..128)) {
        let hex = hex_to_string(&bytes);
        prop_assert_eq!(hex.len(), bytes.len() * 2);
    }

    #[test]
    fn hex_encode_buffer_roundtrip(bytes in proptest::collection::vec(any::<u8>(), 0..128)) {
        let mut buf = vec![0u8; bytes.len() * 2];
        hex_encode(&bytes, &mut buf);
        let hex = std::str::from_utf8(&buf).unwrap();
        let mut decoded = vec![0u8; bytes.len()];
        hex_decode(hex, &mut decoded).unwrap();
        prop_assert_eq!(&decoded, &bytes);
    }

    #[test]
    fn valid_hex_accepted(bytes in proptest::collection::vec(any::<u8>(), 0..64)) {
        let hex = hex_to_string(&bytes);
        prop_assert!(is_valid_hex(&hex));
    }

    #[test]
    fn sha1_oid_hex_roundtrip(bytes in proptest::collection::vec(any::<u8>(), 20..=20)) {
        let oid = ObjectId::from_bytes(&bytes, HashAlgorithm::Sha1).unwrap();
        let hex = oid.to_hex();
        let parsed: ObjectId = hex.parse().unwrap();
        prop_assert_eq!(oid, parsed);
    }

    #[test]
    fn sha256_oid_hex_roundtrip(bytes in proptest::collection::vec(any::<u8>(), 32..=32)) {
        let oid = ObjectId::from_bytes(&bytes, HashAlgorithm::Sha256).unwrap();
        let hex = oid.to_hex();
        let parsed: ObjectId = hex.parse().unwrap();
        prop_assert_eq!(oid, parsed);
    }
}
