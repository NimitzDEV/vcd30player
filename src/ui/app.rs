//! VCD 3.0 Player GUI application implementation using eframe/egui.

use crate::core::kernel::{CANVAS_HEIGHT, CANVAS_WIDTH, VcdKernel};
use eframe::egui::{
    self, Color32, ColorImage, Pos2, Rect, Stroke, StrokeKind, TextureOptions, Vec2,
};
use std::path::PathBuf;

pub struct VcdPlayerApp {
    pub kernel: VcdKernel,
    pub texture: Option<egui::TextureHandle>,
    pub texture_dirty: bool,
    pub show_hotspots: bool,
    pub show_metadata: bool,
    pub show_remote: bool,
    pub show_about: bool,
    pub hovered_hotspot: Option<String>,
    pub status_message: String,
    pub status_timestamp: Option<std::time::Instant>,
    pub idle_message: String,
}

fn setup_custom_fonts(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();

    // Candidate CJK font paths on Windows, Linux, and macOS
    let candidate_font_paths = [
        // Windows (Microsoft YaHei, SimSun, SimHei)
        r"C:\Windows\Fonts\msyh.ttc",
        r"C:\Windows\Fonts\msyh.ttf",
        r"C:\Windows\Fonts\simsun.ttc",
        r"C:\Windows\Fonts\simhei.ttf",
        // Linux
        "/usr/share/fonts/truetype/wqy/wqy-microhei.ttc",
        "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc",
        // macOS
        "/System/Library/Fonts/PingFang.ttc",
        "/Library/Fonts/Songti.ttc",
    ];

    for path in candidate_font_paths {
        if let Ok(bytes) = std::fs::read(path) {
            fonts.font_data.insert(
                "cjk_font".to_owned(),
                std::sync::Arc::new(egui::FontData::from_owned(bytes)),
            );

            // Add CJK font at highest priority for Proportional font family
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

            break;
        }
    }

    ctx.set_fonts(fonts);
}

/// Transient status message linger duration (5 seconds).
pub const STATUS_MESSAGE_DURATION: std::time::Duration = std::time::Duration::from_secs(5);

impl VcdPlayerApp {
    pub fn new(cc: &eframe::CreationContext<'_>, initial_disc: Option<PathBuf>) -> Self {
        setup_custom_fonts(&cc.egui_ctx);

        let mut kernel = VcdKernel::new();
        let mut status = "空闲 - 请点击“加载光盘”载入 VCD 光盘".to_string();
        let mut has_initial_disc = false;

        if let Some(root) = initial_disc {
            if root.exists() {
                if let Err(e) = kernel.open_disc(root.clone()) {
                    status = format!("打开光盘失败: {}", e);
                } else {
                    let disc_desc = kernel
                        .disc_type
                        .as_ref()
                        .map(|t| t.to_string())
                        .unwrap_or_else(|| "光盘".to_string());
                    status = format!("已加载光盘: {}", disc_desc);
                    has_initial_disc = true;
                }
            }
        }

        Self {
            kernel,
            texture: None,
            texture_dirty: true,
            show_hotspots: false,
            show_metadata: false,
            show_remote: false,
            show_about: false,
            hovered_hotspot: None,
            status_message: status,
            status_timestamp: if has_initial_disc {
                Some(std::time::Instant::now())
            } else {
                None
            },
            idle_message: "空闲 - 请点击“加载光盘”载入 VCD 光盘".to_string(),
        }
    }

    /// Creates an app instance wrapping an existing VcdKernel.
    pub fn from_kernel(kernel: VcdKernel) -> Self {
        Self {
            kernel,
            texture: None,
            texture_dirty: true,
            show_hotspots: false,
            show_metadata: false,
            show_remote: false,
            show_about: false,
            hovered_hotspot: None,
            status_message: String::new(),
            status_timestamp: None,
            idle_message: "空闲 - 请点击“加载光盘”载入 VCD 光盘".to_string(),
        }
    }

    /// Sets a temporary status message (Status 1) that will be displayed for 5 seconds.
    pub fn set_status(&mut self, msg: impl Into<String>) {
        self.status_message = msg.into();
        self.status_timestamp = Some(std::time::Instant::now());
    }

    /// Renders default status info when no transient status or hover target is active.
    fn render_default_status(&self, ui: &mut egui::Ui) {
        if self.is_disc_loaded() {
            let page_info = if !self.kernel.current_page_name.is_empty() {
                let title = self
                    .kernel
                    .current_page
                    .as_ref()
                    .map(|d| d.title.as_str())
                    .unwrap_or("");
                if title.is_empty() {
                    format!("📄 {}", self.kernel.current_page_name)
                } else {
                    format!("📄 {} ({})", self.kernel.current_page_name, title)
                }
            } else {
                "未载入页面".to_string()
            };

            ui.label(egui::RichText::new(page_info).strong());
            ui.label(egui::RichText::new("·").color(Color32::GRAY));

            if let Some(ref player) = self.kernel.active_video {
                let (w, h) = player.dimensions();
                let fps = player.framerate();
                ui.label(
                    egui::RichText::new(format!(
                        "🎬 {} ({}x{} @ {:.0}fps)",
                        player.filename, w, h, fps
                    ))
                    .strong(),
                );
            } else {
                ui.label(egui::RichText::new("🎬 未播放视频").color(Color32::DARK_GRAY));
            }
        } else {
            ui.label(egui::RichText::new(&self.idle_message).color(Color32::GRAY));
        }
    }

    /// Checks if a disc or page is currently loaded.
    pub fn is_disc_loaded(&self) -> bool {
        !self.kernel.disc_root.as_os_str().is_empty()
            || self.kernel.current_page.is_some()
            || self.kernel.is_video_active()
    }

    /// Ejects the current disc and resets kernel and UI to initial state.
    pub fn eject_disc(&mut self) {
        if let Some(mut v) = self.kernel.active_video.take() {
            v.stop();
        }
        self.kernel.audio.stop_all();
        self.kernel.disc_root = PathBuf::new();
        self.kernel.current_page_name.clear();
        self.kernel.current_page = None;
        self.kernel.current_bg_image = None;
        for chunk in self.kernel.canvas.chunks_exact_mut(4) {
            chunk[0] = 0;
            chunk[1] = 0;
            chunk[2] = 0;
            chunk[3] = 255;
        }
        self.kernel.history_stack.clear();
        self.kernel.forward_stack.clear();
        self.kernel.autorun_config = None;
        self.kernel.cursor_pos = None;
        self.kernel.active_alert = None;
        self.kernel.sprite_cache.clear();
        self.kernel.karaoke_playlist.clear();
        self.kernel.disc_type = None;
        self.kernel.vm.terminate();
        self.texture = None;
        self.texture_dirty = true;
        self.idle_message = "光盘已弹出，请加载光盘".to_string();
        self.set_status("光盘已弹出，请加载光盘");
    }

    /// Resets the current disc or page to its freshly loaded state and restarts playback.
    pub fn reset_disc(&mut self, ctx: &egui::Context) {
        if !self.kernel.disc_root.as_os_str().is_empty() {
            let root = self.kernel.disc_root.clone();
            if let Err(e) = self.kernel.open_disc(root.clone()) {
                self.set_status(format!("重置失败: {}", e));
            } else {
                let disc_desc = self
                    .kernel
                    .disc_type
                    .as_ref()
                    .map(|t| t.to_string())
                    .unwrap_or_else(|| "光盘".to_string());
                self.set_status(format!("已重置光盘: {}", disc_desc));
                self.texture = None;
                self.texture_dirty = true;
                ctx.request_repaint();
            }
        } else if !self.kernel.current_page_name.is_empty() {
            let page = self.kernel.current_page_name.clone();
            if let Err(e) = self.kernel.load_page(&page, false) {
                self.set_status(format!("重置失败: {}", e));
            } else {
                self.set_status(format!("已重置页面: {}", page));
                self.texture = None;
                self.texture_dirty = true;
                ctx.request_repaint();
            }
        }
    }

    /// Triggers texture upload from video player current frame to GPU.
    fn refresh_video_texture(&mut self, ctx: &egui::Context) {
        if let Some(ref player) = self.kernel.active_video {
            let (w, h) = player.dimensions();
            let color_image = ColorImage::from_rgba_unmultiplied(
                [w as usize, h as usize],
                player.current_frame(),
            );

            if let Some(tex) = &mut self.texture {
                tex.set(color_image, TextureOptions::LINEAR);
            } else {
                self.texture =
                    Some(ctx.load_texture("vcd30_canvas", color_image, TextureOptions::LINEAR));
            }
            self.texture_dirty = false;
        }
    }

    /// Triggers texture upload from kernel canvas to GPU.
    fn refresh_texture(&mut self, ctx: &egui::Context) {
        let color_image = ColorImage::from_rgba_unmultiplied(
            [CANVAS_WIDTH as usize, CANVAS_HEIGHT as usize],
            &self.kernel.canvas,
        );

        if let Some(tex) = &mut self.texture {
            tex.set(color_image, TextureOptions::NEAREST);
        } else {
            self.texture =
                Some(ctx.load_texture("vcd30_canvas", color_image, TextureOptions::NEAREST));
        }
        self.texture_dirty = false;
    }

    /// Maps a screen position to 352x288 canvas coordinate inside the display rect.
    fn screen_to_canvas(&self, screen_pos: Pos2, display_rect: Rect) -> Option<(i32, i32)> {
        if !display_rect.contains(screen_pos) {
            return None;
        }

        let rel_x = (screen_pos.x - display_rect.min.x) / display_rect.width();
        let rel_y = (screen_pos.y - display_rect.min.y) / display_rect.height();

        let canvas_x = (rel_x * CANVAS_WIDTH as f32) as i32;
        let canvas_y = (rel_y * CANVAS_HEIGHT as f32) as i32;

        Some((
            canvas_x.clamp(0, CANVAS_WIDTH as i32 - 1),
            canvas_y.clamp(0, CANVAS_HEIGHT as i32 - 1),
        ))
    }

    /// Maps a canvas coordinate (x, y) to screen position on display rect.
    fn canvas_to_screen(&self, canvas_x: i32, canvas_y: i32, display_rect: Rect) -> Pos2 {
        let x = display_rect.min.x + (canvas_x as f32 / CANVAS_WIDTH as f32) * display_rect.width();
        let y =
            display_rect.min.y + (canvas_y as f32 / CANVAS_HEIGHT as f32) * display_rect.height();
        Pos2::new(x, y)
    }

    /// Renders the VCD player UI into the given egui::Ui.
    pub fn show(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();

        // Drive video playback or active VM
        if self.kernel.is_video_active() {
            self.hovered_hotspot = None;
            let new_frame = self.kernel.update_video();
            if !self.kernel.is_video_active() {
                // Video ended during this update_video call!
                self.texture = None;
                self.texture_dirty = true;
                self.refresh_texture(&ctx);
            } else if new_frame || self.texture_dirty {
                self.refresh_video_texture(&ctx);
            }
            ctx.request_repaint();
        } else {
            if self.kernel.is_vm_active() {
                self.kernel.run_vm();
                self.texture_dirty = true;
                ctx.request_repaint();
            }

            if self.texture_dirty {
                self.refresh_texture(&ctx);
            }
        }

        // 1. Custom Window Title Bar
        let window_title = if !self.kernel.current_page_name.is_empty() {
            let title = self
                .kernel
                .current_page
                .as_ref()
                .map(|d| d.title.as_str())
                .unwrap_or("");
            if title.is_empty() {
                format!("vcd30player - {}", self.kernel.current_page_name)
            } else {
                format!("vcd30player - {} ({})", self.kernel.current_page_name, title)
            }
        } else {
            "vcd30player".to_string()
        };
        crate::ui::titlebar::show_title_bar(ui, &window_title);

        // Bottom Control & Status Panel
        egui::Panel::bottom("bottom_bar").show(ui, |ui| {
            // Equalize top margin to match the 6px bottom margin to separator
            ui.add_space(3.0);

            // Row 1: Disc Loading / Ejection, Reset, Persistent Video Playback Controls
            ui.horizontal(|ui| {
                let is_loaded = self.is_disc_loaded();

                if !is_loaded {
                    ui.scope(|ui| {
                        ui.spacing_mut().item_spacing.x = 0.0;

                        let left_cr = egui::CornerRadius { nw: 4, ne: 0, sw: 4, se: 0 };
                        if ui.add(egui::Button::new("📁 加载光盘").corner_radius(left_cr)).clicked() {
                            if let Some(folder) = rfd::FileDialog::new().pick_folder() {
                                if let Err(e) = self.kernel.open_disc(folder.clone()) {
                                    self.set_status(format!("打开失败: {}", e));
                                } else {
                                    let disc_desc = self
                                        .kernel
                                        .disc_type
                                        .as_ref()
                                        .map(|t| t.to_string())
                                        .unwrap_or_else(|| "光盘".to_string());
                                    self.set_status(format!("已打开: {}", disc_desc));
                                    self.texture = None;
                                    self.texture_dirty = true;
                                    ctx.request_repaint();
                                }
                            }
                        }

                        let right_cr = egui::CornerRadius { nw: 0, ne: 4, sw: 0, se: 4 };
                        ui.visuals_mut().widgets.inactive.corner_radius = right_cr;
                        ui.visuals_mut().widgets.hovered.corner_radius = right_cr;
                        ui.visuals_mut().widgets.active.corner_radius = right_cr;
                        ui.visuals_mut().widgets.open.corner_radius = right_cr;

                        ui.menu_button("▼", |ui| {
                            ui.visuals_mut().widgets.inactive.corner_radius = egui::CornerRadius::same(3);
                            ui.visuals_mut().widgets.hovered.corner_radius = egui::CornerRadius::same(3);
                            if ui.button("📄 加载CHM...").clicked() {
                                ui.close();
                                if let Some(file) = rfd::FileDialog::new()
                                    .add_filter("VCD30 Compiled HTML", &["chm", "CHM"])
                                    .pick_file()
                                {
                                    if let Some(parent) = file.parent() {
                                        let mut disc_dir = parent.to_path_buf();
                                        if disc_dir.ends_with("VCD_DATA") || disc_dir.ends_with("DATA") {
                                            if let Some(p2) = disc_dir.parent() {
                                                disc_dir = p2.to_path_buf();
                                            }
                                        }
                                        self.kernel.disc_root = disc_dir;
                                    }
                                    if let Some(name) = file.file_name().and_then(|s| s.to_str()) {
                                        if let Err(e) = self.kernel.load_page(name, true) {
                                            self.set_status(format!("加载页面失败: {}", e));
                                        } else {
                                            self.set_status(format!("已加载: {}", name));
                                            self.texture = None;
                                            self.texture_dirty = true;
                                            ctx.request_repaint();
                                        }
                                    }
                                }
                            }
                        });
                    });
                } else {
                    if ui.button("⏏ 弹出光盘").clicked() {
                        self.eject_disc();
                        ctx.request_repaint();
                    }
                }

                ui.add(egui::Separator::default().spacing(0.0));

                if ui
                    .add_enabled(is_loaded, egui::Button::new("↺ 重置"))
                    .clicked()
                {
                    self.reset_disc(&ctx);
                }

                ui.add(egui::Separator::default().spacing(0.0));

                // Video playback controls (persistent toolbar row: active during video, disabled when inactive)
                let mut stop_video = false;
                if let Some(ref mut player) = self.kernel.active_video {
                    let is_playing = player.is_playing();
                    let (play_icon, play_tooltip) = if is_playing {
                        ("⏸", "暂停 (Space)")
                    } else {
                        ("▶", "播放 (Space)")
                    };
                    if ui.button(play_icon).on_hover_text(play_tooltip).clicked() {
                        player.toggle_play_pause();
                    }

                    if ui.button("⏹").on_hover_text("停止并返回 (ESC)").clicked() {
                        stop_video = true;
                    }

                    let cur = player.current_time();
                    let dur = player.duration();
                    let cur_min = (cur / 60.0) as u32;
                    let cur_sec = (cur % 60.0) as u32;
                    let dur_min = (dur / 60.0) as u32;
                    let dur_sec = (dur % 60.0) as u32;
                    let time_text = format!("{:02}:{:02} / {:02}:{:02}", cur_min, cur_sec, dur_min, dur_sec);

                    let font_id = egui::TextStyle::Monospace.resolve(ui.style());
                    let time_galley = ui.painter().layout_no_wrap(
                        time_text.clone(),
                        font_id,
                        ui.visuals().text_color(),
                    );
                    let time_width = time_galley.size().x;
                    let slider_width = (ui.available_width() - time_width - ui.spacing().item_spacing.x - 2.0).max(40.0);
                    ui.spacing_mut().slider_width = slider_width;

                    let mut seek_pos = cur;
                    let slider = egui::Slider::new(&mut seek_pos, 0.0..=dur.max(1.0))
                        .show_value(false)
                        .text("");
                    if ui.add(slider).changed() {
                        player.seek(seek_pos);
                    }

                    ui.label(egui::RichText::new(time_text).monospace());
                } else {
                    ui.add_enabled(false, egui::Button::new("▶"))
                        .on_hover_text("播放 (Space)")
                        .on_disabled_hover_text("播放 (Space)");
                    ui.add_enabled(false, egui::Button::new("⏹"))
                        .on_hover_text("停止并返回 (ESC)")
                        .on_disabled_hover_text("停止并返回 (ESC)");

                    let time_text = "--:-- / --:--";
                    let font_id = egui::TextStyle::Monospace.resolve(ui.style());
                    let time_galley = ui.painter().layout_no_wrap(
                        time_text.to_string(),
                        font_id,
                        ui.visuals().text_color(),
                    );
                    let time_width = time_galley.size().x;
                    let slider_width = (ui.available_width() - time_width - ui.spacing().item_spacing.x - 2.0).max(40.0);
                    ui.spacing_mut().slider_width = slider_width;

                    let mut dummy_pos = 0.0;
                    ui.add_enabled(
                        false,
                        egui::Slider::new(&mut dummy_pos, 0.0..=1.0).show_value(false).text(""),
                    );

                    ui.label(egui::RichText::new(time_text).monospace().color(Color32::DARK_GRAY));
                }

                if stop_video {
                    let _ = self.kernel.stop_video_and_exit();
                    self.set_status("已停止视频播放");
                    self.texture = None;
                    self.texture_dirty = true;
                    ctx.request_repaint();
                }
            });

            ui.separator();

            // Row 2: Navigation buttons, Debug checkboxes (right after Forward button), Page info & Status
            ui.horizontal(|ui| {
                let can_back = !self.kernel.history_stack.is_empty();
                let can_fwd = !self.kernel.forward_stack.is_empty();

                if ui
                    .add_enabled(can_back, egui::Button::new("⏮ 后退"))
                    .clicked()
                {
                    if let Ok(true) = self.kernel.go_back() {
                        self.set_status(format!("已后退至: {}", self.kernel.current_page_name));
                        self.texture = None;
                        self.texture_dirty = true;
                        ctx.request_repaint();
                    }
                }

                if ui.button("🏠 主页").clicked() {
                    if let Ok(()) = self.kernel.go_home() {
                        self.set_status("已返回主页");
                        self.texture = None;
                        self.texture_dirty = true;
                        ctx.request_repaint();
                    }
                }

                if ui
                    .add_enabled(can_fwd, egui::Button::new("⏭ 前进"))
                    .clicked()
                {
                    if let Ok(true) = self.kernel.go_forward() {
                        self.set_status(format!("已前进至: {}", self.kernel.current_page_name));
                        self.texture = None;
                        self.texture_dirty = true;
                        ctx.request_repaint();
                    }
                }

                ui.add(egui::Separator::default().spacing(0.0));

                // Requirement 3: 热区高亮和遥控器开关放在最底部的工具条上，在 前进按钮的后面
                ui.checkbox(&mut self.show_hotspots, "🎯 热区高亮");
                ui.checkbox(&mut self.show_remote, "🎮 遥控器");

                ui.add(egui::Separator::default().spacing(0.0));

                // Right side: About button on the far right (after playback status bar)
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("ℹ 关于").on_hover_text("关于软件").clicked() {
                        self.show_about = true;
                    }

                    // Remaining middle space: Unified Status Zone
                    ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                        if let Some(target) = &self.hovered_hotspot {
                            ui.label(
                                egui::RichText::new(format!("👉 目标: {}", target))
                                    .color(Color32::from_rgb(0, 220, 255)),
                            );
                        } else if let Some(ts) = self.status_timestamp {
                            let elapsed = ts.elapsed();
                            if elapsed < STATUS_MESSAGE_DURATION {
                                ui.label(
                                    egui::RichText::new(&self.status_message)
                                        .color(Color32::from_rgb(180, 180, 180)),
                                );
                                let remaining = STATUS_MESSAGE_DURATION.saturating_sub(elapsed);
                                ctx.request_repaint_after(remaining);
                            } else {
                                self.render_default_status(ui);
                            }
                        } else {
                            self.render_default_status(ui);
                        }
                    });
                });
            });

            // Symmetrical bottom margin (6px to window frame border)
            ui.add_space(1.0);
        });

        // 3. Keyboard Input Handling
        if self.kernel.is_video_active() {
            let mut toggle_pause = false;
            let mut stop_video = false;
            let mut seek_delta = 0.0;

            ctx.input(|i| {
                if i.key_pressed(egui::Key::Space) {
                    toggle_pause = true;
                } else if i.key_pressed(egui::Key::Escape) || i.key_pressed(egui::Key::Backspace) {
                    stop_video = true;
                } else if i.key_pressed(egui::Key::ArrowLeft) {
                    seek_delta = -5.0;
                } else if i.key_pressed(egui::Key::ArrowRight) {
                    seek_delta = 5.0;
                }
            });

            if toggle_pause {
                if let Some(ref mut player) = self.kernel.active_video {
                    player.toggle_play_pause();
                }
            }
            if stop_video {
                let _ = self.kernel.stop_video_and_exit();
                self.texture = None;
                self.texture_dirty = true;
                ctx.request_repaint();
            }
            if seek_delta != 0.0 {
                if let Some(ref mut player) = self.kernel.active_video {
                    let cur = player.current_time();
                    player.seek(cur + seek_delta);
                }
            }
        } else {
            let mut remote_key = None;
            ctx.input(|i| {
                if i.key_pressed(egui::Key::Enter) || i.key_pressed(egui::Key::Space) {
                    remote_key = Some(31);
                } else if i.key_pressed(egui::Key::Escape) || i.key_pressed(egui::Key::Backspace) {
                    remote_key = Some(32);
                } else if i.key_pressed(egui::Key::ArrowUp) {
                    remote_key = Some(34);
                } else if i.key_pressed(egui::Key::ArrowDown) {
                    remote_key = Some(35);
                } else if i.key_pressed(egui::Key::ArrowLeft) {
                    remote_key = Some(36);
                } else if i.key_pressed(egui::Key::ArrowRight) {
                    remote_key = Some(37);
                } else {
                    let num_keys = [
                        (egui::Key::Num0, 0),
                        (egui::Key::Num1, 1),
                        (egui::Key::Num2, 2),
                        (egui::Key::Num3, 3),
                        (egui::Key::Num4, 4),
                        (egui::Key::Num5, 5),
                        (egui::Key::Num6, 6),
                        (egui::Key::Num7, 7),
                        (egui::Key::Num8, 8),
                        (egui::Key::Num9, 9),
                    ];
                    for (k, val) in num_keys {
                        if i.key_pressed(k) {
                            remote_key = Some(val);
                            break;
                        }
                    }
                }
            });

            if let Some(key) = remote_key {
                self.kernel.inject_remote_key(key);
                self.texture_dirty = true;
            }
        }

        // 4. Modal Dialog: Unrecognized Instruction Alert
        if let Some(alert) = self.kernel.active_alert.clone() {
            let mut on_skip = false;
            let mut on_terminate = false;
            crate::ui::dialogs::show_unrecognized_instruction_dialog(
                &ctx,
                &alert,
                &mut on_skip,
                &mut on_terminate,
            );
            if on_skip {
                self.kernel.skip_alert_and_continue();
                self.texture_dirty = true;
            } else if on_terminate {
                self.kernel.terminate_script();
                self.texture_dirty = true;
            }
        }

        // Modal Dialog: About Window
        if self.show_about {
            crate::ui::dialogs::show_about_dialog(&ctx, &mut self.show_about);
        }

        // 5. Right Sidebar: Virtual Remote Control
        if self.show_remote {
            egui::Panel::right("remote_control_panel")
                .default_size(170.0)
                .show(ui, |ui| {
                    ui.add_space(4.0);
                    ui.heading("🎮 遥控器");
                    ui.separator();

                    let audio_ok = self.kernel.audio.is_audio_available();
                    let (audio_txt, audio_color) = if audio_ok {
                        ("🔊 音效输出正常", Color32::LIGHT_GREEN)
                    } else {
                        ("🔇 静音兼容模式", Color32::KHAKI)
                    };
                    ui.label(egui::RichText::new(audio_txt).color(audio_color).size(11.0));
                    ui.separator();

                    ui.label(egui::RichText::new("方向控制:").strong());
                    ui.add_space(4.0);

                    let dpad_btn_size = Vec2::new(44.0, 30.0);
                    let dpad_spacing = 4.0;
                    let dpad_w = dpad_btn_size.x * 3.0 + dpad_spacing * 2.0; // 140.0
                    let dpad_margin = ((ui.available_width() - dpad_w) / 2.0).max(0.0);

                    ui.horizontal(|ui| {
                        ui.add_space(dpad_margin);
                        egui::Grid::new("remote_dpad_grid")
                            .spacing([dpad_spacing, dpad_spacing])
                            .show(ui, |ui| {
                                ui.label("");
                                if ui
                                    .add(egui::Button::new("▲ 上").min_size(dpad_btn_size))
                                    .clicked()
                                {
                                    self.kernel.inject_remote_key(34);
                                    self.texture_dirty = true;
                                }
                                ui.label("");
                                ui.end_row();

                                if ui
                                    .add(egui::Button::new("◀ 左").min_size(dpad_btn_size))
                                    .clicked()
                                {
                                    self.kernel.inject_remote_key(36);
                                    self.texture_dirty = true;
                                }
                                if ui
                                    .add(egui::Button::new("确定").min_size(dpad_btn_size))
                                    .clicked()
                                {
                                    self.kernel.inject_remote_key(31);
                                    self.texture_dirty = true;
                                }
                                if ui
                                    .add(egui::Button::new("右 ▶").min_size(dpad_btn_size))
                                    .clicked()
                                {
                                    self.kernel.inject_remote_key(37);
                                    self.texture_dirty = true;
                                }
                                ui.end_row();

                                ui.label("");
                                if ui
                                    .add(egui::Button::new("▼ 下").min_size(dpad_btn_size))
                                    .clicked()
                                {
                                    self.kernel.inject_remote_key(35);
                                    self.texture_dirty = true;
                                }
                                ui.label("");
                                ui.end_row();
                            });
                    });

                    ui.add_space(6.0);
                    ui.horizontal(|ui| {
                        ui.add_space(dpad_margin);
                        if ui
                            .add(
                                egui::Button::new("⏹ 返回 / 退出 (32)")
                                    .min_size(Vec2::new(dpad_w, 28.0)),
                            )
                            .clicked()
                        {
                            self.kernel.inject_remote_key(32);
                            self.texture_dirty = true;
                        }
                    });

                    ui.separator();
                    ui.label(egui::RichText::new("数字按键 (0-9):").strong());
                    ui.add_space(4.0);

                    let num_btn_size = Vec2::new(44.0, 28.0);
                    let num_spacing = 4.0;
                    let num_w = num_btn_size.x * 3.0 + num_spacing * 2.0; // 140.0
                    let num_margin = ((ui.available_width() - num_w) / 2.0).max(0.0);

                    ui.horizontal(|ui| {
                        ui.add_space(num_margin);
                        egui::Grid::new("remote_num_pad")
                            .spacing([num_spacing, num_spacing])
                            .show(ui, |ui| {
                                for row in 0..3 {
                                    for col in 1..=3 {
                                        let num = row * 3 + col;
                                        if ui
                                            .add(
                                                egui::Button::new(format!("{}", num))
                                                    .min_size(num_btn_size),
                                            )
                                            .clicked()
                                        {
                                            self.kernel.inject_remote_key(num);
                                            self.texture_dirty = true;
                                        }
                                    }
                                    ui.end_row();
                                }
                                ui.label("");
                                if ui
                                    .add(egui::Button::new("0").min_size(num_btn_size))
                                    .clicked()
                                {
                                    self.kernel.inject_remote_key(0);
                                    self.texture_dirty = true;
                                }
                                ui.label("");
                                ui.end_row();
                            });
                    });

                    ui.add_space(6.0);
                    ui.separator();
                    let vm_status = match &self.kernel.vm.state {
                        crate::core::script_vm::VmState::Ready => "就绪".to_string(),
                        crate::core::script_vm::VmState::Running => "运行中".to_string(),
                        crate::core::script_vm::VmState::WaitingForKey { target_var }
                        | crate::core::script_vm::VmState::WaitingForKeyWithTimeout { target_var, .. } => {
                            format!("等待输入 -> {}", *target_var as char)
                        }
                        crate::core::script_vm::VmState::WaitingForDelay { .. } => {
                            "延时等待中".to_string()
                        }
                        crate::core::script_vm::VmState::WaitingForVideo => {
                            "🎬 视频播放中".to_string()
                        }
                        crate::core::script_vm::VmState::Finished => "已结束".to_string(),
                        crate::core::script_vm::VmState::PausedForAlert(_) => "⚠️ 暂停警告".to_string(),
                        crate::core::script_vm::VmState::Error(err) => err.clone(),
                    };
                    ui.label(egui::RichText::new(format!("脚本: {}", vm_status)).size(11.0).color(Color32::LIGHT_GRAY));
                });
        }

        // 6. Central Interactive Canvas Viewport
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE.fill(Color32::BLACK))
            .show(ui, |ui| {
                let avail_size = ui.available_size();
                if avail_size.x <= 1.0 || avail_size.y <= 1.0 {
                    return;
                }

                // Preserve aspect ratio
                let aspect = CANVAS_WIDTH as f32 / CANVAS_HEIGHT as f32;
                let mut disp_w = avail_size.x;
                let mut disp_h = disp_w / aspect;

                if disp_h > avail_size.y {
                    disp_h = avail_size.y;
                    disp_w = disp_h * aspect;
                }

                // Center in panel
                let offset_x = (avail_size.x - disp_w) * 0.5;
                let offset_y = (avail_size.y - disp_h) * 0.5;
                let origin = ui.max_rect().min + Vec2::new(offset_x, offset_y);
                let display_rect = Rect::from_min_size(origin, Vec2::new(disp_w, disp_h));

                let painter = ui.painter();

                // Draw background / video texture
                if let Some(texture) = &self.texture {
                    painter.image(
                        texture.id(),
                        display_rect,
                        Rect::from_min_max(Pos2::new(0.0, 0.0), Pos2::new(1.0, 1.0)),
                        Color32::WHITE,
                    );
                }

                // If empty state (no disc loaded and no video active)
                let is_empty = self.kernel.disc_root.as_os_str().is_empty()
                    && self.kernel.current_page.is_none()
                    && !self.kernel.is_video_active();

                if is_empty {
                    painter.text(
                        display_rect.center(),
                        egui::Align2::CENTER_CENTER,
                        "INSERT DISC...",
                        egui::FontId::monospace(22.0),
                        Color32::from_rgb(170, 185, 210),
                    );
                }

                // If video is active, keep canvas completely clean (no cursor, no hotspot overlays)
                if self.kernel.is_video_active() {
                    self.hovered_hotspot = None;
                } else {
                    // Render OSD Cursor if active (e.g. WEIGHT.CHM sex selection / height digit entry)
                    if let Some((cx, cy)) = self.kernel.cursor_pos {
                        let p1 = self.canvas_to_screen(cx, cy, display_rect);
                        let p2 = self.canvas_to_screen(cx + 12, cy + 12, display_rect);
                        let c_rect = Rect::from_two_pos(p1, p2);
                        painter.rect_filled(
                            c_rect,
                            2.0,
                            Color32::from_rgba_unmultiplied(255, 230, 0, 80),
                        );
                        painter.rect_stroke(
                            c_rect,
                            2.0,
                            Stroke::new(2.0, Color32::from_rgb(255, 230, 0)),
                            StrokeKind::Inside,
                        );
                        painter.text(
                            p1 - Vec2::new(4.0, 0.0),
                            egui::Align2::RIGHT_CENTER,
                            "▶",
                            egui::FontId::proportional(16.0),
                            Color32::from_rgb(255, 230, 0),
                        );
                    }

                    // Mouse interaction & Hit testing
                    let response = ui.interact(
                        display_rect,
                        ui.id().with("vcd_screen"),
                        egui::Sense::click_and_drag(),
                    );
                    let hover_pos = ctx.input(|i| i.pointer.hover_pos());

                    let mut current_hit_area = None;
                    let prev_hovered = self.hovered_hotspot.take();

                    if !self.kernel.is_vm_active() {
                        if let Some(mouse_pos) = hover_pos {
                            if let Some((cx, cy)) = self.screen_to_canvas(mouse_pos, display_rect) {
                                if let Some(area) = self.kernel.hit_test(cx, cy) {
                                    ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                                    self.hovered_hotspot = Some(area.target.clone());
                                    current_hit_area = Some(area.clone());
                                }
                            }
                        }
                    }

                    if prev_hovered != self.hovered_hotspot {
                        ctx.request_repaint();
                    }

                    // Handle click on hotspot
                    if response.clicked() && !self.kernel.is_vm_active() {
                        if let Some(area) = current_hit_area {
                            self.hovered_hotspot = None;
                            let target_upper = area.target.trim().to_uppercase();
                            let is_wav = target_upper.ends_with(".WAV");
                            let is_video = target_upper.ends_with(".DAT")
                                || target_upper.ends_with(".MPG")
                                || target_upper.ends_with(".MPEG")
                                || target_upper.starts_with("PLAYVIDEO:")
                                || target_upper.starts_with("MPEG:");
                            match self.kernel.activate_hotspot(&area) {
                                Ok(true) => {
                                    if is_wav {
                                        self.set_status(format!("播放音频: {}", area.target));
                                    } else if is_video || self.kernel.is_video_active() {
                                        self.set_status(format!("播放视频: {}", area.target));
                                        self.texture = None;
                                        self.texture_dirty = true;
                                    } else {
                                        self.set_status(format!("已跳转至: {}", area.target));
                                        self.texture = None;
                                        self.texture_dirty = true;
                                    }
                                    ctx.request_repaint();
                                }
                                Ok(false) => {
                                    self.set_status(format!("触发动作: {}", area.target));
                                    ctx.request_repaint();
                                }
                                Err(e) => {
                                    self.set_status(format!("跳转失败: {}", e));
                                }
                            }
                        }
                    }

                    // Debug: Draw Hotspots overlays
                    if self.show_hotspots {
                        let mut all_areas = Vec::new();
                        if let Some(doc) = &self.kernel.current_page {
                            all_areas.extend(doc.get_all_hotspots());
                        }
                        if let Some(doc) = &self.kernel.overlay_doc {
                            all_areas.extend(doc.get_all_hotspots());
                        }
                        for area in all_areas {
                                let (min_x, min_y, max_x, max_y) = area.display_bounds();
                                let p1 = self.canvas_to_screen(
                                    min_x,
                                    min_y,
                                    display_rect,
                                );
                                let p2 = self.canvas_to_screen(
                                    max_x,
                                    max_y,
                                    display_rect,
                                );
                                let r = Rect::from_two_pos(p1, p2);

                                let is_hovered = self.hovered_hotspot.as_deref() == Some(&area.target);
                                let stroke_color = if is_hovered {
                                    Color32::from_rgb(255, 230, 0)
                                } else {
                                    Color32::from_rgba_unmultiplied(0, 255, 255, 180)
                                };

                                painter.rect_stroke(
                                    r,
                                    2.0,
                                    Stroke::new(1.5, stroke_color),
                                    StrokeKind::Inside,
                                );
                                painter.rect_filled(
                                    r,
                                    2.0,
                                    Color32::from_rgba_unmultiplied(
                                        0,
                                        255,
                                        255,
                                        if is_hovered { 60 } else { 20 },
                                    ),
                                );

                                // Small label
                                let font_id = egui::FontId::proportional(11.0);
                                painter.text(
                                    r.min + Vec2::new(3.0, 2.0),
                                    egui::Align2::LEFT_TOP,
                                    &area.target,
                                    font_id,
                                    stroke_color,
                                );
                            }
                        }
                    }
                });

        // Handle borderless window edge resizing and outer frame stroke
        crate::ui::titlebar::handle_window_edge_resize(&ctx);
        crate::ui::titlebar::paint_window_frame_border(&ctx);
    }
}

impl eframe::App for VcdPlayerApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.show(ui);
    }
}

