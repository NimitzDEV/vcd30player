use crate::video::decoder::MpegDecoder;
use rodio::buffer::SamplesBuffer;
use rodio::Sink;
use std::time::Instant;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VideoPlayState {
    Playing,
    Paused,
    Stopped,
    Ended,
}

pub struct VideoPlayer {
    decoder: MpegDecoder,
    pub state: VideoPlayState,
    audio_sink: Option<Sink>,
    current_time: f64,
    start_time: f64,
    end_time: f64,
    // Wall clock for fallback and smooth interpolation
    play_start_instant: Option<Instant>,
    paused_duration_secs: f64,
    pause_start_instant: Option<Instant>,
    // Video frame presentation
    current_frame_rgba: Vec<u8>,
    current_frame_pts: f64,
    width: u32,
    height: u32,
    framerate: f64,
    duration: f64,
    // Context
    pub filename: String,
    pub exit_target: Option<String>,
    // Audio tracking
    audio_chunks_sent: usize,
    audio_samples_per_chunk: usize,
    sample_rate: u32,
    has_audio: bool,
    has_video: bool,
}

impl VideoPlayer {
    /// Creates a new VideoPlayer instance from raw .DAT or .MPG bytes.
    pub fn new(
        raw_bytes: &[u8],
        filename: String,
        audio_sink: Option<Sink>,
        start_frame: u32,
        end_frame: u32,
        exit_target: Option<String>,
    ) -> Result<Self, String> {
        let mut decoder = MpegDecoder::from_bytes(raw_bytes)?;
        let (width, height) = decoder.dimensions();
        let framerate = if decoder.framerate() > 0.0 {
            decoder.framerate()
        } else {
            25.0
        };
        let sample_rate = if decoder.samplerate() > 0 {
            decoder.samplerate()
        } else {
            44100
        };
        let duration = decoder.duration();
        let has_audio = decoder.has_audio();
        let has_video = decoder.has_video();

        let start_time = (start_frame as f64 / framerate).min(duration);
        let end_time = if end_frame > 0 {
            (end_frame as f64 / framerate).min(duration)
        } else {
            duration
        };

        if start_time > 0.0 {
            decoder.seek(start_time);
        }

        let mut current_frame_rgba = vec![0u8; (width * height * 4) as usize];
        let initial_pts = decoder.decode_video_frame(&mut current_frame_rgba).unwrap_or(start_time);

        Ok(Self {
            decoder,
            state: VideoPlayState::Playing,
            audio_sink,
            current_time: start_time,
            start_time,
            end_time,
            play_start_instant: Some(Instant::now()),
            paused_duration_secs: 0.0,
            pause_start_instant: None,
            current_frame_rgba,
            current_frame_pts: initial_pts,
            width,
            height,
            framerate,
            duration,
            filename,
            exit_target,
            audio_chunks_sent: 0,
            audio_samples_per_chunk: 1152,
            sample_rate,
            has_audio,
            has_video,
        })
    }

    /// Advances playback, keeping audio buffer fed and decoding video frames on time.
    /// Returns true if a new video frame was rendered into `current_frame_rgba`.
    pub fn update(&mut self) -> bool {
        if self.state != VideoPlayState::Playing {
            return false;
        }

        // 1. Keep audio sink fed with decoded MP2 samples (~150ms buffer)
        if self.has_audio {
            if let Some(ref sink) = self.audio_sink {
                // Keep around 4 to 8 chunks (100~200ms) queued in sink
                while sink.len() < 6 && !self.decoder.has_ended() {
                    if let Some(audio) = self.decoder.decode_audio_samples() {
                        self.audio_samples_per_chunk = audio.count;
                        let source = SamplesBuffer::new(2, self.sample_rate, audio.samples);
                        sink.append(source);
                        self.audio_chunks_sent += 1;
                    } else {
                        break;
                    }
                }
            }
        }

        // 2. Compute master playback clock
        let elapsed_wall = if let Some(start) = self.play_start_instant {
            start.elapsed().as_secs_f64() - self.paused_duration_secs
        } else {
            0.0
        };

        let target_clock = self.start_time + elapsed_wall;
        self.current_time = target_clock;

        // Check bounds / end
        if self.current_time >= self.end_time || self.decoder.has_ended() {
            self.state = VideoPlayState::Ended;
            if let Some(ref sink) = self.audio_sink {
                sink.stop();
            }
            return false;
        }

        // 3. Decode video frames to catch up with master clock
        let mut new_frame_decoded = false;
        // Threshold: if current frame PTS is behind target clock by more than half a frame period
        let frame_period = 1.0 / self.framerate;
        while self.current_frame_pts < target_clock && !self.decoder.has_ended() {
            if let Some(pts) = self.decoder.decode_video_frame(&mut self.current_frame_rgba) {
                self.current_frame_pts = pts;
                new_frame_decoded = true;
                // If we caught up, break
                if self.current_frame_pts >= target_clock - frame_period * 0.5 {
                    break;
                }
            } else {
                break;
            }
        }

        new_frame_decoded
    }

    /// Toggles play / pause.
    pub fn toggle_play_pause(&mut self) {
        match self.state {
            VideoPlayState::Playing => {
                self.state = VideoPlayState::Paused;
                self.pause_start_instant = Some(Instant::now());
                if let Some(ref sink) = self.audio_sink {
                    sink.pause();
                }
            }
            VideoPlayState::Paused => {
                self.state = VideoPlayState::Playing;
                if let Some(pause_start) = self.pause_start_instant.take() {
                    self.paused_duration_secs += pause_start.elapsed().as_secs_f64();
                }
                if let Some(ref sink) = self.audio_sink {
                    sink.play();
                }
            }
            VideoPlayState::Ended | VideoPlayState::Stopped => {
                self.seek(self.start_time);
                self.state = VideoPlayState::Playing;
            }
        }
    }

    /// Stops playback and halts audio.
    pub fn stop(&mut self) {
        self.state = VideoPlayState::Stopped;
        if let Some(ref sink) = self.audio_sink {
            sink.stop();
        }
    }

    /// Seeks to a specific target timestamp in seconds.
    pub fn seek(&mut self, target_seconds: f64) {
        let clamped = target_seconds.max(self.start_time).min(self.end_time);
        if let Some(ref sink) = self.audio_sink {
            sink.stop();
        }

        self.decoder.seek(clamped);
        self.current_time = clamped;
        self.audio_chunks_sent = 0;

        // Reset clock baseline
        self.play_start_instant = Some(Instant::now());
        self.paused_duration_secs = 0.0;
        self.start_time = clamped;

        // Immediately decode one frame for preview
        if let Some(pts) = self.decoder.decode_video_frame(&mut self.current_frame_rgba) {
            self.current_frame_pts = pts;
        }

        // Resume if previously playing
        if self.state == VideoPlayState::Playing {
            if let Some(ref sink) = self.audio_sink {
                sink.play();
            }
        }
    }

    /// Returns the latest decoded RGBA frame buffer.
    pub fn current_frame(&self) -> &[u8] {
        &self.current_frame_rgba
    }

    pub fn dimensions(&self) -> (u32, u32) {
        (self.width, self.height)
    }

    pub fn current_time(&self) -> f64 {
        self.current_time
    }

    pub fn duration(&self) -> f64 {
        self.duration
    }

    pub fn framerate(&self) -> f64 {
        self.framerate
    }

    pub fn is_playing(&self) -> bool {
        self.state == VideoPlayState::Playing
    }

    pub fn is_paused(&self) -> bool {
        self.state == VideoPlayState::Paused
    }

    pub fn is_ended(&self) -> bool {
        self.state == VideoPlayState::Ended || self.state == VideoPlayState::Stopped
    }

    pub fn has_video(&self) -> bool {
        self.has_video
    }
}
