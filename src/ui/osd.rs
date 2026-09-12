//! Retro phosphor green CRT On-Screen Display (OSD) engine for VCD player.
//!
//! Replicates the iconic retro CRT/VCD player on-screen display:
//! - Phosphor/fluorescent green text (`#39FF14`) with 4-directional black drop-shadow/outline.
//! - Monospace character generator styling.
//! - Timed auto-fade display (2.0s solid, 0.5s fade out).
//! - Top-left transient action status (PLAY, PAUSE, STOP, AUDIO, PBC, INPUT, etc.).
//! - Optional top-right persistent track & timecode display (`AlwaysInfo` mode).

use eframe::egui::{self, Color32, Pos2, Rect, Vec2};
use std::time::{Duration, Instant};

/// Iconic fluorescent green phosphor color (#39FF14)
pub const OSD_GREEN: Color32 = Color32::from_rgb(57, 255, 20);

/// Phosphor yellow for warnings or special notices
pub const OSD_YELLOW: Color32 = Color32::from_rgb(255, 230, 0);

/// Duration for which OSD stays at full opacity (2.0s)
pub const OSD_SOLID_DURATION: Duration = Duration::from_millis(2000);

/// Duration of the fade-out animation (0.2s)
pub const OSD_FADE_DURATION: Duration = Duration::from_millis(100);

/// Total OSD lifetime (2.2s)
pub const OSD_TOTAL_DURATION: Duration = Duration::from_millis(2200);

/// OSD operating mode
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum OsdMode {
    /// Auto-hide on action (default retro behavior)
    #[default]
    Auto,
    /// Persistent track and timecode display on top-right, plus transient status on top-left
    AlwaysInfo,
    /// OSD completely disabled
    Disabled,
}

impl OsdMode {
    /// Cycles to the next OSD mode (Auto -> AlwaysInfo -> Disabled -> Auto)
    pub fn cycle(self) -> Self {
        match self {
            OsdMode::Auto => OsdMode::AlwaysInfo,
            OsdMode::AlwaysInfo => OsdMode::Disabled,
            OsdMode::Disabled => OsdMode::Auto,
        }
    }

    /// Toolbar icon representing this mode
    pub fn icon(self) -> &'static str {
        match self {
            OsdMode::Auto => "📺",
            OsdMode::AlwaysInfo => "⏱",
            OsdMode::Disabled => "🚫",
        }
    }

    /// Detailed description for hover tooltips
    pub fn tooltip(self) -> &'static str {
        match self {
            OsdMode::Auto => "OSD 屏幕提示: 自动隐藏 (点击切换，快捷键 O)",
            OsdMode::AlwaysInfo => "OSD 屏幕提示: 常驻时间码 (点击切换，快捷键 O)",
            OsdMode::Disabled => "OSD 屏幕提示: 已关闭 (点击切换，快捷键 O)",
        }
    }

    /// Short label displayed on OSD screen when toggled
    pub fn osd_label(self) -> &'static str {
        match self {
            OsdMode::Auto => "OSD: AUTO",
            OsdMode::AlwaysInfo => "OSD: INFO",
            OsdMode::Disabled => "OSD: OFF",
        }
    }
}

/// A single active OSD notification
#[derive(Debug, Clone)]
pub struct OsdItem {
    pub text: String,
    pub created_at: Instant,
    pub color: Color32,
}

impl OsdItem {
    pub fn new(text: impl Into<String>, color: Color32) -> Self {
        Self {
            text: text.into(),
            created_at: Instant::now(),
            color,
        }
    }

    /// Computes current alpha (0.0 to 1.0)
    pub fn alpha(&self) -> f32 {
        let elapsed = self.created_at.elapsed();
        if elapsed < OSD_SOLID_DURATION {
            1.0
        } else if elapsed < OSD_TOTAL_DURATION {
            let fade_elapsed = elapsed - OSD_SOLID_DURATION;
            let frac = fade_elapsed.as_secs_f32() / OSD_FADE_DURATION.as_secs_f32();
            (1.0 - frac).clamp(0.0, 1.0)
        } else {
            0.0
        }
    }

    pub fn is_expired(&self) -> bool {
        self.created_at.elapsed() >= OSD_TOTAL_DURATION
    }
}

/// OSD manager controlling state and rendering
pub struct OsdManager {
    pub mode: OsdMode,
    current: Option<OsdItem>,
}

impl Default for OsdManager {
    fn default() -> Self {
        Self::new()
    }
}

impl OsdManager {
    pub fn new() -> Self {
        Self {
            mode: OsdMode::Auto,
            current: None,
        }
    }

    /// Displays a new OSD message in classic phosphor green
    pub fn show(&mut self, text: impl Into<String>) {
        if self.mode != OsdMode::Disabled {
            self.current = Some(OsdItem::new(text, OSD_GREEN));
        }
    }

    /// Displays a new OSD message with a custom color (e.g. yellow for alerts)
    pub fn show_colored(&mut self, text: impl Into<String>, color: Color32) {
        if self.mode != OsdMode::Disabled {
            self.current = Some(OsdItem::new(text, color));
        }
    }

    /// Cycles the OSD mode and displays a confirmation OSD
    pub fn toggle_mode(&mut self) -> OsdMode {
        self.mode = self.mode.cycle();
        if self.mode != OsdMode::Disabled {
            self.current = Some(OsdItem::new(self.mode.osd_label(), OSD_GREEN));
        } else {
            self.current = None;
        }
        self.mode
    }

    /// Clears any active OSD message immediately
    pub fn clear(&mut self) {
        self.current = None;
    }

    /// Checks whether an OSD message is actively displayed or fading
    pub fn is_active(&self) -> bool {
        if self.mode == OsdMode::Disabled {
            return false;
        }
        self.current
            .as_ref()
            .map_or(false, |item| !item.is_expired())
    }

    /// Renders the OSD overlay onto the given canvas viewport `display_rect`.
    ///
    /// If an item is in its fade-out phase, `ctx.request_repaint()` is automatically
    /// invoked to drive smooth 60fps fade animation.
    pub fn render(
        &mut self,
        ctx: &egui::Context,
        painter: &egui::Painter,
        display_rect: Rect,
        persistent_info: Option<&str>,
    ) {
        if self.mode == OsdMode::Disabled {
            return;
        }

        let font_id = egui::FontId::monospace(19.0);
        let padding = Vec2::new(16.0, 14.0);

        // 1. Render persistent track / timecode info on top-right if in AlwaysInfo mode
        if self.mode == OsdMode::AlwaysInfo {
            if let Some(info) = persistent_info {
                if !info.is_empty() {
                    let top_right = Pos2::new(
                        display_rect.max.x - padding.x,
                        display_rect.min.y + padding.y,
                    );
                    paint_osd_text(
                        painter,
                        top_right,
                        egui::Align2::RIGHT_TOP,
                        info,
                        font_id.clone(),
                        OSD_GREEN,
                        1.0,
                    );
                }
            }
        }

        // 2. Render transient OSD action message on top-left
        if let Some(item) = &self.current {
            let alpha = item.alpha();
            if alpha > 0.0 {
                let top_left = display_rect.min + padding;
                paint_osd_text(
                    painter,
                    top_left,
                    egui::Align2::LEFT_TOP,
                    &item.text,
                    font_id,
                    item.color,
                    alpha,
                );

                // If currently in the fading window (alpha < 1.0), request continuous repainting
                if alpha < 1.0 {
                    ctx.request_repaint();
                } else {
                    // Schedule repaint when the solid period ends and fade starts
                    let elapsed = item.created_at.elapsed();
                    let remaining_solid = OSD_SOLID_DURATION.saturating_sub(elapsed);
                    ctx.request_repaint_after(remaining_solid);
                }
            } else {
                self.current = None;
            }
        }
    }
}

/// Draws sharp retro CRT phosphor text with 4-directional outline and drop shadow for maximum legibility.
fn paint_osd_text(
    painter: &egui::Painter,
    pos: Pos2,
    align: egui::Align2,
    text: &str,
    font_id: egui::FontId,
    color: Color32,
    alpha: f32,
) {
    if alpha <= 0.0 || text.is_empty() {
        return;
    }

    let shadow_alpha = (230.0 * alpha) as u8;
    let shadow_color = Color32::from_rgba_unmultiplied(0, 0, 0, shadow_alpha);

    // 4-directional outline + diagonal drop shadow for perfect contrast over any video scene
    let offsets = [
        Vec2::new(-1.0, 0.0),
        Vec2::new(1.0, 0.0),
        Vec2::new(0.0, -1.0),
        Vec2::new(0.0, 1.0),
        Vec2::new(2.0, 2.0),
    ];

    for offset in offsets {
        painter.text(pos + offset, align, text, font_id.clone(), shadow_color);
    }

    let fg_color =
        Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), (255.0 * alpha) as u8);
    painter.text(pos, align, text, font_id, fg_color);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_osd_mode_cycle() {
        assert_eq!(OsdMode::Auto.cycle(), OsdMode::AlwaysInfo);
        assert_eq!(OsdMode::AlwaysInfo.cycle(), OsdMode::Disabled);
        assert_eq!(OsdMode::Disabled.cycle(), OsdMode::Auto);
    }

    #[test]
    fn test_osd_mode_icons_and_labels() {
        assert_eq!(OsdMode::Auto.icon(), "📺");
        assert_eq!(OsdMode::AlwaysInfo.icon(), "⏱");
        assert_eq!(OsdMode::Disabled.icon(), "🚫");

        assert_eq!(OsdMode::Auto.osd_label(), "OSD: AUTO");
        assert_eq!(OsdMode::AlwaysInfo.osd_label(), "OSD: INFO");
        assert_eq!(OsdMode::Disabled.osd_label(), "OSD: OFF");
    }

    #[test]
    fn test_osd_item_alpha_lifecycle() {
        let item = OsdItem::new("PLAY", OSD_GREEN);
        assert_eq!(item.alpha(), 1.0);
        assert!(!item.is_expired());
    }

    #[test]
    fn test_osd_manager_show_and_toggle() {
        let mut mgr = OsdManager::new();
        assert_eq!(mgr.mode, OsdMode::Auto);

        mgr.show("TEST");
        assert!(mgr.is_active());

        // Toggle to AlwaysInfo
        let mode = mgr.toggle_mode();
        assert_eq!(mode, OsdMode::AlwaysInfo);
        assert!(mgr.is_active());

        // Toggle to Disabled
        let mode = mgr.toggle_mode();
        assert_eq!(mode, OsdMode::Disabled);
        assert!(!mgr.is_active());

        // When disabled, show should be a no-op
        mgr.show("SHOULD_NOT_SHOW");
        assert!(!mgr.is_active());

        // Toggle back to Auto
        let mode = mgr.toggle_mode();
        assert_eq!(mode, OsdMode::Auto);
        assert!(mgr.is_active());
    }
}
