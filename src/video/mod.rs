//! MPEG-1 video demuxing, decoding, and playback module.

pub mod cdxa;
pub mod decoder;
pub mod player;

pub use cdxa::extract_mpeg_ps;
pub use decoder::{DecodedAudio, MpegDecoder};
pub use player::{VideoPlayState, VideoPlayer};
