//! Audio clips in the arrangement (ADR 0025): the waveform in the clip's own ink, the
//! fades drawn over it with their handles, the clip-gain line, the badges in the title
//! bar, and a missing file drawn as what it is.
//!
//! The painter reads, the gestures (`arrangement_audio_edit.rs`) write. Where the handles
//! are is one function, [`handles`], used by the painter, the hit test and the tests —
//! the published geometry of `views::arrangement`'s conventions.

use super::paint::{note_ink, ClipLook};
use super::TimelineGeometry;
use crate::app::CantusApp;
use crate::audio_clips::Fade;
use crate::kit::color::{mix, shade, with_alpha};
use crate::kit::text::{self, Elide, Policy, Role, Text};
use crate::kit::{Brush, Style, Tok};
use crate::waveform::{fill_peaks, Lookup, Which};
use cantus_core::{AudioRegion, Clip, ClipId, Ticks, TrackId};
use egui::{pos2, vec2, Align2, Pos2, Rect};
use std::fmt::Write as _;

#[path = "arrangement_audio_clipfx.rs"]
pub mod clipfx;
#[path = "arrangement_audio_edit.rs"]
pub(super) mod edit;
#[path = "arrangement_warp.rs"]
pub mod warp;

/// A handle on an audio clip.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum Handle {
    FadeIn,
    FadeOut,
    Gain,
    /// A warp marker, by its index (ADR 0061).
    WarpMarker(u16),
    /// A transient in the top band, by its index in the analysis.
    WarpOnset(u32),
    /// The top band, at a content tick (a double-click adds a marker there).
    WarpBand(i32),
    /// The square of the crossfade a clip starts with (ADR 0070).
    Xfade,
    /// A gain envelope's point, by its index (ADR 0070).
    EnvPoint(u16),
    /// The bend's handle of the segment from a point.
    EnvBend(u16),
    /// The envelope's line, at a content tick (a press there adds a point).
    EnvLine(i32),
}

/// Where an audio clip's handles are, in screen points.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Handles {
    pub fade_in: Rect,
    pub fade_out: Rect,
    pub gain: Rect,
    /// The waveform's area (below the title bar).
    pub wave: Rect,
    /// The warp markers and transients drawn on the clip (ADR 0061).
    pub warp: Option<warp::Hits>,
    /// The crossfade the clip starts with (ADR 0070).
    pub xfade: Option<clipfx::XfadeHits>,
    /// Its gain envelope, while the envelopes are shown (ADR 0070).
    pub env: Option<clipfx::EnvHits>,
}

impl Handles {
    /// The handle under `pos`: an envelope's point or bend first (small, and drawn on
    /// top), the crossfade's square, the gain (it sits between the two fades), the fades,
    /// the warp's marks, and the envelope's line last.
    pub fn at(&self, pos: Pos2) -> Option<Handle> {
        let grow = |r: Rect| r.expand(2.0);
        if let Some(h) = self
            .env
            .as_ref()
            .and_then(|e| e.at(pos))
            .filter(|h| !matches!(h, Handle::EnvLine(_)))
        {
            return Some(h);
        }
        if self.xfade.is_some_and(|x| grow(x.square).contains(pos)) {
            return Some(Handle::Xfade);
        }
        if let Some(h) = self.fixed_at(pos) {
            return Some(h);
        }
        self.env.as_ref().and_then(|e| e.at(pos))
    }

    fn fixed_at(&self, pos: Pos2) -> Option<Handle> {
        let grow = |r: Rect| r.expand(2.0);
        if grow(self.gain).contains(pos) {
            Some(Handle::Gain)
        } else if grow(self.fade_in).contains(pos) {
            Some(Handle::FadeIn)
        } else if grow(self.fade_out).contains(pos) {
            Some(Handle::FadeOut)
        } else {
            self.warp.and_then(|w| w.at(pos))
        }
    }
}

/// The side of a handle's square at 100 %.
pub const HANDLE: f32 = 7.0;

/// Where the handles of a clip drawn over `body` (whose waveform area is `inner`) sit:
/// each fade's square at the top of the waveform, at the end of its fade; the gain's at
/// the top centre, on the gain line. `None` when the clip is too small to hold them.
pub fn handles(
    body: Rect,
    inner: Rect,
    a: &AudioRegion,
    px_per_tick: f32,
    k: f32,
) -> Option<Handles> {
    let hs = HANDLE * k;
    if inner.height() < hs * 1.5 || body.width() < hs * 4.0 {
        return None;
    }
    let y = inner.top() + hs * 0.5;
    let fi = (a.fade_in.0 as f32 * px_per_tick).clamp(0.0, body.width());
    let fo = (a.fade_out.0 as f32 * px_per_tick).clamp(0.0, body.width());
    let lo = body.left() + hs * 0.5;
    let hi = body.right() - hs * 0.5;
    let square = |x: f32, y: f32| Rect::from_center_size(pos2(x, y), vec2(hs, hs));
    let gain_y = gain_y(inner, a.gain_db);
    Some(Handles {
        fade_in: square((body.left() + fi).clamp(lo, hi), y),
        fade_out: square((body.right() - fo).clamp(lo, hi), y),
        gain: square(
            body.center().x,
            gain_y.clamp(inner.top() + hs * 0.5, inner.bottom()),
        ),
        wave: inner,
        warp: None,
        xfade: None,
        env: None,
    })
}

/// Where the gain's square sits along a clip whose markers are at `markers` (xs): at
/// `centre` when no marker's line is within a square of it, else in the middle of the gap
/// between markers nearest to it that holds a square — inside `[lo, hi]` (round 29's
/// second review: it covered the centre bar's marker).
pub fn gain_x(centre: f32, markers: &[f32], lo: f32, hi: f32, side: f32) -> f32 {
    if markers.iter().all(|m| (m - centre).abs() >= side) || hi <= lo {
        return centre;
    }
    let mut edges: Vec<f32> = markers
        .iter()
        .copied()
        .filter(|m| *m > lo && *m < hi)
        .collect();
    edges.push(lo - side);
    edges.push(hi + side);
    edges.sort_by(f32::total_cmp);
    edges
        .windows(2)
        .filter(|g| g[1] - g[0] >= 2.0 * side)
        .map(|g| (0.5 * (g[0] + g[1])).clamp(lo, hi))
        .min_by(|a, b| (a - centre).abs().total_cmp(&(b - centre).abs()))
        .unwrap_or(centre)
}

/// A following clip's squares clear of its markers' lines (`markers`, xs) inside
/// `[lo, hi]` (the body less half a square each side): a fade's square on a marker (the
/// window's edge markers lie under them when the fades are short) moved inward past it,
/// one square clear; then the gain's square to the nearest gap that holds it among the
/// markers and the fades' squares — nothing overlaps (round 29's third review).
pub fn clear_squares(h: &mut Handles, markers: &[f32], lo: f32, hi: f32, side: f32) {
    let on = |sq: &Rect| {
        markers
            .iter()
            .copied()
            .filter(|m| *m >= sq.left() - 1.0 && *m <= sq.right() + 1.0)
            .collect::<Vec<f32>>()
    };
    // Inward: the fade in to the right of what it covers, the fade out to the left.
    for _ in 0..markers.len().min(8) {
        let hit = on(&h.fade_in);
        let Some(m) = hit.iter().copied().reduce(f32::max) else {
            break;
        };
        let x = (m + side + 1.0).min(hi);
        h.fade_in = h.fade_in.translate(vec2(x - h.fade_in.center().x, 0.0));
    }
    for _ in 0..markers.len().min(8) {
        let hit = on(&h.fade_out);
        let Some(m) = hit.iter().copied().reduce(f32::min) else {
            break;
        };
        let x = (m - side - 1.0).max(lo);
        h.fade_out = h.fade_out.translate(vec2(x - h.fade_out.center().x, 0.0));
    }
    let mut taken: Vec<f32> = markers.to_vec();
    taken.extend([h.fade_in.center().x, h.fade_out.center().x]);
    let x = gain_x(h.gain.center().x, &taken, lo, hi, side + 1.0);
    h.gain = h.gain.translate(vec2(x - h.gain.center().x, 0.0));
}

/// The y of the gain line: where a full-scale peak lands at this gain (the top at 0 dB
/// and above).
pub fn gain_y(wave: Rect, db: f32) -> f32 {
    let lin = cantus_core::db_to_linear(db).min(1.0);
    let half = wave.height() * 0.5;
    wave.center().y - half * lin
}

/// What the painter needs of a clip, as it is drawn this frame (a drag's preview moves
/// the window).
#[derive(Copy, Clone)]
pub struct Shown<'a> {
    pub track: TrackId,
    pub clip: &'a Clip,
    pub region: &'a AudioRegion,
    pub start: Ticks,
    pub length: Ticks,
    pub offset: Ticks,
    /// The project's tempo map: where the source plays (ADR 0025 §1, ADR 0060,
    /// `cantus_core::audio_time`).
    pub map: &'a cantus_core::TempoMap,
}

/// Buffers reused frame to frame, and the handles drawn this frame (published).
#[derive(Debug, Default)]
pub struct Scratch {
    cols: [Vec<(f32, f32)>; 2],
    /// A warped clip's run of columns (ADR 0061).
    run: Vec<(f32, f32)>,
    pts: Vec<Pos2>,
    tops: Vec<Pos2>,
    badge: String,
    pub handles: Vec<((TrackId, ClipId), Handles)>,
    /// The tracks' overlaps this frame (ADR 0070).
    xcache: clipfx::Cache,
}

/// The width the badges (gain, reverse) take at the title bar's right, so the name is
/// elided before them.
pub fn badge_width(b: &Brush, a: &AudioRegion, scratch: &mut Scratch, s: &Style) -> f32 {
    badge_text(a, &mut scratch.badge);
    if scratch.badge.is_empty() {
        return 0.0;
    }
    let tx = Text::new(&scratch.badge, Role::Micro, s.t.text).caps(false);
    text::measure(b.ctx(), &tx, s.tk()).x + 8.0 * s.k
}

fn badge_text(a: &AudioRegion, out: &mut String) {
    out.clear();
    if a.gain_db.abs() >= 0.05 {
        let _ = write!(out, "{:+.1} dB", a.gain_db);
    }
    if a.reverse {
        if !out.is_empty() {
            out.push_str("  ");
        }
        out.push_str(&crate::i18n::t("audioclips.badge.reverse"));
    }
    // Round 28 (ADR 0054): the clip plays its pitch edits (or is compared with the take).
    if let Some(p) = a.pitch.as_deref().filter(|p| p.edited()) {
        if !out.is_empty() {
            out.push_str("  ");
        }
        out.push_str(&crate::i18n::t(if p.bypass {
            "pitch.badge.original"
        } else {
            "pitch.badge"
        }));
    }
    // Round 29 (ADR 0061): warped (its mode), transposed (semitones and cents).
    if let Some(w) = a.warp.as_deref() {
        if !out.is_empty() {
            out.push_str("  ");
        }
        if w.follow {
            out.push_str(&crate::i18n::t(&format!("warp.badge.{}", w.mode.id())));
        }
        if w.cents != 0 && w.mode.transposes() {
            if w.follow {
                out.push(' ');
            }
            crate::warp::write_cents(out, w.cents);
        }
    }
}

/// Paint an audio clip's body: the waveform, the fades, the gain line, the badges, the
/// handles (`hot`: hovered or selected), or the missing file.
#[allow(clippy::too_many_arguments)]
pub fn paint(
    b: &mut Brush,
    app: &CantusApp,
    geo: &TimelineGeometry,
    lanes: Rect,
    shown: &Shown<'_>,
    body: Rect,
    inner: Rect,
    look: &ClipLook,
    pointer: Option<Pos2>,
    dragging: Option<Handle>,
    show_handles: bool,
    scratch: &mut Scratch,
    s: &Style,
) -> Option<Handles> {
    // An Alt-drag of the right edge stretches the clip (ADR 0061): drawn stretched.
    let stretched_region;
    let stretched;
    let shown = match app
        .arrangement
        .stretch_preview((shown.track, shown.clip.id))
        .zip(app.warp_start((shown.track, shown.clip.id)))
    {
        Some((delta, s)) if shown.clip.length.0 > 0 => {
            let k = (shown.clip.length + delta).0 as f64 / shown.clip.length.0 as f64;
            let mut r = shown.region.clone();
            let offset = shown.offset + s.shift;
            r.warp = Some(Box::new(s.warp.stretched(offset, k.max(1e-3))));
            stretched_region = r;
            stretched = Shown {
                region: &stretched_region,
                offset,
                ..*shown
            };
            &stretched
        }
        _ => shown,
    };
    let k = s.k;
    let project = app.project();
    b.register(
        inner,
        "audio_clip",
        &[Tok::Danger, Tok::Highlight, Tok::Shadow],
        &[],
    );
    let ink = note_ink(look, s);
    let px_per_tick = geo.px_per_beat / cantus_core::PPQ as f32;
    let source = project.audio_source(shown.region.source);
    let look_up = source.map(|src| app.peaks.lookup(src, app.audio_base(), Some(b.ctx())));
    let title_h = super::paint::title_height(body, k);
    // The badges, at the title bar's right (the name was elided before them).
    if title_h > 0.0 {
        badge_text(shown.region, &mut scratch.badge);
        if !scratch.badge.is_empty() {
            let bar = Rect::from_min_max(body.min, pos2(body.right(), body.top() + title_h));
            let right = body.right() - 4.0 * k;
            let r = Rect::from_min_max(
                pos2(bar.left() + 4.0 * k, bar.top()),
                pos2(right, bar.bottom()),
            );
            let tx = Text::new(&scratch.badge, Role::Micro, look.tint.on_large)
                .caps(false)
                .align(Align2::RIGHT_CENTER)
                .policy(Policy::Elide(Elide::End));
            let _ = text::paint(b, r, &tx, s.tk());
        }
    }
    // How it takes part in its track's overlaps (ADR 0070).
    let crossing = clipfx::crossing(app, &mut scratch.xcache, shown);
    match look_up {
        Some(Lookup::Ready(peaks)) => {
            wave(
                b, geo, lanes, shown, inner, &peaks, ink, &crossing, scratch, s,
            );
        }
        Some(Lookup::Pending) => {
            b.hline(
                inner.left(),
                inner.right(),
                inner.center().y,
                b.px(),
                with_alpha(ink, 0.5),
            );
        }
        None | Some(Lookup::Missing) | Some(Lookup::Failed(_)) => {
            missing(
                b,
                lanes,
                shown,
                inner,
                look,
                source.map(|s| s.path.as_str()),
                s,
            );
        }
    }
    // Not in the engine yet (a long file converting on the loader thread): silent, and
    // drawn so. `loading()` first: no lookup at all while nothing loads.
    if app.engine.samples.loading()
        && source.is_some_and(|src| {
            app.engine.samples.audio_status(&src.path) == cantus_engine::SampleStatus::Loading
        })
    {
        parts::loading(b, lanes, inner, look, s);
    }
    // A fade a crossfade replaces is not drawn, nor offered (ADR 0070).
    let keep = (
        !matches!(crossing.into, Some((_, _, Some(_), _))),
        crossing.out_of.is_none(),
    );
    fades(b, shown, body, inner, px_per_tick, ink, keep, scratch, s);
    let mut h = handles(body, inner, shown.region, px_per_tick, k)?;
    if !keep.0 {
        h.fade_in = Rect::NOTHING;
    }
    if !keep.1 {
        h.fade_out = Rect::NOTHING;
    }
    // The warp markers and transients (ADR 0061), over the waveform and the fades.
    h.warp = warp::paint(b, app, geo, lanes, shown, inner, show_handles, false, s);
    // On a warped clip the band holds the markers' flags: the fade and gain squares sit
    // below it, so neither covers a flag (a clip not warped keeps its layout).
    if let Some(band) = h
        .warp
        .filter(|_| shown.region.warp.as_deref().is_some_and(|w| w.follow))
        .map(|w| w.band)
    {
        let floor = band.bottom() + k;
        for sq in [&mut h.fade_in, &mut h.fade_out, &mut h.gain] {
            let dy = (floor - sq.top())
                .min(inner.bottom() - sq.bottom())
                .max(0.0);
            *sq = sq.translate(vec2(0.0, dy));
        }
        // And the gain's square never on a marker's line (a loop's centre bar holds one —
        // round 29's second review): moved to the nearest gap between them.
        if let Some(w) = shown.region.warp.as_deref() {
            let ppt = geo.px_per_beat / cantus_core::PPQ as f32;
            let at = warp::Window {
                offset: shown.offset,
                length: shown.length,
                start: shown.start,
                reversal: shown.region.reverse.then(|| {
                    warp::Reversal::new(
                        std::sync::Arc::new(shown.map.clone()),
                        shown.start,
                        shown.start + shown.length,
                    )
                }),
                origin: geo.x(shown.start.as_beats_f64() as f32) - shown.offset.0 as f32 * ppt,
                ppt,
            };
            let xs: Vec<f32> = warp::marker_places(w, &at, inner, lanes, k)
                .into_iter()
                .map(|p| p.0)
                .collect();
            let hs = HANDLE * k;
            clear_squares(
                &mut h,
                &xs,
                body.left() + hs * 0.5,
                body.right() - hs * 0.5,
                hs,
            );
        }
    }
    // The crossfade it starts with, and its gain envelope (ADR 0070), over the rest.
    let hot = dragging.or_else(|| pointer.and_then(|p| h.at(p)));
    h.xfade = clipfx::paint_xfade(
        b,
        geo,
        shown,
        &crossing,
        inner,
        &[h.gain, h.fade_in, h.fade_out],
        hot,
        s,
    );
    if app.settings.show_clip_gain {
        h.env = clipfx::paint_env(b, geo, lanes, shown, inner, show_handles, hot, s);
    }
    scratch.handles.push(((shown.track, shown.clip.id), h));
    if show_handles {
        paint_handles(b, &h, shown.region, hot, ink, s);
    }
    Some(h)
}

/// The waveform: one strip per channel (two halves for a stereo file when the clip is
/// tall enough), a column per physical pixel, with the clip's gain and fades applied.
#[allow(clippy::too_many_arguments)]
fn wave(
    b: &mut Brush,
    geo: &TimelineGeometry,
    lanes: Rect,
    shown: &Shown<'_>,
    inner: Rect,
    peaks: &crate::waveform::SourcePeaks,
    ink: egui::Color32,
    crossing: &clipfx::Crossing,
    scratch: &mut Scratch,
    s: &Style,
) {
    let x0 = inner.left().max(lanes.left());
    let x1 = inner.right().min(lanes.right());
    if x1 <= x0 || inner.height() < 2.0 {
        return;
    }
    let ppp = b.ppp();
    let n = (((x1 - x0) * ppp).ceil() as usize).clamp(1, 8192);
    let col_w = (x1 - x0) / n as f32;
    let bpm = shown.map.bpm_at(0.0).max(1.0);
    let rate = peaks.rate.max(1) as f64;
    // Frames per beat of timeline, and the content beat at x0.
    let frames_per_beat = 60.0 / bpm * rate;
    let start_b = shown.start.as_beats_f64();
    let d0 = geo.beats(x0) as f64 - start_b; // beats into the clip at x0
    let d1 = geo.beats(x1) as f64 - start_b;
    let off_b = shown.offset.as_beats_f64();
    let len_b = shown.length.as_beats_f64();
    let reverse = shown.region.reverse;
    let (from_b, to_b) = if reverse {
        (off_b + len_b - d1, off_b + len_b - d0)
    } else {
        (off_b + d0, off_b + d1)
    };
    // The region's fine offset, in the peaks' frames (the source's rate).
    let from = from_b * frames_per_beat + shown.region.slip_frames as f64;
    let per = (to_b - from_b) * frames_per_beat / n as f64;
    let stereo = peaks.channels > 1 && inner.height() >= 44.0 * s.k;
    let lanes_n = if stereo { 2 } else { 1 };
    for (c, out) in scratch.cols.iter_mut().enumerate().take(lanes_n) {
        let which = if stereo {
            Which::Channel(c)
        } else {
            Which::Mono
        };
        match shown.region.warp.as_deref().filter(|w| w.follow) {
            // A warped clip (ADR 0061): read through its warp, a run of columns per
            // segment (it mirrors a reversed window itself).
            Some(w) => warp::columns(
                peaks,
                which,
                geo,
                shown,
                w,
                x0,
                x1,
                n,
                out,
                &mut scratch.run,
            ),
            None => {
                if shown.map.is_constant() {
                    peaks.columns(which, from, per, n, out);
                } else {
                    // Through the tempo map (ADR 0060): each column's own stretch of the
                    // source, where the engine plays it.
                    map_columns(peaks, which, shown, geo, x0, col_w, n, out);
                }
                if reverse {
                    out.reverse();
                }
            }
        }
    }
    let gain = cantus_core::db_to_linear(shown.region.gain_db);
    let px_per_tick = geo.px_per_beat as f64 / cantus_core::PPQ as f64;
    let column_gain = |i: usize| {
        let x = x0 as f64 + (i as f64 + 0.5) * col_w as f64;
        let d = Ticks(((x - geo.x(shown.start.as_beats_f64() as f32) as f64) / px_per_tick) as i64);
        clipfx::column_gain(shown, crossing, gain, d)
    };
    let h = inner.height() / lanes_n as f32;
    let fill = with_alpha(ink, 0.92);
    for c in 0..lanes_n {
        let band = Rect::from_min_max(
            pos2(x0, inner.top() + h * c as f32),
            pos2(x1, inner.top() + h * (c + 1) as f32),
        );
        let band = if stereo {
            band.shrink2(vec2(0.0, 1.0))
        } else {
            band
        };
        b.hline(x0, x1, band.center().y, b.px(), with_alpha(ink, 0.28));
        fill_peaks(b, band, &scratch.cols[c], column_gain, fill);
        if stereo && c == 0 {
            b.hline(
                x0,
                x1,
                inner.top() + h,
                b.px(),
                with_alpha(s.t.shadow, 0.45),
            );
        }
    }
}

/// Called once per frame before the clips are painted.
pub fn begin_frame(scratch: &mut Scratch) {
    scratch.handles.clear();
    scratch.xcache.clear();
}

#[path = "arrangement_audio_paint.rs"]
mod parts;
pub use parts::curve_icon;
use parts::{fades, missing, paint_handles};

/// The fade a handle sets.
pub fn fade_of(h: Handle) -> Option<Fade> {
    match h {
        Handle::FadeIn => Some(Fade::In),
        Handle::FadeOut => Some(Fade::Out),
        _ => None,
    }
}

/// A darker body for a missing clip, from the clip's own colours.
pub(super) fn missing_ground(look: &ClipLook, s: &Style) -> egui::Color32 {
    let ground = if s.t.dark {
        s.t.row_black
    } else {
        s.t.row_white
    };
    shade(mix(look.tint.large, ground, 0.8), -0.04)
}

/// The source frame a **reversed** window from `start` to `end` plays at `tick`, as the
/// engine reads it: the frame at its start plus the seconds from `tick` to its end —
/// mirrored in seconds, not in ticks (review 3).
pub fn reversed_frame(
    map: &cantus_core::TempoMap,
    start: Ticks,
    offset: Ticks,
    slip: f64,
    rate: f64,
    end: f64,
    tick: f64,
) -> f64 {
    let first = cantus_core::audio_time::frame_at(map, start, offset, slip, rate, start.0 as f64);
    first + map.seconds_between_f64(tick, end) * rate
}

/// The waveform's columns of a clip under a tempo map that changes (ADR 0060): each
/// column reads the source frames `cantus_core::audio_time` puts under its two edges
/// (backwards, the frames the engine reads there: [`reversed_frame`]).
#[allow(clippy::too_many_arguments)]
fn map_columns(
    peaks: &crate::waveform::SourcePeaks,
    which: Which,
    shown: &Shown,
    geo: &TimelineGeometry,
    x0: f32,
    col_w: f32,
    n: usize,
    out: &mut Vec<(f32, f32)>,
) {
    use cantus_core::audio_time::frame_at;
    let rate = peaks.rate.max(1) as f64;
    let slip = shown.region.slip_frames as f64;
    let end = (shown.start + shown.length).0 as f64;
    let frame = |tick: f64| {
        if shown.region.reverse {
            reversed_frame(shown.map, shown.start, shown.offset, slip, rate, end, tick)
        } else {
            frame_at(shown.map, shown.start, shown.offset, slip, rate, tick)
        }
    };
    let tick_at = |x: f32| geo.beats(x) as f64 * cantus_core::PPQ as f64;
    out.clear();
    let mut one = Vec::with_capacity(1);
    for i in 0..n {
        let (a, b) = (x0 + i as f32 * col_w, x0 + (i + 1) as f32 * col_w);
        let (fa, fb) = (frame(tick_at(a)), frame(tick_at(b)));
        let (lo, hi) = if fa <= fb { (fa, fb) } else { (fb, fa) };
        peaks.columns(which, lo, (hi - lo).max(1e-6), 1, &mut one);
        out.push(one.first().copied().unwrap_or((0.0, 0.0)));
    }
    if shown.region.reverse {
        // `wave` reverses the columns of a backward clip after reading them in order.
        out.reverse();
    }
}

#[cfg(test)]
mod tests {
    use super::{clear_squares, gain_x, handles};
    use cantus_core::{AudioRegion, Ticks};
    use egui::{pos2, Rect};

    /// Round 29's second review (finding 54): the gain's square leaves a marker's line at
    /// the clip's centre for the nearest gap that holds it; with none near, it stays.
    #[test]
    fn the_gain_square_never_sits_on_a_marker() {
        let side = 7.0;
        // A four-bar loop 400 points wide, a marker on each bar: one at the centre.
        let markers = [0.0, 100.0, 200.0, 300.0, 400.0];
        let x = gain_x(200.0, &markers, 7.0, 393.0, side);
        assert!(markers.iter().all(|m| (m - x).abs() >= side), "{x}");
        assert!((x - 150.0).abs() < 1e-3 || (x - 250.0).abs() < 1e-3, "{x}");
        // No marker near the centre: it stays.
        assert_eq!(gain_x(200.0, &[0.0, 400.0], 7.0, 393.0, side), 200.0);
    }

    /// Round 29's third review (finding 40): on a following loop with markers on its
    /// edges and its centre bar, short fades and the gain where the fade in ends, no
    /// square covers a marker's line and none covers another.
    #[test]
    fn no_square_sits_on_a_marker_or_on_another() {
        let side = 7.0;
        let body = Rect::from_min_max(pos2(100.0, 0.0), pos2(500.0, 60.0));
        let inner = body.shrink(2.0);
        let mut a = AudioRegion::of(cantus_core::AudioSourceId(1));
        // The fade in ends at bar 2.5 (x 250), where the gain moves to.
        a.fade_in = Ticks(150 * 10);
        let mut h = handles(body, inner, &a, 0.1, 1.0).unwrap();
        let markers = [100.0, 200.0, 300.0, 400.0, 500.0];
        clear_squares(
            &mut h,
            &markers,
            100.0 + side * 0.5,
            500.0 - side * 0.5,
            side,
        );
        let squares = [h.fade_in, h.fade_out, h.gain];
        for (i, sq) in squares.iter().enumerate() {
            for m in markers {
                assert!(
                    m < sq.left() - 0.5 || m > sq.right() + 0.5,
                    "square {i} {sq:?} on the marker at {m}"
                );
            }
            for other in &squares[i + 1..] {
                assert!(!sq.intersects(*other), "{sq:?} on {other:?}");
            }
        }
    }
}
