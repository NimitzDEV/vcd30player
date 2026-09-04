//! Audio manager implementation using rodio with graceful fallback for headless/silent systems.

use rodio::{Decoder, OutputStream, OutputStreamHandle, Sink, Source};
use std::collections::HashMap;
use std::io::Cursor;
use std::path::Path;

pub struct AudioManager {
    _stream: Option<OutputStream>,
    stream_handle: Option<OutputStreamHandle>,
    bgm_sink: Option<Sink>,
    sound_cache: HashMap<String, Vec<u8>>,
}

impl AudioManager {
    pub fn new() -> Self {
        match OutputStream::try_default() {
            Ok((stream, handle)) => Self {
                _stream: Some(stream),
                stream_handle: Some(handle),
                bgm_sink: None,
                sound_cache: HashMap::new(),
            },
            Err(e) => {
                eprintln!("[AudioManager] Warning: Audio output device unavailable (falling back to silent mode): {}", e);
                Self {
                    _stream: None,
                    stream_handle: None,
                    bgm_sink: None,
                    sound_cache: HashMap::new(),
                }
            }
        }
    }

    pub fn is_audio_available(&self) -> bool {
        self.stream_handle.is_some()
    }

    /// Creates a dedicated Sink for video MP2 audio playback.
    pub fn create_video_sink(&self) -> Option<Sink> {
        let handle = self.stream_handle.as_ref()?;
        Sink::try_new(handle).ok()
    }

    /// Plays a one-shot sound effect asynchronously from raw WAV bytes.
    pub fn play_sound_bytes(&mut self, wav_bytes: &[u8]) {
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
                    sink.detach();
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

    /// Plays looping background music (BGSOUND) from raw WAV bytes.
    pub fn play_bgm_bytes(&mut self, wav_bytes: &[u8]) {
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
                    sink.append(source.repeat_infinite());
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

    /// Plays looping background music (BGSOUND) from a file path.
    pub fn play_bgm_file<P: AsRef<Path>>(&mut self, path: P) {
        let path = path.as_ref();
        match std::fs::read(path) {
            Ok(bytes) => self.play_bgm_bytes(&bytes),
            Err(e) => {
                eprintln!("[AudioManager] Failed to read BGM file {}: {}", path.display(), e);
            }
        }
    }

    /// Stops currently playing background music.
    pub fn stop_bgm(&mut self) {
        if let Some(sink) = self.bgm_sink.take() {
            sink.stop();
        }
    }
}

impl Default for AudioManager {
    fn default() -> Self {
        Self::new()
    }
}
