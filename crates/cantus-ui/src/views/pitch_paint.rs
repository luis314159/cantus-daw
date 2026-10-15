//! The pitch editor's painting: rows and the keyboard, the bar ladder and the ruler, the
//! waveform, the sibilant and breath marks, the blobs and the pitch curves. Everything is
//! read here, nothing edited; only what is in view is drawn.

use super::Geo;
use crate::app::CantusApp;
use crate::kit::color::{mix, on_fill, shade, with_alpha, Hsl};
use crate::kit::text::{self, Elide, Policy, Role, Text};
use crate::kit::{self, Brush, Fill, Style, Tok};
use crate::pitch::{ClipTime, Take};
use cantus_core::pitch::curves::{time_points, Curves};
use cantus_core::pitch::PitchEdit;
use cantus_core::{pitch_name, ClipId, TrackId};
use cantus_dsp::psola::TimeMap;
use cantus_engine::pitch::{FrameKind, TakeAnalysis};
use egui::{pos2, vec2, Align2, Color32, Pos2, Rect};

const GRID_TOKENS: &[Tok] = &[
    Tok::RowWhite,
    Tok::RowBlack,
    Tok::Grid,
    Tok::GridStrong,
    Tok::GridBar,
];
const KEY_TOKENS: &[Tok] = &[Tok::KeyWhite, Tok::KeyBlack, Tok::Text];
const BLOB_TOKENS: &[Tok] = &[Tok::Selection, Tok::Text, Tok::TextDim];

fn is_black(p: i32) -> bool {
    matches!(p.rem_euclid(12), 1 | 3 | 6 | 8 | 10)
}

/// The ground and a line of text while there are no notes to show: the take being
/// analysed, a file that could not be read.
pub(super) fn waiting(ui: &egui::Ui, rect: Rect, take: &Take, s: &Style) {
    let msg = match take {
        Take::Pending => crate::i18n::t("pitch.analysing"),
        Take::Failed(e) => crate::i18n::tf("pitch.failed", &[("reason", e)]),
        _ => crate::i18n::t("pitch.no_source"),
    };
    kit::with_brush(&ui.painter().with_clip_rect(rect), |b| {
        b.register(rect, "pitch_waiting", &[Tok::RowWhite, Tok::TextDim], &[]);
        b.rect(b.snap_rect(rect), Fill::Solid(s.t.row_white));
        let tx = Text::new(&msg, Role::Body, s.t.text_dim)
            .align(Align2::CENTER_CENTER)
            .policy(Policy::Elide(Elide::End));
        let _ = text::paint(b, rect.shrink(s.s(16.0)), &tx, s.tk());
    });
}

/// The ink of the blobs, from the track's colour (the piano roll's recipe).
#[derive(Copy, Clone)]
pub(super) struct Ink {
    pub fill: Color32,
    pub edge: Color32,
    pub label: Color32,
}

pub(super) fn ink(c: Color32, selected: bool, s: &Style) -> Ink {
    if selected {
        let c = s.t.selection;
        return Ink {
            fill: shade(c, 0.02),
            edge: shade(c, -0.32),
            label: on_fill(c),
        };
    }
    let h = Hsl::of(c);
    let sat = h.s.clamp(0.35, 0.75);
    let at = |l: f32| Hsl { h: h.h, s: sat, l }.rgb();
    if s.t.dark {
        Ink {
            fill: at(0.80),
            edge: at(0.50),
            label: at(0.24),
        }
    } else {
        Ink {
            fill: at(0.68),
            edge: at(0.38),
            label: at(0.14),
        }
    }
}

/// Everything in the canvas. Returns each blob's box on screen (for the input).
#[allow(clippy::too_many_arguments)]
pub(super) fn canvas(
    app: &CantusApp,
    ui: &egui::Ui,
    track: TrackId,
    clip: ClipId,
    edit: &PitchEdit,
    analysis: Option<&TakeAnalysis>,
    geo: &Geo,
    s: &Style,
) -> Vec<(u32, Rect)> {
    let color = app
        .project()
        .track(track)
        .map(|t| crate::theme::track_color(t.color))
        .unwrap_or(s.t.brand);
    let time = app.clip_time(track, clip);
    let window = app
        .project()
        .track(track)
        .and_then(|t| t.clip(clip))
        .and_then(|c| time.map(|tm| (tm.frame(c.start), tm.frame(c.end()))));
    let map = TimeMap::new(time_points(edit)).unwrap_or_default();
    let curves = analysis.map(|a| Curves::new(edit, &a.frames_view()));
    let g = geo.grid;
    let blobs = kit::with_brush(&ui.painter().with_clip_rect(g), |b| {
        rows(b, geo, s);
        if let Some(w) = window {
            outside(b, geo, w, s);
        }
        if let Some(tm) = time {
            ladder(b, app, geo, &tm, s);
        }
        waveform(b, app, track, clip, geo, &map, color, s);
        if let Some(a) = analysis {
            marks(b, geo, a, &map, s);
        }
        let blobs = blobs(
            b,
            geo,
            edit,
            analysis,
            &map,
            &app.pitch.view.selected,
            color,
            s,
        );
        if let (Some(a), Some(c)) = (analysis, curves.as_ref()) {
            pitch_curves(b, geo, edit, a, c, &map, s);
        }
        if let Some(super::Gesture::Marquee { from, to }) = &app.pitch.view.gesture {
            let r = Rect::from_two_pos(*from, *to);
            b.rect(r, Fill::Solid(with_alpha(s.t.selection, 0.15)));
            b.rrect_stroke(r, 0.0, b.px(), s.t.selection);
        }
        if let Some(tm) = time {
            let x = geo.x(tm.frame(app.engine.position()));
            if x >= g.left() && x <= g.right() {
                b.vline(x, g.top(), g.bottom(), 1.5 * s.k, s.t.playhead);
            }
        }
        blobs
    });
    kit::with_brush(&ui.painter().with_clip_rect(geo.keys), |b| keys(b, geo, s));
    kit::with_brush(&ui.painter().with_clip_rect(geo.ruler), |b| {
        ruler(b, app, geo, time.as_ref(), s)
    });
    blobs
}

/// The rows: black-key rows darker, a line under every C.
fn rows(b: &mut Brush, geo: &Geo, s: &Style) {
    let t = &s.t;
    let g = b.snap_rect(geo.grid);
    b.register(g, "pitch_grid", GRID_TOKENS, &[]);
    b.rect(g, Fill::Solid(t.row_white));
    let lo = geo.midi(g.bottom()).floor() as i32;
    let hi = geo.midi(g.top()).ceil() as i32;
    for p in lo..=hi {
        let (y0, y1) = (geo.y(p as f32 + 0.5), geo.y(p as f32 - 0.5));
        if is_black(p) {
            b.rect(
                b.snap_rect(Rect::from_min_max(pos2(g.left(), y0), pos2(g.right(), y1))),
                Fill::Solid(t.row_black),
            );
        }
        if p.rem_euclid(12) == 0 {
            b.hline(
                g.left(),
                g.right(),
                y1 - 0.5 * b.px(),
                b.px(),
                t.grid_strong,
            );
        }
    }
    kit::light::ambient(b, g, t);
}

/// Outside the clip's window: "not heard from this clip", a step darker.
fn outside(b: &mut Brush, geo: &Geo, (a, z): (f64, f64), s: &Style) {
    let g = geo.grid;
    let veil = with_alpha(shade(s.t.row_black, -0.07), 0.7);
    let (xa, xz) = (geo.x(a), geo.x(z));
    if xa > g.left() {
        b.rect(
            b.snap_rect(Rect::from_min_max(
                g.min,
                pos2(xa.min(g.right()), g.bottom()),
            )),
            Fill::Solid(veil),
        );
    }
    if xz < g.right() {
        b.rect(
            b.snap_rect(Rect::from_min_max(pos2(xz.max(g.left()), g.top()), g.max)),
            Fill::Solid(veil),
        );
    }
}

/// The bars and beats of the arrangement, over the clip's frames.
fn ladder(b: &mut Brush, app: &CantusApp, geo: &Geo, tm: &ClipTime, s: &Style) {
    let t = &s.t;
    let g = geo.grid;
    let ts = app.project().time_signature;
    let (bar, beat) = (ts.ticks_per_bar().0.max(1), ts.ticks_per_beat().0.max(1));
    // Ordered: a clip played backwards runs its bars right to left.
    let (x0, x1) = (
        tm.tick(geo.frame(g.left())).0,
        tm.tick(geo.frame(g.right())).0,
    );
    let (t0, t1) = (x0.min(x1), x0.max(x1));
    let px_beat =
        (geo.x(tm.frame(cantus_core::Ticks(beat))) - geo.x(tm.frame(cantus_core::Ticks(0)))).abs();
    let step = if px_beat >= 8.0 { beat } else { bar };
    let side = mix(t.grid_bar, t.row_black, 0.55);
    let mut k = t0.div_euclid(step) * step;
    while k <= t1 {
        let x = geo.x(tm.frame(cantus_core::Ticks(k)));
        if k % bar == 0 {
            b.vline(x, g.top(), g.bottom(), 3.0 * b.px(), side);
            b.vline(x, g.top(), g.bottom(), b.px(), t.grid_bar);
        } else {
            b.vline(x, g.top(), g.bottom(), b.px(), t.grid_strong);
        }
        k += step;
    }
}

/// The take's waveform behind the blobs, faint, read through the time map so a moved
/// note's sound moves with it.
#[allow(clippy::too_many_arguments)]
fn waveform(
    b: &mut Brush,
    app: &CantusApp,
    track: TrackId,
    clip: ClipId,
    geo: &Geo,
    map: &TimeMap,
    color: Color32,
    s: &Style,
) {
    let Some((src, _)) = app.pitch_source(track, clip) else {
        return;
    };
    let crate::waveform::Lookup::Ready(peaks) = app.peaks.lookup(&src, app.audio_base(), None)
    else {
        return;
    };
    let g = geo.grid;
    let col = 2.0 * s.k;
    let n = (g.width() / col).ceil() as usize;
    let per = col as f64 / geo.px_per_frame;
    let mut one = Vec::with_capacity(1);
    let mut pairs = Vec::with_capacity(n);
    for i in 0..n {
        let f = geo.frame(g.left() + i as f32 * col);
        let a = map.source(f);
        peaks.columns(crate::waveform::Which::Mono, a, per, 1, &mut one);
        pairs.push(one.first().copied().unwrap_or((0.0, 0.0)));
    }
    let band = Rect::from_min_max(
        pos2(g.left(), g.center().y - g.height() * 0.32),
        pos2(g.left() + n as f32 * col, g.center().y + g.height() * 0.32),
    );
    b.register(band, "pitch_wave", &[Tok::TextDim], &[]);
    let ink = with_alpha(
        mix(color, s.t.text_dim, 0.4),
        if s.t.dark { 0.16 } else { 0.20 },
    );
    crate::waveform::fill_peaks(b, band, &pairs, |_| 1.0, ink);
}

/// Sibilants and breaths, as ticks along the bottom of the grid.
fn marks(b: &mut Brush, geo: &Geo, a: &TakeAnalysis, map: &TimeMap, s: &Style) {
    let g = geo.grid;
    let h = s.s(6.0);
    let col = 2.0 * s.k;
    let n = (g.width() / col).ceil() as usize;
    let hop = a.analysis.hop.max(1) as f64;
    for i in 0..n {
        let x = g.left() + i as f32 * col;
        let src = map.source(geo.frame(x));
        if src < 0.0 {
            continue;
        }
        let ink = match a.kind((src / hop).round() as usize) {
            FrameKind::Sibilant => s.t.highlight,
            FrameKind::Breath => s.t.text_dim,
            _ => continue,
        };
        b.rect(
            Rect::from_min_max(
                pos2(x, g.bottom() - h),
                pos2(x + col * 0.6, g.bottom() - s.s(1.0)),
            ),
            Fill::Solid(with_alpha(ink, 0.7)),
        );
    }
}

/// The level (0 … 1) of the take around source frame `src`, for a blob's thickness.
fn loudness(a: Option<&TakeAnalysis>, src: f64) -> f32 {
    let Some(a) = a else {
        return 0.7;
    };
    let i = (src / a.analysis.hop.max(1) as f64).round().max(0.0) as usize;
    let db = a.analysis.frames.get(i).map_or(-120.0, |f| f.level_db);
    ((db + 50.0) / 44.0).clamp(0.0, 1.0)
}

/// The blobs: each note on the row it sounds, as thick as the voice is loud, its name on
/// it when there is room.
#[allow(clippy::too_many_arguments)]
fn blobs(
    b: &mut Brush,
    geo: &Geo,
    edit: &PitchEdit,
    a: Option<&TakeAnalysis>,
    map: &TimeMap,
    selected: &[u32],
    color: Color32,
    s: &Style,
) -> Vec<(u32, Rect)> {
    let g = geo.grid;
    let mut out = Vec::new();
    let col = 2.0 * s.k;
    let mut top: Vec<Pos2> = Vec::new();
    let mut bottom: Vec<Pos2> = Vec::new();
    for n in &edit.notes {
        let (x0, x1) = (geo.x(n.start as f64), geo.x(n.end as f64));
        let y = geo.y(n.out_centre());
        let full = geo.row_h * 0.62;
        let r = Rect::from_min_max(pos2(x0, y - full), pos2(x1.max(x0 + 2.0), y + full));
        out.push((n.id, r));
        if x1 < g.left() || x0 > g.right() || r.bottom() < g.top() || r.top() > g.bottom() {
            continue;
        }
        let sel = selected.contains(&n.id);
        let k = ink(color, sel, s);
        b.register(r, "pitch_blob", BLOB_TOKENS, &[]);
        top.clear();
        bottom.clear();
        let (ca, cz) = (x0.max(g.left() - col), x1.min(g.right() + col));
        let mut x = ca;
        loop {
            let xc = x.min(cz);
            let src = map.source(geo.frame(xc));
            // Rounded ends: the thickness falls to nothing over the last few points.
            let end = ((xc - x0).min(x1 - xc) / (4.0 * s.k)).clamp(0.0, 1.0);
            let half = full * (0.25 + 0.75 * loudness(a, src)) * end.sqrt().max(0.15);
            top.push(pos2(xc, y - half));
            bottom.push(pos2(xc, y + half));
            if xc >= cz {
                break;
            }
            x += col;
        }
        b.between(&top, &bottom, k.fill);
        b.polyline(&top, b.px().max(s.k * 0.8), k.edge);
        b.polyline(&bottom, b.px().max(s.k * 0.8), k.edge);
        // The note it sits on: a thin line on its row's centre, under the blob's centre.
        let target = geo.y(n.pitch);
        if (target - y).abs() > 0.5 {
            b.hline(x0, x1, target, b.px(), with_alpha(k.edge, 0.6));
        }
        if x1 - x0 >= 30.0 * s.k && geo.row_h >= 10.0 {
            let name = pitch_name(n.out_centre().round().clamp(0.0, 127.0) as u8);
            let tx = Text::new(&name, Role::Micro, k.label)
                .align(Align2::LEFT_CENTER)
                .policy(Policy::Elide(Elide::End));
            let lr = Rect::from_min_max(
                pos2(x0 + 4.0 * s.k, y - full),
                pos2(x1 - 2.0 * s.k, y + full),
            );
            let _ = text::paint(b, lr, &tx, s.tk());
        }
    }
    out
}

/// The pitch curves: what was sung (dim, only where an edit changes it) and what will
/// sound (the text colour), through the blobs.
fn pitch_curves(
    b: &mut Brush,
    geo: &Geo,
    edit: &PitchEdit,
    a: &TakeAnalysis,
    c: &Curves,
    map: &TimeMap,
    s: &Style,
) {
    let g = geo.grid;
    let col = 2.0 * s.k;
    let n = (g.width() / col).ceil() as usize + 1;
    let hop = a.analysis.hop.max(1) as f64;
    let sung_at = |src: f64| -> f32 {
        let x = src / hop;
        let i = x.floor().max(0.0) as usize;
        let (p, q) = (
            a.midi.get(i).copied().unwrap_or(f32::NAN),
            a.midi.get(i + 1).copied().unwrap_or(f32::NAN),
        );
        if p.is_finite() && q.is_finite() {
            p + (q - p) * (x - i as f64) as f32
        } else {
            p
        }
    };
    let edited = edit.edited();
    let (mut was, mut now): (Vec<Pos2>, Vec<Pos2>) = (Vec::new(), Vec::new());
    let dim = with_alpha(s.t.text_dim, 0.55);
    let bright = s.t.text;
    let w = (1.4 * s.k).max(b.px());
    let flush = |b: &mut Brush, pts: &mut Vec<Pos2>, ink: Color32, w: f32| {
        if pts.len() > 1 {
            b.polyline(pts, w, ink);
        }
        pts.clear();
    };
    for i in 0..n {
        let x = g.left() + i as f32 * col;
        let out = geo.frame(x);
        let src = map.source(out);
        let sung = sung_at(src);
        if !sung.is_finite() || src < 0.0 {
            flush(b, &mut was, dim, w * 0.8);
            flush(b, &mut now, bright, w);
            continue;
        }
        if edited {
            // The sung pitch where the voice was: at the source's own time.
            was.push(pos2(geo.x(src), geo.y(sung)));
        }
        now.push(pos2(x, geo.y(sung + c.shift_at(src))));
    }
    flush(b, &mut was, dim, w * 0.8);
    flush(b, &mut now, bright, w);
}

/// The keyboard beside the rows.
fn keys(b: &mut Brush, geo: &Geo, s: &Style) {
    let t = &s.t;
    let r = b.snap_rect(geo.keys);
    b.register(r, "pitch_keys", KEY_TOKENS, &[]);
    b.rect(r, Fill::Solid(shade(t.key_white, -0.18)));
    let lo = geo.midi(r.bottom()).floor() as i32;
    let hi = geo.midi(r.top()).ceil() as i32;
    let edge = shade(t.key_white, -0.22);
    let ink = on_fill(t.key_white);
    for p in lo..=hi {
        let (y0, y1) = (geo.y(p as f32 + 0.5), geo.y(p as f32 - 0.5));
        let row = Rect::from_min_max(pos2(r.left(), y0), pos2(r.right(), y1));
        if is_black(p) {
            let k = Rect::from_min_max(row.min, pos2(r.left() + r.width() * 0.6, row.bottom()));
            b.rect(row, Fill::Solid(t.key_white));
            b.rrect(
                k.shrink2(vec2(0.0, 0.5)),
                s.r(1.5),
                Fill::V(shade(t.key_black, 0.10), t.key_black),
            );
        } else {
            b.rect(
                row,
                Fill::V(shade(t.key_white, 0.02), shade(t.key_white, -0.03)),
            );
            b.hline(
                row.left(),
                row.right(),
                row.bottom() - 0.5 * b.px(),
                b.px(),
                edge,
            );
        }
        if p.rem_euclid(12) == 0 && geo.row_h >= 8.0 {
            let name = pitch_name(p.clamp(0, 127) as u8);
            let tx = Text::new(&name, Role::Label, ink).align(Align2::RIGHT_CENTER);
            let lr = Rect::from_min_max(
                pos2(r.left(), row.top()),
                pos2(r.right() - 4.0 * s.k, row.bottom()),
            );
            let _ = text::paint(b, lr, &tx, s.tk());
        }
    }
    b.vline(
        r.right() - 0.5 * b.px(),
        r.top(),
        r.bottom(),
        b.px(),
        shade(t.key_white, -0.45),
    );
}

/// The ruler: bar numbers in the arrangement's count over the clip's frames, the playhead.
fn ruler(b: &mut Brush, app: &CantusApp, geo: &Geo, time: Option<&ClipTime>, s: &Style) {
    let t = &s.t;
    let r = b.snap_rect(geo.ruler);
    b.register(
        r,
        "pitch_ruler",
        &[Tok::Panel, Tok::TextDim, Tok::Grid],
        &[],
    );
    b.rect(r, Fill::V(shade(t.panel, 0.03), shade(t.panel, -0.03)));
    b.hline(
        r.left(),
        r.right(),
        r.bottom() - 0.5 * b.px(),
        b.px(),
        t.grid_strong,
    );
    let Some(tm) = time else {
        return;
    };
    let ts = app.project().time_signature;
    let bar = ts.ticks_per_bar().0.max(1);
    let px_bar =
        (geo.x(tm.frame(cantus_core::Ticks(bar))) - geo.x(tm.frame(cantus_core::Ticks(0)))).abs();
    let mut every = 1i64;
    while px_bar * (every as f32) < 44.0 * s.k && every < 1024 {
        every *= 2;
    }
    // Ordered: a clip played backwards runs its bars right to left.
    let (x0, x1) = (
        tm.tick(geo.frame(r.left())).0,
        tm.tick(geo.frame(r.right())).0,
    );
    let (t0, t1) = (x0.min(x1), x0.max(x1));
    let mut k = t0.div_euclid(bar);
    while k * bar <= t1 {
        let x = geo.x(tm.frame(cantus_core::Ticks(k * bar)));
        if k >= 0 && k % every == 0 {
            b.vline(x, r.center().y, r.bottom(), b.px(), t.text_dim);
            let label = (k + 1).to_string();
            let tx = Text::new(&label, Role::Label, t.text_dim).align(Align2::LEFT_CENTER);
            let lr = Rect::from_min_max(
                pos2(x + 3.0 * s.k, r.top()),
                pos2(x + 60.0 * s.k, r.bottom()),
            );
            let _ = text::paint(b, lr, &tx, s.tk());
        }
        k += 1;
    }
    let x = geo.x(tm.frame(app.engine.position()));
    if x >= r.left() && x <= r.right() {
        let h = r.height() * 0.45;
        b.polygon(
            &[
                pos2(x - h * 0.6, r.bottom() - h),
                pos2(x + h * 0.6, r.bottom() - h),
                pos2(x, r.bottom()),
            ],
            t.playhead,
        );
    }
}
