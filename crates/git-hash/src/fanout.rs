use crate::{HashError, ObjectId};

/// Fan-out table mapping first byte to cumulative count.
///
/// Used in pack index files for fast object lookup. Each of the 256 entries
/// contains the cumulative number of objects whose first hash byte is ≤ the
/// entry index.
#[derive(Debug)]
pub struct FanoutTable {
    table: [u32; 256],
}

impl FanoutTable {
    /// Build a fan-out table from a sorted slice of OIDs.
    ///
    /// The OIDs **must** be sorted; this function does not verify order.
    pub fn build(oids: &[ObjectId]) -> Self {
        let mut table = [0u32; 256];
        for oid in oids {
            let bucket = oid.first_byte() as usize;
            table[bucket] += 1;
        }
        // Convert counts to cumulative counts.
        for i in 1..256 {
            table[i] += table[i - 1];
        }
        Self { table }
    }

    /// Get the index range for OIDs whose first byte equals `first_byte`.
    pub fn range(&self, first_byte: u8) -> std::ops::Range<usize> {
        let end = self.table[first_byte as usize] as usize;
        let start = if first_byte == 0 {
            0
        } else {
            self.table[(first_byte - 1) as usize] as usize
        };
        start..end
    }

    /// Total number of objects tracked by this fan-out table.
    pub fn total(&self) -> u32 {
        self.table[255]
    }

    /// Read from binary format (pack index): 256 big-endian u32 values.
    pub fn from_bytes(data: &[u8]) -> Result<Self, HashError> {
        if data.len() < 1024 {
            return Err(HashError::InvalidHashLength {
                expected: 1024,
                actual: data.len(),
            });
        }
        let mut table = [0u32; 256];
        for (i, entry) in table.iter_mut().enumerate() {
            let offset = i * 4;
            *entry = u32::from_be_bytes([
                data[offset],
                data[offset + 1],
                data[offset + 2],
                data[offset + 3],
            ]);
        }
        Self::validate(&table)?;
        Ok(Self { table })
    }

    /// Write to binary format: 256 big-endian u32 values (1024 bytes).
    pub fn to_bytes(&self) -> [u8; 1024] {
        let mut buf = [0u8; 1024];
        for i in 0..256 {
            let bytes = self.table[i].to_be_bytes();
            let offset = i * 4;
            buf[offset..offset + 4].copy_from_slice(&bytes);
        }
        buf
    }

    /// Get the raw table entry at the given index.
    pub fn get(&self, index: u8) -> u32 {
        self.table[index as usize]
    }

    fn validate(table: &[u32; 256]) -> Result<(), HashError> {
        // Cumulative counts must be non-decreasing.
        for i in 1..256 {
            if table[i] < table[i - 1] {
                return Err(HashError::InvalidHashLength {
                    expected: table[i - 1] as usize,
                    actual: table[i] as usize,
                });
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::HashAlgorithm;

    fn make_oid(first_byte: u8) -> ObjectId {
        let mut bytes = [0u8; 20];
        bytes[0] = first_byte;
        ObjectId::from_bytes(&bytes, HashAlgorithm::Sha1).unwrap()
    }

    #[test]
    fn build_and_lookup() {
        let mut oids: Vec<ObjectId> = vec![
            make_oid(0x00),
            make_oid(0x00),
            make_oid(0x01),
            make_oid(0x05),
            make_oid(0xff),
        ];
        oids.sort();

        let ft = FanoutTable::build(&oids);
        assert_eq!(ft.total(), 5);
        assert_eq!(ft.range(0x00), 0..2);
        assert_eq!(ft.range(0x01), 2..3);
        assert_eq!(ft.range(0x02), 3..3); // empty
        assert_eq!(ft.range(0x05), 3..4);
        assert_eq!(ft.range(0xff), 4..5);
    }

    #[test]
    fn bytes_roundtrip() {
        let oids: Vec<ObjectId> = (0..=255u8).map(make_oid).collect();
        let ft = FanoutTable::build(&oids);

        let bytes = ft.to_bytes();
        assert_eq!(bytes.len(), 1024);

        let ft2 = FanoutTable::from_bytes(&bytes).unwrap();
        assert_eq!(ft.table, ft2.table);
    }

    #[test]
    fn empty_table() {
        let ft = FanoutTable::build(&[]);
        assert_eq!(ft.total(), 0);
        for b in 0..=255u8 {
            assert!(ft.range(b).is_empty());
        }
    }

    #[test]
    fn from_bytes_too_short() {
        let err = FanoutTable::from_bytes(&[0u8; 100]).unwrap_err();
        assert!(matches!(err, HashError::InvalidHashLength { .. }));
    }
}
