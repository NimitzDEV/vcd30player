//! Video CD (VCD) specification parsing and disc detection module.

pub mod detector;
pub mod entries;
pub mod info;
pub mod lot;
pub mod pbc;
pub mod psd;
pub mod segment;
pub mod track;

pub use detector::{detect_disc, VcdDiscType};
pub use entries::{VcdEntries, VcdEntry};
pub use info::VcdInfo;
pub use lot::LotTable;
pub use pbc::{PbcAction, PbcEngine, PbcState};
pub use psd::{EndListDesc, PlayListDesc, PsdDescriptor, PsdTable, SelectionListDesc};
pub use segment::{
    blit_segment_to_canvas, decode_segment_frame, downsample_704x576_to_352x288,
    resolve_segment_path,
};
pub use track::{scan_disc_tracks, DiscTrackInfo};
