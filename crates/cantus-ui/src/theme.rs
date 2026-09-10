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
    /// Keyboard focus ring (`design::paint_focus_ring`). Must contrast with every
    /// surface, since focus can land on a widget over any of them.
    pub focus: Color32,
    /// Drop shadow for surfaces that float over the layout: windows and menus.
    pub shadow: Color32,
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
    /// Added after the first release: themes written before it fall back to the accent.
    #[serde(default)]
    pub focus: String,
    /// Added after the first release; `#rrggbbaa`, so it can be translucent.
    #[serde(default)]
    pub shadow: String,
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

/// `#rrggbb`, or `#rrggbbaa` when the colour is translucent, so a round trip through the
/// theme file never silently drops alpha.
pub fn to_hex(c: Color32) -> String {
    let [r, g, b, a] = c.to_srgba_unmultiplied();
    if a == 255 {
        format!("#{r:02x}{g:02x}{b:02x}")
    } else {
        format!("#{r:02x}{g:02x}{b:02x}{a:02x}")
    }
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
            // An empty string is a theme written before these tokens existed: derive
            // the ring from that theme's own accent rather than the default theme's.
            focus: parse_hex(&self.focus).unwrap_or_else(|| c(&self.accent_bright, d.gold_bright)),
            shadow: c(&self.shadow, d.shadow),
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

    /// Fill in tokens a theme file written by an older version does not have, deriving
    /// them from the colours it *does* have. Returns whether anything was filled, so the
    /// caller can write the upgraded file back — otherwise a token added in a new release
    /// would stay invisible to anyone still editing their old file by hand.
    pub fn fill_missing(&mut self) -> bool {
        let derived = self.palette();
        let mut filled = false;
        for (field, value) in [
            (&mut self.focus, derived.focus),
            (&mut self.shadow, derived.shadow),
        ] {
            if parse_hex(field).is_none() {
                *field = to_hex(value);
                filled = true;
            }
        }
        filled
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
            focus: to_hex(p.focus),
            shadow: to_hex(p.shadow),
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

/// One editable colour in the theme: which group it belongs to, what to call it, and the
/// hex string in the [`Theme`] itself.
///
/// This exists so the settings dialog can offer a colour picker per token without knowing
/// anything about the palette, and so that adding a token to `Theme` and forgetting to
/// expose it is caught by a test rather than by a user who cannot find it.
pub struct Token<'a> {
    pub group_key: &'static str,
    pub label_key: &'static str,
    pub hex: &'a mut String,
}

impl Theme {
    /// Every colour the user can change, grouped the way the dialog shows them.
    pub fn tokens_mut(&mut self) -> Vec<Token<'_>> {
        let t = |group_key, label_key, hex| Token {
            group_key,
            label_key,
            hex,
        };
        const SURFACES: &str = "theme.group.surfaces";
        const LINES: &str = "theme.group.lines";
        const TEXT: &str = "theme.group.text";
        const ACCENT: &str = "theme.group.accent";
        const STATUS: &str = "theme.group.status";
        const ROLL: &str = "theme.group.roll";
        const METERS: &str = "theme.group.meters";
        vec![
            t(SURFACES, "theme.token.ink", &mut self.ink),
            t(SURFACES, "theme.token.panel", &mut self.panel),
            t(SURFACES, "theme.token.panel_raised", &mut self.panel_raised),
            t(SURFACES, "theme.token.shadow", &mut self.shadow),
            t(LINES, "theme.token.grid", &mut self.grid),
            t(LINES, "theme.token.grid_strong", &mut self.grid_strong),
            t(LINES, "theme.token.grid_bar", &mut self.grid_bar),
            t(TEXT, "theme.token.text", &mut self.text),
            t(TEXT, "theme.token.text_dim", &mut self.text_dim),
            t(ACCENT, "theme.token.accent", &mut self.accent),
            t(ACCENT, "theme.token.accent_bright", &mut self.accent_bright),
            t(ACCENT, "theme.token.focus", &mut self.focus),
            t(ACCENT, "theme.token.selection", &mut self.selection),
            t(STATUS, "theme.token.danger", &mut self.danger),
            t(STATUS, "theme.token.ok", &mut self.ok),
            t(ROLL, "theme.token.white_key", &mut self.white_key),
            t(ROLL, "theme.token.black_key", &mut self.black_key),
            t(ROLL, "theme.token.row_white", &mut self.row_white),
            t(ROLL, "theme.token.row_black", &mut self.row_black),
            t(METERS, "theme.token.meter_low", &mut self.meter_low),
            t(METERS, "theme.token.meter_high", &mut self.meter_high),
            t(METERS, "theme.token.meter_clip", &mut self.meter_clip),
        ]
    }

    /// The groups, in the order the dialog shows them.
    pub const GROUPS: [&'static str; 7] = [
        "theme.group.surfaces",
        "theme.group.lines",
        "theme.group.text",
        "theme.group.accent",
        "theme.group.status",
        "theme.group.roll",
        "theme.group.meters",
    ];
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
        // Paler than the accent so the ring stays visible on top of a gold-filled
        // button, which is what egui paints for a focused widget.
        focus: rgb(0xff, 0xe4, 0xa8),
        shadow: Color32::from_rgba_premultiplied(0, 0, 0, 0x8c),
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

/// The default: white surfaces, ink text, the same gold accent.
///
/// Every foreground colour here clears WCAG AA against the surface it is used on — 4.5:1
/// for text, 3:1 for the focus ring and for `accent_bright`, which is only ever used on
/// the large numeric readouts. The canvas is pure white and the panels a hair grey, so a
/// panel still reads as a distinct surface without a border.
pub const fn paper_palette() -> Palette {
    Palette {
        ink: rgb(0xff, 0xff, 0xff),
        panel: rgb(0xf4, 0xf5, 0xf7),
        panel_raised: rgb(0xea, 0xec, 0xf0),
        grid: rgb(0xe5, 0xe8, 0xed),
        grid_strong: rgb(0xd2, 0xd7, 0xdf),
        grid_bar: rgb(0xb4, 0xbb, 0xc6),
        text: rgb(0x14, 0x16, 0x1a),
        text_dim: rgb(0x5b, 0x62, 0x70),
        // Darker than the dark theme's gold: the same colour that glows on ink is
        // unreadable on white, and 4.5:1 against the panel is what decides it.
        gold: rgb(0x95, 0x66, 0x08),
        gold_bright: rgb(0xb8, 0x80, 0x1a),
        focus: rgb(0x7a, 0x4d, 0x00),
        shadow: Color32::from_rgba_premultiplied(0x1a, 0x1d, 0x23, 0x38),
        selection: rgb(0x25, 0x63, 0xeb),
        danger: rgb(0xb3, 0x26, 0x1e),
        ok: rgb(0x1c, 0x7a, 0x45),
        black_key: rgb(0x2a, 0x2d, 0x34),
        white_key: rgb(0xff, 0xff, 0xff),
        row_black: rgb(0xee, 0xf0, 0xf4),
        row_white: rgb(0xfa, 0xfb, 0xfc),
        meter_low: rgb(0x1c, 0x7a, 0x45),
        meter_high: rgb(0x95, 0x66, 0x08),
        meter_clip: rgb(0xb3, 0x26, 0x1e),
        corner_radius: 3,
        dark: false,
    }
}

/// A warmer light variant: parchment surfaces, the same gold accent, ink text.
pub const fn parchment_palette() -> Palette {
    Palette {
        ink: rgb(0xf3, 0xef, 0xe6),
        panel: rgb(0xe9, 0xe4, 0xd8),
        panel_raised: rgb(0xde, 0xd8, 0xc9),
        grid: rgb(0xd6, 0xd0, 0xc2),
        grid_strong: rgb(0xc4, 0xbd, 0xad),
        grid_bar: rgb(0xa9, 0xa1, 0x90),
        text: rgb(0x1e, 0x1b, 0x16),
        // 0x6b665c landed on exactly 4.50:1 against the panel — legible by a rounding
        // error. This clears it with room.
        text_dim: rgb(0x66, 0x61, 0x58),
        // The gold that glows on ink is far too light on parchment: the original
        // #b07c1e managed 2.88:1 against the panel, so every heading was decorative
        // rather than readable. These clear AA (4.5:1, and 3:1 for the bright variant,
        // which is only used on the large readouts).
        gold: rgb(0x87, 0x5c, 0x08),
        gold_bright: rgb(0xad, 0x78, 0x10),
        // On parchment the ring has to go *darker* than the accent to stay visible.
        focus: rgb(0x6b, 0x45, 0x06),
        shadow: Color32::from_rgba_premultiplied(0x1a, 0x15, 0x0e, 0x40),
        selection: rgb(0x2f, 0x6f, 0xd6),
        danger: rgb(0xb0, 0x33, 0x33),
        ok: rgb(0x24, 0x7a, 0x3b),
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

pub fn paper() -> Theme {
    Theme::from_palette("Paper", &paper_palette())
}

pub fn ink_and_gold() -> Theme {
    Theme::from_palette("Ink & Gold", &ink_and_gold_palette())
}

pub fn parchment() -> Theme {
    Theme::from_palette("Parchment", &parchment_palette())
}

/// A named theme preset: i18n key + constructor.
pub type Preset = (&'static str, fn() -> Theme);

pub const PRESETS: [Preset; 3] = [
    ("theme.paper", paper),
    ("theme.parchment", parchment),
    ("theme.ink_gold", ink_and_gold),
];

/// What a fresh install starts with, and the fallback whenever a theme file cannot be
/// read. Light: a DAW is looked at in daylight as often as in a dark studio, and a white
/// ground is the one that does not assume which.
pub fn default_theme() -> Theme {
    paper()
}

fn current() -> &'static RwLock<Palette> {
    static CURRENT: OnceLock<RwLock<Palette>> = OnceLock::new();
    CURRENT.get_or_init(|| RwLock::new(paper_palette()))
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
    // buttons and the fader cap. The rail is a quiet groove — the filled part of it is
    // drawn in the accent by `slider_trailing_fill` — and the cap reads as a solid object
    // sitting on top of it, with an edge so it does not dissolve into the fill.
    v.widgets.inactive.bg_fill = pal.grid_strong;
    v.widgets.inactive.weak_bg_fill = pal.panel_raised;
    v.widgets.inactive.fg_stroke = Stroke::new(1.0, pal.text);
    // egui paints a slider's groove and its cap with the same `inactive.bg_fill`, so the
    // only thing that makes the cap read as an object sitting on the groove is its edge.
    // Give it a definite one. (A cap with its own fill needs a widget of our own; see
    // STATUS.)
    v.widgets.inactive.bg_stroke = Stroke::new(1.0, pal.grid_bar);
    v.widgets.hovered.weak_bg_fill = pal.grid_strong;
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
    // Only surfaces that float over the layout cast a shadow (design::Elevation).
    v.window_shadow = crate::design::overlay_shadow(pal);
    v.popup_shadow = crate::design::overlay_shadow(pal);
    v.window_stroke = Stroke::new(1.0, pal.grid_strong);
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

/// Relative luminance (WCAG), used to decide which of two tokens is the darker.
fn luminance(c: Color32) -> f32 {
    let ch = |v: u8| {
        let v = v as f32 / 255.0;
        if v <= 0.04045 {
            v / 12.92
        } else {
            ((v + 0.055) / 1.055).powf(2.4)
        }
    };
    0.2126 * ch(c.r()) + 0.7152 * ch(c.g()) + 0.0722 * ch(c.b())
}

/// The colour of a slot cut into a surface — a fader groove, and any recess added later.
///
/// A recess is *darker* than the surface around it in every theme, dark or light: that is
/// how a shadowed slot looks, and it is why the same token cannot serve both. No new
/// palette entry is needed, because the palette already contains two candidates that are
/// darker than `panel_raised` in one theme family each: the canvas (`ink`) on a dark
/// theme, the strong grid line on a light one. Take whichever is actually darker, and if
/// a hand-edited theme leaves neither darker than the surface, darken the surface itself
/// rather than paint an invisible groove.
pub fn recess(pal: &Palette) -> Color32 {
    let surface = luminance(pal.panel_raised);
    let candidate = if luminance(pal.ink) <= luminance(pal.grid_strong) {
        pal.ink
    } else {
        pal.grid_strong
    };
    if luminance(candidate) < surface {
        candidate
    } else {
        pal.panel_raised.gamma_multiply(0.72)
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

    /// Relative luminance, WCAG 2.1.
    fn luminance(c: Color32) -> f64 {
        let ch = |v: u8| {
            let v = v as f64 / 255.0;
            if v <= 0.03928 {
                v / 12.92
            } else {
                ((v + 0.055) / 1.055).powf(2.4)
            }
        };
        0.2126 * ch(c.r()) + 0.7152 * ch(c.g()) + 0.0722 * ch(c.b())
    }

    /// WCAG contrast ratio between two opaque colours.
    fn contrast(a: Color32, b: Color32) -> f64 {
        let (la, lb) = (luminance(a), luminance(b));
        let (hi, lo) = if la > lb { (la, lb) } else { (lb, la) };
        (hi + 0.05) / (lo + 0.05)
    }

    #[test]
    fn every_shipped_theme_is_legible() {
        // A theme that ships with Cantus has to be readable, and "it looked fine on my
        // monitor" is not a standard. WCAG AA: 4.5:1 for text, 3:1 for the focus ring and
        // for `gold_bright`, which is only used on the large numeric readouts.
        for (key, make) in PRESETS {
            let p = make().palette();
            let checks: [(&str, Color32, Color32, f64); 13] = [
                ("text on canvas", p.text, p.ink, 4.5),
                ("text on panel", p.text, p.panel, 4.5),
                ("text on card", p.text, p.panel_raised, 4.5),
                ("dim text on panel", p.text_dim, p.panel, 4.5),
                ("accent on panel", p.gold, p.panel, 4.5),
                ("accent on canvas", p.gold, p.ink, 4.5),
                ("bright accent on panel", p.gold_bright, p.panel, 3.0),
                ("focus ring on panel", p.focus, p.panel, 3.0),
                ("focus ring on canvas", p.focus, p.ink, 3.0),
                ("danger on panel", p.danger, p.panel, 4.5),
                ("black key on white key", p.black_key, p.white_key, 4.5),
                // A selected note is filled with this. If it does not stand out from the
                // canvas the user cannot tell what they have selected — which is exactly
                // what happened when selection was painted white on a white theme.
                ("selection on canvas", p.selection, p.ink, 3.0),
                ("selection on a panel", p.selection, p.panel, 3.0),
            ];
            for (what, fg, bg, need) in checks {
                let got = contrast(fg, bg);
                assert!(
                    got >= need,
                    "{key}: {what} is {got:.2}:1, below the {need}:1 it needs \
                     ({} on {})",
                    to_hex(fg),
                    to_hex(bg)
                );
            }
            // A panel has to be distinguishable from the canvas behind it, or the
            // elevation scale says nothing.
            assert!(
                contrast(p.panel, p.ink) > 1.02,
                "{key}: the panel is indistinguishable from the canvas"
            );
        }
    }

    #[test]
    fn every_colour_in_a_theme_is_editable_in_the_dialog() {
        // Adding a field to `Theme` and forgetting `tokens_mut` would leave a colour the
        // user can only reach by editing JSON. Count the hex fields the serialized form
        // has and require the editor to expose all of them.
        let mut theme = default_theme();
        let exposed = theme.tokens_mut().len();
        let value = serde_json::to_value(default_theme()).unwrap();
        let hex_fields = value
            .as_object()
            .unwrap()
            .iter()
            .filter(|(k, v)| {
                v.as_str().map(|s| s.starts_with('#')).unwrap_or(false) && *k != "name"
            })
            .count();
        assert_eq!(
            exposed, hex_fields,
            "the theme has {hex_fields} colours but the dialog exposes {exposed}"
        );
        // And every token names a group the dialog actually renders.
        for token in default_theme().tokens_mut() {
            assert!(
                Theme::GROUPS.contains(&token.group_key),
                "{} is in group {:?}, which the dialog does not show",
                token.label_key,
                token.group_key
            );
        }
    }

    #[test]
    fn the_default_theme_is_light() {
        let d = default_theme().palette();
        assert!(!d.dark, "the shipped default is a light theme");
        assert!(
            luminance(d.ink) > 0.9,
            "the canvas should be white, got {}",
            to_hex(d.ink)
        );
    }

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
    fn a_theme_file_missing_new_tokens_is_upgraded_in_place() {
        // What a file written before `focus` and `shadow` existed deserializes to.
        let mut old = ink_and_gold();
        old.focus = String::new();
        old.shadow = String::new();
        old.accent_bright = "#00ff00".into();

        assert!(
            old.fill_missing(),
            "missing tokens should be reported as filled"
        );
        // Derived from *this* theme's accent, not from the default theme's.
        assert_eq!(old.focus, "#00ff00");
        assert_eq!(parse_hex(&old.shadow), Some(ink_and_gold_palette().shadow));
        // Alpha survives the round trip through the file.
        assert!(
            old.shadow.len() == 9,
            "shadow should serialize alpha: {}",
            old.shadow
        );
        assert!(
            !old.fill_missing(),
            "a complete theme should not be rewritten"
        );
    }

    #[test]
    fn bad_colours_fall_back_to_defaults() {
        let mut t = ink_and_gold();
        t.accent = "not a colour".into();
        assert_eq!(t.palette().gold, ink_and_gold_palette().gold);
    }
}
