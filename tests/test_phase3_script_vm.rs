use std::path::Path;
use vcd30_player::assets::chm::CompHtmlDoc;
use vcd30_player::core::script_ast::ScriptProgram;
use vcd30_player::core::script_vm::{VcdScriptVm, VmHost, VmState};

#[derive(Default)]
struct MockHost {
    images_drawn: Vec<(String, i32, i32, i32)>,
    cursors_drawn: Vec<(i32, i32)>,
    sounds_played: Vec<String>,
    karaoke_settings: Vec<(i32, i32)>,
    time_ms: u64,
}

impl VmHost for MockHost {
    fn draw_image(&mut self, filename: &str, x: i32, y: i32, mode: i32) {
        self.images_drawn
            .push((filename.to_string(), x, y, mode));
    }

    fn draw_cursor(&mut self, x: i32, y: i32) {
        self.cursors_drawn.push((x, y));
    }

    fn play_sound(&mut self, filename: &str) {
        self.sounds_played.push(filename.to_string());
    }

    fn play_video(&mut self, _filename: &str, _start: i32, _end: i32, _exit_page: Option<&str>) {}

    fn karaoke_set(&mut self, channel: i32, mode: i32) {
        self.karaoke_settings.push((channel, mode));
    }

    fn get_time_ms(&self) -> u64 {
        self.time_ms
    }
}

#[test]
fn test_vm_arithmetic_and_gosub() {
    let code = r#"
10 A = 10 : B = A * 2 + 5
20 GOSUB 100
30 END
100 C = A + B
110 RETURN
"#;
    let prog = ScriptProgram::parse(code);
    let mut vm = VcdScriptVm::new();
    let mut host = MockHost::default();

    vm.load_program(prog);
    let state = vm.run_until_yield(&mut host);

    assert_eq!(state, VmState::Finished);
    assert_eq!(vm.get_variable(b'A'), 10);
    assert_eq!(vm.get_variable(b'B'), 25);
    assert_eq!(vm.get_variable(b'C'), 35);
}

#[test]
fn test_vm_unrecognized_instruction_alert_and_skip() {
    let code = r#"
10 A = 5
20 INVALID_OPCODE_TEST 12345
30 A = A + 10
40 END
"#;
    let prog = ScriptProgram::parse(code);
    let mut vm = VcdScriptVm::new();
    let mut host = MockHost::default();

    vm.load_program(prog);
    let state1 = vm.run_until_yield(&mut host);

    match state1 {
        VmState::PausedForAlert(alert) => {
            assert_eq!(alert.line_no, 20);
            assert!(alert.raw_code.contains("INVALID_OPCODE_TEST"));
        }
        _ => panic!("Expected PausedForAlert, got {:?}", state1),
    }

    // User chooses to skip and continue
    let state2 = vm.skip_unrecognized_and_continue(&mut host);
    assert_eq!(state2, VmState::Finished);
    assert_eq!(vm.get_variable(b'A'), 15);
}

#[test]
fn test_vm_call_irkey_waiting_and_inject() {
    let code = r#"
10 S = 0
20 CALL IRKEY(K)
30 IF K = 31 THEN S = 100
40 END
"#;
    let prog = ScriptProgram::parse(code);
    let mut vm = VcdScriptVm::new();
    let mut host = MockHost::default();

    vm.load_program(prog);
    let state1 = vm.run_until_yield(&mut host);

    assert_eq!(state1, VmState::WaitingForKey { target_var: b'K' });
    assert_eq!(vm.get_variable(b'S'), 0);

    // Inject remote key 31 (Confirm)
    let state2 = vm.inject_key(31, &mut host);
    assert_eq!(state2, VmState::Finished);
    assert_eq!(vm.get_variable(b'K'), 31);
    assert_eq!(vm.get_variable(b'S'), 100);
}

#[test]
fn test_vm_tb_script_execution() {
    let data_dir = Path::new(r"I:\DATA\VCD_DATA");
    if !data_dir.exists() {
        return;
    }

    let tb_path = data_dir.join("T_B.CHM");
    if !tb_path.exists() {
        return;
    }

    let bytes = std::fs::read(&tb_path).unwrap();
    let doc = CompHtmlDoc::parse(&bytes).unwrap();
    let script = doc.get_script().expect("T_B.CHM must have VCDSCRIPT");
    let prog = ScriptProgram::parse(script);

    let mut vm = VcdScriptVm::new();
    let mut host = MockHost::default();

    vm.load_program(prog);

    // Initial run: executes lines 20-21 (KARAOKE SET) until 95 END
    let init_state = vm.run_until_yield(&mut host);
    assert_eq!(init_state, VmState::Finished);
    assert_eq!(host.karaoke_settings.len(), 2);
    assert_eq!(vm.get_variable(b'A'), 0);
    assert_eq!(vm.get_variable(b'B'), 0);

    // Simulate clicking Option 1 (Line 100)
    // 100 DRAWIMAGE "T_B.YBM",0,0,0 : 105 A = 1 : 106 GOSUB 9000 : 107 GOSUB 9030 ...
    vm.start_at_line(100);
    let state100 = vm.run_until_yield(&mut host);
    assert_eq!(state100, VmState::Finished);
    assert_eq!(vm.get_variable(b'A'), 1);

    // Verify sub-image T_DOT.YBM was drawn at (45, 117)
    let dot_drawn = host
        .images_drawn
        .iter()
        .any(|(f, x, y, _)| f.contains("T_DOT.YBM") && *x == 45 && *y == 117);
    assert!(dot_drawn, "Option 1 should draw T_DOT.YBM at (45, 117)");
}

#[test]
fn test_vm_time_delay_loop_yielding() {
    let code = r#"
10 CALL TIME(X) : X = X + 50
20 CALL TIME(Y)
30 IF Y < X THEN GOTO 20
40 A = 999
50 END
"#;
    let prog = ScriptProgram::parse(code);
    let mut vm = VcdScriptVm::new();
    let mut host = MockHost::default();
    host.time_ms = 0;

    vm.load_program(prog);
    let state = vm.run_until_yield(&mut host);

    // Should yield in WaitingForDelay at line 30, waiting until time unit 50
    assert_eq!(state, VmState::WaitingForDelay { until_time: 50 });
    assert_eq!(vm.get_variable(b'A'), 0);

    // If time has not elapsed yet (2000ms / 100 = 20 < 50)
    host.time_ms = 2000;
    let state2 = vm.run_until_yield(&mut host);
    assert_eq!(state2, VmState::WaitingForDelay { until_time: 50 });

    // When time elapses (5000ms / 100 = 50 >= 50)
    host.time_ms = 5000;
    let state3 = vm.run_until_yield(&mut host);
    assert_eq!(state3, VmState::Finished);
    assert_eq!(vm.get_variable(b'A'), 999);
}
