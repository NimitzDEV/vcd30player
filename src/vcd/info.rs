//! Parser for standard Video CD `INFO.VCD` system files.

use std::path::Path;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VcdInfo {
    /// System ID string, e.g. "VIDEO_CD", "SUPERVCD", "HQ-VCD  "
    pub id: String,
    /// Specification version, e.g. 0x02 for VCD 2.0, 0x01 for VCD 1.1 / SVCD
    pub version: u8,
    /// System profile tag, e.g. 0x01 for VCD 1.1, 0x00 for VCD 2.0 / SVCD
    pub sys_prof_tag: u8,
    /// Album / disc description string
    pub album_desc: String,
    /// Total volume count in album
    pub vol_count: u16,
    /// Current volume number
    pub vol_id: u16,
    /// PAL / NTSC format flags for tracks 2..=99
    pub pal_flags: [u8; 13],
    /// Disc status flags
    pub flags: u8,
    /// Size of PSD.VCD file in bytes (0 if non-interactive)
    pub psd_size: u32,
    /// First sector address of segment play items (BCD mm:ss:00)
    pub first_seg_addr: [u8; 3],
    /// Offset multiplier (typically 8)
    pub offset_mult: u8,
    /// Number of valid entries in LOT.VCD
    pub lot_entries: u16,
    /// Number of segment items used
    pub item_count: u16,
    /// Segment play item contents (SPI)
    pub spi_contents: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InfoError {
    DataTooSmall(usize),
    Io(String),
}

impl std::fmt::Display for InfoError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            InfoError::DataTooSmall(s) => {
                write!(f, "INFO.VCD data too small: {} bytes (expected >= 56)", s)
            }
            InfoError::Io(e) => write!(f, "IO error reading INFO.VCD: {}", e),
        }
    }
}

impl std::error::Error for InfoError {}

impl VcdInfo {
    pub fn parse(bytes: &[u8]) -> Result<Self, InfoError> {
        if bytes.len() < 56 {
            return Err(InfoError::DataTooSmall(bytes.len()));
        }

        let id = String::from_utf8_lossy(&bytes[0..8]).trim().to_string();
        let version = bytes[8];
        let sys_prof_tag = bytes[9];
        let album_desc = String::from_utf8_lossy(&bytes[10..26]).trim().to_string();
        let vol_count = u16::from_be_bytes([bytes[26], bytes[27]]);
        let vol_id = u16::from_be_bytes([bytes[28], bytes[29]]);

        let mut pal_flags = [0u8; 13];
        pal_flags.copy_from_slice(&bytes[30..43]);

        let flags = bytes[43];
        let psd_size = u32::from_be_bytes([bytes[44], bytes[45], bytes[46], bytes[47]]);
        let first_seg_addr = [bytes[48], bytes[49], bytes[50]];
        let offset_mult = bytes[51];
        let lot_entries = u16::from_be_bytes([bytes[52], bytes[53]]);
        let item_count = u16::from_be_bytes([bytes[54], bytes[55]]);

        let spi_len = (item_count as usize).min(1980);
        let spi_contents = if bytes.len() >= 56 + spi_len {
            bytes[56..56 + spi_len].to_vec()
        } else if bytes.len() > 56 {
            bytes[56..].to_vec()
        } else {
            Vec::new()
        };

        Ok(Self {
            id,
            version,
            sys_prof_tag,
            album_desc,
            vol_count,
            vol_id,
            pal_flags,
            flags,
            psd_size,
            first_seg_addr,
            offset_mult,
            lot_entries,
            item_count,
            spi_contents,
        })
    }

    pub fn from_file(path: impl AsRef<Path>) -> Result<Self, InfoError> {
        let bytes = std::fs::read(path).map_err(|e| InfoError::Io(e.to_string()))?;
        Self::parse(&bytes)
    }

    /// Checks if this disc is declared as VCD 2.0.
    pub fn is_vcd20(&self) -> bool {
        self.id == "VIDEO_CD" && self.version == 2
    }

    /// Checks if this disc has interactive PBC structures (psd_size > 0).
    pub fn has_pbc(&self) -> bool {
        self.psd_size > 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_info_too_small() {
        assert!(VcdInfo::parse(&[0u8; 30]).is_err());
    }

    #[test]
    fn test_info_parse_synthetic() {
        let mut buf = vec![0u8; 100];
        buf[0..8].copy_from_slice(b"VIDEO_CD");
        buf[8] = 2; // version
        buf[9] = 0; // sys_prof_tag
        buf[10..26].copy_from_slice(b"TEST_ALBUM      ");
        buf[26..28].copy_from_slice(&2u16.to_be_bytes()); // vol_count
        buf[28..30].copy_from_slice(&1u16.to_be_bytes()); // vol_id
        buf[43] = 0x04; // flags
        buf[44..48].copy_from_slice(&496u32.to_be_bytes()); // psd_size
        buf[51] = 8; // offset_mult
        buf[52..54].copy_from_slice(&29u16.to_be_bytes()); // lot_entries
        buf[54..56].copy_from_slice(&2u16.to_be_bytes()); // item_count
        buf[56] = 0x18; // spi 0
        buf[57] = 0x18; // spi 1

        let info = VcdInfo::parse(&buf).expect("Failed to parse valid synthetic INFO");
        assert_eq!(info.id, "VIDEO_CD");
        assert_eq!(info.version, 2);
        assert_eq!(info.sys_prof_tag, 0);
        assert_eq!(info.album_desc, "TEST_ALBUM");
        assert_eq!(info.vol_count, 2);
        assert_eq!(info.vol_id, 1);
        assert_eq!(info.psd_size, 496);
        assert_eq!(info.offset_mult, 8);
        assert_eq!(info.lot_entries, 29);
        assert_eq!(info.item_count, 2);
        assert_eq!(info.spi_contents.len(), 2);
        assert!(info.is_vcd20());
        assert!(info.has_pbc());
    }

    #[test]
    fn test_info_parse_fixture() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/mock_disc/VCD/INFO.VCD");
        if path.exists() {
            let info = VcdInfo::from_file(&path).expect("Failed to parse fixture INFO.VCD");
            assert_eq!(info.id, "VIDEO_CD");
            assert_eq!(info.version, 2);
            assert_eq!(info.vol_count, 1);
            assert_eq!(info.vol_id, 1);
            assert_eq!(info.psd_size, 496);
            assert_eq!(info.item_count, 2);
            assert!(info.is_vcd20());
            assert!(info.has_pbc());
        }
    }
}
