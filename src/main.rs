use eframe::egui::Vec2;
use std::path::PathBuf;
use vcd30_player::ui::app::VcdPlayerApp;

fn main() -> eframe::Result<()> {
    let native_options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_inner_size(Vec2::new(880.0, 720.0))
            .with_min_inner_size(Vec2::new(500.0, 420.0))
            .with_title("vcd30player"),
        ..Default::default()
    };

    let initial_disc = std::env::args().nth(1).map(PathBuf::from);

    eframe::run_native(
        "vcd30player",
        native_options,
        Box::new(|cc| Ok(Box::new(VcdPlayerApp::new(cc, initial_disc)))),
    )
}
