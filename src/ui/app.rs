//! VCD 3.0 Player GUI application implementation using eframe/egui.

use crate::core::kernel::{CANVAS_HEIGHT, CANVAS_WIDTH, VcdKernel};
use eframe::egui::{
    self, Color32, ColorImage, Pos2, Rect, Stroke, StrokeKind, TextureOptions, Vec2,
};
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DrawerTab {
    #[default]
    Remote,
    Tracks,
}

pub struct VcdPlayerApp {
    pub kernel: VcdKernel,
    pub texture: Option<egui::TextureHandle>,
    pub texture_dirty: bool,
    pub show_hotspots: bool,
    pub show_metadata: bool,
    pub show_remote: bool,
    pub show_about: bool,
    pub drawer_tab: DrawerTab,
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

/// Renders a vector-drawn close button for the side panel header (immune to font missing glyph issues).
fn close_panel_button(ui: &mut egui::Ui) -> egui::Response {
    let size = Vec2::new(16.0, 16.0);
    let (rect, response) = ui.allocate_exact_size(size, egui::Sense::click());
    if ui.is_rect_visible(rect) {
        let is_hovered = response.hovered();
        let is_pressed = response.is_pointer_button_down_on();
        if is_pressed {
            ui.painter().rect_filled(rect, 2.0, Color32::from_rgb(200, 30, 45));
        } else if is_hovered {
            ui.painter().rect_filled(rect, 2.0, Color32::from_rgba_unmultiplied(255, 255, 255, 30));
        }
        let fg = if is_hovered {
            Color32::WHITE
        } else {
            Color32::from_rgb(160, 160, 160)
        };
        let stroke = Stroke::new(1.3, fg);
        let center = rect.center();
        let d = 3.2;
        ui.painter().line_segment(
            [center + Vec2::new(-d, -d), center + Vec2::new(d, d)],
            stroke,
        );
        ui.painter().line_segment(
            [center + Vec2::new(-d, d), center + Vec2::new(d, -d)],
            stroke,
        );
    }
    response
}

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
            drawer_tab: DrawerTab::Remote,
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
            drawer_tab: DrawerTab::Remote,
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
            } else if let Some(idx) = self.kernel.current_track_index {
                if let Some(t) = self.kernel.tracks.get(idx) {
                    format!("🎵 轨道 {:02} ({})", t.index, t.title)
                } else {
                    "未载入页面".to_string()
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
        self.kernel.tracks.clear();
        self.kernel.current_track_index = None;
        self.kernel.active_mode = crate::core::kernel::ActiveDiscMode::Vcd30Interactive;
        self.kernel.pbc = None;
        self.kernel.pbc_digit_buffer.clear();
        self.kernel.vm.terminate();
        self.texture = None;
        self.texture_dirty = true;
        self.idle_message = "光盘已弹出，请加载光盘".to_string();
        self.set_status("光盘已弹出，请加载光盘");
    }

    /// Toggles the side panel, dynamically adjusting the window width so the central canvas is not compressed.
    pub fn toggle_side_panel(&mut self, ctx: &egui::Context, open: bool) {
        if self.show_remote == open {
            return;
        }
        self.show_remote = open;
        let panel_width = 230.0;
        let is_maximized = ctx.input(|i| i.viewport().maximized).unwrap_or(false);
        if !is_maximized {
            let cur_size = ctx.viewport_rect().size();
            let new_w = if open {
                cur_size.x + panel_width
            } else {
                (cur_size.x - panel_width).max(500.0)
            };
            ctx.send_viewport_cmd(egui::viewport::ViewportCommand::InnerSize(Vec2::new(
                new_w, cur_size.y,
            )));
        }
    }

    /// Resets playback from the beginning in the current active mode.
    /// Mode switching is not performed here and must be user-triggered.
    pub fn reset_disc(&mut self, ctx: &egui::Context) {
        if !self.kernel.disc_root.as_os_str().is_empty() {
            if let Err(e) = self.kernel.restart_current_mode() {
                self.set_status(format!("重置失败: {}", e));
            } else {
                let mode_desc = self.kernel.active_mode.label();
                self.set_status(format!("已重新从头开始播放 ({})", mode_desc));
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

    /// Injects a remote control key and synchronizes status and canvas.
    pub fn send_remote_key(&mut self, key: i32, ctx: &egui::Context) {
        let old_track = self.kernel.current_track_index;
        let old_page = self.kernel.current_page_name.clone();
        self.kernel.inject_remote_key(key);
        if !self.kernel.pbc_digit_buffer.is_empty() {
            let buf_str: String = self
                .kernel
                .pbc_digit_buffer
                .iter()
                .map(|d| d.to_string())
                .collect();
            self.set_status(format!("输入曲目: {} (按 Enter 确认或等待)", buf_str));
        } else if self.kernel.current_track_index != old_track {
            if let Some(idx) = self.kernel.current_track_index {
                if let Some(t) = self.kernel.tracks.get(idx) {
                    self.set_status(format!("正在播放: 轨道 {:02} ({})", t.index, t.title));
                }
            }
        } else if self.kernel.current_page_name != old_page && !self.kernel.current_page_name.is_empty() {
            self.set_status(format!("已切换至: {}", self.kernel.current_page_name));
        }
        self.texture = None;
        self.texture_dirty = true;
        ctx.request_repaint();
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

        // Check for numeric input buffer timeout (e.g. 2.0s without Enter)
        if let Ok(true) = self.kernel.check_pbc_digit_timeout() {
            if let Some(idx) = self.kernel.current_track_index {
                if let Some(t) = self.kernel.tracks.get(idx) {
                    self.set_status(format!("正在播放: 轨道 {:02} ({})", t.index, t.title));
                }
            }
            self.texture = None;
            self.texture_dirty = true;
            ctx.request_repaint();
        } else if !self.kernel.pbc_digit_buffer.is_empty() {
            ctx.request_repaint_after(std::time::Duration::from_millis(100));
        }

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
        } else if let Some(idx) = self.kernel.current_track_index {
            if let Some(t) = self.kernel.tracks.get(idx) {
                format!("vcd30player - 轨道 {:02} ({})", t.index, t.title)
            } else {
                "vcd30player".to_string()
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

                // 重置按钮 Split Button (↺ 重置 ▼)
                ui.scope(|ui| {
                    ui.spacing_mut().item_spacing.x = 0.0;

                    let left_cr = egui::CornerRadius { nw: 4, ne: 0, sw: 4, se: 0 };
                    if ui
                        .add_enabled(is_loaded, egui::Button::new("↺ 重置").corner_radius(left_cr))
                        .on_hover_text("从头开始重新播放当前模式")
                        .clicked()
                    {
                        self.reset_disc(&ctx);
                    }

                    let right_cr = egui::CornerRadius { nw: 0, ne: 4, sw: 0, se: 4 };
                    ui.visuals_mut().widgets.inactive.corner_radius = right_cr;
                    ui.visuals_mut().widgets.hovered.corner_radius = right_cr;
                    ui.visuals_mut().widgets.active.corner_radius = right_cr;
                    ui.visuals_mut().widgets.open.corner_radius = right_cr;

                    ui.add_enabled_ui(is_loaded, |ui| {
                        ui.menu_button("▼", |ui| {
                            ui.visuals_mut().widgets.inactive.corner_radius = egui::CornerRadius::same(3);
                            ui.visuals_mut().widgets.hovered.corner_radius = egui::CornerRadius::same(3);

                            let supports_vcd30 = self.kernel.supports_vcd30();
                            let supports_vcd20 = self.kernel.supports_vcd20();
                            let supports_vcd10 = self.kernel.supports_vcd10();
                            let cur_mode = self.kernel.active_mode;
                            let mut option_count = 0;

                            if supports_vcd30 && cur_mode != crate::core::kernel::ActiveDiscMode::Vcd30Interactive {
                                option_count += 1;
                                if ui.button("💿 重置并恢复 VCD 3.0 互动模式").clicked() {
                                    ui.close();
                                    if let Err(e) = self.kernel.switch_active_mode(crate::core::kernel::ActiveDiscMode::Vcd30Interactive) {
                                        self.set_status(format!("恢复失败: {}", e));
                                    } else {
                                        self.set_status("已恢复 VCD 3.0 互动模式");
                                        self.texture = None;
                                        self.texture_dirty = true;
                                        ctx.request_repaint();
                                    }
                                }
                            }

                            if supports_vcd20 && cur_mode != crate::core::kernel::ActiveDiscMode::Vcd20Classic {
                                option_count += 1;
                                let label = if cur_mode == crate::core::kernel::ActiveDiscMode::Vcd10Linear && !supports_vcd30 {
                                    "🎬 重置并恢复 VCD 2.0 经典模式"
                                } else {
                                    "🎬 重置并使用 VCD 2.0 模式播放"
                                };
                                if ui.button(label).clicked() {
                                    ui.close();
                                    if let Err(e) = self.kernel.switch_active_mode(crate::core::kernel::ActiveDiscMode::Vcd20Classic) {
                                        self.set_status(format!("切换失败: {}", e));
                                    } else {
                                        self.set_status("已切换至 VCD 2.0 经典模式");
                                        self.texture = None;
                                        self.texture_dirty = true;
                                        ctx.request_repaint();
                                    }
                                }
                            }

                            if supports_vcd10 && cur_mode != crate::core::kernel::ActiveDiscMode::Vcd10Linear {
                                option_count += 1;
                                if ui.button("📺 重置并使用 VCD 1.0 模式播放").clicked() {
                                    ui.close();
                                    if let Err(e) = self.kernel.switch_active_mode(crate::core::kernel::ActiveDiscMode::Vcd10Linear) {
                                        self.set_status(format!("切换失败: {}", e));
                                    } else {
                                        self.set_status("已切换至 VCD 1.0 纯视频模式");
                                        self.texture = None;
                                        self.texture_dirty = true;
                                        ctx.request_repaint();
                                    }
                                }
                            }

                            if option_count == 0 {
                                ui.label("当前已是唯一的可用模式");
                            }
                        });
                    });
                });

                ui.add(egui::Separator::default().spacing(0.0));

                let is_vcd12 = is_loaded && self.kernel.active_mode != crate::core::kernel::ActiveDiscMode::Vcd30Interactive;

                // Video playback controls
                let mut stop_video = false;
                let mut prev_track = false;
                let mut next_track = false;

                if is_vcd12 {
                    let has_tracks = !self.kernel.tracks.is_empty();
                    if ui
                        .add_enabled(has_tracks, egui::Button::new("⏮"))
                        .on_hover_text("上一曲")
                        .clicked()
                    {
                        prev_track = true;
                    }
                }

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
                } else {
                    let can_start = is_vcd12 && !self.kernel.tracks.is_empty();
                    if ui
                        .add_enabled(can_start, egui::Button::new("▶"))
                        .on_hover_text("播放 (Space)")
                        .clicked()
                    {
                        let idx = self.kernel.current_track_index.unwrap_or(0);
                        let _ = self.kernel.play_track(idx);
                        if let Some(t) = self.kernel.tracks.get(idx) {
                            self.set_status(format!("正在播放: 轨道 {:02} ({})", t.index, t.title));
                        }
                        self.texture = None;
                        self.texture_dirty = true;
                        ctx.request_repaint();
                    }
                }

                if is_vcd12 {
                    let has_tracks = !self.kernel.tracks.is_empty();
                    if ui
                        .add_enabled(has_tracks, egui::Button::new("⏭"))
                        .on_hover_text("下一曲")
                        .clicked()
                    {
                        next_track = true;
                    }
                }

                let can_stop = self.kernel.active_video.is_some();
                if ui
                    .add_enabled(can_stop, egui::Button::new("⏹"))
                    .on_hover_text("停止 (ESC)")
                    .clicked()
                {
                    stop_video = true;
                }

                // Right side: Playback Mode (icon), Channel Mode (icon), Timecode, and Slider in remaining space
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    // 1. Rightmost: Playback Mode (Icon only)
                    let mode_btn = ui
                        .add_enabled(
                            is_loaded,
                            egui::Button::new(self.kernel.playback_mode.icon())
                                .min_size(Vec2::new(26.0, 0.0)),
                        )
                        .on_hover_text(format!(
                            "播放模式: {} (点击循环切换)",
                            self.kernel.playback_mode.description()
                        ));
                    if mode_btn.clicked() {
                        self.kernel.playback_mode = self.kernel.playback_mode.cycle();
                        self.set_status(format!("播放模式: {}", self.kernel.playback_mode.description()));
                    }

                    // 2. Channel Mode (Icon only)
                    let channel_btn = ui
                        .add_enabled(
                            is_loaded,
                            egui::Button::new(self.kernel.channel_mode.icon())
                                .min_size(Vec2::new(26.0, 0.0)),
                        )
                        .on_hover_text(format!(
                            "声道模式: {} (点击切换)",
                            self.kernel.channel_mode.description()
                        ));
                    if channel_btn.clicked() {
                        self.kernel.channel_mode = self.kernel.channel_mode.cycle();
                        if let Some(ref mut player) = self.kernel.active_video {
                            player.set_channel_mode(self.kernel.channel_mode);
                        }
                        self.set_status(format!("声道模式: {}", self.kernel.channel_mode.description()));
                    }

                    // Margin between timecode and channel/mode buttons
                    ui.add_space(6.0);

                    // 3. Timecode
                    let time_text = if let Some(ref player) = self.kernel.active_video {
                        let cur = player.current_time();
                        let dur = player.duration();
                        let cur_min = (cur / 60.0) as u32;
                        let cur_sec = (cur % 60.0) as u32;
                        let dur_min = (dur / 60.0) as u32;
                        let dur_sec = (dur % 60.0) as u32;
                        format!("{:02}:{:02} / {:02}:{:02}", cur_min, cur_sec, dur_min, dur_sec)
                    } else {
                        "--:-- / --:--".to_string()
                    };

                    if self.kernel.active_video.is_some() {
                        ui.label(egui::RichText::new(&time_text).monospace());
                    } else {
                        ui.label(egui::RichText::new(&time_text).monospace().color(Color32::DARK_GRAY));
                    }

                    // 4. Remaining middle space: Seek Slider
                    ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                        let avail_w = ui.available_width().max(40.0);
                        ui.spacing_mut().slider_width = avail_w;

                        if let Some(ref mut player) = self.kernel.active_video {
                            let dur = player.duration();
                            let mut seek_pos = player.current_time();
                            let slider = egui::Slider::new(&mut seek_pos, 0.0..=dur.max(1.0))
                                .show_value(false)
                                .text("");
                            if ui.add(slider).changed() {
                                player.seek(seek_pos);
                            }
                        } else {
                            let mut dummy_pos = 0.0;
                            ui.add_enabled(
                                false,
                                egui::Slider::new(&mut dummy_pos, 0.0..=1.0).show_value(false).text(""),
                            );
                        }
                    });
                });

                if prev_track {
                    let _ = self.kernel.play_prev_track();
                    if let Some(idx) = self.kernel.current_track_index {
                        if let Some(t) = self.kernel.tracks.get(idx) {
                            self.set_status(format!("正在播放: 轨道 {:02} ({})", t.index, t.title));
                        }
                    }
                    self.texture = None;
                    self.texture_dirty = true;
                    ctx.request_repaint();
                }

                if next_track {
                    let _ = self.kernel.play_next_track(false);
                    if let Some(idx) = self.kernel.current_track_index {
                        if let Some(t) = self.kernel.tracks.get(idx) {
                            self.set_status(format!("正在播放: 轨道 {:02} ({})", t.index, t.title));
                        }
                    }
                    self.texture = None;
                    self.texture_dirty = true;
                    ctx.request_repaint();
                }

                if stop_video {
                    if self.kernel.active_mode == crate::core::kernel::ActiveDiscMode::Vcd20Classic
                        && self.kernel.pbc.is_some()
                    {
                        let _ = self.kernel.handle_pbc_return();
                        if !self.kernel.current_page_name.is_empty() {
                            self.set_status(format!("已返回: {}", self.kernel.current_page_name));
                        } else {
                            self.set_status("已停止视频播放");
                        }
                    } else {
                        let _ = self.kernel.stop_video_and_exit();
                        self.set_status("已停止视频播放");
                    }
                    self.texture = None;
                    self.texture_dirty = true;
                    ctx.request_repaint();
                }
            });

            ui.separator();

            // Row 2: Navigation buttons, Debug checkboxes (right after Forward button), Page info & Status
            ui.horizontal(|ui| {
                let is_loaded = self.is_disc_loaded();
                let is_vcd30 = is_loaded && self.kernel.active_mode == crate::core::kernel::ActiveDiscMode::Vcd30Interactive;
                let is_vcd20 = is_loaded && self.kernel.active_mode == crate::core::kernel::ActiveDiscMode::Vcd20Classic;

                if is_vcd20 {
                    if ui
                        .add_enabled(is_loaded, egui::Button::new("PBC"))
                        .on_hover_text("呼出/返回 PBC 播放控制菜单")
                        .clicked()
                    {
                        if let Err(e) = self.kernel.trigger_pbc_menu() {
                            self.set_status(format!("PBC: {}", e));
                        } else {
                            self.set_status("已呼出 PBC 菜单");
                            self.texture = None;
                            self.texture_dirty = true;
                            ctx.request_repaint();
                        }
                    }
                    ui.add(egui::Separator::default().spacing(0.0));
                } else if is_vcd30 {
                    let can_back = !self.kernel.history_stack.is_empty();
                    let can_fwd = !self.kernel.forward_stack.is_empty();
                    let can_home = true;

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

                    if ui
                        .add_enabled(can_home, egui::Button::new("🏠 主页"))
                        .on_hover_text("返回主页")
                        .clicked()
                    {
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
                }

                // Requirement 3: 热区高亮和侧边面板开关放在最底部的工具条上，在 前进按钮的后面
                ui.checkbox(&mut self.show_hotspots, "🎯 热区高亮");
                let mut show_panel = self.show_remote;
                if ui
                    .checkbox(&mut show_panel, "🎮 侧边面板")
                    .on_hover_text("展开遥控器与曲目列表侧边栏")
                    .changed()
                {
                    self.toggle_side_panel(&ctx, show_panel);
                }

                ui.add(egui::Separator::default().spacing(0.0));

                // Right side: About button on the far right (after playback status bar, icon only)
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("ℹ").on_hover_text("关于软件").clicked() {
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
            let mut num_pressed = None;

            ctx.input(|i| {
                if i.key_pressed(egui::Key::Space) {
                    toggle_pause = true;
                } else if i.key_pressed(egui::Key::Escape) || i.key_pressed(egui::Key::Backspace) {
                    stop_video = true;
                } else if i.key_pressed(egui::Key::ArrowLeft) {
                    seek_delta = -5.0;
                } else if i.key_pressed(egui::Key::ArrowRight) {
                    seek_delta = 5.0;
                } else if self.kernel.active_mode == crate::core::kernel::ActiveDiscMode::Vcd20Classic
                    || self.kernel.active_mode == crate::core::kernel::ActiveDiscMode::Vcd10Linear
                {
                    if i.key_pressed(egui::Key::Enter) {
                        num_pressed = Some(31);
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
                                num_pressed = Some(val);
                                break;
                            }
                        }
                    }
                }
            });

            if let Some(num) = num_pressed {
                self.send_remote_key(num, &ctx);
            }

            if toggle_pause {
                if let Some(ref mut player) = self.kernel.active_video {
                    player.toggle_play_pause();
                }
            }
            if stop_video {
                if self.kernel.active_mode == crate::core::kernel::ActiveDiscMode::Vcd20Classic
                    && self.kernel.pbc.is_some()
                {
                    let _ = self.kernel.handle_pbc_return();
                } else {
                    let _ = self.kernel.stop_video_and_exit();
                }
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
                self.send_remote_key(key, &ctx);
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

        // 5. Right Sidebar: Combined Remote Control & Track Drawer
        if self.show_remote {
            let mut close_panel = false;
            egui::Panel::right("remote_control_panel")
                .resizable(false)
                .default_size(230.0)
                .show(ui, |ui| {
                    ui.add_space(4.0);

                    // Tab Selector: Remote vs Tracks
                    ui.horizontal(|ui| {
                        ui.selectable_value(&mut self.drawer_tab, DrawerTab::Remote, "🎮 遥控器");
                        let track_count = self.kernel.tracks.len();
                        let track_title = if track_count > 0 {
                            format!("📑 曲目 ({})", track_count)
                        } else {
                            "📑 曲目列表".to_string()
                        };
                        ui.selectable_value(&mut self.drawer_tab, DrawerTab::Tracks, track_title);

                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if close_panel_button(ui).on_hover_text("关闭侧面板").clicked() {
                                close_panel = true;
                            }
                        });
                    });
                    ui.separator();

                    match self.drawer_tab {
                        DrawerTab::Remote => {

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
                                    self.send_remote_key(34, &ctx);
                                }
                                ui.label("");
                                ui.end_row();

                                if ui
                                    .add(egui::Button::new("◀ 左").min_size(dpad_btn_size))
                                    .clicked()
                                {
                                    self.send_remote_key(36, &ctx);
                                }
                                if ui
                                    .add(egui::Button::new("确定").min_size(dpad_btn_size))
                                    .clicked()
                                {
                                    self.send_remote_key(31, &ctx);
                                }
                                if ui
                                    .add(egui::Button::new("右 ▶").min_size(dpad_btn_size))
                                    .clicked()
                                {
                                    self.send_remote_key(37, &ctx);
                                }
                                ui.end_row();

                                ui.label("");
                                if ui
                                    .add(egui::Button::new("▼ 下").min_size(dpad_btn_size))
                                    .clicked()
                                {
                                    self.send_remote_key(35, &ctx);
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
                            self.send_remote_key(32, &ctx);
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
                                            self.send_remote_key(num, &ctx);
                                        }
                                    }
                                    ui.end_row();
                                }
                                ui.label("");
                                if ui
                                    .add(egui::Button::new("0").min_size(num_btn_size))
                                    .clicked()
                                {
                                    self.send_remote_key(0, &ctx);
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
                        }
                        DrawerTab::Tracks => {
                            ui.horizontal(|ui| {
                                ui.label(
                                    egui::RichText::new(format!(
                                        "当前模式: {}",
                                        self.kernel.active_mode.label()
                                    ))
                                    .size(11.0)
                                    .color(Color32::LIGHT_GRAY),
                                );
                            });
                            ui.add_space(4.0);

                            if self.kernel.tracks.is_empty() {
                                ui.vertical_centered(|ui| {
                                    ui.add_space(20.0);
                                    ui.label(egui::RichText::new("暂未发现音视频曲目").color(Color32::GRAY));
                                    ui.label(
                                        egui::RichText::new("请加载有效光盘或视频文件")
                                            .size(11.0)
                                            .color(Color32::DARK_GRAY),
                                    );
                                });
                            } else {
                                let mut selected_track_idx = None;
                                let mut selected_track_title = String::new();

                                egui::ScrollArea::vertical()
                                    .auto_shrink([false, false])
                                    .show(ui, |ui| {
                                        for (i, track) in self.kernel.tracks.iter().enumerate() {
                                            let is_current =
                                                self.kernel.current_track_index == Some(i);
                                            let time_str = track.msf_start.as_deref().unwrap_or("");
                                            let item_label = if time_str.is_empty() {
                                                format!("{:02}. {}", track.index, track.title)
                                            } else {
                                                format!("{:02}. {} [{}]", track.index, track.title, time_str)
                                            };
                                            if ui.selectable_label(is_current, item_label).clicked() {
                                                selected_track_idx = Some(i);
                                                selected_track_title = track.title.clone();
                                            }
                                        }
                                    });

                                if let Some(idx) = selected_track_idx {
                                    let _ = self.kernel.play_track(idx);
                                    self.set_status(format!("正在播放: {}", selected_track_title));
                                    self.texture = None;
                                    self.texture_dirty = true;
                                    ctx.request_repaint();
                                }
                            }
                        }
                    }
                });
            if close_panel {
                self.toggle_side_panel(&ctx, false);
            }
        }

        // 6. Central Interactive Canvas Viewport
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE.fill(Color32::BLACK))
            .show(ui, |ui| {
                let avail_size = ui.available_size();
                if avail_size.x <= 1.0 || avail_size.y <= 1.0 {
                    return;
                }

                // Preserve 4:3 display aspect ratio (standard Video CD display aspect ratio)
                let aspect = 4.0 / 3.0;
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

