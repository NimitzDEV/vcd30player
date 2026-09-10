//! Audio channel mode support (Stereo, Left Channel / Accompaniment, Right Channel / Vocals).

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AudioChannelMode {
    #[default]
    Stereo, // 立体声 (全声道原声)
    LeftOnly, // 左声道 (伴奏音轨)
    RightOnly, // 右声道 (原唱音轨)
}

impl AudioChannelMode {
    /// Cycles through Stereo -> LeftOnly -> RightOnly -> Stereo.
    pub fn cycle(&self) -> Self {
        match self {
            AudioChannelMode::Stereo => AudioChannelMode::LeftOnly,
            AudioChannelMode::LeftOnly => AudioChannelMode::RightOnly,
            AudioChannelMode::RightOnly => AudioChannelMode::Stereo,
        }
    }

    /// Short label displayed on UI buttons.
    pub fn label(&self) -> &'static str {
        match self {
            AudioChannelMode::Stereo => "🔊 立体声",
            AudioChannelMode::LeftOnly => "🎤 左声道",
            AudioChannelMode::RightOnly => "🎵 右声道",
        }
    }

    /// Icon character displayed on compact UI buttons.
    pub fn icon(&self) -> &'static str {
        match self {
            AudioChannelMode::Stereo => "🔊",
            AudioChannelMode::LeftOnly => "🎤",
            AudioChannelMode::RightOnly => "🎵",
        }
    }

    /// Human-readable description with classic VCD context (Accompaniment vs. Vocals).
    pub fn description(&self) -> &'static str {
        match self {
            AudioChannelMode::Stereo => "立体声 (双声道原声)",
            AudioChannelMode::LeftOnly => "左声道 (伴奏音轨)",
            AudioChannelMode::RightOnly => "右声道 (原唱音轨)",
        }
    }

    /// In-place channel mixing on interleaved 2-channel samples [L0, R0, L1, R1, ...].
    /// - Stereo: no-op
    /// - LeftOnly: duplicate Left channel to Right channel
    /// - RightOnly: duplicate Right channel to Left channel
    pub fn apply_to_interleaved_samples<T: Copy>(&self, samples: &mut [T]) {
        match self {
            AudioChannelMode::Stereo => {}
            AudioChannelMode::LeftOnly => {
                for pair in samples.chunks_exact_mut(2) {
                    pair[1] = pair[0];
                }
            }
            AudioChannelMode::RightOnly => {
                for pair in samples.chunks_exact_mut(2) {
                    pair[0] = pair[1];
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_channel_cycle() {
        assert_eq!(AudioChannelMode::Stereo.cycle(), AudioChannelMode::LeftOnly);
        assert_eq!(AudioChannelMode::LeftOnly.cycle(), AudioChannelMode::RightOnly);
        assert_eq!(AudioChannelMode::RightOnly.cycle(), AudioChannelMode::Stereo);
    }

    #[test]
    fn test_apply_stereo() {
        let mut samples = vec![1.0f32, 2.0f32, 3.0f32, 4.0f32];
        AudioChannelMode::Stereo.apply_to_interleaved_samples(&mut samples);
        assert_eq!(samples, vec![1.0, 2.0, 3.0, 4.0]);
    }

    #[test]
    fn test_apply_left_only() {
        let mut samples = vec![1.0f32, 2.0f32, 3.0f32, 4.0f32];
        AudioChannelMode::LeftOnly.apply_to_interleaved_samples(&mut samples);
        assert_eq!(samples, vec![1.0, 1.0, 3.0, 3.0]);
    }

    #[test]
    fn test_apply_right_only() {
        let mut samples = vec![1.0f32, 2.0f32, 3.0f32, 4.0f32];
        AudioChannelMode::RightOnly.apply_to_interleaved_samples(&mut samples);
        assert_eq!(samples, vec![2.0, 2.0, 4.0, 4.0]);
    }
}
