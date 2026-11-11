//! How the desk looks in round 39 (des-mixer's sheets, approved by the owner; the measured
//! spec is `docs/round39/design/des-mixer/README.md`): the palette the mixer derives from
//! the theme's tokens, the fader's law that the meter shares, each width mode's measures,
//! and the strip's parts as drawings — the body and its stripe, the groove, ticks and cap,
//! the two meter bars, the keys, the insert panel, the EQ thumbnail, the socket and the
//! frozen hatch. Pure painting: the strip (`mixer_strip`) decides the boxes and senses.
//!
//! **No new colour** (README, Tokens): every colour here is a theme token or a mix of two.

use super::layout::WidthMode;
use crate::kit::color::{mix, with_alpha};
use crate::kit::glyph_mixer::{self as mg, Ink, MGlyph};
use crate::kit::text::{self, Elide, Policy, Role, Text};
use crate::kit::{Brush, Fill, Style};
use egui::{pos2, vec2, Align2, Color32, Pos2, Rect};

/// The mixer's colours: Slate's names (`base surface raised well edge lip line ink1–4
/// accent play rec solo clip auto meter_lo/mid/hi`), taken from the theme's tokens, and the
/// few mixes des-mixer names.
#[derive(Copy, Clone, Debug)]
pub(crate) struct Pal {
    pub dark: bool,
    pub base: Color32,
    pub surface: Color32,
    pub raised: Color32,
    pub well: Color32,
    pub well_hi: Color32,
    pub edge: Color32,
    pub lip: Color32,
    pub line: Color32,
    pub ink1: Color32,
    pub ink2: Color32,
    pub ink3: Color32,
    pub ink4: Color32,
    pub accent: Color32,
    pub play: Color32,
    pub rec: Color32,
    pub solo: Color32,
    pub clip: Color32,
    pub auto: Color32,
    pub meter_lo: Color32,
    pub meter_mid: Color32,
    pub meter_hi: Color32,
    /// `strip.panel = mix(surface, well, .45)`: the insert list's and the EQ's ground.
    pub panel: Color32,
    /// `strip.empty = mix(surface, base, .55)`.
    pub empty: Color32,
    /// `fader.groove = mix(well, edge, .55)`.
    pub groove: Color32,
    /// `meter.groove = mix(well, ink4, .22)`.
    pub meter_groove: Color32,
    /// `meter.clip_idle = mix(well, ink4, .30)`.
    pub clip_idle: Color32,
    /// `bar.track = mix(well, ink4, .35)`.
    pub bar_track: Color32,
    /// `mute.on = ink2 (Night) / ink3 (Paper)`.
    pub mute_on: Color32,
    /// The strip's light from above: its top colour (surface + 3.5 % ink 1).
    pub strip_top: Color32,
    /// The fader cap's three stops, top to bottom.
    pub fcap: [Color32; 3],
    /// A key's letter on a lit key.
    pub on_ink: Color32,
    /// The selection's tint over a selected strip's body (clear in Slate; r39:lapis2).
    pub sel_tint: Color32,
}

impl Pal {
    pub fn of(s: &Style) -> Pal {
        let t = &s.t;
        let dark = t.dark;
        let base = t.panel;
        let surface = t.strip;
        let well = t.well;
        let ink1 = t.text;
        let ink3 = t.placeholder;
        let ink4 = mg::ink4(s);
        let edge = crate::kit::color::opaque(t.inner_shadow);
        let raised = if dark { t.deck_title } else { t.header_cell };
        let meter_lo = t.meter_low;
        let meter_hi = t.meter_high;
        Pal {
            dark,
            base,
            surface,
            raised,
            well,
            well_hi: mix(well, ink1, 0.03),
            edge,
            lip: crate::kit::color::opaque(t.lip),
            line: t.grid_bar,
            ink1,
            ink2: t.text_dim,
            ink3,
            ink4,
            accent: t.highlight,
            play: t.led,
            rec: t.danger,
            solo: t.meter_high,
            clip: t.meter_clip,
            auto: t.hue_mod,
            meter_lo,
            meter_mid: mix(meter_lo, meter_hi, 0.5),
            meter_hi,
            panel: mix(surface, well, 0.45),
            empty: mix(surface, base, 0.55),
            groove: mix(well, edge, 0.55),
            meter_groove: mix(well, ink4, 0.22),
            clip_idle: mix(well, ink4, 0.30),
            bar_track: mix(well, ink4, 0.35),
            mute_on: if dark { t.text_dim } else { ink3 },
            strip_top: if dark {
                mix(surface, ink1, 0.035)
            } else {
                mix(surface, t.light, 0.3)
            },
            fcap: if dark {
                [
                    mix(raised, ink1, 0.15),
                    mix(raised, ink1, 0.07),
                    mix(raised, ink1, 0.03),
                ]
            } else {
                [
                    t.light,
                    mix(t.light, surface, 0.3),
                    mix(surface, edge, 0.25),
                ]
            },
            on_ink: if dark { t.ink } else { t.light },
            sel_tint: t.selection_tint,
        }
    }

    /// A strip's colour as its stripe wears it: the user's colour at `k` over the base (on
    /// Paper pulled toward ink first, so it reads on light ground).
    pub fn hue(&self, c: Color32, k: f32) -> Color32 {
        let c = if self.dark {
            c
        } else {
            mix(c, self.ink1, 0.18)
        };
        mix(self.base, c, k)
    }

    /// The stripe at a strip's head in its colour `c` at `k` ([`Self::hue`]), read on what it
    /// sits on: a strip's top at rest and a selected strip's body (round 39, `fix2`). On a
    /// light theme taken down in lightness (hue and saturation kept) until it reads at 3:1 on
    /// both (WCAG 1.4.11), as `kit::color::track_mark` holds the track's other marks; a dark
    /// theme's stripe is [`Self::hue`] as it was. A muted or silenced strip's dimmed stripe
    /// (`k` under 0.5) says that state and is left as it is.
    pub fn stripe(&self, c: Color32, k: f32) -> Color32 {
        let on = if k >= 1.0 { c } else { self.hue(c, k) };
        if self.dark || k < 0.5 {
            return on;
        }
        let grounds = [self.strip_top, self.surface, self.selected(c)];
        type Seen = (Color32, [Color32; 3], Color32);
        thread_local! {
            static SEEN: std::cell::RefCell<Vec<Seen>> = const { std::cell::RefCell::new(Vec::new()) };
        }
        SEEN.with(|v| {
            let mut v = v.borrow_mut();
            if let Some((_, _, out)) = v.iter().find(|(k, g, _)| *k == on && *g == grounds) {
                return *out;
            }
            let out = crate::kit::color::ensure_contrast_all(on, &grounds, 3.0);
            if v.len() >= 64 {
                v.clear();
            }
            v.push((on, grounds, out));
            out
        })
    }

    /// The selected strip's body: `mix(surface, hue, .13)` (Paper .14).
    /// Through the theme's selection tint (none in Slate; r39:lapis2).
    pub fn selected(&self, c: Color32) -> Color32 {
        let body = mix(self.surface, c, if self.dark { 0.13 } else { 0.14 });
        crate::kit::color::selected_fill(body, self.sel_tint, 1.0)
    }

    /// A selected strip's body with no colour of its own: a step toward ink 1, through
    /// the selection tint.
    pub fn selected_plain(&self) -> Color32 {
        crate::kit::color::selected_fill(mix(self.surface, self.ink1, 0.05), self.sel_tint, 1.0)
    }
}

/// The fader's law, which the meter beside it shares so one tick reads both: the fraction
/// of the travel from the top for each mark (README, "The fader's law").
pub(crate) mod law {
    const LAW: [(f32, f32); 9] = [
        (6.0, 0.0),
        (0.0, 0.16),
        (-6.0, 0.31),
        (-12.0, 0.45),
        (-18.0, 0.56),
        (-24.0, 0.65),
        (-36.0, 0.79),
        (-48.0, 0.88),
        (cantus_core::MIN_GAIN_DB, 1.0),
    ];
    /// The ticks: +6, 0, −6, −12, −18, −24, −36, −48.
    pub const TICKS: [f32; 8] = [6.0, 0.0, -6.0, -12.0, -18.0, -24.0, -36.0, -48.0];

    /// Where `db` sits on the travel, 0 at the top … 1 at the foot.
    pub fn frac(db: f32) -> f32 {
        if db >= LAW[0].0 {
            return 0.0;
        }
        for w in LAW.windows(2) {
            let ((a, fa), (b, fb)) = (w[0], w[1]);
            if db >= b {
                return fa + (fb - fa) * (a - db) / (a - b);
            }
        }
        1.0
    }

    /// The level at `f` of the travel (the inverse of [`frac`]); the foot is silence.
    pub fn db_at(f: f32) -> f32 {
        let f = f.clamp(0.0, 1.0);
        if f >= 1.0 {
            return cantus_core::MIN_GAIN_DB;
        }
        for w in LAW.windows(2) {
            let ((a, fa), (b, fb)) = (w[0], w[1]);
            if f <= fb {
                let db = a + (b - a) * (f - fa) / (fb - fa);
                return db.max(cantus_core::MIN_GAIN_DB);
            }
        }
        cantus_core::MIN_GAIN_DB
    }
}

/// A strip's measures across, in points from its left edge (README, "Fader and meter
/// geometry"): the groove's centre, the cap, the meter's left and each bar's width.
#[derive(Copy, Clone, Debug, PartialEq)]
pub(crate) struct Geom {
    pub groove_x: f32,
    pub cap: egui::Vec2,
    pub meter_x: f32,
    pub bar_w: f32,
}

pub(crate) fn geom(mode: WidthMode, master: bool) -> Geom {
    let (gx, cw, ch, mx, mw) = match (mode, master) {
        (WidthMode::Compact, false) => (10.0, 16.0, 10.0, 19.0, 2.0),
        (WidthMode::Wide, false) => (21.0, 22.0, 13.0, 37.0, 4.0),
        (WidthMode::Xl, false) => (28.0, 26.0, 14.0, 64.0, 5.0),
        (WidthMode::Compact, true) => (14.0, 18.0, 10.0, 26.0, 3.0),
        (WidthMode::Wide, true) => (31.0, 22.0, 13.0, 47.0, 4.0),
        (WidthMode::Xl, true) => (42.0, 26.0, 14.0, 70.0, 6.0),
    };
    Geom {
        groove_x: gx,
        cap: vec2(cw, ch),
        meter_x: mx,
        bar_w: mw,
    }
}

/// The master's width.
pub(crate) fn master_w(mode: WidthMode) -> f32 {
    match mode {
        WidthMode::Compact => 40.0,
        WidthMode::Wide => 68.0,
        WidthMode::Xl => 110.0,
    }
}

/// The weights des-mixer sets type in.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub(crate) enum W {
    Regular,
    Medium,
    SemiBold,
}

/// A text in des-mixer's terms: `size` points in weight `w`, as one of the kit's roles
/// scaled (Slate: Label is Regular 11, Control Medium 12, Section SemiBold 11).
pub(crate) fn words(s: &str, size: f32, w: W, color: Color32, k: f32) -> (Text<'_>, f32) {
    let (role, base) = match w {
        W::Regular => (Role::Label, 11.0),
        W::Medium => (Role::Control, 12.0),
        W::SemiBold => (Role::Section, 11.0),
    };
    (Text::new(s, role, color).caps(false), k * size / base)
}

/// Paint `s` with its baseline at `y`, anchored at `x` by `align` (left, centre or right),
/// at most `room` wide (then the ellipsis, never a glyph cut). Returns its width.
#[allow(clippy::too_many_arguments)]
pub(crate) fn say(
    b: &mut Brush,
    st: &Style,
    s: &str,
    x: f32,
    y: f32,
    align: Align2,
    room: f32,
    size: f32,
    w: W,
    color: Color32,
) -> f32 {
    if s.is_empty() || room <= 1.0 {
        return 0.0;
    }
    let (t, k) = words(s, size, w, color, st.tk());
    let t = t
        .align(Align2([align.x(), egui::Align::Center]))
        .policy(Policy::Elide(Elide::End));
    let h = size * 1.6;
    let cy = y - size * 0.36;
    let left = match align.x() {
        egui::Align::Min => x,
        egui::Align::Center => x - room * 0.5,
        egui::Align::Max => x - room,
    };
    let r = Rect::from_min_max(pos2(left, cy - h * 0.5), pos2(left + room, cy + h * 0.5));
    text::paint(b, r, &t, k).map_or(0.0, |p| p.rect.width())
}

/// How wide `s` is set at `size` in `w`.
pub(crate) fn width_of(ctx: &egui::Context, st: &Style, s: &str, size: f32, w: W) -> f32 {
    let (t, k) = words(s, size, w, st.t.text, st.tk());
    text::measure(ctx, &t, k).x
}

/// The strip's body: r 3, lit from above (surface + 3.5 % ink 1 at the top, surface from
/// 35 % down) — or a flat fill (selected, empty).
pub(crate) fn body(b: &mut Brush, r: Rect, p: &Pal, flat: Option<Color32>) {
    let r = b.snap_rect(r);
    match flat {
        Some(c) => b.rrect(r, 3.0, Fill::Solid(c)),
        None => {
            let stops = [(0.0, p.strip_top), (0.35, p.surface), (1.0, p.surface)];
            b.rrect(r, 3.0, Fill::Stops(&stops));
        }
    }
}

/// The stripe at the strip's top, 3 high, in the strip's colour.
pub(crate) fn stripe(b: &mut Brush, r: Rect, c: Color32) {
    let r = b.snap_rect(Rect::from_min_size(r.min, vec2(r.width(), 3.0)));
    b.rect(r, Fill::Solid(c));
}

/// The fader's groove: 3 px, `fader.groove`, 3 past each end of the travel.
pub(crate) fn groove(b: &mut Brush, gx: f32, top: f32, bot: f32, p: &Pal) {
    let r = Rect::from_min_max(pos2(gx - 1.5, top - 3.0), pos2(gx + 1.5, bot + 3.0));
    b.rrect(b.snap_rect(r), 1.5, Fill::Solid(p.groove));
}

/// The scale's ticks at `tx`: 0 dB 3 px in ink 3, the others 2 px `mix(ink4, base, .25)`.
pub(crate) fn ticks(b: &mut Brush, tx: f32, top: f32, bot: f32, p: &Pal) {
    let px = b.px();
    for d in law::TICKS {
        let y = b.snap(top + (bot - top) * law::frac(d));
        let (w, c) = if d == 0.0 {
            (3.0, p.ink3)
        } else {
            (2.0, mix(p.ink4, p.base, 0.25))
        };
        b.rect(
            Rect::from_min_size(pos2(b.snap(tx), y), vec2(w, px)),
            Fill::Solid(c),
        );
    }
}

/// The scale's numbers (Regular 9, ink 3; 0 in ink 2), anchored at `x` by `align`.
pub(crate) fn scale_numbers(
    b: &mut Brush,
    st: &Style,
    x: f32,
    align: Align2,
    top: f32,
    bot: f32,
    p: &Pal,
) {
    for (d, s) in [
        (6.0, "+6"),
        (0.0, "0"),
        (-6.0, "\u{2212}6"),
        (-12.0, "\u{2212}12"),
        (-18.0, "\u{2212}18"),
        (-24.0, "\u{2212}24"),
        (-36.0, "\u{2212}36"),
        (-48.0, "\u{2212}48"),
    ] {
        let y = (top + (bot - top) * law::frac(d)).round() + 3.0;
        let c = if d == 0.0 { p.ink2 } else { p.ink3 };
        say(b, st, s, x, y, align, 18.0, 9.0, W::Regular, c);
    }
}

/// The cap at `y` (its centre line): a 1-px `edge`, the `fcap` gradient, two grip lines
/// when it is tall enough, the centre line (ink 1, or the strip's colour when selected).
pub(crate) fn cap(
    b: &mut Brush,
    gx: f32,
    y: f32,
    size: egui::Vec2,
    p: &Pal,
    centre: Color32,
    hot: bool,
) {
    let (hw, hh) = (size.x * 0.5, (size.y * 0.5).floor());
    let y = b.snap(y);
    let outer = Rect::from_min_max(
        pos2(gx - hw - 1.0, y - hh - 1.0),
        pos2(gx + hw + 1.0, y + hh + 1.0),
    );
    b.rrect(b.snap_rect(outer), 2.5, Fill::Solid(p.edge));
    let inner = Rect::from_min_max(pos2(gx - hw, y - hh), pos2(gx + hw, y + hh));
    let f = if hot {
        [
            mix(p.fcap[0], p.ink1, 0.08),
            mix(p.fcap[1], p.ink1, 0.08),
            mix(p.fcap[2], p.ink1, 0.08),
        ]
    } else {
        p.fcap
    };
    let stops = [(0.0, f[0]), (0.5, f[1]), (1.0, f[2])];
    b.rrect(b.snap_rect(inner), 2.0, Fill::Stops(&stops));
    let grip = mix(p.fcap[1], p.edge, 0.55);
    let px = b.px();
    if size.y >= 11.0 {
        for gy in [y - hh + 2.0, y + hh - 2.0] {
            b.rect(
                Rect::from_min_size(pos2(gx - hw + 4.0, gy), vec2(size.x - 8.0, px)),
                Fill::Solid(grip),
            );
        }
    }
    b.rect(
        Rect::from_min_size(pos2(gx - hw + 2.0, y), vec2(size.x - 4.0, px)),
        Fill::Solid(centre),
    );
}

/// The ghost of an empty strip's cap: a flat 4-px bar.
pub(crate) fn cap_ghost(b: &mut Brush, gx: f32, y: f32, w: f32, p: &Pal) {
    let r = Rect::from_min_max(
        pos2(gx - w * 0.5 + 2.0, y - 2.0),
        pos2(gx + w * 0.5 - 2.0, y + 2.0),
    );
    b.rrect(
        b.snap_rect(r),
        2.0,
        Fill::Solid(mix(p.surface, p.ink4, 0.45)),
    );
}

/// What a meter shows: two levels and their held peaks in dB, or nothing (stopped).
#[derive(Copy, Clone, Debug, PartialEq)]
pub(crate) struct Bars {
    pub level: [f32; 2],
    pub hold: [f32; 2],
}

/// The two bars on the fader's law (README, Meter): grooves `meter.groove`; green to −12,
/// yellow-green to −3, amber to 0, `clip` above; the held peak 1 px; the clip cell 8 above
/// the travel, lit `clip` once the strip went over. Stopped: the grooves alone. `dim`: a
/// muted strip's meter at 38 %.
#[allow(clippy::too_many_arguments)]
pub(crate) fn meter(
    b: &mut Brush,
    mx: f32,
    top: f32,
    bot: f32,
    bw: f32,
    bars: Option<Bars>,
    clipped: bool,
    dim: bool,
    p: &Pal,
) {
    let gap = 1.0;
    for k in 0..2 {
        let x = mx + k as f32 * (bw + gap);
        let r = Rect::from_min_max(pos2(x, top - 3.0), pos2(x + bw, bot + 3.0));
        b.rrect(b.snap_rect(r), 1.0, Fill::Solid(p.meter_groove));
    }
    let cw = 2.0 * bw + gap;
    let cell = Rect::from_min_size(pos2(mx, top - 8.0), vec2(cw, 3.0));
    b.rrect(
        b.snap_rect(cell),
        1.0,
        Fill::Solid(if clipped { p.clip } else { p.clip_idle }),
    );
    let Some(bars) = bars else {
        return;
    };
    let (mut lo, mut mid, mut hi) = (p.meter_lo, p.meter_mid, p.meter_hi);
    if dim {
        lo = mix(p.surface, lo, 0.38);
        mid = mix(p.surface, mid, 0.38);
        hi = mix(p.surface, hi, 0.38);
    }
    let ppp = b.ppp();
    let snap = move |v: f32| (v * ppp).round() / ppp;
    let fy = move |d: f32| snap(top + (bot - top) * law::frac(d));
    let (ylo, yhi, y0) = (fy(-12.0), fy(-3.0), fy(0.0));
    let bottom = snap(bot + 3.0);
    let px = b.px();
    for k in 0..2 {
        let x = snap(mx + k as f32 * (bw + gap));
        let v = bars.level[k];
        if v > crate::kit::meter::MIN_DB {
            let ty = fy(v);
            let seg = |b: &mut Brush, a: f32, z: f32, c: Color32| {
                if z > a {
                    b.rect(
                        Rect::from_min_max(pos2(x, a), pos2(x + bw, z)),
                        Fill::Solid(c),
                    );
                }
            };
            seg(b, ty.max(ylo), bottom, lo);
            if ty < ylo {
                seg(b, ty.max(yhi), ylo, mid);
            }
            if ty < yhi {
                seg(b, ty.max(y0), yhi, hi);
            }
            if ty < y0 {
                seg(b, ty, y0, p.clip);
            }
        }
        let h = bars.hold[k];
        if h > crate::kit::meter::MIN_DB {
            let zone = if h < -12.0 {
                lo
            } else if h < -3.0 {
                mid
            } else {
                hi
            };
            let c = if h > 0.0 {
                p.clip
            } else {
                mix(zone, p.ink1, 0.35)
            };
            b.rect(
                Rect::from_min_size(pos2(x, fy(h)), vec2(bw, px)),
                Fill::Solid(c),
            );
        }
    }
}

/// Slate's well: a 1-px lip showing under, a 1-px dark edge, the flat well inside.
pub(crate) fn well(b: &mut Brush, r: Rect, p: &Pal, hot: bool) {
    let r = b.snap_rect(r);
    let px = b.px();
    b.rrect(r, 3.0, Fill::Solid(p.lip));
    b.rrect(
        Rect::from_min_max(r.min, pos2(r.right(), r.bottom() - px)),
        3.0,
        Fill::Solid(p.edge),
    );
    b.rrect(
        r.shrink(px),
        2.0,
        Fill::Solid(if hot { p.well_hi } else { p.well }),
    );
}

/// A glyph of set 3 centred at `c`, des-mixer's `size` across.
pub(crate) fn glyph(
    b: &mut Brush,
    s: &Style,
    c: Pos2,
    size: f32,
    g: MGlyph,
    ink: Ink,
    ground: Color32,
) {
    let inks = mg::inks(s, ink, ground);
    let _ = mg::paint(b, c, size, g, &inks, 1.0);
}

/// A 5-px plus on the pixel grid, ink 4: the first empty row's "add".
pub(crate) fn plus(b: &mut Brush, c: Pos2, ink: Color32) {
    let px = b.px();
    let (x, y) = (b.snap(c.x), b.snap(c.y));
    b.rect(
        Rect::from_min_size(pos2(x - 2.0, y), vec2(5.0, px)),
        Fill::Solid(ink),
    );
    b.rect(
        Rect::from_min_size(pos2(x, y - 2.0), vec2(px, 5.0)),
        Fill::Solid(ink),
    );
}

/// The quiet panel the insert list and the EQ sit on: `strip.panel`, r 3, no edge.
pub(crate) fn panel(b: &mut Brush, r: Rect, p: &Pal) {
    b.rrect(b.snap_rect(r), 3.0, Fill::Solid(p.panel));
}

/// The strip's EQ as a curve in its panel (its own bands and its EQ inserts, sampled once
/// per setting by the desk): the area under it in `accent` 12 %, the line 1 px ink 2; a
/// flat EQ is one line in ink 4. `dots`: three of its points as rings (Large).
#[allow(clippy::too_many_arguments)]
pub(crate) fn eq_curve(
    b: &mut Brush,
    r: Rect,
    db: &[f32],
    shaped: bool,
    dots: Option<&[Option<f32>; 3]>,
    dim: bool,
    p: &Pal,
    pts: &mut Vec<Pos2>,
) {
    panel(b, r, p);
    let mid = r.center().y - 0.5;
    let span = r.height() * 0.5 - 3.0;
    // ±12 dB fill the panel's half.
    let k = span / 12.0;
    let x0 = r.left() + 2.0;
    let w = r.width() - 4.0;
    let n = db.len().max(2);
    pts.clear();
    for (i, d) in db.iter().enumerate() {
        let y = if shaped { mid - d * k } else { mid };
        pts.push(pos2(
            x0 + w * i as f32 / (n - 1) as f32,
            y.clamp(r.top() + 1.0, r.bottom() - 1.0),
        ));
    }
    if pts.len() < 2 {
        return;
    }
    let ink = if !shaped || dim { p.ink4 } else { p.ink2 };
    if shaped && !dim {
        b.area(pts, mid, with_alpha(p.accent, 0.12));
    }
    b.polyline(pts, b.px(), ink);
    // Large: a dot on the curve where each band is (r39:lo-mixer: the real bands).
    if let (Some(dots), true) = (dots, shaped) {
        for (f, c) in dots.iter().zip([p.solo, p.play, p.accent]) {
            let Some(f) = *f else {
                continue;
            };
            let at = (n - 1) as f32 * f.clamp(0.0, 1.0);
            let (i, t) = (at.floor() as usize, at.fract());
            let q0 = pts[i.min(n - 1)];
            let q1 = pts[(i + 1).min(n - 1)];
            let q = q0 + (q1 - q0) * t;
            b.circle(q, 2.0, Fill::Solid(p.well));
            b.circle_stroke(q, 2.0, b.px(), c);
        }
    }
}

/// A frozen strip's sound block: 1-px `accent` lines at 22 %, 4 px apart, at 45°.
pub(crate) fn hatch(b: &mut Brush, r: Rect, p: &Pal) {
    let c = with_alpha(p.accent, 0.22);
    let step = 4.0 * std::f32::consts::SQRT_2;
    let mut d = 0.0;
    // Lines x + y = const, clipped to the rect.
    while d < r.width() + r.height() {
        let (sx, sy) = (r.left() + d, r.top());
        let a = if sx <= r.right() {
            pos2(sx, sy)
        } else {
            pos2(r.right(), sy + (sx - r.right()))
        };
        let end_x = r.left() + d - r.height();
        let z = if end_x >= r.left() {
            pos2(end_x, r.bottom())
        } else {
            pos2(r.left(), r.top() + d)
        };
        if a.y < r.bottom() && z.x <= r.right() {
            b.segment(a, z, b.px(), c, false);
        }
        d += step;
    }
}

/// "−6.0", "+1.4", "0.0" (one decimal, a true minus); silence writes nothing (no "−∞").
pub(crate) fn write_db(out: &mut String, db: f32) {
    use std::fmt::Write as _;
    if db <= cantus_core::MIN_GAIN_DB + 0.01 {
        return;
    }
    if db < -0.05 {
        let _ = write!(out, "\u{2212}{:.1}", -db);
    } else if db >= 0.05 {
        let _ = write!(out, "+{:.1}", db);
    } else {
        out.push_str("0.0");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_law_is_des_mixers_and_inverts() {
        for (db, f) in [
            (6.0, 0.0),
            (0.0, 0.16),
            (-6.0, 0.31),
            (-12.0, 0.45),
            (-24.0, 0.65),
            (-48.0, 0.88),
        ] {
            assert!((law::frac(db) - f).abs() < 1e-5, "{db}");
            assert!((law::db_at(f) - db).abs() < 1e-3, "{f}");
        }
        assert_eq!(law::frac(cantus_core::MIN_GAIN_DB), 1.0);
        assert_eq!(law::frac(12.0), 0.0);
        assert_eq!(law::db_at(1.0), cantus_core::MIN_GAIN_DB);
        for i in 0..=100 {
            let f = i as f32 / 100.0;
            assert!((law::frac(law::db_at(f)) - f).abs() < 1e-3, "{f}");
        }
    }

    #[test]
    fn the_strip_widths_are_the_sheets() {
        assert_eq!(WidthMode::Compact.pitch(), 32.0);
        assert_eq!(WidthMode::Wide.pitch(), 62.0);
        assert_eq!(WidthMode::Xl.pitch(), 96.0);
        assert_eq!(master_w(WidthMode::Wide), 68.0);
        assert_eq!(geom(WidthMode::Wide, false).meter_x, 37.0);
    }

    #[test]
    fn silence_writes_no_minus_infinity() {
        let mut s = String::new();
        write_db(&mut s, cantus_core::MIN_GAIN_DB);
        assert!(s.is_empty());
        write_db(&mut s, -3.54);
        assert_eq!(s, "\u{2212}3.5");
        s.clear();
        write_db(&mut s, 1.44);
        assert_eq!(s, "+1.4");
    }
}
