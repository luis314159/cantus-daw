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
use crate::audio_clips::{fade_gain_at, Fade};
use crate::kit::color::{mix, shade, with_alpha};
use crate::kit::text::{self, Elide, Policy, Role, Text};
use crate::kit::{Brush, Style, Tok};
use crate::waveform::{fill_peaks, Lookup, Which};
use cantus_core::{AudioRegion, Clip, ClipId, Ticks, TrackId};
use egui::{pos2, vec2, Align2, Pos2, Rect};
use std::fmt::Write as _;

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
}

impl Handles {
    /// The handle under `pos`, the gain first (it sits between the two fades).
    pub fn at(&self, pos: Pos2) -> Option<Handle> {
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
    })
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
pub struct Shown<'a> {
    pub track: TrackId,
    pub clip: &'a Clip,
    pub region: &'a AudioRegion,
    pub start: Ticks,
    pub length: Ticks,
    pub offset: Ticks,
    /// The project's tempo: content ticks to seconds (ADR 0025 §1).
    pub bpm: f64,
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
    match look_up {
        Some(Lookup::Ready(peaks)) => {
            wave(b, geo, lanes, shown, inner, &peaks, ink, scratch, s);
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
    fades(b, shown, body, inner, px_per_tick, ink, scratch, s);
    let mut h = handles(body, inner, shown.region, px_per_tick, k)?;
    // The warp markers and transients (ADR 0061), over the waveform and the fades.
    h.warp = warp::paint(b, app, geo, lanes, shown, inner, show_handles, s);
    scratch.handles.push(((shown.track, shown.clip.id), h));
    if show_handles {
        let hot = dragging.or_else(|| pointer.and_then(|p| h.at(p)));
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
    let bpm = shown.bpm.max(1.0);
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
            // A warped clip: read through its warp, a run of columns per segment.
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
                peaks.columns(which, from, per, n, out);
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
        gain * fade_gain_at(shown.region, shown.length, d)
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
