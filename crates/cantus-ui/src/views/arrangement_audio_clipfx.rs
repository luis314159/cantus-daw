//! Crossfades over overlaps and clip gain envelopes, drawn on audio clips (ADR 0070).
//!
//! **A crossfade** is drawn on the later clip (it lies over the earlier one there), as
//! Studio One draws it: the overlap veiled, the earlier clip's fade out falling across it
//! and the later one's fade in rising, on the crossfade's curve, with one square at the top
//! of the overlap — dragged sideways it makes the crossfade longer or shorter about its
//! centre, double-clicked it opens the crossfade's editor, right-clicked its menu. An
//! overlap with no crossfade is drawn as one (a dashed edge, the square hollow): it plays
//! both clips summed.
//!
//! **A gain envelope** (shown with *Show clip gain envelopes*) is a line across the clip on
//! its own scale (+12 dB at the top, 0 dB a quarter down, −∞ at the bottom): points as
//! dots, a bend's handle half-way along each segment that bends. The waveform is drawn
//! with the line applied.

use super::super::TimelineGeometry;
use super::{Handle, Shown, HANDLE};
use crate::app::CantusApp;
use crate::clipfx::{content_at, env_v, window_ats};
use crate::kit::color::{shade, with_alpha};
use crate::kit::{Brush, Fill, Style};
use cantus_core::clip_gain::{gain_of, GainEnvelope};
use cantus_core::crossfade::{self, XfadeCurve};
use cantus_core::{AudioRegion, ClipId, Ticks, TrackId};
use egui::{pos2, vec2, Pos2, Rect};

/// The most envelope points one clip offers to the pointer at once (more are drawn; zoom
/// in to grab them).
pub const ENV_HITS: usize = 64;
/// How finely the line's height is kept for the pointer (across the clip's width).
pub const LINE_SAMPLES: usize = 96;

/// A clip's part in one overlap, on the timeline.
#[derive(Copy, Clone, Debug, PartialEq)]
struct Part {
    from: Ticks,
    to: Ticks,
    curve: Option<XfadeCurve>,
    /// It starts the overlap (the later window's first piece: it draws the square).
    first: bool,
}

/// A track's clips' parts in its overlaps: (into, out of), by clip.
type Parts = std::collections::HashMap<ClipId, (Option<Part>, Option<Part>)>;

/// The overlaps of the tracks drawn this frame: each track's found once, then each clip's
/// part looked up by its id (review 1: no scan of every overlap per clip).
#[derive(Debug, Default)]
pub struct Cache {
    tracks: std::collections::HashMap<TrackId, Parts>,
}

impl Cache {
    pub fn clear(&mut self) {
        self.tracks.clear();
    }

    fn of(
        &mut self,
        app: &CantusApp,
        track: TrackId,
        clip: ClipId,
    ) -> (Option<Part>, Option<Part>) {
        let parts = self.tracks.entry(track).or_insert_with(|| {
            let mut m = std::collections::HashMap::new();
            if let Some(t) = app.project().track(track) {
                for o in crossfade::overlaps_of_track(t) {
                    for id in &o.right {
                        let e: &mut (Option<Part>, Option<Part>) = m.entry(*id).or_default();
                        e.0 = Some(Part {
                            from: o.from,
                            to: o.to,
                            curve: o.curve,
                            first: *id == o.right_first,
                        });
                    }
                    for id in &o.left {
                        let e: &mut (Option<Part>, Option<Part>) = m.entry(*id).or_default();
                        e.1 = Some(Part {
                            from: o.from,
                            to: o.to,
                            curve: o.curve,
                            first: false,
                        });
                    }
                }
            }
            m
        });
        parts.get(&clip).copied().unwrap_or_default()
    }
}

/// How a clip as drawn takes part in overlaps, in ticks from its window's start.
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub struct Crossing {
    /// It is (a piece of) the later window of an overlap: where the overlap starts (before
    /// this piece, for the second piece of a split one), how long it is, its curve (`None`:
    /// summed), and whether this piece starts it (and draws its square).
    pub into: Option<(Ticks, Ticks, Option<XfadeCurve>, bool)>,
    /// It is (a piece of) the earlier window of a crossed overlap: where, how long, the
    /// curve.
    pub out_of: Option<(Ticks, Ticks, XfadeCurve)>,
}

impl Crossing {
    /// The crossfade's gain at `d` ticks into the window.
    pub fn gain(&self, d: Ticks) -> f32 {
        let mut g = 1.0;
        if let Some((at, len, Some(c), _)) = self.into {
            if d >= at && d < at + len {
                g *= c.gain_in(((d.0 - at.0) as f32 + 0.5) / len.0.max(1) as f32);
            }
        }
        if let Some((at, len, c)) = self.out_of {
            if d >= at {
                g *= c.gain_out(((d.0 - at.0) as f32 + 0.5) / len.0.max(1) as f32);
            }
        }
        g
    }

    fn crossed_in(&self) -> bool {
        matches!(self.into, Some((_, _, Some(_), _)))
    }
}

/// How `shown` takes part in its track's overlaps. A clip drawn where a drag previews it
/// takes part in none (its crossings are drawn where it lands).
pub fn crossing(app: &CantusApp, cache: &mut Cache, shown: &Shown<'_>) -> Crossing {
    let c = shown.clip;
    if shown.start != c.start || shown.length != c.length {
        return Crossing::default();
    }
    let (into, out_of) = cache.of(app, shown.track, c.id);
    Crossing {
        into: into.map(|p| (p.from - c.start, p.to - p.from, p.curve, p.first)),
        out_of: out_of.and_then(|p| p.curve.map(|k| (p.from - c.start, p.to - p.from, k))),
    }
}

/// The gain the waveform is drawn with at `d` ticks into the window: the clip's gain, its
/// fades (those a crossfade does not replace — as the engine plays it), the crossfades and
/// the envelope (repeating with a looping window's content).
pub fn column_gain(shown: &Shown<'_>, x: &Crossing, gain: f32, d: Ticks) -> f32 {
    let a = shown.region;
    let keep_in = !x.crossed_in();
    let keep_out = x.out_of.is_none();
    let mut g = gain * fades_gain(a, shown.length, d, keep_in, keep_out) * x.gain(d);
    if let Some(e) = a.gain_env.as_deref().filter(|e| !e.is_flat()) {
        let t = content_at(
            shown.offset,
            shown.length,
            shown.clip.period(),
            a.reverse,
            d,
        );
        g *= gain_of(e.db_at(t.0 as f64));
    }
    g
}

/// The fades' gain (`audio_clips::fade_gain_at`), leaving out a fade a crossfade replaces.
fn fades_gain(a: &AudioRegion, length: Ticks, d: Ticks, keep_in: bool, keep_out: bool) -> f32 {
    let len = length.0.max(1) as f64;
    let fi = if keep_in {
        a.fade_in.0.max(0) as f64
    } else {
        0.0
    };
    let fo = if keep_out {
        a.fade_out.0.max(0) as f64
    } else {
        0.0
    };
    let (mut fi, mut fo) = (fi, fo);
    if fi + fo > len {
        let k = len / (fi + fo);
        fi *= k;
        fo *= k;
    }
    let d = d.0 as f64;
    let mut g = 1.0f32;
    if fi > 0.0 && d < fi {
        g *= a.fade_in_curve.gain((d / fi) as f32);
    }
    let left = len - d;
    if fo > 0.0 && left < fo {
        g *= a.fade_out_curve.gain((left / fo) as f32);
    }
    g
}

// ---- hits --------------------------------------------------------------------------------

/// Where a crossfade's parts are on screen.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct XfadeHits {
    /// The overlap, the clip's waveform height.
    pub band: Rect,
    /// The square at its top.
    pub square: Rect,
}

/// Where an envelope's parts are on screen.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct EnvHits {
    /// The waveform's area the line is drawn over.
    pub area: Rect,
    /// The points offered to the pointer, with their indices in the envelope.
    pub n: usize,
    pub pts: [Pos2; ENV_HITS],
    pub idx: [u16; ENV_HITS],
    /// A bend's handle per point offered (`NaN`: its segment does not bend).
    pub bends: [Pos2; ENV_HITS],
    /// The line's height at `LINE_SAMPLES` places across `area`.
    pub line: [f32; LINE_SAMPLES],
    /// To read a content tick off an x: the window's start on screen, points per tick,
    /// and the window.
    pub x_start: f32,
    pub ppt: f32,
    pub offset: Ticks,
    pub length: Ticks,
    pub reverse: bool,
    /// A looping window's period: its line repeats with its content.
    pub period: Option<Ticks>,
    /// The interface's scale (how near the pointer must come).
    pub k: f32,
}

impl EnvHits {
    /// The content tick under `x`.
    pub fn content_tick(&self, x: f32) -> Ticks {
        let d = Ticks(((x - self.x_start) / self.ppt.max(1e-9)).round() as i64);
        content_at(
            self.offset,
            self.length,
            self.period,
            self.reverse,
            d.max(Ticks::ZERO).min(self.length),
        )
    }

    /// The xs of a content tick: one, or one in each repetition of a looping window.
    pub fn xs_of(&self, t: Ticks) -> Vec<f32> {
        window_ats(self.offset, self.length, self.period, self.reverse, t)
            .into_iter()
            .map(|d| self.x_start + d.0 as f32 * self.ppt)
            .collect()
    }

    /// The line's height under `x`.
    pub fn line_y(&self, x: f32) -> f32 {
        let w = self.area.width().max(1.0);
        let f = ((x - self.area.left()) / w * (LINE_SAMPLES - 1) as f32)
            .clamp(0.0, (LINE_SAMPLES - 1) as f32);
        let i = f.floor() as usize;
        let j = (i + 1).min(LINE_SAMPLES - 1);
        let u = f - i as f32;
        self.line[i] + (self.line[j] - self.line[i]) * u
    }

    /// The part under `pos`: a point, a bend's handle, or the line itself.
    pub fn at(&self, pos: Pos2) -> Option<Handle> {
        let k = self.k;
        let reach = 5.0 * k;
        let near = |p: Pos2| p.x.is_finite() && (p - pos).length() <= reach;
        if let Some(i) = (0..self.n).find(|&i| near(self.pts[i])) {
            return Some(Handle::EnvPoint(self.idx[i]));
        }
        if let Some(i) = (0..self.n).find(|&i| near(self.bends[i])) {
            return Some(Handle::EnvBend(self.idx[i]));
        }
        if self.area.contains(pos) && (self.line_y(pos.x) - pos.y).abs() <= 4.0 * k {
            return Some(Handle::EnvLine(self.content_tick(pos.x).0 as i32));
        }
        None
    }
}

/// The square at the top of an overlap, kept off the clip's own squares (its gain's and
/// its fades': review 1, a short summed overlap's square took the fade in's hits).
pub fn xfade_square(band: Rect, inner: Rect, avoid: &[Rect], k: f32) -> Rect {
    let hs = HANDLE * k;
    let mut sq =
        Rect::from_center_size(pos2(band.center().x, inner.top() + hs * 0.5), vec2(hs, hs));
    for _ in 0..avoid.len() + 1 {
        if !avoid.iter().any(|r| sq.expand(2.0).intersects(*r)) {
            break;
        }
        sq = sq.translate(vec2(0.0, hs + 2.0 * k));
    }
    sq
}

// ---- painting ------------------------------------------------------------------------------

/// Paint the crossfade `shown` starts with (if any) over its waveform; returns its hits.
#[allow(clippy::too_many_arguments)]
pub fn paint_xfade(
    b: &mut Brush,
    geo: &TimelineGeometry,
    shown: &Shown<'_>,
    x: &Crossing,
    inner: Rect,
    avoid: &[Rect],
    hot: Option<Handle>,
    s: &Style,
) -> Option<XfadeHits> {
    // Drawn by the piece that starts the overlap.
    let (_, len, curve, first) = x.into?;
    if !first {
        return None;
    }
    let k = s.k;
    let x0 = geo.x(shown.start.as_beats_f64() as f32);
    let x1 = geo.x((shown.start + len).as_beats_f64() as f32);
    if x1 - x0 < 2.0 {
        return None;
    }
    let band = Rect::from_min_max(pos2(x0, inner.top()), pos2(x1, inner.bottom()));
    let lit = matches!(hot, Some(Handle::Xfade));
    let tone = if lit { s.t.highlight } else { s.t.selection };
    b.rect(
        band,
        Fill::Solid(with_alpha(tone, if s.t.dark { 0.16 } else { 0.12 })),
    );
    match curve {
        Some(c) => {
            // The two halves of the curve, from one stretch: the earlier clip's falling,
            // the later's rising (amplitude, bottom to top).
            let n = (((x1 - x0) / 2.0).ceil() as usize).clamp(2, 128);
            let mut down: Vec<Pos2> = Vec::with_capacity(n + 1);
            let mut up: Vec<Pos2> = Vec::with_capacity(n + 1);
            for i in 0..=n {
                let u = i as f32 / n as f32;
                let px = x0 + (x1 - x0) * u;
                let (gi, go) = c.gains(u);
                up.push(pos2(px, band.bottom() - gi * band.height()));
                down.push(pos2(px, band.bottom() - go * band.height()));
            }
            let w = 1.4 * k;
            b.polyline(&down, w, with_alpha(shade(tone, -0.1), 0.95));
            b.polyline(&up, w, tone);
        }
        None => {
            // Summed: the overlap's two edges, dashed.
            let (dash, gap) = (3.0 * k, 3.0 * k);
            for px in [x0, x1] {
                b.dashed(
                    &[pos2(px, band.top()), pos2(px, band.bottom())],
                    b.px(),
                    with_alpha(tone, 0.9),
                    dash,
                    gap,
                );
            }
        }
    }
    let square = xfade_square(band, inner, avoid, k);
    let r = b.snap_rect(square);
    b.rrect(
        r.expand(b.px()),
        1.5 * k,
        Fill::Solid(with_alpha(s.t.shadow, 0.85)),
    );
    if curve.is_some() {
        b.rrect(r, 1.0 * k, Fill::V(shade(tone, 0.08), shade(tone, -0.08)));
    } else {
        b.rrect_stroke(r, 1.0 * k, 1.2 * k, tone);
    }
    Some(XfadeHits { band, square })
}

/// Paint `shown`'s gain envelope across `inner` (handles too when `hot_clip`); returns the
/// hits. A clip with no envelope draws its flat line at 0 dB, where a first point goes.
#[allow(clippy::too_many_arguments)]
pub fn paint_env(
    b: &mut Brush,
    geo: &TimelineGeometry,
    lanes: Rect,
    shown: &Shown<'_>,
    inner: Rect,
    hot_clip: bool,
    hot: Option<Handle>,
    s: &Style,
) -> Option<EnvHits> {
    if inner.height() < 10.0 * s.k || inner.width() < 4.0 {
        return None;
    }
    let k = s.k;
    let flat = GainEnvelope::default();
    let env = shown.region.gain_env.as_deref().unwrap_or(&flat);
    let ppt = geo.px_per_beat / cantus_core::PPQ as f32;
    let mut h = EnvHits {
        area: inner,
        n: 0,
        pts: [Pos2::new(f32::NAN, f32::NAN); ENV_HITS],
        idx: [0; ENV_HITS],
        bends: [Pos2::new(f32::NAN, f32::NAN); ENV_HITS],
        line: [0.0; LINE_SAMPLES],
        x_start: geo.x(shown.start.as_beats_f64() as f32),
        ppt,
        offset: shown.offset,
        length: shown.length,
        reverse: shown.region.reverse,
        period: shown.clip.period(),
        k,
    };
    let y_of = |db: f32| inner.top() + env_v(db) * inner.height();
    // The line, a vertex every two points across what is on screen.
    let (vx0, vx1) = (
        inner.left().max(lanes.left()),
        inner.right().min(lanes.right()),
    );
    for i in 0..LINE_SAMPLES {
        let px = inner.left() + inner.width() * i as f32 / (LINE_SAMPLES - 1) as f32;
        h.line[i] = y_of(env.db_at(h.content_tick(px).0 as f64));
    }
    if vx1 > vx0 {
        let n = (((vx1 - vx0) / 2.0).ceil() as usize).clamp(2, 2048);
        let pts: Vec<Pos2> = (0..=n)
            .map(|i| {
                let px = vx0 + (vx1 - vx0) * i as f32 / n as f32;
                pos2(px, y_of(env.db_at(h.content_tick(px).0 as f64)))
            })
            .collect();
        let lit = matches!(
            hot,
            Some(Handle::EnvLine(_) | Handle::EnvPoint(_) | Handle::EnvBend(_))
        );
        let ink = if lit { s.t.highlight } else { s.t.selection };
        b.polyline(&pts, 1.5 * k, with_alpha(ink, 0.95));
    }
    // The points (and the bends' handles), those on screen.
    let r = 3.0 * k;
    for (i, p) in env.points.iter().enumerate() {
        let qxs = env.points.get(i + 1).map(|q| (q, h.xs_of(q.tick)));
        // Once, or in each repetition of a looping window (review 1).
        for (rep, px) in h.xs_of(p.tick).into_iter().enumerate() {
            if px < vx0 - r || px > vx1 + r {
                continue;
            }
            let at = pos2(px, y_of(p.db));
            let bend = qxs.as_ref().and_then(|(q, xs)| {
                let qx = *xs.get(rep)?;
                let bends = p.shape.has_tension() && p.shape != cantus_core::Shape::Hold;
                ((qx - px).abs() >= 16.0 * k && (q.db - p.db).abs() > 0.05 && bends).then(|| {
                    let mid = cantus_core::Ticks(p.tick.0 + (q.tick.0 - p.tick.0) / 2);
                    pos2((px + qx) * 0.5, y_of(env.db_at(mid.0 as f64)))
                })
            });
            if h.n < ENV_HITS && inner.expand(r).contains(at) {
                h.pts[h.n] = at;
                h.idx[h.n] = i.min(u16::MAX as usize) as u16;
                h.bends[h.n] = bend.unwrap_or(Pos2::new(f32::NAN, f32::NAN));
                h.n += 1;
            }
            let this = Some(Handle::EnvPoint(i as u16));
            let fill = if hot == this {
                s.t.highlight
            } else {
                s.t.selection
            };
            if hot_clip || hot == this {
                b.dot(at, r + b.px(), with_alpha(s.t.shadow, 0.85));
                b.dot(at, r, fill);
            } else {
                b.dot(at, r * 0.6, fill);
            }
            if let (Some(m), true) = (bend, hot_clip) {
                let lit = hot == Some(Handle::EnvBend(i as u16));
                let c = if lit { s.t.highlight } else { s.t.selection };
                b.circle_stroke(m, r * 0.85, 1.2 * k, c);
            }
        }
    }
    Some(h)
}
