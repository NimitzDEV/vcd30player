//! Precise Video CD version detector across VCD 1.0, 1.1, 2.0, 3.0, and Super VCD.

use std::fmt;
use std::path::{Path, PathBuf};

use super::entries::VcdEntries;
use super::info::VcdInfo;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VcdDiscType {
    /// VCD 3.0 Interactive Multimedia disc (EnReach I-Reader format)
    Vcd30Interactive {
        version_str: String,
        has_vcd20_layer: bool,
        entry_count: usize,
    },
    /// VCD 2.0 Standard interactive disc with Playback Control (PBC)
    Vcd20WithPbc {
        entry_count: usize,
        segment_count: usize,
    },
    /// VCD 2.0 Linear disc (No PBC, pure sequential playback)
    Vcd20Linear { entry_count: usize },
    /// VCD 1.1 Standard disc (with tracks / entry points)
    Vcd11 { entry_count: usize },
    /// VCD 1.0 Original linear disc
    Vcd10,
    /// Super VCD (SVCD) disc (MPEG-2 variable bitrate)
    SuperVcd,
    /// HQ-VCD disc
    HqVcd,
    /// Raw video disc with MPEG files in MPEGAV or root
    RawVideoDisc { video_count: usize },
    /// Unknown or non-VCD directory
    Unknown,
}

impl fmt::Display for VcdDiscType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            VcdDiscType::Vcd30Interactive {
                has_vcd20_layer,
                entry_count,
                ..
            } => {
                if *has_vcd20_layer && *entry_count > 0 {
                    write!(f, "VCD 3.0 (兼容 VCD 2.0，共 {} 个章节)", entry_count)
                } else if *has_vcd20_layer {
                    write!(f, "VCD 3.0 (兼容 VCD 2.0)")
                } else {
                    write!(f, "VCD 3.0 (互动版)")
                }
            }
            VcdDiscType::Vcd20WithPbc {
                entry_count,
                segment_count,
            } => {
                write!(
                    f,
                    "VCD 2.0 (带 PBC 交互，{} 章节，{} 菜单段)",
                    entry_count, segment_count
                )
            }
            VcdDiscType::Vcd20Linear { entry_count } => {
                write!(f, "VCD 2.0 (纯视频，{} 章节)", entry_count)
            }
            VcdDiscType::Vcd11 { entry_count } => {
                write!(f, "VCD 1.1 ({} 章节)", entry_count)
            }
            VcdDiscType::Vcd10 => {
                write!(f, "VCD 1.0 (纯视频)")
            }
            VcdDiscType::SuperVcd => {
                write!(f, "Super VCD (SVCD)")
            }
            VcdDiscType::HqVcd => {
                write!(f, "HQ-VCD")
            }
            VcdDiscType::RawVideoDisc { video_count } => {
                write!(f, "视频数据盘 (共 {} 个视频)", video_count)
            }
            VcdDiscType::Unknown => {
                write!(f, "未知光盘格式")
            }
        }
    }
}

impl VcdDiscType {
    /// Returns the concise, pure English version label (e.g. "VCD 3.0", "VCD 2.0", "VCD 1.1", "VCD 1.0", "SVCD").
    pub fn version_label(&self) -> &'static str {
        match self {
            VcdDiscType::Vcd30Interactive { .. } => "VCD 3.0",
            VcdDiscType::Vcd20WithPbc { .. } | VcdDiscType::Vcd20Linear { .. } => "VCD 2.0",
            VcdDiscType::Vcd11 { .. } => "VCD 1.1",
            VcdDiscType::Vcd10 => "VCD 1.0",
            VcdDiscType::SuperVcd => "SVCD",
            VcdDiscType::HqVcd => "HQ-VCD",
            VcdDiscType::RawVideoDisc { .. } => "VIDEO CD",
            VcdDiscType::Unknown => "UNKNOWN DISC",
        }
    }

    /// Returns true if this disc format supports VCD 3.0 Interactive mode.
    pub fn supports_vcd30(&self) -> bool {
        matches!(self, VcdDiscType::Vcd30Interactive { .. })
    }

    /// Returns true if this disc format supports VCD 2.0 Classic mode.
    pub fn supports_vcd20(&self) -> bool {
        matches!(
            self,
            VcdDiscType::Vcd30Interactive { .. }
                | VcdDiscType::Vcd20WithPbc { .. }
                | VcdDiscType::Vcd20Linear { .. }
        )
    }

    /// Returns true if this disc format supports VCD 1.0 Linear mode.
    pub fn supports_vcd10(&self) -> bool {
        !matches!(self, VcdDiscType::Unknown)
    }
}

/// Finds a file or folder path case-insensitively given path segments.
pub fn find_path_ci(root: &Path, rel_parts: &[&str]) -> Option<PathBuf> {
    let mut current = root.to_path_buf();
    for part in rel_parts {
        if !current.exists() {
            return None;
        }
        let direct = current.join(part);
        if direct.exists() {
            current = direct;
            continue;
        }
        let entries = std::fs::read_dir(&current).ok()?;
        let mut found = None;
        for entry in entries.flatten() {
            if let Some(name) = entry.file_name().to_str() {
                if name.eq_ignore_ascii_case(part) {
                    found = Some(entry.path());
                    break;
                }
            }
        }
        current = found?;
    }
    Some(current)
}

/// Detects the exact VCD version and format of a disc root directory.
pub fn detect_disc(disc_root: &Path) -> VcdDiscType {
    if !disc_root.exists() {
        return VcdDiscType::Unknown;
    }

    // 1. Check for VCD 3.0 Interactive Disc:
    // Requires PROGRAM/JAVA/AUTORUN.CLS with Java class magic (0xCAFEBABE),
    // and DATA/VCD_DATA containing compiled HTML (.CHM) files.
    let cls_path = find_path_ci(disc_root, &["PROGRAM", "JAVA", "AUTORUN.CLS"])
        .or_else(|| find_path_ci(disc_root, &["AUTORUN.CLS"]));

    let mut is_vcd30 = false;
    if let Some(ref cls) = cls_path {
        if let Ok(bytes) = std::fs::read(cls) {
            if bytes.len() >= 4 && &bytes[0..4] == &[0xCA, 0xFE, 0xBA, 0xBE] {
                // Check if DATA/VCD_DATA exists with .CHM files
                if let Some(vcd_data_dir) = find_path_ci(disc_root, &["DATA", "VCD_DATA"]) {
                    if let Ok(entries) = std::fs::read_dir(&vcd_data_dir) {
                        for entry in entries.flatten() {
                            if let Some(ext) = entry.path().extension().and_then(|s| s.to_str()) {
                                if ext.eq_ignore_ascii_case("chm") {
                                    is_vcd30 = true;
                                    break;
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    // Also check for optional VCD standard layer (INFO.VCD / ENTRIES.VCD)
    let info_path = find_path_ci(disc_root, &["VCD", "INFO.VCD"]);
    let entries_path = find_path_ci(disc_root, &["VCD", "ENTRIES.VCD"]);

    let vcd_info = info_path.and_then(|p| VcdInfo::from_file(p).ok());
    let vcd_entries = entries_path.and_then(|p| VcdEntries::from_file(p).ok());
    let entry_count = vcd_entries.as_ref().map(|e| e.entry_count).unwrap_or(0);

    if is_vcd30 {
        let has_vcd20_layer = vcd_info.as_ref().map(|i| i.is_vcd20()).unwrap_or(false);
        return VcdDiscType::Vcd30Interactive {
            version_str: "3.0".to_string(),
            has_vcd20_layer,
            entry_count,
        };
    }

    // 2. Check standard VCD White Book metadata (INFO.VCD)
    if let Some(info) = vcd_info {
        if info.id.eq_ignore_ascii_case("SUPERVCD")
            || find_path_ci(disc_root, &["SVCD"]).is_some()
        {
            return VcdDiscType::SuperVcd;
        }

        if info.id.eq_ignore_ascii_case("HQ-VCD") {
            return VcdDiscType::HqVcd;
        }

        if info.version == 2 {
            let has_pbc_files = find_path_ci(disc_root, &["VCD", "PSD.VCD"]).is_some()
                && find_path_ci(disc_root, &["VCD", "LOT.VCD"]).is_some();
            if info.has_pbc() || has_pbc_files {
                return VcdDiscType::Vcd20WithPbc {
                    entry_count,
                    segment_count: info.item_count as usize,
                };
            } else {
                return VcdDiscType::Vcd20Linear { entry_count };
            }
        } else if info.version == 1 {
            if info.sys_prof_tag == 1 {
                return VcdDiscType::Vcd11 { entry_count };
            } else {
                return VcdDiscType::Vcd10;
            }
        }
    }

    // 3. Fallback: Check for raw video directory (MPEGAV or root video files)
    let mut video_count = 0;
    let mpegav_dir = find_path_ci(disc_root, &["MPEGAV"]).unwrap_or_else(|| disc_root.to_path_buf());
    if let Ok(entries) = std::fs::read_dir(mpegav_dir) {
        for entry in entries.flatten() {
            if let Some(ext) = entry.path().extension().and_then(|s| s.to_str()) {
                if ext.eq_ignore_ascii_case("dat") || ext.eq_ignore_ascii_case("mpg") || ext.eq_ignore_ascii_case("mpeg") {
                    video_count += 1;
                }
            }
        }
    }

    if video_count > 0 {
        VcdDiscType::RawVideoDisc { video_count }
    } else {
        VcdDiscType::Unknown
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_display_formatting() {
        let t1 = VcdDiscType::Vcd30Interactive {
            version_str: "3.0".to_string(),
            has_vcd20_layer: true,
            entry_count: 27,
        };
        assert_eq!(t1.to_string(), "VCD 3.0 (兼容 VCD 2.0，共 27 个章节)");

        let t2 = VcdDiscType::Vcd20WithPbc {
            entry_count: 10,
            segment_count: 5,
        };
        assert_eq!(t2.to_string(), "VCD 2.0 (带 PBC 交互，10 章节，5 菜单段)");

        let t3 = VcdDiscType::Vcd20Linear { entry_count: 4 };
        assert_eq!(t3.to_string(), "VCD 2.0 (纯视频，4 章节)");

        let t4 = VcdDiscType::Vcd11 { entry_count: 2 };
        assert_eq!(t4.to_string(), "VCD 1.1 (2 章节)");

        let t5 = VcdDiscType::Vcd10;
        assert_eq!(t5.to_string(), "VCD 1.0 (纯视频)");

        let t6 = VcdDiscType::SuperVcd;
        assert_eq!(t6.to_string(), "Super VCD (SVCD)");

        let t7 = VcdDiscType::HqVcd;
        assert_eq!(t7.to_string(), "HQ-VCD");

        let t8 = VcdDiscType::RawVideoDisc { video_count: 3 };
        assert_eq!(t8.to_string(), "视频数据盘 (共 3 个视频)");

        let t9 = VcdDiscType::Unknown;
        assert_eq!(t9.to_string(), "未知光盘格式");
    }

    #[test]
    fn test_version_label() {
        let t1 = VcdDiscType::Vcd30Interactive {
            version_str: "3.0".to_string(),
            has_vcd20_layer: true,
            entry_count: 27,
        };
        assert_eq!(t1.version_label(), "VCD 3.0");

        let t2 = VcdDiscType::Vcd20WithPbc {
            entry_count: 10,
            segment_count: 5,
        };
        assert_eq!(t2.version_label(), "VCD 2.0");

        let t3 = VcdDiscType::Vcd20Linear { entry_count: 4 };
        assert_eq!(t3.version_label(), "VCD 2.0");

        let t4 = VcdDiscType::Vcd11 { entry_count: 2 };
        assert_eq!(t4.version_label(), "VCD 1.1");

        let t5 = VcdDiscType::Vcd10;
        assert_eq!(t5.version_label(), "VCD 1.0");

        let t6 = VcdDiscType::SuperVcd;
        assert_eq!(t6.version_label(), "SVCD");

        let t7 = VcdDiscType::HqVcd;
        assert_eq!(t7.version_label(), "HQ-VCD");

        let t8 = VcdDiscType::RawVideoDisc { video_count: 3 };
        assert_eq!(t8.version_label(), "VIDEO CD");

        let t9 = VcdDiscType::Unknown;
        assert_eq!(t9.version_label(), "UNKNOWN DISC");
    }

    #[test]
    fn test_detect_mock_disc() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/mock_disc");
        let disc_type = detect_disc(&path);
        match disc_type {
            VcdDiscType::Vcd30Interactive {
                has_vcd20_layer,
                entry_count,
                ..
            } => {
                assert!(has_vcd20_layer, "Mock disc should have VCD 2.0 layer");
                assert_eq!(entry_count, 27, "Mock disc should have 27 entries");
            }
            other => panic!("Expected Vcd30Interactive, got {:?}", other),
        }
    }

    #[test]
    fn test_detect_live_disc_if_available() {
        if let Some(val) = std::env::var_os("VCD_TEST_DISC") {
            let live_path = PathBuf::from(val);
            if live_path.exists() {
                let disc_type = detect_disc(&live_path);
                match disc_type {
                    VcdDiscType::Vcd30Interactive {
                        has_vcd20_layer,
                        entry_count,
                        ..
                    } => {
                        assert!(has_vcd20_layer);
                        assert_eq!(entry_count, 27);
                    }
                    other => panic!("Expected test disc to be Vcd30Interactive, got {:?}", other),
                }
            }
        }
    }

    #[test]
    fn test_find_path_ci() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/mock_disc");
        // Test uppercase/lowercase resolution
        let found = find_path_ci(&path, &["program", "java", "autorun.cls"]);
        assert!(found.is_some(), "Case-insensitive find should succeed");
    }

    #[test]
    fn test_detect_synthetic_raw_video() {
        let temp_dir = std::env::temp_dir().join("vcd_test_raw_video");
        let mpegav = temp_dir.join("MPEGAV");
        let _ = std::fs::create_dir_all(&mpegav);
        let _ = std::fs::write(mpegav.join("AVSEQ01.DAT"), b"MOCK_MPEG");

        let disc_type = detect_disc(&temp_dir);
        assert_eq!(disc_type, VcdDiscType::RawVideoDisc { video_count: 1 });

        let _ = std::fs::remove_dir_all(&temp_dir);
    }
}
