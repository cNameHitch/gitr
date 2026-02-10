//! EOL normalization for git add and git checkout.
//!
//! Implements CRLF/LF line-ending conversion based on gitattributes:
//! - On `add` (clean path): CRLF -> LF (normalize for storage)
//! - On `checkout` (smudge path): LF -> CRLF (denormalize for working tree)
//!
//! Respects the `text`, `eol`, and `binary` attributes, as well as `text=auto`
//! which uses binary detection heuristics.

use bstr::BStr;

use crate::attributes::{AttributeStack, AttributeValue, Eol};

/// The maximum number of bytes to inspect for binary detection (`text=auto`).
/// Matches C git's `FIRST_FEW_BYTES` constant.
const BINARY_CHECK_LEN: usize = 8000;

/// Binary detection: check if the first 8000 bytes contain a NUL byte.
///
/// This matches C git's `buffer_is_binary()` heuristic. Files detected as
/// binary are never subject to EOL normalization.
pub fn is_binary_content(data: &[u8]) -> bool {
    let check_len = data.len().min(BINARY_CHECK_LEN);
    data[..check_len].contains(&0)
}

/// Determine the EOL conversion action for a given path and direction.
///
/// Returns the appropriate conversion to apply, or `EolConversion::None`
/// if no conversion is needed.
pub fn eol_conversion_for_path(
    attrs: &AttributeStack,
    path: &BStr,
    content: &[u8],
    direction: ConversionDirection,
) -> EolConversion {
    // Check if explicitly marked binary — never convert
    if attrs.is_binary(path) {
        return EolConversion::None;
    }

    let text_attr = attrs.get(path, BStr::new(b"text"));

    match text_attr {
        AttributeValue::Unset => {
            // -text: explicitly disabled, no conversion
            EolConversion::None
        }
        AttributeValue::Set => {
            // text: always normalize
            eol_for_direction(attrs, path, direction)
        }
        AttributeValue::Value(ref val) if val.as_slice() == b"auto" => {
            // text=auto: normalize only if content is not binary
            if is_binary_content(content) {
                EolConversion::None
            } else {
                eol_for_direction(attrs, path, direction)
            }
        }
        AttributeValue::Value(_) => {
            // text=<other>: treat as "set" (normalize)
            eol_for_direction(attrs, path, direction)
        }
        AttributeValue::Unspecified => {
            // No text attribute set — no conversion
            EolConversion::None
        }
    }
}

/// Direction of content flow.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConversionDirection {
    /// Content flowing from working tree to object database (add/clean).
    ToGit,
    /// Content flowing from object database to working tree (checkout/smudge).
    ToWorkTree,
}

/// The type of EOL conversion to perform.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EolConversion {
    /// No conversion needed.
    None,
    /// Convert CRLF to LF (normalize for storage).
    CrLfToLf,
    /// Convert LF to CRLF (denormalize for working tree).
    LfToCrLf,
}

/// Determine the EOL conversion based on the `eol` attribute and direction.
fn eol_for_direction(
    attrs: &AttributeStack,
    path: &BStr,
    direction: ConversionDirection,
) -> EolConversion {
    let eol = attrs.eol_for(path);

    match direction {
        ConversionDirection::ToGit => {
            // On add: always normalize CRLF -> LF regardless of eol setting
            EolConversion::CrLfToLf
        }
        ConversionDirection::ToWorkTree => {
            match eol {
                Some(Eol::CrLf) => EolConversion::LfToCrLf,
                Some(Eol::Lf) => EolConversion::None, // Already LF in repo
                None => {
                    // Default: keep as LF (Unix default)
                    // On Windows, this would be CRLF, but we default to LF
                    EolConversion::None
                }
            }
        }
    }
}

/// Apply EOL conversion to content.
///
/// Returns the converted content, or `None` if no conversion was needed
/// (the original data can be used as-is).
pub fn apply_eol_conversion(data: &[u8], conversion: EolConversion) -> Option<Vec<u8>> {
    match conversion {
        EolConversion::None => None,
        EolConversion::CrLfToLf => crlf_to_lf(data),
        EolConversion::LfToCrLf => lf_to_crlf(data),
    }
}

/// Convert CRLF line endings to LF.
///
/// Returns `None` if the data contains no CRLF sequences (no conversion needed).
fn crlf_to_lf(data: &[u8]) -> Option<Vec<u8>> {
    // Quick scan: if no CRLF present, return None
    let has_crlf = data.windows(2).any(|w| w == b"\r\n");
    if !has_crlf {
        return None;
    }

    let mut result = Vec::with_capacity(data.len());
    let mut i = 0;
    while i < data.len() {
        if i + 1 < data.len() && data[i] == b'\r' && data[i + 1] == b'\n' {
            result.push(b'\n');
            i += 2;
        } else {
            result.push(data[i]);
            i += 1;
        }
    }
    Some(result)
}

/// Convert LF line endings to CRLF.
///
/// Returns `None` if all LF bytes are already preceded by CR (no conversion needed).
fn lf_to_crlf(data: &[u8]) -> Option<Vec<u8>> {
    // Quick scan: check if there are any bare LF (not preceded by CR)
    let has_bare_lf = data.iter().enumerate().any(|(i, &b)| {
        b == b'\n' && (i == 0 || data[i - 1] != b'\r')
    });
    if !has_bare_lf {
        return None;
    }

    let mut result = Vec::with_capacity(data.len() + data.len() / 10);
    for (i, &b) in data.iter().enumerate() {
        if b == b'\n' && (i == 0 || data[i - 1] != b'\r') {
            result.push(b'\r');
        }
        result.push(b);
    }
    Some(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn binary_detection_nul_byte() {
        assert!(is_binary_content(b"hello\x00world"));
        assert!(!is_binary_content(b"hello world"));
        assert!(!is_binary_content(b""));
    }

    #[test]
    fn binary_detection_beyond_8000() {
        let mut data = vec![b'a'; 10000];
        data[9000] = 0;
        assert!(!is_binary_content(&data));
    }

    #[test]
    fn binary_detection_within_8000() {
        let mut data = vec![b'a'; 10000];
        data[7999] = 0;
        assert!(is_binary_content(&data));
    }

    #[test]
    fn crlf_to_lf_conversion() {
        assert_eq!(
            crlf_to_lf(b"hello\r\nworld\r\n"),
            Some(b"hello\nworld\n".to_vec())
        );
    }

    #[test]
    fn crlf_to_lf_no_crlf() {
        assert_eq!(crlf_to_lf(b"hello\nworld\n"), None);
    }

    #[test]
    fn crlf_to_lf_mixed() {
        assert_eq!(
            crlf_to_lf(b"hello\r\nworld\n"),
            Some(b"hello\nworld\n".to_vec())
        );
    }

    #[test]
    fn lf_to_crlf_conversion() {
        assert_eq!(
            lf_to_crlf(b"hello\nworld\n"),
            Some(b"hello\r\nworld\r\n".to_vec())
        );
    }

    #[test]
    fn lf_to_crlf_already_crlf() {
        assert_eq!(lf_to_crlf(b"hello\r\nworld\r\n"), None);
    }

    #[test]
    fn lf_to_crlf_mixed() {
        assert_eq!(
            lf_to_crlf(b"hello\r\nworld\n"),
            Some(b"hello\r\nworld\r\n".to_vec())
        );
    }

    #[test]
    fn text_set_converts_on_add() {
        let mut attrs = AttributeStack::new();
        attrs.add_patterns(b"*.txt text\n", Path::new(".gitattributes"));
        let path = BStr::new(b"readme.txt");
        let content = b"hello\r\nworld\r\n";

        let conv = eol_conversion_for_path(&attrs, path, content, ConversionDirection::ToGit);
        assert_eq!(conv, EolConversion::CrLfToLf);
    }

    #[test]
    fn text_unset_no_conversion() {
        let mut attrs = AttributeStack::new();
        attrs.add_patterns(b"*.bin -text\n", Path::new(".gitattributes"));
        let path = BStr::new(b"data.bin");
        let content = b"hello\r\nworld\r\n";

        let conv = eol_conversion_for_path(&attrs, path, content, ConversionDirection::ToGit);
        assert_eq!(conv, EolConversion::None);
    }

    #[test]
    fn text_auto_binary_no_conversion() {
        let mut attrs = AttributeStack::new();
        attrs.add_patterns(b"* text=auto\n", Path::new(".gitattributes"));
        let path = BStr::new(b"image.png");
        let content = b"hello\x00world";

        let conv = eol_conversion_for_path(&attrs, path, content, ConversionDirection::ToGit);
        assert_eq!(conv, EolConversion::None);
    }

    #[test]
    fn text_auto_text_converts() {
        let mut attrs = AttributeStack::new();
        attrs.add_patterns(b"* text=auto\n", Path::new(".gitattributes"));
        let path = BStr::new(b"readme.txt");
        let content = b"hello\r\nworld\r\n";

        let conv = eol_conversion_for_path(&attrs, path, content, ConversionDirection::ToGit);
        assert_eq!(conv, EolConversion::CrLfToLf);
    }

    #[test]
    fn eol_crlf_on_checkout() {
        let mut attrs = AttributeStack::new();
        attrs.add_patterns(b"*.txt text eol=crlf\n", Path::new(".gitattributes"));
        let path = BStr::new(b"readme.txt");
        let content = b"hello\nworld\n";

        let conv =
            eol_conversion_for_path(&attrs, path, content, ConversionDirection::ToWorkTree);
        assert_eq!(conv, EolConversion::LfToCrLf);
    }

    #[test]
    fn eol_lf_on_checkout_no_conversion() {
        let mut attrs = AttributeStack::new();
        attrs.add_patterns(b"*.txt text eol=lf\n", Path::new(".gitattributes"));
        let path = BStr::new(b"readme.txt");
        let content = b"hello\nworld\n";

        let conv =
            eol_conversion_for_path(&attrs, path, content, ConversionDirection::ToWorkTree);
        assert_eq!(conv, EolConversion::None);
    }

    #[test]
    fn unspecified_no_conversion() {
        let attrs = AttributeStack::new();
        let path = BStr::new(b"readme.txt");
        let content = b"hello\r\nworld\r\n";

        let conv = eol_conversion_for_path(&attrs, path, content, ConversionDirection::ToGit);
        assert_eq!(conv, EolConversion::None);
    }

    #[test]
    fn binary_attribute_no_conversion() {
        let mut attrs = AttributeStack::new();
        attrs.add_patterns(b"*.png binary\n", Path::new(".gitattributes"));
        let path = BStr::new(b"logo.png");
        let content = b"hello\r\nworld\r\n";

        let conv = eol_conversion_for_path(&attrs, path, content, ConversionDirection::ToGit);
        assert_eq!(conv, EolConversion::None);
    }

    #[test]
    fn apply_eol_none_returns_none() {
        assert!(apply_eol_conversion(b"data", EolConversion::None).is_none());
    }

    #[test]
    fn apply_eol_crlf_to_lf() {
        let result = apply_eol_conversion(b"a\r\nb\r\n", EolConversion::CrLfToLf);
        assert_eq!(result, Some(b"a\nb\n".to_vec()));
    }

    #[test]
    fn apply_eol_lf_to_crlf() {
        let result = apply_eol_conversion(b"a\nb\n", EolConversion::LfToCrLf);
        assert_eq!(result, Some(b"a\r\nb\r\n".to_vec()));
    }
}
