mod common;

use vcd30_player::video::{extract_mpeg_ps, MpegDecoder, VideoPlayState, VideoPlayer};

#[test]
fn test_cdxa_extraction_and_decoder_metadata() {
    let Some(root) = common::get_live_disc_root() else {
        eprintln!("No live CD-ROM mounted, skipping real video test.");
        return;
    };
    let dat_path = root.join("MPEGAV").join("MUSIC01.DAT");
    if !dat_path.exists() {
        eprintln!("Disc not found at {}, skipping test.", dat_path.display());
        return;
    }

    let raw_bytes = std::fs::read(&dat_path).expect("Failed to read MUSIC01.DAT");
    assert!(raw_bytes.len() > 100_000);

    // 1. Demux test
    let ps_stream = extract_mpeg_ps(&raw_bytes).expect("Failed to extract MPEG PS");
    assert!(!ps_stream.is_empty());
    assert_eq!(&ps_stream[0..4], &[0x00, 0x00, 0x01, 0xba], "Stream must begin with MPEG Pack Header");

    // 2. Decoder test
    let mut decoder = MpegDecoder::from_bytes(&raw_bytes).expect("Failed to create MpegDecoder");
    let (w, h) = decoder.dimensions();
    assert_eq!(w, 352, "VCD width must be 352");
    assert_eq!(h, 288, "VCD height must be 288");
    assert!((decoder.framerate() - 25.0).abs() < 0.1, "VCD PAL framerate should be ~25.0");
    assert!(decoder.duration() > 10.0, "Duration should be positive");
    assert!(decoder.has_video());
    assert!(decoder.has_audio());

    // 3. Frame decoding test
    let mut rgba = vec![0u8; (w * h * 4) as usize];
    let pts = decoder.decode_video_frame(&mut rgba);
    assert!(pts.is_some(), "First video frame should decode successfully");
    // Verify frame contains non-black content (not all zeros)
    let non_zero_count = rgba.iter().filter(|&&b| b > 0).count();
    assert!(non_zero_count > 1000, "Decoded frame should contain visible pixel data");
    // Verify alpha is 255 for all pixels (opaque)
    assert!(
        rgba.chunks_exact(4).all(|p| p[3] == 255),
        "Decoded video frame must have alpha = 255 (fully opaque)"
    );


    // 4. Audio decoding test
    let audio = decoder.decode_audio_samples();
    assert!(audio.is_some(), "Audio sample packet should decode successfully");
    let audio_buf = audio.unwrap();
    assert!(audio_buf.count > 0, "Decoded audio sample count must be positive");
    assert_eq!(audio_buf.samples.len(), audio_buf.count * 2);
}

#[test]
fn test_video_player_playback_and_seek() {
    let Some(root) = common::get_live_disc_root() else {
        eprintln!("No live CD-ROM mounted, skipping real video test.");
        return;
    };
    let dat_path = root.join("MPEGAV").join("MUSIC01.DAT");
    if !dat_path.exists() {
        eprintln!("Disc not found at {}, skipping test.", dat_path.display());
        return;
    }

    let raw_bytes = std::fs::read(&dat_path).expect("Failed to read MUSIC01.DAT");
    let mut player = VideoPlayer::new(
        &raw_bytes,
        "MUSIC01.DAT".to_string(),
        None, // headless / silent mode
        0,
        150,
        Some("HOMEPAGE.CHM".to_string()),
    )
    .expect("Failed to create VideoPlayer");

    assert_eq!(player.state, VideoPlayState::Playing);
    assert_eq!(player.dimensions(), (352, 288));
    assert_eq!(player.exit_target.as_deref(), Some("HOMEPAGE.CHM"));

    // Update
    let _ = player.update();
    assert_eq!(player.state, VideoPlayState::Playing);

    // Pause / Resume
    player.toggle_play_pause();
    assert_eq!(player.state, VideoPlayState::Paused);
    assert!(player.is_paused());

    player.toggle_play_pause();
    assert_eq!(player.state, VideoPlayState::Playing);
    assert!(player.is_playing());

    // Seek
    player.seek(2.5);
    assert!((player.current_time() - 2.5).abs() < 0.5);

    // Frame buffer check
    assert_eq!(player.current_frame().len(), 352 * 288 * 4);
}
