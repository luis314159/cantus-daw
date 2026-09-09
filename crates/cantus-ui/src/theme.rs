//! Cantus visual identity as **data**. The default theme, "Ink & Gold", is deep low-glare
//! surfaces with a single warm accent — original by design, inspired by illuminated
//! manuscripts, not by any other DAW (idea.md §8). Every colour lives in a JSON file the
//! user can edit; the app reloads it while running.

use egui::{Color32, CornerRadius, Stroke, Visuals};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::{OnceLock, RwLock};
use std::time::{Instant, SystemTime};

/// The palette in use, copied out for painting. Cheap to copy (a handful of colours).
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Palette {
    pub ink: Color32,
    pub panel: Color32,
    pub panel_raised: Color32,
    pub grid: Color32,
    pub grid_strong: Color32,
    pub grid_bar: Color32,
    pub text: Color32,
    pub text_dim: Color32,
    pub gold: Color32,
    pub gold_bright: Color32,
    pub selection: Color32,
    pub danger: Color32,
    pub ok: Color32,
    pub black_key: Color32,
    pub white_key: Color32,
    pub row_black: Color32,
    pub row_white: Color32,
    pub meter_low: Color32,
    pub meter_high: Color32,
    pub meter_clip: Color32,
    pub corner_radius: u8,
    pub dark: bool,
}

/// Serialized form: hex colours so the file is editable by hand.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Theme {
    pub name: String,
    #[serde(default = "default_true")]
    pub dark: bool,
    #[serde(default = "default_radius")]
    pub corner_radius: u8,
    pub ink: String,
    pub panel: String,
    pub panel_raised: String,
    pub grid: String,
    pub grid_strong: String,
    pub grid_bar: String,
    pub text: String,
    pub text_dim: String,
    pub accent: String,
    pub accent_bright: String,
    pub selection: String,
    pub danger: String,
    pub ok: String,
    pub black_key: String,
    pub white_key: String,
    pub row_black: String,
    pub row_white: String,
    pub meter_low: String,
    pub meter_high: String,
    pub meter_clip: String,
}

fn default_true() -> bool {
    true
}
fn default_radius() -> u8 {
    3
}

pub fn parse_hex(s: &str) -> Option<Color32> {
    let s = s.trim().trim_start_matches('#');
    let v = u32::from_str_radix(s, 16).ok()?;
    match s.len() {
        6 => Some(Color32::from_rgb((v >> 16) as u8, (v >> 8) as u8, v as u8)),
        8 => Some(Color32::from_rgba_unmultiplied(
            (v >> 24) as u8,
            (v >> 16) as u8,
            (v >> 8) as u8,
            v as u8,
        )),
        _ => None,
    }
}

pub fn to_hex(c: Color32) -> String {
    format!("#{:02x}{:02x}{:02x}", c.r(), c.g(), c.b())
}

impl Theme {
    /// Convert to a palette. Unparseable colours fall back to the default theme's.
    pub fn palette(&self) -> Palette {
        let d = ink_and_gold_palette();
        let c = |s: &str, fallback: Color32| parse_hex(s).unwrap_or(fallback);
        Palette {
            ink: c(&self.ink, d.ink),
            panel: c(&self.panel, d.panel),
            panel_raised: c(&self.panel_raised, d.panel_raised),
            grid: c(&self.grid, d.grid),
            grid_strong: c(&self.grid_strong, d.grid_strong),
            grid_bar: c(&self.grid_bar, d.grid_bar),
            text: c(&self.text, d.text),
            text_dim: c(&self.text_dim, d.text_dim),
            gold: c(&self.accent, d.gold),
            gold_bright: c(&self.accent_bright, d.gold_bright),
            selection: c(&self.selection, d.selection),
            danger: c(&self.danger, d.danger),
            ok: c(&self.ok, d.ok),
            black_key: c(&self.black_key, d.black_key),
            white_key: c(&self.white_key, d.white_key),
            row_black: c(&self.row_black, d.row_black),
            row_white: c(&self.row_white, d.row_white),
            meter_low: c(&self.meter_low, d.meter_low),
            meter_high: c(&self.meter_high, d.meter_high),
            meter_clip: c(&self.meter_clip, d.meter_clip),
            corner_radius: self.corner_radius,
            dark: self.dark,
        }
    }

    pub fn from_palette(name: &str, p: &Palette) -> Theme {
        Theme {
            name: name.to_string(),
            dark: p.dark,
            corner_radius: p.corner_radius,
            ink: to_hex(p.ink),
            panel: to_hex(p.panel),
            panel_raised: to_hex(p.panel_raised),
            grid: to_hex(p.grid),
            grid_strong: to_hex(p.grid_strong),
            grid_bar: to_hex(p.grid_bar),
            text: to_hex(p.text),
            text_dim: to_hex(p.text_dim),
            accent: to_hex(p.gold),
            accent_bright: to_hex(p.gold_bright),
            selection: to_hex(p.selection),
            danger: to_hex(p.danger),
            ok: to_hex(p.ok),
            black_key: to_hex(p.black_key),
            white_key: to_hex(p.white_key),
            row_black: to_hex(p.row_black),
            row_white: to_hex(p.row_white),
            meter_low: to_hex(p.meter_low),
            meter_high: to_hex(p.meter_high),
            meter_clip: to_hex(p.meter_clip),
        }
    }
}

const fn rgb(r: u8, g: u8, b: u8) -> Color32 {
    Color32::from_rgb(r, g, b)
}

/// The default theme.
pub const fn ink_and_gold_palette() -> Palette {
    Palette {
        ink: rgb(0x10, 0x12, 0x18),
        panel: rgb(0x17, 0x1a, 0x22),
        panel_raised: rgb(0x1f, 0x23, 0x2d),
        grid: rgb(0x23, 0x27, 0x33),
        grid_strong: rgb(0x2f, 0x34, 0x42),
        grid_bar: rgb(0x3b, 0x41, 0x52),
        text: rgb(0xe6, 0xe2, 0xd8),
        text_dim: rgb(0x8f, 0x93, 0x9e),
        gold: rgb(0xd4, 0xa2, 0x4c),
        gold_bright: rgb(0xf2, 0xc1, 0x4e),
        selection: rgb(0x7f, 0xb2, 0xff),
        danger: rgb(0xd9, 0x5d, 0x5d),
        ok: rgb(0x5c, 0xb8, 0x6e),
        black_key: rgb(0x14, 0x16, 0x1c),
        white_key: rgb(0xd8, 0xd3, 0xc6),
        row_black: rgb(0x13, 0x15, 0x1b),
        row_white: rgb(0x19, 0x1c, 0x24),
        meter_low: rgb(0x5c, 0xb8, 0x6e),
        meter_high: rgb(0xd4, 0xa2, 0x4c),
        meter_clip: rgb(0xd9, 0x5d, 0x5d),
        corner_radius: 3,
        dark: true,
    }
}

/// A light variant: parchment surfaces, the same gold accent, ink text.
pub const fn parchment_palette() -> Palette {
    Palette {
        ink: rgb(0xf3, 0xef, 0xe6),
        panel: rgb(0xe9, 0xe4, 0xd8),
        panel_raised: rgb(0xde, 0xd8, 0xc9),
        grid: rgb(0xd6, 0xd0, 0xc2),
        grid_strong: rgb(0xc4, 0xbd, 0xad),
        grid_bar: rgb(0xa9, 0xa1, 0x90),
        text: rgb(0x1e, 0x1b, 0x16),
        text_dim: rgb(0x6b, 0x66, 0x5c),
        gold: rgb(0xb0, 0x7c, 0x1e),
        gold_bright: rgb(0xd0, 0x92, 0x1c),
        selection: rgb(0x2f, 0x6f, 0xd6),
        danger: rgb(0xc0, 0x3b, 0x3b),
        ok: rgb(0x2f, 0x8f, 0x46),
        black_key: rgb(0x2a, 0x27, 0x22),
        white_key: rgb(0xfb, 0xf8, 0xf0),
        row_black: rgb(0xe6, 0xe0, 0xd2),
        row_white: rgb(0xf0, 0xeb, 0xe0),
        meter_low: rgb(0x2f, 0x8f, 0x46),
        meter_high: rgb(0xb0, 0x7c, 0x1e),
        meter_clip: rgb(0xc0, 0x3b, 0x3b),
        corner_radius: 3,
        dark: false,
    }
}

pub fn ink_and_gold() -> Theme {
    Theme::from_palette("Ink & Gold", &ink_and_gold_palette())
}

pub fn parchment() -> Theme {
    Theme::from_palette("Parchment", &parchment_palette())
}

/// A named theme preset: i18n key + constructor.
pub type Preset = (&'static str, fn() -> Theme);

pub const PRESETS: [Preset; 2] = [
    ("theme.ink_gold", ink_and_gold),
    ("theme.parchment", parchment),
];

fn current() -> &'static RwLock<Palette> {
    static CURRENT: OnceLock<RwLock<Palette>> = OnceLock::new();
    CURRENT.get_or_init(|| RwLock::new(ink_and_gold_palette()))
}

/// The active palette.
pub fn p() -> Palette {
    *current().read().unwrap()
}

/// Install a palette for every view and push matching widget visuals into egui.
pub fn apply(theme: &Theme, ctx: &egui::Context) {
    let pal = theme.palette();
    *current().write().unwrap() = pal;
    ctx.set_theme(if pal.dark {
        egui::Theme::Dark
    } else {
        egui::Theme::Light
    });
    ctx.set_visuals_of(egui::Theme::Dark, visuals(&pal));
    ctx.set_visuals_of(egui::Theme::Light, visuals(&pal));
}

pub fn visuals(pal: &Palette) -> Visuals {
    let mut v = if pal.dark {
        Visuals::dark()
    } else {
        Visuals::light()
    };
    v.panel_fill = pal.panel;
    v.window_fill = pal.panel_raised;
    v.extreme_bg_color = pal.ink;
    v.faint_bg_color = pal.panel_raised;
    v.override_text_color = Some(pal.text);
    v.selection.bg_fill = pal.gold.gamma_multiply(0.35);
    v.selection.stroke = Stroke::new(1.0, pal.gold);
    v.hyperlink_color = pal.gold;
    v.widgets.noninteractive.bg_fill = pal.panel;
    v.widgets.noninteractive.bg_stroke = Stroke::new(1.0, pal.grid);
    // `bg_fill` paints slider rails, checkboxes and combo boxes; `weak_bg_fill` paints
    // buttons. Rails need contrast against the panel, buttons should sit quietly.
    v.widgets.inactive.bg_fill = pal.grid_strong;
    v.widgets.inactive.weak_bg_fill = pal.panel_raised;
    v.widgets.inactive.fg_stroke = Stroke::new(1.0, pal.text);
    v.widgets.hovered.bg_fill = pal.grid_bar;
    v.widgets.hovered.weak_bg_fill = pal.grid_strong;
    v.widgets.hovered.fg_stroke = Stroke::new(1.5, pal.text);
    v.widgets.hovered.bg_stroke = Stroke::new(1.0, pal.gold.gamma_multiply(0.6));
    v.widgets.active.fg_stroke = Stroke::new(2.0, pal.gold_bright);
    v.widgets.active.bg_fill = pal.gold.gamma_multiply(0.5);
    v.widgets.active.weak_bg_fill = pal.gold.gamma_multiply(0.5);
    v.widgets.active.bg_stroke = Stroke::new(1.0, pal.gold);
    v.widgets.open.bg_fill = pal.grid_strong;
    v.slider_trailing_fill = true;
    let r = CornerRadius::same(pal.corner_radius);
    v.widgets.noninteractive.corner_radius = r;
    v.widgets.inactive.corner_radius = r;
    v.widgets.hovered.corner_radius = r;
    v.widgets.active.corner_radius = r;
    v.widgets.open.corner_radius = r;
    v.window_corner_radius = CornerRadius::same(pal.corner_radius + 1);
    v.menu_corner_radius = CornerRadius::same(pal.corner_radius + 1);
    v
}

/// Track colour → egui colour.
pub fn track_color(c: cantus_core::Color) -> Color32 {
    Color32::from_rgb(c.r, c.g, c.b)
}

/// Meter colour for a linear peak value.
pub fn meter_color(peak: f32) -> Color32 {
    let pal = p();
    if peak >= 1.0 {
        pal.meter_clip
    } else if peak > 0.5 {
        pal.meter_high
    } else {
        pal.meter_low
    }
}

// ---- theme file ---------------------------------------------------------------------

pub fn load_file(path: &Path) -> Result<Theme, String> {
    let text = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
    serde_json::from_str(&text).map_err(|e| e.to_string())
}

pub fn save_file(theme: &Theme, path: &Path) -> Result<(), String> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    let text = serde_json::to_string_pretty(theme).map_err(|e| e.to_string())?;
    std::fs::write(path, text).map_err(|e| e.to_string())
}

/// Polls the theme file's modification time and reports changes (hot reload).
#[derive(Debug)]
pub struct ThemeWatcher {
    pub path: PathBuf,
    mtime: Option<SystemTime>,
    last_poll: Instant,
}

impl ThemeWatcher {
    pub fn new(path: PathBuf) -> Self {
        let mtime = std::fs::metadata(&path).and_then(|m| m.modified()).ok();
        ThemeWatcher {
            path,
            mtime,
            last_poll: Instant::now(),
        }
    }

    /// Returns the freshly parsed theme when the file changed since the last poll.
    pub fn poll(&mut self) -> Option<Result<Theme, String>> {
        if self.last_poll.elapsed().as_secs_f32() < 1.0 {
            return None;
        }
        self.last_poll = Instant::now();
        let mtime = std::fs::metadata(&self.path)
            .and_then(|m| m.modified())
            .ok();
        if mtime.is_some() && mtime != self.mtime {
            self.mtime = mtime;
            Some(load_file(&self.path))
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hex_roundtrip_and_presets_parse() {
        assert_eq!(
            parse_hex("#d4a24c"),
            Some(Color32::from_rgb(0xd4, 0xa2, 0x4c))
        );
        assert_eq!(parse_hex("zz"), None);
        for (_, make) in PRESETS {
            let theme = make();
            let json = serde_json::to_string(&theme).unwrap();
            let back: Theme = serde_json::from_str(&json).unwrap();
            assert_eq!(back, theme);
            assert_eq!(back.palette(), theme.palette());
        }
        assert_eq!(ink_and_gold().palette(), ink_and_gold_palette());
    }

    #[test]
    fn bad_colours_fall_back_to_defaults() {
        let mut t = ink_and_gold();
        t.accent = "not a colour".into();
        assert_eq!(t.palette().gold, ink_and_gold_palette().gold);
    }
}
