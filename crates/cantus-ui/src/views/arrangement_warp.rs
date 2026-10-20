//! Warping in the arrangement (round 29, ADR 0061): a warped clip's waveform read through
//! its warp (piecewise, a run of columns per marker segment), its transients as faint ticks
//! along the top band, its warp markers as handles — and the gestures on them, which the
//! audio clip's handle dispatch (`arrangement_audio_edit.rs`) hands here.
//!
//! **Gestures** (Ableton's warp markers, FL's audio stretch): double-click the waveform's
//! top band to add a marker there (on the transient under the pointer when there is one);
//! drag a marker to move it — the audio between its neighbours stretches — snapped to the
//! grid unless Ctrl; **Alt**-drag a marker to slide the content under it instead; drag in
//! the band to add a marker and move it in one go; double-click a marker to remove it, or
//! select it (a click) and press Del. The band takes a press only on a clip that follows
//! the tempo (round 29's review: on any other, a press near its top moves it as ever); in
//! the warp editor, where warping is what is asked, on any clip.
//! **Alt**-drag a clip's right edge stretches the whole clip (`clips.rs`, ADR 0061 §5).

use super::super::TimelineGeometry;
use super::{Handle, Shown};
use crate::app::CantusApp;
use crate::clips::ClipRef;
use crate::kit::color::with_alpha;
use crate::kit::{Brush, Style};
use crate::waveform::{SourcePeaks, Which};
use cantus_core::warp::Warp;
use cantus_core::{Ticks, PPQ};
use egui::{pos2, Pos2, Rect};

/// Markers and transients a clip offers the pointer per frame: the nearest to it.
pub const MAX_HITS: usize = 64;
/// The top band's height at 100 %.
pub const BAND: f32 = 7.0;
/// How near the pointer must be to a marker or a transient, points at 100 %.
pub const REACH: f32 = 4.0;

/// Where a reversed window's content is heard (round 29's third review): the engine renders
/// the window forwards and plays the render backwards sample by sample, so what lies at
/// forward timeline tick `t` is heard `seconds(t, end)` after the window's start — the tick
/// mirror where one tempo holds over the window (to the bit), not across a tempo change.
/// Its own inverse: [`Reversal::heard`] of a heard tick is the forward one.
#[derive(Clone, Debug)]
pub struct Reversal {
    map: std::sync::Arc<cantus_core::TempoMap>,
    start: f64,
    end: f64,
    held: bool,
}

impl Reversal {
    /// The window `[start, end)` under `map`.
    pub fn new(map: std::sync::Arc<cantus_core::TempoMap>, start: Ticks, end: Ticks) -> Self {
        let (s, e) = (start.0 as f64, end.0 as f64);
        let held = map
            .held_at(s)
            .is_some_and(|(_, from, to)| from <= s && to >= e);
        Reversal {
            map,
            start: s,
            end: e,
            held,
        }
    }

    /// The timeline tick where forward tick `t` is heard (and back).
    pub fn heard(&self, t: f64) -> f64 {
        if self.held {
            self.start + self.end - t
        } else {
            self.map
                .tick_after_seconds(self.start, self.map.seconds_between_f64(t, self.end))
        }
    }

    /// The content tick drawn (heard) where content tick `c` plays, content tick 0 at
    /// timeline tick `origin` — and back.
    pub fn content(&self, origin: f64, c: f64) -> f64 {
        self.heard(origin + c) - origin
    }

    /// The same, as a table of knots a hit test can hold by value ([`Mirror`]).
    pub fn mirror(&self, origin: f64) -> Mirror {
        let mut knots: Vec<f64> = vec![self.start, self.end];
        let mut t = self.start;
        while let Some(n) = self.map.next_change_after(t).filter(|n| *n < self.end) {
            knots.push(n);
            knots.push(self.heard(n));
            t = n;
        }
        // Inside a ramp the mirror is curved: samples between.
        if !self.held {
            for k in 1..MIRROR_KNOTS / 2 {
                knots.push(
                    self.start + (self.end - self.start) * k as f64 / (MIRROR_KNOTS / 2) as f64,
                );
            }
        }
        knots.sort_by(f64::total_cmp);
        knots.dedup_by(|a, b| (*a - *b).abs() < 1e-9);
        let n = knots.len().min(MIRROR_KNOTS);
        let step = (knots.len() - 1) as f64 / (n - 1).max(1) as f64;
        let mut m = Mirror {
            n,
            drawn: [0.0; MIRROR_KNOTS],
            content: [0.0; MIRROR_KNOTS],
        };
        for i in 0..n {
            let t = knots[((i as f64 * step).round() as usize).min(knots.len() - 1)];
            // Heard at t: the content forward there.
            m.drawn[i] = t - origin;
            m.content[i] = self.heard(t) - origin;
        }
        m
    }
}

/// The most knots a [`Mirror`] holds.
pub const MIRROR_KNOTS: usize = 24;

/// A reversed window's content mirror held by value: the content tick under a drawn one,
/// exact between its knots where the tempo holds (the tempo points and their mirrors), a
/// line between samples inside a ramp.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Mirror {
    n: usize,
    drawn: [f64; MIRROR_KNOTS],
    content: [f64; MIRROR_KNOTS],
}

impl Mirror {
    /// The content tick played where content tick `c` is drawn.
    pub fn content(&self, c: f64) -> f64 {
        let (d, v) = (&self.drawn[..self.n], &self.content[..self.n]);
        if self.n < 2 {
            return v.first().copied().unwrap_or(c);
        }
        let i = d.partition_point(|x| *x <= c).clamp(1, self.n - 1);
        let (a, b) = (i - 1, i);
        let u = (c - d[a]) / (d[b] - d[a]).max(1e-12);
        v[a] + (v[b] - v[a]) * u
    }
}

/// Where a clip's markers and transients were drawn this frame: the hit test's geometry.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Hits {
    pub band: Rect,
    pub wave: Rect,
    n: usize,
    xs: [f32; MAX_HITS],
    ids: [u16; MAX_HITS],
    m: usize,
    onset_xs: [f32; MAX_HITS],
    onset_ids: [u32; MAX_HITS],
    /// The x of content tick 0, and points per tick.
    origin: f32,
    px_per_tick: f32,
    reach: f32,
    /// Whether the band and the transients take a press (a warped clip, or the editor).
    offer: bool,
    /// A reversed clip's mirror: its content is drawn where it is heard, so a place on it
    /// is read back the same way.
    mirror: Option<Mirror>,
}

impl Hits {
    fn new(band: Rect, wave: Rect, origin: f32, px_per_tick: f32, k: f32) -> Self {
        Hits {
            band,
            wave,
            n: 0,
            xs: [0.0; MAX_HITS],
            ids: [0; MAX_HITS],
            m: 0,
            onset_xs: [0.0; MAX_HITS],
            onset_ids: [0; MAX_HITS],
            origin,
            px_per_tick,
            reach: REACH * k,
            offer: true,
            mirror: None,
        }
    }

    /// The content tick under x (a reversed clip's mirrored, as it is drawn).
    pub fn tick_at(&self, x: f32) -> Ticks {
        let c = ((x - self.origin) / self.px_per_tick.max(1e-9)) as f64;
        let c = match &self.mirror {
            Some(m) => m.content(c),
            None => c,
        };
        Ticks(c.round() as i64)
    }

    /// The markers' xs drawn this frame (tests).
    pub fn marker_count(&self) -> usize {
        self.n
    }

    /// The marker or transient under `pos`, or the band.
    pub fn at(&self, pos: Pos2) -> Option<Handle> {
        let near = |xs: &[f32]| {
            xs.iter()
                .enumerate()
                .filter(|(_, x)| (**x - pos.x).abs() <= self.reach)
                .min_by(|a, b| (a.1 - pos.x).abs().total_cmp(&(b.1 - pos.x).abs()))
                .map(|(i, _)| i)
        };
        let column = |r: Rect| pos.y >= r.top() - 1.0 && pos.y <= r.bottom();
        if column(self.wave.union(self.band)) {
            if let Some(i) = near(&self.xs[..self.n]) {
                if pos.y <= self.band.bottom() + self.reach * 2.0 {
                    return Some(Handle::WarpMarker(self.ids[i]));
                }
            }
        }
        if self.offer && self.band.expand2(egui::vec2(0.0, 1.0)).contains(pos) {
            if let Some(i) = near(&self.onset_xs[..self.m]) {
                return Some(Handle::WarpOnset(self.onset_ids[i]));
            }
            return Some(Handle::WarpBand(
                self.tick_at(pos.x)
                    .0
                    .clamp(i32::MIN as i64, i32::MAX as i64) as i32,
            ));
        }
        None
    }
}

/// The waveform's columns of a warped clip: `n` columns from x0 to x1, each read through
/// the warp (a run of columns per marker segment, where the warp is linear).
#[allow(clippy::too_many_arguments)]
pub fn columns(
    peaks: &SourcePeaks,
    which: Which,
    geo: &TimelineGeometry,
    shown: &Shown<'_>,
    w: &Warp,
    x0: f32,
    x1: f32,
    n: usize,
    out: &mut Vec<(f32, f32)>,
    run: &mut Vec<(f32, f32)>,
) {
    out.clear();
    let col_w = (x1 - x0) as f64 / n as f64;
    let ppt = geo.px_per_beat as f64 / PPQ as f64;
    let origin = geo.x(shown.start.as_beats_f64() as f32) as f64 - shown.offset.0 as f64 * ppt;
    let scale = peaks.rate as f64 / w.rate.max(1) as f64;
    let content = |x: f64| (x - origin) / ppt;
    // Reversed: each content tick drawn where it is heard (mirrored through the map).
    let rev = shown.region.reverse.then(|| {
        Reversal::new(
            std::sync::Arc::new(shown.map.clone()),
            shown.start,
            shown.start + shown.length,
        )
    });
    let origin_t = (shown.start - shown.offset).0 as f64;
    let mirrored = |c: f64| rev.as_ref().map_or(c, |r| r.content(origin_t, c));
    // Read through the warp.
    let frame = |x: f64| w.frame_at(mirrored(content(x))) * scale;
    let mut i = 0usize;
    while i < n {
        // The run: columns up to the next marker's x (where the slope changes).
        let xa = x0 as f64 + i as f64 * col_w;
        let ca = content(xa + 0.5 * col_w);
        let next = w
            .markers
            .iter()
            .map(|m| m.tick.0 as f64)
            .map(mirrored)
            .filter(|t| *t > ca)
            .fold(f64::INFINITY, f64::min);
        let end_x = origin + next * ppt;
        let j = if end_x.is_finite() {
            (((end_x - x0 as f64) / col_w).ceil() as usize).clamp(i + 1, n)
        } else {
            n
        };
        let fa = frame(xa);
        let fb = frame(x0 as f64 + j as f64 * col_w);
        let (from, per) = if fb >= fa {
            (fa, (fb - fa) / (j - i) as f64)
        } else {
            (fb, (fa - fb) / (j - i) as f64)
        };
        peaks.columns(which, from, per.max(1e-6), j - i, run);
        if fb < fa {
            run.reverse();
        }
        out.extend_from_slice(run);
        i = j;
    }
}

/// Paint the transients (faint ticks at the band's foot) and the markers (a gold line with
/// a flag in the band; the selected one bright) of clip `r` over `inner`; returns where they
/// are for the hit test. `show`: the clip is hovered, selected or warped.
#[allow(clippy::too_many_arguments)]
pub fn paint(
    b: &mut Brush,
    app: &CantusApp,
    geo: &TimelineGeometry,
    lanes: Rect,
    shown: &Shown<'_>,
    inner: Rect,
    show: bool,
    editor: bool,
    s: &Style,
) -> Option<Hits> {
    let k = s.k;
    if inner.height() < 8.0 * k {
        return None;
    }
    let r: ClipRef = (shown.track, shown.clip.id);
    // The band: a third of a short clip at most.
    let bh = (BAND * k).min(inner.height() * 0.35);
    let band = Rect::from_min_max(inner.min, pos2(inner.right(), inner.top() + bh));
    let ppt = geo.px_per_beat / PPQ as f32;
    let origin = geo.x(shown.start.as_beats_f64() as f32) - shown.offset.0 as f32 * ppt;
    let mut hits = Hits::new(band, inner, origin, ppt, k);
    let warp = shown.region.warp.as_deref();
    hits.offer = editor || warp.is_some_and(|w| w.follow);
    let origin_t = (shown.start - shown.offset).0 as f64;
    let rev = shown.region.reverse.then(|| {
        Reversal::new(
            crate::views::bars::map(app.project()),
            shown.start,
            shown.start + shown.length,
        )
    });
    hits.mirror = rev.as_ref().map(|r| r.mirror(origin_t));
    // Every handle drawn is offered by its nearness to the pointer: the nearest
    // [`MAX_HITS`] (round 29's review: the first ones in order left the rest ungrabbable).
    let pointer = b
        .ctx()
        .pointer_hover_pos()
        .map_or(inner.center().x, |p| p.x);
    let mut onsets_all: Vec<(f32, u32)> = Vec::new();
    let mut markers_all: Vec<(f32, u16)> = Vec::new();
    let selected = app.warp.selected.filter(|(c, _)| *c == r).map(|(_, i)| i);
    let visible =
        |x: f32| x >= inner.left().max(lanes.left()) && x <= inner.right().min(lanes.right());
    // The transients, when the analysis is in.
    if show {
        if let Some(a) = app.warp_analysis(r) {
            let map = crate::views::bars::map(app.project());
            let (start, offset) = (shown.start, shown.offset);
            let (lo, hi) = (start.0 as f64, (start + shown.length).0 as f64);
            let ink = with_alpha(s.t.text_dim, 0.6);
            for (id, f) in a.onsets.iter().enumerate() {
                let t = cantus_core::audio_time::clip_tick_at_frame(
                    &map,
                    start,
                    offset,
                    shown.region,
                    a.rate.max(1) as f64,
                    *f,
                );
                let t = rev.as_ref().map_or(t, |r| r.heard(t));
                if t < lo || t > hi {
                    continue;
                }
                let x = geo.x((t / PPQ as f64) as f32);
                if !visible(x) {
                    continue;
                }
                b.vline(x, band.bottom() - bh * 0.6, band.bottom(), b.px(), ink);
                onsets_all.push((x, id as u32));
            }
        }
        b.hline(
            band.left(),
            band.right(),
            band.bottom(),
            b.px(),
            with_alpha(s.t.text_dim, 0.25),
        );
    }
    // The markers: the user's (the warp's own are never drawn — round 29's second review),
    // the window's two ends among them.
    if let Some(w) = warp.filter(|w| w.follow) {
        let at = Window {
            offset: shown.offset,
            length: shown.length,
            start: shown.start,
            reversal: rev.clone(),
            origin,
            ppt,
        };
        for (x, i) in marker_places(w, &at, inner, lanes, k) {
            let i = i as usize;
            let on = selected == Some(i);
            let ink = if on { s.t.highlight } else { s.t.brand_bright };
            b.vline(
                x,
                band.top(),
                inner.bottom(),
                b.px() * if on { 2.0 } else { 1.0 },
                with_alpha(ink, if on { 0.95 } else { 0.7 }),
            );
            // The flag inside the band (a marker on the window's edge too).
            let hw = bh * 0.6;
            let fx = x.clamp(band.left() + hw, (band.right() - hw).max(band.left() + hw));
            b.polygon(
                &[
                    pos2(fx - hw, band.top()),
                    pos2(fx + hw, band.top()),
                    pos2(x, band.bottom()),
                ],
                ink,
            );
            markers_all.push((x, i as u16));
        }
    }
    for (x, id) in nearest(onsets_all, pointer) {
        hits.onset_xs[hits.m] = x;
        hits.onset_ids[hits.m] = id;
        hits.m += 1;
    }
    for (x, id) in nearest(markers_all, pointer) {
        hits.xs[hits.n] = x;
        hits.ids[hits.n] = id;
        hits.n += 1;
    }
    Some(hits)
}

/// Where a clip's window lies, for its markers' places.
#[derive(Clone, Debug)]
pub struct Window {
    pub offset: Ticks,
    pub length: Ticks,
    /// The window's start on the timeline.
    pub start: Ticks,
    /// A reversed window's mirror through the map.
    pub reversal: Option<Reversal>,
    /// The x of content tick 0, and points a tick.
    pub origin: f32,
    pub ppt: f32,
}

/// The user's markers of `w` drawn over a clip's waveform area `inner` (inside `lanes`):
/// `(x, index)` — the warp's own never (round 29's second review), a marker on the window's
/// start or end always: tested in content ticks, and its x against the clip's body (`inner`
/// is the body inset by two points), never culled by a float's rounding.
pub fn marker_places(w: &Warp, at: &Window, inner: Rect, lanes: Rect, k: f32) -> Vec<(f32, u16)> {
    let off = at.offset.0 as f64;
    let len = at.length.0 as f64;
    let edge = 2.0 * k + 0.5;
    let (lo, hi) = (
        (inner.left() - edge).max(lanes.left()),
        (inner.right() + edge).min(lanes.right()),
    );
    let mut out = Vec::new();
    for (i, m) in w.markers.iter().enumerate() {
        let c = m.tick.0 as f64;
        if m.auto || c < off || c > off + len {
            continue;
        }
        let origin_t = (at.start - at.offset).0 as f64;
        let c = at.reversal.as_ref().map_or(c, |r| r.content(origin_t, c));
        let x = at.origin + c as f32 * at.ppt;
        if x >= lo && x <= hi {
            out.push((x.clamp(lo, hi), i.min(u16::MAX as usize) as u16));
        }
    }
    out
}

/// How fast the source plays just after marker `i` of clip `r` (1: as recorded), against
/// the tempo in force where the marker is heard (round 29's second review: the song's
/// first tempo read wrong under a later one).
pub fn marker_speed(app: &CantusApp, r: ClipRef, i: usize) -> Option<f64> {
    let w = app.clip_warp(r)?;
    let c = app.project().track(r.0)?.clip(r.1)?;
    let m = w.markers.get(i)?;
    let at = (c.start - c.offset + m.tick).0 as f64;
    let bpm = crate::views::bars::map(app.project()).bpm_at(at);
    Some(w.speed_at(m.tick.0 as f64 + 0.5, bpm))
}

/// Of handles drawn at `(x, id)`, the [`MAX_HITS`] nearest to the pointer at `x`.
fn nearest<T: Copy>(mut all: Vec<(f32, T)>, x: f32) -> Vec<(f32, T)> {
    all.sort_by(|a, b| (a.0 - x).abs().total_cmp(&(b.0 - x).abs()));
    all.truncate(MAX_HITS);
    all
}

// ---- the gestures --------------------------------------------------------------------

/// A warp marker being dragged: the warp at the press (with the marker a band drag made),
/// which marker, and whether Alt slides the content under it.
#[derive(Clone, Debug)]
pub struct MarkerDrag {
    pub warp0: Warp,
    pub marker: usize,
    pub slide: bool,
    /// The grid it snaps to (`None`: free) — the arrangement's, or the warp editor's own.
    pub snap: Option<Ticks>,
    /// The warp the clip turns to follow from with this drag's first step, when it does
    /// not follow yet (its windows kept on their audio then — round 29's second review).
    pub base: Option<Warp>,
    /// What a content tick counted from the clip's offset at the press becomes with
    /// `base`.
    pub shift: Ticks,
}

/// The arrangement's modifiers this frame (set before its input).
pub fn modifiers(app: &mut CantusApp, mods: egui::Modifiers) {
    app.arrangement.audio.mods = mods;
}

/// The button went down on a warp handle of `r`: a marker to drag, or — on the band or a
/// transient — a marker made there to drag (nothing is written until it moves).
pub fn begin(app: &mut CantusApp, r: ClipRef, handle: Handle) -> Option<MarkerDrag> {
    let slide = app.arrangement.audio.mods.alt;
    let start = app.warp_start(r)?;
    let w = start.warp;
    let (warp0, marker) = match handle {
        // Only the user's markers are offered (the warp's own are never grabbed).
        Handle::WarpMarker(i) if w.markers.get(i as usize).is_some_and(|m| !m.auto) => {
            (w, i as usize)
        }
        Handle::WarpOnset(id) => {
            let f = *app.warp_analysis(r)?.onsets.get(id as usize)?;
            let n = w.add_marker_at_frame(f)?;
            let i = n.markers.iter().position(|m| m.frame == f)?;
            (n, i)
        }
        Handle::WarpBand(t) => {
            let at = Ticks(t as i64) + start.shift;
            let n = w.add_marker(at)?;
            let i = n.marker_at(at)?;
            (n, i)
        }
        _ => return None,
    };
    app.warp.selected = Some((r, marker));
    let a = &app.arrangement;
    Some(MarkerDrag {
        warp0,
        marker,
        slide,
        snap: a.snap.then_some(a.grid),
        base: start.base,
        shift: start.shift,
    })
}

/// The pointer moved `dx` ticks along the timeline: the marker to its new tick (the grid's
/// line under it unless Ctrl), or — sliding — the content under it by as much.
pub fn update(app: &mut CantusApp, r: ClipRef, d: &MarkerDrag, dx: Ticks) {
    let Some(c) = app.project().track(r.0).and_then(|t| t.clip(r.1)) else {
        return;
    };
    let Some(m) = d.warp0.markers.get(d.marker).copied() else {
        return;
    };
    let reverse = c.audio.as_ref().is_some_and(|a| a.reverse);
    // The offset the clip plays from with the drag's warp (its window kept on its audio
    // as it turns to follow, on the first step).
    let offset = if d.base.is_some() && !app.clip_warp(r).is_some_and(|w| w.follow) {
        c.offset + d.shift
    } else {
        c.offset
    };
    // Where the marker is heard, and the content heard at a timeline tick: reversed, its
    // content runs right to left, mirrored through the map (round 29's third review).
    let origin = c.start - offset;
    let o = origin.0 as f64;
    let span = c
        .content_length
        .filter(|p| *p < c.length)
        .unwrap_or(c.length);
    let rev = reverse.then(|| {
        Reversal::new(
            crate::views::bars::map(app.project()),
            c.start,
            c.start + span,
        )
    });
    let heard = |t: f64| rev.as_ref().map_or(o + t, |r| r.heard(o + t));
    let content_at = |h: f64| rev.as_ref().map_or(h - o, |r| r.heard(h) - o);
    let h0 = heard(m.tick.0 as f64);
    let (next, op) = if d.slide {
        let f = d.warp0.frame_at(content_at(h0 - dx.0 as f64).round());
        (
            d.warp0.move_content(d.marker, f),
            cantus_core::warp::WarpOp::MoveContent,
        )
    } else {
        let mut tick = Ticks(content_at(h0 + dx.0 as f64).round() as i64);
        if let Some(g) = d.snap.filter(|_| !app.arrangement.audio.mods.command) {
            let at = crate::views::bars::snap(
                app.project(),
                Ticks((h0 + dx.0 as f64).round() as i64),
                g,
            );
            tick = Ticks(content_at(at.0 as f64).round() as i64);
        }
        (
            d.warp0.move_marker(d.marker, tick),
            cantus_core::warp::WarpOp::MoveMarker,
        )
    };
    let keep = d
        .base
        .as_ref()
        .map_or(crate::warp::Keep::No, crate::warp::Keep::Base);
    app.commit_warp_keeping(r, Some(next), op, true, keep);
}

/// A double-click on a warp handle: a marker removed; on the band or a transient, one added.
pub fn double_click(app: &mut CantusApp, r: ClipRef, handle: Handle) {
    match handle {
        Handle::WarpMarker(i) => {
            app.remove_warp_marker(r, i as usize);
        }
        Handle::WarpOnset(id) => {
            if let Some(f) = app
                .warp_analysis(r)
                .and_then(|a| a.onsets.get(id as usize).copied())
            {
                app.add_warp_marker(r, Ticks::ZERO, Some(f));
            }
        }
        Handle::WarpBand(t) => {
            app.add_warp_marker(r, Ticks(t as i64), None);
        }
        _ => {}
    }
    app.end_gesture();
}

/// The hint for a warp handle (the marker's segment tempo while it is dragged). Returns
/// whether it was one.
pub fn hint(ctx: &egui::Context, app: &CantusApp, r: ClipRef, handle: Handle, live: bool) -> bool {
    use crate::i18n::t;
    use crate::kit::hint::{self, Caps};
    use std::fmt::Write as _;
    match handle {
        Handle::WarpMarker(i) => {
            let speed = marker_speed(app, r, i as usize);
            let caps = Caps {
                menu: true,
                ..Caps::NONE
            };
            hint::set(ctx, &t("warp.marker"), caps, live, |b| match speed {
                Some(k) => write!(b, "×{k:.3}"),
                None => Ok(()),
            });
            true
        }
        Handle::WarpOnset(_) => {
            hint::set_text(
                ctx,
                &t("warp.transient"),
                &t("warp.transient.tip"),
                Caps::NONE,
            );
            true
        }
        Handle::WarpBand(_) => {
            hint::set_text(ctx, &t("warp.band"), &t("warp.band.tip"), Caps::NONE);
            true
        }
        _ => false,
    }
}

/// A click on a clip: a warp marker under it is selected; anywhere else lets it go.
pub fn clicked(app: &mut CantusApp, r: ClipRef, handle: Option<Handle>) {
    app.warp.selected = match handle {
        Some(Handle::WarpMarker(i)) => Some((r, i as usize)),
        _ => None,
    };
}

// ---- the clip menu's warp section ------------------------------------------------------

/// The warp entries of an audio clip's menu (a list of its own, after the audio entries):
/// the modes, *Follow the project tempo*, *Quantise audio…*, *Detect tempo*, *Warp from
/// here*, *Set the project tempo from this clip*, *Edit warp*, *Bounce warped audio*,
/// *Reset warp* — and *Remove warp marker* when the menu was opened on one.
pub fn clip_menu(app: &mut CantusApp, ui: &mut egui::Ui, r: ClipRef) {
    use crate::kit::menu::{Check, Item};
    use crate::kit::text::Src;
    use cantus_core::warp::WarpMode;
    if crate::audio_clips::region(app.project(), r).is_none() {
        return;
    }
    let w = app.clip_warp(r).cloned();
    let warped = w.as_ref().is_some_and(|w| w.follow);
    let mode = w.as_ref().map(|w| w.mode);
    let on_marker = match app.arrangement.audio.menu {
        Some(Handle::WarpMarker(i))
            if w.as_ref().is_some_and(|w| (i as usize) < w.markers.len()) =>
        {
            Some(i as usize)
        }
        _ => None,
    };
    let ready = app.warp_analysis(r).is_some();
    let action = |label: &'static str, check: Check, enabled: bool| Item::Action {
        label: Src::Key(label),
        check,
        shortcut: None,
        submenu: false,
        enabled,
    };
    let mut items = vec![Item::Band(Src::Key("warp.menu.band"))];
    let mut acts: Vec<Option<Pick>> = vec![None];
    // The mode is a setting of its own: picking one does not warp the clip (round 29's
    // review); Follow does, on any audio clip.
    let shown = mode.unwrap_or(WarpMode::Complex);
    for m in WarpMode::ALL {
        items.push(action(mode_key(m), Check::Radio(shown == m), true));
        acts.push(Some(Pick::Mode(m)));
    }
    items.push(action(
        "warp.menu.follow",
        if warped { Check::On } else { Check::Off },
        true,
    ));
    acts.push(Some(Pick::Follow));
    items.push(Item::Separator);
    acts.push(None);
    for (label, pick, enabled) in [
        ("warp.menu.quantise", Pick::Quantise, true),
        ("warp.menu.detect", Pick::Detect, ready),
        ("warp.menu.from_here", Pick::FromHere, ready),
        (
            "warp.menu.project_tempo",
            Pick::ProjectTempo,
            ready || w.is_some(),
        ),
        ("warp.menu.edit", Pick::Edit, true),
        ("warp.menu.bounce", Pick::Bounce, app.can_bounce(r)),
        ("warp.menu.reset", Pick::Reset, w.is_some()),
    ] {
        items.push(action(label, Check::None, enabled));
        acts.push(Some(pick));
    }
    if let Some(i) = on_marker {
        items.push(action("warp.menu.remove_marker", Check::None, true));
        acts.push(Some(Pick::Remove(i)));
    }
    let Some(i) = crate::kit::widgets::menu_list(ui, &items, super::super::MENU_W)
        .1
        .clicked
    else {
        return;
    };
    let Some(Some(pick)) = acts.get(i).copied() else {
        return;
    };
    match pick {
        Pick::Mode(m) => {
            app.set_warp_mode(r, m);
        }
        Pick::Follow => {
            app.set_warp_follow(r, !warped);
        }
        // Every audio clip selected with it, else it alone.
        Pick::Quantise => {
            let sel = app.clip_selection();
            let clips = if sel.contains(&r) { sel } else { vec![r] };
            app.open_quantise(clips);
        }
        Pick::Detect => {
            app.warp_by_analysis(r);
        }
        // From where the clip was right-clicked.
        Pick::FromHere => {
            let at = app.warp.menu_tick.unwrap_or_else(|| app.engine.position());
            app.warp_from_here(r, at);
        }
        Pick::ProjectTempo => {
            app.project_tempo_from_clip(r);
        }
        Pick::Edit => app.open_warp_editor(r),
        Pick::Bounce => {
            app.bounce_warped(r);
        }
        Pick::Reset => {
            app.reset_warp(r);
        }
        Pick::Remove(i) => {
            app.remove_warp_marker(r, i);
        }
    }
    ui.close();
}

/// The words of a mode.
pub fn mode_key(m: cantus_core::warp::WarpMode) -> &'static str {
    use cantus_core::warp::WarpMode;
    match m {
        WarpMode::Complex => "warp.mode.complex",
        WarpMode::Rhythm => "warp.mode.rhythm",
        WarpMode::Melodic => "warp.mode.melodic",
        WarpMode::Repitch => "warp.mode.repitch",
    }
}

#[derive(Copy, Clone, Debug)]
enum Pick {
    Mode(cantus_core::warp::WarpMode),
    Follow,
    Quantise,
    Detect,
    FromHere,
    ProjectTempo,
    Edit,
    Bounce,
    Reset,
    Remove(usize),
}

impl Hits {
    /// The x of every marker drawn (tests, the shots).
    pub fn marker_xs(&self) -> &[f32] {
        &self.xs[..self.n]
    }

    /// The x of every transient drawn.
    pub fn onset_xs(&self) -> &[f32] {
        &self.onset_xs[..self.m]
    }
}

/// Whether a drag of a clip's right edge stretches it (Alt held; ADR 0061 §5).
pub fn stretching(app: &CantusApp) -> bool {
    app.arrangement.audio.mods.alt
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hits() -> Hits {
        let band = Rect::from_min_max(pos2(0.0, 0.0), pos2(1000.0, 7.0));
        let wave = Rect::from_min_max(pos2(0.0, 0.0), pos2(1000.0, 60.0));
        // Content tick 0 at x 0, a tick a point.
        Hits::new(band, wave, 0.0, 1.0, 1.0)
    }

    /// Round 29's second review (findings 46, 49): the markers on a window's start and end
    /// are drawn and offered (the body's edges, outside the inset waveform area); the
    /// warp's own never are.
    #[test]
    fn the_windows_end_markers_are_offered_and_the_warps_own_never() {
        let mut w = Warp::new(48_000, 120.0);
        for (t, auto) in [
            (960i64, false),
            (1_200, true),
            (1_920, false),
            (2_880, false),
        ] {
            w.markers.push(cantus_core::warp::WarpMarker {
                frame: t as f64 * 25.0,
                tick: Ticks(t),
                auto,
            });
        }
        // The window: content 960 to 2880, its body from x 100 to 292 (a tick a tenth of a
        // point); the waveform area inset two points.
        let at = Window {
            offset: Ticks(960),
            length: Ticks(1_920),
            start: Ticks(960),
            reversal: None,
            origin: 100.0 - 96.0,
            ppt: 0.1,
        };
        let inner = Rect::from_min_max(pos2(102.0, 10.0), pos2(290.0, 60.0));
        let lanes = Rect::from_min_max(pos2(0.0, 0.0), pos2(1000.0, 100.0));
        let places = marker_places(&w, &at, inner, lanes, 1.0);
        let ids: Vec<u16> = places.iter().map(|p| p.1).collect();
        assert_eq!(ids, vec![0, 2, 3], "both ends, never the warp's own");
        assert!((places[0].0 - 100.0).abs() < 1e-3 && (places[2].0 - 292.0).abs() < 1e-3);
    }

    /// Round 29's review: on a reversed clip a place on the band is the mirrored content
    /// tick, as the markers are drawn.
    #[test]
    fn a_reversed_clips_band_is_read_mirrored() {
        let mut h = hits();
        assert_eq!(h.tick_at(100.0), Ticks(100));
        // Window: offset 0, length 400 content ticks, under one tempo.
        let map = std::sync::Arc::new(cantus_core::TempoMap::constant(
            120.0,
            cantus_core::TimeSignature::default(),
        ));
        h.mirror = Some(Reversal::new(map, Ticks::ZERO, Ticks(400)).mirror(0.0));
        assert_eq!(h.tick_at(100.0), Ticks(300));
    }

    /// Round 29's review: of ninety markers drawn, the ones by the pointer are offered —
    /// the last as well as the first.
    #[test]
    fn the_handles_offered_are_the_nearest() {
        let all: Vec<(f32, u16)> = (0..90).map(|i| (i as f32 * 10.0, i as u16)).collect();
        let near = nearest(all.clone(), 880.0);
        assert_eq!(near.len(), MAX_HITS);
        assert_eq!(near[0], (880.0, 88));
        assert!(near.iter().any(|h| h.1 == 89));
        assert!(nearest(all, 0.0).iter().any(|h| h.1 == 0));
    }

    /// Round 29's review: a band takes a press only where it is offered.
    #[test]
    fn a_band_not_offered_takes_no_press() {
        let mut h = hits();
        h.offer = false;
        assert_eq!(h.at(pos2(500.0, 3.0)), None);
        h.offer = true;
        assert!(matches!(
            h.at(pos2(500.0, 3.0)),
            Some(Handle::WarpBand(500))
        ));
    }
}
