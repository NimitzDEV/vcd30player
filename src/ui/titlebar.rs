use eframe::egui::{
    self, Color32, CursorIcon, Margin, Pos2, Rect, Response, Sense, Stroke, StrokeKind, Vec2,
};
use egui::viewport::{ResizeDirection, ViewportCommand};

/// Renders the custom window title bar with centered title and matching single toolbar height.
pub fn show_title_bar(ui: &mut egui::Ui, title: &str) {
    let ctx = ui.ctx().clone();
    let is_maximized = ctx.input(|i| i.viewport().maximized).unwrap_or(false);

    let frame = egui::Frame::NONE
        .fill(Color32::from_rgb(24, 25, 29))
        .inner_margin(Margin::ZERO);

    egui::Panel::top("custom_title_bar")
        .frame(frame)
        .show_separator_line(true)
        .show(ui, |ui| {
            let titlebar_height = 26.0;
            ui.set_height(titlebar_height);

            let titlebar_rect = Rect::from_min_size(
                ui.min_rect().min,
                Vec2::new(ui.available_width(), titlebar_height),
            );

            // Right side: Window Control Buttons (Min, Max/Restore, Close)
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.spacing_mut().item_spacing.x = 0.0;

                // Close Button
                if title_bar_button(ui, TitleButtonType::Close, "关闭 (Alt+F4)").clicked() {
                    ctx.send_viewport_cmd(ViewportCommand::Close);
                }

                // Maximize / Restore Button
                let (max_btn, max_tooltip) = if is_maximized {
                    (TitleButtonType::Restore, "向下还原")
                } else {
                    (TitleButtonType::Maximize, "最大化")
                };
                if title_bar_button(ui, max_btn, max_tooltip).clicked() {
                    ctx.send_viewport_cmd(ViewportCommand::Maximized(!is_maximized));
                }

                // Minimize Button
                if title_bar_button(ui, TitleButtonType::Minimize, "最小化").clicked() {
                    ctx.send_viewport_cmd(ViewportCommand::Minimized(true));
                }

                // Middle/Drag area takes all remaining width to the left of the control buttons
                let drag_width = ui.available_width();
                if drag_width > 0.0 {
                    let (_, drag_response) = ui.allocate_exact_size(
                        Vec2::new(drag_width, titlebar_height),
                        Sense::click_and_drag(),
                    );
                    if drag_response.drag_started_by(egui::PointerButton::Primary) {
                        ctx.send_viewport_cmd(ViewportCommand::StartDrag);
                    }
                    if drag_response.double_clicked() {
                        ctx.send_viewport_cmd(ViewportCommand::Maximized(!is_maximized));
                    }
                }
            });

            let painter = ui.painter();

            // Left icon at x = min.x + 14.0
            painter.text(
                Pos2::new(titlebar_rect.min.x + 14.0, titlebar_rect.center().y),
                egui::Align2::CENTER_CENTER,
                "💿",
                egui::FontId::proportional(12.0),
                Color32::from_rgb(180, 185, 195),
            );

            // Centered title in the exact middle of the entire titlebar width
            painter.text(
                titlebar_rect.center(),
                egui::Align2::CENTER_CENTER,
                title,
                egui::FontId::proportional(12.0),
                Color32::from_rgb(215, 220, 230),
            );
        });
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum TitleButtonType {
    Minimize,
    Maximize,
    Restore,
    Close,
}

/// Custom titlebar button with hover, press, and crisp vector-rendered symbols.
fn title_bar_button(
    ui: &mut egui::Ui,
    btn_type: TitleButtonType,
    tooltip: &str,
) -> Response {
    let desired_size = Vec2::new(40.0, 26.0);
    let (rect, response) = ui.allocate_exact_size(desired_size, Sense::click());

    if ui.is_rect_visible(rect) {
        let is_hovered = response.hovered();
        let is_pressed = response.is_pointer_button_down_on();
        let is_close = btn_type == TitleButtonType::Close;

        let bg_color = if is_close {
            if is_pressed {
                Color32::from_rgb(200, 30, 45)
            } else if is_hovered {
                Color32::from_rgb(232, 17, 35)
            } else {
                Color32::TRANSPARENT
            }
        } else {
            if is_pressed {
                Color32::from_rgba_unmultiplied(255, 255, 255, 38)
            } else if is_hovered {
                Color32::from_rgba_unmultiplied(255, 255, 255, 22)
            } else {
                Color32::TRANSPARENT
            }
        };

        if bg_color != Color32::TRANSPARENT {
            ui.painter().rect_filled(rect, 0.0, bg_color);
        }

        let fg_color = if is_hovered {
            Color32::WHITE
        } else {
            Color32::from_rgb(180, 185, 195)
        };

        let stroke = Stroke::new(1.1, fg_color);
        let center = rect.center();

        match btn_type {
            TitleButtonType::Minimize => {
                // Horizontal line: 9px wide
                ui.painter().line_segment(
                    [center + Vec2::new(-4.5, 0.0), center + Vec2::new(4.5, 0.0)],
                    stroke,
                );
            }
            TitleButtonType::Maximize => {
                // Square: 9x9 px
                let sq_rect = Rect::from_center_size(center, Vec2::new(9.0, 9.0));
                ui.painter().rect_stroke(sq_rect, 0.0, stroke, StrokeKind::Inside);
            }
            TitleButtonType::Restore => {
                let panel_fill = Color32::from_rgb(24, 25, 29);
                let actual_bg = if bg_color != Color32::TRANSPARENT {
                    bg_color
                } else {
                    panel_fill
                };

                // Back square: 7x7 px, offset top-right
                let back_sq = Rect::from_min_size(center + Vec2::new(-1.5, -4.5), Vec2::new(7.0, 7.0));
                ui.painter().rect_stroke(back_sq, 0.0, stroke, StrokeKind::Inside);

                // Front square: 7x7 px, offset bottom-left
                let front_sq = Rect::from_min_size(center + Vec2::new(-4.5, -1.5), Vec2::new(7.0, 7.0));
                ui.painter().rect_filled(front_sq, 0.0, actual_bg);
                ui.painter().rect_stroke(front_sq, 0.0, stroke, StrokeKind::Inside);
            }
            TitleButtonType::Close => {
                // Diagonal X: 8x8 px
                let d = 4.0;
                ui.painter().line_segment(
                    [center + Vec2::new(-d, -d), center + Vec2::new(d, d)],
                    stroke,
                );
                ui.painter().line_segment(
                    [center + Vec2::new(-d, d), center + Vec2::new(d, -d)],
                    stroke,
                );
            }
        }
    }

    response.on_hover_text(tooltip)
}

/// Handles window edge and corner resizing by detecting pointer hover near window boundaries.
pub fn handle_window_edge_resize(ctx: &egui::Context) {
    let is_maximized = ctx.input(|i| i.viewport().maximized).unwrap_or(false);
    let is_fullscreen = ctx.input(|i| i.viewport().fullscreen).unwrap_or(false);
    if is_maximized || is_fullscreen {
        return;
    }

    let screen_rect = ctx.viewport_rect();
    let pointer_pos = ctx.input(|i| i.pointer.latest_pos());
    let Some(pos) = pointer_pos else {
        return;
    };

    if !screen_rect.contains(pos) {
        return;
    }

    let margin = 6.0;
    let on_left = pos.x <= screen_rect.min.x + margin;
    let on_right = pos.x >= screen_rect.max.x - margin;
    let on_top = pos.y <= screen_rect.min.y + margin;
    let on_bottom = pos.y >= screen_rect.max.y - margin;

    // Avoid overriding close/min/max buttons in top-right area (3 * 40.0 = 120.0 px)
    let in_titlebar_controls = on_top && pos.x >= screen_rect.max.x - 125.0;

    let resize_info = if on_top && on_left {
        Some((ResizeDirection::NorthWest, CursorIcon::ResizeNorthWest))
    } else if on_top && on_right && !in_titlebar_controls {
        Some((ResizeDirection::NorthEast, CursorIcon::ResizeNorthEast))
    } else if on_bottom && on_left {
        Some((ResizeDirection::SouthWest, CursorIcon::ResizeSouthWest))
    } else if on_bottom && on_right {
        Some((ResizeDirection::SouthEast, CursorIcon::ResizeSouthEast))
    } else if on_top && !in_titlebar_controls {
        Some((ResizeDirection::North, CursorIcon::ResizeNorth))
    } else if on_bottom {
        Some((ResizeDirection::South, CursorIcon::ResizeSouth))
    } else if on_left {
        Some((ResizeDirection::West, CursorIcon::ResizeWest))
    } else if on_right {
        Some((ResizeDirection::East, CursorIcon::ResizeEast))
    } else {
        None
    };

    if let Some((dir, cursor)) = resize_info {
        ctx.set_cursor_icon(cursor);
        if ctx.input(|i| i.pointer.primary_down() && i.pointer.any_pressed()) {
            ctx.send_viewport_cmd(ViewportCommand::BeginResize(dir));
        }
    }
}

/// Paints a subtle 1px window border around the entire viewport for borderless mode.
pub fn paint_window_frame_border(ctx: &egui::Context) {
    let is_maximized = ctx.input(|i| i.viewport().maximized).unwrap_or(false);
    let is_fullscreen = ctx.input(|i| i.viewport().fullscreen).unwrap_or(false);
    if !is_maximized && !is_fullscreen {
        let screen_rect = ctx.viewport_rect();
        let painter = ctx.layer_painter(egui::LayerId::new(
            egui::Order::Foreground,
            egui::Id::new("window_outer_border"),
        ));
        painter.rect_stroke(
            screen_rect,
            0.0,
            Stroke::new(1.0, Color32::from_rgb(55, 58, 65)),
            StrokeKind::Inside,
        );
    }
}
