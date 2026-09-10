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

/// Returns the primary test disc root for fixture-based tests (mock_disc).
#[allow(dead_code)]
pub fn get_test_disc_root() -> PathBuf {
    mock_disc_root()
}

/// Returns an optional live disc root if configured via the VCD_TEST_DISC environment variable.
/// Returns None in CI or when no test disc is configured.
#[allow(dead_code)]
pub fn get_live_disc_root() -> Option<PathBuf> {
    if let Some(val) = std::env::var_os("VCD_TEST_DISC") {
        let p = PathBuf::from(val);
        if p.exists() {
            return Some(p);
        }
    }
    None
}
