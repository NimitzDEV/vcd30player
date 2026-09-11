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
pub fn show_about_dialog(
    ctx: &egui::Context,
    open: &mut bool,
    updater: &mut crate::updater::UpdateManager,
) {
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

            // Separator between metadata and update section
            ui.add_space(8.0);
            ui.separator();
            ui.add_space(6.0);

            // Update status & Action row
            let check_status = updater.check_status.clone();
            ui.horizontal(|ui| {
                // Left: Update status
                match &check_status {
                    crate::updater::UpdateCheckStatus::Idle => {
                        ui.label(RichText::new("未检查更新").color(Color32::from_rgb(160, 160, 160)));
                    }
                    crate::updater::UpdateCheckStatus::Checking => {
                        ui.spinner();
                        ui.label(
                            RichText::new("正在检查更新...").color(Color32::from_rgb(100, 200, 255)),
                        );
                    }
                    crate::updater::UpdateCheckStatus::UpToDate => {
                        ui.label(
                            RichText::new(format!("已是最新版本 (v{})", env!("CARGO_PKG_VERSION")))
                                .color(Color32::from_rgb(120, 220, 120)),
                        );
                    }
                    crate::updater::UpdateCheckStatus::UpdateAvailable(info) => {
                        ui.label(
                            RichText::new(format!("可更新到 v{}", info.version))
                                .color(Color32::from_rgb(255, 180, 50))
                                .strong(),
                        );
                    }
                    crate::updater::UpdateCheckStatus::CheckFailed(err) => {
                        ui.label(
                            RichText::new("检查更新失败").color(Color32::from_rgb(255, 100, 100)),
                        )
                        .on_hover_text(err);
                    }
                }

                // Right: Update action button
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    match &check_status {
                        crate::updater::UpdateCheckStatus::UpdateAvailable(_) => {
                            if ui
                                .add(
                                    egui::Button::new(
                                        RichText::new("查看详情")
                                            .color(Color32::WHITE)
                                            .strong(),
                                    )
                                    .fill(Color32::from_rgb(35, 134, 54))
                                    .min_size(Vec2::new(75.0, 22.0)),
                                )
                                .clicked()
                            {
                                updater.show_details_window = true;
                            }
                        }
                        crate::updater::UpdateCheckStatus::Checking => {
                            ui.add_enabled(
                                false,
                                egui::Button::new("检查中...").min_size(Vec2::new(75.0, 22.0)),
                            );
                        }
                        _ => {
                            if ui
                                .add(egui::Button::new("检查更新").min_size(Vec2::new(75.0, 22.0)))
                                .clicked()
                            {
                                updater.check_for_updates();
                            }
                        }
                    }
                });
            });

            ui.add_space(8.0);
            ui.separator();
            ui.add_space(6.0);

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

/// Displays an update details modal dialog showing bilingual changelog and download controls.
pub fn show_update_details_dialog(
    ctx: &egui::Context,
    updater: &mut crate::updater::UpdateManager,
) {
    if !updater.show_details_window {
        return;
    }

    let Some(info) = (match &updater.check_status {
        crate::updater::UpdateCheckStatus::UpdateAvailable(info) => Some(info.clone()),
        _ => None,
    }) else {
        updater.show_details_window = false;
        return;
    };

    let mut is_open = updater.show_details_window;

    egui::Window::new(format!("🚀 软件更新 - v{}", info.version))
        .open(&mut is_open)
        .collapsible(false)
        .resizable(false)
        .default_width(480.0)
        .min_width(450.0)
        .max_width(520.0)
        .default_height(360.0)
        .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
        .show(ctx, |ui| {
            ui.add_space(4.0);

            // Version info banner
            ui.horizontal(|ui| {
                ui.label(RichText::new("🚀").size(24.0));
                ui.vertical(|ui| {
                    ui.label(
                        RichText::new(format!("发现新版本: v{}", info.version))
                            .size(16.0)
                            .strong()
                            .color(Color32::WHITE),
                    );
                    let raw_date = info.release_date.as_deref().unwrap_or("未知");
                    let date_str = crate::updater::format_release_date(raw_date);
                    ui.label(
                        RichText::new(format!("发布日期: {}", date_str))
                            .size(12.0)
                            .color(Color32::LIGHT_GRAY),
                    );
                });
            });

            ui.add_space(8.0);
            ui.separator();
            ui.add_space(4.0);

            // Bilingual Tabs
            ui.horizontal(|ui| {
                ui.selectable_value(
                    &mut updater.active_tab,
                    crate::updater::ChangelogTab::Chinese,
                    "🇨🇳 中文更新日志",
                );
                ui.selectable_value(
                    &mut updater.active_tab,
                    crate::updater::ChangelogTab::English,
                    "🌐 English Changelog",
                );
            });

            ui.add_space(4.0);

            // Scrollable changelog view
            egui::Frame::group(ui.style())
                .fill(Color32::from_rgb(25, 27, 33))
                .show(ui, |ui| {
                    egui::ScrollArea::vertical()
                        .max_height(200.0)
                        .min_scrolled_height(140.0)
                        .auto_shrink([false, false])
                        .show(ui, |ui| {
                            let text = match updater.active_tab {
                                crate::updater::ChangelogTab::Chinese => info
                                    .changelog
                                    .zh
                                    .as_deref()
                                    .unwrap_or("暂无中文更新日志。"),
                                crate::updater::ChangelogTab::English => info
                                    .changelog
                                    .en
                                    .as_deref()
                                    .unwrap_or("No English changelog available."),
                            };
                            ui.add(
                                egui::Label::new(
                                    RichText::new(text)
                                        .size(13.0)
                                        .color(Color32::from_rgb(220, 225, 235)),
                                )
                                .wrap(),
                            );
                        });
                });

            ui.add_space(8.0);
            ui.separator();
            ui.add_space(6.0);

            // Bottom action area: Download / Progress / Install
            let download_state = updater.download_state.clone();
            let current_asset = updater.get_current_platform_asset();
            let mut start_download = false;
            let mut cancel_download = false;
            let mut apply_update = false;

            match &download_state {
                crate::updater::DownloadState::NotStarted => {
                    ui.horizontal(|ui| {
                        if let Some(asset) = &current_asset {
                            let mb = asset.size as f64 / 1_048_576.0;
                            ui.label(
                                RichText::new(format!("安装包大小: {:.1} MB", mb))
                                    .size(12.0)
                                    .color(Color32::LIGHT_GRAY),
                            );
                        } else {
                            ui.label(
                                RichText::new("未检测到当前平台的安装包")
                                    .size(12.0)
                                    .color(Color32::from_rgb(255, 120, 120)),
                            );
                        }

                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            let can_download = current_asset.is_some();
                            if ui
                                .add_enabled(
                                    can_download,
                                    egui::Button::new(
                                        RichText::new("⬇ 下载更新")
                                            .color(Color32::WHITE)
                                            .strong(),
                                    )
                                    .fill(Color32::from_rgb(35, 134, 54))
                                    .min_size(Vec2::new(95.0, 26.0)),
                                )
                                .clicked()
                            {
                                start_download = true;
                            }
                        });
                    });
                }
                crate::updater::DownloadState::Downloading { downloaded, total } => {
                    ui.horizontal(|ui| {
                        let frac = if *total > 0 {
                            (*downloaded as f32 / *total as f32).clamp(0.0, 1.0)
                        } else {
                            0.0
                        };
                        let dl_mb = *downloaded as f64 / 1_048_576.0;
                        let tot_mb = *total as f64 / 1_048_576.0;
                        let pct = frac * 100.0;

                        let btn_width = 65.0;
                        let spacing = ui.spacing().item_spacing.x;
                        let bar_width = (ui.available_width() - btn_width - spacing).max(100.0);

                        // ProgressBar occupies full available width minus the button space
                        ui.add(
                            egui::ProgressBar::new(frac)
                                .desired_width(bar_width)
                                .text(format!("{:.1}% ({:.1} MB / {:.1} MB)", pct, dl_mb, tot_mb))
                                .animate(true),
                        );

                        if ui
                            .add(
                                egui::Button::new(RichText::new("取消").color(Color32::WHITE))
                                    .fill(Color32::from_rgb(180, 40, 40))
                                    .min_size(Vec2::new(btn_width, 24.0)),
                            )
                            .clicked()
                        {
                            cancel_download = true;
                        }
                    });
                }
                crate::updater::DownloadState::Downloaded { .. } => {
                    ui.horizontal(|ui| {
                        ui.label(
                            RichText::new("✅ 下载完成并通过完整性校验")
                                .color(Color32::from_rgb(100, 220, 100))
                                .strong(),
                        );

                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui
                                .add(
                                    egui::Button::new(
                                        RichText::new("🚀 立即更新")
                                            .color(Color32::WHITE)
                                            .strong(),
                                    )
                                    .fill(Color32::from_rgb(35, 134, 54))
                                    .min_size(Vec2::new(95.0, 26.0)),
                                )
                                .clicked()
                            {
                                apply_update = true;
                            }
                        });
                    });
                }
                crate::updater::DownloadState::Installing => {
                    ui.horizontal(|ui| {
                        ui.spinner();
                        ui.label(
                            RichText::new("正在解压并就地更新，即将重启...")
                                .color(Color32::LIGHT_BLUE)
                                .strong(),
                        );
                    });
                }
                crate::updater::DownloadState::Failed(err)
                | crate::updater::DownloadState::InstallFailed(err) => {
                    ui.horizontal(|ui| {
                        ui.label(
                            RichText::new(format!("❌ {}", err))
                                .color(Color32::from_rgb(255, 100, 100))
                                .size(12.0),
                        );

                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui
                                .add(
                                    egui::Button::new("重试下载")
                                        .fill(Color32::from_rgb(200, 120, 40))
                                        .min_size(Vec2::new(80.0, 24.0)),
                                )
                                .clicked()
                            {
                                start_download = true;
                            }
                        });
                    });
                }
            }

            if start_download {
                updater.start_download();
            } else if cancel_download {
                updater.cancel_download();
            } else if apply_update {
                updater.apply_update();
            }

            ui.add_space(4.0);
        });

    updater.show_details_window = is_open;
}
