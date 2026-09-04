//! VCD 3.0 Player GUI application implementation using eframe/egui.

use crate::core::kernel::{CANVAS_HEIGHT, CANVAS_WIDTH, VcdKernel};
use eframe::egui::{
    self, Color32, ColorImage, Pos2, Rect, Stroke, StrokeKind, TextureOptions, Vec2,
};
use std::path::{Path, PathBuf};

pub struct VcdPlayerApp {
    pub kernel: VcdKernel,
    pub texture: Option<egui::TextureHandle>,
    pub texture_dirty: bool,
    pub show_hotspots: bool,
    pub show_metadata: bool,
    pub hovered_hotspot: Option<String>,
    pub status_message: String,
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

impl VcdPlayerApp {
    pub fn new(cc: &eframe::CreationContext<'_>, initial_disc: Option<PathBuf>) -> Self {
        setup_custom_fonts(&cc.egui_ctx);

        let mut kernel = VcdKernel::new();
        let mut status = "就绪 - 请打开光盘目录".to_string();

        if let Some(root) = initial_disc {
            if root.exists() {
                if let Err(e) = kernel.open_disc(root.clone()) {
                    status = format!("打开光盘失败: {}", e);
                } else {
                    status = format!("已加载光盘: {}", root.display());
                }
            }
        } else {
            // Check default I:\ drive
            let default_drive = Path::new(r"I:\");
            if default_drive.exists() {
                if let Ok(()) = kernel.open_disc(default_drive.to_path_buf()) {
                    status = "已自动载入光驱 I:\\".to_string();
                }
            }
        }

        Self {
            kernel,
            texture: None,
            texture_dirty: true,
            show_hotspots: false,
            show_metadata: false,
            hovered_hotspot: None,
            status_message: status,
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
}

impl eframe::App for VcdPlayerApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();

        if self.texture_dirty {
            self.refresh_texture(&ctx);
        }

        // 1. Top Menu Bar Panel
        egui::Panel::top("top_menu_bar").show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.menu_button("文件(F)", |ui| {
                    if ui.button("📁 打开光盘目录 (Open Disc)...").clicked() {
                        ui.close();
                        if let Some(folder) = rfd::FileDialog::new().pick_folder() {
                            if let Err(e) = self.kernel.open_disc(folder.clone()) {
                                self.status_message = format!("打开失败: {}", e);
                            } else {
                                self.status_message = format!("已打开: {}", folder.display());
                                self.texture_dirty = true;
                            }
                        }
                    }

                    if ui.button("📄 打开单个 .CHM 页面...").clicked() {
                        ui.close();
                        if let Some(file) = rfd::FileDialog::new()
                            .add_filter("VCD30 Compiled HTML", &["chm", "CHM"])
                            .pick_file()
                        {
                            if let Some(name) = file.file_name().and_then(|s| s.to_str()) {
                                if let Err(e) = self.kernel.load_page(name, true) {
                                    self.status_message = format!("加载页面失败: {}", e);
                                } else {
                                    self.status_message = format!("已加载: {}", name);
                                    self.texture_dirty = true;
                                }
                            }
                        }
                    }

                    ui.separator();
                    if ui.button("❌ 退出 (Exit)").clicked() {
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                });

                ui.menu_button("导航(N)", |ui| {
                    let can_back = !self.kernel.history_stack.is_empty();
                    let can_fwd = !self.kernel.forward_stack.is_empty();

                    if ui
                        .add_enabled(can_back, egui::Button::new("⏮ 后退 (Back)"))
                        .clicked()
                    {
                        ui.close();
                        if let Ok(true) = self.kernel.go_back() {
                            self.texture_dirty = true;
                        }
                    }

                    if ui
                        .add_enabled(can_fwd, egui::Button::new("⏭ 前进 (Forward)"))
                        .clicked()
                    {
                        ui.close();
                        if let Ok(true) = self.kernel.go_forward() {
                            self.texture_dirty = true;
                        }
                    }

                    if ui.button("🏠 主页 (Home)").clicked() {
                        ui.close();
                        if let Ok(()) = self.kernel.go_home() {
                            self.texture_dirty = true;
                        }
                    }
                });

                ui.menu_button("视图(V)", |ui| {
                    if ui
                        .checkbox(&mut self.show_hotspots, "显示交互热区边框 (Debug Hotspots)")
                        .clicked()
                    {
                        ui.close();
                    }
                    if ui
                        .checkbox(&mut self.show_metadata, "显示页面元数据面板")
                        .clicked()
                    {
                        ui.close();
                    }
                });

                ui.menu_button("帮助(H)", |ui| {
                    if ui.button("ℹ 关于 VCD 3.0 现代模拟器").clicked() {
                        ui.close();
                        self.status_message =
                            "vcd30player Runtime (Rust) v0.1.0".to_string();
                    }
                });

                // Quick toggle on right
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.checkbox(&mut self.show_hotspots, "🎯 热区高亮");
                });
            });
        });

        // 2. Bottom Navigation & Status Bar Panel
        egui::Panel::bottom("bottom_bar").show(ui, |ui| {
            ui.horizontal(|ui| {
                let can_back = !self.kernel.history_stack.is_empty();
                let can_fwd = !self.kernel.forward_stack.is_empty();

                if ui
                    .add_enabled(can_back, egui::Button::new("⏮ 后退"))
                    .clicked()
                {
                    if let Ok(true) = self.kernel.go_back() {
                        self.texture_dirty = true;
                    }
                }

                if ui.button("🏠 主页").clicked() {
                    if let Ok(()) = self.kernel.go_home() {
                        self.texture_dirty = true;
                    }
                }

                if ui
                    .add_enabled(can_fwd, egui::Button::new("⏭ 前进"))
                    .clicked()
                {
                    if let Ok(true) = self.kernel.go_forward() {
                        self.texture_dirty = true;
                    }
                }

                ui.separator();

                // Status text
                let page_info = if !self.kernel.current_page_name.is_empty() {
                    let title = self
                        .kernel
                        .current_page
                        .as_ref()
                        .map(|d| d.title.as_str())
                        .unwrap_or("");
                    format!("📄 {} ({})", self.kernel.current_page_name, title)
                } else {
                    "未载入页面".to_string()
                };

                ui.label(egui::RichText::new(page_info).strong());

                ui.separator();
                ui.label(&self.status_message);

                if let Some(target) = &self.hovered_hotspot {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(
                            egui::RichText::new(format!("👉 目标: {}", target))
                                .color(Color32::from_rgb(0, 220, 255)),
                        );
                    });
                }
            });
        });

        // 3. Central Interactive Canvas Viewport
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
                let origin = ui.min_rect().min + Vec2::new(offset_x, offset_y);
                let display_rect = Rect::from_min_size(origin, Vec2::new(disp_w, disp_h));

                let painter = ui.painter();

                // Draw background texture
                if let Some(texture) = &self.texture {
                    painter.image(
                        texture.id(),
                        display_rect,
                        Rect::from_min_max(Pos2::new(0.0, 0.0), Pos2::new(1.0, 1.0)),
                        Color32::WHITE,
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
                self.hovered_hotspot = None;

                if let Some(mouse_pos) = hover_pos {
                    if let Some((cx, cy)) = self.screen_to_canvas(mouse_pos, display_rect) {
                        if let Some(area) = self.kernel.hit_test(cx, cy) {
                            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                            self.hovered_hotspot = Some(area.target.clone());
                            current_hit_area = Some(area.clone());
                        }
                    }
                }

                // Handle click on hotspot
                if response.clicked() {
                    if let Some(area) = current_hit_area {
                        match self.kernel.activate_hotspot(&area) {
                            Ok(true) => {
                                self.status_message = format!("已跳转至: {}", area.target);
                                self.texture_dirty = true;
                            }
                            Ok(false) => {
                                self.status_message = format!("触发动作: {}", area.target);
                            }
                            Err(e) => {
                                self.status_message = format!("跳转失败: {}", e);
                            }
                        }
                    }
                }

                // Debug: Draw Hotspots overlays
                if self.show_hotspots {
                    if let Some(doc) = &self.kernel.current_page {
                        for area in doc.get_all_hotspots() {
                            let p1 = self.canvas_to_screen(
                                area.x1.min(area.x2),
                                area.y1.min(area.y2),
                                display_rect,
                            );
                            let p2 = self.canvas_to_screen(
                                area.x1.max(area.x2),
                                area.y1.max(area.y2),
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
    }
}
