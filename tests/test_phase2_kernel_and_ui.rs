use std::path::PathBuf;
use vcd30_player::core::kernel::{CANVAS_HEIGHT, CANVAS_WIDTH, VcdKernel};

const DISC_ROOT: &str = r"I:\";

#[test]
fn test_kernel_open_disc_and_autorun() {
    let disc_path = PathBuf::from(DISC_ROOT);
    if !disc_path.exists() {
        eprintln!("Disc I:\\ not mounted, skipping test");
        return;
    }

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
    let disc_path = PathBuf::from(DISC_ROOT);
    if !disc_path.exists() {
        return;
    }

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
    let disc_path = PathBuf::from(DISC_ROOT);
    if !disc_path.exists() {
        return;
    }

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
