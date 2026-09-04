use std::path::Path;
use vcd30_player::audio::AudioManager;

#[test]
fn test_audio_manager_graceful_init() {
    let mut audio = AudioManager::new();
    println!("Audio available: {}", audio.is_audio_available());
    // Playing dummy or empty data should not panic
    audio.play_sound_bytes(&[]);
    audio.stop_bgm();
}

#[test]
fn test_audio_manager_play_disc_wav() {
    let wav_path = Path::new(r"I:\DATA\VCD_DATA\STAMP.WAV");
    if !wav_path.exists() {
        eprintln!("Disc I:\\ not mounted, skipping audio file playback test");
        return;
    }
    let mut audio = AudioManager::new();
    audio.play_sound_file(wav_path);
    audio.stop_bgm();
}
