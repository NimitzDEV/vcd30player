mod common;

use vcd30_player::video::{extract_mpeg_ps, MpegDecoder, VideoPlayState, VideoPlayer};

#[test]
fn test_cdxa_extraction_and_decoder_metadata() {
    let Some(root) = common::get_live_disc_root() else {
        eprintln!("No live CD-ROM mounted, skipping real video test.");
        return;
    };
    let dat_path = if root.join("MPEGAV").join("MUSIC01.DAT").exists() {
        root.join("MPEGAV").join("MUSIC01.DAT")
    } else if root.join("MPEGAV").join("AVSEQ01.DAT").exists() {
        root.join("MPEGAV").join("AVSEQ01.DAT")
    } else {
        eprintln!("DAT file not found in MPEGAV, skipping test.");
        return;
    };

    let raw_bytes = std::fs::read(&dat_path).expect("Failed to read DAT file");
    assert!(raw_bytes.len() > 100_000);

    // 1. Demux test
    let ps_stream = extract_mpeg_ps(&raw_bytes).expect("Failed to extract MPEG PS");
    assert!(!ps_stream.is_empty());
    assert_eq!(&ps_stream[0..4], &[0x00, 0x00, 0x01, 0xba], "Stream must begin with MPEG Pack Header");

    // 2. Decoder test
    let mut decoder = MpegDecoder::from_bytes(&raw_bytes).expect("Failed to create MpegDecoder");
    let (w, h) = decoder.dimensions();
    assert_eq!(w, 352, "VCD width must be 352");
    assert!(h == 288 || h == 240, "VCD height must be 288 (PAL) or 240 (NTSC), got {}", h);
    let is_pal = (decoder.framerate() - 25.0).abs() < 0.1;
    let is_ntsc = (decoder.framerate() - 29.97).abs() < 0.1 || (decoder.framerate() - 30.0).abs() < 0.1;
    assert!(is_pal || is_ntsc, "VCD framerate should be PAL (~25.0) or NTSC (~29.97), got {}", decoder.framerate());
    println!("Video metadata: {}x{}, fps={}, duration={}", w, h, decoder.framerate(), decoder.duration());
    assert!(decoder.duration() >= 0.0, "Duration should be non-negative");
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
    let (dat_path, dat_name) = if root.join("MPEGAV").join("MUSIC01.DAT").exists() {
        (root.join("MPEGAV").join("MUSIC01.DAT"), "MUSIC01.DAT")
    } else if root.join("MPEGAV").join("AVSEQ01.DAT").exists() {
        (root.join("MPEGAV").join("AVSEQ01.DAT"), "AVSEQ01.DAT")
    } else {
        eprintln!("DAT file not found in MPEGAV, skipping test.");
        return;
    };

    let raw_bytes = std::fs::read(&dat_path).expect("Failed to read DAT file");
    let mut player = VideoPlayer::new(
        &raw_bytes,
        dat_name.to_string(),
        None, // headless / silent mode
        0,
        150,
        Some("HOMEPAGE.CHM".to_string()),
    )
    .expect("Failed to create VideoPlayer");

    assert_eq!(player.state, VideoPlayState::Playing);
    assert!(
        player.dimensions() == (352, 288) || player.dimensions() == (352, 240),
        "Player dimensions must be PAL (352, 288) or NTSC (352, 240), got {:?}",
        player.dimensions()
    );
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

    // Forward Seek
    player.seek(2.5);
    assert!((player.current_time() - 2.5).abs() < 0.5);

    // Backward Seek (must be able to seek back to earlier timestamps)
    player.seek(0.8);
    assert!((player.current_time() - 0.8).abs() < 0.5, "Must be able to seek backward to 0.8s, got {}", player.current_time());

    // Seek all the way back to start (0.0s)
    player.seek(0.0);
    assert!((player.current_time() - 0.0).abs() < 0.5, "Must be able to seek backward to start (0.0s), got {}", player.current_time());

    // Frame buffer check
    let (pw, ph) = player.dimensions();
    assert_eq!(player.current_frame().len(), (pw * ph * 4) as usize);
}
