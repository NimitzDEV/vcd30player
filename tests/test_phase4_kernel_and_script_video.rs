use std::path::PathBuf;
use vcd30_player::core::kernel::VcdKernel;
use vcd30_player::core::script_ast::{ScriptProgram, Statement};
use vcd30_player::core::script_vm::{VcdScriptVm, VmHost, VmState};

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
    let disc_path = PathBuf::from(r"I:\");
    if !disc_path.exists() {
        eprintln!("Disc I:\\ not mounted, skipping test");
        return;
    }

    let mut kernel = VcdKernel::new();
    kernel.open_disc(disc_path).expect("Failed to open disc");

    // Stop opening video if playing so we can load SC1.CHM
    if kernel.is_video_active() {
        kernel.stop_video_and_exit().expect("Failed to stop opening video");
    }

    // Load SC1.CHM which has MUSIC15.DAT and MUSIC16.DAT links
    kernel.load_page("SC1.CHM", true).expect("Failed to load SC1.CHM");
    assert_eq!(kernel.current_page_name, "SC1.CHM");

    let dat_hotspot = {
        let doc = kernel.current_page.as_ref().unwrap();
        doc
            .get_all_hotspots()
            .into_iter()
            .find(|h| h.target.to_uppercase().contains("MUSIC16.DAT"))
            .cloned()
            .expect("SC1.CHM should have a hotspot targeting MUSIC16.DAT")
    };

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
