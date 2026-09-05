mod common;

use vcd30_player::assets::chm::CompHtmlDoc;
use vcd30_player::assets::cls::AutoRunConfig;
use vcd30_player::assets::ybm::YbmImage;

#[test]
fn test_decode_cover_ybm() {
    let disc_root = common::get_test_disc_root();
    let path = disc_root
        .join("PROGRAM")
        .join("JAVA")
        .join("COVER.YBM");
    if !path.exists() {
        eprintln!(
            "Warning: {} does not exist, skipping disc test",
            path.display()
        );
        return;
    }

    let bytes = std::fs::read(&path).expect("Failed to read COVER.YBM");
    let img = YbmImage::decode(&bytes).expect("Failed to decode COVER.YBM");

    assert_eq!(img.width, 352);
    assert_eq!(img.height, 288);
    assert_eq!(img.pixels.len(), 352 * 288);

    let rgba = img.to_rgba8();
    assert_eq!(rgba.len(), 352 * 288 * 4);

    // Top-left pixel should be background (black)
    assert!(rgba[0] < 30 && rgba[1] < 30 && rgba[2] < 30);

    // Center pixel area should contain bright pixels from "VCD 3.0"
    let mut found_bright = false;
    for y in 120..160 {
        for x in 150..200 {
            let idx = (y * 352 + x) * 4;
            if rgba[idx] > 200 || rgba[idx + 1] > 200 {
                found_bright = true;
                break;
            }
        }
        if found_bright {
            break;
        }
    }
    assert!(
        found_bright,
        "Did not find expected bright text pixels in COVER center!"
    );
}

#[test]
fn test_decode_ssun_and_stamp_ybm() {
    let disc_root = common::get_test_disc_root();
    let data_dir = disc_root.join("DATA").join("VCD_DATA");
    if !data_dir.exists() {
        return;
    }

    for name in &[
        "SSUN.YBM",
        "STAMP.YBM",
        "HOMEPAGE.YBM",
        "T_B.YBM",
        "W_MAN.YBM",
        "T_DOT.YBM",
    ] {
        let p = data_dir.join(name);
        if !p.exists() {
            continue;
        }
        let bytes = std::fs::read(&p).unwrap();
        let img = YbmImage::decode(&bytes).unwrap();
        assert!(img.width > 0 && img.height > 0);
        assert_eq!(img.pixels.len(), (img.width * img.height) as usize);

        let rgba = img.to_rgba8();
        assert_eq!(rgba.len(), (img.width * img.height * 4) as usize);
    }
}

#[test]
fn test_canvas_blitting() {
    let disc_root = common::get_test_disc_root();
    let mut canvas = vec![0u8; 352 * 288 * 4];
    let data_dir = disc_root.join("DATA").join("VCD_DATA");
    if !data_dir.exists() {
        return;
    }

    let dot_path = data_dir.join("T_DOT.YBM");
    if dot_path.exists() {
        let bytes = std::fs::read(&dot_path).unwrap();
        let dot_img = YbmImage::decode(&bytes).unwrap();
        // Blit sprite at (50, 50)
        dot_img.blit_to_rgba_canvas(&mut canvas, 352, 288, 50, 50, None);
        // Verify pixel at (50, 50) is modified
        let idx = (50 * 352 + 50) * 4;
        assert_eq!(canvas[idx + 3], 255);
    }
}

#[test]
fn test_parse_chm_homepage() {
    let disc_root = common::get_test_disc_root();
    let p = disc_root
        .join("DATA")
        .join("VCD_DATA")
        .join("HOMEPAGE.CHM");
    if !p.exists() {
        return;
    }

    let bytes = std::fs::read(&p).expect("Failed to read HOMEPAGE.CHM");
    let doc = CompHtmlDoc::parse(&bytes).expect("Failed to parse HOMEPAGE.CHM");

    assert_eq!(doc.width, 352);
    assert_eq!(doc.height, 288);
    assert_eq!(doc.get_background_image(), Some("HOMEPAGE.YBM"));

    let hotspots = doc.get_all_hotspots();
    assert!(
        !hotspots.is_empty(),
        "HOMEPAGE.CHM should have hotspot areas"
    );
    assert!(
        hotspots.iter().any(|h| h.target.contains("HOME.CHM")),
        "HOMEPAGE.CHM should route to HOME.CHM"
    );
}

#[test]
fn test_parse_chm_with_vcdscript() {
    let disc_root = common::get_test_disc_root();
    let data_dir = disc_root.join("DATA").join("VCD_DATA");
    if !data_dir.exists() {
        return;
    }

    // T_B.CHM has VCDSCRIPT
    let tb_path = data_dir.join("T_B.CHM");
    if tb_path.exists() {
        let bytes = std::fs::read(&tb_path).unwrap();
        let doc = CompHtmlDoc::parse(&bytes).unwrap();
        let script = doc.get_script().expect("T_B.CHM must have VCDSCRIPT");
        assert!(script.contains("KARAOKE SET"));
        assert!(script.contains("DRAWIMAGE"));
        assert!(script.contains("GOSUB"));
    }

    // WEIGHT.CHM has VCDSCRIPT
    let weight_path = data_dir.join("WEIGHT.CHM");
    if weight_path.exists() {
        let bytes = std::fs::read(&weight_path).unwrap();
        let doc = CompHtmlDoc::parse(&bytes).unwrap();
        let script = doc.get_script().expect("WEIGHT.CHM must have VCDSCRIPT");
        assert!(script.contains("CALL IRKEY"));
        assert!(script.contains("DRAWCURSOR"));
        assert!(script.contains("PLAYSOUND"));
    }
}

#[test]
fn test_batch_parse_all_disc_chms() {
    let disc_root = common::get_test_disc_root();
    let data_dir = disc_root.join("DATA").join("VCD_DATA");
    if !data_dir.exists() {
        return;
    }

    let mut count = 0;
    for entry in std::fs::read_dir(&data_dir).unwrap() {
        let entry = entry.unwrap();
        let path = entry.path();
        if path
            .extension()
            .and_then(|s| s.to_str())
            .map(|s| s.eq_ignore_ascii_case("CHM"))
            .unwrap_or(false)
        {
            let bytes = std::fs::read(&path).unwrap();
            let doc =
                CompHtmlDoc::parse(&bytes).expect(&format!("Failed to parse {}", path.display()));
            assert!(doc.width > 0 && doc.height > 0);
            count += 1;
        }
    }
    println!("Successfully parsed {} CHM files on test disc!", count);
    assert!(
        count >= 10,
        "Expected at least 10 CHM files on disc, found {}",
        count
    );
}

#[test]
fn test_parse_autorun_cls() {
    let disc_root = common::get_test_disc_root();
    let p = disc_root
        .join("PROGRAM")
        .join("JAVA")
        .join("AUTORUN.CLS");
    if !p.exists() {
        return;
    }

    let bytes = std::fs::read(&p).expect("Failed to read AUTORUN.CLS");
    let cfg = AutoRunConfig::parse(&bytes).expect("Failed to parse AUTORUN.CLS");

    assert_eq!(cfg.cover_ybm, "COVER.YBM");
    assert_eq!(cfg.opening_mpeg, "MUSIC01.DAT");
    assert_eq!(cfg.homepage_chm, "HOMEPAGE.CHM");
}
