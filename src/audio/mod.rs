pub mod channel;
pub use channel::AudioChannelMode;

use rodio::{Decoder, OutputStream, OutputStreamHandle, Sink, Source};
use std::collections::HashMap;
use std::io::Cursor;
use std::path::Path;

pub struct AudioManager {
    _stream: Option<OutputStream>,
    stream_handle: Option<OutputStreamHandle>,
    bgm_sink: Option<Sink>,
    sfx_sink: Option<Sink>,
    sound_cache: HashMap<String, Vec<u8>>,
}

impl AudioManager {
    /// Creates a silent, no-op AudioManager without touching native audio drivers.
    pub fn silent() -> Self {
        Self {
            _stream: None,
            stream_handle: None,
            bgm_sink: None,
            sfx_sink: None,
            sound_cache: HashMap::new(),
        }
    }

    pub fn new() -> Self {
        // Headless CI environments (GitHub Actions, etc.) on Windows have no audio hardware,
        // and WASAPI COM calls can trigger a hard STATUS_ACCESS_VIOLATION (0xc0000005).
        // Safely bypass audio hardware initialization in CI or when explicitly disabled.
        if std::env::var_os("CI").is_some()
            || std::env::var_os("GITHUB_ACTIONS").is_some()
            || std::env::var_os("VCD_NO_AUDIO").is_some()
            || std::env::var_os("VCD_HEADLESS").is_some()
        {
            return Self::silent();
        }

        match OutputStream::try_default() {
            Ok((stream, handle)) => Self {
                _stream: Some(stream),
                stream_handle: Some(handle),
                bgm_sink: None,
                sfx_sink: None,
                sound_cache: HashMap::new(),
            },
            Err(e) => {
                eprintln!("[AudioManager] Warning: Audio output device unavailable (falling back to silent mode): {}", e);
                Self::silent()
            }
        }
    }

    pub fn is_audio_available(&self) -> bool {
        self.stream_handle.is_some()
    }

    /// Whether background audio (BGM/narration) is currently playing.
    pub fn is_bgm_playing(&self) -> bool {
        self.bgm_sink.as_ref().map_or(false, |s| !s.empty())
    }

    /// Whether a one-shot sound effect (SFX) is currently playing.
    pub fn is_sound_playing(&self) -> bool {
        self.sfx_sink.as_ref().map_or(false, |s| !s.empty())
    }

    /// Creates a dedicated Sink for video MP2 audio playback.
    pub fn create_video_sink(&self) -> Option<Sink> {
        let handle = self.stream_handle.as_ref()?;
        Sink::try_new(handle).ok()
    }

    /// Plays a one-shot sound effect asynchronously from raw WAV bytes.
    /// Cancels any previous sound effect so re-triggering starts promptly.
    pub fn play_sound_bytes(&mut self, wav_bytes: &[u8]) {
        self.stop_sound();

        let Some(handle) = &self.stream_handle else {
            return;
        };

        if wav_bytes.is_empty() {
            return;
        }

        let cursor = Cursor::new(wav_bytes.to_vec());
        match Decoder::new(cursor) {
            Ok(source) => match Sink::try_new(handle) {
                Ok(sink) => {
                    sink.append(source);
                    self.sfx_sink = Some(sink);
                }
                Err(e) => {
                    eprintln!("[AudioManager] Failed to create sound sink: {}", e);
                }
            },
            Err(e) => {
                eprintln!("[AudioManager] Failed to decode WAV audio: {}", e);
            }
        }
    }

    /// Plays a one-shot sound effect from a file path.
    pub fn play_sound_file<P: AsRef<Path>>(&mut self, path: P) {
        let path = path.as_ref();
        let path_str = path.to_string_lossy().to_string();

        if let Some(cached) = self.sound_cache.get(&path_str) {
            let bytes = cached.clone();
            self.play_sound_bytes(&bytes);
            return;
        }

        match std::fs::read(path) {
            Ok(bytes) => {
                self.play_sound_bytes(&bytes);
                self.sound_cache.insert(path_str, bytes);
            }
            Err(e) => {
                eprintln!("[AudioManager] Failed to read sound file {}: {}", path.display(), e);
            }
        }
    }

    /// Plays background music / narration (BGSOUND) from raw WAV bytes with loop count.
    ///
    /// - `loop_count == 0 || loop_count == u32::MAX`: Infinite loop.
    /// - `loop_count == 1`: Plays exactly once without looping.
    /// - `loop_count > 1`: Plays `loop_count` times then finishes.
    pub fn play_bgm_bytes(&mut self, wav_bytes: &[u8], loop_count: u32) {
        self.stop_bgm();

        let Some(handle) = &self.stream_handle else {
            return;
        };

        if wav_bytes.is_empty() {
            return;
        }

        let cursor = Cursor::new(wav_bytes.to_vec());
        match Decoder::new(cursor) {
            Ok(source) => match Sink::try_new(handle) {
                Ok(sink) => {
                    if loop_count == 0 || loop_count == u32::MAX {
                        sink.append(source.repeat_infinite());
                    } else {
                        sink.append(source);
                        for _ in 1..loop_count.min(50) {
                            if let Ok(extra) = Decoder::new(Cursor::new(wav_bytes.to_vec())) {
                                sink.append(extra);
                            }
                        }
                    }
                    self.bgm_sink = Some(sink);
                }
                Err(e) => {
                    eprintln!("[AudioManager] Failed to create BGM sink: {}", e);
                }
            },
            Err(e) => {
                eprintln!("[AudioManager] Failed to decode BGM WAV: {}", e);
            }
        }
    }

    /// Plays background music / narration (BGSOUND) from a file path with loop count.
    pub fn play_bgm_file<P: AsRef<Path>>(&mut self, path: P, loop_count: u32) {
        let path = path.as_ref();
        let path_str = path.to_string_lossy().to_string();

        if let Some(cached) = self.sound_cache.get(&path_str) {
            let bytes = cached.clone();
            self.play_bgm_bytes(&bytes, loop_count);
            return;
        }

        match std::fs::read(path) {
            Ok(bytes) => {
                self.play_bgm_bytes(&bytes, loop_count);
                self.sound_cache.insert(path_str, bytes);
            }
            Err(e) => {
                eprintln!("[AudioManager] Failed to read BGM file {}: {}", path.display(), e);
            }
        }
    }

    /// Plays background music in infinite loop.
    pub fn play_bgm_file_infinite<P: AsRef<Path>>(&mut self, path: P) {
        self.play_bgm_file(path, 0);
    }

    /// Stops currently playing background music.
    pub fn stop_bgm(&mut self) {
        if let Some(sink) = self.bgm_sink.take() {
            sink.stop();
        }
    }

    /// Stops currently playing sound effect.
    pub fn stop_sound(&mut self) {
        if let Some(sink) = self.sfx_sink.take() {
            sink.stop();
        }
    }

    /// Stops both background music and sound effects.
    pub fn stop_all(&mut self) {
        self.stop_bgm();
        self.stop_sound();
    }
}

impl Default for AudioManager {
    fn default() -> Self {
        Self::new()
    }
}
