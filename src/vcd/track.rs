//! Video CD track and chapter discovery across standard MPEGAV directories and ENTRIES.VCD.

use std::path::Path;

use super::detector::find_path_ci;
use super::entries::VcdEntries;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiscTrackInfo {
    /// 1-based track / chapter index
    pub index: usize,
    /// Physical CD track number (typically 2..=99)
    pub track_no: u8,
    /// Human-friendly display title (e.g. "MUSIC01.DAT")
    pub title: String,
    /// Relative or absolute path / filename for loading (e.g. "MPEGAV/MUSIC01.DAT")
    pub file_name: String,
    /// Physical start timecode MM:SS:FF if available from ENTRIES.VCD
    pub msf_start: Option<String>,
}

/// Discovers all playable audio/video tracks on a VCD disc root.
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

    let mut tracks = Vec::with_capacity(files.len());
    for (i, fname) in files.iter().enumerate() {
        let index = i + 1;
        let mut track_no = (index + 1).min(99) as u8;
        let mut msf_start = None;

        if let Some(ref v_entries) = vcd_entries {
            if i < v_entries.entries.len() {
                let entry = &v_entries.entries[i];
                track_no = entry.track_no;
                msf_start = Some(entry.msf_string());
            }
        }

        tracks.push(DiscTrackInfo {
            index,
            track_no,
            title: fname.clone(),
            file_name: format!("MPEGAV/{}", fname),
            msf_start,
        });
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
    fn test_scan_live_disc_tracks_if_available() {
        if let Some(val) = std::env::var_os("VCD_TEST_DISC") {
            let live_path = PathBuf::from(val);
            if live_path.exists() {
                let tracks = scan_disc_tracks(&live_path);
                assert_eq!(tracks.len(), 27, "Live disc should have exactly 27 tracks");
                assert_eq!(tracks[0].title, "MUSIC01.DAT");
                assert_eq!(tracks[26].title, "MUSIC27.DAT");
                assert_eq!(tracks[0].msf_start.as_deref(), Some("05:02:70"));
            }
        }
    }
}
