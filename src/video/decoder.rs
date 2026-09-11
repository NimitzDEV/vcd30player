//! Safe Rust wrapper for pl_mpeg MPEG-1 video and MP2 audio decoder.

use crate::video::cdxa;
use std::os::raw::{c_int, c_uint};

pub const PLM_AUDIO_SAMPLES_PER_FRAME: usize = 1152;

#[repr(C)]
struct PlmPlane {
    width: c_uint,
    height: c_uint,
    data: *mut u8,
}

#[repr(C)]
pub struct PlmFrame {
    pub time: f64,
    pub width: c_uint,
    pub height: c_uint,
    y: PlmPlane,
    cr: PlmPlane,
    cb: PlmPlane,
}

#[repr(C)]
pub struct PlmSamples {
    pub time: f64,
    pub count: c_uint,
    pub interleaved: [f32; PLM_AUDIO_SAMPLES_PER_FRAME * 2],
}

enum PlmOpaque {}

unsafe extern "C" {
    fn plm_create_with_memory(bytes: *const u8, length: usize, free_when_done: c_int) -> *mut PlmOpaque;
    fn plm_destroy(self_ptr: *mut PlmOpaque);
    fn plm_get_width(self_ptr: *mut PlmOpaque) -> c_int;
    fn plm_get_height(self_ptr: *mut PlmOpaque) -> c_int;
    fn plm_get_framerate(self_ptr: *mut PlmOpaque) -> f64;
    fn plm_get_samplerate(self_ptr: *mut PlmOpaque) -> c_int;
    fn plm_get_duration(self_ptr: *mut PlmOpaque) -> f64;
    fn plm_get_time(self_ptr: *mut PlmOpaque) -> f64;
    fn plm_has_ended(self_ptr: *mut PlmOpaque) -> c_int;
    fn plm_seek(self_ptr: *mut PlmOpaque, time: f64, seek_exact: c_int) -> c_int;
    fn plm_probe(self_ptr: *mut PlmOpaque, probesize: usize) -> c_int;
    fn plm_decode_video(self_ptr: *mut PlmOpaque) -> *mut PlmFrame;
    fn plm_decode_audio(self_ptr: *mut PlmOpaque) -> *mut PlmSamples;
    fn plm_frame_to_rgba(frame: *mut PlmFrame, dest: *mut u8, stride: c_int);
    fn plm_get_num_video_streams(self_ptr: *mut PlmOpaque) -> c_int;
    fn plm_get_num_audio_streams(self_ptr: *mut PlmOpaque) -> c_int;
}

/// Decoded audio buffer containing interleaved stereo f32 samples.
#[derive(Debug, Clone)]
pub struct DecodedAudio {
    pub time: f64,
    pub count: usize,
    pub samples: Vec<f32>,
}

/// Safe Rust wrapper around a pl_mpeg instance.
pub struct MpegDecoder {
    raw: *mut PlmOpaque,
    _stream_data: Vec<u8>,
    width: u32,
    height: u32,
    framerate: f64,
    samplerate: u32,
    duration: f64,
    has_audio: bool,
    has_video: bool,
}

unsafe impl Send for MpegDecoder {}

impl MpegDecoder {
    /// Creates a decoder from raw file bytes (supports both RIFF/CDXA `.DAT` and standard `.MPG`).
    pub fn from_bytes(raw_bytes: &[u8]) -> Result<Self, String> {
        let ps_stream = cdxa::extract_mpeg_ps(raw_bytes)?;
        Self::from_ps_stream(ps_stream)
    }

    /// Creates a decoder from an already demuxed MPEG-1 Program Stream.
    pub fn from_ps_stream(ps_stream: Vec<u8>) -> Result<Self, String> {
        if ps_stream.is_empty() {
            return Err("Empty MPEG Program Stream".to_string());
        }

        let raw = unsafe {
            plm_create_with_memory(ps_stream.as_ptr(), ps_stream.len(), 0)
        };
        if raw.is_null() {
            return Err("Failed to create plm_t instance from MPEG stream".to_string());
        }

        // Probe up to 500KB to detect all video and audio streams
        unsafe { plm_probe(raw, 500 * 1024) };

        let width = unsafe { plm_get_width(raw) } as u32;
        let height = unsafe { plm_get_height(raw) } as u32;
        let framerate = unsafe { plm_get_framerate(raw) };
        let samplerate = unsafe { plm_get_samplerate(raw) } as u32;
        let raw_duration = unsafe { plm_get_duration(raw) };
        let duration = if raw_duration >= 0.0 { raw_duration } else { 0.0 };
        let num_video = unsafe { plm_get_num_video_streams(raw) };
        let num_audio = unsafe { plm_get_num_audio_streams(raw) };

        Ok(Self {
            raw,
            _stream_data: ps_stream,
            width,
            height,
            framerate,
            samplerate,
            duration,
            has_video: num_video > 0,
            has_audio: num_audio > 0,
        })
    }

    /// Decodes the next video frame into the provided RGBA buffer.
    /// Returns the presentation time stamp (PTS) in seconds if a frame was decoded.
    pub fn decode_video_frame(&mut self, rgba_out: &mut [u8]) -> Option<f64> {
        if self.raw.is_null() || !self.has_video {
            return None;
        }

        let frame_ptr = unsafe { plm_decode_video(self.raw) };
        if frame_ptr.is_null() {
            return None;
        }

        let frame = unsafe { &*frame_ptr };
        let expected_size = (frame.width * frame.height * 4) as usize;
        if rgba_out.len() >= expected_size {
            unsafe {
                plm_frame_to_rgba(frame_ptr, rgba_out.as_mut_ptr(), (frame.width * 4) as i32);
            }
            for pixel in rgba_out[..expected_size].chunks_exact_mut(4) {
                pixel[3] = 255;
            }
        }

        Some(frame.time)
    }


    /// Decodes the next audio frame.
    /// Returns the interleaved stereo samples and timestamp if an audio packet was decoded.
    pub fn decode_audio_samples(&mut self) -> Option<DecodedAudio> {
        if self.raw.is_null() || !self.has_audio {
            return None;
        }

        let samples_ptr = unsafe { plm_decode_audio(self.raw) };
        if samples_ptr.is_null() {
            return None;
        }

        let s = unsafe { &*samples_ptr };
        let sample_count = (s.count as usize).min(PLM_AUDIO_SAMPLES_PER_FRAME);
        let total_floats = sample_count * 2;

        Some(DecodedAudio {
            time: s.time,
            count: sample_count,
            samples: s.interleaved[..total_floats].to_vec(),
        })
    }

    /// Seeks to a specific timestamp in seconds.
    pub fn seek(&mut self, seconds: f64) -> bool {
        if self.raw.is_null() {
            return false;
        }
        unsafe { plm_seek(self.raw, seconds, 1) != 0 }
    }

    /// Checks if the stream has reached the end.
    pub fn has_ended(&self) -> bool {
        if self.raw.is_null() {
            return true;
        }
        unsafe { plm_has_ended(self.raw) != 0 }
    }

    /// Returns the current playback timestamp in seconds.
    pub fn current_time(&self) -> f64 {
        if self.raw.is_null() {
            0.0
        } else {
            unsafe { plm_get_time(self.raw) }
        }
    }

    pub fn dimensions(&self) -> (u32, u32) {
        (self.width, self.height)
    }

    pub fn framerate(&self) -> f64 {
        self.framerate
    }

    pub fn samplerate(&self) -> u32 {
        self.samplerate
    }

    pub fn duration(&self) -> f64 {
        self.duration
    }

    pub fn has_video(&self) -> bool {
        self.has_video
    }

    pub fn has_audio(&self) -> bool {
        self.has_audio
    }
}

impl Drop for MpegDecoder {
    fn drop(&mut self) {
        if !self.raw.is_null() {
            unsafe { plm_destroy(self.raw) };
            self.raw = std::ptr::null_mut();
        }
    }
}
