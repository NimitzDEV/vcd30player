use std::path::{Path, PathBuf};

/// Returns the path to the versioned, self-contained mock disc fixtures.
/// Guaranteed to exist and work in CI and on all operating systems.
#[allow(dead_code)]
pub fn mock_disc_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("mock_disc")
}

/// Returns the primary test disc root.
/// If VCD_TEST_DISC environment variable is set and exists, uses that.
/// Otherwise defaults to the self-contained mock_disc fixtures.
#[allow(dead_code)]
pub fn get_test_disc_root() -> PathBuf {
    if let Some(val) = std::env::var_os("VCD_TEST_DISC") {
        let p = PathBuf::from(val);
        if p.exists() {
            return p;
        }
    }
    mock_disc_root()
}

/// Returns an optional live disc root if a real physical or virtual CD-ROM is mounted.
/// Returns None in CI or when no physical disc is available.
#[allow(dead_code)]
pub fn get_live_disc_root() -> Option<PathBuf> {
    if let Some(val) = std::env::var_os("VCD_TEST_DISC") {
        let p = PathBuf::from(val);
        if p.exists() {
            return Some(p);
        }
    }
    for letter in ['H', 'I', 'G', 'J'] {
        let p = PathBuf::from(format!(r"{}:\", letter));
        if p.join("DATA").join("VCD_DATA").exists() {
            return Some(p);
        }
    }
    None
}
