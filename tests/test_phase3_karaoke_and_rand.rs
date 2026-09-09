mod common;

use vcd30_player::core::kernel::{KaraokePlaylist, VcdKernel};
use vcd30_player::core::script_ast::ScriptProgram;
use vcd30_player::core::script_vm::{VcdScriptVm, VmHost, VmState};

#[derive(Default)]
struct MockKaraokeHost {
    playlist: KaraokePlaylist,
    video_played: Option<String>,
    time_units: i32,
}

impl VmHost for MockKaraokeHost {
    fn draw_image(&mut self, _filename: &str, _x: i32, _y: i32, _mode: i32) {}
    fn draw_cursor(&mut self, _x: i32, _y: i32) {}
    fn play_sound(&mut self, _filename: &str) {}
    fn play_video(&mut self, filename: &str, _start: i32, _end: i32, _exit_page: Option<&str>) {
        self.video_played = Some(filename.to_string());
    }
    fn karaoke_set(&mut self, index: i32, val: i32) {
        self.playlist.set(index, val);
    }
    fn karaoke_get(&self, index: i32) -> i32 {
        self.playlist.get(index)
    }
    fn karaoke_del(&mut self, index: i32) {
        self.playlist.del(index);
    }
    fn karaoke_ins(&mut self, index: i32, val: i32) {
        self.playlist.ins(index, val);
    }
    fn karaoke_play(&mut self) -> bool {
        let song_id = match self.playlist.play() {
            Some(id) => id,
            None => return false,
        };
        let track = if song_id < 10 {
            format!("MPEGAV/MUSIC0{}.DAT", song_id)
        } else {
            format!("MPEGAV/MUSIC{}.DAT", song_id)
        };
        self.video_played = Some(track);
        true
    }
    fn get_time_ms(&self) -> u64 {
        12345
    }
    fn get_time_units(&self) -> i32 {
        self.time_units
    }
}

#[test]
fn test_call_rand_prng_execution() {
    let code = r#"
10 CALL RAND(N)
20 CALL RAND(M)
30 A = N - (N / 20) * 20
40 END
"#;
    let prog = ScriptProgram::parse(code);
    let mut vm = VcdScriptVm::new();
    let mut host = MockKaraokeHost::default();

    vm.load_program(prog);
    let state = vm.run_until_yield(&mut host);

    assert_eq!(state, VmState::Finished);
    let n = vm.get_variable(b'N');
    let m = vm.get_variable(b'M');
    let a = vm.get_variable(b'A');

    // N and M should be positive pseudo-random numbers, not equal
    assert!(n > 0, "N should be positive, got {}", n);
    assert!(m > 0, "M should be positive, got {}", m);
    assert_ne!(n, m, "Two consecutive RAND calls should produce different values");

    // A is N % 20, should be in 0..=19
    assert!(a >= 0 && a < 20, "N % 20 must be in 0..=19, got {}", a);
}

#[test]
fn test_karaoke_playlist_operations() {
    let code = r#"
10 KARAOKE SET 1, 4
20 KARAOKE SET 2, 8
30 KARAOKE GET 1, A
40 KARAOKE GET 2, B
50 KARAOKE INS 2, 6
60 KARAOKE GET 2, C
70 KARAOKE GET 3, D
80 KARAOKE DEL 1
90 KARAOKE GET 1, E
100 KARAOKE GET 3, F
110 END
"#;
    let prog = ScriptProgram::parse(code);
    let mut vm = VcdScriptVm::new();
    let mut host = MockKaraokeHost::default();

    vm.load_program(prog);
    let state = vm.run_until_yield(&mut host);

    assert_eq!(state, VmState::Finished);
    assert_eq!(vm.get_variable(b'A'), 4);
    assert_eq!(vm.get_variable(b'B'), 8);
    // After INS 2, 6: playlist is [4, 6, 8]
    assert_eq!(vm.get_variable(b'C'), 6);
    assert_eq!(vm.get_variable(b'D'), 8);
    // After DEL 1: playlist is [6, 8]
    assert_eq!(vm.get_variable(b'E'), 6);
    assert_eq!(vm.get_variable(b'F'), -1); // Out of bounds returns -1!
}

#[test]
fn test_karaoke_play_video_triggering() {
    let code = r#"
10 KARAOKE SET 1, 7
20 KARAOKE PLAY
30 END
"#;
    let prog = ScriptProgram::parse(code);
    let mut vm = VcdScriptVm::new();
    let mut host = MockKaraokeHost::default();

    vm.load_program(prog);
    let state = vm.run_until_yield(&mut host);

    // After KARAOKE PLAY, VM should yield WaitingForVideo
    assert_eq!(state, VmState::WaitingForVideo);
    assert_eq!(
        host.video_played.as_deref(),
        Some("MPEGAV/MUSIC07.DAT"),
        "KARAOKE PLAY should trigger MUSIC07.DAT for song 7"
    );
    assert!(host.playlist.is_empty(), "Song 1 should have been popped from playlist");
}

#[test]
fn test_typo_resilience_kraroke_and_drawimgae() {
    let code = r#"
10 KRAROKE SET 1, 5
20 KRAROKE GET 1, N
30 KRARAOKE GET 1, M
40 DRAWIMGAE "KARA_N01.YBM", 10, 20, 0
50 END
"#;
    let prog = ScriptProgram::parse(code);
    let mut vm = VcdScriptVm::new();
    let mut host = MockKaraokeHost::default();

    vm.load_program(prog);
    let mut state = vm.run_until_yield(&mut host);
    while matches!(state, VmState::WaitingForDelay { .. }) {
        host.time_units += 4;
        state = vm.run_until_yield(&mut host);
    }

    assert_eq!(state, VmState::Finished);
    assert_eq!(vm.get_variable(b'N'), 5);
    assert_eq!(vm.get_variable(b'M'), 5);
}

#[test]
fn test_kara_pr1_and_kara_2_disc_integration() {
    let disc_path = common::get_test_disc_root();
    let mut kernel = VcdKernel::new();
    kernel.open_disc(disc_path).unwrap();

    // Test 1: Load KARA_2.CHM
    let res = kernel.load_page("KARA_2.CHM", true);
    assert!(res.is_ok(), "KARA_2.CHM must load successfully");
    assert_eq!(kernel.current_page_name, "KARA_2.CHM");

    // Initially playlist is empty, lines 200-235 finish quickly
    assert_eq!(kernel.karaoke_playlist.len(), 0);

    // Simulate clicking song button 1010 (Song B=4) -> line 2000 SET KARAOKE
    kernel.vm.start_at_line(1010);
    let mut state_song = kernel.run_vm();
    while kernel.is_vm_active() {
        kernel.start_time -= std::time::Duration::from_millis(400);
        state_song = kernel.run_vm();
    }
    assert_eq!(state_song, VmState::Finished);
    assert_eq!(kernel.karaoke_playlist.len(), 1);
    assert_eq!(kernel.karaoke_playlist[0], 4);

    // Test 2: Load KARA_PR1.CHM (Random song picker)
    let res = kernel.load_page("KARA_PR1.CHM", true);
    assert!(res.is_ok(), "KARA_PR1.CHM must load successfully");
    assert_eq!(kernel.current_page_name, "KARA_PR1.CHM");
    let n = kernel.vm.get_variable(b'N');
    let m = kernel.vm.get_variable(b'M');
    assert!(n >= 1 && n <= 10, "KARA_PR1 must pick N in 1..=10, got {}", n);
    assert_eq!(m, n + 3, "M must be N + 3");
}

#[test]
fn test_if_then_else_parsing_and_execution() {
    let code = r#"
10 X = 5
20 IF X = 10 THEN Y = 100 ELSE Y = 200
30 IF X = 5 THEN Z = 300 ELSE Z = 400
40 END
"#;
    let prog = ScriptProgram::parse(code);
    let mut vm = VcdScriptVm::new();
    let mut host = MockKaraokeHost::default();

    vm.load_program(prog);
    let state = vm.run_until_yield(&mut host);

    assert_eq!(state, VmState::Finished);
    assert_eq!(vm.get_variable(b'Y'), 200, "ELSE branch should execute when cond false");
    assert_eq!(vm.get_variable(b'Z'), 300, "THEN branch should execute when cond true");
}

#[test]
fn test_kara_1_sequential_playback_flow() {
    let disc_path = common::get_test_disc_root();
    let mut kernel = VcdKernel::new();
    kernel.open_disc(disc_path).unwrap();

    // 1. Initialise KARA_1.CHM (sets 19 slots to 0)
    kernel.load_page("KARA_1.CHM", true).unwrap();
    assert_eq!(kernel.karaoke_playlist.len(), 19);
    for i in 0..19 {
        assert_eq!(kernel.karaoke_playlist[i], 0);
    }

    // 2. Navigate to KARA_P1.CHM (sequential player)
    kernel.load_page("KARA_P1.CHM", true).unwrap();
    while matches!(kernel.vm.state, VmState::WaitingForDelay { .. }) {
        kernel.start_time -= std::time::Duration::from_millis(400);
        kernel.run_vm();
    }
    // At line 610, it should be waiting for key with 1-second timeout
    assert!(matches!(
        kernel.vm.state,
        VmState::WaitingForKeyWithTimeout { target_var: b'I', .. }
    ));
    assert_eq!(kernel.karaoke_playlist[0], 4, "First song should be 4 (MUSIC04.DAT)");
    assert_eq!(kernel.karaoke_playlist[1], 1, "Track counter B should be 1");

    // Simulate clicking Continue (hotspot line 700) or 1s timeout:
    kernel.vm.start_at_line(700);
    let state = kernel.run_vm();
    assert_eq!(state, VmState::WaitingForVideo);
    assert!(kernel.is_video_active(), "Video MUSIC04.DAT should be playing");

    // 3. Stop video (simulate playback end)
    kernel.stop_video_and_exit().unwrap();
    while matches!(kernel.vm.state, VmState::WaitingForDelay { .. }) {
        kernel.start_time -= std::time::Duration::from_millis(400);
        kernel.run_vm();
    }
    // on_video_finished() should reset PC to Entry 1 (Line 20)
    // Line 20 reads B=1, increments to B=2, sets song 5 (MUSIC05.DAT), sets B=2, enters wait loop!
    assert!(matches!(
        kernel.vm.state,
        VmState::WaitingForKeyWithTimeout { target_var: b'I', .. }
    ));
    assert_eq!(kernel.karaoke_playlist[0], 5, "Second song should be 5 (MUSIC05.DAT)");
    assert_eq!(kernel.karaoke_playlist[1], 2, "Track counter B should be 2");

    // 4. Play second song
    kernel.vm.start_at_line(700);
    let state = kernel.run_vm();
    assert_eq!(state, VmState::WaitingForVideo);
    kernel.stop_video_and_exit().unwrap();
    while matches!(kernel.vm.state, VmState::WaitingForDelay { .. }) {
        kernel.start_time -= std::time::Duration::from_millis(400);
        kernel.run_vm();
    }

    // Line 20 reads B=2, increments to B=3, sets song 6 (MUSIC06.DAT)...
    assert!(matches!(
        kernel.vm.state,
        VmState::WaitingForKeyWithTimeout { target_var: b'I', .. }
    ));
    assert_eq!(kernel.karaoke_playlist[0], 6, "Third song should be 6 (MUSIC06.DAT)");
    assert_eq!(kernel.karaoke_playlist[1], 3, "Track counter B should be 3");
}

#[test]
fn test_kara_2l1_delete_and_href() {
    let disc_path = common::get_test_disc_root();
    let mut kernel = VcdKernel::new();
    kernel.open_disc(disc_path).unwrap();

    // Set up playlist with 3 songs: 4, 5, 6
    kernel.karaoke_playlist.set(1, 4);
    kernel.karaoke_playlist.set(2, 5);
    kernel.karaoke_playlist.set(3, 6);
    assert_eq!(kernel.karaoke_playlist.len(), 3);

    // Load KARA_2L1.CHM (Delete last song)
    kernel.load_page("KARA_2L1.CHM", true).unwrap();

    // Must finish without loop limit error
    assert_eq!(kernel.vm.state, VmState::Finished, "KARA_2L1.CHM must finish without loop error");
    assert_eq!(kernel.karaoke_playlist.len(), 2, "Playlist should now have 2 songs");
    assert_eq!(kernel.karaoke_playlist.get(1), 4);
    assert_eq!(kernel.karaoke_playlist.get(2), 5);
    assert_eq!(kernel.karaoke_playlist.get(3), -1);

    // Hotspot check: KARA_2L1.CHM must have the full-screen chunk 13 Href hotspot
    let area = kernel.hit_test(100, 100).cloned().expect("Should hit full-screen Href hotspot");
    assert_eq!(area.target, "KARA_2L.CHM");
    let activated = kernel.activate_hotspot(&area).unwrap();
    assert!(activated);
    assert_eq!(kernel.current_page_name, "KARA_2L.CHM");
}

#[test]
fn test_inspect_all_kara_pages() {
    let disc_path = common::get_test_disc_root().join("DATA").join("VCD_DATA");
    if !disc_path.exists() {
        return;
    }

    for name in ["KARA_1.CHM", "KARA_2.CHM", "KARA_P1.CHM", "KARA_PR1.CHM", "KARA_PS.CHM", "KARA_2L.CHM", "KARA_2L1.CHM"] {
        let p = disc_path.join(name);
        if let Ok(bytes) = std::fs::read(&p) {
            if let Ok(doc) = vcd30_player::assets::chm::CompHtmlDoc::parse(&bytes) {
                println!("=== CHM: {} ===", name);
                for (i, area) in doc.get_all_hotspots().iter().enumerate() {
                    println!("  Hotspot {}: target='{}', line={:?}, rect={:?}", i, area.target, area.script_entry_line, (area.x1, area.y1, area.x2, area.y2));
                }
            }
        }
    }
}

