//! Video CD track and chapter discovery across standard MPEGAV directories and ENTRIES.VCD.

use std::path::Path;

use super::detector::find_path_ci;
use super::entries::VcdEntries;

#[derive(Debug, Clone, PartialEq)]
pub struct DiscTrackInfo {
    /// 1-based track / chapter index
    pub index: usize,
    /// Physical CD track number (typically 2..=99)
    pub track_no: u8,
    /// Human-friendly display title (e.g. "MUSIC01.DAT" or "AVSEQ01.DAT (01)")
    pub title: String,
    /// Relative or absolute path / filename for loading (e.g. "MPEGAV/MUSIC01.DAT")
    pub file_name: String,
    /// Physical start timecode MM:SS:FF if available from ENTRIES.VCD
    pub msf_start: Option<String>,
    /// Playback start time in seconds relative to the beginning of the file
    pub start_seconds: f64,
    /// Playback end time in seconds relative to the beginning of the file (None if plays until EOF)
    pub end_seconds: Option<f64>,
}

/// Discovers all playable audio/video tracks and virtual chapters on a VCD disc root.
pub fn scan_disc_tracks(disc_root: &Path) -> Vec<DiscTrackInfo> {
    if !disc_root.exists() {
        return Vec::new();
    }

    // 1. Check for ENTRIES.VCD metadata
    let entries_path = find_path_ci(disc_root, &["VCD", "ENTRIES.VCD"]);
    let vcd_entries = entries_path.and_then(|p| VcdEntries::from_file(p).ok());

    // 2. Scan MPEGAV directory for video files
    let mpegav_dir = find_path_ci(disc_root, &["MPEGAV"]).unwrap_or_else(|| disc_root.to_path_buf());
    let mut files = Vec::new();

    if let Ok(dir_entries) = std::fs::read_dir(&mpegav_dir) {
        for entry in dir_entries.flatten() {
            let path = entry.path();
            if let Some(ext) = path.extension().and_then(|s| s.to_str()) {
                if ext.eq_ignore_ascii_case("dat")
                    || ext.eq_ignore_ascii_case("mpg")
                    || ext.eq_ignore_ascii_case("mpeg")
                {
                    if let Some(file_name) = path.file_name().and_then(|s| s.to_str()) {
                        files.push(file_name.to_string());
                    }
                }
            }
        }
    }

    // Sort files naturally (case-insensitive ASCII ordering)
    files.sort_by(|a, b| a.to_ascii_uppercase().cmp(&b.to_ascii_uppercase()));

    if files.is_empty() {
        return Vec::new();
    }

    let mut tracks = Vec::new();

    if let Some(v_entries) = vcd_entries.filter(|e| !e.entries.is_empty()) {
        // Map entries to files
        // In White Book, Track 1 is data, Track 2 is files[0], Track 3 is files[1], etc.
        // If there is only 1 file, all entries belong to files[0].
        let mut file_entries: Vec<Vec<&super::entries::VcdEntry>> = vec![Vec::new(); files.len()];

        for entry in &v_entries.entries {
            let file_idx = if files.len() == 1 {
                0
            } else if entry.track_no >= 2 && ((entry.track_no - 2) as usize) < files.len() {
                (entry.track_no - 2) as usize
            } else if ((entry.index - 1) as usize) < files.len() {
                (entry.index - 1) as usize
            } else {
                0
            };
            file_entries[file_idx].push(entry);
        }

        for (file_idx, fname) in files.iter().enumerate() {
            let mut entries_for_file = file_entries[file_idx].clone();
            if entries_for_file.is_empty() {
                // Unreferenced file in ENTRIES.VCD (auxiliary or padding clip, not a standard track)
                continue;
            } else if entries_for_file.len() == 1 {
                // Single entry for this file
                let e = entries_for_file[0];
                tracks.push(DiscTrackInfo {
                    index: 0,
                    track_no: e.track_no,
                    title: fname.clone(),
                    file_name: format!("MPEGAV/{}", fname),
                    msf_start: Some(e.msf_string()),
                    start_seconds: 0.0,
                    end_seconds: None,
                });
            } else {
                // Multiple entries: virtual chapters!
                entries_for_file.sort_by(|a, b| {
                    a.total_seconds()
                        .partial_cmp(&b.total_seconds())
                        .unwrap_or(std::cmp::Ordering::Equal)
                });

                let base_seconds = entries_for_file[0].total_seconds();
                for (j, e) in entries_for_file.iter().enumerate() {
                    let start_sec = (e.total_seconds() - base_seconds).max(0.0);
                    let end_sec = if j + 1 < entries_for_file.len() {
                        let next_sec = (entries_for_file[j + 1].total_seconds() - base_seconds).max(start_sec);
                        Some(next_sec)
                    } else {
                        None
                    };

                    tracks.push(DiscTrackInfo {
                        index: 0,
                        track_no: e.track_no,
                        title: format!("{} ({:02})", fname, j + 1),
                        file_name: format!("MPEGAV/{}", fname),
                        msf_start: Some(e.msf_string()),
                        start_seconds: start_sec,
                        end_seconds: end_sec,
                    });
                }
            }
        }

        // If no files matched any entries, fallback to treating all files as tracks
        if tracks.is_empty() {
            for (i, fname) in files.iter().enumerate() {
                let track_no = (i + 2).min(99) as u8;
                tracks.push(DiscTrackInfo {
                    index: 0,
                    track_no,
                    title: fname.clone(),
                    file_name: format!("MPEGAV/{}", fname),
                    msf_start: None,
                    start_seconds: 0.0,
                    end_seconds: None,
                });
            }
        }
    } else {
        // Fallback: 1 track per file when ENTRIES.VCD is missing or empty
        for (i, fname) in files.iter().enumerate() {
            let track_no = (i + 2).min(99) as u8;
            tracks.push(DiscTrackInfo {
                index: 0,
                track_no,
                title: fname.clone(),
                file_name: format!("MPEGAV/{}", fname),
                msf_start: None,
                start_seconds: 0.0,
                end_seconds: None,
            });
        }
    }

    // Assign sequential 1-based indices
    for (i, track) in tracks.iter_mut().enumerate() {
        track.index = i + 1;
    }

    tracks
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_scan_mock_disc_tracks() {
        let disc_path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/mock_disc");
        let tracks = scan_disc_tracks(&disc_path);
        assert!(!tracks.is_empty(), "Mock disc should have tracks in MPEGAV");

        let first = &tracks[0];
        assert_eq!(first.index, 1);
        assert!(first.file_name.starts_with("MPEGAV/"));
        assert!(first.msf_start.is_some(), "Track 1 should have MSF start timecode from ENTRIES.VCD");
    }

    #[test]
    fn test_scan_single_file_virtual_chapters() {
        let temp_dir = std::env::temp_dir().join(format!("vcd_virtual_chap_{}", std::process::id()));
        let mpegav = temp_dir.join("MPEGAV");
        let vcd = temp_dir.join("VCD");
        let _ = std::fs::remove_dir_all(&temp_dir);
        std::fs::create_dir_all(&mpegav).unwrap();
        std::fs::create_dir_all(&vcd).unwrap();

        // 1 single video file
        std::fs::write(mpegav.join("AVSEQ01.DAT"), b"mock video").unwrap();

        // ENTRIES.VCD with 3 entries
        // Header: "ENTRYVCD" (8B) + ver (0x02) + sys_prof (0x01) + count (0x0003) = 12B
        // Entry 1: Track 02, 02:00:00 (BCD: 0x02, 0x02, 0x00, 0x00) -> 120.0s (base)
        // Entry 2: Track 02, 05:30:00 (BCD: 0x02, 0x05, 0x30, 0x00) -> 330.0s (+210s)
        // Entry 3: Track 02, 09:15:37 (BCD: 0x02, 0x09, 0x15, 0x37) -> 555.4933s (+435.4933s)
        let mut entries_data = Vec::new();
        entries_data.extend_from_slice(b"ENTRYVCD");
        entries_data.push(0x02);
        entries_data.push(0x01);
        entries_data.extend_from_slice(&3u16.to_be_bytes());
        // Entry 1
        entries_data.extend_from_slice(&[0x02, 0x02, 0x00, 0x00]);
        // Entry 2
        entries_data.extend_from_slice(&[0x02, 0x05, 0x30, 0x00]);
        // Entry 3
        entries_data.extend_from_slice(&[0x02, 0x09, 0x15, 0x37]);
        std::fs::write(vcd.join("ENTRIES.VCD"), &entries_data).unwrap();

        let tracks = scan_disc_tracks(&temp_dir);
        assert_eq!(tracks.len(), 3);
        assert_eq!(tracks[0].index, 1);
        assert_eq!(tracks[0].title, "AVSEQ01.DAT (01)");
        assert_eq!(tracks[0].start_seconds, 0.0);
        assert_eq!(tracks[0].end_seconds, Some(210.0));

        assert_eq!(tracks[1].index, 2);
        assert_eq!(tracks[1].title, "AVSEQ01.DAT (02)");
        assert_eq!(tracks[1].start_seconds, 210.0);
        assert!((tracks[1].end_seconds.unwrap() - 435.4933).abs() < 0.01);

        assert_eq!(tracks[2].index, 3);
        assert_eq!(tracks[2].title, "AVSEQ01.DAT (03)");
        assert!((tracks[2].start_seconds - 435.4933).abs() < 0.01);
        assert_eq!(tracks[2].end_seconds, None);

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_scan_live_disc_tracks_if_available() {
        if let Some(val) = std::env::var_os("VCD_TEST_DISC") {
            let live_path = PathBuf::from(val);
            if live_path.exists() {
                let tracks = scan_disc_tracks(&live_path);
                assert_eq!(tracks.len(), 27, "Live disc should have exactly 27 tracks");
                assert_eq!(tracks[0].title, "MUSIC01.DAT");
                assert_eq!(tracks[26].title, "MUSIC27.DAT");
                assert_eq!(tracks[0].msf_start.as_deref(), Some("05:02.70"));
            }
        }
    }
}
