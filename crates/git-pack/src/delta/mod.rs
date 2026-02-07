//! Delta encoding and decoding.
//!
//! Git packfiles use delta compression to store similar objects compactly.
//! A delta instruction stream describes how to reconstruct a target object
//! from a base (source) object using copy and insert operations.
//!
//! Delta format:
//! ```text
//! [source_size: varint] [target_size: varint]
//! [instruction]*
//! ```
//!
//! Instructions:
//! - Copy:   `[1SSSOOOO] [offset_bytes] [size_bytes]`
//! - Insert: `[0NNNNNNN] [N literal bytes]`

pub mod apply;
pub mod compute;

/// A single delta instruction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DeltaInstruction {
    /// Copy bytes from the base (source) object.
    Copy { offset: u64, size: usize },
    /// Insert literal bytes into the output.
    Insert(Vec<u8>),
}

/// Read a variable-length size from delta header bytes.
///
/// Returns `(value, bytes_consumed)`.
pub fn read_varint(data: &[u8]) -> Option<(usize, usize)> {
    let mut value: usize = 0;
    let mut shift = 0;
    let mut pos = 0;

    loop {
        if pos >= data.len() {
            return None;
        }
        let byte = data[pos];
        pos += 1;
        value |= ((byte & 0x7f) as usize) << shift;
        shift += 7;
        if byte & 0x80 == 0 {
            break;
        }
    }
    Some((value, pos))
}

/// Parse a delta instruction stream into structured instructions.
///
/// Returns `(source_size, target_size, instructions)`.
pub fn parse_delta_instructions(
    delta: &[u8],
) -> Result<(usize, usize, Vec<DeltaInstruction>), crate::PackError> {
    let mut pos = 0;

    // Read source size
    let (source_size, consumed) = read_varint(&delta[pos..]).ok_or_else(|| {
        crate::PackError::InvalidDelta {
            offset: 0,
            reason: "truncated source size".into(),
        }
    })?;
    pos += consumed;

    // Read target size
    let (target_size, consumed) = read_varint(&delta[pos..]).ok_or_else(|| {
        crate::PackError::InvalidDelta {
            offset: pos as u64,
            reason: "truncated target size".into(),
        }
    })?;
    pos += consumed;

    let mut instructions = Vec::new();

    while pos < delta.len() {
        let cmd = delta[pos];
        pos += 1;

        if cmd & 0x80 != 0 {
            // Copy instruction
            let mut offset: u64 = 0;
            let mut size: usize = 0;

            if cmd & 0x01 != 0 {
                if pos >= delta.len() {
                    return Err(crate::PackError::InvalidDelta {
                        offset: pos as u64,
                        reason: "truncated copy offset".into(),
                    });
                }
                offset |= delta[pos] as u64;
                pos += 1;
            }
            if cmd & 0x02 != 0 {
                if pos >= delta.len() {
                    return Err(crate::PackError::InvalidDelta {
                        offset: pos as u64,
                        reason: "truncated copy offset".into(),
                    });
                }
                offset |= (delta[pos] as u64) << 8;
                pos += 1;
            }
            if cmd & 0x04 != 0 {
                if pos >= delta.len() {
                    return Err(crate::PackError::InvalidDelta {
                        offset: pos as u64,
                        reason: "truncated copy offset".into(),
                    });
                }
                offset |= (delta[pos] as u64) << 16;
                pos += 1;
            }
            if cmd & 0x08 != 0 {
                if pos >= delta.len() {
                    return Err(crate::PackError::InvalidDelta {
                        offset: pos as u64,
                        reason: "truncated copy offset".into(),
                    });
                }
                offset |= (delta[pos] as u64) << 24;
                pos += 1;
            }

            if cmd & 0x10 != 0 {
                if pos >= delta.len() {
                    return Err(crate::PackError::InvalidDelta {
                        offset: pos as u64,
                        reason: "truncated copy size".into(),
                    });
                }
                size |= delta[pos] as usize;
                pos += 1;
            }
            if cmd & 0x20 != 0 {
                if pos >= delta.len() {
                    return Err(crate::PackError::InvalidDelta {
                        offset: pos as u64,
                        reason: "truncated copy size".into(),
                    });
                }
                size |= (delta[pos] as usize) << 8;
                pos += 1;
            }
            if cmd & 0x40 != 0 {
                if pos >= delta.len() {
                    return Err(crate::PackError::InvalidDelta {
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

            instructions.push(DeltaInstruction::Copy { offset, size });
        } else if cmd != 0 {
            // Insert instruction
            let n = cmd as usize;
            if pos + n > delta.len() {
                return Err(crate::PackError::InvalidDelta {
                    offset: pos as u64,
                    reason: "truncated insert data".into(),
                });
            }
            instructions.push(DeltaInstruction::Insert(delta[pos..pos + n].to_vec()));
            pos += n;
        } else {
            // cmd == 0 is reserved/invalid
            return Err(crate::PackError::InvalidDelta {
                offset: (pos - 1) as u64,
                reason: "unexpected delta opcode 0".into(),
            });
        }
    }

    Ok((source_size, target_size, instructions))
}

/// Encode a variable-length integer for delta headers.
pub fn write_varint(mut value: usize) -> Vec<u8> {
    let mut buf = Vec::with_capacity(5);
    loop {
        let mut byte = (value & 0x7f) as u8;
        value >>= 7;
        if value > 0 {
            byte |= 0x80;
        }
        buf.push(byte);
        if value == 0 {
            break;
        }
    }
    buf
}

/// Encode a copy instruction.
pub fn encode_copy(offset: u64, size: usize) -> Vec<u8> {
    let mut buf = Vec::with_capacity(8);
    let mut cmd: u8 = 0x80;
    let mut extra = Vec::new();

    let off = offset as u32;
    if off & 0x0000_00ff != 0 {
        cmd |= 0x01;
        extra.push((off & 0xff) as u8);
    }
    if off & 0x0000_ff00 != 0 {
        cmd |= 0x02;
        extra.push(((off >> 8) & 0xff) as u8);
    }
    if off & 0x00ff_0000 != 0 {
        cmd |= 0x04;
        extra.push(((off >> 16) & 0xff) as u8);
    }
    if off & 0xff00_0000 != 0 {
        cmd |= 0x08;
        extra.push(((off >> 24) & 0xff) as u8);
    }

    let sz = if size == 0x10000 { 0usize } else { size };
    if sz & 0x0000_00ff != 0 {
        cmd |= 0x10;
        extra.push((sz & 0xff) as u8);
    }
    if sz & 0x0000_ff00 != 0 {
        cmd |= 0x20;
        extra.push(((sz >> 8) & 0xff) as u8);
    }
    if sz & 0xff_0000 != 0 {
        cmd |= 0x40;
        extra.push(((sz >> 16) & 0xff) as u8);
    }

    buf.push(cmd);
    buf.extend_from_slice(&extra);
    buf
}

/// Encode an insert instruction. Data must be 1-127 bytes.
pub fn encode_insert(data: &[u8]) -> Vec<u8> {
    assert!(!data.is_empty() && data.len() <= 127);
    let mut buf = Vec::with_capacity(1 + data.len());
    buf.push(data.len() as u8);
    buf.extend_from_slice(data);
    buf
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn varint_roundtrip() {
        for value in [0, 1, 127, 128, 255, 256, 16383, 16384, 1_000_000] {
            let encoded = write_varint(value);
            let (decoded, consumed) = read_varint(&encoded).unwrap();
            assert_eq!(decoded, value);
            assert_eq!(consumed, encoded.len());
        }
    }

    #[test]
    fn parse_simple_delta() {
        let mut delta = Vec::new();
        // Source size: 100
        delta.extend_from_slice(&write_varint(100));
        // Target size: 50
        delta.extend_from_slice(&write_varint(50));
        // Copy 10 bytes from offset 5
        delta.extend_from_slice(&encode_copy(5, 10));
        // Insert 3 bytes
        delta.extend_from_slice(&encode_insert(&[0xAA, 0xBB, 0xCC]));

        let (src_size, tgt_size, instructions) = parse_delta_instructions(&delta).unwrap();
        assert_eq!(src_size, 100);
        assert_eq!(tgt_size, 50);
        assert_eq!(instructions.len(), 2);
        assert_eq!(
            instructions[0],
            DeltaInstruction::Copy {
                offset: 5,
                size: 10,
            }
        );
        assert_eq!(
            instructions[1],
            DeltaInstruction::Insert(vec![0xAA, 0xBB, 0xCC])
        );
    }

    #[test]
    fn copy_with_zero_size_means_64k() {
        let mut delta = Vec::new();
        delta.extend_from_slice(&write_varint(0x20000));
        delta.extend_from_slice(&write_varint(0x10000));
        // Copy with all size bits zero → 0x10000
        delta.push(0x80 | 0x01); // cmd: copy, offset byte 0 present
        delta.push(0x00); // offset = 0

        let (_, _, instructions) = parse_delta_instructions(&delta).unwrap();
        assert_eq!(
            instructions[0],
            DeltaInstruction::Copy {
                offset: 0,
                size: 0x10000,
            }
        );
    }

    #[test]
    fn opcode_zero_is_error() {
        let mut delta = Vec::new();
        delta.extend_from_slice(&write_varint(10));
        delta.extend_from_slice(&write_varint(10));
        delta.push(0x00); // invalid opcode

        let result = parse_delta_instructions(&delta);
        assert!(result.is_err());
    }
}
