mod common;

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
    let wav_path = common::get_test_disc_root()
        .join("DATA")
        .join("VCD_DATA")
        .join("STAMP.WAV");
    if !wav_path.exists() {
        eprintln!("STAMP.WAV not found, skipping audio file playback test");
        return;
    }
    let mut audio = AudioManager::new();
    audio.play_sound_file(&wav_path);
    audio.stop_bgm();
}

#[test]
fn test_inspect_yt02_chm() {
    use vcd30_player::assets::chm::CompHtmlDoc;
    let live_root = common::get_live_disc_root();
    let Some(root) = live_root else {
        println!("No live disc mounted, skipping live YT02.CHM test");
        return;
    };
    let path = root.join("DATA").join("VCD_DATA").join("YT02.CHM");
    if !path.exists() {
        println!("YT02.CHM not found on disc, skipping");
        return;
    }
    let bytes = std::fs::read(&path).unwrap();
    let doc = CompHtmlDoc::parse(&bytes).unwrap();
    println!("=== YT02.CHM ===");
    println!("Title: {}", doc.title);
    println!("Width: {}, Height: {}", doc.width, doc.height);
    println!("Chunks count: {}", doc.chunks.len());

    let m02_path = root.join("DATA").join("VCD_DATA").join("M02.WAV");
    if m02_path.exists() {
        let b = std::fs::read(m02_path).unwrap();
        let cursor = std::io::Cursor::new(b);
        let dec = rodio::Decoder::new(cursor).unwrap();
        use rodio::Source;
        println!("M02.WAV: channels={}, sample_rate={}, duration={:?}", dec.channels(), dec.sample_rate(), dec.total_duration());
    }

    let pipa_path = root.join("DATA").join("VCD_DATA").join("PIPA.WAV");
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
    let live_root = common::get_live_disc_root();
    let Some(root) = live_root else {
        println!("No live disc mounted, skipping live YT02 audio test");
        return;
    };

    let yt02_file = root.join("DATA").join("VCD_DATA").join("YT02.CHM");
    if !yt02_file.exists() {
        println!("YT02.CHM not found, skipping");
        return;
    }

    let mut kernel = VcdKernel::new();
    kernel.open_disc(root.clone()).unwrap();

    // 1. Navigate to YT02.CHM
    kernel.load_page("YT02.CHM", true).unwrap();
    assert_eq!(kernel.current_page_name, "YT02.CHM");

    // BGSOUND should be active with loop_count = 1
    assert!(kernel.audio.is_bgm_playing());

    // 2. Click PIPA.WAV hotspot
    let pipa_area = kernel
        .current_page
        .as_ref()
        .unwrap()
        .get_all_hotspots()
        .into_iter()
        .find(|a| a.target == "PIPA.WAV")
        .expect("PIPA.WAV hotspot should exist")
        .clone();

    let activated = kernel.activate_hotspot(&pipa_area).unwrap();
    assert!(!activated);

    // BGM should be stopped and PIPA.WAV played
    assert!(!kernel.audio.is_bgm_playing());

    // Clean up
    kernel.audio.stop_all();
}

#[test]
fn test_weight_chm_audio_timing_and_intro_delay() {
    let disc_root = common::get_test_disc_root();
    let data_dir = disc_root.join("DATA").join("VCD_DATA");
    if !data_dir.exists() {
        return;
    }

    use rodio::Source;
    let w99_path = data_dir.join("W99.WAV");
    if w99_path.exists() {
        let b = std::fs::read(&w99_path).unwrap();
        let dec = rodio::Decoder::new(std::io::Cursor::new(b)).unwrap();
        let w99_duration = dec.total_duration().unwrap();
        // W99.WAV is ~2.185 seconds
        assert!(w99_duration.as_millis() >= 2100 && w99_duration.as_millis() <= 2300);
    }

    let weight_chm_path = data_dir.join("WEIGHT.CHM");
    if weight_chm_path.exists() {
        use vcd30_player::core::kernel::VcdKernel;
        use vcd30_player::core::script_vm::VmState;

        let mut kernel = VcdKernel::new();
        kernel.open_disc(disc_root).unwrap();
        kernel.load_page("WEIGHT.CHM", false).unwrap();

        // 1. Initially yields WaitingForDelay for 30 units (3000ms)
        assert!(matches!(kernel.vm.state, VmState::WaitingForDelay { .. }));

        // 2. Fast forward 2.3 seconds (greater than W99.WAV 2.185s duration, but under 3.0s intro delay)
        // With the 100ms/unit fix, VM MUST STILL BE IN WaitingForDelay (ensures W99.WAV finishes completely!)
        kernel.start_time = std::time::Instant::now() - std::time::Duration::from_millis(2300);
        let state_mid = kernel.run_vm();
        assert!(
            matches!(state_mid, VmState::WaitingForDelay { .. }),
            "At 2.3s, W99.WAV has finished playing and VM should still wait for 3.0s delay to complete!"
        );

        // 3. Fast forward past 3.0 seconds (e.g. 3.5s) -> VM should advance to gender prompt
        kernel.start_time = std::time::Instant::now() - std::time::Duration::from_millis(3500);
        let state_end = kernel.run_vm();
        assert!(
            matches!(
                state_end,
                VmState::WaitingForKey { target_var: b'X' }
                    | VmState::WaitingForKeyWithTimeout { target_var: b'X', .. }
            ),
            "At 3.5s, delay completed, W08.WAV triggered, waiting for gender selection (Key X)!"
        );
    }
}
