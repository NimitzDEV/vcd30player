mod common;

use vcd30_player::core::kernel::VcdKernel;
use vcd30_player::core::script_ast::{ScriptProgram, Statement};
use vcd30_player::core::script_vm::{VcdScriptVm, VmHost, VmState};
use vcd30_player::egui;

struct TestHost {
    video_played: Option<(String, i32, i32, Option<String>)>,
}

impl VmHost for TestHost {
    fn draw_image(&mut self, _filename: &str, _x: i32, _y: i32, _mode: i32) {}
    fn draw_cursor(&mut self, _x: i32, _y: i32) {}
    fn play_sound(&mut self, _filename: &str) {}
    fn play_video(&mut self, filename: &str, start_frame: i32, end_frame: i32, exit_page: Option<&str>) {
        self.video_played = Some((
            filename.to_string(),
            start_frame,
            end_frame,
            exit_page.map(|s| s.to_string()),
        ));
    }
    fn karaoke_set(&mut self, _channel: i32, _mode: i32) {}
    fn get_time_ms(&self) -> u64 {
        0
    }
}

#[test]
fn test_playvideo_script_parsing_and_vm_execution() {
    let script = r#"
100 PLAYVIDEO "MUSIC02.DAT",0,21775,"HOME.CHM"
200 END
"#;
    let prog = ScriptProgram::parse(script);
    let stmts_100 = prog.lines.get(&100).expect("Line 100 missing");
    assert_eq!(stmts_100.len(), 1);

    match &stmts_100[0] {
        Statement::PlayVideo {
            file,
            exit_page,
            ..
        } => {
            assert_eq!(file, "MUSIC02.DAT");
            assert_eq!(exit_page.as_deref(), Some("HOME.CHM"));
        }
        other => panic!("Expected Statement::PlayVideo, got {:?}", other),
    }

    let mut vm = VcdScriptVm::new();
    vm.load_program(prog);

    let mut host = TestHost { video_played: None };
    let state = vm.run_until_yield(&mut host);

    assert_eq!(state, VmState::WaitingForVideo);
    assert_eq!(
        host.video_played,
        Some((
            "MUSIC02.DAT".to_string(),
            0,
            21775,
            Some("HOME.CHM".to_string())
        ))
    );
}

#[test]
fn test_empty_startup_state() {
    let kernel = VcdKernel::new();
    assert!(kernel.disc_root.as_os_str().is_empty());
    assert!(kernel.current_page.is_none());
    assert!(kernel.current_page_name.is_empty());
    assert!(!kernel.is_video_active());
}

#[test]
fn test_hotspot_dat_target_triggers_video() {
    let disc_path = common::get_test_disc_root();
    let mut kernel = VcdKernel::new();
    kernel.open_disc(disc_path).expect("Failed to open disc");

    // Stop opening video if playing so we can load SC1.CHM
    if kernel.is_video_active() {
        kernel.stop_video_and_exit().expect("Failed to stop opening video");
    }

    // Load SC1.CHM which has MUSIC15.DAT and MUSIC16.DAT links
    kernel.load_page("SC1.CHM", true).expect("Failed to load SC1.CHM");
    assert_eq!(kernel.current_page_name, "SC1.CHM");

    let dat_hotspot = kernel
        .current_page
        .as_ref()
        .unwrap()
        .get_all_hotspots()
        .into_iter()
        .find(|h| h.target.to_uppercase().contains("MUSIC16.DAT"))
        .cloned()
        .expect("SC1.CHM should have a hotspot targeting MUSIC16.DAT");

    // Click hotspot
    let activated = kernel.activate_hotspot(&dat_hotspot).expect("Failed to activate hotspot");
    assert!(activated);
    assert!(kernel.is_video_active());

    let video = kernel.active_video.as_ref().unwrap();
    assert!(video.filename.to_uppercase().contains("MUSIC16.DAT"));
    assert_eq!(video.exit_target, None);

    // Stop video and verify return to SC1.CHM
    kernel.stop_video_and_exit().expect("Failed to exit video");
    assert!(!kernel.is_video_active());
    assert_eq!(kernel.current_page_name, "SC1.CHM");
}

#[test]
fn test_video_playback_escape_key_no_deadlock() {
    let disc_path = common::get_test_disc_root();
    let mut kernel = VcdKernel::new();
    kernel.open_disc(disc_path).expect("Failed to open disc");

    // Stop opening video if playing so we can load SC1.CHM
    if kernel.is_video_active() {
        kernel.stop_video_and_exit().expect("Failed to stop opening video");
    }

    kernel.load_page("SC1.CHM", true).expect("Failed to load SC1.CHM");
    let dat_hotspot = kernel
        .current_page
        .as_ref()
        .unwrap()
        .get_all_hotspots()
        .into_iter()
        .find(|h| h.target.to_uppercase().contains("MUSIC16.DAT"))
        .cloned()
        .expect("SC1.CHM should have a hotspot targeting MUSIC16.DAT");

    kernel.activate_hotspot(&dat_hotspot).expect("Failed to activate hotspot");
    assert!(kernel.is_video_active(), "Video should be active");

    let mut app = vcd30_player::ui::app::VcdPlayerApp::from_kernel(kernel);
    // Pause video so short mock test video doesn't end automatically before ESC is pressed
    app.kernel.active_video.as_mut().unwrap().toggle_play_pause();

    let ctx = egui::Context::default();

    // Pass 1: Render one video frame in UI
    let mut raw_input = egui::RawInput::default();
    raw_input.screen_rect = Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::Vec2::new(800.0, 600.0)));
    let mut out1 = ctx.run_ui(raw_input, |ui| {
        app.show(ui);
    });
    out1.textures_delta.clear();
    assert!(app.kernel.is_video_active());

    // Pass 2: Simulate pressing Escape key during video playback
    let mut raw_input_esc = egui::RawInput::default();
    raw_input_esc.screen_rect = Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::Vec2::new(800.0, 600.0)));
    raw_input_esc.events.push(egui::Event::Key {
        key: egui::Key::Escape,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: egui::Modifiers::NONE,
    });

    // This must complete immediately without deadlocking on RwLock (previously panicked after 10s)
    let start = std::time::Instant::now();
    let mut out2 = ctx.run_ui(raw_input_esc, |ui| {
        app.show(ui);
    });
    out2.textures_delta.clear();

    assert!(start.elapsed() < std::time::Duration::from_secs(2), "Must not deadlock on Escape key");
    assert!(!app.kernel.is_video_active(), "Video must be stopped by ESC");
    assert_eq!(app.kernel.current_page_name, "SC1.CHM", "Must return to SC1.CHM");
}

