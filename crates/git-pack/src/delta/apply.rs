//! Apply delta instructions to reconstruct objects.

use crate::PackError;
use super::read_varint;

/// Apply a delta instruction stream to a base object, producing the target.
///
/// The delta format is:
/// ```text
/// [source_size: varint] [target_size: varint] [instructions...]
/// ```
///
/// This function validates sizes and performs bounds checking on all
/// copy operations to prevent out-of-bounds reads.
pub fn apply_delta(base: &[u8], delta: &[u8]) -> Result<Vec<u8>, PackError> {
    let mut pos = 0;

    // Read source size
    let (source_size, consumed) = read_varint(&delta[pos..]).ok_or_else(|| {
        PackError::InvalidDelta {
            offset: 0,
            reason: "truncated source size".into(),
        }
    })?;
    pos += consumed;

    // Read target size
    let (target_size, consumed) = read_varint(&delta[pos..]).ok_or_else(|| {
        PackError::InvalidDelta {
            offset: pos as u64,
            reason: "truncated target size".into(),
        }
    })?;
    pos += consumed;

    // Validate source size
    if source_size != base.len() {
        return Err(PackError::InvalidDelta {
            offset: 0,
            reason: format!(
                "source size mismatch: delta says {source_size}, base is {}",
                base.len()
            ),
        });
    }

    let mut output = Vec::with_capacity(target_size);

    while pos < delta.len() {
        let cmd = delta[pos];
        pos += 1;

        if cmd & 0x80 != 0 {
            // Copy instruction
            let mut offset: usize = 0;
            let mut size: usize = 0;

            if cmd & 0x01 != 0 {
                if pos >= delta.len() {
                    return Err(PackError::InvalidDelta {
                        offset: pos as u64,
                        reason: "truncated copy offset".into(),
                    });
                }
                offset |= delta[pos] as usize;
                pos += 1;
            }
            if cmd & 0x02 != 0 {
                if pos >= delta.len() {
                    return Err(PackError::InvalidDelta {
                        offset: pos as u64,
                        reason: "truncated copy offset".into(),
                    });
                }
                offset |= (delta[pos] as usize) << 8;
                pos += 1;
            }
            if cmd & 0x04 != 0 {
                if pos >= delta.len() {
                    return Err(PackError::InvalidDelta {
                        offset: pos as u64,
                        reason: "truncated copy offset".into(),
                    });
                }
                offset |= (delta[pos] as usize) << 16;
                pos += 1;
            }
            if cmd & 0x08 != 0 {
                if pos >= delta.len() {
                    return Err(PackError::InvalidDelta {
                        offset: pos as u64,
                        reason: "truncated copy offset".into(),
                    });
                }
                offset |= (delta[pos] as usize) << 24;
                pos += 1;
            }

            if cmd & 0x10 != 0 {
                if pos >= delta.len() {
                    return Err(PackError::InvalidDelta {
                        offset: pos as u64,
                        reason: "truncated copy size".into(),
                    });
                }
                size |= delta[pos] as usize;
                pos += 1;
            }
            if cmd & 0x20 != 0 {
                if pos >= delta.len() {
                    return Err(PackError::InvalidDelta {
                        offset: pos as u64,
                        reason: "truncated copy size".into(),
                    });
                }
                size |= (delta[pos] as usize) << 8;
                pos += 1;
            }
            if cmd & 0x40 != 0 {
                if pos >= delta.len() {
                    return Err(PackError::InvalidDelta {
                        offset: pos as u64,
                        reason: "truncated copy size".into(),
                    });
                }
                size |= (delta[pos] as usize) << 16;
                pos += 1;
            }

            // Size of 0 means 0x10000 (65536)
            if size == 0 {
                size = 0x10000;
            }

            // Bounds check
            if offset + size > base.len() {
                return Err(PackError::InvalidDelta {
                    offset: pos as u64,
                    reason: format!(
                        "copy out of bounds: offset={offset}, size={size}, base_len={}",
                        base.len()
                    ),
                });
            }

            output.extend_from_slice(&base[offset..offset + size]);
        } else if cmd != 0 {
            // Insert instruction
            let n = cmd as usize;
            if pos + n > delta.len() {
                return Err(PackError::InvalidDelta {
                    offset: pos as u64,
                    reason: "truncated insert data".into(),
                });
            }
            output.extend_from_slice(&delta[pos..pos + n]);
            pos += n;
        } else {
            return Err(PackError::InvalidDelta {
                offset: (pos - 1) as u64,
                reason: "unexpected delta opcode 0".into(),
            });
        }
    }

    // Validate target size
    if output.len() != target_size {
        return Err(PackError::InvalidDelta {
            offset: 0,
            reason: format!(
                "target size mismatch: delta says {target_size}, got {}",
                output.len()
            ),
        });
    }

    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::delta::{encode_copy, encode_insert, write_varint};

    fn build_delta(source_size: usize, target_size: usize, instructions: &[u8]) -> Vec<u8> {
        let mut delta = Vec::new();
        delta.extend_from_slice(&write_varint(source_size));
        delta.extend_from_slice(&write_varint(target_size));
        delta.extend_from_slice(instructions);
        delta
    }

    #[test]
    fn apply_copy_only() {
        let base = b"Hello, World!";
        let mut instructions = Vec::new();
        // Copy "Hello" (offset=0, size=5)
        instructions.extend_from_slice(&encode_copy(0, 5));
        // Copy "World" (offset=7, size=5)
        instructions.extend_from_slice(&encode_copy(7, 5));

        let delta = build_delta(base.len(), 10, &instructions);
        let result = apply_delta(base, &delta).unwrap();
        assert_eq!(result, b"HelloWorld");
    }

    #[test]
    fn apply_insert_only() {
        let base = b"unused base";
        let mut instructions = Vec::new();
        instructions.extend_from_slice(&encode_insert(b"NEW"));

        let delta = build_delta(base.len(), 3, &instructions);
        let result = apply_delta(base, &delta).unwrap();
        assert_eq!(result, b"NEW");
    }

    #[test]
    fn apply_mixed_instructions() {
        let base = b"ABCDEFGHIJ";
        let mut instructions = Vec::new();
        // Copy "ABC"
        instructions.extend_from_slice(&encode_copy(0, 3));
        // Insert "xyz"
        instructions.extend_from_slice(&encode_insert(b"xyz"));
        // Copy "HIJ"
        instructions.extend_from_slice(&encode_copy(7, 3));

        let delta = build_delta(base.len(), 9, &instructions);
        let result = apply_delta(base, &delta).unwrap();
        assert_eq!(result, b"ABCxyzHIJ");
    }

    #[test]
    fn copy_out_of_bounds_fails() {
        let base = b"short";
        let mut instructions = Vec::new();
        instructions.extend_from_slice(&encode_copy(0, 100)); // too large

        let delta = build_delta(base.len(), 100, &instructions);
        let result = apply_delta(base, &delta);
        assert!(result.is_err());
    }

    #[test]
    fn target_size_mismatch_fails() {
        let base = b"Hello";
        let mut instructions = Vec::new();
        instructions.extend_from_slice(&encode_copy(0, 5));

        // Claim target is 10, but we only produce 5
        let delta = build_delta(base.len(), 10, &instructions);
        let result = apply_delta(base, &delta);
        assert!(result.is_err());
    }

    #[test]
    fn source_size_mismatch_fails() {
        let base = b"Hello";
        let mut instructions = Vec::new();
        instructions.extend_from_slice(&encode_copy(0, 5));

        // Claim source is 100, but base is 5
        let delta = build_delta(100, 5, &instructions);
        let result = apply_delta(base, &delta);
        assert!(result.is_err());
    }

    #[test]
    fn empty_delta_produces_empty_output() {
        let base = b"anything";
        let delta = build_delta(base.len(), 0, &[]);
        let result = apply_delta(base, &delta).unwrap();
        assert!(result.is_empty());
    }
}
