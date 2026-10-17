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
//! select it (a click) and press Del. A clip not yet warped is warped by its first marker.
//! **Alt**-drag a clip's right edge stretches the whole clip (`clips.rs`, ADR 0061 §5).

use super::{Handle, Shown};
use super::super::TimelineGeometry;
use crate::app::CantusApp;
use crate::clips::ClipRef;
use crate::kit::color::with_alpha;
use crate::kit::{Brush, Style};
use crate::waveform::{SourcePeaks, Which};
use cantus_core::warp::Warp;
use cantus_core::{Ticks, PPQ};
use egui::{pos2, Pos2, Rect};

/// Markers and transients a clip can offer the pointer per frame (beyond, the nearest are
/// kept: closer than this they are too dense to grab one by one).
pub const MAX_HITS: usize = 64;
/// The top band's height at 100 %.
pub const BAND: f32 = 7.0;
/// How near the pointer must be to a marker or a transient, points at 100 %.
pub const REACH: f32 = 4.0;

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
        }
    }

    /// The content tick under x.
    pub fn tick_at(&self, x: f32) -> Ticks {
        Ticks(((x - self.origin) / self.px_per_tick.max(1e-9)).round() as i64)
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
        if self.band.expand2(egui::vec2(0.0, 1.0)).contains(pos) {
            if let Some(i) = near(&self.onset_xs[..self.m]) {
                return Some(Handle::WarpOnset(self.onset_ids[i]));
            }
            return Some(Handle::WarpBand(self.tick_at(pos.x).0.clamp(i32::MIN as i64, i32::MAX as i64) as i32));
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
    let len = shown.length.0 as f64;
    let reverse = shown.region.reverse;
    let off = shown.offset.0 as f64;
    // Read (reversed: mirrored inside the window) through the warp.
    let frame = |x: f64| {
        let c = content(x);
        let c = if reverse { 2.0 * off + len - c } else { c };
        w.frame_at(c) * scale
    };
    let mut i = 0usize;
    while i < n {
        // The run: columns up to the next marker's x (where the slope changes).
        let xa = x0 as f64 + i as f64 * col_w;
        let ca = content(xa + 0.5 * col_w);
        let next = w
            .markers
            .iter()
            .map(|m| m.tick.0 as f64)
            .map(|t| if reverse { 2.0 * off + len - t } else { t })
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
    s: &Style,
) -> Option<Hits> {
    let k = s.k;
    if inner.height() < BAND * k * 3.0 {
        return None;
    }
    let r: ClipRef = (shown.track, shown.clip.id);
    let band = Rect::from_min_max(inner.min, pos2(inner.right(), inner.top() + BAND * k));
    let ppt = geo.px_per_beat / PPQ as f32;
    let origin = geo.x(shown.start.as_beats_f64() as f32) - shown.offset.0 as f32 * ppt;
    let mut hits = Hits::new(band, inner, origin, ppt, k);
    let warp = shown.region.warp.as_deref();
    let selected = app.warp.selected.filter(|(c, _)| *c == r).map(|(_, i)| i);
    let x_of = |content: f64| origin + content as f32 * ppt;
    let visible = |x: f32| x >= inner.left().max(lanes.left()) && x <= inner.right().min(lanes.right());
    // The transients, when the analysis is in.
    if show {
        if let Some(a) = app.warp_analysis(r) {
            let map = app.project().tempo_map();
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
                let t = if shown.region.reverse { lo + hi - t } else { t };
                if t < lo || t > hi {
                    continue;
                }
                let x = geo.x((t / PPQ as f64) as f32);
                if !visible(x) {
                    continue;
                }
                b.vline(x, band.bottom() - BAND * k * 0.6, band.bottom(), b.px(), ink);
                if hits.m < MAX_HITS {
                    hits.onset_xs[hits.m] = x;
                    hits.onset_ids[hits.m] = id as u32;
                    hits.m += 1;
                }
            }
        }
        b.hline(band.left(), band.right(), band.bottom(), b.px(), with_alpha(s.t.text_dim, 0.25));
    }
    // The markers.
    if let Some(w) = warp.filter(|w| w.follow) {
        let off = shown.offset.0 as f64;
        let len = shown.length.0 as f64;
        for (i, m) in w.markers.iter().enumerate() {
            let c = m.tick.0 as f64;
            if c < off || c > off + len {
                continue;
            }
            let c = if shown.region.reverse { 2.0 * off + len - c } else { c };
            let x = x_of(c);
            if !visible(x) {
                continue;
            }
            let on = selected == Some(i);
            let ink = if on { s.t.highlight } else { s.t.brand_bright };
            b.vline(x, band.top(), inner.bottom(), b.px() * if on { 2.0 } else { 1.0 }, with_alpha(ink, if on { 0.95 } else { 0.7 }));
            let hw = BAND * k * 0.6;
            b.polygon(
                &[
                    pos2(x - hw, band.top()),
                    pos2(x + hw, band.top()),
                    pos2(x, band.bottom()),
                ],
                ink,
            );
            if hits.n < MAX_HITS {
                hits.xs[hits.n] = x;
                hits.ids[hits.n] = i as u16;
                hits.n += 1;
            }
        }
    }
    Some(hits)
}
