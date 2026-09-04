//! Modal dialogs for VCD 3.0 Player.

use crate::core::script_vm::UnrecognizedInstruction;
use eframe::egui::{self, Color32, RichText, Vec2};

/// Displays a modal warning dialog when an unrecognized VCDSCRIPT instruction is encountered.
pub fn show_unrecognized_instruction_dialog(
    ctx: &egui::Context,
    alert: &UnrecognizedInstruction,
    on_skip: &mut bool,
    on_terminate: &mut bool,
) {
    egui::Window::new("⚠️ 未识别的 VCDSCRIPT 指令")
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, Vec2::new(0.0, 0.0))
        .default_width(450.0)
        .show(ctx, |ui| {
            ui.add_space(8.0);
            ui.label(
                RichText::new("脚本解释器在执行交互指令时遇到了未识别或未支持的语句：")
                    .color(Color32::from_rgb(255, 200, 100))
                    .size(14.0),
            );

            ui.add_space(10.0);

            egui::Frame::group(ui.style())
                .fill(Color32::from_rgb(30, 30, 35))
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("目标行号:").strong());
                        ui.label(RichText::new(format!("第 {} 行", alert.line_no)).color(Color32::LIGHT_BLUE));
                    });

                    ui.horizontal(|ui| {
                        ui.label(RichText::new("原始指令:").strong());
                        ui.label(
                            RichText::new(&alert.raw_code)
                                .monospace()
                                .color(Color32::from_rgb(255, 120, 120)),
                        );
                    });

                    ui.horizontal(|ui| {
                        ui.label(RichText::new("诊断原因:").strong());
                        ui.label(RichText::new(&alert.reason).color(Color32::LIGHT_GRAY));
                    });
                });

            ui.add_space(14.0);

            ui.horizontal(|ui| {
                if ui
                    .button(
                        RichText::new("⏩ 跳过并继续执行")
                            .color(Color32::WHITE)
                            .strong(),
                    )
                    .clicked()
                {
                    *on_skip = true;
                }

                ui.add_space(12.0);

                if ui
                    .button(
                        RichText::new("⏹ 终止脚本执行")
                            .color(Color32::from_rgb(255, 120, 120))
                            .strong(),
                    )
                    .clicked()
                {
                    *on_terminate = true;
                }
            });

            ui.add_space(8.0);
        });
}
