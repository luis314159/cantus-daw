//! Lenis's screen (design sheet v1): the analyser's phosphor glass, dark in every theme,
//! in three lanes over one frequency axis (20 Hz … 20 kHz, logarithmic):
//!
//! - **The cut per frequency** along the top: a bar for each of the 128 display points
//!   hanging from the lane's top, its length the cut in dB (0 … 24, the scale on the right).
//! - **The spectrum**: the input's spectrum as one fine glowing line over a faint body, the
//!   cut hanging under it as a veil down to the output's line — so the eye sees where it
//!   smooths and how much. With *Delta* on, what is removed is the picture: its own
//!   spectrum lit in the cut's colour, the input a dim line above it.
//! - **The focus**: the focus curve (0 left alone, 1 as set, 2 twice), its six bands as
//!   numbered nodes, the two limits as gates across the whole screen with their tabs; the
//!   screen beyond the limits shaded and hatched.
//!
//! The legend and the quality's frame (points, milliseconds, the bins' width) along the
//! top. Every number from the engine's telemetry (`super::picture`).

use super::super::canvas::{self, Ink, Ruler};
use super::art::{self, c};
use super::picture::{Picture, FLOOR_DB};
use crate::kit::color::{mix, with_alpha};
use crate::kit::text::{self as ktext, Role, Text};
use crate::kit::{Brush, Fill, Style};
use cantus_dsp::lenis::{detect, display_hz, LenisSettings, DISPLAY, FOCUS_BANDS};
use cantus_dsp::math;
use egui::{pos2, vec2, Align2, Color32, Pos2, Rect};

/// The frequency axis, Hz.
pub const LO_HZ: f32 = 20.0;
pub const HI_HZ: f32 = 20_000.0;
/// The spectrum's range, dBFS.
pub const TOP_DB: f32 = 0.0;
pub const BOTTOM_DB: f32 = -96.0;
/// The cut lane's range, dB.
pub const CUT_DB: f32 = 24.0;

/// Where the screen's parts stand.
#[derive(Copy, Clone, Debug)]
pub struct Plot {
    /// The glass.
    pub glass: Rect,
    /// The legend's row(s) along the top.
    pub head: Rect,
    /// The three lanes' common span (the frequency axis's width).
    pub area: Rect,
    pub lane: Rect,
    pub spec: Rect,
    pub focus: Rect,
}

impl Plot {
    /// The lanes in `glass`, the head `head_h` points tall.
    pub fn lay(glass: Rect, k: f32, head_h: f32) -> Plot {
        let inner = Rect::from_min_max(
            pos2(glass.left() + 46.0 * k, glass.top() + 10.0 * k),
            pos2(glass.right() - 46.0 * k, glass.bottom() - 24.0 * k),
        );
        let head = Rect::from_min_size(
            pos2(glass.left() + 14.0 * k, inner.top()),
            vec2(glass.width() - 28.0 * k, head_h),
        );
        let area = Rect::from_min_max(pos2(inner.left(), head.bottom() + 8.0 * k), inner.max);
        let h = area.height();
        let gap = 8.0 * k;
        let lane_h = (h * 0.17).max(40.0 * k);
        let focus_h = (h * 0.24).max(58.0 * k);
        let lane = Rect::from_min_size(area.min, vec2(area.width(), lane_h));
        let focus = Rect::from_min_max(pos2(area.left(), area.bottom() - focus_h), area.max);
        let spec = Rect::from_min_max(
            pos2(area.left(), lane.bottom() + gap),
            pos2(area.right(), focus.top() - gap),
        );
        Plot {
            glass,
            head,
            area,
            lane,
            spec,
            focus,
        }
    }

    /// The x of a frequency.
    pub fn x(&self, hz: f32) -> f32 {
        let t = math::log2(hz.clamp(LO_HZ, HI_HZ) / LO_HZ) / math::log2(HI_HZ / LO_HZ);
        self.area.left() + self.area.width() * t
    }

    /// The frequency at `x`.
    pub fn hz(&self, x: f32) -> f32 {
        let t = ((x - self.area.left()) / self.area.width().max(1.0)).clamp(0.0, 1.0);
        LO_HZ * math::powf(HI_HZ / LO_HZ, t)
    }

    /// The y of a level on the spectrum.
    pub fn y_db(&self, db: f32) -> f32 {
        let t = ((db - BOTTOM_DB) / (TOP_DB - BOTTOM_DB)).clamp(0.0, 1.0);
        self.spec.bottom() - self.spec.height() * t
    }

    /// The y of a cut on the lane (hanging from its top).
    pub fn y_cut(&self, cut: f32) -> f32 {
        self.lane.top() + self.lane.height() * (cut / CUT_DB).clamp(0.0, 1.0)
    }

    /// The y of a focus weight (0 … 2) on its lane.
    pub fn y_focus(&self, w: f32) -> f32 {
        let pad = 9.0 * self.focus.height() / 70.0;
        let r = self.focus.shrink2(vec2(0.0, pad));
        r.bottom() - r.height() * (w / 2.0).clamp(0.0, 1.0)
    }

    /// The focus weight at `y`.
    pub fn w_at(&self, y: f32) -> f32 {
        let pad = 9.0 * self.focus.height() / 70.0;
        let r = self.focus.shrink2(vec2(0.0, pad));
        (2.0 * (r.bottom() - y) / r.height().max(1.0)).clamp(0.0, 2.0)
    }

    /// Where band `n`'s node stands.
    pub fn node(&self, set: &LenisSettings, n: usize) -> Pos2 {
        let b = set.bands[n];
        let w = if b.on { 1.0 + b.amount } else { 1.0 };
        pos2(self.x(b.hz), self.y_focus(w))
    }

    /// The tab of the low (`hi` false) or the high gate, as wide as its words `w`.
    pub fn tab(&self, set: &LenisSettings, hi: bool, w: f32, k: f32) -> Rect {
        let x = self.x(if hi { set.high_hz } else { set.low_hz });
        let size = vec2(w + 12.0 * k, 16.0 * k);
        let top = self.spec.top() + 4.0 * k;
        let min_x = if hi { x - size.x } else { x };
        let min_x = min_x.clamp(self.area.left(), self.area.right() - size.x);
        Rect::from_min_size(pos2(min_x, top), size)
    }
}

/// The colours the screen glows in, from the colourway.
#[derive(Copy, Clone, Debug)]
pub struct Glow {
    pub trace: Color32,
    pub cut: Color32,
    pub focus: Color32,
    pub grid: Color32,
}

impl Glow {
    pub fn of(s: &Style) -> Glow {
        let f = art::finish(s);
        Glow {
            trace: c(f.trace),
            cut: c(f.cut),
            focus: c(f.focus),
            grid: c(f.graticule),
        }
    }
}

/// What the pointer is over or holding on the screen.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Grip {
    Lo,
    Hi,
    Band(usize),
}

/// The words the screen writes that change only with the settings (made by the editor,
/// kept between frames).
pub struct Words<'a> {
    /// "1024-point frame · 10.6 ms · bins of 47 Hz".
    pub frame: &'a str,
    /// "From 150 Hz", "To 20 kHz".
    pub from: &'a str,
    pub to: &'a str,
}

/// A frequency in words: "150 Hz", "2.5 kHz", "20 kHz".
pub fn hz_words(hz: f32, out: &mut String) {
    use std::fmt::Write as _;
    out.clear();
    let khz = hz / 1000.0;
    if hz >= 9_950.0 || (hz >= 1000.0 && (khz - khz.round()).abs() < 0.05) {
        let _ = write!(out, "{:.0} kHz", khz);
    } else if hz >= 1000.0 {
        let _ = write!(out, "{:.1} kHz", hz / 1000.0);
    } else {
        let _ = write!(out, "{hz:.0} Hz");
    }
}

/// The height the head needs for `words` across `w` points (one row, or two when the
/// legend and the frame do not fit side by side).
pub fn head_height(ctx: &egui::Context, k: f32, w: f32, frame: &str, delta: bool) -> f32 {
    let row = 15.0 * k;
    let legend = legend_width(ctx, k, delta);
    let fw = ktext::measure(
        ctx,
        &Text::new(frame, Role::Micro, Color32::TRANSPARENT).caps(false),
        k,
    )
    .x;
    if legend + fw + 24.0 * k <= w {
        row
    } else {
        2.0 * row
    }
}

const LEGEND: [&str; 3] = ["lenis.d.spectrum", "lenis.d.out", "lenis.d.focus"];
const LEGEND_DELTA: [&str; 3] = ["lenis.d.removed", "lenis.d.spectrum", "lenis.d.focus"];

fn legend_width(ctx: &egui::Context, k: f32, delta: bool) -> f32 {
    let keys = if delta { LEGEND_DELTA } else { LEGEND };
    keys.iter()
        .map(|key| {
            14.0 * k
                + ktext::measure(ctx, &Text::key(key, Role::Micro, Color32::TRANSPARENT), k).x
                + 14.0 * k
        })
        .sum()
}

/// The frequencies the axis names, and whether each is a decade (a stronger line).
const MARKS: [(f32, bool); 10] = [
    (20.0, false),
    (50.0, false),
    (100.0, true),
    (200.0, false),
    (500.0, false),
    (1000.0, true),
    (2000.0, false),
    (5000.0, false),
    (10_000.0, true),
    (20_000.0, false),
];

/// The screen: the glass, the lanes, the curves, the words, the glass's cover.
#[allow(clippy::too_many_arguments)]
pub fn paint(
    b: &mut Brush,
    s: &Style,
    ink: &Ink,
    p: &Plot,
    set: &LenisSettings,
    pic: &Picture,
    sel: usize,
    hot: Option<Grip>,
    words: &Words<'_>,
) {
    let k = s.k;
    let g = Glow::of(s);
    art::glass(b, s, p.glass);
    graticule(b, s, ink, p, &g);
    shade_outside(b, s, p, set);
    lane(b, s, ink, p, pic, &g);
    spectrum(b, s, ink, p, set, pic, &g);
    focus(b, s, ink, p, set, sel, hot, &g);
    gates(b, s, ink, p, set, hot, words, &g);
    head(b, s, ink, p, set, words, &g);
    if set.delta {
        let at = pos2(p.spec.center().x, p.spec.top() + 26.0 * k);
        canvas::put(
            b,
            at,
            Align2::CENTER_TOP,
            &Text::key("lenis.d.listening", Role::Label, g.cut).caps(false),
            k,
            Some(with_alpha(ink.plate, 0.9)),
        );
    }
    if !pic.seen {
        canvas::put(
            b,
            p.spec.center(),
            Align2::CENTER_CENTER,
            &Text::key("lenis.d.rest", Role::Label, ink.dim).caps(false),
            k,
            None,
        );
    }
    art::glass_cover(b, s, p.glass);
}

fn graticule(b: &mut Brush, s: &Style, ink: &Ink, p: &Plot, g: &Glow) {
    let k = s.k;
    let fine = with_alpha(g.grid, 0.10);
    let strong = with_alpha(g.grid, 0.22);
    // The frequency lines through every lane, the decades stronger.
    for (hz, decade) in MARKS {
        let x = p.x(hz);
        let col = if decade { strong } else { fine };
        for r in [p.lane, p.spec, p.focus] {
            b.vline(x, r.top(), r.bottom(), 0.6 * k, col);
        }
    }
    // A fine tick every third of an octave along the spectrum's foot and the lane's.
    let mut hz = 25.0f32;
    while hz < HI_HZ {
        let x = p.x(hz);
        b.vline(
            x,
            p.spec.bottom() - 3.0 * k,
            p.spec.bottom(),
            0.6 * k,
            strong,
        );
        b.vline(x, p.lane.bottom() - 2.0 * k, p.lane.bottom(), 0.6 * k, fine);
        hz *= math::powf(2.0, 1.0 / 3.0);
    }
    // The spectrum's levels: a line every 12 dB, numbered every 24 on the left.
    let mut db = TOP_DB;
    while db >= BOTTOM_DB - 0.1 {
        let y = p.y_db(db);
        let major = ((db as i32) % 24) == 0;
        b.hline(
            p.spec.left(),
            p.spec.right(),
            y,
            0.6 * k,
            if major { strong } else { fine },
        );
        if major && db > BOTTOM_DB + 1.0 {
            canvas::putf(
                b,
                pos2(p.spec.left() - 5.0 * k, y),
                Align2::RIGHT_CENTER,
                Role::Micro,
                ink.dim,
                k,
                None,
                format_args!("{}", Minus(db)),
            );
        }
        db -= 12.0;
    }
    canvas::putf(
        b,
        pos2(p.spec.left() - 5.0 * k, p.spec.bottom()),
        Align2::RIGHT_BOTTOM,
        Role::Micro,
        ink.dim,
        k,
        None,
        format_args!("dBFS"),
    );
    // The lane's scale on the right, every 6 dB.
    for cut in [6.0f32, 12.0, 18.0] {
        let y = p.y_cut(cut);
        b.hline(p.lane.left(), p.lane.right(), y, 0.6 * k, fine);
        canvas::putf(
            b,
            pos2(p.lane.right() + 5.0 * k, y),
            Align2::LEFT_CENTER,
            Role::Micro,
            with_alpha(g.cut, 0.9),
            k,
            None,
            format_args!("{cut:.0} dB"),
        );
    }
    b.hline(
        p.lane.left(),
        p.lane.right(),
        p.lane.top(),
        0.8 * k,
        with_alpha(g.cut, 0.45),
    );
    // The focus lane's guides: left alone, as set, twice.
    for (w, a) in [(0.0f32, 0.30f32), (1.0, 0.22), (2.0, 0.12)] {
        b.hline(
            p.focus.left(),
            p.focus.right(),
            p.y_focus(w),
            0.6 * k,
            with_alpha(g.focus, a),
        );
    }
    // The frequencies under the screen.
    HZ.with(|cell| {
        let Ok(mut buf) = cell.try_borrow_mut() else {
            return;
        };
        let mut ruler = Ruler::new();
        for (i, (hz, _)) in MARKS.iter().enumerate() {
            hz_words(*hz, &mut buf);
            let align = if i == 0 {
                Align2::LEFT_TOP
            } else if i == MARKS.len() - 1 {
                Align2::RIGHT_TOP
            } else {
                Align2::CENTER_TOP
            };
            let t = Text::new(&buf, Role::Micro, ink.dim).caps(false);
            let size = ktext::measure(b.ctx(), &t, k);
            let at = pos2(p.x(*hz), p.area.bottom() + 5.0 * k);
            let r = align.anchor_size(at, size);
            if ruler.fits(r.left(), 8.0 * k) {
                ruler.placed(canvas::put(b, at, align, &t, k, None));
            }
        }
    });
}

thread_local! {
    /// The axis's words, written into one reused buffer.
    static HZ: std::cell::RefCell<String> = std::cell::RefCell::new(String::with_capacity(16));
}

/// A negative level written with a true minus.
struct Minus(f32);

impl std::fmt::Display for Minus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.0 < 0.0 {
            write!(f, "\u{2212}{:.0}", -self.0)
        } else {
            write!(f, "{:.0}", self.0)
        }
    }
}

/// The screen beyond the focus's limits: shaded and hatched, as a mask over the glass.
fn shade_outside(b: &mut Brush, s: &Style, p: &Plot, set: &LenisSettings) {
    let k = s.k;
    let (x0, x1) = (p.x(set.low_hz), p.x(set.high_hz));
    let dark = with_alpha(art::shadow(), 0.42);
    let hatch = with_alpha(c(art::finish(s).graticule), 0.07);
    for r in [
        Rect::from_min_max(p.area.min, pos2(x0, p.area.bottom())),
        Rect::from_min_max(pos2(x1, p.area.top()), p.area.max),
    ] {
        if r.width() > 0.5 {
            b.rect(r, Fill::Solid(dark));
            canvas::hatch(b, r, 7.0 * k, 0.6 * k, hatch);
        }
    }
}

/// The cut per frequency: a bar hanging from the lane's top at every display point.
fn lane(b: &mut Brush, s: &Style, ink: &Ink, p: &Plot, pic: &Picture, g: &Glow) {
    let k = s.k;
    let step = p.lane.width() / DISPLAY as f32;
    let w = (step * 0.62).max(1.0 * k);
    for i in 0..DISPLAY {
        let cut = pic.cut[i];
        if cut < 0.05 {
            continue;
        }
        let x = p.x(display_hz(i));
        let y = p.y_cut(cut);
        let r = Rect::from_min_max(pos2(x - w * 0.5, p.lane.top()), pos2(x + w * 0.5, y));
        b.rect(r, Fill::V(with_alpha(g.cut, 0.35), with_alpha(g.cut, 0.95)));
        b.rect(
            Rect::from_min_max(pos2(r.left(), y - 1.2 * k), pos2(r.right(), y)),
            Fill::Solid(mix(g.cut, art::glint(), 0.35)),
        );
    }
    canvas::put(
        b,
        pos2(p.lane.left() + 4.0 * k, p.lane.bottom() - 3.0 * k),
        Align2::LEFT_BOTTOM,
        &Text::key("lenis.d.cut", Role::Micro, ink.dim),
        k,
        Some(with_alpha(ink.plate, 0.85)),
    );
}

/// The removed part's level at a point: the input at `level` dB cut by `cut` dB leaves
/// `level + 20·log10(1 − 10^(−cut/20))` behind it.
pub fn removed_db(level: f32, cut: f32) -> f32 {
    if cut < 0.01 {
        return FLOOR_DB;
    }
    let left = 1.0 - math::db_to_gain(-cut);
    (level + 20.0 * math::log10(left.max(1e-6))).max(FLOOR_DB)
}

/// The spectrum: the input's line and body, the cut's veil under it down to the output's
/// line; or, with the delta, what is removed lit and the input dim above it.
fn spectrum(
    b: &mut Brush,
    s: &Style,
    ink: &Ink,
    p: &Plot,
    set: &LenisSettings,
    pic: &Picture,
    g: &Glow,
) {
    let k = s.k;
    let mut input = [Pos2::ZERO; DISPLAY];
    let mut output = [Pos2::ZERO; DISPLAY];
    for i in 0..DISPLAY {
        let x = p.x(display_hz(i));
        let lv = pic.level[i];
        input[i] = pos2(x, p.y_db(lv));
        let out = if set.delta {
            removed_db(lv, pic.cut[i])
        } else {
            lv - pic.cut[i]
        };
        output[i] = pos2(x, p.y_db(out));
    }
    let foot = p.spec.bottom();
    if set.delta {
        // What is removed is the picture: its body and its line in the cut's colour, the
        // input a dim line over it for where it came from.
        b.area(&output, foot, with_alpha(g.cut, 0.22));
        canvas::trace(b, &output, 1.6 * k, g.cut, ink);
        b.polyline(&input, 1.0 * k, with_alpha(g.trace, 0.45));
        return;
    }
    // The input's body, faint, then the veil the cut hangs under the line.
    // The body: faint to the foot, a brighter band just under the line (a phosphor's
    // bloom).
    b.area(&input, foot, with_alpha(g.trace, 0.06));
    let mut upper = input;
    for (depth, a) in [(6.0f32, 0.06f32), (14.0, 0.04), (26.0, 0.025)] {
        let mut lower = input;
        for q in lower.iter_mut() {
            q.y = (q.y + depth * k).min(foot);
        }
        b.between(&upper, &lower, with_alpha(g.trace, a));
        upper = lower;
    }
    b.between(&input, &output, with_alpha(g.cut, 0.42));
    b.polyline(&output, 0.9 * k, with_alpha(g.cut, 0.85));
    canvas::trace(b, &input, 1.5 * k, g.trace, ink);
}

/// The focus lane: the curve and its fill, the bands' nodes, the selected band's reach.
#[allow(clippy::too_many_arguments)]
fn focus(
    b: &mut Brush,
    s: &Style,
    ink: &Ink,
    p: &Plot,
    set: &LenisSettings,
    sel: usize,
    hot: Option<Grip>,
    g: &Glow,
) {
    let k = s.k;
    const N: usize = 160;
    let mut pts = [Pos2::ZERO; N];
    for (i, q) in pts.iter_mut().enumerate() {
        let x = p.area.left() + p.area.width() * i as f32 / (N - 1) as f32;
        *q = pos2(x, p.y_focus(detect::focus_weight(set, p.hz(x))));
    }
    b.area(&pts, p.y_focus(0.0), with_alpha(g.focus, 0.14));
    b.polyline(&pts, 1.4 * k, g.focus);
    // The words of the guides, at the lane's left.
    let plate = Some(with_alpha(ink.plate, 0.85));
    for (w, key) in [
        (2.0f32, "lenis.d.twice"),
        (1.0, "lenis.d.as_set"),
        (0.0, "lenis.d.alone"),
    ] {
        canvas::put(
            b,
            pos2(p.focus.left() + 4.0 * k, p.y_focus(w)),
            Align2::LEFT_CENTER,
            &Text::key(key, Role::Micro, with_alpha(g.focus, 0.9)),
            k,
            plate,
        );
    }
    // The selected band's reach: its half-height points, dashed.
    let bsel = set.bands[sel.min(FOCUS_BANDS - 1)];
    if bsel.on {
        let node = p.node(set, sel);
        for d in [-0.5f32, 0.5] {
            let x = p.x(bsel.hz * math::powf(2.0, d * bsel.width_oct));
            canvas::dashed(
                b,
                pos2(x, p.focus.top() + 2.0 * k),
                pos2(x, p.focus.bottom() - 2.0 * k),
                0.7 * k,
                with_alpha(g.focus, 0.55),
                3.0 * k,
                3.0 * k,
            );
        }
        b.hline(
            p.x(bsel.hz * math::powf(2.0, -0.5 * bsel.width_oct)),
            p.x(bsel.hz * math::powf(2.0, 0.5 * bsel.width_oct)),
            node.y,
            0.7 * k,
            with_alpha(g.focus, 0.45),
        );
    }
    // The nodes: the bands switched on lit and numbered, the others small hollow rings
    // waiting on the "as set" line.
    for n in 0..FOCUS_BANDS {
        let band = set.bands[n];
        let at = p.node(set, n);
        let is_hot = hot == Some(Grip::Band(n));
        let rad = if band.on { 8.0 * k } else { 6.0 * k };
        if band.on {
            b.radial(
                p.focus.expand(12.0 * k),
                at,
                egui::Vec2::splat(rad * 2.2),
                &[
                    (0.0, with_alpha(g.focus, 0.35)),
                    (1.0, with_alpha(g.focus, 0.0)),
                ],
                8,
            );
            b.circle(
                at,
                rad,
                Fill::Solid(if is_hot {
                    mix(g.focus, art::glint(), 0.3)
                } else {
                    g.focus
                }),
            );
        } else {
            b.circle(at, rad, Fill::Solid(with_alpha(ink.ground, 0.9)));
            b.circle_stroke(
                at,
                rad,
                1.0 * k,
                with_alpha(g.focus, if is_hot { 0.95 } else { 0.55 }),
            );
        }
        if n == sel {
            b.circle_stroke(at, rad + 3.0 * k, 1.0 * k, with_alpha(g.focus, 0.9));
        }
        let num = if band.on {
            ink.ground
        } else {
            with_alpha(g.focus, 0.85)
        };
        canvas::putf(
            b,
            at,
            Align2::CENTER_CENTER,
            Role::Micro,
            num,
            k,
            None,
            format_args!("{}", n + 1),
        );
    }
}

/// The focus's two limits: a line across every lane, a tab naming it.
#[allow(clippy::too_many_arguments)]
fn gates(
    b: &mut Brush,
    s: &Style,
    ink: &Ink,
    p: &Plot,
    set: &LenisSettings,
    hot: Option<Grip>,
    words: &Words<'_>,
    g: &Glow,
) {
    let k = s.k;
    for (hi, text) in [(false, words.from), (true, words.to)] {
        let grip = if hi { Grip::Hi } else { Grip::Lo };
        let lit = hot == Some(grip);
        let x = p.x(if hi { set.high_hz } else { set.low_hz });
        let col = with_alpha(g.focus, if lit { 1.0 } else { 0.75 });
        b.vline(
            x,
            p.area.top(),
            p.area.bottom(),
            if lit { 1.6 * k } else { 1.0 * k },
            col,
        );
        let t = Text::new(text, Role::Micro, g.focus).caps(false);
        let w = ktext::measure(b.ctx(), &t, k).x;
        let tab = p.tab(set, hi, w, k);
        b.rrect(tab, 3.0 * k, Fill::Solid(with_alpha(ink.ground, 0.92)));
        b.rrect_stroke(tab, 3.0 * k, if lit { 1.2 * k } else { 0.8 * k }, col);
        canvas::put(b, tab.center(), Align2::CENTER_CENTER, &t, k, None);
    }
}

/// The head: the legend (each colour named) on the left, the quality's frame on the right
/// (under the legend when they would meet).
#[allow(clippy::too_many_arguments)]
fn head(
    b: &mut Brush,
    s: &Style,
    ink: &Ink,
    p: &Plot,
    set: &LenisSettings,
    words: &Words<'_>,
    g: &Glow,
) {
    let k = s.k;
    let row = 15.0 * k;
    let keys = if set.delta { LEGEND_DELTA } else { LEGEND };
    let cols = if set.delta {
        [g.cut, with_alpha(g.trace, 0.6), g.focus]
    } else {
        [g.trace, g.cut, g.focus]
    };
    let y = p.head.top() + row * 0.5;
    let mut x = p.head.left();
    for (key, col) in keys.iter().zip(cols) {
        b.rect(
            Rect::from_center_size(pos2(x + 5.0 * k, y), vec2(10.0 * k, 2.4 * k)),
            Fill::Solid(col),
        );
        let t = canvas::put(
            b,
            pos2(x + 14.0 * k, y),
            Align2::LEFT_CENTER,
            &Text::key(key, Role::Micro, ink.text),
            k,
            None,
        );
        x = t.right() + 14.0 * k;
    }
    let two = p.head.height() > row * 1.5;
    let (at, align) = if two {
        (pos2(p.head.left(), y + row), Align2::LEFT_CENTER)
    } else {
        (pos2(p.head.right(), y), Align2::RIGHT_CENTER)
    };
    canvas::put(
        b,
        at,
        align,
        &Text::new(words.frame, Role::Micro, ink.dim).caps(false),
        k,
        None,
    );
}
