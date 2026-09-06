//! Parser for `AUTORUN.CLS` (Standard JDK 1.1 Class file) to extract disc configuration.
//!
//! Extracts:
//! - License / Cover info (UserName, TitleName, COVER.YBM)
//! - Opening background MPEG video (MUSIC01.DAT)
//! - Initial interactive page (HOMEPAGE.CHM)

use std::fmt;

pub const JAVA_CLASS_MAGIC: u32 = 0xCAFEBABE;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClsError {
    DataTooSmall(usize),
    InvalidMagic(u32),
    UnsupportedVersion { major: u16, minor: u16 },
    CorruptedConstantPool,
    MissingConfigString(String),
}

impl fmt::Display for ClsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ClsError::DataTooSmall(len) => {
                write!(f, "Data too small for Java class: {} bytes", len)
            }
            ClsError::InvalidMagic(m) => {
                write!(f, "Invalid class magic: 0x{:08X} (expected 0xCAFEBABE)", m)
            }
            ClsError::UnsupportedVersion { major, minor } => {
                write!(f, "Unsupported Java class version: {}.{}", major, minor)
            }
            ClsError::CorruptedConstantPool => write!(f, "Corrupted constant pool in class file"),
            ClsError::MissingConfigString(s) => {
                write!(f, "Missing expected configuration string: {}", s)
            }
        }
    }
}

impl std::error::Error for ClsError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AutoRunConfig {
    pub user_name: String,
    pub title_name: String,
    pub cover_ybm: String,
    pub license_signature: String,
    pub opening_mpeg: Option<String>,
    pub homepage_chm: String,
}

impl AutoRunConfig {
    /// Parses an `AUTORUN.CLS` binary.
    pub fn parse(bytes: &[u8]) -> Result<Self, ClsError> {
        if bytes.len() < 10 {
            return Err(ClsError::DataTooSmall(bytes.len()));
        }

        let magic = u32::from_be_bytes(bytes[0..4].try_into().unwrap());
        if magic != JAVA_CLASS_MAGIC {
            return Err(ClsError::InvalidMagic(magic));
        }

        let minor = u16::from_be_bytes(bytes[4..6].try_into().unwrap());
        let major = u16::from_be_bytes(bytes[6..8].try_into().unwrap());

        if major > 50 {
            return Err(ClsError::UnsupportedVersion { major, minor });
        }

        let cp_count = u16::from_be_bytes(bytes[8..10].try_into().unwrap()) as usize;
        let mut offset = 10;

        // Parse constant pool strings
        // In class format, CP index is 1-based
        let mut strings = Vec::new();
        let mut i = 1;
        while i < cp_count {
            if offset >= bytes.len() {
                return Err(ClsError::CorruptedConstantPool);
            }

            let tag = bytes[offset];
            offset += 1;

            match tag {
                1 => {
                    // CONSTANT_Utf8
                    if offset + 2 > bytes.len() {
                        return Err(ClsError::CorruptedConstantPool);
                    }
                    let len =
                        u16::from_be_bytes(bytes[offset..offset + 2].try_into().unwrap()) as usize;
                    offset += 2;
                    if offset + len > bytes.len() {
                        return Err(ClsError::CorruptedConstantPool);
                    }
                    let s = String::from_utf8_lossy(&bytes[offset..offset + len]).to_string();
                    strings.push(s);
                    offset += len;
                }
                7 => {
                    // CONSTANT_Class
                    offset += 2;
                }
                8 => {
                    // CONSTANT_String
                    offset += 2;
                }
                9 | 10 | 11 | 12 => {
                    // Fieldref, Methodref, InterfaceMethodref, NameAndType
                    offset += 4;
                }
                3 | 4 => {
                    // Integer, Float
                    offset += 4;
                }
                5 | 6 => {
                    // Long, Double (takes two entries)
                    offset += 8;
                    i += 1;
                }
                _ => {
                    // Unknown tag, stop constant pool parsing
                    break;
                }
            }
            i += 1;
        }

        // Find configuration strings from parsed UTF-8 entries
        let mut user_name = "UserName VCD30".to_string();
        let mut title_name = "TitleName Demo".to_string();
        let mut cover_ybm = "COVER.YBM".to_string();
        let mut license_sig = String::new();
        let mut homepage_chm = "HOMEPAGE.CHM".to_string();

        // Check if PlayMpeg invocation symbol or .DAT string is present in the constant pool
        let has_play_mpeg = strings.iter().any(|s| s == "PlayMpeg");
        let dat_string = strings
            .iter()
            .find(|s| s.to_uppercase().ends_with(".DAT"))
            .cloned();

        let opening_mpeg = if let Some(dat) = dat_string {
            Some(dat)
        } else if has_play_mpeg {
            Some("MUSIC01.DAT".to_string())
        } else {
            None
        };

        for s in &strings {
            if s.starts_with("UserName") {
                user_name = s.clone();
            } else if s.starts_with("TitleName") {
                title_name = s.clone();
            } else if s.to_uppercase().ends_with(".YBM") {
                cover_ybm = s.clone();
            } else if s.to_uppercase().ends_with(".CHM") {
                homepage_chm = s.clone();
            } else if s.contains('^') || s.contains('#') {
                license_sig = s.clone();
            }
        }

        Ok(Self {
            user_name,
            title_name,
            cover_ybm,
            license_signature: license_sig,
            opening_mpeg,
            homepage_chm,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_class_magic_validation() {
        let bad = vec![0u8; 20];
        assert_eq!(AutoRunConfig::parse(&bad), Err(ClsError::InvalidMagic(0)));
    }

    #[test]
    fn test_cls_no_opening_video_when_not_called() {
        // Construct a mock class with only LicenseCheck and HOMEPAGE.CHM, no PlayMpeg or .DAT
        let mut class_bytes = Vec::new();
        class_bytes.extend_from_slice(&JAVA_CLASS_MAGIC.to_be_bytes());
        class_bytes.extend_from_slice(&3u16.to_be_bytes()); // minor
        class_bytes.extend_from_slice(&45u16.to_be_bytes()); // major 45.3

        let utf8_entries = [
            "COVER.YBM",
            "HOMEPAGE.CHM",
            "UserName LeiShi",
            "TitleName LeiShi",
            "^%$@# ]`d_a`b5#:5:9<99B*+",
        ];
        let cp_count = (utf8_entries.len() + 1) as u16;
        class_bytes.extend_from_slice(&cp_count.to_be_bytes());

        for entry in &utf8_entries {
            class_bytes.push(1); // CONSTANT_Utf8 tag
            class_bytes.extend_from_slice(&(entry.len() as u16).to_be_bytes());
            class_bytes.extend_from_slice(entry.as_bytes());
        }

        let cfg = AutoRunConfig::parse(&class_bytes).expect("Failed to parse mock CLS");
        assert_eq!(cfg.cover_ybm, "COVER.YBM");
        assert_eq!(cfg.homepage_chm, "HOMEPAGE.CHM");
        assert_eq!(cfg.opening_mpeg, None, "Must not assume opening video when absent in CLS");
    }

    #[test]
    fn test_cls_with_play_mpeg_and_dat() {
        // Construct a mock class with PlayMpeg and MUSIC01.DAT
        let mut class_bytes = Vec::new();
        class_bytes.extend_from_slice(&JAVA_CLASS_MAGIC.to_be_bytes());
        class_bytes.extend_from_slice(&3u16.to_be_bytes());
        class_bytes.extend_from_slice(&45u16.to_be_bytes());

        let utf8_entries = [
            "COVER.YBM",
            "HOMEPAGE.CHM",
            "PlayMpeg",
            "MUSIC01.DAT",
        ];
        let cp_count = (utf8_entries.len() + 1) as u16;
        class_bytes.extend_from_slice(&cp_count.to_be_bytes());

        for entry in &utf8_entries {
            class_bytes.push(1);
            class_bytes.extend_from_slice(&(entry.len() as u16).to_be_bytes());
            class_bytes.extend_from_slice(entry.as_bytes());
        }

        let cfg = AutoRunConfig::parse(&class_bytes).expect("Failed to parse mock CLS");
        assert_eq!(cfg.opening_mpeg, Some("MUSIC01.DAT".to_string()));
    }
}
