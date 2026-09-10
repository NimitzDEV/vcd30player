//! Video CD (VCD) specification parsing and disc detection module.

pub mod detector;
pub mod entries;
pub mod info;

pub use detector::{detect_disc, VcdDiscType};
pub use entries::{VcdEntries, VcdEntry};
pub use info::VcdInfo;
