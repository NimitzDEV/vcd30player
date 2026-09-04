use eframe::egui;

#[test]
fn test_cjk_font_loading_and_layout() {
    let ctx = egui::Context::default();

    let mut fonts = egui::FontDefinitions::default();

    let candidate_font_paths = [
        r"C:\Windows\Fonts\msyh.ttc",
        r"C:\Windows\Fonts\msyh.ttf",
        r"C:\Windows\Fonts\simsun.ttc",
        r"C:\Windows\Fonts\simhei.ttf",
        "/usr/share/fonts/truetype/wqy/wqy-microhei.ttc",
        "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc",
        "/System/Library/Fonts/PingFang.ttc",
        "/Library/Fonts/Songti.ttc",
    ];

    let mut loaded = false;
    for path in candidate_font_paths {
        if let Ok(bytes) = std::fs::read(path) {
            println!("Found font: {}, size: {} bytes", path, bytes.len());
            fonts.font_data.insert(
                "cjk_font".to_owned(),
                std::sync::Arc::new(egui::FontData::from_owned(bytes)),
            );

            // Put cjk_font at index 0 so it handles Latin and CJK consistently
            fonts
                .families
                .entry(egui::FontFamily::Proportional)
                .or_default()
                .insert(0, "cjk_font".to_owned());

            fonts
                .families
                .entry(egui::FontFamily::Monospace)
                .or_default()
                .push("cjk_font".to_owned());

            loaded = true;
            break;
        }
    }

    if !loaded {
        eprintln!("No CJK font found on this environment, skipping test");
        return;
    }

    ctx.set_fonts(fonts);

    let mut output = ctx.run_ui(egui::RawInput::default(), |ctx| {
        let chinese_text = "就绪 - 请打开光盘目录 文件 帮助 🎯 热区高亮";
        let galley = ctx.fonts_mut(|f| {
            f.layout(
                chinese_text.to_owned(),
                egui::FontId::proportional(14.0),
                egui::Color32::WHITE,
                1000.0,
            )
        });

        assert!(!galley.rows.is_empty(), "Galley should layout Chinese text");
        assert!(galley.rect.width() > 0.0);
    });
    output.textures_delta.clear();
}
