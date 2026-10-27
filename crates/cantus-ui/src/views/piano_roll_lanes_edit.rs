//! A controller lane's body (ADR 0068): its curve painted over the beats, and what the
//! pointer does in it — the pencil, line and curve tools, a point dragged, Shift for a
//! selection, a right-click to delete a point. Every edit is one `SetClipLanes` with the
//! clip's lanes as they are now, merged while the stroke lasts: one undo step a stroke.

use super::super::{ticks_to_beats, Geo};
use super::LaneTool;
use crate::app::CantusApp;
use crate::kit::color::with_alpha;
use crate::kit::hint::{self, Caps};
use crate::kit::{self, Brush, Fill, Style};
use crate::views::auto_curve::{self as curve, Frame, Ink};
use cantus_core::clip_lanes::{edit, ClipLane, LaneKind};
use cantus_core::{AutoPoint, Clip, ClipId, Command, MergeKey, Ticks, TrackId};
use egui::{pos2, Color32, PointerButton, Pos2, Rect, Response};

/// A stroke, a line or a move under way in one lane.
#[derive(Debug)]
pub(in crate::views::piano_roll) struct LaneDrag {
    kind: LaneKind,
    track: TrackId,
    clip: ClipId,
    /// The clip's lanes when it began: every frame is computed from them.
    original: Vec<ClipLane>,
    mode: Mode,
    key: MergeKey,
}

#[derive(Debug)]
enum Mode {
    /// The pencil's samples so far.
    Pencil(Vec<(Ticks, f32)>),
    /// A line or a curve from its press.
    Line { from: (Ticks, f32), curve: bool },
    /// Points being moved, from where the press was (tick, value).
    Move { idx: Vec<usize>, from: (f64, f32) },
    /// A selection's rectangle, from its press.
    Marquee(Pos2),
}

/// The value axis of a lane's body: 1 at the top, 0 at the bottom, with room for a point.
fn frame_of(geo: &Geo<'_>, body: Rect, s: &Style) -> Frame {
    let x0 = geo.x(0.0);
    let px_per_tick = geo.px_per_beat / cantus_core::PPQ as f32;
    Frame {
        x0,
        px_per_tick,
        top: body.top() + 5.0 * s.k,
        bottom: body.bottom() - 4.0 * s.k,
        clip_left: body.left(),
        clip_right: body.right(),
        origin: Ticks::ZERO,
    }
}

/// The instrument's bend range in semitones, where the project knows it: a native
/// instrument's own setting, ±2 for one that has none (the processor's plain MIDI); `None`
/// for a hosted plug-in (its range is its own) or an instrument that reads no bend.
/// Whether the track plays a hosted instrument (its bend range is its own).
fn hosted(app: &CantusApp, track: TrackId) -> bool {
    app.project()
        .track(track)
        .is_some_and(|t| t.instrument.external().is_some())
}

pub fn bend_range(app: &CantusApp, track: TrackId) -> Option<f32> {
    let t = app.project().track(track)?;
    let n = t.instrument.native()?;
    for key in ["bend_range", "pb_range", "bend"] {
        if n.kind.param(key).is_some() {
            return Some(n.get(key));
        }
    }
    Some(2.0)
}

/// A lane's value in words: "+1.25 st", "64", "On".
fn value_words(app: &CantusApp, track: TrackId, kind: LaneKind, v: f32) -> String {
    match kind {
        LaneKind::PitchBend => match bend_range(app, track) {
            Some(r) => format!("{:+.2} st", (v - 0.5) * 2.0 * r),
            None => format!("{:+.0}", kind.midi(v) as i32 - 8192),
        },
        k if k.is_switch() => {
            if v >= 0.5 {
                crate::i18n::t("automodes.value.on")
            } else {
                crate::i18n::t("automodes.value.off")
            }
        }
        k => format!("{}", k.midi(v)),
    }
}

/// Paint one controller lane's body: its ground, the beats, its centre and quarter lines,
/// the curve (filled from the bottom, or from the centre for the bend), its points (lit
/// when selected) and, for the bend, the range at its edges.
#[allow(clippy::too_many_arguments)]
pub fn paint_ctl(
    b: &mut Brush,
    app: &CantusApp,
    geo: &Geo<'_>,
    body: Rect,
    clip: &Clip,
    lane: &ClipLane,
    selected: &[usize],
    track: TrackId,
    ink: Color32,
    grid_ticks: Ticks,
    s: &Style,
) {
    let t = &s.t;
    let body = b.snap_rect(body);
    b.register(
        body,
        "controller_lane",
        &[kit::Tok::RowBlack, kit::Tok::Grid],
        &[],
    );
    b.rect(body, Fill::Solid(kit::color::mix(t.row_black, t.ink, 0.25)));
    super::super::paint::ladder(b, app, geo, body, clip, grid_ticks, s);
    let f = frame_of(geo, body, s);
    // The value lines: the centre (the bend's rest), the quarters faint.
    for (v, strong) in [(0.25, false), (0.5, true), (0.75, false)] {
        let c = if strong { t.grid_strong } else { t.grid };
        b.hline(body.left(), body.right(), f.y(v), b.px(), c);
    }
    // Outside the clip's window, dimmed: what is there is kept, not heard.
    let a = geo.x(ticks_to_beats(clip.offset));
    let z = geo.x(ticks_to_beats(
        clip.offset + clip.period().unwrap_or(clip.length),
    ));
    for (l, r) in [(body.left(), a), (z, body.right())] {
        if r > l {
            let shade = Rect::from_min_max(
                pos2(l.max(body.left()), body.top()),
                pos2(r.min(body.right()), body.bottom()),
            );
            if shade.width() > 0.0 {
                b.rect(shade, Fill::Solid(with_alpha(t.ink, 0.45)));
            }
        }
    }
    let mut pts: Vec<Pos2> = Vec::new();
    let from = Ticks(f.tick(body.left()).floor() as i64).max(Ticks::ZERO);
    let to = Ticks(f.tick(body.right()).ceil() as i64 + 1);
    curve::trace_points(&lane.points, from, to, &f, 3.0, &mut pts);
    let base = if lane.kind == LaneKind::PitchBend {
        f.y(0.5)
    } else {
        f.bottom
    };
    curve::paint(b, &pts, base, Ink::of(ink, 1.5 * s.k, s));
    let ground = kit::color::mix(t.row_black, t.ink, 0.25);
    let r = 2.5 * s.k;
    for (i, p) in lane.points.iter().enumerate() {
        let c = f.point(p.tick, p.value);
        if c.x < body.left() - r || c.x > body.right() + r {
            continue;
        }
        let lit = selected.contains(&i);
        let col = if lit { t.selection } else { ink };
        curve::point(b, c, r, col, ground, lit);
    }
    if lane.kind == LaneKind::PitchBend && body.height() >= 30.0 * s.k {
        let words = match bend_range(app, track) {
            Some(rg) => (format!("+{rg:.0} st"), format!("\u{2212}{rg:.0} st")),
            None if hosted(app, track) => (
                crate::i18n::t("automodes.range.plugin_top"),
                crate::i18n::t("automodes.range.plugin_bottom"),
            ),
            None => (crate::i18n::t("automodes.range.none"), String::new()),
        };
        let line = 13.0 * s.k;
        for (w, top) in [
            (&words.0, body.top() + 2.0 * s.k),
            (&words.1, body.bottom() - 2.0 * s.k - line),
        ] {
            let tx = kit::text::Text::new(w, kit::text::Role::Micro, t.text_dim)
                .align(egui::Align2::RIGHT_CENTER);
            let r = Rect::from_min_max(
                pos2(body.right() - 200.0 * s.k, top),
                pos2(body.right() - 6.0 * s.k, top + line),
            );
            let _ = kit::text::paint(b, r, &tx, s.tk());
        }
    }
}

/// The lane of `kind` in the clip, if it has one.
fn lane_of(app: &CantusApp, track: TrackId, clip: ClipId, kind: LaneKind) -> Option<ClipLane> {
    app.project()
        .track(track)?
        .clip(clip)?
        .lanes
        .iter()
        .find(|l| l.kind == kind)
        .cloned()
}

/// Run one frame of an edit: the clip's lanes with `kind`'s points replaced.
fn set(app: &mut CantusApp, d: &LaneDrag, points: Vec<AutoPoint>) {
    let mut lanes = d.original.clone();
    match lanes.iter_mut().find(|l| l.kind == d.kind) {
        Some(l) => l.points = points,
        None => lanes.push(ClipLane::new(d.kind, points)),
    }
    app.exec_merging(
        Command::SetClipLanes {
            track: d.track,
            clip: d.clip,
            lanes,
        },
        d.key,
    );
}

/// The pointer in a controller lane's body.
#[allow(clippy::too_many_arguments)]
pub fn ctl_input(
    app: &mut CantusApp,
    ui: &egui::Ui,
    geo: &Geo<'_>,
    body: Rect,
    resp: &Response,
    kind: LaneKind,
    track: TrackId,
    clip: ClipId,
) {
    let s = Style::current();
    let f = frame_of(geo, body, &s);
    let Some(lane) = lane_of(app, track, clip, kind) else {
        return;
    };
    let mods = ui.input(|i| i.modifiers);
    let snapping = app.piano_roll.snap != mods.alt;
    let grid = app.piano_roll.grid;
    let origin = app
        .project()
        .track(track)
        .and_then(|t| t.clip(clip))
        .map_or(Ticks::ZERO, |c| c.start - c.offset);
    let snap = |app: &CantusApp, tick: Ticks| {
        if snapping {
            crate::views::bars::snap_content(app.project(), tick, grid, origin)
        } else {
            tick
        }
    };
    let at =
        |p: Pos2| -> (Ticks, f32) { (Ticks(f.tick(p.x).round().max(0.0) as i64), f.value(p.y)) };
    let point_under = |p: Pos2| {
        lane.points
            .iter()
            .enumerate()
            .map(|(i, q)| (i, f.point(q.tick, q.value).distance(p)))
            .filter(|(_, d)| *d <= 6.0 * s.k)
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(i, _)| i)
    };
    if let Some(p) = resp.hover_pos() {
        let (tick, v) = at(p);
        let here = lane.value_at(tick.0 as f64).unwrap_or(v);
        let name = super::lane_name(kind);
        let words = value_words(app, track, kind, here);
        hint::set_text(
            ui.ctx(),
            &name,
            &format!(
                "{words} \u{b7} {}",
                crate::i18n::t("automodes.lane.ctl_tip")
            ),
            Caps::NONE,
        );
    }
    // A right-click on a point deletes it (FL).
    if resp.secondary_clicked() {
        if let Some(i) = resp.interact_pointer_pos().and_then(point_under) {
            let mut pts = lane.points.clone();
            pts.remove(i);
            let d = begin(app, track, clip, kind, Mode::Marquee(Pos2::ZERO));
            set(app, &d, pts);
            app.end_gesture();
            app.piano_roll.lanes.set_selected(kind, Vec::new());
        }
        return;
    }
    if resp.drag_started_by(PointerButton::Primary)
        || resp.clicked() && app.piano_roll.lanes.drag.is_none()
    {
        let Some(press) = ui.input(|i| i.pointer.press_origin()) else {
            return;
        };
        let (tick, v) = at(press);
        let selected = app.piano_roll.lanes.selected(kind).to_vec();
        let mode = if mods.shift {
            Mode::Marquee(press)
        } else if let Some(i) = point_under(press) {
            // A point: it (and the selection it is in) moves.
            let idx = if selected.contains(&i) {
                selected
            } else {
                vec![i]
            };
            app.piano_roll.lanes.set_selected(kind, idx.clone());
            Mode::Move {
                idx,
                from: (f.tick(press.x), f.value(press.y)),
            }
        } else {
            app.piano_roll.lanes.set_selected(kind, Vec::new());
            match app.piano_roll.lanes.tool {
                LaneTool::Pencil => Mode::Pencil(vec![(tick, v)]),
                LaneTool::Line => Mode::Line {
                    from: (snap(app, tick), v),
                    curve: false,
                },
                LaneTool::Curve => Mode::Line {
                    from: (snap(app, tick), v),
                    curve: true,
                },
            }
        };
        let d = begin(app, track, clip, kind, mode);
        app.piano_roll.lanes.drag = Some(d);
    }
    let pos = resp.interact_pointer_pos();
    if let (Some(pos), true) = (
        pos,
        resp.dragged_by(PointerButton::Primary) || resp.clicked(),
    ) {
        let Some(mut d) = app.piano_roll.lanes.drag.take() else {
            return;
        };
        let original = d
            .original
            .iter()
            .find(|l| l.kind == kind)
            .map(|l| l.points.clone())
            .unwrap_or_default();
        match &mut d.mode {
            Mode::Pencil(samples) => {
                let (tick, v) = at(pos);
                // Fill the stroke between frames, so a fast stroke leaves no gaps.
                if let Some(&(t0, v0)) = samples.last() {
                    let n = ((tick.0 - t0.0).abs() / 10).clamp(1, 512);
                    for k in 1..=n {
                        let u = k as f32 / n as f32;
                        let tt = Ticks(t0.0 + ((tick.0 - t0.0) as f32 * u).round() as i64);
                        samples.push((tt, v0 + (v - v0) * u));
                    }
                } else {
                    samples.push((tick, v));
                }
                let pts = edit::pencil(&original, kind, samples);
                set(app, &d, pts);
            }
            Mode::Line { from, curve } => {
                let (tick, v) = at(pos);
                let tick = snap(app, tick);
                let bend = if *curve {
                    if mods.alt {
                        -0.6
                    } else {
                        0.6
                    }
                } else {
                    0.0
                };
                let pts = edit::line(&original, from.0, from.1, tick, v, bend);
                set(app, &d, pts);
            }
            Mode::Move { idx, from } => {
                let dt = f.tick(pos.x) - from.0;
                let dv = f.value(pos.y) - from.1;
                let dt = if mods.command { 0.0 } else { dt };
                let mut pts = original.clone();
                for &i in idx.iter() {
                    if let Some(p) = pts.get_mut(i) {
                        let t = (p.tick.0 as f64 + dt).round().max(0.0) as i64;
                        p.tick = snap(app, Ticks(t));
                        p.value = (p.value + dv).clamp(0.0, 1.0);
                    }
                }
                // Sorted again, the selection following its points.
                let moved: Vec<AutoPoint> =
                    idx.iter().filter_map(|i| pts.get(*i).copied()).collect();
                let mut order: Vec<usize> = (0..pts.len()).collect();
                order.sort_by_key(|i| pts[*i].tick);
                let sorted: Vec<AutoPoint> = order.iter().map(|i| pts[*i]).collect();
                let sel: Vec<usize> = sorted
                    .iter()
                    .enumerate()
                    .filter(|(_, p)| moved.contains(p))
                    .map(|(i, _)| i)
                    .collect();
                set(app, &d, sorted);
                app.piano_roll.lanes.set_selected(kind, sel);
            }
            Mode::Marquee(a) => {
                let r = Rect::from_two_pos(*a, pos);
                let sel: Vec<usize> = lane
                    .points
                    .iter()
                    .enumerate()
                    .filter(|(_, p)| r.contains(f.point(p.tick, p.value)))
                    .map(|(i, _)| i)
                    .collect();
                app.piano_roll.lanes.set_selected(kind, sel);
            }
        }
        app.piano_roll.lanes.drag = Some(d);
    }
    if (resp.drag_stopped() || resp.clicked()) && app.piano_roll.lanes.drag.take().is_some() {
        app.end_gesture();
    }
}

/// A drag begins: the clip's lanes as they are, a stroke's own undo step.
fn begin(
    app: &mut CantusApp,
    track: TrackId,
    clip: ClipId,
    kind: LaneKind,
    mode: Mode,
) -> LaneDrag {
    let original = app
        .project()
        .track(track)
        .and_then(|t| t.clip(clip))
        .map(|c| c.lanes.clone())
        .unwrap_or_default();
    let v = &mut app.piano_roll.lanes;
    v.stroke += 1;
    LaneDrag {
        kind,
        track,
        clip,
        original,
        mode,
        key: MergeKey::of(&("lane-stroke", clip.0, v.stroke)),
    }
}

/// Delete the points selected in the clip's lanes: one undo step. Says whether there were
/// any.
pub fn delete_selected(app: &mut CantusApp, track: TrackId, clip: ClipId) -> bool {
    let sel = std::mem::take(&mut app.piano_roll.lanes.sel);
    if sel.iter().all(|(_, v)| v.is_empty()) {
        return false;
    }
    let Some(c) = app.project().track(track).and_then(|t| t.clip(clip)) else {
        return false;
    };
    let lanes: Vec<ClipLane> = c
        .lanes
        .iter()
        .map(|l| {
            let gone = sel
                .iter()
                .find(|(k, _)| *k == l.kind)
                .map_or(&[][..], |(_, v)| v.as_slice());
            ClipLane::new(
                l.kind,
                l.points
                    .iter()
                    .enumerate()
                    .filter(|(i, _)| !gone.contains(i))
                    .map(|(_, p)| *p)
                    .collect(),
            )
        })
        .collect();
    app.exec(Command::SetClipLanes { track, clip, lanes })
}
