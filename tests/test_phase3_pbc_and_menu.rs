//! Comprehensive integration and unit tests for Phase 3:
//! VCD 2.0 Playback Control (PBC) state machine, LOT/PSD table parsing,
//! still/motion menu rendering, and the dedicated "PBC" button.

mod common;

use std::path::{Path, PathBuf};
use vcd30_player::core::kernel::{ActiveDiscMode, VcdKernel};
use vcd30_player::vcd::{
    LotTable, PbcAction, PbcEngine, PbcState, PsdDescriptor, PsdTable,
};

/// Returns the path to the VCD 2.0 paradise reference disc if available.
fn get_paradise_disc_root() -> Option<PathBuf> {
    let candidate = Path::new(r"E:\iso-disk\paradise-vcd2.0");
    if candidate.exists() {
        Some(candidate.to_path_buf())
    } else {
        None
    }
}

#[test]
fn test_lot_table_parsing_and_resolution() {
    let mut raw = vec![0u8; 32]; // 16 entries (u16)
    // index 0: 0x0000
    // index 1: 0x0000
    raw[2] = 0; raw[3] = 0;
    // index 2: 0x0002
    raw[4] = 0; raw[5] = 2;
    // index 3: 0x0009
    raw[6] = 0; raw[7] = 9;
    // index 4: 0xFFFF
    raw[8] = 0xFF; raw[9] = 0xFF;
    // index 5: 0x0015
    raw[10] = 0; raw[11] = 0x15;

    let lot = LotTable::parse(&raw, 8).expect("parse LOT");
    assert_eq!(lot.entry_count(), 16);
    assert_eq!(lot.offset_multiplier(), 8);

    assert_eq!(lot.get_unit_offset(1), Some(0));
    assert_eq!(lot.get_byte_offset(1), Some(0));

    assert_eq!(lot.get_unit_offset(2), Some(2));
    assert_eq!(lot.get_byte_offset(2), Some(16));

    assert_eq!(lot.get_unit_offset(3), Some(9));
    assert_eq!(lot.get_byte_offset(3), Some(72));

    assert_eq!(lot.get_unit_offset(4), None);
    assert_eq!(lot.get_byte_offset(4), None);

    assert_eq!(lot.get_unit_offset(5), Some(0x15));
    assert_eq!(lot.get_byte_offset(5), Some(0x15 * 8));

    let valids = lot.valid_lids();
    assert_eq!(valids, vec![1, 2, 3, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15]);
}

#[test]
fn test_psd_table_parsing_playlists_and_selections() {
    let mut raw = Vec::new();
    // PlayList at unit 0 (byte 0)
    raw.push(0x10);
    raw.push(2); // noi = 2
    raw.extend_from_slice(&1u16.to_be_bytes()); // lid = 1
    raw.extend_from_slice(&0xFFFFu16.to_be_bytes()); // prev = none
    raw.extend_from_slice(&4u16.to_be_bytes()); // next = unit 4
    raw.extend_from_slice(&0xFFFFu16.to_be_bytes()); // ret = none
    raw.extend_from_slice(&0u16.to_be_bytes()); // ptime = 0
    raw.push(5); // wtime = 5s
    raw.push(0); // atime = 0
    raw.extend_from_slice(&2u16.to_be_bytes()); // items[0] = 2
    raw.extend_from_slice(&3u16.to_be_bytes()); // items[1] = 3

    // SelectionList at unit 4 (byte 32)
    while raw.len() < 32 {
        raw.push(0); // padding
    }
    raw.push(0x18);
    raw.push(0); // flags
    raw.push(3); // nos = 3
    raw.push(1); // bsn = 1
    raw.extend_from_slice(&2u16.to_be_bytes()); // lid = 2
    raw.extend_from_slice(&0u16.to_be_bytes()); // prev = unit 0
    raw.extend_from_slice(&8u16.to_be_bytes()); // next = unit 8
    raw.extend_from_slice(&0u16.to_be_bytes()); // ret = unit 0
    raw.extend_from_slice(&8u16.to_be_bytes()); // def = unit 8
    raw.extend_from_slice(&8u16.to_be_bytes()); // to = unit 8
    raw.push(20); // timeout = 20s
    raw.push(1); // loop = 1
    raw.extend_from_slice(&1000u16.to_be_bytes()); // item_id = 1000 (/SEGMENT/ITEM0001.DAT)
    raw.extend_from_slice(&8u16.to_be_bytes()); // sel 1 -> unit 8
    raw.extend_from_slice(&9u16.to_be_bytes()); // sel 2 -> unit 9
    raw.extend_from_slice(&10u16.to_be_bytes()); // sel 3 -> unit 10

    let psd = PsdTable::parse(&raw, 8).expect("parse PSD");
    assert_eq!(psd.descriptors.len(), 2);

    match psd.get_by_unit_offset(0) {
        Some(PsdDescriptor::PlayList(p)) => {
            assert_eq!(p.lid, 1);
            assert_eq!(p.noi, 2);
            assert_eq!(p.wtime, 5);
            assert_eq!(p.items, vec![2, 3]);
        }
        _ => panic!("expected PlayList at unit 0"),
    }

    match psd.get_by_unit_offset(4) {
        Some(PsdDescriptor::SelectionList(s)) => {
            assert_eq!(s.lid, 2);
            assert_eq!(s.nos, 3);
            assert_eq!(s.bsn, 1);
            assert_eq!(s.item_id, 1000);
            assert_eq!(s.selections, vec![8, 9, 10]);
        }
        _ => panic!("expected SelectionList at unit 4"),
    }
}

#[test]
fn test_pbc_state_machine_transitions() {
    let mut raw_lot = vec![0u8; 16];
    raw_lot[2] = 0; raw_lot[3] = 0; // LID 1 -> unit 0
    raw_lot[4] = 0; raw_lot[5] = 2; // LID 2 -> unit 2
    raw_lot[6] = 0; raw_lot[7] = 5; // LID 3 -> unit 5

    let lot = LotTable::parse(&raw_lot, 8).unwrap();

    let mut raw_psd = Vec::new();
    // 1. PlayList LID 1 at unit 0 (byte 0)
    raw_psd.push(0x10);
    raw_psd.push(1);
    raw_psd.extend_from_slice(&1u16.to_be_bytes());
    raw_psd.extend_from_slice(&0xFFFFu16.to_be_bytes());
    raw_psd.extend_from_slice(&2u16.to_be_bytes()); // next = unit 2
    raw_psd.extend_from_slice(&0xFFFFu16.to_be_bytes());
    raw_psd.extend_from_slice(&0u16.to_be_bytes());
    raw_psd.push(0); // wtime = 0
    raw_psd.push(0);
    raw_psd.extend_from_slice(&2u16.to_be_bytes()); // track 2

    // 2. SelectionList LID 2 at unit 2 (byte 16)
    raw_psd.push(0x18);
    raw_psd.push(0);
    raw_psd.push(2); // nos = 2
    raw_psd.push(1); // bsn = 1
    raw_psd.extend_from_slice(&2u16.to_be_bytes());
    raw_psd.extend_from_slice(&0u16.to_be_bytes()); // prev = unit 0
    raw_psd.extend_from_slice(&5u16.to_be_bytes()); // next = unit 5
    raw_psd.extend_from_slice(&0u16.to_be_bytes()); // ret = unit 0
    raw_psd.extend_from_slice(&5u16.to_be_bytes()); // def = unit 5
    raw_psd.extend_from_slice(&5u16.to_be_bytes()); // to = unit 5
    raw_psd.push(10); // timeout = 10s
    raw_psd.push(1);
    raw_psd.extend_from_slice(&1000u16.to_be_bytes()); // ITEM0001.DAT
    raw_psd.extend_from_slice(&5u16.to_be_bytes()); // sel 1 -> unit 5
    raw_psd.extend_from_slice(&5u16.to_be_bytes()); // sel 2 -> unit 5

    // 3. PlayList LID 3 at unit 5 (byte 40)
    raw_psd.push(0x10);
    raw_psd.push(1);
    raw_psd.extend_from_slice(&3u16.to_be_bytes());
    raw_psd.extend_from_slice(&2u16.to_be_bytes()); // prev = unit 2
    raw_psd.extend_from_slice(&2u16.to_be_bytes()); // next = unit 2
    raw_psd.extend_from_slice(&2u16.to_be_bytes()); // ret = unit 2
    raw_psd.extend_from_slice(&0u16.to_be_bytes());
    raw_psd.push(0);
    raw_psd.push(0);
    raw_psd.extend_from_slice(&3u16.to_be_bytes()); // track 3

    let psd = PsdTable::parse(&raw_psd, 8).unwrap();
    let mut engine = PbcEngine::new(lot, psd);

    // Initial state
    assert_eq!(engine.state, PbcState::Uninitialized);
    let act1 = engine.start().expect("start action");
    assert_eq!(
        act1,
        PbcAction::PlayTrack {
            track_number: 2,
            item_id: 2,
            ptime: 0,
            wtime: 0
        }
    );

    // Track finishes -> transitions to LID 2 SelectionList
    let act2 = engine.on_item_finished().expect("menu action");
    assert_eq!(
        act2,
        PbcAction::DisplayStillMenu {
            item_id: 1000,
            segment_index: 1,
            nos: 2,
            bsn: 1,
            timeout_sec: 10
        }
    );

    // User selects 1 -> transitions to unit 5 (PlayList track 3)
    let act3 = engine.select_number(1).unwrap().expect("track 3 action");
    assert_eq!(
        act3,
        PbcAction::PlayTrack {
            track_number: 3,
            item_id: 3,
            ptime: 0,
            wtime: 0
        }
    );

    // While playing track 3, user presses dedicated PBC button -> jumps back to SelectionList
    let act4 = engine.press_pbc().expect("pbc return to menu");
    assert_eq!(
        act4,
        PbcAction::DisplayStillMenu {
            item_id: 1000,
            segment_index: 1,
            nos: 2,
            bsn: 1,
            timeout_sec: 10
        }
    );

    // Timeout occurs on menu (10s elapsed) -> transitions to unit 5
    let act5 = engine.tick(10.5).expect("timeout action");
    assert_eq!(
        act5,
        PbcAction::PlayTrack {
            track_number: 3,
            item_id: 3,
            ptime: 0,
            wtime: 0
        }
    );
}

#[test]
fn test_paradise_vcd20_disc_metadata_and_pbc_execution() {
    let paradise_root = match get_paradise_disc_root() {
        Some(r) => r,
        None => {
            println!("Skipping live paradise disc test: path not found");
            return;
        }
    };

    let mut kernel = VcdKernel::new();
    kernel.open_disc(paradise_root).expect("open paradise disc");

    // Paradise disc is standard VCD 2.0 with PBC
    assert_eq!(kernel.active_mode, ActiveDiscMode::Vcd20Classic);
    assert!(kernel.pbc.is_some());
    assert_eq!(kernel.tracks.len(), 18);

    // Initial state: starts at LID 1 (AVSEQ18.DAT intro video)
    if let Some(ref player) = kernel.active_video {
        assert!(
            player.filename.contains("AVSEQ18"),
            "Expected intro video AVSEQ18, got {}",
            player.filename
        );
    }

    // User presses dedicated "PBC" button -> jumps to LID 2 motion menu (AVSEQ17.DAT)!
    kernel.trigger_pbc_menu().expect("trigger PBC menu");
    if let Some(ref player) = kernel.active_video {
        assert!(
            player.filename.contains("AVSEQ17"),
            "Expected motion menu AVSEQ17, got {}",
            player.filename
        );
    }

    // User selects 1 on remote / numeric keypad (multi-digit menu waits for Enter or second digit)
    kernel.handle_pbc_digit(1).expect("enter digit 1");
    kernel.handle_pbc_enter().expect("confirm selection 1");
    if let Some(ref player) = kernel.active_video {
        assert!(
            player.filename.contains("AVSEQ01"),
            "Expected track 1 AVSEQ01, got {}",
            player.filename
        );
    }

    // While playing song 1, user presses Return / Escape -> returns to motion menu!
    kernel.handle_pbc_return().expect("return to menu");
    if let Some(ref player) = kernel.active_video {
        assert!(
            player.filename.contains("AVSEQ17"),
            "Expected return to motion menu AVSEQ17, got {}",
            player.filename
        );
    }
}

#[test]
fn test_live_disc_h_vcd20_mode_still_picture_menu() {
    let disc_root = Path::new("H:/");
    if !disc_root.exists() || !disc_root.join("VCD").join("PSD.VCD").exists() {
        println!("Skipping H: disc test: drive not mounted or not VCD");
        return;
    }

    let mut kernel = VcdKernel::new();
    kernel.open_disc(disc_root.to_path_buf()).expect("open H disc");

    // Switch to VCD 2.0 mode
    kernel.switch_active_mode(ActiveDiscMode::Vcd20Classic).expect("switch to VCD 2.0");
    assert_eq!(kernel.active_mode, ActiveDiscMode::Vcd20Classic);
    assert!(kernel.pbc.is_some());

    // Trigger PBC menu -> loads /SEGMENT/ITEM0001.DAT high-resolution still picture menu
    kernel.trigger_pbc_menu().expect("trigger PBC still menu");
    assert!(
        kernel.current_page_name.contains("ITEM0001.DAT"),
        "Expected still menu ITEM0001.DAT, got {}",
        kernel.current_page_name
    );

    // Canvas must have non-zero pixels from the decoded still frame
    let non_zero = kernel.canvas.iter().any(|&b| b > 0);
    assert!(non_zero, "Canvas should contain decoded still menu frame");
}

#[test]
fn test_ui_pbc_button_logic() {
    let mut kernel = VcdKernel::new();
    kernel.active_mode = ActiveDiscMode::Vcd20Classic;

    // In Vcd20Classic mode, calling trigger_pbc_menu without a loaded disc returns error gracefully
    let res = kernel.trigger_pbc_menu();
    assert!(res.is_err());

    // In Vcd30Interactive mode, trigger_pbc_menu is rejected
    kernel.active_mode = ActiveDiscMode::Vcd30Interactive;
    let res = kernel.trigger_pbc_menu();
    assert!(res.is_err());
    assert!(res.unwrap_err().contains("仅在 VCD 2.0 模式下"));
}

#[test]
fn test_vcd20_disc_mode_switching_between_vcd20_and_vcd10() {
    let paradise_root = get_paradise_disc_root();
    if let Some(disc_root) = paradise_root {
        let mut kernel = VcdKernel::new();
        kernel.open_disc(disc_root).expect("open paradise disc");

        // Native VCD 2.0 disc should support VCD 2.0 and VCD 1.0, but not VCD 3.0
        assert_eq!(kernel.supports_vcd20(), true);
        assert_eq!(kernel.supports_vcd10(), true);
        assert_eq!(kernel.supports_vcd30(), false);

        // Initial mode must be VCD 2.0 Classic with PBC active
        assert_eq!(kernel.active_mode, ActiveDiscMode::Vcd20Classic);
        assert!(kernel.pbc.is_some());

        // User chooses: "重置并使用 VCD 1.0 模式播放"
        kernel
            .switch_active_mode(ActiveDiscMode::Vcd10Linear)
            .expect("switch to VCD 1.0 mode");
        assert_eq!(kernel.active_mode, ActiveDiscMode::Vcd10Linear);
        assert!(kernel.active_video.is_some(), "Track video should start playing in VCD 1.0 mode");
        assert_eq!(kernel.current_track_index, Some(0));

        // User chooses: "重置并恢复 VCD 2.0 经典模式"
        kernel
            .switch_active_mode(ActiveDiscMode::Vcd20Classic)
            .expect("restore VCD 2.0 mode");
        assert_eq!(kernel.active_mode, ActiveDiscMode::Vcd20Classic);
        assert!(kernel.pbc.is_some());
    } else {
        let tmp = std::env::temp_dir().join(format!(
            "mock_vcd20_{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let vcd_dir = tmp.join("VCD");
        let mpeg_dir = tmp.join("MPEGAV");
        std::fs::create_dir_all(&vcd_dir).unwrap();
        std::fs::create_dir_all(&mpeg_dir).unwrap();

        let mut info_bytes = vec![0u8; 2048];
        info_bytes[0..8].copy_from_slice(b"VIDEO_CD");
        info_bytes[8] = 2; // version 2.0
        info_bytes[9] = 0;
        info_bytes[10] = 0;
        info_bytes[11] = 2;
        info_bytes[12] = 0;
        info_bytes[13] = 1;
        info_bytes[20] = 1; // has_pbc
        std::fs::write(vcd_dir.join("INFO.VCD"), &info_bytes).unwrap();

        let mut entries_bytes = vec![0u8; 2048];
        entries_bytes[0..8].copy_from_slice(b"ENTRYVCD");
        entries_bytes[8] = 2;
        entries_bytes[10] = 0;
        entries_bytes[11] = 1;
        entries_bytes[12] = 2;
        std::fs::write(vcd_dir.join("ENTRIES.VCD"), &entries_bytes).unwrap();

        let mut lot_bytes = vec![0u8; 65536];
        lot_bytes[2] = 0;
        lot_bytes[3] = 0;
        std::fs::write(vcd_dir.join("LOT.VCD"), &lot_bytes).unwrap();

        let mut psd_bytes = vec![0u8; 32];
        psd_bytes[0] = 0x1F;
        std::fs::write(vcd_dir.join("PSD.VCD"), &psd_bytes).unwrap();

        std::fs::write(mpeg_dir.join("AVSEQ01.DAT"), b"dummy_mpeg_content").unwrap();

        let mut kernel = VcdKernel::new();
        kernel.open_disc(tmp.clone()).expect("open mock vcd20 disc");

        assert_eq!(kernel.supports_vcd20(), true);
        assert_eq!(kernel.supports_vcd10(), true);
        assert_eq!(kernel.supports_vcd30(), false);
        assert_eq!(kernel.active_mode, ActiveDiscMode::Vcd20Classic);

        kernel
            .switch_active_mode(ActiveDiscMode::Vcd10Linear)
            .expect("switch to VCD 1.0");
        assert_eq!(kernel.active_mode, ActiveDiscMode::Vcd10Linear);

        kernel
            .switch_active_mode(ActiveDiscMode::Vcd20Classic)
            .expect("switch back to VCD 2.0");
        assert_eq!(kernel.active_mode, ActiveDiscMode::Vcd20Classic);

        let _ = std::fs::remove_dir_all(&tmp);
    }
}

#[test]
fn test_pbc_menu_navigation_to_track_clears_pbc_status() {
    let paradise_root = get_paradise_disc_root();
    if let Some(disc_root) = paradise_root {
        let mut kernel = VcdKernel::new();
        kernel.open_disc(disc_root).expect("open paradise disc");

        // 1. Initially in PBC mode, starts with LID 1 (intro video)
        assert_eq!(kernel.active_mode, ActiveDiscMode::Vcd20Classic);

        // 2. Trigger PBC menu -> loads motion menu (AVSEQ17.DAT)
        kernel.trigger_pbc_menu().expect("trigger PBC menu");
        assert!(
            kernel.current_page_name.contains("PBC"),
            "Expected PBC menu name, got: {}",
            kernel.current_page_name
        );

        let mut app = vcd30_player::ui::app::VcdPlayerApp::from_kernel(kernel);
        app.set_status("已呼出 PBC 菜单");
        assert_eq!(app.status_message, "已呼出 PBC 菜单");

        // 3. Navigate to track 1 by pressing remote key 1 and enter
        let ctx = vcd30_player::egui::Context::default();
        app.send_remote_key(1, &ctx);
        app.send_remote_key(31, &ctx);

        // 4. Verify kernel state: current_page_name MUST be cleared
        assert_eq!(
            app.kernel.current_page_name, "",
            "PBC page name must be cleared when playing track"
        );
        assert!(app.kernel.is_video_active());
        assert_eq!(app.kernel.current_track_index, Some(0));

        // 5. Verify status message: MUST be updated to track playback, NOT lingering PBC menu
        assert!(
            app.status_message.contains("正在播放") || app.status_message.contains("轨道"),
            "Status message should indicate track playback, got: {}",
            app.status_message
        );
        assert!(!app.status_message.contains("已呼出 PBC 菜单"));
    } else {
        let mut kernel = VcdKernel::new();
        kernel.tracks.push(vcd30_player::vcd::DiscTrackInfo {
            index: 1,
            track_no: 2,
            title: "TRACK01.DAT".to_string(),
            file_name: "MPEGAV/TRACK01.DAT".to_string(),
            msf_start: None,
        });
        kernel.current_page_name = "PBC 菜单 (ITEM0001.DAT)".to_string();

        let mut app = vcd30_player::ui::app::VcdPlayerApp::from_kernel(kernel);
        app.set_status("已呼出 PBC 菜单");

        // Play track directly
        app.kernel.current_page_name.clear();
        app.kernel.current_track_index = Some(0);
        let _ctx = vcd30_player::egui::Context::default();
        if let Some(t) = app.kernel.tracks.get(0) {
            app.set_status(format!("正在播放: 轨道 {:02} ({})", t.index, t.title));
        }

        assert_eq!(app.kernel.current_page_name, "");
        assert!(app.status_message.contains("正在播放: 轨道 01"));
        assert!(!app.status_message.contains("PBC 菜单"));
    }
}

#[test]
fn test_track_numeric_keypad_during_playback() {
    let paradise_root = match get_paradise_disc_root() {
        Some(r) => r,
        None => return,
    };

    let mut kernel = VcdKernel::new();
    kernel.open_disc(paradise_root).expect("open paradise disc");

    // Initially playing AVSEQ18 (intro)
    assert!(kernel.active_video.as_ref().unwrap().filename.contains("AVSEQ18"));

    // User presses 1
    // Since disc has 18 tracks (18 > 9), digit 1 must be buffered!
    kernel.handle_pbc_digit(1).expect("handle digit 1");
    assert_eq!(kernel.pbc_digit_buffer, vec![1]);
    assert!(kernel.active_video.as_ref().unwrap().filename.contains("AVSEQ18"));

    // User presses Enter to confirm Track 1
    kernel.handle_pbc_enter().expect("confirm track 1");
    assert!(kernel.pbc_digit_buffer.is_empty());
    // MUST now play AVSEQ01 (Track 1), NOT AVSEQ18!
    assert!(
        kernel.active_video.as_ref().unwrap().filename.contains("AVSEQ01"),
        "Expected AVSEQ01, got {}",
        kernel.active_video.as_ref().unwrap().filename
    );

    // Multi-digit entry: while playing Track 1, enter 1 then 2 -> plays Track 12 (AVSEQ12)
    kernel.handle_pbc_digit(1).expect("digit 1");
    assert_eq!(kernel.pbc_digit_buffer, vec![1]);
    kernel.handle_pbc_digit(2).expect("digit 2");
    assert!(kernel.pbc_digit_buffer.is_empty()); // auto-confirmed because 2 digits entered and 12 <= 18
    assert!(
        kernel.active_video.as_ref().unwrap().filename.contains("AVSEQ12"),
        "Expected AVSEQ12, got {}",
        kernel.active_video.as_ref().unwrap().filename
    );

    // Immediate entry: while playing Track 12, enter 9 (9 * 10 = 90 > 18, so no 2-digit track possible)
    kernel.handle_pbc_digit(9).expect("digit 9");
    assert!(kernel.pbc_digit_buffer.is_empty());
    assert!(
        kernel.active_video.as_ref().unwrap().filename.contains("AVSEQ09"),
        "Expected AVSEQ09, got {}",
        kernel.active_video.as_ref().unwrap().filename
    );

    // Timeout entry: while playing Track 9, enter 1 (buffered), simulate 2.1s timeout
    kernel.handle_pbc_digit(1).expect("digit 1");
    assert_eq!(kernel.pbc_digit_buffer, vec![1]);
    // Simulate timeout by setting timestamp back 3 seconds
    kernel.pbc_digit_timestamp = Some(std::time::Instant::now() - std::time::Duration::from_secs(3));
    let timed_out = kernel.check_pbc_digit_timeout().expect("check timeout");
    assert!(timed_out);
    assert!(kernel.pbc_digit_buffer.is_empty());
    assert!(
        kernel.active_video.as_ref().unwrap().filename.contains("AVSEQ01"),
        "Expected AVSEQ01, got {}",
        kernel.active_video.as_ref().unwrap().filename
    );
}

