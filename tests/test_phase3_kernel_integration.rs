mod common;

use vcd30_player::core::kernel::VcdKernel;
use vcd30_player::core::script_vm::VmState;

#[test]
fn test_kernel_tb_script_and_hotspot_routing() {
    let disc_path = common::get_test_disc_root();
    let mut kernel = VcdKernel::new();
    kernel.open_disc(disc_path).unwrap();

    // Navigate to T_B.CHM
    kernel.load_page("T_B.CHM", true).unwrap();
    assert_eq!(kernel.current_page_name, "T_B.CHM");

    // Hit test on Option 1 (56, 114) to (68, 127)
    let hit = kernel.hit_test(60, 120);
    assert!(hit.is_some());
    let area = hit.unwrap().clone();
    assert_eq!(
        area.script_entry_line,
        Some(100),
        "Option 1 should route to script line 100"
    );

    // Click Option 1
    let activated = kernel.activate_hotspot(&area).unwrap();
    assert!(activated);

    // Fast-forward VM past DRAWIMAGE 300ms display delay
    while kernel.is_vm_active() {
        kernel.start_time -= std::time::Duration::from_millis(400);
        kernel.run_vm();
    }

    // Verify script updated variable A = 1
    assert_eq!(kernel.vm.get_variable(b'A'), 1);

    // Hit test on Next Page button (260, 255)
    let next_hit = kernel.hit_test(260, 255).unwrap().clone();
    assert_eq!(next_hit.script_entry_line, None);
    assert_eq!(next_hit.target, "T_C.CHM");

    // Click Next Page -> navigates to T_C.CHM
    kernel.activate_hotspot(&next_hit).unwrap();
    assert_eq!(kernel.current_page_name, "T_C.CHM");
}

#[test]
fn test_kernel_weight_interactive_script_and_remote_keys() {
    let disc_path = common::get_test_disc_root();
    let mut kernel = VcdKernel::new();
    kernel.open_disc(disc_path).unwrap();

    // Navigate to PROGRAM.CHM first, so history stack has ["HOMEPAGE.CHM", "PROGRAM.CHM"]
    kernel.load_page("PROGRAM.CHM", true).unwrap();

    // Navigate to WEIGHT.CHM
    kernel.load_page("WEIGHT.CHM", true).unwrap();
    assert_eq!(kernel.current_page_name, "WEIGHT.CHM");

    // Initially lines 6-8 yields WaitingForDelay for 30 units (3.0s intro delay)
    assert!(matches!(kernel.vm.state, VmState::WaitingForDelay { .. }));

    // Fast-forward past the 30-unit (3.0s) intro delay
    kernel.start_time = std::time::Instant::now() - std::time::Duration::from_secs(4);
    let state = kernel.run_vm();

    // Should stop at line 50: CALL IRKEY(X) waiting for input
    assert!(matches!(
        state,
        VmState::WaitingForKey { target_var: b'X' }
            | VmState::WaitingForKeyWithTimeout { target_var: b'X', .. }
    ));

    // Cursor should be at (272, 123) (Man)
    assert_eq!(kernel.cursor_pos, Some((272, 123)));

    // Press Down arrow (35) -> moves cursor to (294, 123) (Woman)
    let state_down = kernel.inject_remote_key(35);
    assert!(matches!(
        state_down,
        VmState::WaitingForKey { target_var: b'X' }
            | VmState::WaitingForKeyWithTimeout { target_var: b'X', .. }
    ));
    assert_eq!(kernel.cursor_pos, Some((294, 123)));

    // Press Up arrow (34) -> moves cursor back to (272, 123) (Man)
    let state_up = kernel.inject_remote_key(34);
    assert!(matches!(
        state_up,
        VmState::WaitingForKey { target_var: b'X' }
            | VmState::WaitingForKeyWithTimeout { target_var: b'X', .. }
    ));
    assert_eq!(kernel.cursor_pos, Some((272, 123)));

    let send_key = |kernel: &mut VcdKernel, key: i32| {
        let mut state = kernel.inject_remote_key(key);
        while kernel.is_vm_active() {
            kernel.start_time -= std::time::Duration::from_millis(400);
            state = kernel.run_vm();
        }
        state
    };

    // Press Confirm (31) on Man -> sets S = 1, enters line 200, waits for height input!
    let state_enter = send_key(&mut kernel, 31);
    assert_eq!(kernel.vm.get_variable(b'S'), 1);
    assert!(matches!(
        state_enter,
        VmState::WaitingForKey { target_var: b'K' }
    ));

    // Enter height digits: 1, 7, 5
    send_key(&mut kernel, 1);
    send_key(&mut kernel, 7);
    let state_h = send_key(&mut kernel, 5);
    assert_eq!(kernel.vm.get_variable(b'H'), 175);
    // Now waiting for weight input at line 5000: CALL IRKEY(K)
    assert!(matches!(
        state_h,
        VmState::WaitingForKey { target_var: b'K' }
    ));

    // Enter weight digits: 6, 5, then Confirm (31)
    send_key(&mut kernel, 6);
    send_key(&mut kernel, 5);
    let state_calc = kernel.inject_remote_key(31);
    assert_eq!(kernel.vm.get_variable(b'W'), 65);

    // Verify: after calculating result (ideal weight: 61..73kg),
    // VM does NOT crash into Error, but cleanly yields WaitingForDelay!
    assert!(
        matches!(state_calc, VmState::WaitingForDelay { .. }),
        "Expected WaitingForDelay on result screen, got {:?}",
        state_calc
    );

    // Test Issue 2 Part B: Hit test on return button (anchor at 300, 263)
    // Thanks to bounding box expansion, clicking near the icon (e.g. 295, 260) succeeds:
    let hit = kernel.hit_test(295, 260);
    assert!(hit.is_some(), "Near-anchor click on return button should succeed");
    assert_eq!(hit.unwrap().target, "PROGRAM.CHM");

    // Test Issue 1: Remote control key 32 ("返回/退出") while in delay state:
    // Should immediately navigate back to PROGRAM.CHM!
    kernel.inject_remote_key(32);
    assert_eq!(
        kernel.current_page_name, "PROGRAM.CHM",
        "Key 32 should navigate back from WEIGHT.CHM to PROGRAM.CHM"
    );
}
