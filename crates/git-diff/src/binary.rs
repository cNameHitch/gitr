//! Binary file detection.
//!
//! Checks for null bytes in the first 8KB of a file, matching
//! C git's heuristic for binary detection.

/// Check if data appears to be binary by looking for null bytes.
///
/// Matches C git's buffer_is_binary(): checks the first 8KB for NUL bytes.
pub fn is_binary(data: &[u8]) -> bool {
    let check_len = data.len().min(8192);
    data[..check_len].contains(&0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_is_not_binary() {
        assert!(!is_binary(b"hello world\nthis is text\n"));
    }

    #[test]
    fn null_byte_is_binary() {
        assert!(is_binary(b"hello\x00world"));
    }

    #[test]
    fn empty_is_not_binary() {
        assert!(!is_binary(b""));
    }

    #[test]
    fn null_at_8k_boundary() {
        let mut data = vec![b'a'; 8192];
        assert!(!is_binary(&data));
        data[8191] = 0;
        assert!(is_binary(&data));
    }

    #[test]
    fn null_beyond_8k_not_detected() {
        let mut data = vec![b'a'; 10000];
        data[9000] = 0;
        assert!(!is_binary(&data));
    }
}
