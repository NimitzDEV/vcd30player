mod common;

use vcd30_player::core::kernel::{ActiveDiscMode, PlaybackMode, CANVAS_HEIGHT, CANVAS_WIDTH, VcdKernel};
use vcd30_player::audio::AudioChannelMode;
use vcd30_player::ui::app::DrawerTab;

#[test]
fn test_kernel_open_disc_and_autorun() {
    let disc_path = common::get_test_disc_root();
    let mut kernel = VcdKernel::new();
    kernel.open_disc(disc_path).expect("Failed to open disc");

    assert_eq!(kernel.current_page_name, "HOMEPAGE.CHM");
    assert!(kernel.current_page.is_some());
    assert!(kernel.current_bg_image.is_some());

    // Canvas must be 352x288x4 RGBA bytes
    assert_eq!(
        kernel.canvas.len(),
        (CANVAS_WIDTH * CANVAS_HEIGHT * 4) as usize
    );

    // Canvas should contain non-zero pixels (from HOMEPAGE.YBM)
    let non_zero_count = kernel.canvas.iter().filter(|&&b| b > 0).count();
    assert!(
        non_zero_count > 1000,
        "Canvas should be populated with image pixels"
    );

    // Verify live disc if mounted
    if let Some(live_root) = common::get_live_disc_root() {
        let mut live_kernel = VcdKernel::new();
        if live_kernel.open_disc(live_root).is_ok() {
            let cls_has_video = live_kernel
                .autorun_config
                .as_ref()
                .and_then(|c| c.opening_mpeg.as_ref())
                .is_some();
            assert_eq!(
                live_kernel.is_video_active(),
                cls_has_video,
                "Kernel video active state must match whether AUTORUN.CLS specifies an opening video"
            );
            assert_eq!(live_kernel.current_page_name, "HOMEPAGE.CHM");
        }
    }
}

#[test]
fn test_kernel_hotspot_hit_testing_and_navigation() {
    let disc_path = common::get_test_disc_root();
    let mut kernel = VcdKernel::new();
    kernel.open_disc(disc_path).unwrap();

    // In HOMEPAGE.CHM, the full screen area (5, 5) to (350, 285) routes to HOME.CHM
    let center_hit = kernel.hit_test(176, 144);
    assert!(
        center_hit.is_some(),
        "Center of HOMEPAGE should hit the navigation hotspot"
    );

    let area = center_hit.unwrap().clone();
    assert!(
        area.target.contains("HOME.CHM"),
        "Target should route to HOME.CHM"
    );

    // Outside hotspot (0, 0)
    let corner_hit = kernel.hit_test(0, 0);
    assert!(
        corner_hit.is_none(),
        "Point (0, 0) should be outside (5, 5) hotspot box"
    );

    // Click hotspot and navigate to HOME.CHM
    let navigated = kernel
        .activate_hotspot(&area)
        .expect("Failed to activate hotspot");
    assert!(navigated);
    assert_eq!(kernel.current_page_name, "HOME.CHM");
    assert_eq!(kernel.history_stack, vec!["HOMEPAGE.CHM"]);

    // Test Back navigation
    let back_ok = kernel.go_back().expect("Failed to go back");
    assert!(back_ok);
    assert_eq!(kernel.current_page_name, "HOMEPAGE.CHM");
    assert_eq!(kernel.forward_stack, vec!["HOME.CHM"]);

    // Test Forward navigation
    let fwd_ok = kernel.go_forward().expect("Failed to go forward");
    assert!(fwd_ok);
    assert_eq!(kernel.current_page_name, "HOME.CHM");
}

#[test]
fn test_kernel_navigation_to_subpages() {
    let disc_path = common::get_test_disc_root();
    let mut kernel = VcdKernel::new();
    kernel.open_disc(disc_path).unwrap();

    // Navigate directly to T_B.CHM (teaching page)
    kernel
        .load_page("T_B.CHM", true)
        .expect("Failed to load T_B.CHM");
    assert_eq!(kernel.current_page_name, "T_B.CHM");
    assert_eq!(kernel.history_stack, vec!["HOMEPAGE.CHM"]);

    // In T_B.CHM, verify multiple button hotspots exist
    let hotspots = kernel.current_page.as_ref().unwrap().get_all_hotspots();
    assert!(
        hotspots.len() >= 4,
        "T_B.CHM should have multiple interactive button hotspots"
    );

    // Check next page button at (255, 255) -> T_C.CHM
    let hit_next = kernel.hit_test(260, 255);
    assert!(
        hit_next.is_some(),
        "Should hit Next Page button at (260, 255)"
    );
    let next_area = hit_next.unwrap().clone();
    assert_eq!(next_area.target, "T_C.CHM");

    // Click Next Page -> routes to T_C.CHM
    kernel.activate_hotspot(&next_area).unwrap();
    assert_eq!(kernel.current_page_name, "T_C.CHM");

    // Home navigation returns to main menu
    kernel.go_home().expect("Failed to go home");
    assert_eq!(kernel.current_page_name, "HOME.CHM");
}

#[test]
fn test_ok_help1_polygon_hotspot_navigation() {
    let disc_path = common::get_test_disc_root();
    let mut kernel = VcdKernel::new();
    kernel.open_disc(disc_path).unwrap();

    // Load OK_HELP1.CHM
    kernel.load_page("OK_HELP1.CHM", true).unwrap();
    assert_eq!(kernel.current_page_name, "OK_HELP1.CHM");

    // Check hotspots on OK_HELP1.CHM
    let hotspots = kernel.current_page.as_ref().unwrap().get_all_hotspots();
    assert_eq!(hotspots.len(), 1, "OK_HELP1.CHM should have 1 hotspot");
    let area = &hotspots[0];
    assert_eq!(
        area.target, "OK_HELP2.CHM",
        "Target link of polygon hotspot in OK_HELP1.CHM must be OK_HELP2.CHM"
    );

    // Hit test at bottom-right corner triangle button:
    // Vertices are (311, 253), (328, 253), (320, 266)
    let hit = kernel.hit_test(320, 260);
    assert!(hit.is_some(), "Should hit bottom-right corner button at (320, 260)");
    let hit_area = hit.unwrap().clone();
    assert_eq!(hit_area.target, "OK_HELP2.CHM");

    // Click the button -> navigates to OK_HELP2.CHM
    let activated = kernel.activate_hotspot(&hit_area).unwrap();
    assert!(activated, "Clicking hotspot should successfully trigger navigation");
    assert_eq!(kernel.current_page_name, "OK_HELP2.CHM");

    // In OK_HELP2.CHM, clicking bottom-right button navigates to HOME.CHM
    let hit2 = kernel.hit_test(320, 260);
    assert!(hit2.is_some(), "Should hit bottom-right button in OK_HELP2.CHM");
    let hit_area2 = hit2.unwrap().clone();
    assert_eq!(hit_area2.target, "HOME.CHM");

    let activated2 = kernel.activate_hotspot(&hit_area2).unwrap();
    assert!(activated2);
    assert_eq!(kernel.current_page_name, "HOME.CHM");
}

#[test]
fn test_psyche_dense_hotspots_no_overlap() {
    let disc_path = common::get_test_disc_root();
    let mut kernel = VcdKernel::new();
    kernel.open_disc(disc_path).unwrap();

    // Load PSYCHE.CHM
    kernel.load_page("PSYCHE.CHM", true).unwrap();
    assert_eq!(kernel.current_page_name, "PSYCHE.CHM");

    // Option 1: Y in 114..127
    let hit1 = kernel.hit_test(60, 120).expect("Should hit Option 1");
    assert_eq!(hit1.script_entry_line, Some(100));

    // Option 2: Y in 127..140 (crucial test: must hit Option 2, NOT Option 1!)
    let hit2 = kernel.hit_test(60, 133).expect("Should hit Option 2");
    assert_eq!(
        hit2.script_entry_line,
        Some(150),
        "Clicking Option 2 must hit line 150 without being shadowed by Option 1"
    );

    // Option 3: Y in 140..153 (crucial test: must hit Option 3, NOT Option 2!)
    let hit3 = kernel.hit_test(60, 146).expect("Should hit Option 3");
    assert_eq!(
        hit3.script_entry_line,
        Some(180),
        "Clicking Option 3 must hit line 180 without being shadowed by Option 2"
    );

    // Option 4: Y in 204..215
    let hit4 = kernel.hit_test(60, 210).expect("Should hit Option 4");
    assert_eq!(hit4.script_entry_line, Some(200));

    // Option 5: Y in 216..227
    let hit5 = kernel.hit_test(60, 222).expect("Should hit Option 5");
    assert_eq!(hit5.script_entry_line, Some(250));

    // Option 6: Y in 229..241
    let hit6 = kernel.hit_test(60, 235).expect("Should hit Option 6");
    assert_eq!(hit6.script_entry_line, Some(280));
}

#[test]
fn test_ui_about_dialog_state() {
    let kernel = VcdKernel::new();
    let mut app = vcd30_player::ui::app::VcdPlayerApp::from_kernel(kernel);
    assert!(!app.show_about, "About dialog must be initially closed");
    app.show_about = true;
    assert!(app.show_about, "About dialog state must be toggleable");
}

#[test]
fn test_ui_status_duration_and_no_path() {
    assert_eq!(
        vcd30_player::ui::app::STATUS_MESSAGE_DURATION,
        std::time::Duration::from_secs(5),
        "Status message duration must be 5 seconds"
    );

    let disc_path = common::get_test_disc_root();
    let mut kernel = VcdKernel::new();
    kernel.open_disc(disc_path.clone()).expect("Failed to open disc");

    let mut app = vcd30_player::ui::app::VcdPlayerApp::from_kernel(kernel);
    // Reset disc using headless egui Context
    let ctx = vcd30_player::egui::Context::default();
    app.reset_disc(&ctx);

    // The status message must NOT contain file path like 'C:\' or 'mock_disc'
    let status_str = &app.status_message;
    assert!(
        status_str.contains("从头开始") || status_str.starts_with("已重置"),
        "Status must indicate reset/restart, got: {}",
        status_str
    );
    assert!(
        !status_str.contains(":\\") && !status_str.contains("mock_disc") && !status_str.contains("/"),
        "Status message must not contain disc paths, got: {}",
        status_str
    );
    assert!(
        status_str.contains("VCD 3.0"),
        "Status message must contain disc type, got: {}",
        status_str
    );
}

#[test]
fn test_audio_channel_mode_cycle_and_apply() {
    let mode = AudioChannelMode::Stereo;
    assert_eq!(mode.label(), "🔊 立体声");
    assert_eq!(mode.icon(), "🔊");
    assert_eq!(mode.cycle(), AudioChannelMode::LeftOnly);
    assert_eq!(mode.cycle().icon(), "🎤");
    assert_eq!(mode.cycle().cycle(), AudioChannelMode::RightOnly);
    assert_eq!(mode.cycle().cycle().icon(), "🎵");
    assert_eq!(mode.cycle().cycle().cycle(), AudioChannelMode::Stereo);

    // Verify audio channel mixing on interleaved samples
    let mut samples = vec![100i16, 200i16, 300i16, 400i16];
    AudioChannelMode::LeftOnly.apply_to_interleaved_samples(&mut samples);
    assert_eq!(samples, vec![100, 100, 300, 300], "Left channel must be copied to right channel");

    let mut samples2 = vec![100i16, 200i16, 300i16, 400i16];
    AudioChannelMode::RightOnly.apply_to_interleaved_samples(&mut samples2);
    assert_eq!(samples2, vec![200, 200, 400, 400], "Right channel must be copied to left channel");
}

#[test]
fn test_playback_mode_cycle() {
    let mode = PlaybackMode::Sequential;
    assert_eq!(mode.label(), "➡ 顺序播放");
    assert_eq!(mode.icon(), "➡");
    assert_eq!(mode.cycle(), PlaybackMode::ListRepeat);
    assert_eq!(mode.cycle().label(), "🔁 列表循环");
    assert_eq!(mode.cycle().icon(), "🔁");
    assert_eq!(mode.cycle().cycle(), PlaybackMode::SingleRepeat);
    assert_eq!(mode.cycle().cycle().label(), "🔂 单曲循环");
    assert_eq!(mode.cycle().cycle().icon(), "🔂");
    assert_eq!(mode.cycle().cycle().cycle(), PlaybackMode::Sequential);
}

#[test]
fn test_active_disc_mode_switching_and_track_navigation() {
    let disc_path = common::get_test_disc_root();
    let mut kernel = VcdKernel::new();
    kernel.open_disc(disc_path).expect("Failed to open disc");

    // Initially loads in VCD 3.0 Interactive mode
    assert_eq!(kernel.active_mode, ActiveDiscMode::Vcd30Interactive);
    assert!(!kernel.tracks.is_empty(), "Disc tracks must be populated");
    let track_count = kernel.tracks.len();

    // Switch to VCD 2.0 Classic mode
    kernel.switch_active_mode(ActiveDiscMode::Vcd20Classic).expect("Failed to switch to VCD 2.0");
    assert_eq!(kernel.active_mode, ActiveDiscMode::Vcd20Classic);
    assert!(kernel.current_page.is_none(), "CHM page should be cleared in VCD 2.0 mode");
    assert_eq!(kernel.current_track_index, Some(0), "Track 0 should be active in VCD 2.0 mode");
    assert!(kernel.active_video.is_some(), "Video should be active playing track 0");

    // Test next track navigation
    let has_next = kernel.play_next_track(false).expect("Failed to play next track");
    if track_count > 1 {
        assert!(has_next);
        assert_eq!(kernel.current_track_index, Some(1));
    }

    // Test prev track navigation
    let has_prev = kernel.play_prev_track().expect("Failed to play prev track");
    assert!(has_prev);
    assert_eq!(kernel.current_track_index, Some(0));

    // Switch back to VCD 3.0 Interactive mode
    kernel.switch_active_mode(ActiveDiscMode::Vcd30Interactive).expect("Failed to restore VCD 3.0");
    assert_eq!(kernel.active_mode, ActiveDiscMode::Vcd30Interactive);
    assert_eq!(kernel.current_page_name, "HOMEPAGE.CHM");
    assert!(kernel.current_page.is_some());
}

#[test]
fn test_kernel_restart_current_mode_preserves_mode() {
    let disc_path = common::get_test_disc_root();
    let mut kernel = VcdKernel::new();
    kernel.open_disc(disc_path).expect("Failed to open disc");

    // 1. In VCD 3.0 Interactive mode: navigate to subpage then restart
    kernel.load_page("T_B.CHM", true).unwrap();
    assert_eq!(kernel.current_page_name, "T_B.CHM");
    kernel.restart_current_mode().expect("Failed to restart VCD 3.0 mode");
    assert_eq!(kernel.active_mode, ActiveDiscMode::Vcd30Interactive, "Mode must remain VCD 3.0");
    assert_eq!(kernel.current_page_name, "HOMEPAGE.CHM", "Must reset to HOMEPAGE.CHM");

    // 2. In VCD 2.0 Classic mode: advance track then restart
    kernel.switch_active_mode(ActiveDiscMode::Vcd20Classic).unwrap();
    assert_eq!(kernel.active_mode, ActiveDiscMode::Vcd20Classic);
    if kernel.tracks.len() > 1 {
        let _ = kernel.play_next_track(false);
        assert_eq!(kernel.current_track_index, Some(1));
    }
    kernel.restart_current_mode().expect("Failed to restart VCD 2.0 mode");
    assert_eq!(kernel.active_mode, ActiveDiscMode::Vcd20Classic, "Mode must remain VCD 2.0");
    assert_eq!(kernel.current_track_index, Some(0), "Track must reset to track 0");
}

#[test]
fn test_ui_drawer_tab_selection() {
    let kernel = VcdKernel::new();
    let mut app = vcd30_player::ui::app::VcdPlayerApp::from_kernel(kernel);
    assert_eq!(app.drawer_tab, DrawerTab::Remote, "Drawer should default to Remote tab");

    app.drawer_tab = DrawerTab::Tracks;
    assert_eq!(app.drawer_tab, DrawerTab::Tracks, "Drawer tab should be switchable to Tracks");
}

#[test]
fn test_navigation_disabled_in_non_vcd30_mode() {
    let disc_path = common::get_test_disc_root();
    let mut kernel = VcdKernel::new();
    kernel.open_disc(disc_path).expect("Failed to open disc");

    // Switch to VCD 2.0 Classic mode
    kernel.switch_active_mode(ActiveDiscMode::Vcd20Classic).unwrap();
    assert_eq!(kernel.active_mode, ActiveDiscMode::Vcd20Classic);
    assert!(kernel.current_page.is_none());

    // In VCD 2.0 mode, go_home, go_back, go_forward must not load any CHM or trigger interactive VM
    kernel.go_home().expect("go_home in VCD 2.0 should be safe no-op");
    assert!(kernel.current_page.is_none(), "CHM page must not be loaded in VCD 2.0 mode");
    assert!(kernel.current_page_name.is_empty());
    assert!(!kernel.is_vm_active(), "VM must not be activated in VCD 2.0 mode");

    let back_res = kernel.go_back().unwrap();
    assert!(!back_res, "go_back must return false in VCD 2.0 mode");

    let fwd_res = kernel.go_forward().unwrap();
    assert!(!fwd_res, "go_forward must return false in VCD 2.0 mode");
}
