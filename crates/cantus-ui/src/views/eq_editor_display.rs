//! The EQ's display: its geometry, what it draws (through the kit's brush, in the theme's
//! tokens, every word measured and placed where it touches nothing), and every gesture on
//! it — the plot, the selectors over it, the frequency axis (or the piano) under it, the
//! right-click menus and the typing boxes.

// The display walks parallel arrays (the bands, their curves, their switches) by band
// number and point index; iterators over one of them would hide that.
#![allow(clippy::needless_range_loop)]

use super::*;
use crate::kit::color::{ensure_contrast, on_fill, with_alpha};
use crate::kit::menu::Item;
use crate::kit::text::{self as ktext, Elide, Policy, Role, Text};
use crate::kit::{with_brush, Brush, Fill, State};
use egui::{Align2, Vec2};

/// The strip at the display's top that holds its selectors.
pub(super) const HEAD_H: f32 = 30.0;
/// Height of the frequency axis under the plot; the piano is taller.
const AXIS_H: f32 = 20.0;
const PIANO_H: f32 = 32.0;
const DOT_R: f32 = 8.0;

/// Geometry of the display, shared by drawing and hit-testing.
#[derive(Copy, Clone, Debug)]
pub(super) struct Geo {
    pub rect: Rect,
    pub range: f32,
    pub lo: f32,
    pub hi: f32,
}

impl Geo {
    pub fn new(rect: Rect, range: f32, view: (f32, f32)) -> Geo {
        Geo {
            rect,
            range,
            lo: view.0,
            hi: view.1,
        }
    }
    pub fn x(&self, hz: f32) -> f32 {
        let f = (hz.max(1.0) / self.lo).ln() / (self.hi / self.lo).ln();
        self.rect.left() + f * self.rect.width()
    }
    pub fn hz(&self, x: f32) -> f32 {
        let f = (x - self.rect.left()) / self.rect.width().max(1.0);
        self.lo * (self.hi / self.lo).powf(f)
    }
    /// The frequency of point `i` of `n` across the view.
    pub fn hz_at(&self, i: usize, n: usize) -> f32 {
        self.lo * (self.hi / self.lo).powf(i as f32 / (n - 1) as f32)
    }
    /// The x of point `i` of `n`.
    pub fn x_at(&self, i: usize, n: usize) -> f32 {
        self.rect.left() + self.rect.width() * i as f32 / (n - 1) as f32
    }
    pub fn y(&self, db: f32) -> f32 {
        self.rect.center().y - (db / self.range).clamp(-1.1, 1.1) * self.rect.height() * 0.5
    }
    pub fn db(&self, y: f32) -> f32 {
        (self.rect.center().y - y) / (self.rect.height() * 0.5) * self.range
    }
    /// The analyser's scale: its own, from `-floor` dB to 0 dBFS over the full height.
    pub fn y_spec(&self, db: f32, floor: f32) -> f32 {
        let f = ((db + floor) / floor).clamp(0.0, 1.0);
        self.rect.bottom() - f * self.rect.height()
    }
    pub fn dot(&self, b: &BandSettings) -> Pos2 {
        let gain = if b.shape.has_gain() { b.gain_db } else { 0.0 };
        pos2(self.x(b.freq), self.y(gain))
    }
}

/// A drag in progress on the plot.
#[derive(Clone, Debug)]
pub(super) struct Drag {
    kind: DragKind,
    /// The gesture it belongs to: the host folds it into one step.
    gesture: u64,
    /// Pointer movement so far, already scaled for fine moves.
    acc: Vec2,
    /// `Some(true)` locks to frequency, `Some(false)` to gain (Alt).
    lock: Option<bool>,
}

#[derive(Clone, Debug)]
enum DragKind {
    /// The selected bands, where each started: (band, freq, gain, q).
    Bands {
        start: Vec<(usize, f32, f32, f32)>,
        /// Ctrl: vertical movement changes Q instead of gain.
        q_mode: bool,
    },
    /// A dynamic band's range handle.
    Range { band: usize, start: f32 },
}

/// A drag on the frequency axis: zoom (vertical) and pan (horizontal).
#[derive(Clone, Debug)]
pub(super) struct AxisDrag {
    anchor: f32,
    view: (f32, f32),
    acc: Vec2,
}

/// Which right-click list is open.
#[derive(Copy, Clone, Debug, PartialEq)]
pub(super) enum MenuKind {
    Band(usize),
    Shape(usize),
    Place(usize),
    Background,
}

/// An open right-click list, and where it opened.
#[derive(Copy, Clone, Debug)]
pub(super) struct Menu {
    kind: MenuKind,
    at: Pos2,
}

impl Menu {
    pub(super) fn new(kind: MenuKind, at: Pos2) -> Menu {
        Menu { kind, at }
    }
}

/// A curve across the display, and what it was computed from.
#[derive(Clone, Debug)]
struct Cached {
    band: BandSettings,
    mode: Mode,
    view: (f32, f32),
    sr: f32,
    db: Vec<f32>,
}

/// The slot of the ghost's curve (what a double-click would make).
const GHOST: usize = 3 * EQ_BANDS;

/// The curves the display draws, designed once and read until what they depend on
/// changes: per band its curve (`0..24`), its dynamic reach (`24..48`) and where it is
/// now (`48..72`), and the ghost.
#[derive(Debug, Default)]
pub(super) struct Curves {
    slots: Vec<Option<Cached>>,
}

impl Curves {
    fn get(
        &mut self,
        slot: usize,
        b: &BandSettings,
        mode: Mode,
        view: (f32, f32),
        sr: f32,
        hz: &[f32; POINTS],
    ) -> &[f32] {
        if self.slots.len() <= GHOST {
            self.slots.resize_with(GHOST + 1, || None);
        }
        let hit = matches!(&self.slots[slot],
            Some(c) if c.band == *b && c.mode == mode && c.view == view && c.sr == sr);
        if !hit {
            let c = self.slots[slot].get_or_insert_with(|| Cached {
                band: *b,
                mode,
                view,
                sr,
                db: vec![0.0; POINTS],
            });
            c.band = *b;
            c.mode = mode;
            c.view = view;
            c.sr = sr;
            aequalis::band_curve_db(b, mode, sr, hz, &mut c.db);
        }
        match &self.slots[slot] {
            Some(c) => &c.db,
            None => &[],
        }
    }
}

/// What the display computes each frame, in buffers kept between frames.
#[derive(Debug)]
pub(super) struct Buf {
    hz: [f32; POINTS],
    hz_view: (f32, f32),
    /// The summed curve of the bands on the whole image (`0`), and of the whole image
    /// plus the bands placed on each part of it (`1..=4`: L, R, M, S).
    total: [[f32; POINTS]; 5],
    /// Which parts have bands of their own.
    part: [bool; 5],
    a: [f32; POINTS],
    b: [f32; POINTS],
    pts: [Pos2; POINTS],
    pts2: [Pos2; POINTS],
    hit: [bool; POINTS],
    /// The hatching's per-column curve heights.
    cols: Vec<f32>,
    /// The grab's peaks (point indices), left to right.
    peaks: [usize; 8],
    npeaks: usize,
    /// Words being measured before they are placed.
    s: String,
    /// New pairs arrived since the spectra were last computed.
    pub(super) fresh: bool,
    /// The host's recent pairs as last read: (how many, the last pair's bits).
    pub(super) tail: (usize, u32, u32),
}

impl Default for Buf {
    fn default() -> Self {
        Buf {
            hz: [0.0; POINTS],
            hz_view: (0.0, 0.0),
            total: [[0.0; POINTS]; 5],
            part: [false; 5],
            a: [0.0; POINTS],
            b: [0.0; POINTS],
            pts: [Pos2::ZERO; POINTS],
            pts2: [Pos2::ZERO; POINTS],
            hit: [false; POINTS],
            cols: Vec::new(),
            peaks: [0; 8],
            npeaks: 0,
            s: String::new(),
            fresh: true,
            tail: (0, 0, 0),
        }
    }
}

/// Rects already taken by marks and words: a word goes where it touches none of them, or
/// is not painted (`cantus-audit.md` #26: band dots and their M/S badges overlapped).
struct Placer {
    r: [Rect; 192],
    n: usize,
}

impl Placer {
    fn new() -> Placer {
        Placer {
            r: [Rect::NOTHING; 192],
            n: 0,
        }
    }
    fn free(&self, r: Rect) -> bool {
        self.r[..self.n].iter().all(|o| !o.intersects(r))
    }
    fn add(&mut self, r: Rect) {
        if self.n < self.r.len() {
            self.r[self.n] = r;
            self.n += 1;
        }
    }
    /// The first of `options` that is free and inside `within`, taken.
    fn place(&mut self, options: &[Rect], within: Rect) -> Option<Rect> {
        let r = options
            .iter()
            .copied()
            .find(|r| within.contains_rect(*r) && self.free(*r))?;
        self.add(r);
        Some(r)
    }
}

fn used_at(on: &[bool; EQ_BANDS], band: usize) -> bool {
    on.get(band).copied().unwrap_or(false)
}

/// The shape a double-click (or a press on the curve) makes at a point of the plot.
fn shape_at(rect: Rect, pt: Pos2, on_curve: bool) -> EqShape {
    let fx = (pt.x - rect.left()) / rect.width();
    let fy = (pt.y - rect.top()) / rect.height();
    if on_curve {
        return if fx < 0.1 {
            EqShape::LowShelf
        } else if fx > 0.9 {
            EqShape::HighShelf
        } else {
            EqShape::Bell
        };
    }
    if fx < 0.05 {
        EqShape::LowCut
    } else if fx > 0.95 {
        EqShape::HighCut
    } else if fx < 0.12 {
        EqShape::LowShelf
    } else if fx > 0.88 {
        EqShape::HighShelf
    } else if fy > 0.9 {
        EqShape::Notch
    } else {
        EqShape::Bell
    }
}

/// The part of the spectrum an audition plays, Hz.
fn audible_region(b: &BandSettings, view: (f32, f32)) -> (f32, f32) {
    match b.shape {
        EqShape::LowShelf | EqShape::LowCut => (view.0, b.freq),
        EqShape::HighShelf | EqShape::HighCut => (b.freq, view.1),
        EqShape::TiltShelf => view,
        _ => {
            // A band-pass's −3 dB points: bandwidth in octaves from its Q.
            let bw = 2.0 / std::f32::consts::LN_2 * (1.0 / (2.0 * b.q.max(0.1))).asinh();
            let k = 2f32.powf(bw * 0.5);
            (b.freq / k, b.freq * k)
        }
    }
}

/// The spectrum's peaks worth grabbing: local maxima that stand at least 6 dB over their
/// surroundings within a third of an octave, the eight loudest, left to right.
fn peaks(spec: &[f32; POINTS], geo: &Geo, floor: f32, out: &mut [usize; 8]) -> usize {
    let n = POINTS;
    let per_oct = (n - 1) as f32 / (geo.hi / geo.lo).log2();
    let w = (per_oct / 3.0).ceil().max(2.0) as usize;
    let mut found = 0;
    for i in 1..n - 1 {
        let v = spec[i];
        if v <= -floor + 12.0 {
            continue;
        }
        let lo = i.saturating_sub(w);
        let hi = (i + w).min(n - 1);
        if spec[lo..=hi].iter().any(|x| *x > v) {
            continue;
        }
        let left = spec[lo..i].iter().copied().fold(f32::MAX, f32::min);
        let right = spec[i + 1..=hi].iter().copied().fold(f32::MAX, f32::min);
        if v - left.max(right) < 6.0 {
            continue;
        }
        if found < out.len() {
            out[found] = i;
            found += 1;
        } else if let Some(q) =
            (0..out.len()).min_by(|a, b| spec[out[*a]].total_cmp(&spec[out[*b]]))
        {
            // Keep the eight loudest.
            if spec[out[q]] < v {
                out[q] = i;
            }
        }
    }
    out[..found].sort_unstable();
    found
}

/// The grid's frequency lines for a view — decades, and their 2 and 5 marks — as
/// (Hz, a decade line, labelled).
fn grid_lines(lo: f32, hi: f32, mut f: impl FnMut(f32, bool, bool)) {
    let span_oct = (hi / lo).log2();
    for decade in [1.0f32, 10.0, 100.0, 1_000.0, 10_000.0] {
        for m in 1..10 {
            let hz = decade * m as f32;
            if hz < lo || hz > hi {
                continue;
            }
            let strong = m == 1;
            let labelled = strong || m == 5 || (m == 2 && span_oct < 6.0) || span_oct < 3.0;
            if strong || m == 2 || m == 5 || span_oct < 5.0 {
                f(hz, strong, labelled);
            }
        }
    }
}

/// An axis tick: `50`, `500`, `1k`, `2.5k`, `20k`.
fn axis_label(hz: f32, out: &mut String) {
    out.clear();
    if hz >= 1_000.0 {
        let k = hz / 1_000.0;
        if (k - k.round()).abs() < 0.01 {
            let _ = write!(out, "{:.0}k", k);
        } else {
            let _ = write!(out, "{k:.1}k");
        }
    } else {
        let _ = write!(out, "{hz:.0}");
    }
}

/// A moving average of `v` over `±w` points, into `out`.
fn smooth(v: &[f32; POINTS], w: usize, out: &mut [f32; POINTS]) {
    for (i, o) in out.iter_mut().enumerate() {
        let lo = i.saturating_sub(w);
        let hi = (i + w).min(POINTS - 1);
        *o = v[lo..=hi].iter().sum::<f32>() / (hi - lo + 1) as f32;
    }
}

/// A dashed polyline: `dash` on, `gap` off, measured along the line.
pub(super) fn dashed(b: &mut Brush, pts: &[Pos2], w: f32, color: Color32, dash: f32, gap: f32) {
    let mut on = true;
    let mut left = dash;
    for win in pts.windows(2) {
        let (mut a, c) = (win[0], win[1]);
        let mut len = (c - a).length();
        while len > 1e-4 {
            let step = left.min(len);
            let e = a + (c - a) * (step / len);
            if on {
                b.segment(a, e, w, color, false);
            }
            left -= step;
            len -= step;
            a = e;
            if left <= 1e-4 {
                on = !on;
                left = if on { dash } else { gap };
            }
        }
    }
}

/// Diagonal hatching under a curve (points left to right) down to `bottom`. `ys` is
/// scratch kept between frames: the curve's height at every pixel column, so each step
/// along a hatch line is one lookup.
fn hatch_under(
    b: &mut Brush,
    pts: &[Pos2],
    bottom: f32,
    step: f32,
    color: Color32,
    ys: &mut Vec<f32>,
) {
    let (Some(first), Some(last)) = (pts.first(), pts.last()) else {
        return;
    };
    if pts.len() < 2 {
        return;
    }
    let (x0, x1) = (first.x, last.x.max(first.x + 1.0));
    let cols = (x1 - x0).ceil() as usize + 1;
    ys.clear();
    let mut j = 1;
    for c in 0..cols {
        let x = x0 + c as f32;
        while j + 1 < pts.len() && pts[j].x < x {
            j += 1;
        }
        let (a, q) = (pts[j - 1], pts[j]);
        let y = if (q.x - a.x).abs() < 1e-3 {
            a.y
        } else {
            a.y + (q.y - a.y) * ((x - a.x) / (q.x - a.x)).clamp(0.0, 1.0)
        };
        ys.push(y);
    }
    let top = ys.iter().copied().fold(bottom, f32::min);
    let h = (bottom - top).max(1.0);
    let px = b.px();
    let mut c = x0 - h;
    while c < x1 {
        // The part of this line over the curve's span and under its highest point.
        let t0 = (x0 - c).max(0.0);
        let t1 = (x1 - c).min(h);
        let mut run: Option<(Pos2, Pos2)> = None;
        let mut t = t0;
        while t <= t1 {
            let at = pos2(c + t, bottom - t);
            let col = ((at.x - x0) as usize).min(cols - 1);
            let inside = at.y >= ys[col];
            match (&mut run, inside) {
                (Some((_, end)), true) => *end = at,
                (None, true) => run = Some((at, at)),
                (Some((a, e)), false) => {
                    b.segment(*a, *e, px, color, false);
                    run = None;
                }
                (None, false) => {}
            }
            t += 2.0;
        }
        if let Some((a, e)) = run {
            b.segment(a, e, px, color, false);
        }
        c += step;
    }
}

/// L, R, M, S: a letter and a colour each.
fn placement_style(pl: Placement, ink: &Ink) -> (&'static str, Color32) {
    match pl {
        Placement::Stereo => ("", ink.hue),
        Placement::Left => ("L", ink.text),
        Placement::Right => ("R", band_color(4)),
        Placement::Mid => ("M", band_color(3)),
        Placement::Side => ("S", band_color(6)),
    }
}

/// `A4`, `C#2 +13`, written into `out`.
fn note_label(hz: f32, out: &mut String) {
    const NAMES: [&str; 12] = [
        "C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B",
    ];
    let (n, cents) = parse::note_of(hz);
    let n = n.clamp(0, 127);
    let _ = write!(out, "{}{}", NAMES[(n % 12) as usize], n / 12 - 1);
    if cents != 0 {
        let _ = write!(out, " {cents:+}");
    }
}

/// `1.2 kHz`, `85 Hz`, written into `out`.
fn hz_label(hz: f32, out: &mut String) {
    crate::views::fx::controls::write_readout(hz, "Hz", out);
}

/// What the gesture code needs from the drawing pass.
struct GestureCtx {
    hovered_band: Option<usize>,
    press: Hits,
}

/// What is at a point of the plot.
#[derive(Copy, Clone, Debug, Default)]
struct Hits {
    pos: Option<Pos2>,
    band: Option<usize>,
    handle: Option<usize>,
    on_curve: bool,
    /// A grabbable peak: its frequency and the Q its width suggests.
    peak: Option<(f32, f32)>,
}

/// What one frame of the display knows about the EQ.
struct Frame<'a> {
    bands: &'a [BandSettings; aequalis::MAX_BANDS],
    on: [bool; EQ_BANDS],
    mode: Mode,
    solo: Option<usize>,
    /// The band each dynamic band has moved by, dB.
    deltas: [f32; EQ_BANDS],
    key_on: bool,
    sr: f32,
    view: (f32, f32),
}

/// The display in `rect`: its selectors at the top, the plot, the axis under it — drawn,
/// then acted on.
#[allow(clippy::too_many_arguments)]
pub(super) fn show(
    host: &mut dyn FxHost,
    st: &mut EqEditor,
    ui: &mut egui::Ui,
    s: &Style,
    rect: Rect,
    tel: &[f32; 64],
    sr: f32,
    words: &mut Words,
) {
    let k = s.k;
    let ink = Ink::of(s);
    let bands = super::bands_of(st, host);
    let spec = host.spec();
    let mut on = [false; EQ_BANDS];
    for (i, u) in on.iter_mut().enumerate() {
        *u = used(spec, i);
    }
    let mode = Mode::from_index(spec.get("mode").round() as usize);
    let solo = {
        let s = spec.get("solo").round() as usize;
        (s >= 1).then(|| s - 1)
    };
    let deltas = if st.injected.is_some() {
        st.history.back().map(|h| h.0).unwrap_or([0.0; EQ_BANDS])
    } else {
        host.eq_deltas()
            .unwrap_or_else(|| st.history.back().map(|h| h.0).unwrap_or([0.0; EQ_BANDS]))
    };
    let head = Rect::from_min_size(rect.min, vec2(rect.width(), HEAD_H * k));
    let axis_h = if st.piano { PIANO_H } else { AXIS_H } * k;
    let plot = Rect::from_min_max(
        pos2(rect.left(), head.bottom()),
        pos2(rect.right(), rect.bottom() - axis_h),
    );
    let axis_rect = Rect::from_min_max(pos2(rect.left(), plot.bottom()), rect.max);
    let resp = ui.interact(plot, ui.id().with("eq-plot"), Sense::click_and_drag());
    let axis_resp = ui.interact(axis_rect, ui.id().with("eq-axis"), Sense::click_and_drag());
    let view = (
        st.view.0.max(VIEW_LIMITS.0),
        st.view.1.min(VIEW_LIMITS.1.min(sr * 0.5)),
    );
    let geo = Geo::new(plot, st.range_db, view);
    st.plot = Some(plot);
    st.plot_view = view;
    if st.buf.hz_view != view {
        st.buf.hz_view = view;
        for i in 0..POINTS {
            st.buf.hz[i] = geo.hz_at(i, POINTS);
        }
        // The spectra are read at new frequencies.
        st.buf.fresh = true;
    }
    let key_on = tel[aequalis::telemetry::KEY_ON] >= 0.5;
    spectra(st, tel, sr, key_on);
    totals(st, &bands, &on, mode, view, sr);
    let fr = Frame {
        bands: &bands,
        on,
        mode,
        solo,
        deltas,
        key_on,
        sr,
        view,
    };

    // ---- hit-testing for this frame ----------------------------------------------------
    let dot_r = DOT_R * k;
    let pointer = resp.hover_pos();
    let total = st.buf.total[0];
    let curve_y = |x: f32| -> f32 {
        let f = ((x - plot.left()) / plot.width() * (POINTS - 1) as f32)
            .clamp(0.0, (POINTS - 1) as f32);
        let i = f.floor() as usize;
        let t = f - i as f32;
        let a = total[i];
        let b = total[(i + 1).min(POINTS - 1)];
        geo.y(a + (b - a) * t)
    };
    let near_dot = |pt: Pos2| -> Option<usize> {
        (0..EQ_BANDS)
            .filter(|i| on[*i])
            .map(|i| (i, geo.dot(&bands[i]).distance(pt)))
            .filter(|(_, d)| *d <= dot_r + 4.0 * k)
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(i, _)| i)
    };
    let range_handle = |i: usize| -> Option<Pos2> {
        let b = &bands[i];
        (b.dynamic && b.shape.has_gain() && on[i])
            .then(|| pos2(geo.x(b.freq), geo.y(b.gain_db + b.range_db)))
    };
    let near_handle = |pt: Pos2| -> Option<usize> {
        (0..EQ_BANDS).find(|i| range_handle(*i).is_some_and(|h| h.distance(pt) <= 8.0 * k))
    };
    let floor = st.show.range_db;
    let grab_spec = if st.show.post { st.post } else { st.pre };
    st.buf.npeaks = if st.show.grab && st.has_data() {
        peaks(&grab_spec, &geo, floor, &mut st.buf.peaks)
    } else {
        0
    };
    let peak_list = st.buf.peaks;
    let npeaks = st.buf.npeaks;
    let peak_at = |i: usize| pos2(geo.x_at(i, POINTS), geo.y_spec(grab_spec[i], floor));
    let near_peak = |pt: Pos2| -> Option<usize> {
        peak_list[..npeaks]
            .iter()
            .copied()
            .find(|i| peak_at(*i).distance(pt) <= 10.0 * k)
    };
    let peak_q = |i: usize| -> (f32, f32) {
        let hz = geo.hz_at(i, POINTS);
        // Q from the peak's width: where it falls 3 dB either side.
        let v = grab_spec[i];
        let mut lo = i;
        while lo > 0 && grab_spec[lo - 1] > v - 3.0 {
            lo -= 1;
        }
        let mut hi = i;
        while hi + 1 < POINTS && grab_spec[hi + 1] > v - 3.0 {
            hi += 1;
        }
        let bw = (geo.hz_at((hi + 1).min(POINTS - 1), POINTS) / geo.hz_at(lo, POINTS)).max(1.01);
        (hz, (bw.sqrt() / (bw - 1.0)).clamp(1.0, 12.0))
    };
    let hovered_band = pointer.and_then(near_dot);
    let hovered_handle = pointer.and_then(near_handle);
    // What is under the pointer now, and what was under it where the button went down: a
    // drag is recognised a few pixels after the press, and must start from what was
    // pressed, not from where the pointer has got to.
    let hits = |pt: Option<Pos2>| Hits {
        pos: pt,
        band: pt.and_then(near_dot),
        handle: pt.and_then(near_handle),
        on_curve: pt.is_some_and(|pt| (pt.y - curve_y(pt.x)).abs() < 6.0 * k),
        peak: pt.and_then(near_peak).map(peak_q),
    };
    let press = ui
        .input(|i| i.pointer.press_origin())
        .filter(|pt| plot.contains(*pt));
    let gctx = GestureCtx {
        hovered_band,
        press: hits(press),
    };
    let ghost = pointer.filter(|_| {
        hovered_band.is_none()
            && hovered_handle.is_none()
            && st.drag.is_none()
            && st.rubber.is_none()
            && !st.show.grab
    });
    let ghost_on_curve = ghost.is_some_and(|pt| (pt.y - curve_y(pt.x)).abs() < 6.0 * k);
    let hot = Hot {
        pointer,
        band: hovered_band,
        handle: hovered_handle,
        ghost,
        ghost_on_curve,
        focused: resp.has_focus(),
    };

    let painter = ui.painter_at(Rect::from_min_max(rect.min, plot.max));
    with_brush(&painter, |b| {
        paint(
            b,
            st,
            s,
            &ink,
            &geo,
            &fr,
            &hot,
            Rect::from_min_max(rect.min, plot.max),
        );
    });
    panel::head(host, st, ui, s, head, words);
    axis(st, ui, s, &ink, axis_rect, &axis_resp, &geo, &fr);
    gestures(host, st, ui, &resp, &fr, &geo, gctx);
    menus(host, st, ui, s, &fr);
    if st.typing.is_some() {
        typing_popup(host, st, ui, s, &fr, &geo);
    }
    if resp.hovered() && st.drag.is_none() {
        crate::kit::hint::set_text(
            ui.ctx(),
            frame::tr("eq.display"),
            frame::tr("eq.display_tip"),
            crate::kit::hint::Caps {
                menu: true,
                typed: true,
                ..crate::kit::hint::Caps::NONE
            },
        );
    }
}

/// What the pointer is over, for the painter.
struct Hot {
    pointer: Option<Pos2>,
    band: Option<usize>,
    handle: Option<usize>,
    /// Where a double-click would make a band.
    ghost: Option<Pos2>,
    ghost_on_curve: bool,
    focused: bool,
}

/// Bring the spectra up to date: the analyser's (when new pairs came) and the key strip's
/// (from the unit's third octaves), and let EQ match listen.
fn spectra(st: &mut EqEditor, tel: &[f32; 64], sr: f32, key_on: bool) {
    let show = st.show;
    let now = Instant::now();
    let dt = st
        .last
        .map_or(0.0, |l| now.duration_since(l).as_secs_f32().min(0.5));
    st.last = Some(now);
    let fall = AnalyserView::FALL[show.speed.min(4)] * dt;
    let tilt_at = |hz: f32| show.tilt * (hz / 1_000.0).log2();
    let has_data = st.has_data();
    if has_data && !show.grab && st.buf.fresh {
        st.buf.fresh = false;
        if let Some(a) = st.analyser.as_mut() {
            a.compute();
            for i in 0..POINTS {
                let hz = st.buf.hz[i];
                let pa = a.level_db(0, sr, hz, 1.0 / 6.0) + tilt_at(hz);
                let pb = a.level_db(1, sr, hz, 1.0 / 6.0) + tilt_at(hz);
                if show.freeze {
                    st.pre[i] = st.pre[i].max(pa);
                    st.post[i] = st.post[i].max(pb);
                } else {
                    st.pre[i] = pa.max(st.pre[i] - fall);
                    st.post[i] = pb.max(st.post[i] - fall);
                }
            }
        }
    }
    // The key strip's spectrum, from the unit's third octaves, read at every point.
    if key_on && !show.grab {
        let mut thirds = [(0.0f32, 0.0f32); aequalis::KEY_BANDS];
        for (k, t) in thirds.iter_mut().enumerate() {
            *t = (aequalis::key_band_hz(k), tel[aequalis::telemetry::KEY + k]);
        }
        for i in 0..POINTS {
            let hz = st.buf.hz[i];
            let v = aequalis::fit::interpolate(&thirds, hz) + tilt_at(hz);
            st.key[i] = if show.freeze {
                st.key[i].max(v)
            } else {
                v.max(st.key[i] - fall)
            };
        }
    } else if !key_on && !show.freeze {
        st.key.fill(-200.0);
    }
    // EQ match listens while it is open: the input's and the key's long-term spectra.
    if let (Some(m), Some(a)) = (st.matching.as_mut(), st.analyser.as_ref()) {
        if m.listening && key_on && a.has_data() {
            for k in 0..aequalis::KEY_BANDS {
                let hz = aequalis::key_band_hz(k);
                let input = a.level_db(0, sr, hz, 1.0 / 3.0);
                let key = tel[aequalis::telemetry::KEY + k];
                m.input[k] += 10f64.powf(input as f64 / 10.0);
                m.reference[k] += 10f64.powf(key as f64 / 10.0);
            }
            m.frames += 1;
        }
    }
}

/// The summed curves: the whole image's, and each part's where bands are placed on it.
fn totals(
    st: &mut EqEditor,
    bands: &[BandSettings; aequalis::MAX_BANDS],
    on: &[bool; EQ_BANDS],
    mode: Mode,
    view: (f32, f32),
    sr: f32,
) {
    let Buf {
        hz, total, part, ..
    } = &mut st.buf;
    for t in total.iter_mut() {
        t.fill(0.0);
    }
    *part = [false; 5];
    for i in 0..EQ_BANDS {
        if !on[i] || !bands[i].active {
            continue;
        }
        let c = st.curves.get(i, &bands[i], mode, view, sr, hz);
        let p = bands[i].placement.index().min(4);
        if p == 0 {
            for t in total.iter_mut() {
                for (a, v) in t.iter_mut().zip(c) {
                    *a += v;
                }
            }
        } else {
            part[p] = true;
            for (a, v) in total[p].iter_mut().zip(c) {
                *a += v;
            }
        }
    }
}

/// Paint the display (its selectors' strip and the plot): the grid, the spectra, the
/// curves, the dots — then every word, where it touches nothing.
#[allow(clippy::too_many_arguments)]
fn paint(
    b: &mut Brush,
    st: &mut EqEditor,
    s: &Style,
    ink: &Ink,
    geo: &Geo,
    fr: &Frame<'_>,
    hot: &Hot,
    area: Rect,
) {
    let k = s.k;
    let plot = geo.rect;
    let px = b.px();
    let show = st.show;
    let floor = show.range_db;
    let bands = fr.bands;
    canvas::ground_b(b, area, ink, s.hue_tok);

    // ---- the grid: frequency lines, the EQ's dB lines ------------------------------------
    grid_lines(geo.lo, geo.hi, |hz, strong, _| {
        let c = if strong {
            ink.grid_strong
        } else {
            with_alpha(ink.grid, 0.7)
        };
        b.vline(b.snap_mid(geo.x(hz)), plot.top(), plot.bottom(), px, c);
    });
    let step = db_step(geo.range);
    let mut db = -geo.range;
    while db <= geo.range + 1e-3 {
        let c = if db.abs() < 1e-3 {
            ink.grid_strong
        } else {
            ink.grid
        };
        b.hline(plot.left(), plot.right(), b.snap_mid(geo.y(db)), px, c);
        db += step;
    }

    // ---- the audition: everything outside the band's region is shaded --------------------
    if let Some(sb) = fr.solo.filter(|s| used_at(&fr.on, *s)) {
        let (a, c) = audible_region(&bands[sb], fr.view);
        let shade = with_alpha(ink.ground, 0.72);
        let (xa, xb) = (geo.x(a).max(plot.left()), geo.x(c).min(plot.right()));
        if xa > plot.left() {
            b.rect(
                Rect::from_x_y_ranges(plot.left()..=xa, plot.y_range()),
                Fill::Solid(shade),
            );
        }
        if xb < plot.right() {
            b.rect(
                Rect::from_x_y_ranges(xb..=plot.right(), plot.y_range()),
                Fill::Solid(shade),
            );
        }
        for x in [xa, xb] {
            b.vline(b.snap_mid(x), plot.top(), plot.bottom(), px, band_color(sb));
        }
    }

    // ---- spectra --------------------------------------------------------------------------
    let has_data = st.has_data();
    let Buf {
        total,
        part,
        a: sa,
        b: sb2,
        pts,
        pts2,
        hit,
        cols,
        hz,
        ..
    } = &mut st.buf;
    let spec_pts = |v: &[f32; POINTS], out: &mut [Pos2; POINTS]| {
        for i in 0..POINTS {
            out[i] = pos2(geo.x_at(i, POINTS), geo.y_spec(v[i], floor));
        }
    };
    if show.pre && has_data {
        spec_pts(&st.pre, pts);
        // The analyser in the look's second voice (Aequalis's amber); plain, dim as ever.
        b.area(
            &pts[..],
            plot.bottom(),
            with_alpha(ink.voice(ink.dim, true), if ink.dark { 0.14 } else { 0.10 }),
        );
    }
    if show.post && has_data {
        spec_pts(&st.post, pts);
        b.polyline(
            &pts[..],
            1.25 * k,
            with_alpha(ink.voice(ink.dim, true), if ink.dark { 0.80 } else { 0.90 }),
        );
    }
    // The key strip: dashed, in the text colour — a line no band colour can be confused
    // with — smoothed over a sixth of an octave, since it is measured in third octaves.
    if show.key && fr.key_on {
        smooth(&st.key, 3, sa);
        spec_pts(sa, pts);
        dashed(b, &pts[..], 1.25 * k, ink.text, 6.0 * k, 4.0 * k);
    }
    // Collisions: where the output and the key strip are both loud, and near each other,
    // tinted and hatched in the danger colour, so it reads without colour.
    if show.collisions && fr.key_on && has_data {
        smooth(if show.post { &st.post } else { &st.pre }, 5, sa);
        smooth(&st.key, 5, sb2);
        let thresh = -floor * 0.5;
        for i in 0..POINTS {
            hit[i] = sa[i] > thresh && sb2[i] > thresh && (sa[i] - sb2[i]).abs() < 8.0;
        }
        let mut i = 0;
        while i < POINTS {
            if !hit[i] {
                i += 1;
                continue;
            }
            let start = i;
            let mut end = i;
            let mut gap = 0;
            // Gaps shorter than a sixth of an octave are bridged, so the hatching reads as
            // regions, not as noise.
            while i < POINTS && gap <= 5 {
                if hit[i] {
                    end = i;
                    gap = 0;
                } else {
                    gap += 1;
                }
                i += 1;
            }
            let n = end - start + 1;
            for j in 0..n {
                let q = start + j;
                pts[j] = pos2(geo.x_at(q, POINTS), geo.y_spec(sa[q].min(sb2[q]), floor));
            }
            b.area(&pts[..n], plot.bottom(), with_alpha(ink.danger, 0.06));
            hatch_under(
                b,
                &pts[..n],
                plot.bottom(),
                6.0 * k,
                with_alpha(ink.danger, 0.28),
                cols,
            );
            b.polyline(&pts[..n], 1.5 * k, ink.danger);
        }
    }

    // ---- bands ------------------------------------------------------------------------------
    let curve_pts = |db: &[f32], out: &mut [Pos2; POINTS]| {
        for (i, o) in out.iter_mut().enumerate() {
            *o = pos2(
                geo.x_at(i, POINTS),
                geo.y(db.get(i).copied().unwrap_or(0.0)),
            );
        }
    };
    let zero = geo.y(0.0);
    for i in 0..EQ_BANDS {
        let band = &bands[i];
        if !fr.on[i] || !band.active {
            continue;
        }
        let color = band_color(i);
        let c = st.curves.get(i, band, fr.mode, fr.view, fr.sr, hz);
        curve_pts(c, pts);
        if st.selection.contains(&i) {
            // A cut's area is most of the display: fill it faintly, or it drowns the rest.
            let fill = if band.shape.has_gain() { 0.20 } else { 0.07 };
            b.area(&pts[..], zero, with_alpha(color, fill));
        }
        b.polyline(&pts[..], k, with_alpha(color, 0.75));
        // A dynamic band: the region its range can take it to, and where it is now.
        if band.dynamic && band.shape.has_gain() {
            let moved = BandSettings {
                gain_db: band.gain_db + band.range_db,
                ..*band
            };
            let reach = st
                .curves
                .get(EQ_BANDS + i, &moved, fr.mode, fr.view, fr.sr, hz);
            curve_pts(reach, pts2);
            b.between(&pts[..], &pts2[..], with_alpha(color, 0.10));
            dashed(b, &pts2[..], k, color, 4.0 * k, 3.0 * k);
            if fr.deltas[i].abs() > 0.05 {
                let live = BandSettings {
                    gain_db: band.gain_db + fr.deltas[i],
                    ..*band
                };
                let now = st
                    .curves
                    .get(2 * EQ_BANDS + i, &live, fr.mode, fr.view, fr.sr, hz);
                curve_pts(now, pts2);
                b.polyline(&pts2[..], 2.0 * k, color);
            }
        }
    }
    // The result: the whole image's curve in the device's hue, filled from 0 dB; where a
    // part of the image (L, R, M, S) differs from it, that part's own curve in its colour.
    curve_pts(&total[0], pts);
    b.area(&pts[..], zero, ink.fill(0.5));
    b.polyline(&pts[..], 2.0 * k, ink.hue);
    let mut ends = [None; 5];
    for (p, pl) in Placement::ALL.iter().enumerate().skip(1) {
        if !part[p] {
            continue;
        }
        let (_, color) = placement_style(*pl, ink);
        curve_pts(&total[p], pts2);
        let mut run_start: Option<usize> = None;
        let mut last = None;
        for i in 0..=POINTS {
            let differs = i < POINTS && (total[p][i] - total[0][i]).abs() > 0.05;
            match (differs, run_start) {
                (true, None) => run_start = Some(i.saturating_sub(1)),
                (false, Some(s0)) => {
                    let e = i.min(POINTS - 1);
                    b.polyline(&pts2[s0..=e], 2.0 * k, color);
                    last = Some(pts2[e]);
                    run_start = None;
                }
                _ => {}
            }
        }
        ends[p] = last;
    }

    // ---- EQ match: the difference it aims at, thick, and what the bands would make of it ----
    if let Some(m) = st.matching.as_ref() {
        if let Some(target) = m.target() {
            for i in 0..POINTS {
                let f = hz[i];
                pts[i] = pos2(
                    geo.x_at(i, POINTS),
                    geo.y(aequalis::fit::interpolate(&target, f)),
                );
            }
            b.polyline(&pts[..], 3.0 * k, ink.text);
        }
        if let Some(fit) = &m.fit {
            for i in 0..POINTS {
                let f = hz[i];
                let d: f32 = fit
                    .bands
                    .iter()
                    .map(|bb| aequalis::analog_db(bb, fr.sr, f))
                    .sum();
                pts[i] = pos2(geo.x_at(i, POINTS), geo.y(d));
            }
            dashed(b, &pts[..], 2.0 * k, s.t.brand, 6.0 * k, 3.0 * k);
        }
    }

    // ---- the ghost: what a double-click here would make, dashed ----------------------------
    if let Some(pt) = hot.ghost {
        let shape = shape_at(plot, pt, hot.ghost_on_curve);
        let gain = if shape.has_gain() && !hot.ghost_on_curve {
            geo.db(pt.y).clamp(-30.0, 30.0)
        } else {
            0.0
        };
        if !shape.has_gain() || gain.abs() > 0.2 {
            let ghost = BandSettings {
                active: true,
                shape,
                freq: geo.hz(pt.x),
                gain_db: gain,
                q: if matches!(shape, EqShape::Bell | EqShape::Notch) {
                    1.0
                } else {
                    0.71
                },
                slope: 24.0,
                ..BandSettings::default()
            };
            let c = st.curves.get(GHOST, &ghost, fr.mode, fr.view, fr.sr, hz);
            curve_pts(c, pts2);
            dashed(b, &pts2[..], k, with_alpha(ink.dim, 0.8), 3.0 * k, 4.0 * k);
        }
    }
    if let Some(pt) = hot.pointer {
        b.vline(
            b.snap_mid(pt.x),
            plot.top(),
            plot.bottom(),
            px,
            with_alpha(ink.dim, 0.45),
        );
    }
    // The rubber band.
    if let Some((a, c)) = st.rubber {
        let r = Rect::from_two_pos(a, c);
        b.rect(r, Fill::Solid(with_alpha(s.t.selection, 0.18)));
        b.rrect_stroke(b.snap_rect(r), 0.0, px, s.t.focus);
    }
    let total0 = total[0];
    let (npeaks, peaks) = (st.buf.npeaks, st.buf.peaks);
    marks(
        b,
        st,
        s,
        ink,
        geo,
        fr,
        hot,
        &ends,
        &total0,
        &peaks[..npeaks],
    );
    canvas::edge_b(
        b,
        area,
        &Ink {
            edge: if hot.focused { s.t.focus } else { ink.edge },
            ..*ink
        },
    );
}

/// The EQ scale's step for a range.
fn db_step(range: f32) -> f32 {
    if range <= 3.0 {
        1.0
    } else if range <= 6.0 {
        2.0
    } else if range <= 12.0 {
        3.0
    } else {
        6.0
    }
}

/// The marks over the curves — range handles, the dots, the grab's peaks — and then every
/// word, each placed where it touches no dot and no other word.
#[allow(clippy::too_many_arguments)]
fn marks(
    b: &mut Brush,
    st: &mut EqEditor,
    s: &Style,
    ink: &Ink,
    geo: &Geo,
    fr: &Frame<'_>,
    hot: &Hot,
    ends: &[Option<Pos2>; 5],
    total: &[f32; POINTS],
    peaks: &[usize],
) {
    let k = s.k;
    let tk = s.tk();
    let plot = geo.rect;
    let bands = fr.bands;
    let r = DOT_R * k;
    let mut placer = Placer::new();
    // Every dot takes its place first: nothing is written over a dot.
    for i in 0..EQ_BANDS {
        if fr.on[i] {
            placer.add(Rect::from_center_size(
                geo.dot(&bands[i]),
                Vec2::splat(2.0 * r + 2.0 * k),
            ));
        }
    }

    // ---- range handles and the dots ------------------------------------------------------
    for i in 0..EQ_BANDS {
        if !fr.on[i] {
            continue;
        }
        let band = &bands[i];
        let c = geo.dot(band);
        let color = band_color(i);
        if band.dynamic && band.shape.has_gain() {
            let h = pos2(geo.x(band.freq), geo.y(band.gain_db + band.range_db));
            b.segment(c, h, k, with_alpha(color, 0.7), false);
            let up = band.range_db > 0.0;
            let dy = if up { -1.0 } else { 1.0 };
            let tri = [
                pos2(h.x, h.y + dy * 6.0 * k),
                pos2(h.x - 5.0 * k, h.y - dy * 3.0 * k),
                pos2(h.x + 5.0 * k, h.y - dy * 3.0 * k),
            ];
            let fill = if hot.handle == Some(i) {
                color
            } else {
                with_alpha(color, 0.8)
            };
            b.polygon(&tri, fill);
            placer.add(Rect::from_center_size(h, Vec2::splat(12.0 * k)));
            // Where the band is right now.
            if band.active && fr.deltas[i].abs() > 0.05 {
                let y = b.snap_mid(geo.y(band.gain_db + fr.deltas[i]));
                b.hline(c.x - r - 3.0 * k, c.x + r + 3.0 * k, y, 2.5 * k, color);
            }
        }
    }
    for i in 0..EQ_BANDS {
        if !fr.on[i] {
            continue;
        }
        let band = &bands[i];
        let c = geo.dot(band);
        let color = band_color(i);
        let selected = st.selection.contains(&i);
        if selected {
            crate::kit::light::glow(b, c, r * 2.2, color, 0.8);
            b.dot(c, r, color);
        } else {
            b.dot(c, r, with_alpha(ink.ground, 0.88));
            let a = if band.active { 1.0 } else { 0.45 };
            b.circle_stroke(c, r - 0.75 * k, 1.5 * k, with_alpha(color, a));
        }
        if hot.band == Some(i) && !selected {
            b.circle_stroke(c, r + 2.5 * k, k, ink.text);
        }
        let num = if selected {
            on_fill(color)
        } else if band.active {
            ensure_contrast(color, ink.ground, 3.0)
        } else {
            ink.dim
        };
        let t = Text::new(DIGITS[i], Role::Micro, num).caps(false);
        let _ = ktext::paint(
            b,
            Rect::from_center_size(c, Vec2::splat(2.0 * r + 4.0 * k)),
            &t,
            tk,
        );
    }
    // Spectrum Grab: the peaks, ready to be pulled into a band.
    for &i in peaks {
        let at = pos2(
            geo.x_at(i, POINTS),
            geo.y_spec(st.buf_peak_level(i), st.show.range_db),
        );
        let near = hot.pointer.is_some_and(|pt| at.distance(pt) <= 10.0 * k);
        b.circle_stroke(
            at,
            if near { 7.0 } else { 5.0 } * k,
            if near { 2.0 } else { 1.5 } * k,
            s.t.highlight,
        );
        placer.add(Rect::from_center_size(at, Vec2::splat(12.0 * k)));
    }

    // ---- words, most important first ------------------------------------------------------
    let plate = Some(ink.plate);
    // The readout of what is under the pointer, top right (top left when the pointer is
    // there): frequency, note, the curve's gain, and the spectra's levels.
    if let Some(pt) = hot.pointer {
        readout(b, st, s, ink, geo, fr, pt, total, &mut placer);
    }
    if st.show.grab {
        let at = pos2(plot.center().x, plot.top() + 4.0 * k);
        let t = Text::key("eq.grab_hint", Role::Body, s.t.highlight);
        let size = ktext::measure(b.ctx(), &t, tk);
        let rr = Align2::CENTER_TOP.anchor_size(at, size);
        if placer.free(rr) {
            placer.add(rr);
            canvas::put(b, at, Align2::CENTER_TOP, &t, tk, plate);
        }
    }
    // The selected band's numbers, on a plate beside its dot — never over it.
    if let Some(i) = st.primary.filter(|i| used_at(&fr.on, *i)) {
        band_plate(b, st, s, ink, geo, fr, i, &mut placer);
    }
    // L, R, M, S: each placed band's letter on a tag beside its dot.
    for i in 0..EQ_BANDS {
        if !fr.on[i] || bands[i].placement == Placement::Stereo {
            continue;
        }
        let (glyph, color) = placement_style(bands[i].placement, ink);
        let c = geo.dot(&bands[i]);
        tag(b, s, &mut placer, c, r + 2.0 * k, glyph, color, plot);
    }
    // The auditioned band says so.
    if let Some(i) = fr.solo.filter(|i| used_at(&fr.on, *i)) {
        let c = geo.dot(&bands[i]);
        let t = Text::key("eq.solo_short", Role::Micro, band_color(i));
        let size = ktext::measure(b.ctx(), &t, tk) + vec2(6.0 * k, 2.0 * k);
        let below = Rect::from_center_size(pos2(c.x, c.y + r + 3.0 * k + size.y * 0.5), size);
        let above = below.translate(vec2(0.0, -(2.0 * r + 6.0 * k + size.y)));
        if let Some(rr) = placer.place(&[below, above], plot) {
            canvas::put(b, rr.center(), Align2::CENTER_CENTER, &t, tk, plate);
        }
    }
    // Each part's curve named where it ends.
    for (p, pl) in Placement::ALL.iter().enumerate().skip(1) {
        if let Some(end) = ends[p] {
            let (glyph, color) = placement_style(*pl, ink);
            let at = pos2(end.x.min(plot.right() - 14.0 * k), end.y);
            tag(b, s, &mut placer, at, 2.0 * k, glyph, color, plot);
        }
    }
    // The grab's peaks, labelled: frequency (or note) and level.
    for &i in peaks {
        let at = pos2(
            geo.x_at(i, POINTS),
            geo.y_spec(st.buf_peak_level(i), st.show.range_db),
        );
        let hz = geo.hz_at(i, POINTS);
        let level = st.buf_peak_level(i) - st.show.tilt * (hz / 1_000.0).log2();
        let mut words = std::mem::take(&mut st.buf.s);
        words.clear();
        if st.piano {
            note_label(hz, &mut words);
        } else {
            hz_label(hz, &mut words);
        }
        let _ = write!(words, "  {level:.0} dB");
        let t = Text::new(&words, Role::Body, ink.text).caps(false);
        let size = ktext::measure(b.ctx(), &t, tk) + vec2(6.0 * k, 2.0 * k);
        let above = Rect::from_center_size(pos2(at.x, at.y - 9.0 * k - size.y * 0.5), size);
        let below = above.translate(vec2(0.0, 18.0 * k + size.y));
        if let Some(rr) = placer.place(&[above, below], plot) {
            canvas::put(b, rr.center(), Align2::CENTER_CENTER, &t, tk, plate);
        }
        st.buf.s = words;
    }
    legend(b, st, s, ink, fr, plot, &mut placer);
    scales(b, st, s, ink, geo, &mut placer);
    if (0..EQ_BANDS).all(|i| !fr.on[i]) && hot.pointer.is_none() {
        let t = Text::key("eq.empty", Role::Body, ink.dim);
        canvas::put(b, plot.center(), Align2::CENTER_CENTER, &t, tk, plate);
    }
}

impl EqEditor {
    /// The level the grab reads at point `i`: the output's, or the input's when the output
    /// is not shown.
    fn buf_peak_level(&self, i: usize) -> f32 {
        if self.show.post {
            self.post[i]
        } else {
            self.pre[i]
        }
    }
}

/// A letter on a small tag in `color`, beside `c` (at `gap` from it): the first side where
/// it touches nothing — right, left, above, below, then the diagonals.
#[allow(clippy::too_many_arguments)]
fn tag(
    b: &mut Brush,
    s: &Style,
    placer: &mut Placer,
    c: Pos2,
    gap: f32,
    glyph: &str,
    color: Color32,
    within: Rect,
) {
    let k = s.k;
    let side = 12.0 * k;
    let d = gap + side * 0.5 + k;
    let at = |dx: f32, dy: f32| Rect::from_center_size(c + vec2(dx, dy), Vec2::splat(side));
    let options = [
        at(d, 0.0),
        at(-d, 0.0),
        at(0.0, -d),
        at(0.0, d),
        at(d * 0.8, -d * 0.8),
        at(-d * 0.8, -d * 0.8),
        at(d * 0.8, d * 0.8),
        at(-d * 0.8, d * 0.8),
    ];
    let Some(rr) = placer.place(&options, within) else {
        return;
    };
    b.rrect(b.snap_rect(rr), s.r(3.0), Fill::Solid(color));
    let t = Text::new(glyph, Role::Micro, on_fill(color)).caps(false);
    let _ = ktext::paint(b, rr, &t, s.tk());
}

/// The selected band's numbers on a plate: "Band 5 · Bell · Dynamic" over
/// "6.2 kHz  +0.0 dB  Q 3.00", beside its dot where it covers nothing.
#[allow(clippy::too_many_arguments)]
fn band_plate(
    b: &mut Brush,
    st: &mut EqEditor,
    s: &Style,
    ink: &Ink,
    geo: &Geo,
    fr: &Frame<'_>,
    i: usize,
    placer: &mut Placer,
) {
    let k = s.k;
    let tk = s.tk();
    let band = &fr.bands[i];
    let c = geo.dot(band);
    let mut l1 = std::mem::take(&mut st.buf.s);
    l1.clear();
    let _ = write!(
        l1,
        "{} {}  {}",
        frame::tr("eq.band"),
        DIGITS[i],
        frame::tr(SHAPE_KEYS[band.shape.index().min(8)])
    );
    if band.placement != Placement::Stereo {
        let _ = write!(
            l1,
            " \u{b7} {}",
            frame::tr(PLACE_KEYS[band.placement.index().min(4)])
        );
    }
    if band.dynamic && band.shape.has_gain() {
        let _ = write!(l1, " \u{b7} {}", frame::tr("fx.eq.dyn"));
    }
    if fr.mode == Mode::LinearPhase && band.shape == EqShape::AllPass {
        let _ = write!(l1, " \u{b7} {}", frame::tr("eq.allpass_lp"));
    }
    let split = l1.len();
    hz_label(band.freq, &mut l1);
    if band.shape.has_gain() {
        let _ = write!(l1, "  {:+.1} dB", band.gain_db);
    }
    if band.shape.has_slope() {
        let _ = write!(l1, "  {:.0} dB/oct", band.slope);
    } else {
        let _ = write!(l1, "  Q {:.2}", band.q);
    }
    let (w1, w2) = l1.split_at(split);
    let t1 = Text::new(w1, Role::Micro, band_color(i)).caps(false);
    let t2 = Text::new(w2, Role::Body, ink.text).caps(false);
    let s1 = ktext::measure(b.ctx(), &t1, tk);
    let s2 = ktext::measure(b.ctx(), &t2, tk);
    let pad = vec2(6.0 * k, 3.0 * k);
    let size = vec2(s1.x.max(s2.x), s1.y + s2.y) + 2.0 * pad;
    let off = DOT_R * k + 8.0 * k;
    let right_up = Rect::from_min_size(pos2(c.x + off, c.y - off - size.y), size);
    let left_up = Rect::from_min_size(pos2(c.x - off - size.x, c.y - off - size.y), size);
    let right_down = Rect::from_min_size(pos2(c.x + off, c.y + off), size);
    let left_down = Rect::from_min_size(pos2(c.x - off - size.x, c.y + off), size);
    let right = Rect::from_min_size(pos2(c.x + off, c.y - size.y * 0.5), size);
    let left = Rect::from_min_size(pos2(c.x - off - size.x, c.y - size.y * 0.5), size);
    let within = geo.rect.shrink(2.0 * k);
    if let Some(rr) = placer.place(
        &[right_up, left_up, right_down, left_down, right, left],
        within,
    ) {
        b.rrect(b.snap_rect(rr), s.r(4.0), Fill::Solid(ink.plate));
        b.rrect_stroke(b.snap_rect(rr), s.r(4.0), b.px(), band_color(i));
        let inner = rr.shrink2(pad);
        let r1 = Rect::from_min_size(inner.min, vec2(inner.width(), s1.y));
        let r2 = Rect::from_min_size(pos2(inner.left(), r1.bottom()), vec2(inner.width(), s2.y));
        let _ = ktext::paint(b, r1, &t1.align(Align2::LEFT_CENTER), tk);
        let _ = ktext::paint(b, r2, &t2.align(Align2::LEFT_CENTER), tk);
    }
    st.buf.s = l1;
}

/// The value tag: what is under the pointer — frequency, note, the curve's gain there,
/// the spectra's levels — on a plate at the plot's top right, or its top left when the
/// pointer is under the right one.
#[allow(clippy::too_many_arguments)]
fn readout(
    b: &mut Brush,
    st: &mut EqEditor,
    s: &Style,
    ink: &Ink,
    geo: &Geo,
    fr: &Frame<'_>,
    pt: Pos2,
    total: &[f32; POINTS],
    placer: &mut Placer,
) {
    let k = s.k;
    let tk = s.tk();
    let plot = geo.rect;
    let hz = geo.hz(pt.x);
    let i = (((pt.x - plot.left()) / plot.width()) * (POINTS - 1) as f32)
        .round()
        .clamp(0.0, (POINTS - 1) as f32) as usize;
    let untilt = |v: f32| v - st.show.tilt * (hz / 1_000.0).log2();
    let mut w = std::mem::take(&mut st.buf.s);
    w.clear();
    hz_label(hz, &mut w);
    w.push_str("  \u{b7}  ");
    note_label(hz, &mut w);
    let _ = write!(
        w,
        "  \u{b7}  {} {:+.1} dB",
        frame::tr("eq.readout.eq"),
        total[i]
    );
    let split = w.len();
    if st.has_data() {
        let _ = write!(
            w,
            "{} {:.0}  {} {:.0} dB",
            frame::tr("eq.an.pre"),
            untilt(st.pre[i]),
            frame::tr("eq.an.post"),
            untilt(st.post[i])
        );
    }
    if fr.key_on {
        if w.len() > split {
            w.push_str("  \u{b7}  ");
        }
        let _ = write!(w, "{} {:.0} dB", frame::tr("eq.an.key"), untilt(st.key[i]));
    }
    let (a, c) = w.split_at(split);
    let t1 = Text::new(a, Role::Body, ink.text).caps(false);
    let t2 = Text::new(c, Role::Body, ink.dim).caps(false);
    let s1 = ktext::measure(b.ctx(), &t1, tk);
    let s2 = if c.is_empty() {
        Vec2::ZERO
    } else {
        ktext::measure(b.ctx(), &t2, tk)
    };
    let pad = vec2(6.0 * k, 3.0 * k);
    let size = vec2(s1.x.max(s2.x), s1.y + s2.y) + 2.0 * pad;
    let size = vec2(size.x.min(plot.width() - 8.0 * k), size.y);
    let right = Rect::from_min_size(
        pos2(plot.right() - 36.0 * k - size.x, plot.top() + 4.0 * k),
        size,
    );
    let left = Rect::from_min_size(pos2(plot.left() + 36.0 * k, plot.top() + 4.0 * k), size);
    let rr = if right.expand(8.0 * k).contains(pt) {
        left
    } else {
        right
    };
    placer.add(rr);
    b.rrect(b.snap_rect(rr), s.r(4.0), Fill::Solid(ink.plate));
    let inner = rr.shrink2(pad);
    let r1 = Rect::from_min_size(inner.min, vec2(inner.width(), s1.y));
    let _ = ktext::paint(
        b,
        r1,
        &t1.align(Align2::LEFT_CENTER)
            .policy(Policy::Elide(Elide::End)),
        tk,
    );
    if !c.is_empty() {
        let r2 = Rect::from_min_size(pos2(inner.left(), r1.bottom()), vec2(inner.width(), s2.y));
        let _ = ktext::paint(
            b,
            r2,
            &t2.align(Align2::LEFT_CENTER)
                .policy(Policy::Elide(Elide::End)),
            tk,
        );
    }
    st.buf.s = w;
}

/// What the lines are, at the plot's bottom right, so they are never a guess: the input's
/// fill, the output's line, the key's dashes — and "Frozen" while the analyser holds.
fn legend(
    b: &mut Brush,
    st: &EqEditor,
    s: &Style,
    ink: &Ink,
    fr: &Frame<'_>,
    plot: Rect,
    placer: &mut Placer,
) {
    let k = s.k;
    let tk = s.tk();
    let show = &st.show;
    let items: [(&str, bool, u8); 4] = [
        ("eq.an.frozen", show.freeze, 3),
        ("eq.an.key", show.key && fr.key_on, 2),
        ("eq.an.post", show.post && st.has_data(), 1),
        ("eq.an.pre", show.pre && st.has_data(), 0),
    ];
    // Measure the whole row first: it is painted only where it touches nothing.
    let sw = 16.0 * k;
    let gap = 6.0 * k;
    let mut w = 0.0;
    let mut n = 0;
    for (key, on, _) in items {
        if on {
            let t = Text::key(key, Role::Micro, ink.dim);
            w += ktext::measure(b.ctx(), &t, tk).x + sw + gap * 2.0;
            n += 1;
        }
    }
    if n == 0 {
        return;
    }
    let h = 14.0 * k;
    let row = Rect::from_min_size(
        pos2(plot.right() - 8.0 * k - w, plot.bottom() - 4.0 * k - h),
        vec2(w, h),
    );
    if !placer.free(row) {
        return;
    }
    placer.add(row);
    b.rrect(
        b.snap_rect(row.expand(2.0 * k)),
        s.r(3.0),
        Fill::Solid(ink.plate),
    );
    let mut x = row.right();
    let y = row.center().y;
    for (key, on, kind) in items {
        if !on {
            continue;
        }
        let t = Text::key(key, Role::Micro, ink.dim);
        let tw = ktext::measure(b.ctx(), &t, tk).x;
        let _ = ktext::paint(
            b,
            Rect::from_min_max(pos2(x - tw - 1.0, row.top()), pos2(x, row.bottom())),
            &t.align(Align2::RIGHT_CENTER),
            tk,
        );
        x -= tw + gap;
        let a = pos2(x - sw, y);
        let c = pos2(x, y);
        match kind {
            0 => b.rect(
                Rect::from_min_max(a - vec2(0.0, 4.0 * k), c + vec2(0.0, 4.0 * k)),
                Fill::Solid(with_alpha(ink.voice(ink.dim, true), 0.3)),
            ),
            1 => b.segment(a, c, 1.5 * k, ink.voice(ink.dim, true), false),
            2 => dashed(b, &[a, c], 1.5 * k, ink.text, 4.0 * k, 2.0 * k),
            _ => b.dot(pos2(x - sw * 0.5, y), 3.0 * k, s.t.highlight),
        }
        x -= sw + gap;
    }
}

/// The scales, where they touch nothing: the EQ's dB at the left, the analyser's at the
/// right, and the names of the frequency regions along the top.
fn scales(b: &mut Brush, st: &EqEditor, s: &Style, ink: &Ink, geo: &Geo, placer: &mut Placer) {
    let k = s.k;
    let tk = s.tk();
    let plot = geo.rect;
    let floor = st.show.range_db;
    // Each label sits just above its line (the top one just under it), and is left out
    // where it would land on a dot or a word.
    let step = db_step(geo.range);
    let mut db = -geo.range;
    let mut words = [0u8; 8];
    while db <= geo.range + 1e-3 {
        let y = geo.y(db);
        let n = db_word(db, &mut words);
        let text = std::str::from_utf8(&words[..n]).unwrap_or("");
        let t = Text::new(text, Role::Micro, ink.dim).caps(false);
        let size = ktext::measure(b.ctx(), &t, tk);
        let top = if y - size.y - 1.0 < plot.top() {
            y + 2.0
        } else {
            y - size.y - 1.0
        };
        let rr = Rect::from_min_size(pos2(plot.left() + 4.0 * k, top), size);
        if placer.free(rr) {
            placer.add(rr);
            let _ = ktext::paint(b, rr.expand(1.0), &t, tk);
        }
        db += step;
    }
    let mut a = -20.0;
    while a > -floor + 1.0 {
        let y = geo.y_spec(a, floor);
        let n = db_word(a, &mut words);
        let text = std::str::from_utf8(&words[..n]).unwrap_or("");
        let t = Text::new(text, Role::Micro, with_alpha(ink.dim, 0.8)).caps(false);
        let size = ktext::measure(b.ctx(), &t, tk);
        let rr = Rect::from_min_size(
            pos2(plot.right() - 4.0 * k - size.x, y - size.y - 1.0),
            size,
        );
        if placer.free(rr) {
            placer.add(rr);
            let _ = ktext::paint(b, rr.expand(1.0), &t, tk);
        }
        a -= 20.0;
    }
    // The regions of the spectrum, named (FL's band-name ruler).
    const REGIONS: [(&str, f32, f32); 7] = [
        ("kit.eq.sub", 20.0, 60.0),
        ("kit.eq.bass", 60.0, 250.0),
        ("kit.eq.low_mid", 250.0, 500.0),
        ("kit.eq.mid", 500.0, 2_000.0),
        ("kit.eq.high_mid", 2_000.0, 4_000.0),
        ("kit.eq.presence", 4_000.0, 6_000.0),
        ("kit.eq.treble", 6_000.0, 20_000.0),
    ];
    let mut ruler = canvas::Ruler::new();
    for (key, lo, hi) in REGIONS {
        let x = geo.x((lo * hi).sqrt());
        let t = Text::key(key, Role::Micro, with_alpha(ink.dim, 0.85));
        let size = ktext::measure(b.ctx(), &t, tk);
        let rr = Rect::from_center_size(pos2(x, plot.top() + 4.0 * k + size.y * 0.5), size);
        if !plot.shrink2(vec2(30.0 * k, 0.0)).contains_rect(rr)
            || !ruler.fits(rr.left(), 8.0 * k)
            || !placer.free(rr)
        {
            continue;
        }
        placer.add(rr);
        ruler.placed(rr);
        let _ = ktext::paint(b, rr.expand(1.0), &t, tk);
    }
}

/// A dB number as the scales print it — `+12`, `0`, `−12` — into a small stack buffer.
fn db_word(db: f32, out: &mut [u8; 8]) -> usize {
    let v = db.round() as i32;
    let mut cur = std::io::Cursor::new(&mut out[..]);
    let _ = if v > 0 {
        std::io::Write::write_fmt(&mut cur, format_args!("+{v}"))
    } else if v < 0 {
        std::io::Write::write_fmt(&mut cur, format_args!("\u{2212}{}", -v))
    } else {
        std::io::Write::write_fmt(&mut cur, format_args!("0"))
    };
    cur.position() as usize
}

/// The frequency axis under the plot — a ruler, or a piano — and its gestures: drag up
/// or down to zoom around the pointer, sideways to pan, double-click to reset, the wheel
/// to zoom.
#[allow(clippy::too_many_arguments)]
fn axis(
    st: &mut EqEditor,
    ui: &mut egui::Ui,
    s: &Style,
    ink: &Ink,
    rect: Rect,
    resp: &egui::Response,
    geo: &Geo,
    fr: &Frame<'_>,
) {
    let k = s.k;
    let tk = s.tk();
    let t = &s.t;
    let axis_geo = Geo { rect, ..*geo };
    let hover = resp.hover_pos();
    let piano = st.piano;
    let mut words = std::mem::take(&mut st.buf.s);
    with_brush(&ui.painter_at(rect), |b| {
        b.register(
            rect,
            "eq_axis",
            &[
                crate::kit::Tok::Panel,
                crate::kit::Tok::KeyWhite,
                crate::kit::Tok::KeyBlack,
                crate::kit::Tok::TextDim,
            ],
            &[s.hue_tok],
        );
        let px = b.px();
        if piano {
            // A0 to C8, then as far as the view goes.
            let lo_note = parse::note_of(geo.lo).0 - 1;
            let hi_note = parse::note_of(geo.hi).0 + 1;
            let key_x = |n: i32, cents: f32| axis_geo.x(parse::note_hz(n as f32, cents));
            b.rect(b.snap_rect(rect), Fill::Solid(t.key_black));
            for n in lo_note..=hi_note {
                if matches!(n.rem_euclid(12), 1 | 3 | 6 | 8 | 10) {
                    continue;
                }
                let r = Rect::from_x_y_ranges(
                    key_x(n, -50.0).max(rect.left())..=key_x(n, 50.0).min(rect.right()),
                    rect.y_range(),
                );
                if r.width() <= 0.0 {
                    continue;
                }
                b.rect(
                    r,
                    Fill::V(t.key_white, crate::kit::color::shade(t.key_white, -0.06)),
                );
                b.vline(
                    b.snap_mid(r.left()),
                    rect.top(),
                    rect.bottom(),
                    px,
                    t.key_black,
                );
            }
            for n in lo_note..=hi_note {
                if matches!(n.rem_euclid(12), 1 | 3 | 6 | 8 | 10) {
                    let r = Rect::from_x_y_ranges(
                        key_x(n, -40.0)..=key_x(n, 40.0),
                        rect.top()..=rect.top() + rect.height() * 0.6,
                    );
                    b.rect(r, Fill::Solid(t.key_black));
                }
            }
            // The Cs, named, where there is room.
            let mut ruler = canvas::Ruler::new();
            for n in lo_note..=hi_note {
                if n.rem_euclid(12) != 0 {
                    continue;
                }
                let x = key_x(n, 0.0);
                let at = pos2(x, rect.bottom() - 2.0 * k);
                if x < rect.left() + 6.0 * k
                    || x > rect.right() - 6.0 * k
                    || !ruler.fits(x - 8.0 * k, 4.0 * k)
                {
                    continue;
                }
                let r = canvas::putf(
                    b,
                    at,
                    Align2::CENTER_BOTTOM,
                    Role::Micro,
                    ensure_contrast(t.label, t.key_white, 4.5),
                    tk,
                    None,
                    format_args!("C{}", n / 12 - 1),
                );
                ruler.placed(r);
            }
        } else {
            b.rect(
                b.snap_rect(rect),
                Fill::V(t.panel, crate::kit::color::shade(t.panel, -0.02)),
            );
            let mut ruler = canvas::Ruler::new();
            grid_lines(geo.lo, geo.hi, |hz, strong, labelled| {
                let x = b.snap_mid(axis_geo.x(hz));
                b.vline(
                    x,
                    rect.top(),
                    rect.top() + if strong { 6.0 } else { 3.0 } * k,
                    px,
                    ink.grid_strong,
                );
                if labelled {
                    axis_label(hz, &mut words);
                    let tt = Text::new(&words, Role::Micro, t.text_dim).caps(false);
                    let w = ktext::measure(b.ctx(), &tt, tk).x;
                    if ruler.fits(x - w * 0.5, 6.0 * k)
                        && x - w * 0.5 > rect.left()
                        && x + w * 0.5 < rect.right()
                    {
                        let r = canvas::put(
                            b,
                            pos2(x, rect.bottom() - 2.0 * k),
                            Align2::CENTER_BOTTOM,
                            &tt,
                            tk,
                            None,
                        );
                        ruler.placed(r);
                    }
                }
            });
        }
        // Where each band sits on the axis.
        for i in 0..EQ_BANDS {
            if !fr.on[i] {
                continue;
            }
            let x = axis_geo.x(fr.bands[i].freq);
            if x < rect.left() || x > rect.right() {
                continue;
            }
            let tri = [
                pos2(x, rect.top() + 7.0 * k),
                pos2(x - 4.0 * k, rect.top()),
                pos2(x + 4.0 * k, rect.top()),
            ];
            b.polygon(&tri, band_color(i));
        }
        if let Some(pt) = hover {
            let hz = axis_geo.hz(pt.x);
            words.clear();
            if piano {
                note_label(hz, &mut words);
                words.push_str("  ");
                hz_label(hz, &mut words);
            } else {
                hz_label(hz, &mut words);
                words.push_str("  ");
                note_label(hz, &mut words);
            }
            let tt = Text::new(&words, Role::Body, t.text).caps(false);
            let size = ktext::measure(b.ctx(), &tt, tk) + vec2(10.0 * k, 2.0 * k);
            let size = vec2(size.x, size.y.min(rect.height()));
            let cx =
                pt.x.clamp(rect.left() + size.x * 0.5, rect.right() - size.x * 0.5);
            let r = Rect::from_center_size(pos2(cx, rect.center().y), size);
            b.rrect(b.snap_rect(r), s.r(3.0), Fill::Solid(t.panel_raised));
            b.rrect_stroke(b.snap_rect(r), s.r(3.0), px, s.t.focus);
            let _ = ktext::paint(b, r, &tt, tk);
        }
    });
    st.buf.s = words;
    if hover.is_some() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeVertical);
        crate::kit::hint::set_text(
            ui.ctx(),
            frame::tr("eq.axis"),
            frame::tr("eq.axis_tip"),
            crate::kit::hint::Caps::NONE,
        );
    }
    // Zoom and pan.
    let clamp_view = |lo: f32, hi: f32| -> (f32, f32) {
        let (min, max) = VIEW_LIMITS;
        let (mut lo, mut hi) = (lo, hi);
        if hi / lo < 2.0 {
            let mid = (lo * hi).sqrt();
            lo = mid / 2f32.sqrt();
            hi = mid * 2f32.sqrt();
        }
        if lo < min {
            hi *= min / lo;
            lo = min;
        }
        if hi > max {
            lo /= hi / max;
            hi = max;
        }
        (lo.max(min), hi.min(max))
    };
    if resp.drag_started() {
        if let Some(pt) = resp.interact_pointer_pos() {
            st.axis = Some(AxisDrag {
                anchor: axis_geo.hz(pt.x),
                view: st.view,
                acc: Vec2::ZERO,
            });
        }
    }
    if let Some(a) = st.axis.as_mut() {
        if resp.dragged() {
            a.acc += resp.drag_delta();
            let zoom = 2f32.powf(a.acc.y / 120.0);
            let (lo, hi) = a.view;
            let span = (hi / lo).powf(zoom);
            let frac = (a.anchor / lo).ln() / (hi / lo).ln();
            let new_lo = a.anchor / span.powf(frac);
            let shift = span.powf(-a.acc.x / rect.width().max(1.0));
            st.view = clamp_view(new_lo * shift, new_lo * span * shift);
        }
    }
    if resp.drag_stopped() {
        st.axis = None;
    }
    if resp.double_clicked() {
        st.view = VIEW;
    }
    if resp.hovered() {
        let scroll = ui.input(|i| i.smooth_scroll_delta.y);
        if scroll != 0.0 {
            if let Some(pt) = resp.hover_pos() {
                let anchor = axis_geo.hz(pt.x);
                let (lo, hi) = st.view;
                let zoom = 2f32.powf(-scroll / 240.0);
                let span = (hi / lo).powf(zoom);
                let frac = (anchor / lo).ln() / (hi / lo).ln();
                let new_lo = anchor / span.powf(frac);
                st.view = clamp_view(new_lo, new_lo * span);
            }
        }
    }
}

/// Wheel notches this frame, with the modifiers held.
fn wheel(ui: &egui::Ui) -> (f32, egui::Modifiers) {
    ui.input(|i| {
        let mut notches = 0.0;
        let mut mods = i.modifiers;
        for e in &i.raw.events {
            if let egui::Event::MouseWheel {
                unit,
                delta,
                modifiers,
                ..
            } = e
            {
                let d = if delta.y != 0.0 { delta.y } else { delta.x };
                notches += match unit {
                    egui::MouseWheelUnit::Point => d / 40.0,
                    egui::MouseWheelUnit::Line => d,
                    egui::MouseWheelUnit::Page => d * 8.0,
                };
                mods = *modifiers;
            }
        }
        (notches, mods)
    })
}

/// The gesture a burst of wheel turns belongs to: turns within half a second are one
/// undo step.
fn burst(host: &mut dyn FxHost, st: &mut EqEditor) -> u64 {
    let now = Instant::now();
    let fresh = st
        .burst_at
        .is_none_or(|t| now.duration_since(t).as_millis() > 500);
    st.burst_at = Some(now);
    if fresh {
        host.end_gesture();
        st.next_gesture();
    }
    st.gesture
}

fn gestures(
    host: &mut dyn FxHost,
    st: &mut EqEditor,
    ui: &mut egui::Ui,
    resp: &egui::Response,
    fr: &Frame<'_>,
    geo: &Geo,
    g: GestureCtx,
) {
    let mods = ui.input(|i| i.modifiers);
    let fine = mods.shift;
    let rect = geo.rect;
    let bands = fr.bands;

    if resp.drag_started() || resp.clicked() || resp.double_clicked() || resp.secondary_clicked() {
        resp.request_focus();
    }

    // ---- press and drag ------------------------------------------------------------------
    if resp.drag_started() {
        let origin = g
            .press
            .pos
            .or(resp.interact_pointer_pos())
            .unwrap_or_default();
        // The pixels moved before the drag was recognised count too.
        let lead = resp
            .interact_pointer_pos()
            .map_or(Vec2::ZERO, |pt| pt - origin - resp.drag_delta());
        let gesture = st.next_gesture();
        let spec = host.spec();
        if let Some(band) = g.press.handle {
            let start = spec.get(bkey(band, "range"));
            st.drag = Some(Drag {
                kind: DragKind::Range { band, start },
                gesture,
                acc: lead,
                lock: None,
            });
        } else if let Some(band) = g.press.band {
            if !st.selection.contains(&band) {
                if !(mods.command || mods.shift) {
                    st.selection.clear();
                }
                st.selection.insert(band);
            }
            st.primary = Some(band);
            let start = st
                .selection
                .iter()
                .map(|b| {
                    (
                        *b,
                        spec.get(bkey(*b, "freq")),
                        spec.get(bkey(*b, "gain")),
                        spec.get(bkey(*b, "q")),
                    )
                })
                .collect();
            st.drag = Some(Drag {
                kind: DragKind::Bands {
                    start,
                    q_mode: mods.command,
                },
                gesture,
                acc: lead,
                lock: None,
            });
        } else if let Some((hz, q)) = g.press.peak {
            // Spectrum Grab: a bell at the peak, pulled from 0 dB.
            if let Some(band) =
                add_band(host, st, EqShape::Bell, hz, 0.0, &[("q", q)], Some(gesture))
            {
                st.drag = Some(Drag {
                    kind: DragKind::Bands {
                        start: vec![(band, hz, 0.0, q)],
                        q_mode: false,
                    },
                    gesture,
                    acc: lead,
                    lock: None,
                });
            }
        } else if g.press.on_curve {
            // Pull the curve: a band where it was pressed, gain following the pointer.
            let shape = shape_at(rect, origin, true);
            let hz = geo.hz(origin.x);
            let extra: &[(&'static str, f32)] = if mods.alt {
                &[("dyn", 1.0), ("range", -6.0)]
            } else {
                &[]
            };
            if let Some(band) = add_band(host, st, shape, hz, 0.0, extra, Some(gesture)) {
                let q = if shape == EqShape::Bell { 1.0 } else { 0.71 };
                st.drag = Some(Drag {
                    kind: DragKind::Bands {
                        start: vec![(band, hz, 0.0, q)],
                        q_mode: false,
                    },
                    gesture,
                    acc: lead,
                    lock: None,
                });
            }
        } else {
            st.rubber = Some((origin, origin));
        }
    }
    if resp.dragged() {
        let delta = resp.drag_delta() * if fine { 0.2 } else { 1.0 };
        if let Some((a, _)) = st.rubber {
            if let Some(pt) = resp.interact_pointer_pos() {
                st.rubber = Some((a, pt));
            }
        }
        if let Some(d) = st.drag.as_mut() {
            d.acc += delta;
            if mods.alt && d.lock.is_none() && d.acc.length() > 4.0 {
                d.lock = Some(d.acc.x.abs() >= d.acc.y.abs());
            }
            let (dx, dy) = match d.lock {
                Some(true) => (d.acc.x, 0.0),
                Some(false) => (0.0, d.acc.y),
                None => (d.acc.x, d.acc.y),
            };
            let mut edits: Vec<(&str, Option<f32>)> = Vec::new();
            let mut widest = 0.0f32;
            match &d.kind {
                DragKind::Bands { start, q_mode } => {
                    for &(band, f0, g0, q0) in start {
                        let shape = bands[band].shape;
                        let mut f = geo.hz(geo.x(f0) + dx).clamp(10.0, 30_000.0);
                        if st.piano && !fine {
                            let (n, _) = parse::note_of(f);
                            f = parse::note_hz(n as f32, 0.0);
                        }
                        edits.push((bkey(band, "freq"), Some(f)));
                        if *q_mode {
                            let q = (q0 * 2f32.powf(-dy / 60.0)).clamp(0.025, 40.0);
                            edits.push((bkey(band, "q"), Some(q)));
                        } else if shape.has_gain() {
                            let gain =
                                (g0 - dy / (rect.height() * 0.5) * geo.range).clamp(-30.0, 30.0);
                            widest = widest.max(gain.abs());
                            edits.push((bkey(band, "gain"), Some(gain)));
                        }
                    }
                }
                DragKind::Range { band, start } => {
                    let range = (start - dy / (rect.height() * 0.5) * geo.range).clamp(-30.0, 30.0);
                    widest = widest.max((bands[*band].gain_db + range).abs());
                    edits.push((bkey(*band, "range"), Some(range)));
                }
            }
            let gesture = d.gesture;
            // The dB range grows to keep a dragged band on the display.
            st.range_db = st.range_for(widest);
            host.apply(&edits, "cmd.eq_band", Some(gesture));
        }
    }
    if resp.drag_stopped() {
        if let Some((a, c)) = st.rubber.take() {
            let r = Rect::from_two_pos(a, c);
            if !(mods.command || mods.shift) {
                st.selection.clear();
            }
            let mut first = None;
            for i in 0..EQ_BANDS {
                if fr.on[i] && r.contains(geo.dot(&bands[i])) {
                    st.selection.insert(i);
                    first = first.or(Some(i));
                }
            }
            st.primary = first.or(st.primary);
        }
        if st.drag.take().is_some() {
            host.end_gesture();
        }
    }

    // ---- clicks ----------------------------------------------------------------------------
    if resp.clicked() {
        match g.hovered_band {
            Some(b) if mods.alt => {
                let on = host.spec().get(bkey(b, "on")) >= 0.5;
                set_band(host, b, &[("on", if on { 0.0 } else { 1.0 })], None);
            }
            Some(b) => {
                if mods.command {
                    if !st.selection.remove(&b) {
                        st.selection.insert(b);
                    }
                } else if mods.shift {
                    // A consecutive range of band numbers, from the primary to this one.
                    let from = st.primary.unwrap_or(b);
                    let (lo, hi) = (from.min(b), from.max(b));
                    st.selection.extend((lo..=hi).filter(|i| fr.on[*i]));
                } else {
                    st.selection.clear();
                    st.selection.insert(b);
                }
                st.primary = Some(b);
            }
            None => {
                st.selection.clear();
                st.primary = None;
            }
        }
    }
    if resp.double_clicked() {
        match g.hovered_band {
            Some(b) => open_typing(st, b, &bands[b]),
            None => {
                if let Some(pt) = resp.interact_pointer_pos() {
                    let shape = shape_at(rect, pt, false);
                    let gain = if shape.has_gain() {
                        geo.db(pt.y).clamp(-30.0, 30.0)
                    } else {
                        0.0
                    };
                    let extra: &[(&'static str, f32)] = if mods.alt {
                        &[("dyn", 1.0), ("range", -6.0)]
                    } else {
                        &[]
                    };
                    add_band(host, st, shape, geo.hz(pt.x), gain, extra, None);
                }
            }
        }
    }

    // ---- the wheel: Q (slope on a cut), Ctrl for gain, Alt for the dynamic range ------------
    if resp.hovered() {
        let mut target = [usize::MAX; EQ_BANDS];
        let mut n = 0;
        match (&st.drag, g.hovered_band) {
            (
                Some(Drag {
                    kind: DragKind::Bands { start, .. },
                    ..
                }),
                _,
            ) => {
                for s0 in start.iter().take(EQ_BANDS) {
                    target[n] = s0.0;
                    n += 1;
                }
            }
            (_, Some(b)) => {
                target[0] = b;
                n = 1;
            }
            _ => {}
        }
        let (notches, wmods) = if n > 0 { wheel(ui) } else { (0.0, mods) };
        if notches != 0.0 {
            let what = if wmods.command {
                "gain"
            } else if wmods.alt {
                "range"
            } else {
                "q"
            };
            let gesture = burst(host, st);
            let step = if wmods.shift { 0.2 } else { 1.0 };
            let spec = host.spec();
            let mut edits: Vec<(&str, Option<f32>)> = Vec::new();
            for &b in &target[..n] {
                let band = &bands[b];
                match what {
                    "gain" if band.shape.has_gain() => edits.push((
                        bkey(b, "gain"),
                        Some((spec.get(bkey(b, "gain")) + 0.5 * notches * step).clamp(-30.0, 30.0)),
                    )),
                    "range" if band.shape.has_gain() => {
                        edits.push((bkey(b, "dyn"), Some(1.0)));
                        edits.push((
                            bkey(b, "range"),
                            Some(
                                (spec.get(bkey(b, "range")) + 0.5 * notches * step)
                                    .clamp(-30.0, 30.0),
                            ),
                        ));
                    }
                    "q" if band.shape.has_slope() => {
                        let s0 = spec.get(bkey(b, "slope"));
                        edits.push((
                            bkey(b, "slope"),
                            Some((s0 + 6.0 * notches.signum()).clamp(6.0, 96.0)),
                        ));
                    }
                    "q" => edits.push((
                        bkey(b, "q"),
                        Some(
                            (spec.get(bkey(b, "q")) * 2f32.powf(notches * step / 6.0))
                                .clamp(0.025, 40.0),
                        ),
                    )),
                    _ => {}
                }
            }
            host.apply(&edits, "cmd.eq_band", Some(gesture));
        }
    }

    // ---- right-click: the band's menu, or the display's ------------------------------------
    if resp.secondary_clicked() {
        if let Some(at) = resp.interact_pointer_pos() {
            let kind = match g.hovered_band.or(st.primary).filter(|b| fr.on[*b]) {
                Some(b) if g.hovered_band.is_some() => MenuKind::Band(b),
                _ => MenuKind::Background,
            };
            if let MenuKind::Band(b) = kind {
                if !st.selection.contains(&b) {
                    st.select(&[b]);
                }
                st.primary = Some(b);
            }
            st.menu = Some(Menu { kind, at });
        }
    }

    // ---- keyboard ------------------------------------------------------------------------------
    if resp.has_focus() && st.typing.is_none() {
        ui.memory_mut(|m| {
            m.set_focus_lock_filter(
                resp.id,
                egui::EventFilter {
                    tab: true,
                    horizontal_arrows: true,
                    vertical_arrows: true,
                    escape: true,
                },
            )
        });
        keyboard(host, st, ui, fr);
    }
}

fn open_typing(st: &mut EqEditor, band: usize, b: &BandSettings) {
    st.typing = Some(Typing {
        band,
        fields: [
            format!("{:.1}", b.freq),
            format!("{:.1}", b.gain_db),
            format!("{:.2}", b.q),
        ],
        focus: 0,
        fresh: true,
    });
    st.select(&[band]);
}

/// Keys, while the display has focus. Numbers select, arrows nudge, brackets change Q,
/// letters switch; Space, undo and redo still reach the app.
fn keyboard(host: &mut dyn FxHost, st: &mut EqEditor, ui: &mut egui::Ui, fr: &Frame<'_>) {
    use egui::Key;
    let mut presses = [(Key::Escape, egui::Modifiers::NONE); 16];
    let n = ui.input(|i| {
        let mut n = 0;
        for e in &i.events {
            if let egui::Event::Key {
                key,
                pressed: true,
                modifiers,
                ..
            } = e
            {
                if n < presses.len() {
                    presses[n] = (*key, *modifiers);
                    n += 1;
                }
            }
        }
        n
    });
    let mut used_bands = [0usize; EQ_BANDS];
    let mut nb = 0;
    for i in 0..EQ_BANDS {
        if fr.on[i] {
            used_bands[nb] = i;
            nb += 1;
        }
    }
    let used_bands = &used_bands[..nb];
    for &(key, m) in &presses[..n] {
        let fine = m.shift;
        let digit = match key {
            Key::Num1 => Some(0),
            Key::Num2 => Some(1),
            Key::Num3 => Some(2),
            Key::Num4 => Some(3),
            Key::Num5 => Some(4),
            Key::Num6 => Some(5),
            Key::Num7 => Some(6),
            Key::Num8 => Some(7),
            Key::Num9 => Some(8),
            Key::Num0 => Some(9),
            _ => None,
        };
        if let Some(d) = digit {
            if fr.on[d] {
                if m.command {
                    st.selection.insert(d);
                    st.primary = Some(d);
                } else {
                    st.select(&[d]);
                }
            }
            continue;
        }
        let any = !st.selection.is_empty();
        match key {
            Key::Tab if !used_bands.is_empty() => {
                let pos = st
                    .primary
                    .and_then(|c| used_bands.iter().position(|b| *b == c));
                let next = match (pos, m.shift) {
                    (Some(p), false) => used_bands[(p + 1) % used_bands.len()],
                    (Some(p), true) => used_bands[(p + used_bands.len() - 1) % used_bands.len()],
                    (None, _) => used_bands[0],
                };
                st.select(&[next]);
            }
            Key::ArrowLeft | Key::ArrowRight | Key::ArrowUp | Key::ArrowDown if any => {
                let (semis, db) = match key {
                    Key::ArrowLeft => (-1.0, 0.0),
                    Key::ArrowRight => (1.0, 0.0),
                    Key::ArrowUp => (0.0, 0.5),
                    _ => (0.0, -0.5),
                };
                let k = if fine { 0.1 } else { 1.0 };
                nudge(host, st, semis * k, db * if fine { 0.2 } else { 1.0 });
            }
            Key::OpenBracket | Key::CloseBracket if any => {
                let step = if fine { 1.0 / 24.0 } else { 1.0 / 6.0 };
                let f = 2f32.powf(if key == Key::CloseBracket {
                    step
                } else {
                    -step
                });
                scale_q(host, st, f);
            }
            Key::Delete | Key::Backspace if any => {
                let mut sel = [0usize; EQ_BANDS];
                let mut ns = 0;
                for b in &st.selection {
                    sel[ns] = *b;
                    ns += 1;
                }
                remove_bands(host, st, &sel[..ns]);
            }
            Key::B if any => toggle_selection(host, st, "on"),
            Key::D if any => toggle_selection(host, st, "dyn"),
            Key::S if m.is_none() => {
                let soloed = host.spec().get("solo").round() as usize;
                match st.primary {
                    Some(p) if soloed != p + 1 => set_solo(host, Some(p)),
                    _ if soloed > 0 => set_solo(host, None),
                    _ => {}
                }
            }
            Key::P if any => cycle_placement(host, st),
            Key::A if m.command => {
                st.selection = used_bands.iter().copied().collect();
                st.primary = used_bands.first().copied();
            }
            Key::Enter => {
                if let Some(p) = st.primary {
                    open_typing(st, p, &fr.bands[p]);
                }
            }
            Key::Escape => {
                if st.drag.take().is_some() {
                    // Esc during a drag puts everything back.
                    host.end_gesture();
                    host.undo();
                } else if host.spec().get("solo") >= 1.0 {
                    set_solo(host, None);
                } else {
                    st.selection.clear();
                    st.primary = None;
                }
            }
            _ => {}
        }
    }
    // What is not the EQ's still reaches the app: play, stop, undo, redo, save.
    if let Some((app, _, _)) = host.daw() {
        use crate::keymap::Action;
        let actions = app.keymap.pressed(ui.ctx(), true, true);
        for a in actions {
            if matches!(
                a,
                Action::PlayPause | Action::Stop | Action::Undo | Action::Redo | Action::Save
            ) {
                app.run_action(a);
            }
        }
    }
}

/// What a row of a right-click list does.
#[derive(Copy, Clone, Debug, PartialEq)]
enum Act {
    None,
    Open(MenuKind),
    Shape(usize, usize),
    Place(usize, usize),
    Split(usize, bool),
    Dynamic(usize),
    Solo(usize),
    Bypass(usize),
    Type(usize),
    Remove(usize),
    SelectAll,
    ResetView,
    Analyser(u8),
}

/// The open right-click list, drawn with the kit's menu, and what it does. The shape and
/// placement rows open a list of their own in the same place.
fn menus(host: &mut dyn FxHost, st: &mut EqEditor, ui: &mut egui::Ui, s: &Style, fr: &Frame<'_>) {
    let Some(menu) = st.menu else {
        return;
    };
    let spec = host.spec();
    let mut items: Vec<Item<'_>> = Vec::with_capacity(16);
    let mut acts: Vec<Act> = Vec::with_capacity(16);
    let mut row = |items: &mut Vec<Item<'_>>, it, act| {
        items.push(it);
        acts.push(act);
    };
    match menu.kind {
        MenuKind::Band(b) if fr.on[b] => {
            let band = &fr.bands[b];
            row(&mut items, Item::Band(Src::Str(DIGITS[b])), Act::None);
            row(
                &mut items,
                Item::Action {
                    label: Src::Key(SHAPE_KEYS[band.shape.index().min(8)]),
                    check: crate::kit::menu::Check::None,
                    shortcut: None,
                    submenu: true,
                    enabled: true,
                },
                Act::Open(MenuKind::Shape(b)),
            );
            row(
                &mut items,
                Item::Action {
                    label: Src::Key(PLACE_KEYS[band.placement.index().min(4)]),
                    check: crate::kit::menu::Check::None,
                    shortcut: None,
                    submenu: true,
                    enabled: true,
                },
                Act::Open(MenuKind::Place(b)),
            );
            row(
                &mut items,
                frame::action(Src::Key("eq.split_lr")),
                Act::Split(b, false),
            );
            row(
                &mut items,
                frame::action(Src::Key("eq.split_ms")),
                Act::Split(b, true),
            );
            row(&mut items, Item::Separator, Act::None);
            if band.shape.has_gain() {
                row(
                    &mut items,
                    frame::check(Src::Key("fx.eq.dyn"), band.dynamic),
                    Act::Dynamic(b),
                );
            }
            let soloed = spec.get("solo").round() as usize == b + 1;
            row(
                &mut items,
                frame::check(Src::Key("eq.solo"), soloed),
                Act::Solo(b),
            );
            let on = spec.get(bkey(b, "on")) >= 0.5;
            row(
                &mut items,
                frame::check(Src::Key("eq.bypass_band"), !on),
                Act::Bypass(b),
            );
            row(
                &mut items,
                frame::action(Src::Key("eq.type_values")),
                Act::Type(b),
            );
            row(&mut items, Item::Separator, Act::None);
            row(
                &mut items,
                frame::action(Src::Key("eq.remove_band")),
                Act::Remove(b),
            );
        }
        MenuKind::Shape(b) if fr.on[b] => {
            row(&mut items, Item::Band(Src::Key("fx.eq.shape")), Act::None);
            for (i, key) in SHAPE_KEYS.iter().enumerate() {
                row(
                    &mut items,
                    frame::radio(Src::Key(key), fr.bands[b].shape.index() == i),
                    Act::Shape(b, i),
                );
            }
        }
        MenuKind::Place(b) if fr.on[b] => {
            row(&mut items, Item::Band(Src::Key("fx.eq.place")), Act::None);
            for (i, key) in PLACE_KEYS.iter().enumerate() {
                row(
                    &mut items,
                    frame::radio(Src::Key(key), fr.bands[b].placement.index() == i),
                    Act::Place(b, i),
                );
            }
        }
        MenuKind::Background => {
            let any = fr.on.iter().any(|u| *u);
            if any {
                row(
                    &mut items,
                    frame::action(Src::Key("eq.select_all")),
                    Act::SelectAll,
                );
            } else {
                row(&mut items, frame::disabled(Src::Key("eq.empty")), Act::None);
            }
            row(
                &mut items,
                frame::action(Src::Key("eq.reset_view")),
                Act::ResetView,
            );
            row(&mut items, Item::Band(Src::Key("eq.analyser")), Act::None);
            let show = st.show;
            for (i, (key, on)) in [
                ("eq.an.pre", show.pre),
                ("eq.an.post", show.post),
                ("eq.an.key", show.key),
                ("eq.an.freeze", show.freeze),
                ("eq.an.grab", show.grab),
                ("eq.an.collisions", show.collisions),
            ]
            .into_iter()
            .enumerate()
            {
                row(
                    &mut items,
                    frame::check(Src::Key(key), on),
                    Act::Analyser(i as u8),
                );
            }
        }
        _ => {
            st.menu = None;
            return;
        }
    }
    let mut close = false;
    let at = menu.at;
    let chosen = frame::popup_list(
        ui.ctx(),
        ui.id().with("eq-menu"),
        at,
        Rect::from_center_size(at, Vec2::splat(1.0)),
        s,
        &items,
        220.0 * s.k,
        &mut close,
    );
    drop(items);
    if close {
        st.menu = None;
    }
    let Some(act) = chosen.and_then(|i| acts.get(i).copied()) else {
        return;
    };
    st.menu = None;
    match act {
        Act::None => {}
        Act::Open(kind) => st.menu = Some(Menu { kind, at }),
        Act::Shape(b, i) => set_band(host, b, &[("shape", i as f32)], None),
        Act::Place(b, i) => set_band(host, b, &[("place", i as f32)], None),
        Act::Split(b, ms) => {
            split(host, st, b, ms);
        }
        Act::Dynamic(b) => {
            let on = host.spec().get(bkey(b, "dyn")) >= 0.5;
            set_band(host, b, &[("dyn", if on { 0.0 } else { 1.0 })], None);
        }
        Act::Solo(b) => {
            let soloed = host.spec().get("solo").round() as usize == b + 1;
            set_solo(host, (!soloed).then_some(b));
        }
        Act::Bypass(b) => {
            let on = host.spec().get(bkey(b, "on")) >= 0.5;
            set_band(host, b, &[("on", if on { 0.0 } else { 1.0 })], None);
        }
        Act::Type(b) => open_typing(st, b, &fr.bands[b]),
        Act::Remove(b) => remove_bands(host, st, &[b]),
        Act::SelectAll => {
            st.selection = (0..EQ_BANDS).filter(|b| fr.on[*b]).collect();
            st.primary = st.selection.iter().next().copied();
        }
        Act::ResetView => st.view = VIEW,
        Act::Analyser(i) => {
            let show = &mut st.show;
            let flag = match i {
                0 => &mut show.pre,
                1 => &mut show.post,
                2 => &mut show.key,
                3 => &mut show.freeze,
                4 => &mut show.grab,
                _ => &mut show.collisions,
            };
            *flag = !*flag;
        }
    }
}

/// The exact-entry plate for a band: Freq, Gain, Q boxes. Tab moves between them, Enter
/// sets all three as one step, Esc leaves without changing anything.
fn typing_popup(
    host: &mut dyn FxHost,
    st: &mut EqEditor,
    ui: &mut egui::Ui,
    s: &Style,
    fr: &Frame<'_>,
    geo: &Geo,
) {
    let Some(mut typing) = st.typing.take() else {
        return;
    };
    // Opened this frame (by Enter, perhaps): that Enter is not a commit.
    let was_fresh = typing.fresh;
    let k = s.k;
    let b = typing.band;
    let title = tf("eq.band_n", &[("n", &(b + 1))]);
    let band = fr.bands[b];
    let has_gain = band.shape.has_gain();
    let dot = geo.dot(&band);
    let at = dot + vec2(DOT_R * k + 12.0 * k, DOT_R * k + 8.0 * k);
    let mut commit = false;
    let mut cancel = false;
    let rows = if has_gain { 3 } else { 2 };
    let size = vec2(
        236.0,
        crate::kit::band::TITLE + 8.0 + rows as f32 * 26.0 + 18.0 + 26.0 + 8.0,
    ) * k;
    let hue = s.with_hue(band_color(b));
    egui::Area::new(ui.id().with("eq-typing"))
        .order(egui::Order::Foreground)
        .fixed_pos(at)
        .constrain(true)
        .show(ui.ctx(), |ui| {
            let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
            let look = crate::kit::band::CardLook {
                title: Src::Str(&title),
                switchable: false,
                selector: None,
                menu: false,
                display: None,
            };
            let cr = with_brush(ui.painter(), |bb| {
                bb.rrect_soft(
                    rect.translate(vec2(0.0, 4.0 * k)),
                    8.0 * k,
                    Fill::Solid(with_alpha(
                        s.t.dark_shade,
                        if s.t.dark { 0.4 } else { 0.16 },
                    )),
                    12.0 * k,
                );
                crate::kit::band::card(bb, rect, &hue, State::LIT, &look)
            });
            let labels = ["fx.eq.freq", "fx.eq.gain", "fx.eq.q"];
            let mut y = cr.body.top();
            for (i, label) in labels.iter().enumerate() {
                if i == 1 && !has_gain {
                    continue;
                }
                let lr = Rect::from_min_size(pos2(cr.body.left(), y), vec2(56.0 * k, 22.0 * k));
                with_brush(ui.painter(), |bb| {
                    let t = Text::key(label, Role::Label, s.t.label)
                        .align(Align2::LEFT_CENTER)
                        .policy(Policy::Abbreviate);
                    let _ = ktext::paint(bb, lr, &t, s.tk());
                });
                let fr_rect =
                    Rect::from_min_max(pos2(lr.right(), y), pos2(cr.body.right(), y + 22.0 * k));
                let mut child = ui.new_child(egui::UiBuilder::new().max_rect(fr_rect));
                let r = crate::views::fx::controls::text_field(
                    &mut child,
                    &hue,
                    ui.id().with(("eq-typing-field", i)),
                    &mut typing.fields[i],
                    frame::tr(label),
                    fr_rect.width() / k,
                );
                if typing.fresh && i == 0 {
                    r.request_focus();
                    typing.fresh = false;
                }
                if r.has_focus() {
                    typing.focus = i;
                }
                y += 26.0 * k;
            }
            let hint =
                Rect::from_min_size(pos2(cr.body.left(), y), vec2(cr.body.width(), 16.0 * k));
            with_brush(ui.painter(), |bb| {
                canvas::line_in(
                    bb,
                    hint,
                    Role::Micro,
                    s.t.text_dim,
                    s.tk(),
                    format_args!("{}", frame::tr("eq.type_hint")),
                );
            });
            y += 20.0 * k;
            let keys =
                Rect::from_min_max(pos2(cr.body.left(), y), pos2(cr.body.right(), y + 22.0 * k));
            let mut child = ui.new_child(
                egui::UiBuilder::new()
                    .max_rect(keys)
                    .layout(egui::Layout::right_to_left(egui::Align::Center)),
            );
            child.spacing_mut().item_spacing.x = 6.0 * k;
            let kw = (keys.width() / k - 6.0) * 0.5;
            if crate::views::fx::controls::key(
                &mut child,
                &hue,
                Src::Key("eq.type_set"),
                frame::tr("eq.type_set"),
                kw,
                true,
            )
            .clicked()
            {
                commit = true;
            }
            if crate::views::fx::controls::key(
                &mut child,
                &hue,
                Src::Key("layout.cancel"),
                frame::tr("layout.cancel"),
                kw,
                false,
            )
            .clicked()
            {
                cancel = true;
            }
        });
    if !was_fresh && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
        commit = true;
    }
    if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
        cancel = true;
    }
    if commit {
        let mut fields = [("", 0.0f32); 3];
        let mut n = 0;
        if let Some(f) = parse::parse_freq(&typing.fields[0]) {
            fields[n] = ("freq", f.clamp(10.0, 30_000.0));
            n += 1;
        }
        if has_gain {
            if let Some(g) = parse::parse_gain(&typing.fields[1]) {
                let g = g.clamp(-30.0, 30.0);
                fields[n] = ("gain", g);
                n += 1;
                st.range_db = st.range_for(g);
            }
        }
        if let Some(q) = parse::parse_q(&typing.fields[2]) {
            fields[n] = ("q", q.clamp(0.025, 40.0));
            n += 1;
        }
        if n == 0 {
            host.status(t("eq.type_error"));
            st.typing = Some(typing);
        } else {
            set_band(host, b, &fields[..n], None);
        }
    } else if !cancel {
        st.typing = Some(typing);
    }
}
