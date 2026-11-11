//! r39:el-arrange — the arrangement's inks in Slate (`docs/round39/design/des-arrange/`,
//! ADR 0111): the design's names (gutter, base, surface, ink 1–4, well, edge, accent, rec,
//! solo, auto…) read from the installed tokens, so every theme family and its light draw
//! the sheets' roles in its own colours; and the text sizes the sheets measure (Regular 10,
//! Medium 11, SemiBold 9…) set through the kit's roles at a scale.

use crate::kit::color::{ensure_contrast_all, mix, with_alpha};

/// The arrangement's bar lines against its lanes, and its beat lines (round 39, *themes*).
pub const BAR_LINE: f32 = 3.0;
pub const BEAT_LINE: f32 = 1.35;

/// The bar line, the beat line and solo's ink of `t`, each held to its floor — worked out
/// once a theme (the last answer kept, keyed by the colours it is made from): `Ink::of` runs
/// many times a frame.
fn held(t: &crate::kit::Tokens) -> (Color32, Color32, Color32) {
    type Key = [Color32; 8];
    type Held = (Color32, Color32, Color32);
    thread_local! {
        static LAST: std::cell::Cell<Option<(Key, Held)>> = const { std::cell::Cell::new(None) };
    }
    let key = [
        t.grid_strong,
        t.grid_bar,
        t.grid,
        t.row_black,
        t.row_white,
        t.meter_high,
        t.well,
        t.toolbar,
    ];
    if let Some((k, v)) = LAST.get() {
        if k == key {
            return v;
        }
    }
    let lanes = [t.row_black, t.row_white];
    let v = (
        ensure_contrast_all(mix(t.grid_strong, t.grid_bar, 0.4), &lanes, BAR_LINE),
        ensure_contrast_all(t.grid, &lanes, BEAT_LINE),
        // Solo's amber is the meters'; where a light theme's meters glow on a dark groove it
        // is too pale for a key on its well: moved to 3:1 there.
        ensure_contrast_all(t.meter_high, &[t.well, t.toolbar, t.row_black], 3.0),
    );
    LAST.set(Some((key, v)));
    v
}
use crate::kit::text::{self, Elide, Painted, Policy, Role, Text};
use crate::kit::{Brush, Style};
use egui::{Align2, Color32, Rect};

/// The sheets' roles, from the tokens.
#[derive(Copy, Clone, Debug)]
pub struct Ink {
    pub gutter: Color32,
    pub base: Color32,
    pub lane2: Color32,
    pub surface: Color32,
    pub raised: Color32,
    pub well: Color32,
    pub well_hi: Color32,
    /// The 1-px dark edge of a well (not the kit's `edge`, which is a line ink).
    pub edge: Color32,
    pub grid_bar: Color32,
    pub grid_beat: Color32,
    pub ink1: Color32,
    pub ink2: Color32,
    pub ink3: Color32,
    pub ink4: Color32,
    pub accent: Color32,
    pub rec: Color32,
    pub solo: Color32,
    pub play: Color32,
    pub auto: Color32,
    pub meter_lo: Color32,
    pub meter_mid: Color32,
    /// The selection's tint over a selected header and lane (clear in Slate; r39:lapis2).
    pub sel_tint: Color32,
    pub dark: bool,
}

impl Ink {
    pub fn of(s: &Style) -> Ink {
        let t = &s.t;
        let held = held(t);
        Ink {
            gutter: t.ink,
            base: t.row_black,
            lane2: t.row_white,
            surface: t.toolbar,
            raised: t.menu,
            well: t.well,
            well_hi: mix(t.well, t.toolbar, 0.45),
            edge: t.meter_well,
            // Round 39, *themes* (the owner: every theme easy to read): a bar line at 3:1
            // on both lanes (WCAG 1.4.11, the grid is how a song is read), a beat line a
            // visible step fainter — each theme's own line ink, moved only where it falls
            // short.
            grid_bar: held.0,
            grid_beat: held.1,
            ink1: t.text,
            ink2: mix(t.text_dim, t.text, 0.23),
            ink3: mix(t.placeholder, t.text_dim, 0.5),
            ink4: mix(t.placeholder, t.row_black, 0.5),
            accent: t.highlight,
            rec: t.danger,
            // Solo's amber is the meters'; where a light theme's meters glow on a dark
            // groove it is too pale for a key on its well: moved to 3:1 there.
            solo: held.2,
            play: t.led,
            auto: t.hue_mod,
            meter_lo: t.meter_low,
            meter_mid: mix(t.meter_low, t.meter_high, 0.5),
            sel_tint: t.selection_tint,
            dark: t.dark,
        }
    }

    /// `b` over `a` at `k` (the sheets' `a→b k%`).
    pub fn to(a: Color32, b: Color32, k: f32) -> Color32 {
        mix(a, b, k)
    }

    /// The user's colour as the theme spends it: on Paper it darkens toward the ink first
    /// to read on the light lanes (the sheets' `hue_for`).
    pub fn hue(&self, c: Color32) -> Color32 {
        if self.dark {
            c
        } else {
            mix(c, self.ink1, 0.18)
        }
    }

    /// The user's colour at strength `k` over the lanes.
    pub fn tint(&self, c: Color32, k: f32) -> Color32 {
        mix(self.base, self.hue(c), k)
    }

    /// A colour at alpha.
    pub fn a(c: Color32, alpha: f32) -> Color32 {
        with_alpha(c, alpha)
    }

    /// A track's stripe on its header (selected: its colour whole; at rest: the stripe's
    /// strength over the lanes); on a light theme held at 3:1 on the header's surface and the
    /// lanes (round 39, *themes*), its hue kept. Night's stripes are as they were.
    pub fn stripe(&self, c: Color32, selected: bool) -> Color32 {
        let own = if selected {
            self.hue(c)
        } else {
            self.tint(c, self.stripe_rest())
        };
        if self.dark {
            own
        } else {
            ensure_contrast_all(own, &[self.surface, self.base, self.lane2], 3.0)
        }
    }

    /// The stripe at rest (72 % Night, 70 % Paper).
    pub fn stripe_rest(&self) -> f32 {
        if self.dark {
            0.72
        } else {
            0.70
        }
    }

    /// A selected header's ground: `c` toward the track's colour by [`Ink::head_sel`],
    /// then through the theme's selection tint (none in Slate; r39:lapis2).
    pub fn head_ground(&self, c: Color32, hue: Color32) -> Color32 {
        crate::kit::color::selected_fill(
            Ink::to(c, self.hue(hue), self.head_sel()),
            self.sel_tint,
            1.0,
        )
    }

    /// A selected header's tint (13 % Night, 16 % Paper).
    pub fn head_sel(&self) -> f32 {
        if self.dark {
            0.13
        } else {
            0.16
        }
    }
}

/// The weight a sheet asks for.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum W {
    Regular,
    Medium,
    SemiBold,
}

/// The role that sets `w` in the installed face.
fn role(w: W) -> Role {
    match w {
        W::Regular => Role::Body,
        W::Medium => Role::Control,
        W::SemiBold => Role::Section,
    }
}

/// A text of `size` points (at 100 %) in weight `w`: the role's face at a scale.
pub fn text_k(s: &Style, w: W, size: f32) -> f32 {
    s.tk() * size / role(w).size().max(1.0)
}

/// Paint `str` in `rect` at `size` and weight `w`, aligned `align`, elided at its end.
#[allow(clippy::too_many_arguments)]
pub fn say(
    b: &mut Brush,
    rect: Rect,
    str: &str,
    w: W,
    size: f32,
    ink: Color32,
    align: Align2,
    s: &Style,
) -> Option<Painted> {
    let tx = Text::new(str, role(w), ink)
        .caps(false)
        .align(align)
        .policy(Policy::Elide(Elide::End));
    text::paint(b, rect, &tx, text_k(s, w, size))
}

/// How wide `str` is at `size` and weight `w`.
pub fn width(ctx: &egui::Context, str: &str, w: W, size: f32, s: &Style) -> f32 {
    let tx = Text::new(str, role(w), egui::Color32::PLACEHOLDER).caps(false);
    text::measure(ctx, &tx, text_k(s, w, size)).x
}
