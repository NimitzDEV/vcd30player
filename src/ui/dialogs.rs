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

/// Displays an "About" modal dialog showing software information, author, and GitHub/Issue links.
pub fn show_about_dialog(ctx: &egui::Context, open: &mut bool) {
    let mut is_open = *open;
    let mut close_clicked = false;

    egui::Window::new("ℹ 关于 VCD 3.0 Player")
        .open(&mut is_open)
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, Vec2::new(0.0, 0.0))
        .default_width(380.0)
        .show(ctx, |ui| {
            ui.add_space(4.0);

            // Title and Version
            ui.horizontal(|ui| {
                ui.label(RichText::new("💿").size(28.0));
                ui.vertical(|ui| {
                    ui.label(
                        RichText::new("VCD 3.0 Player")
                            .size(18.0)
                            .strong()
                            .color(Color32::WHITE),
                    );
                    ui.label(
                        RichText::new(format!("版本: v{}", env!("CARGO_PKG_VERSION")))
                            .size(12.0)
                            .color(Color32::LIGHT_GRAY),
                    );
                });
            });

            ui.add_space(4.0);
            ui.label(
                RichText::new("现代化跨平台 VCD 3.0 交互式多媒体播放器")
                    .size(13.0)
                    .color(Color32::from_rgb(200, 205, 215)),
            );

            ui.add_space(8.0);
            ui.separator();
            ui.add_space(6.0);

            // Metadata info grid / frame
            egui::Frame::group(ui.style())
                .fill(Color32::from_rgb(30, 32, 38))
                .show(ui, |ui| {
                    egui::Grid::new("about_info_grid")
                        .num_columns(2)
                        .spacing([12.0, 8.0])
                        .show(ui, |ui| {
                            ui.label(RichText::new("软件名称:").strong().color(Color32::LIGHT_GRAY));
                            ui.label(RichText::new("VCD 3.0 Player (vcd30_player)").strong());
                            ui.end_row();

                            ui.label(RichText::new("软件作者:").strong().color(Color32::LIGHT_GRAY));
                            ui.label(
                                RichText::new("NimitzDEV")
                                    .color(Color32::from_rgb(100, 200, 255))
                                    .strong(),
                            );
                            ui.end_row();

                            ui.label(RichText::new("GitHub 地址:").strong().color(Color32::LIGHT_GRAY));
                            ui.hyperlink_to(
                                "https://github.com/NimitzDEV/vcd30player",
                                "https://github.com/NimitzDEV/vcd30player",
                            );
                            ui.end_row();

                            ui.label(RichText::new("提交 Issue:").strong().color(Color32::LIGHT_GRAY));
                            ui.hyperlink_to(
                                "https://github.com/NimitzDEV/vcd30player/issues",
                                "https://github.com/NimitzDEV/vcd30player/issues",
                            );
                            ui.end_row();

                            ui.label(RichText::new("开源许可:").strong().color(Color32::LIGHT_GRAY));
                            ui.label(RichText::new("MIT OR Apache-2.0").color(Color32::GRAY));
                            ui.end_row();
                        });
                });

            ui.add_space(10.0);

            // Close button
            ui.horizontal(|ui| {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui
                        .add(egui::Button::new(RichText::new("关闭").strong()).min_size(Vec2::new(70.0, 24.0)))
                        .clicked()
                    {
                        close_clicked = true;
                    }
                });
            });

            ui.add_space(4.0);
        });

    if close_clicked {
        is_open = false;
    }
    *open = is_open;
}
