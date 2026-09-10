//! Parser for standard Video CD `ENTRIES.VCD` chapter/entry point files.

use std::path::Path;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VcdEntry {
    /// 1-based index of the entry point
    pub index: usize,
    /// CD track number (decoded from BCD, typically 2..=99)
    pub track_no: u8,
    /// Minute (decoded from BCD, 0..=99)
    pub min: u8,
    /// Second (decoded from BCD, 0..=59)
    pub sec: u8,
    /// Frame (decoded from BCD, 0..=74, 75 frames per second)
    pub frame: u8,
}

impl VcdEntry {
    /// Formats the MSF time as a standard MM:SS:FF string.
    pub fn msf_string(&self) -> String {
        format!("{:02}:{:02}:{:02}", self.min, self.sec, self.frame)
    }

    /// Converts the MSF time to total seconds (including fractional frames).
    pub fn total_seconds(&self) -> f64 {
        (self.min as f64) * 60.0 + (self.sec as f64) + (self.frame as f64) / 75.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VcdEntries {
    pub id: String, // "ENTRYVCD" or "ENTRYSVD"
    pub version: u8,
    pub sys_prof_tag: u8,
    pub entry_count: usize,
    pub entries: Vec<VcdEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EntriesError {
    DataTooSmall(usize),
    Io(String),
}

impl std::fmt::Display for EntriesError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EntriesError::DataTooSmall(s) => {
                write!(f, "ENTRIES.VCD data too small: {} bytes (expected >= 12)", s)
            }
            EntriesError::Io(e) => write!(f, "IO error reading ENTRIES.VCD: {}", e),
        }
    }
}

impl std::error::Error for EntriesError {}

fn bcd_to_u8(b: u8) -> u8 {
    let high = (b >> 4) & 0x0F;
    let low = b & 0x0F;
    high * 10 + low
}

impl VcdEntries {
    pub fn parse(bytes: &[u8]) -> Result<Self, EntriesError> {
        if bytes.len() < 12 {
            return Err(EntriesError::DataTooSmall(bytes.len()));
        }

        let id = String::from_utf8_lossy(&bytes[0..8]).trim().to_string();
        let version = bytes[8];
        let sys_prof_tag = bytes[9];
        let entry_count = u16::from_be_bytes([bytes[10], bytes[11]]) as usize;

        let mut entries = Vec::with_capacity(entry_count);
        for i in 0..entry_count {
            let offset = 12 + i * 4;
            if offset + 4 > bytes.len() {
                break;
            }
            let track_bcd = bytes[offset];
            let min_bcd = bytes[offset + 1];
            let sec_bcd = bytes[offset + 2];
            let frame_bcd = bytes[offset + 3];

            entries.push(VcdEntry {
                index: i + 1,
                track_no: bcd_to_u8(track_bcd),
                min: bcd_to_u8(min_bcd),
                sec: bcd_to_u8(sec_bcd),
                frame: bcd_to_u8(frame_bcd),
            });
        }

        Ok(Self {
            id,
            version,
            sys_prof_tag,
            entry_count,
            entries,
        })
    }

    pub fn from_file(path: impl AsRef<Path>) -> Result<Self, EntriesError> {
        let bytes = std::fs::read(path).map_err(|e| EntriesError::Io(e.to_string()))?;
        Self::parse(&bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_entries_too_small() {
        assert!(VcdEntries::parse(&[0u8; 10]).is_err());
    }

    #[test]
    fn test_entries_parse_synthetic() {
        let mut buf = vec![0u8; 12 + 2 * 4];
        buf[0..8].copy_from_slice(b"ENTRYVCD");
        buf[8] = 2; // version
        buf[9] = 0; // sys_prof_tag
        buf[10..12].copy_from_slice(&2u16.to_be_bytes()); // 2 entries

        // Entry 1: Track 2, 01:23:45 (in BCD: 0x02, 0x01, 0x23, 0x45)
        buf[12] = 0x02;
        buf[13] = 0x01;
        buf[14] = 0x23;
        buf[15] = 0x45;

        // Entry 2: Track 2, 05:12:00 (in BCD: 0x02, 0x05, 0x12, 0x00)
        buf[16] = 0x02;
        buf[17] = 0x05;
        buf[18] = 0x12;
        buf[19] = 0x00;

        let entries = VcdEntries::parse(&buf).expect("Failed to parse valid synthetic ENTRIES");
        assert_eq!(entries.id, "ENTRYVCD");
        assert_eq!(entries.version, 2);
        assert_eq!(entries.sys_prof_tag, 0);
        assert_eq!(entries.entry_count, 2);
        assert_eq!(entries.entries.len(), 2);

        let e1 = &entries.entries[0];
        assert_eq!(e1.index, 1);
        assert_eq!(e1.track_no, 2);
        assert_eq!(e1.min, 1);
        assert_eq!(e1.sec, 23);
        assert_eq!(e1.frame, 45);
        assert_eq!(e1.msf_string(), "01:23:45");
        let expected_sec = 1.0 * 60.0 + 23.0 + 45.0 / 75.0;
        assert!((e1.total_seconds() - expected_sec).abs() < 1e-6);

        let e2 = &entries.entries[1];
        assert_eq!(e2.index, 2);
        assert_eq!(e2.track_no, 2);
        assert_eq!(e2.min, 5);
        assert_eq!(e2.sec, 12);
        assert_eq!(e2.frame, 0);
        assert_eq!(e2.msf_string(), "05:12:00");
    }

    #[test]
    fn test_entries_parse_fixture() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/mock_disc/VCD/ENTRIES.VCD");
        if path.exists() {
            let entries = VcdEntries::from_file(&path).expect("Failed to parse fixture ENTRIES.VCD");
            assert_eq!(entries.id, "ENTRYVCD");
            assert_eq!(entries.version, 2);
            assert_eq!(entries.entry_count, 27);
            assert_eq!(entries.entries.len(), 27);

            // First entry in fixture
            let first = &entries.entries[0];
            assert_eq!(first.index, 1);
            assert_eq!(first.track_no, 2);
            assert_eq!(first.msf_string(), "05:02:70");

            // 27th entry
            let last = &entries.entries[26];
            assert_eq!(last.index, 27);
            assert_eq!(last.track_no, 28);
            assert_eq!(last.msf_string(), "24:00:62");
        }
    }
}
