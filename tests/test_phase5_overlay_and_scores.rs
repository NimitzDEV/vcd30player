mod common;

use common::get_live_disc_root;
use vcd30_player::assets::chm::{ChunkPayload, CompHtmlDoc, MapArea, MicroScriptOp};
use vcd30_player::assets::ybm::RgbColor;
use vcd30_player::core::kernel::{var_name_to_index, VcdKernel};

#[test]
fn test_var_name_to_index_and_micro_scripts() {
    assert_eq!(var_name_to_index("i_a"), Some(0));
    assert_eq!(var_name_to_index("i_b"), Some(1));
    assert_eq!(var_name_to_index("i_e"), Some(4));
    assert_eq!(var_name_to_index("i_f"), Some(5));
    assert_eq!(var_name_to_index("A"), Some(0));
    assert_eq!(var_name_to_index("e"), Some(4));
    assert_eq!(var_name_to_index("10"), None);
    assert_eq!(var_name_to_index(""), None);

    let mut kernel = VcdKernel::new();
    assert_eq!(kernel.get_variable_by_name("i_e"), 0);

    // Test op1 + op2
    let add_op = MicroScriptOp {
        target_var: "i_e".to_string(),
        op1: "i_e".to_string(),
        opcode: 0,
        op2: "10".to_string(),
    };
    kernel.execute_micro_script(&add_op);
    assert_eq!(kernel.get_variable_by_name("i_e"), 10);

    kernel.execute_micro_script(&add_op);
    assert_eq!(kernel.get_variable_by_name("i_e"), 20);

    // Test op1 - op2
    let sub_op = MicroScriptOp {
        target_var: "i_e".to_string(),
        op1: "i_e".to_string(),
        opcode: 1,
        op2: "5".to_string(),
    };
    kernel.execute_micro_script(&sub_op);
    assert_eq!(kernel.get_variable_by_name("i_e"), 15);

    // Test op1 * op2
    let mul_op = MicroScriptOp {
        target_var: "i_e".to_string(),
        op1: "i_e".to_string(),
        opcode: 2,
        op2: "2".to_string(),
    };
    kernel.execute_micro_script(&mul_op);
    assert_eq!(kernel.get_variable_by_name("i_e"), 30);

    // Test reset: target = "0", op2 = ""
    let reset_op = MicroScriptOp {
        target_var: "i_e".to_string(),
        op1: "0".to_string(),
        opcode: 0,
        op2: "".to_string(),
    };
    kernel.execute_micro_script(&reset_op);
    assert_eq!(kernel.get_variable_by_name("i_e"), 0);
}

#[test]
fn test_chm_overlay_and_variable_text_rendering() {
    let mut kernel = VcdKernel::new();
    kernel.set_variable_by_name("i_e", 80);

    let overlay_doc = CompHtmlDoc {
        title: "SCOREV5".to_string(),
        author: String::new(),
        width: 352,
        height: 288,
        bg_color_idx: 0,
        palette_count: 0,
        palette: [RgbColor { r: 0, g: 0, b: 0 }; 256],
        chunks: vec![ChunkPayload::VariableText {
            x: 180,
            y: 168,
            var_name: "i_e".to_string(),
        }],
    };

    assert!(overlay_doc.is_overlay());
    assert_eq!(
        overlay_doc.get_variable_texts(),
        vec![(180, 168, "i_e")]
    );

    kernel.apply_overlay_doc(overlay_doc);
    assert!(kernel.overlay_doc.is_some());

    // Check that pixels at (180, 168) have text drawn
    let mut black_pixels = 0;
    for y in 168..168 + 16 {
        for x in 180..180 + 20 {
            let offset = ((y * 352 + x) * 4) as usize;
            if kernel.canvas[offset] == 0
                && kernel.canvas[offset + 1] == 0
                && kernel.canvas[offset + 2] == 0
                && kernel.canvas[offset + 3] == 255
            {
                black_pixels += 1;
            }
        }
    }
    assert!(black_pixels > 0, "Canvas must have glyph pixels drawn");
}

#[test]
fn test_live_disc_score05x_and_scorev5_flow() {
    let disc_root = match get_live_disc_root() {
        Some(p) => p,
        None => {
            eprintln!("No live disc detected, skipping real disc test");
            return;
        }
    };

    let mut kernel = VcdKernel::new();
    if kernel.open_disc(disc_root).is_err() {
        return;
    }

    if kernel.find_file("SCORE05X.CHM").is_none() || kernel.find_file("SCOREV5.CHM").is_none() {
        eprintln!("Disc does not contain SCORE05X.CHM or SCOREV5.CHM, skipping");
        return;
    }

    // 1. Simulate answering questions to reach 80 points
    kernel.set_variable_by_name("i_e", 80);

    // 2. Load SCORE05X.CHM (base page)
    kernel.load_page("SCORE05X.CHM", false).expect("Load SCORE05X.CHM");
    assert_eq!(kernel.current_page_name, "SCORE05X.CHM");
    assert!(kernel.overlay_doc.is_none());

    // Count non-black pixels from background image SCORE01.YBM
    let non_black_before = kernel
        .canvas
        .chunks_exact(4)
        .filter(|c| c[0] > 10 || c[1] > 10 || c[2] > 10)
        .count();
    assert!(
        non_black_before > 50000,
        "SCORE05X base page must display background image"
    );

    // 3. Hit test hotspot to SCOREV5.CHM: pt[0]=(93, 153), pt[1]=(152, 179)
    let hit_area: MapArea = (*kernel.hit_test(120, 166).expect("Must hit SCOREV5 hotspot")).clone();
    assert_eq!(hit_area.target, "SCOREV5.CHM");
    assert!(hit_area.is_overlay, "Hotspot must have overlay flag = true");

    // 4. Activate hotspot -> loads SCOREV5 as an overlay
    let activated = kernel.activate_hotspot(&hit_area).expect("Activate hotspot");
    assert!(activated);
    assert!(kernel.overlay_doc.is_some(), "Overlay doc must be active");
    assert_eq!(
        kernel.current_page_name, "SCORE05X.CHM",
        "Base page name must be preserved"
    );

    // 5. Verify screen is NOT blank black
    let non_black_after = kernel
        .canvas
        .chunks_exact(4)
        .filter(|c| c[0] > 10 || c[1] > 10 || c[2] > 10)
        .count();
    assert!(
        non_black_after > 50000,
        "Screen must retain base background when overlay is active"
    );

    // 6. Verify base page hotspots remain clickable (GAME.CHM at pt=(131, 33)..(236, 72))
    let exit_hit: MapArea = (*kernel.hit_test(180, 50).expect("Must hit base page GAME.CHM hotspot")).clone();
    assert_eq!(exit_hit.target, "GAME.CHM");

    // 7. Clicking GAME.CHM dismisses overlay and navigates to GAME.CHM
    kernel.activate_hotspot(&exit_hit).expect("Activate GAME.CHM");
    assert!(kernel.overlay_doc.is_none(), "Overlay must be cleared on navigation");
    assert_eq!(kernel.current_page_name, "GAME.CHM");
}

#[test]
fn test_live_disc_game051_answer_score_accumulation() {
    let disc_root = match get_live_disc_root() {
        Some(p) => p,
        None => return,
    };

    let mut kernel = VcdKernel::new();
    if kernel.open_disc(disc_root).is_err() {
        return;
    }

    if kernel.find_file("GAME051.CHM").is_none() {
        return;
    }

    kernel.load_page("GAME051.CHM", false).expect("Load GAME051.CHM");
    kernel.set_variable_by_name("i_e", 0);

    let (correct_area, exit_area): (MapArea, MapArea) = {
        let doc = kernel.current_page.as_ref().unwrap();
        let hotspots = doc.get_all_hotspots();

        // Area 6 is the correct answer with micro-script `i_e += 10`
        let correct = (*hotspots
            .iter()
            .find(|a| a.micro_scripts.iter().any(|s| s.target_var == "i_e" && s.op2 == "10"))
            .expect("Must find correct answer hotspot with micro_script"))
        .clone();

        // Area 0 is the exit button to GAME.CHM which resets `i_e = 0`
        let exit = (*hotspots
            .iter()
            .find(|a| a.micro_scripts.iter().any(|s| s.target_var == "i_e" && s.op1 == "0" && s.op2.is_empty()))
            .expect("Must find exit button hotspot with reset micro_script"))
        .clone();

        (correct, exit)
    };

    kernel.activate_hotspot(&correct_area).expect("Activate correct answer");
    assert_eq!(kernel.get_variable_by_name("i_e"), 10, "Score must increase by 10");

    kernel.activate_hotspot(&exit_area).expect("Activate exit button");
    assert_eq!(kernel.get_variable_by_name("i_e"), 0, "Score must reset to 0 on exit");
}

#[test]
fn test_live_disc_ex011_overlay_and_underline_hotspot() {
    let disc_root = match get_live_disc_root() {
        Some(p) => p,
        None => return,
    };

    let mut kernel = VcdKernel::new();
    if kernel.open_disc(disc_root).is_err() {
        return;
    }

    if kernel.find_file("EX011.CHM").is_none() {
        return;
    }

    kernel.load_page("EX011.CHM", false).expect("Load EX011.CHM");
    assert_eq!(kernel.current_page_name, "EX011.CHM");
    assert!(kernel.overlay_doc.is_none());

    // Test 1: Hit test with underline upward tolerance in blank space (y=75, underline is y=80..85)
    let hit_blank = kernel.hit_test(125, 75);
    assert!(hit_blank.is_some(), "Clicking in blank above underline must hit");
    let area0 = hit_blank.unwrap().clone();
    assert_eq!(area0.target, "AS011_1.CHM");
    assert!(area0.is_overlay);

    // Test 2: Exact hit test on underline (y=82)
    let hit_exact = kernel.hit_test(125, 82);
    assert!(hit_exact.is_some());
    assert_eq!(hit_exact.unwrap().target, "AS011_1.CHM");

    // Capture pixel at (50, 245) before activating overlay
    let pixel_idx = ((245 * 352 + 50) * 4) as usize;
    let base_pixel = [
        kernel.canvas[pixel_idx],
        kernel.canvas[pixel_idx + 1],
        kernel.canvas[pixel_idx + 2],
        kernel.canvas[pixel_idx + 3],
    ];

    // Test 3: Activate overlay hotspot AS011_1.CHM
    kernel.activate_hotspot(&area0).expect("Activate AS011_1.CHM");
    assert!(kernel.overlay_doc.is_some(), "Overlay doc must be active");
    assert_eq!(kernel.current_page_name, "EX011.CHM", "Base page remains EX011.CHM");

    // The sub-image AS011_1.YBM was blitted to (0, 228), modifying the answer box pixels
    let mut any_changed = false;
    for y in 228..266 {
        for x in 0..164 {
            let idx = ((y * 352 + x) * 4) as usize;
            if [kernel.canvas[idx], kernel.canvas[idx + 1], kernel.canvas[idx + 2]] != [base_pixel[0], base_pixel[1], base_pixel[2]] {
                any_changed = true;
                break;
            }
        }
        if any_changed {
            break;
        }
    }
    assert!(any_changed, "Overlay sub-image must have blitted onto canvas in the answer area");

    // Test 4: Hit test Question 2 underline (y=105, bounds 62,110 - 92,115) while overlay is active
    let hit_q2 = kernel.hit_test(75, 105);
    assert!(hit_q2.is_some(), "Base page hotspots remain clickable when overlay has no conflicting hotspots");
    let area1 = hit_q2.unwrap().clone();
    assert_eq!(area1.target, "AS011_2.CHM");
    kernel.activate_hotspot(&area1).expect("Activate AS011_2.CHM");
    assert!(kernel.overlay_doc.is_some());

    // Test 5: Return button (E011.CHM) dismisses overlay and navigates
    let hit_ret = kernel.hit_test(300, 250);
    assert!(hit_ret.is_some());
    let area_ret = hit_ret.unwrap().clone();
    assert_eq!(area_ret.target, "E011.CHM");
    assert!(!area_ret.is_overlay);
    kernel.activate_hotspot(&area_ret).expect("Activate return button");
    assert!(kernel.overlay_doc.is_none(), "Overlay must be cleared on navigation");
    assert_eq!(kernel.current_page_name, "E011.CHM");
}

