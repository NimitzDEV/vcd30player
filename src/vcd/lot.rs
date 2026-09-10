//! VCD 2.0 List ID Offset Table (`LOT.VCD`) parser.
//!
//! `LOT.VCD` is a 64KB table containing 32,768 16-bit big-endian unsigned integers.
//! The table provides O(1) direct mapping from List ID (LID 1..=32767) to unit offsets in `PSD.VCD`.
//! Offset 0 is reserved (0x0000). Entries with value 0xFFFF indicate an unused or invalid LID.
//! The actual byte offset in `PSD.VCD` is `unit_offset * offset_multiplier` (typically 8 bytes).

use std::path::Path;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LotTable {
    offsets: Vec<u16>,
    offset_mult: usize,
}

impl LotTable {
    /// Parses a LOT table from raw bytes.
    ///
    /// `offset_mult` is specified in `INFO.VCD` (field at offset 0x0033), defaulting to 8 if 0.
    pub fn parse(data: &[u8], offset_mult: usize) -> Result<Self, String> {
        if data.len() < 4 {
            return Err("LOT data too small: minimum size is 4 bytes".to_string());
        }

        let mult = if offset_mult == 0 { 8 } else { offset_mult };
        let count = data.len() / 2;
        let mut offsets = Vec::with_capacity(count);

        for chunk in data.chunks_exact(2) {
            offsets.push(u16::from_be_bytes([chunk[0], chunk[1]]));
        }

        Ok(Self {
            offsets,
            offset_mult: mult,
        })
    }

    /// Loads and parses `LOT.VCD` from a given file path.
    pub fn from_file<P: AsRef<Path>>(path: P, offset_mult: usize) -> Result<Self, String> {
        let bytes = std::fs::read(path.as_ref())
            .map_err(|e| format!("Failed to read LOT file: {}", e))?;
        Self::parse(&bytes, offset_mult)
    }

    /// Returns the offset multiplier (typically 8).
    pub fn offset_multiplier(&self) -> usize {
        self.offset_mult
    }

    /// Returns the raw unit offset (in units of `offset_mult` bytes) for a given 1-based LID.
    /// Returns `None` if the LID is out of bounds or marked as 0xFFFF.
    pub fn get_unit_offset(&self, lid: u16) -> Option<u16> {
        let idx = lid as usize;
        if idx < self.offsets.len() {
            let val = self.offsets[idx];
            if val != 0xFFFF {
                return Some(val);
            }
        }
        None
    }

    /// Returns the byte offset in `PSD.VCD` for a given 1-based LID.
    pub fn get_byte_offset(&self, lid: u16) -> Option<usize> {
        self.get_unit_offset(lid).map(|u| (u as usize) * self.offset_mult)
    }

    /// Returns a list of all valid LIDs (1-based) defined in this LOT table.
    pub fn valid_lids(&self) -> Vec<u16> {
        let mut lids = Vec::new();
        for (idx, &val) in self.offsets.iter().enumerate().skip(1) {
            if val != 0xFFFF {
                lids.push(idx as u16);
            }
        }
        lids
    }

    /// Returns total number of parsed entries.
    pub fn entry_count(&self) -> usize {
        self.offsets.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lot_parse_synthetic() {
        let mut raw = vec![0u8; 16]; // 8 u16 entries
        // Index 0: Reserved 0x0000
        // Index 1 (LID 1): unit offset 0x0000
        raw[2] = 0x00;
        raw[3] = 0x00;
        // Index 2 (LID 2): unit offset 0x0002 -> byte offset 16
        raw[4] = 0x00;
        raw[5] = 0x02;
        // Index 3 (LID 3): unit offset 0x0009 -> byte offset 72
        raw[6] = 0x00;
        raw[7] = 0x09;
        // Index 4 (LID 4): 0xFFFF (invalid)
        raw[8] = 0xFF;
        raw[9] = 0xFF;

        let lot = LotTable::parse(&raw, 8).expect("failed to parse lot");
        assert_eq!(lot.entry_count(), 8);
        assert_eq!(lot.get_unit_offset(1), Some(0));
        assert_eq!(lot.get_byte_offset(1), Some(0));

        assert_eq!(lot.get_unit_offset(2), Some(2));
        assert_eq!(lot.get_byte_offset(2), Some(16));

        assert_eq!(lot.get_unit_offset(3), Some(9));
        assert_eq!(lot.get_byte_offset(3), Some(72));

        assert_eq!(lot.get_unit_offset(4), None);
        assert_eq!(lot.get_byte_offset(4), None);
        assert_eq!(lot.get_unit_offset(100), None);

        let valid = lot.valid_lids();
        assert_eq!(valid, vec![1, 2, 3, 5, 6, 7]);
    }

    #[test]
    fn test_lot_too_small() {
        let err = LotTable::parse(&[0, 0], 8).unwrap_err();
        assert!(err.contains("too small"));
    }
}
