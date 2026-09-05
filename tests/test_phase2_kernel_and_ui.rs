mod common;

use vcd30_player::core::kernel::{CANVAS_HEIGHT, CANVAS_WIDTH, VcdKernel};

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
