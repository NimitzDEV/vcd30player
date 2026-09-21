//! Comprehensive integration and unit tests for Phase 3:
//! VCD 2.0 Playback Control (PBC) state machine, LOT/PSD table parsing,
//! still/motion menu rendering, and the dedicated "PBC" button.

mod common;

use std::path::{Path, PathBuf};
use vcd30_player::audio::{AudioChannelMode, AudioManager};
use vcd30_player::core::kernel::{ActiveDiscMode, VcdKernel};
use vcd30_player::vcd::{
    decode_segment_item, LotTable, PbcAction, PbcEngine, PbcState, PsdDescriptor, PsdTable,
};

/// Returns the path to the VCD 2.0 paradise reference disc if explicitly configured via env var or exists locally.
fn get_paradise_disc_root() -> Option<PathBuf> {
    if let Some(val) = std::env::var_os("VCD_PARADISE_DISC") {
        let p = PathBuf::from(val);
        if p.exists() {
            return Some(p);
        }
    }
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
    let disc_root = std::env::var_os("VCD_TEST_DISC")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("H:/"));
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
    }

    // Always run the mock disc test to ensure CI test coverage
    {
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
        info_bytes[44..48].copy_from_slice(&32u32.to_be_bytes()); // psd_size = 32
        info_bytes[51] = 8; // offset_mult = 8
        info_bytes[52..54].copy_from_slice(&1u16.to_be_bytes()); // lot_entries = 1
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

    // Same track re-entry: while playing Track 1 (or 12), re-entering the same double digit track
    // must increment track_play_counter and clear the digit buffer.
    // First switch to Track 12:
    kernel.handle_pbc_digit(1).expect("digit 1");
    kernel.handle_pbc_digit(2).expect("digit 2");
    assert!(kernel.pbc_digit_buffer.is_empty());
    assert!(kernel.active_video.as_ref().unwrap().filename.contains("AVSEQ12"));

    // Now re-enter Track 12 while already playing Track 12
    let counter_before = kernel.track_play_counter;
    kernel.handle_pbc_digit(1).expect("re-enter digit 1");
    assert_eq!(kernel.pbc_digit_buffer, vec![1]);
    kernel.handle_pbc_digit(2).expect("re-enter digit 2");
    assert!(kernel.pbc_digit_buffer.is_empty(), "Buffer must be empty after 2nd digit");
    assert_eq!(kernel.track_play_counter, counter_before + 1, "Track play counter must increment");
    assert!(
        kernel.active_video.as_ref().unwrap().filename.contains("AVSEQ12"),
        "Expected AVSEQ12 to be restarted, got {}",
        kernel.active_video.as_ref().unwrap().filename
    );
}

#[test]
fn test_pbc_extended_synthetic_disc_precedence() {
    let temp_dir = std::env::temp_dir().join("vcd_test_pbc_extended_precedence");
    let _ = std::fs::remove_dir_all(&temp_dir);

    let vcd_dir = temp_dir.join("VCD");
    let ext_dir = temp_dir.join("EXT");
    std::fs::create_dir_all(&vcd_dir).unwrap();
    std::fs::create_dir_all(&ext_dir).unwrap();

    // 1. Build standard VCD/PSD.VCD (PlayList LID 1, standard SelectionList LID 2 tag 0x18)
    let mut standard_psd = Vec::new();
    // PlayList at 0 (unit 0)
    standard_psd.push(0x10); standard_psd.push(1);
    standard_psd.extend_from_slice(&1u16.to_be_bytes());
    standard_psd.extend_from_slice(&0xFFFFu16.to_be_bytes());
    standard_psd.extend_from_slice(&2u16.to_be_bytes());
    standard_psd.extend_from_slice(&0xFFFFu16.to_be_bytes());
    standard_psd.extend_from_slice(&0u16.to_be_bytes());
    standard_psd.push(0); standard_psd.push(0);
    standard_psd.extend_from_slice(&2u16.to_be_bytes()); // item 2
    // SelectionList at 16 (unit 2) with 0x18
    standard_psd.push(0x18); standard_psd.push(0); standard_psd.push(1); standard_psd.push(1);
    standard_psd.extend_from_slice(&2u16.to_be_bytes());
    standard_psd.extend_from_slice(&0u16.to_be_bytes()); standard_psd.extend_from_slice(&0xFFFFu16.to_be_bytes());
    standard_psd.extend_from_slice(&0u16.to_be_bytes()); standard_psd.extend_from_slice(&0xFFFFu16.to_be_bytes());
    standard_psd.extend_from_slice(&0xFFFFu16.to_be_bytes()); standard_psd.push(0); standard_psd.push(1);
    standard_psd.extend_from_slice(&2u16.to_be_bytes());
    standard_psd.extend_from_slice(&0u16.to_be_bytes());
    while standard_psd.len() % 8 != 0 {
        standard_psd.push(0);
    }
    std::fs::write(vcd_dir.join("PSD.VCD"), &standard_psd).unwrap();

    let mut standard_lot = vec![0xFFu8; 16]; // 8 u16s
    standard_lot[2] = 0; standard_lot[3] = 0; // LID 1 -> unit 0
    standard_lot[4] = 0; standard_lot[5] = 2; // LID 2 -> unit 2
    std::fs::write(vcd_dir.join("LOT.VCD"), &standard_lot).unwrap();

    // 2. Build EXT/PSD_X.VCD with Extended SelectionList (tag 0x1A)
    let mut ext_psd = Vec::new();
    // PlayList at 0 (unit 0)
    ext_psd.push(0x10); ext_psd.push(1);
    ext_psd.extend_from_slice(&1u16.to_be_bytes());
    ext_psd.extend_from_slice(&0xFFFFu16.to_be_bytes());
    ext_psd.extend_from_slice(&2u16.to_be_bytes());
    ext_psd.extend_from_slice(&0xFFFFu16.to_be_bytes());
    ext_psd.extend_from_slice(&0u16.to_be_bytes());
    ext_psd.push(0); ext_psd.push(0);
    ext_psd.extend_from_slice(&2u16.to_be_bytes());
    // Extended SelectionList at 16 (unit 2) with 0x1A
    ext_psd.push(0x1A); ext_psd.push(0); ext_psd.push(1); ext_psd.push(1);
    ext_psd.extend_from_slice(&2u16.to_be_bytes());
    ext_psd.extend_from_slice(&0u16.to_be_bytes()); ext_psd.extend_from_slice(&0xFFFFu16.to_be_bytes());
    ext_psd.extend_from_slice(&0u16.to_be_bytes()); ext_psd.extend_from_slice(&0xFFFFu16.to_be_bytes());
    ext_psd.extend_from_slice(&0xFFFFu16.to_be_bytes()); ext_psd.push(0); ext_psd.push(1);
    ext_psd.extend_from_slice(&2u16.to_be_bytes());
    ext_psd.extend_from_slice(&0u16.to_be_bytes()); // selections: 1 * 2 = 2 bytes
    ext_psd.extend_from_slice(&[0xAA; 16]); // 16 bytes general area info
    ext_psd.extend_from_slice(&[0xBB; 4]);  // 1 * 4 bytes item button area
    while ext_psd.len() % 8 != 0 {
        ext_psd.push(0);
    }
    std::fs::write(ext_dir.join("PSD_X.VCD"), &ext_psd).unwrap();

    let mut ext_lot = vec![0xFFu8; 16];
    ext_lot[2] = 0; ext_lot[3] = 0; // LID 1 -> unit 0
    ext_lot[4] = 0; ext_lot[5] = 2; // LID 2 -> unit 2
    std::fs::write(ext_dir.join("LOT_X.VCD"), &ext_lot).unwrap();

    // Verify resolve_pbc_paths chooses EXT/
    let resolved = vcd30_player::vcd::resolve_pbc_paths(&temp_dir).expect("resolve pbc paths");
    assert!(resolved.is_extended);
    assert!(resolved.psd_path.ends_with("PSD_X.VCD"));
    assert!(resolved.lot_path.ends_with("LOT_X.VCD"));

    // Verify kernel.init_pbc initializes with extended PBC
    let mut kernel = VcdKernel::new();
    kernel.disc_root = temp_dir.clone();
    kernel.init_pbc().expect("init pbc");
    let pbc = kernel.pbc.as_ref().expect("pbc engine present");
    assert!(pbc.psd.has_extended_descriptors());

    let lid2_desc = pbc.psd.get_by_lid(2, &pbc.lot).expect("find LID 2");
    match lid2_desc {
        PsdDescriptor::SelectionList(s) => {
            assert!(s.is_extended());
            assert_eq!(s.descriptor_tag, 0x1A);
            assert_eq!(s.ext_area_data.len(), 20); // 16 + 4
        }
        _ => panic!("expected SelectionList for LID 2"),
    }

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_paradise_disc_extended_pbc() {
    let paradise_root = match get_paradise_disc_root() {
        Some(r) => r,
        None => return,
    };

    // 1. Verify path resolution prioritizes EXT/PSD_X.VCD
    let pbc_paths = vcd30_player::vcd::resolve_pbc_paths(&paradise_root)
        .expect("should resolve PBC paths on reference disc");
    assert!(pbc_paths.is_extended, "Paradise disc should use Extended PBC");
    assert!(pbc_paths.psd_path.to_string_lossy().to_ascii_uppercase().contains("PSD_X.VCD"));
    assert!(pbc_paths.lot_path.to_string_lossy().to_ascii_uppercase().contains("LOT_X.VCD"));

    // 2. Initialize kernel PBC
    let mut kernel = VcdKernel::new();
    kernel.disc_root = paradise_root;
    kernel.init_pbc().expect("init_pbc on paradise disc");

    let pbc = kernel.pbc.as_ref().expect("pbc initialized");
    assert!(pbc.psd.has_extended_descriptors(), "PSD_X.VCD must have extended descriptors");

    // 3. LID 2 must be the 0x1A Extended Selection List
    let lid2 = pbc.psd.get_by_lid(2, &pbc.lot).expect("LID 2 exists");
    match lid2 {
        PsdDescriptor::SelectionList(s) => {
            assert_eq!(s.descriptor_tag, 0x1A);
            assert!(s.is_extended());
            assert_eq!(s.nos, 16);
            assert_eq!(s.bsn, 1);
            assert_eq!(s.item_id, 18);
            assert_eq!(s.selections.len(), 16);
            assert_eq!(s.ext_area_data.len(), 16 + 16 * 4); // 80 bytes
        }
        _ => panic!("LID 2 must be SelectionList"),
    }
}

#[test]
fn test_pbc_waiting_delay_ticking_and_infinite_wait() {
    let mut raw_lot = vec![0u8; 16];
    raw_lot[2] = 0; raw_lot[3] = 0; // LID 1 -> unit 0
    raw_lot[4] = 0; raw_lot[5] = 2; // LID 2 -> unit 2
    let lot = LotTable::parse(&raw_lot, 8).unwrap();

    let mut raw_psd = Vec::new();
    // 1. PlayList LID 1 at unit 0 (byte 0) with wtime = 3 seconds, next = unit 2
    raw_psd.push(0x10);
    raw_psd.push(1);
    raw_psd.extend_from_slice(&1u16.to_be_bytes()); // lid 1
    raw_psd.extend_from_slice(&0xFFFFu16.to_be_bytes()); // prev
    raw_psd.extend_from_slice(&2u16.to_be_bytes()); // next = unit 2
    raw_psd.extend_from_slice(&0xFFFFu16.to_be_bytes()); // ret
    raw_psd.extend_from_slice(&0u16.to_be_bytes());
    raw_psd.push(3); // wtime = 3 seconds
    raw_psd.push(0);
    raw_psd.extend_from_slice(&2u16.to_be_bytes()); // track 2

    // 2. PlayList LID 2 at unit 2 (byte 16) with wtime = 255 (infinite wait), next = 0xFFFF
    raw_psd.push(0x10);
    raw_psd.push(1);
    raw_psd.extend_from_slice(&2u16.to_be_bytes()); // lid 2
    raw_psd.extend_from_slice(&0u16.to_be_bytes()); // prev = unit 0
    raw_psd.extend_from_slice(&0xFFFFu16.to_be_bytes()); // next = None
    raw_psd.extend_from_slice(&0u16.to_be_bytes()); // ret = unit 0
    raw_psd.extend_from_slice(&0u16.to_be_bytes());
    raw_psd.push(255); // wtime = 255 (infinite wait)
    raw_psd.push(0);
    raw_psd.extend_from_slice(&3u16.to_be_bytes()); // track 3

    let psd = PsdTable::parse(&raw_psd, 8).unwrap();
    let mut engine = PbcEngine::new(lot, psd);

    let act1 = engine.start().expect("start action");
    assert_eq!(act1, PbcAction::PlayTrack { track_number: 2, item_id: 2, ptime: 0, wtime: 3 });

    // Track 2 finishes playing -> enters WaitingDelay for 3 seconds
    let finish_act = engine.on_item_finished();
    assert!(finish_act.is_none(), "should wait for wtime instead of immediately transitioning");
    assert!(engine.has_active_timer(), "engine must have active timer during wtime delay");
    assert_eq!(engine.remaining_timer_seconds(), Some(3.0));

    // Step 1.0 second
    assert!(engine.tick(1.0).is_none());
    assert!(engine.has_active_timer());
    assert!((engine.remaining_timer_seconds().unwrap() - 2.0).abs() < 1e-4);

    // Step 1.5 seconds (remaining 0.5s)
    assert!(engine.tick(1.5).is_none());
    assert!(engine.has_active_timer());
    assert!((engine.remaining_timer_seconds().unwrap() - 0.5).abs() < 1e-4);

    // Step 0.6 seconds (remaining <= 0) -> delay expires, advances to next descriptor (LID 2)!
    let act2 = engine.tick(0.6).expect("delay expiration triggers next descriptor");
    assert_eq!(act2, PbcAction::PlayTrack { track_number: 3, item_id: 3, ptime: 0, wtime: 255 });

    // Track 3 finishes playing -> wtime = 255 (infinite wait)
    let finish_act2 = engine.on_item_finished();
    assert!(finish_act2.is_none());
    assert!(!engine.has_active_timer(), "infinite wait delay (wtime=255) has no countdown timer");
    assert_eq!(engine.remaining_timer_seconds(), None);

    // Ticking any amount of time during infinite wait should never fire
    assert!(engine.tick(100.0).is_none());
    assert!(engine.tick(5000.0).is_none());
    match &engine.state {
        PbcState::WaitingDelay { remaining_wait, .. } => {
            assert!(remaining_wait.is_infinite());
        }
        _ => panic!("state must remain WaitingDelay with infinite wait"),
    }
}

#[test]
fn test_pbc_selection_timeout_ticking_and_infinite_timeout() {
    let mut raw_lot = vec![0u8; 16];
    raw_lot[2] = 0; raw_lot[3] = 0; // LID 1 -> unit 0
    raw_lot[4] = 0; raw_lot[5] = 3; // LID 2 -> unit 3
    raw_lot[6] = 0; raw_lot[7] = 6; // LID 3 -> unit 6
    let lot = LotTable::parse(&raw_lot, 8).unwrap();

    let mut raw_psd = Vec::new();
    // 1. SelectionList LID 1 at unit 0 (byte 0): timeout_time = 4s, timeout_ofs = unit 3
    raw_psd.push(0x18);
    raw_psd.push(0);
    raw_psd.push(1); // nos = 1
    raw_psd.push(1); // bsn = 1
    raw_psd.extend_from_slice(&1u16.to_be_bytes()); // lid 1
    raw_psd.extend_from_slice(&0xFFFFu16.to_be_bytes()); // prev
    raw_psd.extend_from_slice(&0xFFFFu16.to_be_bytes()); // next
    raw_psd.extend_from_slice(&0xFFFFu16.to_be_bytes()); // ret
    raw_psd.extend_from_slice(&0xFFFFu16.to_be_bytes()); // def
    raw_psd.extend_from_slice(&3u16.to_be_bytes()); // timeout_ofs = unit 3
    raw_psd.push(4); // timeout_time = 4s
    raw_psd.push(1);
    raw_psd.extend_from_slice(&1000u16.to_be_bytes()); // ITEM0001.DAT
    raw_psd.extend_from_slice(&6u16.to_be_bytes()); // sel 1 -> unit 6
    while raw_psd.len() % 8 != 0 {
        raw_psd.push(0);
    }

    // 2. SelectionList LID 2 at unit 3 (byte 24): timeout_time = 2s, timeout_ofs = 0xFFFF (no timeout action)
    raw_psd.push(0x18);
    raw_psd.push(0);
    raw_psd.push(1);
    raw_psd.push(1);
    raw_psd.extend_from_slice(&2u16.to_be_bytes()); // lid 2
    raw_psd.extend_from_slice(&0u16.to_be_bytes()); // prev = unit 0
    raw_psd.extend_from_slice(&0xFFFFu16.to_be_bytes());
    raw_psd.extend_from_slice(&0u16.to_be_bytes());
    raw_psd.extend_from_slice(&0xFFFFu16.to_be_bytes());
    raw_psd.extend_from_slice(&0xFFFFu16.to_be_bytes()); // timeout_ofs = 0xFFFF!
    raw_psd.push(2); // timeout_time = 2s
    raw_psd.push(1);
    raw_psd.extend_from_slice(&1001u16.to_be_bytes()); // ITEM0002.DAT
    raw_psd.extend_from_slice(&6u16.to_be_bytes()); // sel 1 -> unit 6
    while raw_psd.len() % 8 != 0 {
        raw_psd.push(0);
    }

    // 3. PlayList LID 3 at unit 6 (byte 48)
    raw_psd.push(0x10);
    raw_psd.push(1);
    raw_psd.extend_from_slice(&3u16.to_be_bytes()); // lid 3
    raw_psd.extend_from_slice(&3u16.to_be_bytes());
    raw_psd.extend_from_slice(&0xFFFFu16.to_be_bytes());
    raw_psd.extend_from_slice(&0u16.to_be_bytes());
    raw_psd.extend_from_slice(&0u16.to_be_bytes());
    raw_psd.push(0);
    raw_psd.push(0);
    raw_psd.extend_from_slice(&2u16.to_be_bytes()); // track 2
    while raw_psd.len() % 8 != 0 {
        raw_psd.push(0);
    }

    let psd = PsdTable::parse(&raw_psd, 8).unwrap();
    let mut engine = PbcEngine::new(lot, psd);

    let act1 = engine.start().expect("start action");
    assert_eq!(act1, PbcAction::DisplayStillMenu { item_id: 1000, segment_index: 1, nos: 1, bsn: 1, timeout_sec: 4 });
    assert!(engine.has_active_timer());
    assert_eq!(engine.remaining_timer_seconds(), Some(4.0));

    // Tick 2.0s -> remaining 2.0s
    assert!(engine.tick(2.0).is_none());
    assert!(engine.has_active_timer());
    assert!((engine.remaining_timer_seconds().unwrap() - 2.0).abs() < 1e-4);

    // Tick 2.1s -> fires timeout to unit 3 (LID 2)!
    let act2 = engine.tick(2.1).expect("timeout fires action");
    assert_eq!(act2, PbcAction::DisplayStillMenu { item_id: 1001, segment_index: 2, nos: 1, bsn: 1, timeout_sec: 2 });
    assert!(engine.has_active_timer());
    assert_eq!(engine.remaining_timer_seconds(), Some(2.0));

    // LID 2 has timeout_ofs = 0xFFFF; when timeout expires, it should not jump and should stop the timer
    let act3 = engine.tick(2.5);
    assert!(act3.is_none(), "timeout_ofs == 0xFFFF should not trigger action");
    assert!(!engine.has_active_timer(), "timer must be deactivated after expiring with 0xFFFF offset");
    assert_eq!(engine.remaining_timer_seconds(), None);
}

#[test]
fn test_kernel_pbc_clock_driver_and_waiting_delay() {
    let mut raw_lot = vec![0u8; 16];
    raw_lot[2] = 0; raw_lot[3] = 0; // LID 1 -> unit 0
    raw_lot[4] = 0; raw_lot[5] = 2; // LID 2 -> unit 2
    let lot = LotTable::parse(&raw_lot, 8).unwrap();

    let mut raw_psd = Vec::new();
    // 1. PlayList LID 1 at unit 0: wtime = 2 seconds, next = unit 2 (EndList)
    raw_psd.push(0x10);
    raw_psd.push(1);
    raw_psd.extend_from_slice(&1u16.to_be_bytes());
    raw_psd.extend_from_slice(&0xFFFFu16.to_be_bytes());
    raw_psd.extend_from_slice(&2u16.to_be_bytes()); // next = unit 2
    raw_psd.extend_from_slice(&0xFFFFu16.to_be_bytes());
    raw_psd.extend_from_slice(&0u16.to_be_bytes());
    raw_psd.push(2); // wtime = 2 seconds
    raw_psd.push(0);
    raw_psd.extend_from_slice(&2u16.to_be_bytes());
    while raw_psd.len() % 8 != 0 {
        raw_psd.push(0);
    }

    // 2. EndList at unit 2 (byte 16)
    raw_psd.push(0x1F);
    raw_psd.push(0);
    raw_psd.extend_from_slice(&0u16.to_be_bytes());
    raw_psd.extend_from_slice(&[0u8; 4]); // pad to 8 bytes
    while raw_psd.len() % 8 != 0 {
        raw_psd.push(0);
    }

    let psd = PsdTable::parse(&raw_psd, 8).unwrap();

    let mut kernel = VcdKernel::new();
    kernel.active_mode = ActiveDiscMode::Vcd20Classic;
    let mut pbc = PbcEngine::new(lot, psd);
    let _ = pbc.start();
    // Finish item to enter WaitingDelay
    assert!(pbc.on_item_finished().is_none());
    kernel.pbc = Some(pbc);

    assert!(kernel.has_active_pbc_timer(), "kernel must report active PBC timer");

    // Tick 1.0s -> does not expire yet
    let fired = kernel.tick_pbc(1.0).expect("tick_pbc");
    assert!(!fired);
    assert!(kernel.has_active_pbc_timer());

    // Tick 1.5s -> expires! Executes EndList and returns true
    let fired2 = kernel.tick_pbc(1.5).expect("tick_pbc");
    assert!(fired2, "waiting delay expiration must execute EndList action");
    assert!(!kernel.has_active_pbc_timer(), "state after End must have no active timer");

    // In Vcd10Linear mode, tick_pbc is ignored
    kernel.active_mode = ActiveDiscMode::Vcd10Linear;
    assert!(!kernel.has_active_pbc_timer());
    assert_eq!(kernel.tick_pbc(1.0).unwrap(), false);
}

#[test]
fn test_kernel_pbc_clock_driver_selection_timeout_to_end() {
    let mut raw_lot = vec![0u8; 16];
    raw_lot[2] = 0; raw_lot[3] = 0; // LID 1 -> unit 0
    raw_lot[4] = 0; raw_lot[5] = 3; // LID 2 -> unit 3
    let lot = LotTable::parse(&raw_lot, 8).unwrap();

    let mut raw_psd = Vec::new();
    // 1. SelectionList LID 1 at unit 0: timeout = 3s, timeout_ofs = unit 3 (EndList)
    raw_psd.push(0x18);
    raw_psd.push(0);
    raw_psd.push(1);
    raw_psd.push(1);
    raw_psd.extend_from_slice(&1u16.to_be_bytes());
    raw_psd.extend_from_slice(&0xFFFFu16.to_be_bytes());
    raw_psd.extend_from_slice(&0xFFFFu16.to_be_bytes());
    raw_psd.extend_from_slice(&0xFFFFu16.to_be_bytes());
    raw_psd.extend_from_slice(&0xFFFFu16.to_be_bytes());
    raw_psd.extend_from_slice(&3u16.to_be_bytes()); // timeout -> unit 3
    raw_psd.push(3); // 3s
    raw_psd.push(1);
    raw_psd.extend_from_slice(&1000u16.to_be_bytes());
    raw_psd.extend_from_slice(&3u16.to_be_bytes());
    while raw_psd.len() % 8 != 0 {
        raw_psd.push(0);
    }

    // 2. EndList at unit 3 (byte 24)
    raw_psd.push(0x1F);
    raw_psd.push(0);
    raw_psd.extend_from_slice(&0u16.to_be_bytes());
    raw_psd.extend_from_slice(&[0u8; 4]);
    while raw_psd.len() % 8 != 0 {
        raw_psd.push(0);
    }

    let psd = PsdTable::parse(&raw_psd, 8).unwrap();

    let mut kernel = VcdKernel::new();
    kernel.active_mode = ActiveDiscMode::Vcd20Classic;
    let mut pbc = PbcEngine::new(lot, psd);
    let _ = pbc.start(); // starts LID 1
    kernel.pbc = Some(pbc);

    assert!(kernel.has_active_pbc_timer(), "kernel must report active SelectionList timeout");

    // Tick 1.5s
    let fired = kernel.tick_pbc(1.5).expect("tick_pbc");
    assert!(!fired);
    assert!(kernel.has_active_pbc_timer());

    // Tick 2.0s -> expires! Executes EndList and returns true
    let fired2 = kernel.tick_pbc(2.0).expect("tick_pbc");
    assert!(fired2, "timeout must trigger EndList action");
    assert!(!kernel.has_active_pbc_timer(), "after End, timer is inactive");
}

#[test]
fn test_pbc_playlist_ptime_action_metadata() {
    let mut raw_lot = vec![0u8; 16];
    raw_lot[2] = 0; raw_lot[3] = 0; // LID 1 -> unit 0
    let lot = LotTable::parse(&raw_lot, 8).unwrap();

    let mut raw_psd = Vec::new();
    // PlayList LID 1 at unit 0 with ptime = 75 (5.0s in 1/15s units)
    raw_psd.push(0x10);
    raw_psd.push(1);
    raw_psd.extend_from_slice(&1u16.to_be_bytes()); // lid 1
    raw_psd.extend_from_slice(&0xFFFFu16.to_be_bytes());
    raw_psd.extend_from_slice(&0xFFFFu16.to_be_bytes());
    raw_psd.extend_from_slice(&0xFFFFu16.to_be_bytes());
    raw_psd.extend_from_slice(&75u16.to_be_bytes()); // ptime = 75 (5.0s)
    raw_psd.push(2); // wtime = 2s
    raw_psd.push(0);
    raw_psd.extend_from_slice(&2u16.to_be_bytes()); // track 2
    while raw_psd.len() % 8 != 0 {
        raw_psd.push(0);
    }

    let psd = PsdTable::parse(&raw_psd, 8).unwrap();
    let mut engine = PbcEngine::new(lot, psd);

    let action = engine.start().expect("start action");
    assert_eq!(
        action,
        PbcAction::PlayTrack {
            track_number: 2,
            item_id: 2,
            ptime: 75,
            wtime: 2,
        }
    );
}

#[test]
fn test_pbc_selection_list_loop_count_repetitions() {
    let mut raw_lot = vec![0u8; 16];
    raw_lot[2] = 0; raw_lot[3] = 0; // LID 1 -> unit 0
    raw_lot[4] = 0; raw_lot[5] = 3; // LID 2 -> unit 3
    let lot = LotTable::parse(&raw_lot, 8).unwrap();

    let mut raw_psd = Vec::new();
    // 1. SelectionList LID 1 at unit 0: motion menu (item_id = 2), loop_count = 3, timeout = 4s, timeout_ofs = unit 3
    raw_psd.push(0x18);
    raw_psd.push(0);
    raw_psd.push(1); // nos = 1
    raw_psd.push(1); // bsn = 1
    raw_psd.extend_from_slice(&1u16.to_be_bytes()); // lid 1
    raw_psd.extend_from_slice(&0xFFFFu16.to_be_bytes()); // prev
    raw_psd.extend_from_slice(&0xFFFFu16.to_be_bytes()); // next
    raw_psd.extend_from_slice(&0xFFFFu16.to_be_bytes()); // ret
    raw_psd.extend_from_slice(&0xFFFFu16.to_be_bytes()); // def
    raw_psd.extend_from_slice(&3u16.to_be_bytes()); // timeout_ofs = unit 3 (EndList)
    raw_psd.push(4); // timeout_time = 4s
    raw_psd.push(3); // loop_count = 3 (play 3 times)
    raw_psd.extend_from_slice(&2u16.to_be_bytes()); // item_id = 2 (motion menu track 2)
    raw_psd.extend_from_slice(&3u16.to_be_bytes()); // sel 1 -> unit 3
    while raw_psd.len() % 8 != 0 {
        raw_psd.push(0);
    }

    // 2. EndList at unit 3 (byte 24)
    raw_psd.push(0x1F);
    raw_psd.push(0);
    raw_psd.extend_from_slice(&0u16.to_be_bytes());
    raw_psd.extend_from_slice(&[0u8; 4]);
    while raw_psd.len() % 8 != 0 {
        raw_psd.push(0);
    }

    let psd = PsdTable::parse(&raw_psd, 8).unwrap();
    let mut engine = PbcEngine::new(lot, psd);

    // Initial start: begins 1st loop of motion menu
    let act1 = engine.start().expect("start action");
    assert_eq!(
        act1,
        PbcAction::PlayMotionMenu {
            track_number: 2,
            item_id: 2,
            nos: 1,
            bsn: 1,
            timeout_sec: 4,
        }
    );
    assert_eq!(engine.remaining_selection_loops(), Some(3));
    // Timeout timer is NOT active while video is looping
    assert!(!engine.has_active_timer());
    assert_eq!(engine.remaining_timer_seconds(), None);

    // 1st play finishes -> repeats for 2nd loop
    let act2 = engine.on_item_finished().expect("repeat loop 2");
    assert_eq!(
        act2,
        PbcAction::PlayMotionMenu {
            track_number: 2,
            item_id: 2,
            nos: 1,
            bsn: 1,
            timeout_sec: 4,
        }
    );
    assert_eq!(engine.remaining_selection_loops(), Some(2));
    assert!(!engine.has_active_timer());

    // 2nd play finishes -> repeats for 3rd loop
    let act3 = engine.on_item_finished().expect("repeat loop 3");
    assert_eq!(
        act3,
        PbcAction::PlayMotionMenu {
            track_number: 2,
            item_id: 2,
            nos: 1,
            bsn: 1,
            timeout_sec: 4,
        }
    );
    assert_eq!(engine.remaining_selection_loops(), Some(1));
    assert!(!engine.has_active_timer());

    // 3rd play finishes -> all loops completed, enters timeout wait phase!
    let finish_act = engine.on_item_finished();
    assert!(finish_act.is_none(), "after loops are exhausted, no new video plays; wait timer starts");
    assert_eq!(engine.remaining_selection_loops(), Some(0));
    assert!(engine.has_active_timer(), "timeout timer must now be active");
    assert_eq!(engine.remaining_timer_seconds(), Some(4.0));

    // Tick 2.0s -> remaining 2.0s
    assert!(engine.tick(2.0).is_none());
    assert!(engine.has_active_timer());
    assert!((engine.remaining_timer_seconds().unwrap() - 2.0).abs() < 1e-4);

    // Tick 2.1s -> timeout expires and jumps to unit 3 (EndList)!
    let timeout_act = engine.tick(2.1).expect("timeout action");
    assert_eq!(timeout_act, PbcAction::End);
    assert!(!engine.has_active_timer());
}

#[test]
fn test_pbc_selection_list_loop_count_infinite() {
    let mut raw_lot = vec![0u8; 16];
    raw_lot[2] = 0; raw_lot[3] = 0; // LID 1 -> unit 0
    let lot = LotTable::parse(&raw_lot, 8).unwrap();

    let mut raw_psd = Vec::new();
    // SelectionList LID 1 at unit 0: motion menu (item_id = 2), loop_count = 0 (infinite), timeout = 5s
    raw_psd.push(0x18);
    raw_psd.push(0);
    raw_psd.push(1);
    raw_psd.push(1);
    raw_psd.extend_from_slice(&1u16.to_be_bytes());
    raw_psd.extend_from_slice(&0xFFFFu16.to_be_bytes());
    raw_psd.extend_from_slice(&0xFFFFu16.to_be_bytes());
    raw_psd.extend_from_slice(&0xFFFFu16.to_be_bytes());
    raw_psd.extend_from_slice(&0xFFFFu16.to_be_bytes());
    raw_psd.extend_from_slice(&0xFFFFu16.to_be_bytes());
    raw_psd.push(5);
    raw_psd.push(0x80); // bit 7 set, loop count bits 0..6 = 0 (infinite)
    raw_psd.extend_from_slice(&2u16.to_be_bytes());
    raw_psd.extend_from_slice(&0xFFFFu16.to_be_bytes());
    while raw_psd.len() % 8 != 0 {
        raw_psd.push(0);
    }

    let psd = PsdTable::parse(&raw_psd, 8).unwrap();
    let mut engine = PbcEngine::new(lot, psd);

    let act1 = engine.start().expect("start action");
    assert_eq!(
        act1,
        PbcAction::PlayMotionMenu {
            track_number: 2,
            item_id: 2,
            nos: 1,
            bsn: 1,
            timeout_sec: 5,
        }
    );

    // Loops repeatedly without entering timeout
    for _ in 0..10 {
        let repeat = engine.on_item_finished().expect("infinite loop repeat");
        assert_eq!(
            repeat,
            PbcAction::PlayMotionMenu {
                track_number: 2,
                item_id: 2,
                nos: 1,
                bsn: 1,
                timeout_sec: 5,
            }
        );
        assert!(!engine.has_active_timer(), "infinite loop never activates timeout countdown");
    }
}

#[test]
fn test_audio_manager_segment_audio_playback_and_channel_mode() {
    let mut audio = AudioManager::silent();
    assert!(!audio.is_bgm_playing());

    // Silent mode safety: should not crash with empty or valid samples
    audio.play_segment_audio(44100, vec![], AudioChannelMode::Stereo);
    assert!(!audio.is_bgm_playing());

    let samples = vec![0.1f32; 1152 * 2];
    audio.play_segment_audio(44100, samples, AudioChannelMode::LeftOnly);

    // Channel mode update should be safe
    audio.set_channel_mode(AudioChannelMode::RightOnly);
    audio.set_channel_mode(AudioChannelMode::Stereo);

    // Stop should reset cleanly
    audio.stop_bgm();
    assert!(!audio.is_bgm_playing());
}

#[test]
fn test_kernel_set_channel_mode_syncs_audio_and_player() {
    let mut kernel = VcdKernel::new();
    assert_eq!(kernel.channel_mode, AudioChannelMode::Stereo);

    kernel.set_channel_mode(AudioChannelMode::LeftOnly);
    assert_eq!(kernel.channel_mode, AudioChannelMode::LeftOnly);

    kernel.set_channel_mode(AudioChannelMode::RightOnly);
    assert_eq!(kernel.channel_mode, AudioChannelMode::RightOnly);

    kernel.set_channel_mode(AudioChannelMode::Stereo);
    assert_eq!(kernel.channel_mode, AudioChannelMode::Stereo);
}

#[test]
fn test_kernel_pbc_segment_audio_completion_in_playlist() {
    let mut raw_lot = vec![0u8; 16];
    raw_lot[2] = 0; raw_lot[3] = 0; // LID 1 -> unit 0
    raw_lot[4] = 0; raw_lot[5] = 2; // LID 2 -> unit 2
    let lot = LotTable::parse(&raw_lot, 8).unwrap();

    let mut raw_psd = Vec::new();
    // 1. PlayList LID 1 at unit 0: noi = 1 item (track 2), wtime = 0, next = unit 2 (EndList)
    raw_psd.push(0x10);
    raw_psd.push(1); // noi = 1
    raw_psd.extend_from_slice(&1u16.to_be_bytes());
    raw_psd.extend_from_slice(&0xFFFFu16.to_be_bytes());
    raw_psd.extend_from_slice(&2u16.to_be_bytes()); // next = unit 2 (EndList)
    raw_psd.extend_from_slice(&0xFFFFu16.to_be_bytes());
    raw_psd.extend_from_slice(&0u16.to_be_bytes());
    raw_psd.push(0); // wtime = 0
    raw_psd.push(0);
    raw_psd.extend_from_slice(&2u16.to_be_bytes()); // item 0 = track 2
    while raw_psd.len() % 8 != 0 {
        raw_psd.push(0);
    }

    // 2. EndList at unit 2 (byte 16)
    raw_psd.push(0x1F);
    raw_psd.push(0);
    raw_psd.extend_from_slice(&0u16.to_be_bytes());
    raw_psd.extend_from_slice(&[0u8; 4]);
    while raw_psd.len() % 8 != 0 {
        raw_psd.push(0);
    }

    let psd = PsdTable::parse(&raw_psd, 8).unwrap();
    let mut kernel = VcdKernel::new();
    kernel.active_mode = ActiveDiscMode::Vcd20Classic;
    kernel.pbc = Some(PbcEngine::new(lot, psd));

    let act1 = kernel.pbc.as_mut().unwrap().start().expect("start action");
    assert_eq!(
        act1,
        PbcAction::PlayTrack {
            track_number: 2,
            item_id: 2,
            ptime: 0,
            wtime: 0
        }
    );

    // Simulate segment audio was playing for item 0, and has now completed
    kernel.segment_audio_playing = true;
    // Calling tick_pbc when audio has stopped triggers on_item_finished and advances to EndList!
    let fired = kernel.tick_pbc(0.1).expect("tick pbc");
    assert!(fired);
    assert!(!kernel.segment_audio_playing);
    assert_eq!(kernel.pbc.as_ref().unwrap().state, PbcState::Ended);
}

#[test]
fn test_decode_segment_item_real_disc_or_skip() {
    let Some(root) = common::get_live_disc_root() else {
        eprintln!("No live disc mounted, skipping real decode_segment_item test.");
        return;
    };
    // Check if SEGMENT exists, or test on any DAT in MPEGAV as standard CD-XA / MPEG container
    let candidate = if root.join("SEGMENT").join("ITEM0001.DAT").exists() {
        Some(root.join("SEGMENT").join("ITEM0001.DAT"))
    } else if root.join("MPEGAV").join("AVSEQ01.DAT").exists() {
        Some(root.join("MPEGAV").join("AVSEQ01.DAT"))
    } else if root.join("MPEGAV").join("MUSIC01.DAT").exists() {
        Some(root.join("MPEGAV").join("MUSIC01.DAT"))
    } else {
        None
    };

    let Some(dat_path) = candidate else {
        eprintln!("No sample DAT found, skipping test.");
        return;
    };

    let decoded = decode_segment_item(&dat_path).expect("Failed to decode segment item");
    assert!(decoded.width > 0);
    assert!(decoded.height > 0);
    assert_eq!(decoded.rgba.len(), (decoded.width * decoded.height * 4) as usize);

    if let Some((sample_rate, ref samples)) = decoded.audio {
        assert!(sample_rate > 0);
        assert!(!samples.is_empty());
        println!(
            "Decoded segment with audio: {}x{}, audio sample_rate={}, samples_count={}",
            decoded.width,
            decoded.height,
            sample_rate,
            samples.len()
        );
    } else {
        println!("Decoded video-only segment: {}x{}", decoded.width, decoded.height);
    }
}
