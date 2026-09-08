//! Cantus visual identity: ink and gold. Deep, low-glare surfaces for long sessions, a
//! single warm accent for what matters (playhead, selection, active states). Original by
//! design — inspired by illuminated manuscripts, not by any other DAW (idea.md §8).

use egui::{Color32, CornerRadius, Stroke, Visuals};

pub const INK: Color32 = Color32::from_rgb(0x10, 0x12, 0x18);
pub const PANEL: Color32 = Color32::from_rgb(0x17, 0x1a, 0x22);
pub const PANEL_RAISED: Color32 = Color32::from_rgb(0x1f, 0x23, 0x2d);
pub const GRID: Color32 = Color32::from_rgb(0x23, 0x27, 0x33);
pub const GRID_STRONG: Color32 = Color32::from_rgb(0x2f, 0x34, 0x42);
pub const GRID_BAR: Color32 = Color32::from_rgb(0x3b, 0x41, 0x52);
pub const TEXT: Color32 = Color32::from_rgb(0xe6, 0xe2, 0xd8);
pub const TEXT_DIM: Color32 = Color32::from_rgb(0x8f, 0x93, 0x9e);
pub const GOLD: Color32 = Color32::from_rgb(0xd4, 0xa2, 0x4c);
pub const GOLD_BRIGHT: Color32 = Color32::from_rgb(0xf2, 0xc1, 0x4e);
pub const SELECTION: Color32 = Color32::from_rgb(0x7f, 0xb2, 0xff);
pub const DANGER: Color32 = Color32::from_rgb(0xd9, 0x5d, 0x5d);
pub const OK: Color32 = Color32::from_rgb(0x5c, 0xb8, 0x6e);
pub const BLACK_KEY: Color32 = Color32::from_rgb(0x14, 0x16, 0x1c);
pub const WHITE_KEY: Color32 = Color32::from_rgb(0xd8, 0xd3, 0xc6);
pub const ROW_BLACK: Color32 = Color32::from_rgb(0x13, 0x15, 0x1b);
pub const ROW_WHITE: Color32 = Color32::from_rgb(0x19, 0x1c, 0x24);
pub const METER_LOW: Color32 = Color32::from_rgb(0x5c, 0xb8, 0x6e);
pub const METER_HIGH: Color32 = Color32::from_rgb(0xd4, 0xa2, 0x4c);
pub const METER_CLIP: Color32 = Color32::from_rgb(0xd9, 0x5d, 0x5d);

pub fn visuals() -> Visuals {
    let mut v = Visuals::dark();
    v.panel_fill = PANEL;
    v.window_fill = PANEL_RAISED;
    v.extreme_bg_color = INK;
    v.faint_bg_color = PANEL_RAISED;
    v.override_text_color = Some(TEXT);
    v.selection.bg_fill = GOLD.gamma_multiply(0.35);
    v.selection.stroke = Stroke::new(1.0, GOLD);
    v.hyperlink_color = GOLD;
    v.widgets.noninteractive.bg_fill = PANEL;
    v.widgets.noninteractive.bg_stroke = Stroke::new(1.0, GRID);
    // `bg_fill` paints slider rails, checkboxes and combo boxes; `weak_bg_fill` paints
    // buttons. Rails need contrast against the panel, buttons should sit quietly.
    v.widgets.inactive.bg_fill = GRID_STRONG;
    v.widgets.inactive.weak_bg_fill = PANEL_RAISED;
    v.widgets.inactive.fg_stroke = Stroke::new(1.0, TEXT);
    v.widgets.hovered.bg_fill = GRID_BAR;
    v.widgets.hovered.weak_bg_fill = GRID_STRONG;
    v.widgets.hovered.fg_stroke = Stroke::new(1.5, TEXT);
    v.widgets.active.fg_stroke = Stroke::new(2.0, GOLD_BRIGHT);
    v.slider_trailing_fill = true;
    v.widgets.hovered.bg_stroke = Stroke::new(1.0, GOLD.gamma_multiply(0.6));
    v.widgets.active.bg_fill = GOLD.gamma_multiply(0.5);
    v.widgets.active.weak_bg_fill = GOLD.gamma_multiply(0.5);
    v.widgets.active.bg_stroke = Stroke::new(1.0, GOLD);
    v.widgets.open.bg_fill = GRID_STRONG;
    let r = CornerRadius::same(3);
    v.widgets.noninteractive.corner_radius = r;
    v.widgets.inactive.corner_radius = r;
    v.widgets.hovered.corner_radius = r;
    v.widgets.active.corner_radius = r;
    v.widgets.open.corner_radius = r;
    v.window_corner_radius = CornerRadius::same(4);
    v.menu_corner_radius = CornerRadius::same(4);
    v
}

/// Track colour → egui colour.
pub fn track_color(c: cantus_core::Color) -> Color32 {
    Color32::from_rgb(c.r, c.g, c.b)
}

/// Meter colour for a linear peak value.
pub fn meter_color(peak: f32) -> Color32 {
    if peak >= 1.0 {
        METER_CLIP
    } else if peak > 0.5 {
        METER_HIGH
    } else {
        METER_LOW
    }
}
