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

#[test]
fn test_inspect_yt02_chm() {
    use vcd30_player::assets::chm::CompHtmlDoc;
    let paths = [
        r"J:\DATA\VCD_DATA\YT02.CHM",
        r"I:\DATA\VCD_DATA\YT02.CHM",
    ];
    let mut found_path = None;
    for p in &paths {
        if Path::new(p).exists() {
            found_path = Some(*p);
            break;
        }
    }
    let Some(path) = found_path else {
        println!("YT02.CHM not found, skipping");
        return;
    };
    let bytes = std::fs::read(path).unwrap();
    let doc = CompHtmlDoc::parse(&bytes).unwrap();
    println!("=== YT02.CHM ===");
    println!("Title: {}", doc.title);
    println!("Width: {}, Height: {}", doc.width, doc.height);
    println!("Chunks count: {}", doc.chunks.len());

    let m02_path = Path::new(r"J:\DATA\VCD_DATA\M02.WAV");
    if m02_path.exists() {
        let b = std::fs::read(m02_path).unwrap();
        let cursor = std::io::Cursor::new(b);
        let dec = rodio::Decoder::new(cursor).unwrap();
        use rodio::Source;
        println!("M02.WAV: channels={}, sample_rate={}, duration={:?}", dec.channels(), dec.sample_rate(), dec.total_duration());
    }

    let pipa_path = Path::new(r"J:\DATA\VCD_DATA\PIPA.WAV");
    if pipa_path.exists() {
        let b = std::fs::read(pipa_path).unwrap();
        let cursor = std::io::Cursor::new(b);
        let dec = rodio::Decoder::new(cursor).unwrap();
        use rodio::Source;
        println!("PIPA.WAV: channels={}, sample_rate={}, duration={:?}", dec.channels(), dec.sample_rate(), dec.total_duration());
    }

    assert_eq!(doc.title, "YuWen");
    assert_eq!(doc.width, 352);
    assert_eq!(doc.height, 288);
    assert_eq!(doc.get_background_sound_info(), Some(("M02.WAV", 1)));

    let hotspots = doc.get_all_hotspots();
    assert_eq!(hotspots.len(), 2);
    assert!(hotspots.iter().any(|a| a.target == "PIPA.WAV"));
    assert!(hotspots.iter().any(|a| a.target == "YQ01.CHM"));
}




#[test]
fn test_yt02_chm_audio_and_wav_hotspot_triggering() {
    use vcd30_player::core::kernel::VcdKernel;
    let disc_candidates = [
        std::path::PathBuf::from(r"J:\"),
        std::path::PathBuf::from(r"I:\"),
    ];
    let disc_root = disc_candidates.into_iter().find(|p| p.exists());
    let Some(root) = disc_root else {
        println!("No test disc mounted, skipping disc-dependent test");
        return;
    };

    let yt02_file = root.join(r"DATA\VCD_DATA\YT02.CHM");
    if !yt02_file.exists() {
        println!("YT02.CHM not found, skipping");
        return;
    }

    let mut kernel = VcdKernel::new();
    kernel.open_disc(root).expect("Failed to open disc");
    kernel.load_page("YT02.CHM", false).expect("Failed to load YT02.CHM");

    // 1. Verify BGSOUND is parsed with loop_count = 1 and filename = "M02.WAV"
    let doc = kernel.current_page.as_ref().unwrap();
    let bg_info = doc.get_background_sound_info();
    assert_eq!(bg_info, Some(("M02.WAV", 1)));

    // 2. If audio output device is available, BGM should be active
    if kernel.audio.is_audio_available() {
        assert!(kernel.audio.is_bgm_playing());
    }

    // 3. Find the PIPA.WAV hotspot
    let all_hotspots = doc.get_all_hotspots();
    let pipa_hotspot = all_hotspots
        .into_iter()
        .find(|a| a.target.to_uppercase().ends_with("PIPA.WAV"))
        .expect("PIPA.WAV hotspot not found in YT02.CHM");

    // Verify raw bounds
    assert_eq!(pipa_hotspot.raw_bounds(), (22, 30, 106, 260));

    // 4. Hit test at center of pipa hotspot (50, 100)
    let hit_area = kernel.hit_test(50, 100).cloned().expect("Hit test should find PIPA.WAV hotspot");
    assert_eq!(hit_area.target, "PIPA.WAV");

    // 5. Activate the hotspot
    let activated = kernel.activate_hotspot(&hit_area).expect("Failed to activate hotspot");
    assert!(activated, "WAV hotspot must return Ok(true)");

    // 6. Verify BGM is stopped and SFX is playing
    if kernel.audio.is_audio_available() {
        assert!(!kernel.audio.is_bgm_playing(), "BGM must be stopped when user activates WAV sample");
        assert!(kernel.audio.is_sound_playing(), "SFX must be playing PIPA.WAV");
    }

    // 7. Test re-triggering while playing
    let re_activated = kernel.activate_hotspot(&hit_area).expect("Reactivation failed");
    assert!(re_activated);
    if kernel.audio.is_audio_available() {
        assert!(kernel.audio.is_sound_playing());
    }

    // Clean up
    kernel.audio.stop_all();
}





