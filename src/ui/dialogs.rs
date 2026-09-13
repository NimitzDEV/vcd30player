//! Modal dialogs for VCD 3.0 Player.

use crate::core::script_vm::UnrecognizedInstruction;
use eframe::egui::{self, Color32, RichText, Vec2};

/// Displays a modal warning dialog when an unrecognized VCDSCRIPT instruction is encountered.
pub fn show_unrecognized_instruction_dialog(
    ctx: &egui::Context,
    alert: &UnrecognizedInstruction,
    on_skip: &mut bool,
    on_terminate: &mut bool,
    i18n: &crate::i18n::I18nManager,
) {
    egui::Window::new(i18n.t("alert.window_title"))
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, Vec2::new(0.0, 0.0))
        .default_width(450.0)
        .show(ctx, |ui| {
            ui.add_space(8.0);
            ui.label(
                RichText::new(i18n.t("alert.desc"))
                    .color(Color32::from_rgb(255, 200, 100))
                    .size(14.0),
            );

            ui.add_space(10.0);

            egui::Frame::group(ui.style())
                .fill(Color32::from_rgb(30, 30, 35))
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        let line_str = alert.line_no.to_string();
                        ui.label(RichText::new(i18n.t_fmt("alert.line_no", &[&line_str])).strong().color(Color32::LIGHT_BLUE));
                    });

                    ui.horizontal(|ui| {
                        ui.label(RichText::new(i18n.t("alert.raw_code")).strong());
                        ui.label(
                            RichText::new(&alert.raw_code)
                                .monospace()
                                .color(Color32::from_rgb(255, 120, 120)),
                        );
                    });

                    ui.horizontal(|ui| {
                        ui.label(RichText::new(i18n.t("alert.reason")).strong());
                        ui.label(RichText::new(&alert.reason).color(Color32::LIGHT_GRAY));
                    });
                });

            ui.add_space(14.0);

            ui.horizontal(|ui| {
                if ui
                    .button(
                        RichText::new(i18n.t("alert.skip"))
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
                        RichText::new(i18n.t("alert.terminate"))
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
    i18n: &crate::i18n::I18nManager,
) {
    let mut is_open = *open;
    let mut close_clicked = false;

    egui::Window::new(i18n.t("about.window_title"))
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
                        RichText::new(i18n.t("about.app_title"))
                            .size(18.0)
                            .strong()
                            .color(Color32::WHITE),
                    );
                    ui.label(
                        RichText::new(format!("{} v{}", i18n.t("about.version"), env!("CARGO_PKG_VERSION")))
                            .size(12.0)
                            .color(Color32::LIGHT_GRAY),
                    );
                });
            });

            ui.add_space(4.0);
            ui.label(
                RichText::new(i18n.t("about.subtitle"))
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
                            ui.label(RichText::new(i18n.t("about.author")).strong().color(Color32::LIGHT_GRAY));
                            ui.label(
                                RichText::new("NimitzDEV")
                                    .color(Color32::from_rgb(100, 200, 255))
                                    .strong(),
                            );
                            ui.end_row();

                            ui.label(RichText::new(i18n.t("about.github_repo")).strong().color(Color32::LIGHT_GRAY));
                            ui.hyperlink_to(
                                "https://github.com/NimitzDEV/vcd30player",
                                "https://github.com/NimitzDEV/vcd30player",
                            );
                            ui.end_row();

                            ui.label(RichText::new(i18n.t("about.issues")).strong().color(Color32::LIGHT_GRAY));
                            ui.hyperlink_to(
                                "https://github.com/NimitzDEV/vcd30player/issues",
                                "https://github.com/NimitzDEV/vcd30player/issues",
                            );
                            ui.end_row();

                            ui.label(RichText::new(i18n.t("about.license")).strong().color(Color32::LIGHT_GRAY));
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
                        ui.label(RichText::new(i18n.t("about.status_label")).color(Color32::from_rgb(160, 160, 160)));
                    }
                    crate::updater::UpdateCheckStatus::Checking => {
                        ui.spinner();
                        ui.label(
                            RichText::new(i18n.t("about.checking")).color(Color32::from_rgb(100, 200, 255)),
                        );
                    }
                    crate::updater::UpdateCheckStatus::UpToDate => {
                        ui.label(
                            RichText::new(i18n.t("about.latest"))
                                .color(Color32::from_rgb(120, 220, 120)),
                        );
                    }
                    crate::updater::UpdateCheckStatus::UpdateAvailable(info) => {
                        ui.label(
                            RichText::new(i18n.t_fmt("about.new_version", &[&info.version]))
                                .color(Color32::from_rgb(255, 180, 50))
                                .strong(),
                        );
                    }
                    crate::updater::UpdateCheckStatus::CheckFailed(err) => {
                        ui.label(
                            RichText::new(i18n.t_fmt("about.update_failed", &[err])).color(Color32::from_rgb(255, 100, 100)),
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
                                        RichText::new(i18n.t("about.view_update_btn"))
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
                                egui::Button::new("...").min_size(Vec2::new(75.0, 22.0)),
                            );
                        }
                        _ => {
                            if ui
                                .add(egui::Button::new(i18n.t("about.check_btn")).min_size(Vec2::new(75.0, 22.0)))
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
                        .add(egui::Button::new(RichText::new(i18n.t("about.close")).strong()).min_size(Vec2::new(70.0, 24.0)))
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
        .min_width(480.0)
        .max_width(480.0)
        .default_height(360.0)
        .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
        .show(ctx, |ui| {
            ui.set_max_width(480.0);
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
                    let is_install =
                        matches!(download_state, crate::updater::DownloadState::InstallFailed(_));
                    ui.horizontal(|ui| {
                        let label_text = if is_install {
                            "❌ 更新安装失败"
                        } else {
                            "❌ 下载更新失败"
                        };
                        ui.label(
                            RichText::new(label_text)
                                .color(Color32::from_rgb(255, 100, 100))
                                .strong(),
                        );

                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            let btn_text = if is_install {
                                "重试更新"
                            } else {
                                "重试下载"
                            };
                            if ui
                                .add(
                                    egui::Button::new(btn_text)
                                        .fill(Color32::from_rgb(200, 120, 40))
                                        .min_size(Vec2::new(80.0, 24.0)),
                                )
                                .clicked()
                            {
                                if is_install {
                                    apply_update = true;
                                } else {
                                    start_download = true;
                                }
                            }
                        });
                    });

                    ui.add_space(4.0);

                    // Render error detail in a dedicated, wrapped and width-constrained box
                    let max_w = ui.available_width();
                    egui::Frame::new()
                        .fill(Color32::from_rgba_premultiplied(45, 20, 20, 180))
                        .stroke(egui::Stroke::new(1.0, Color32::from_rgb(120, 40, 40)))
                        .corner_radius(4)
                        .inner_margin(egui::Margin::symmetric(8, 6))
                        .show(ui, |ui| {
                            ui.set_max_width(max_w - 16.0);
                            ui.add(
                                egui::Label::new(
                                    RichText::new(err)
                                        .color(Color32::from_rgb(255, 140, 140))
                                        .size(11.5),
                                )
                                .wrap(),
                            )
                            .on_hover_text(err);
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

/// Displays the Settings modal dialog for configuring application preferences (e.g., interface language).
pub fn show_settings_dialog(
    ctx: &egui::Context,
    open: &mut bool,
    i18n: &mut crate::i18n::I18nManager,
) {
    let mut is_open = *open;
    let mut close_clicked = false;

    let window_width = if i18n.active_locale().starts_with("zh") { 300.0 } else { 360.0 };

    egui::Window::new(format!("⚙ {}", i18n.t("settings.title")))
        .id(egui::Id::new("vcd30_settings_modal_window"))
        .open(&mut is_open)
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
        .fixed_size(Vec2::new(window_width, 75.0))
        .show(ctx, |ui| {
            ui.add_space(2.0);

            ui.horizontal(|ui| {
                ui.label(RichText::new(i18n.t("settings.language_section")).strong());

                let current_lang = i18n.selected_language().to_string();
                let current_display = if current_lang == "auto" {
                    format!("🌐 {} ({})", i18n.t("settings.language_auto"), i18n.resolved_locale_name())
                } else if let Some((_, name)) = i18n.available_locales().iter().find(|(k, _)| k == &current_lang) {
                    name.clone()
                } else {
                    current_lang.clone()
                };

                let mut selected_changed = None;
                let combo_w = (ui.available_width() - 4.0).max(140.0);

                egui::ComboBox::from_id_salt("settings_lang_combo")
                    .selected_text(&current_display)
                    .width(combo_w)
                    .show_ui(ui, |ui| {
                        let auto_label = format!("🌐 {}", i18n.t("settings.language_auto"));
                        if ui.selectable_label(current_lang == "auto", auto_label).clicked() {
                            selected_changed = Some("auto".to_string());
                        }

                        ui.separator();

                        for (locale_id, name) in i18n.available_locales() {
                            let is_selected = current_lang == *locale_id;
                            if ui.selectable_label(is_selected, name).clicked() {
                                selected_changed = Some(locale_id.clone());
                            }
                        }
                    });

                if let Some(new_lang) = selected_changed {
                    i18n.set_language(new_lang);
                    ctx.request_repaint();
                }
            });

            ui.add_space(6.0);
            ui.separator();
            ui.add_space(4.0);

            // Bottom row: Close button wrapped in ui.horizontal to prevent layout height inflation
            ui.horizontal(|ui| {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui
                        .add(egui::Button::new(RichText::new(i18n.t("settings.close")).strong()).min_size(Vec2::new(60.0, 22.0)))
                        .clicked()
                    {
                        close_clicked = true;
                    }
                });
            });

            ui.add_space(2.0);
        });

    if close_clicked {
        is_open = false;
    }
    *open = is_open;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_settings_dialog_rendering_and_language_switch() {
        let ctx = egui::Context::default();
        let mut open = true;
        let mut i18n = crate::i18n::I18nManager::new();
        i18n.set_language("zh-CN".to_string());

        let mut input = egui::RawInput::default();
        input.screen_rect = Some(egui::Rect::from_min_size(egui::Pos2::ZERO, Vec2::new(800.0, 600.0)));

        let step = |ctx: &egui::Context, open: &mut bool, i18n: &mut crate::i18n::I18nManager| -> egui::FullOutput {
            let mut out = ctx.run_ui(input.clone(), |ctx| {
                show_settings_dialog(ctx, open, i18n);
            });
            out.textures_delta.clear();
            out
        };

        // Chinese layout & render (2 frames for egui window positioning)
        let _ = step(&ctx, &mut open, &mut i18n);
        let _ = step(&ctx, &mut open, &mut i18n);
        let rect_zh = ctx.memory(|mem| mem.area_rect(egui::Id::new("vcd30_settings_modal_window"))).unwrap();

        // Switch to English
        i18n.set_language("en-US".to_string());

        // English layout & render (2 frames)
        let _ = step(&ctx, &mut open, &mut i18n);
        let _ = step(&ctx, &mut open, &mut i18n);
        let rect_en = ctx.memory(|mem| mem.area_rect(egui::Id::new("vcd30_settings_modal_window"))).unwrap();

        assert_eq!(rect_zh.width(), 300.0);
        assert_eq!(rect_en.width(), 360.0);
        assert!(rect_en.width() > rect_zh.width());

        // Switch back to Chinese
        i18n.set_language("zh-CN".to_string());
        let _ = step(&ctx, &mut open, &mut i18n);
        let _ = step(&ctx, &mut open, &mut i18n);
        let rect_zh2 = ctx.memory(|mem| mem.area_rect(egui::Id::new("vcd30_settings_modal_window"))).unwrap();
        assert_eq!(rect_zh2.width(), 300.0);
    }
}

