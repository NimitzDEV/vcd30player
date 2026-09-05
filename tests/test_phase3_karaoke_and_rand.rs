use std::path::Path;
use vcd30_player::core::kernel::VcdKernel;
use vcd30_player::core::script_ast::ScriptProgram;
use vcd30_player::core::script_vm::{VcdScriptVm, VmHost, VmState};

#[derive(Default)]
struct MockKaraokeHost {
    playlist: Vec<i32>,
    video_played: Option<String>,
}

impl VmHost for MockKaraokeHost {
    fn draw_image(&mut self, _filename: &str, _x: i32, _y: i32, _mode: i32) {}
    fn draw_cursor(&mut self, _x: i32, _y: i32) {}
    fn play_sound(&mut self, _filename: &str) {}
    fn play_video(&mut self, filename: &str, _start: i32, _end: i32, _exit_page: Option<&str>) {
        self.video_played = Some(filename.to_string());
    }
    fn karaoke_set(&mut self, index: i32, val: i32) {
        let idx = if index <= 0 { 1 } else { index as usize };
        if idx <= self.playlist.len() {
            self.playlist[idx - 1] = val;
        } else if self.playlist.len() < 19 {
            self.playlist.push(val);
        }
    }
    fn karaoke_get(&self, index: i32) -> i32 {
        if index <= 0 || (index as usize) > self.playlist.len() {
            -1
        } else {
            self.playlist[(index - 1) as usize]
        }
    }
    fn karaoke_del(&mut self, index: i32) {
        if index > 0 && (index as usize) <= self.playlist.len() {
            self.playlist.remove((index - 1) as usize);
        }
    }
    fn karaoke_ins(&mut self, index: i32, val: i32) {
        if index > 0 && (index as usize) <= self.playlist.len() && self.playlist.len() < 19 {
            self.playlist.insert((index - 1) as usize, val);
        }
    }
    fn karaoke_play(&mut self) -> bool {
        if self.playlist.is_empty() {
            return false;
        }
        let song_id = self.playlist.remove(0);
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
    let state = vm.run_until_yield(&mut host);

    assert_eq!(state, VmState::Finished);
    assert_eq!(vm.get_variable(b'N'), 5);
    assert_eq!(vm.get_variable(b'M'), 5);
}

#[test]
fn test_kara_pr1_and_kara_2_disc_integration() {
    let disc_path = Path::new(r"I:\DATA\VCD_DATA");
    if !disc_path.exists() {
        return;
    }

    let mut kernel = VcdKernel::new();
    kernel.open_disc(std::path::PathBuf::from(r"I:\")).unwrap();

    // Test 1: Load KARA_2.CHM
    let res = kernel.load_page("KARA_2.CHM", true);
    assert!(res.is_ok(), "KARA_2.CHM must load successfully");
    assert_eq!(kernel.current_page_name, "KARA_2.CHM");

    // Initially playlist is empty, lines 200-235 finish quickly
    assert_eq!(kernel.karaoke_playlist.len(), 0);

    // Simulate clicking song button 1010 (Song B=4) -> line 2000 SET KARAOKE
    kernel.vm.start_at_line(1010);
    let state_song = kernel.run_vm();
    assert_eq!(state_song, VmState::Finished);
    assert_eq!(kernel.karaoke_playlist.len(), 1);
    assert_eq!(kernel.karaoke_playlist[0], 4);

    // Test 2: Load KARA_PR1.CHM (Random song picker)
    let res2 = kernel.load_page("KARA_PR1.CHM", true);
    assert!(res2.is_ok(), "KARA_PR1.CHM must load successfully");
    assert_eq!(kernel.current_page_name, "KARA_PR1.CHM");

    // Variable N should be picked at random in 1..=10, and M = N + 3
    let n = kernel.vm.get_variable(b'N');
    let m = kernel.vm.get_variable(b'M');
    assert!(n >= 1 && n <= 10, "KARA_PR1 must pick N in 1..=10, got {}", n);
    assert_eq!(m, n + 3, "M must be N + 3");
}
