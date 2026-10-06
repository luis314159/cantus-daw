//! Automation lanes under a track (ADR 0027): one lane per automated target, below the
//! track's row and its take lanes — Ableton's and Bitwig's lanes, holding FL's automation
//! clips. The clips are drawn by the arrangement's own clip painter (chrome, selection,
//! move, trim, split, loop all as for any clip); this module draws their curve, the
//! lanes' ground and headers, and edits points in place:
//!
//! - **drag a point** to move it (snapped in time like a clip edge; Ctrl for free);
//! - **drag the ring** halfway along a segment to bend it (tension);
//! - **press on the line** between points to add one there, and keep dragging it;
//! - **double-click a point** to delete it; **right-click** it for its shape.
//!
//! The large editor, for everything else, is the piano roll's area (`auto_editor`).

use super::{ticks_to_beats, TimelineGeometry, TrackRow};
use crate::app::CantusApp;
use crate::kit::color::{mix, shade, with_alpha};
use crate::kit::hint::{self, Caps};
use crate::kit::text::{self, Elide, Policy, Role, Text};
use crate::kit::{Brush, Fill, Style, Tok};
use crate::views::auto_curve::{self as curve, Frame};
use crate::views::clips_chrome::Tint;
use cantus_core::automation::edit;
use cantus_core::{
    AutoPoint, AutoTarget, Automation, Clip, ClipId, Command, MergeKey, Ticks, Track, TrackId,
};
use egui::{pos2, Align2, Pos2, Rect, Sense};

/// An automation lane's height at the base row height.
pub(super) fn lane_height(base: f32) -> f32 {
    (base * 1.05).round().max(28.0)
}

/// How many lanes a track has: one per distinct target among its automation clips.
pub(super) fn lane_count(t: &Track) -> usize {
    let autos = || t.clips.iter().filter_map(|c| c.automation.as_ref());
    autos()
        .enumerate()
        .filter(|(i, a)| !autos().take(*i).any(|b| b.target == a.target))
        .count()
}

/// The height the lanes add under a track, while they are shown.
pub(super) fn height(t: &Track, base: f32) -> f32 {
    if t.automation_open {
        lane_count(t) as f32 * lane_height(base)
    } else {
        0.0
    }
}

/// Where lane `lane` (0…) of a row is drawn; `None` while the lanes are hidden.
pub(super) fn lane_row(r: &TrackRow, lane: usize, base: f32) -> Option<TrackRow> {
    let h = lane_height(base);
    let top = r.top + r.height + r.takes + lane as f32 * h;
    (r.auto > 0.0 && top + h <= r.top + r.height + r.takes + r.auto + 0.5).then_some(TrackRow {
        top,
        height: h,
        takes: 0.0,
        auto: 0.0,
    })
}

/// A point or a bend being dragged in a clip.
#[derive(Clone, Debug)]
struct PointDrag {
    r: (TrackId, ClipId),
    what: Grab,
    /// The curve as it was at the press: every frame of the drag rewrites from it, so the
    /// drag is one merged step and a point never drifts.
    at_press: Automation,
}

#[derive(Copy, Clone, Debug, PartialEq)]
enum Grab {
    Point(usize),
    Bend(usize),
}

/// One automation clip as drawn this frame: where its content is on screen.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Drawn {
    pub track: TrackId,
    pub clip: ClipId,
    pub body: Rect,
    /// The curve's frame for the repetition under `body`'s left edge (content tick 0 at
    /// `frame.x0`); a looped clip's later repetitions are offsets of it.
    pub frame: Frame,
}

/// The lanes' state: their order per track, compiled once per revision, what was drawn
/// (published for tests and for the input pass), and a drag in progress.
#[derive(Debug, Default)]
pub struct State {
    rev: u64,
    /// Per track (by index): its lanes' targets in order.
    targets: Vec<Vec<AutoTarget>>,
    pub drawn: Vec<Drawn>,
    drag: Option<PointDrag>,
    /// The scratch the curve is traced into.
    pts: Vec<Pos2>,
}

impl State {
    /// Bring the lane order up to date with the project (once per revision).
    pub(super) fn refresh(&mut self, app: &CantusApp) {
        let rev = app.bus.revision();
        if rev == self.rev && self.targets.len() == app.project().tracks.len() {
            return;
        }
        self.rev = rev;
        self.targets = app
            .project()
            .tracks
            .iter()
            .map(|t| {
                let mut v: Vec<AutoTarget> = t
                    .clips
                    .iter()
                    .filter_map(|c| c.automation.as_ref().map(|a| a.target.clone()))
                    .collect();
                v.sort();
                v.dedup();
                v
            })
            .collect();
    }

    /// The lane a target has under track `i`.
    pub fn lane_of(&self, i: usize, target: &AutoTarget) -> Option<usize> {
        self.targets.get(i)?.iter().position(|t| t == target)
    }

    pub fn targets(&self, i: usize) -> &[AutoTarget] {
        self.targets.get(i).map_or(&[], |v| v.as_slice())
    }
}

/// The lanes' ground: a darker band than the track's row, the base line of each lane, a
/// hairline between lanes.
pub(super) fn grounds(
    b: &mut Brush,
    st: &State,
    base: f32,
    geo: &TimelineGeometry,
    rect: Rect,
    s: &Style,
) {
    let t = &s.t;
    for (i, row) in geo.rows.iter().enumerate() {
        if row.auto <= 0.0 {
            continue;
        }
        for lane in 0..st.targets(i).len() {
            let Some(r) = lane_row(row, lane, base) else {
                break;
            };
            if r.top > rect.bottom() || r.top + r.height < rect.top() {
                continue;
            }
            let band = Rect::from_min_max(
                pos2(rect.left(), r.top),
                pos2(rect.right(), r.top + r.height),
            );
            b.register(band, "auto_lane", &[Tok::Ink, Tok::Grid], &[]);
            b.rect(
                b.snap_rect(band),
                Fill::Solid(with_alpha(t.ink, if t.dark { 0.30 } else { 0.07 })),
            );
            b.hline(
                rect.left(),
                rect.right(),
                band.bottom() - 0.5 * b.px(),
                b.px(),
                t.grid,
            );
        }
    }
}

/// The lane a clip is drawn in, if it is an automation clip and its lanes are shown.
pub(super) fn clip_row(
    st: &State,
    base: f32,
    track: usize,
    row: &TrackRow,
    clip: &Clip,
) -> Option<Option<TrackRow>> {
    let a = clip.automation.as_ref()?;
    let lane = st.lane_of(track, &a.target);
    Some(lane.and_then(|l| lane_row(row, l, base)))
}

/// The curve in an automation clip's body: the area under it, the line, and — when there
/// is room — its points, and its bend rings while the clip is selected or hovered.
#[allow(clippy::too_many_arguments)]
pub(super) fn paint_body(
    b: &mut Brush,
    st: &mut State,
    r: (TrackId, ClipId),
    clip: &Clip,
    window: cantus_core::ClipWindow,
    inner: Rect,
    body: Rect,
    visible: Rect,
    geo: &TimelineGeometry,
    ink: egui::Color32,
    ground: egui::Color32,
    lit: bool,
    pointer: Option<Pos2>,
    s: &Style,
) {
    let Some(auto) = clip.automation.as_ref() else {
        return;
    };
    if inner.height() < 4.0 || inner.width() < 2.0 {
        return;
    }
    let px_per_tick = geo.px_per_beat / cantus_core::PPQ as f32;
    let pad = (inner.height() * 0.08).clamp(1.0, 4.0 * s.k);
    // Each repetition of the content (one for a clip that does not loop): timeline =
    // content + shift.
    let period = window.period().filter(|p| *p < window.length);
    let mut rep = window.start;
    let mut first = true;
    loop {
        let rep_end = match period {
            Some(p) => (rep + p).min(window.start + window.length),
            None => window.start + window.length,
        };
        let shift = rep - window.offset;
        let f = Frame {
            x0: geo.x(ticks_to_beats(shift)),
            px_per_tick,
            top: inner.top() + pad,
            bottom: inner.bottom() - pad,
            clip_left: visible.left().max(inner.left()),
            clip_right: visible.right().min(inner.right()),
        };
        if first {
            st.drawn.push(Drawn {
                track: r.0,
                clip: r.1,
                body,
                frame: f,
            });
            first = false;
        }
        let (ca, cb) = (rep - shift, rep_end - shift);
        st.pts.clear();
        curve::trace(auto, ca, cb, &f, 3.0, &mut st.pts);
        curve::paint(
            b,
            &st.pts,
            inner.bottom(),
            curve::Ink::of(ink, 1.5 * s.k, s),
        );
        // Points, when they are far enough apart to be told apart.
        let span_px = (cb - ca).0 as f32 * px_per_tick;
        let n = auto
            .points
            .iter()
            .filter(|p| p.tick >= ca && p.tick < cb)
            .count()
            .max(1);
        if inner.height() >= 16.0 * s.k && span_px / n as f32 >= 7.0 * s.k {
            let rad = 2.5 * s.k;
            let near = pointer.filter(|p| body.contains(*p));
            for p in auto.points.iter().filter(|p| p.tick >= ca && p.tick <= cb) {
                let c = f.point(p.tick, p.value);
                if c.x < f.clip_left - rad || c.x > f.clip_right + rad {
                    continue;
                }
                let hot = near.is_some_and(|q| q.distance(c) <= 6.0 * s.k);
                curve::point(b, c, if hot { rad * 1.3 } else { rad }, ink, ground, hot);
            }
            if lit || near.is_some() {
                for i in 0..auto.points.len().saturating_sub(1) {
                    let (p, q) = (auto.points[i], auto.points[i + 1]);
                    if p.tick < ca || q.tick > cb {
                        continue;
                    }
                    if let Some(c) = curve::handle_at(auto, i, &f) {
                        if (q.tick - p.tick).0 as f32 * px_per_tick >= 18.0 * s.k {
                            let hot = near.is_some_and(|x| x.distance(c) <= 6.0 * s.k);
                            curve::handle(b, c, 2.5 * s.k, ink, hot);
                        }
                    }
                }
            }
        }
        match period {
            Some(p) if rep_end < window.start + window.length => rep += p,
            _ => break,
        }
    }
}

/// The lanes' headers: the parameter's name over the device's, the value it has where
/// the song is, in a band of the track's colour.
#[allow(clippy::too_many_arguments)]
pub(super) fn paint_headers(
    b: &mut Brush,
    app: &CantusApp,
    i: usize,
    track: &Track,
    row: &TrackRow,
    clip: Rect,
    tint: Tint,
    s: &Style,
) {
    let _ = track;
    let t = &s.t;
    let k = s.k;
    let base = app.arrangement.track_height;
    let lanes = app.auto_lanes();
    let project = app.project();
    let mut value = String::new();
    for (lane, target) in app.arrangement.auto.targets(i).iter().enumerate() {
        let Some(r) = lane_row(row, lane, base) else {
            break;
        };
        if r.top > clip.bottom() || r.top + r.height < clip.top() {
            continue;
        }
        let band = Rect::from_min_max(
            pos2(clip.left(), r.top),
            pos2(clip.right(), r.top + r.height),
        );
        b.register(band, "auto_header", &[Tok::Panel, Tok::Text], &[]);
        b.rect(
            b.snap_rect(band),
            Fill::Solid(shade(t.panel, if t.dark { -0.04 } else { -0.03 })),
        );
        // The track's colour as a spine, so the lane reads as the track's.
        let spine = Rect::from_min_max(band.min, pos2(band.left() + 4.0 * k, band.bottom()));
        b.rect(
            b.snap_rect(spine),
            Fill::Solid(mix(tint.large, t.panel, 0.35)),
        );
        b.hline(
            band.left(),
            band.right(),
            band.bottom() - 0.5 * b.px(),
            b.px(),
            t.grid,
        );
        // A small curve glyph: this row is automation.
        let g = Rect::from_center_size(
            pos2(band.left() + 14.0 * k, band.center().y),
            egui::vec2(12.0 * k, 9.0 * k),
        );
        glyph(b, g, with_alpha(t.text_dim, 0.9), k);
        let (place, what) = crate::automation::target_parts(project, target);
        value.clear();
        if let Some(l) = lanes.iter().find(|l| &l.target == target) {
            crate::automation::write_value(
                project,
                target,
                l.value_at(app.engine.position().0 as f64),
                &mut value,
            );
        }
        let right = Rect::from_min_max(
            pos2(band.right() - 64.0 * k, band.top()),
            pos2(band.right() - 6.0 * k, band.bottom()),
        );
        let _ = text::paint(
            b,
            right,
            &Text::new(&value, Role::Value, t.text).align(Align2::RIGHT_CENTER),
            s.tk(),
        );
        let names = Rect::from_min_max(
            pos2(g.right() + 6.0 * k, band.top()),
            pos2(right.left() - 4.0 * k, band.bottom()),
        );
        if r.height >= 30.0 * k {
            let (top, bottom) = names.split_top_bottom_at_fraction(0.5);
            let _ = text::paint(
                b,
                top.translate(egui::vec2(0.0, 2.0 * k)),
                &Text::new(&what, Role::Label, t.text)
                    .align(Align2::LEFT_CENTER)
                    .policy(Policy::Elide(Elide::End)),
                s.tk(),
            );
            let _ = text::paint(
                b,
                bottom.translate(egui::vec2(0.0, -2.0 * k)),
                &Text::new(&place, Role::Micro, t.text_dim)
                    .align(Align2::LEFT_CENTER)
                    .policy(Policy::Elide(Elide::End)),
                s.tk(),
            );
        } else {
            let _ = text::paint(
                b,
                names,
                &Text::new(&format!("{what} \u{b7} {place}"), Role::Label, t.text)
                    .align(Align2::LEFT_CENTER)
                    .policy(Policy::Elide(Elide::End)),
                s.tk(),
            );
        }
    }
}

/// The lane glyph: a little ramp and hold, drawn.
fn glyph(b: &mut Brush, r: Rect, c: egui::Color32, k: f32) {
    let pts = [
        pos2(r.left(), r.bottom()),
        pos2(r.left() + r.width() * 0.45, r.top()),
        pos2(r.right() - r.width() * 0.2, r.top()),
        pos2(r.right() - r.width() * 0.2, r.center().y),
        pos2(r.right(), r.center().y),
    ];
    b.polyline(&pts, 1.25 * k, c);
}

/// The lanes' headers as controls: a click selects the lane's clips (the first opens in
/// the editor), a right-click offers hiding the lanes and deleting the lane.
pub(super) fn header_input(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    i: usize,
    id: TrackId,
    row: &TrackRow,
    clip: Rect,
) {
    let base = app.arrangement.track_height;
    let targets = app.arrangement.auto.targets(i).to_vec();
    for (lane, target) in targets.iter().enumerate() {
        let Some(r) = lane_row(row, lane, base) else {
            break;
        };
        let band = Rect::from_min_max(
            pos2(clip.left(), r.top),
            pos2(clip.right(), r.top + r.height),
        )
        .intersect(clip);
        if band.height() <= 0.0 {
            continue;
        }
        let resp = ui.interact(
            band,
            ui.id().with(("auto-lane-head", id.0, lane)),
            Sense::click(),
        );
        if resp.hovered() {
            let name = crate::automation::target_name(app.project(), target);
            hint::set_text(
                ui.ctx(),
                &name,
                "",
                Caps {
                    menu: true,
                    ..Caps::NONE
                },
            );
        }
        let clips = lane_clips(app, id, target);
        if resp.clicked() {
            app.set_clip_selection(clips.iter().map(|c| (id, *c)).collect());
            if let Some(c) = clips.first() {
                app.select_clip(id, *c);
            }
        }
        egui::Popup::context_menu(&resp)
            .frame(crate::views::clips_chrome::menu_frame())
            .show(|ui| {
                let items = [
                    crate::kit::menu::Item::action(crate::kit::text::Src::Key("auto.lane.hide")),
                    crate::kit::menu::Item::action(crate::kit::text::Src::Key("auto.lane.delete")),
                ];
                match crate::kit::widgets::menu_list(ui, &items, 220.0).1.clicked {
                    Some(0) => {
                        app.exec(Command::SetTrackAutomationOpen { id, open: false });
                        ui.close();
                    }
                    Some(1) => {
                        let commands = clips
                            .iter()
                            .map(|c| Command::RemoveClip {
                                track: id,
                                clip: *c,
                            })
                            .collect();
                        app.exec(Command::Batch {
                            label_key: "auto.cmd.delete_lane".into(),
                            commands,
                        });
                        ui.close();
                    }
                    _ => {}
                }
            });
    }
}

/// The clips of one lane, in time order.
fn lane_clips(app: &CantusApp, track: TrackId, target: &AutoTarget) -> Vec<ClipId> {
    app.project()
        .track(track)
        .map(|t| {
            t.clips
                .iter()
                .filter(|c| c.automation.as_ref().is_some_and(|a| &a.target == target))
                .map(|c| c.id)
                .collect()
        })
        .unwrap_or_default()
}

/// What is under the pointer in a drawn clip.
fn grab_at(auto: &Automation, d: &Drawn, pos: Pos2, k: f32) -> Option<Grab> {
    if !d.body.contains(pos) {
        return None;
    }
    if let Some(i) = curve::point_at(auto, &d.frame, pos, 6.0 * k) {
        return Some(Grab::Point(i));
    }
    curve::handle_under(auto, &d.frame, pos, 6.0 * k).map(Grab::Bend)
}

/// Points edited in place, over the clips the painter drew this frame. Registered after
/// the timeline, so a press on a point, a ring or the line is the curve's; anywhere else
/// in the clip it is the clip's (move, trim).
pub(super) fn input(app: &mut CantusApp, ui: &mut egui::Ui, lanes: Rect, s: &Style) {
    let k = s.k;
    let pointer = ui
        .input(|i| i.pointer.hover_pos())
        .filter(|p| lanes.contains(*p));
    let drawn = std::mem::take(&mut app.arrangement.auto.drawn);
    for d in &drawn {
        if d.body.height() < 16.0 * k {
            continue;
        }
        let Some(auto) = clip_auto(app, (d.track, d.clip)) else {
            continue;
        };
        let r = (d.track, d.clip);
        let dragging = app.arrangement.auto.drag.as_ref().is_some_and(|g| g.r == r);
        let grab = pointer.and_then(|p| grab_at(&auto, d, p, k));
        let on_line = pointer.filter(|p| d.body.contains(*p)).is_some_and(|p| {
            curve::distance_to_curve(&auto, &d.frame, p).is_some_and(|dy| dy <= 4.0 * k)
        });
        // While the button is down, the press decides: a drag that started on the curve
        // keeps its widget even when the pointer has left the point.
        let held = ui
            .input(|i| {
                i.pointer
                    .primary_down()
                    .then(|| i.pointer.press_origin())
                    .flatten()
            })
            .is_some_and(|p| {
                grab_at(&auto, d, p, k).is_some()
                    || (d.body.contains(p)
                        && curve::distance_to_curve(&auto, &d.frame, p)
                            .is_some_and(|dy| dy <= 4.0 * k))
            });
        if !(dragging || held || grab.is_some() || on_line) {
            continue;
        }
        let area = d.body.intersect(lanes);
        let resp = ui.interact(
            area,
            ui.id().with(("auto-curve", d.clip.0)),
            Sense::click_and_drag(),
        );
        ui.ctx().set_cursor_icon(match grab {
            Some(Grab::Point(_)) => egui::CursorIcon::Grab,
            Some(Grab::Bend(_)) => egui::CursorIcon::ResizeVertical,
            None => egui::CursorIcon::Crosshair,
        });
        if let Some(Grab::Point(i)) = grab {
            point_hint(app, ui, &auto, i);
        } else if on_line {
            hint::set_text(ui.ctx(), &crate::i18n::t("auto.hint.line"), "", Caps::NONE);
        }
        if resp.drag_started_by(egui::PointerButton::Primary) {
            let press = ui.input(|i| i.pointer.press_origin()).unwrap_or_default();
            let what = match grab_at(&auto, d, press, k) {
                Some(g) => Some(g),
                None => {
                    // On the line: a new point there, and it is the one dragged.
                    let mut a = auto.clone();
                    let tick = Ticks(d.frame.tick(press.x).round().max(0.0) as i64);
                    let v = a.value_at(tick.0 as f64).unwrap_or(0.5);
                    let i = edit::insert(&mut a.points, AutoPoint::new(tick, v));
                    set_curve(app, r, a, true);
                    Some(Grab::Point(i))
                }
            };
            if let Some(what) = what {
                if !app.clip_selection().contains(&r) {
                    app.select_clip(d.track, d.clip);
                }
                let at_press = clip_auto(app, r).unwrap_or_else(|| auto.clone());
                app.arrangement.auto.drag = Some(PointDrag { r, what, at_press });
            }
        }
        if resp.dragged_by(egui::PointerButton::Primary) {
            if let (Some(pos), Some(g)) = (
                resp.interact_pointer_pos(),
                app.arrangement.auto.drag.clone(),
            ) {
                let off_grid = ui.input(|i| i.modifiers.command);
                let a = dragged(app, &g, &d.frame, pos, off_grid);
                set_curve(app, r, a, true);
            }
        }
        if resp.drag_stopped() {
            app.arrangement.auto.drag = None;
            app.end_gesture();
        }
        if resp.clicked() && grab.is_none() && on_line {
            if let Some(p) = resp.interact_pointer_pos() {
                let mut a = auto.clone();
                let tick = Ticks(d.frame.tick(p.x).round().max(0.0) as i64);
                edit::insert(&mut a.points, AutoPoint::new(tick, d.frame.value(p.y)));
                set_curve(app, r, a, false);
            }
        }
        if resp.double_clicked() {
            if let Some(Grab::Point(i)) = grab {
                let mut a = auto.clone();
                if a.points.len() > 1 {
                    a.points.remove(i);
                    set_curve(app, r, a, false);
                }
            }
        }
        if let Some(Grab::Point(i)) = grab {
            egui::Popup::context_menu(&resp)
                .frame(crate::views::clips_chrome::menu_frame())
                .show(|ui| point_menu(app, ui, r, i));
        }
    }
    app.arrangement.auto.drawn = drawn;
}

/// The curve of an automation clip, cloned (small: a target and a few points).
fn clip_auto(app: &CantusApp, r: (TrackId, ClipId)) -> Option<Automation> {
    app.project().track(r.0)?.clip(r.1)?.automation.clone()
}

/// Write a clip's curve: one step, or (in a drag) merged into the drag's step.
pub(crate) fn set_curve(app: &mut CantusApp, r: (TrackId, ClipId), a: Automation, merge: bool) {
    let cmd = Command::SetClipAutomation {
        track: r.0,
        clip: r.1,
        automation: Some(a),
    };
    if merge {
        app.exec_merging(cmd, MergeKey::of(&("auto-curve", r.1 .0)));
    } else {
        app.exec(cmd);
    }
}

/// The curve with the dragged point or bend where the pointer is.
fn dragged(app: &CantusApp, g: &PointDrag, f: &Frame, pos: Pos2, off_grid: bool) -> Automation {
    let mut a = g.at_press.clone();
    match g.what {
        Grab::Point(i) if i < a.points.len() => {
            let raw = f.tick(pos.x).max(0.0);
            let mut tick = Ticks(raw.round() as i64);
            if app.arrangement.snap && !off_grid {
                tick = tick.snap(app.arrangement.grid);
            }
            // A point stays between its neighbours: the order is the curve.
            let lo = if i > 0 {
                a.points[i - 1].tick
            } else {
                Ticks::ZERO
            };
            let hi = a.points.get(i + 1).map_or(Ticks(i64::MAX / 4), |p| p.tick);
            a.points[i].tick = tick.clamp(lo, hi);
            a.points[i].value = f.value(pos.y);
        }
        Grab::Bend(i) if i + 1 < a.points.len() => {
            let (p, q) = (a.points[i], a.points[i + 1]);
            let v = f.value(pos.y);
            let mid = if (q.value - p.value).abs() > 1e-6 {
                (v - p.value) / (q.value - p.value)
            } else {
                0.5
            };
            a.points[i].tension = edit::tension_for_mid(p.shape, mid);
        }
        _ => {}
    }
    a
}

/// The hint for a point: its value in the parameter's units.
fn point_hint(app: &CantusApp, ui: &egui::Ui, a: &Automation, i: usize) {
    let Some(pt) = a.points.get(i) else { return };
    let project = app.project();
    let Some(range) = cantus_core::automation::range::range_of(project, &a.target) else {
        return;
    };
    let mut v = String::new();
    crate::automation::write_value(project, &a.target, range.value(pt.value), &mut v);
    hint::set_text(
        ui.ctx(),
        &v,
        &crate::i18n::t("auto.hint.point"),
        Caps::PARAM,
    );
}

/// A point's menu: its shape, and delete.
fn point_menu(app: &mut CantusApp, ui: &mut egui::Ui, r: (TrackId, ClipId), i: usize) {
    use crate::kit::menu::{Check, Item};
    use crate::kit::text::Src;
    let Some(mut a) = clip_auto(app, r) else {
        return;
    };
    let Some(now) = a.points.get(i).map(|p| p.shape) else {
        return;
    };
    let mut items: Vec<Item<'_>> = cantus_core::Shape::ALL
        .iter()
        .map(|sh| Item::Action {
            label: Src::Key(sh.label_key()),
            check: Check::Radio(*sh == now),
            shortcut: None,
            submenu: false,
            enabled: true,
        })
        .collect();
    items.push(Item::Separator);
    items.push(Item::action(Src::Key("auto.point.delete")));
    let Some(clicked) = crate::kit::widgets::menu_list(ui, &items, 200.0).1.clicked else {
        return;
    };
    if clicked < cantus_core::Shape::ALL.len() {
        a.points[i].shape = cantus_core::Shape::ALL[clicked];
    } else if a.points.len() > 1 {
        a.points.remove(i);
    }
    set_curve(app, r, a, false);
    ui.close();
}

/// The automation section of a clip's menu: open in the editor, and FL's articulators.
pub(super) fn clip_menu(app: &mut CantusApp, ui: &mut egui::Ui, r: (TrackId, ClipId)) {
    use crate::kit::menu::Item;
    use crate::kit::text::Src;
    let Some(mut a) = clip_auto(app, r) else {
        return;
    };
    let items = [
        Item::Band(Src::Key("auto.menu.band")),
        Item::action(Src::Key("auto.menu.open")),
        Item::action(Src::Key("auto.art.flip_v")),
        Item::action(Src::Key("auto.art.flip_h")),
        Item::action(Src::Key("auto.art.smooth")),
        Item::action(Src::Key("auto.art.randomize")),
        Item::action(Src::Key("auto.art.normalize")),
    ];
    let Some(clicked) = crate::kit::widgets::menu_list(ui, &items, 240.0).1.clicked else {
        return;
    };
    let len = app
        .project()
        .track(r.0)
        .and_then(|t| t.clip(r.1))
        .map_or(Ticks::ZERO, |c| c.length);
    match clicked {
        1 => app.open_automation_editor(r.0, r.1),
        2 => edit::flip_vertical(&mut a.points, &[]),
        3 => {
            let to = a.points.last().map_or(len, |p| p.tick.max(len));
            edit::flip_horizontal(&mut a.points, Ticks::ZERO, to)
        }
        4 => edit::smooth(&mut a.points, 0.5),
        5 => edit::randomize(&mut a.points, &[], 0.08, app.bus.revision()),
        6 => edit::normalize(&mut a.points),
        _ => return,
    }
    if clicked > 1 {
        set_curve(app, r, a, false);
    }
    ui.close();
}

/// The screen position of point `i` of an automation clip drawn this frame — for tests,
/// which aim at what was drawn rather than recompute the layout.
pub fn point_pos(app: &CantusApp, r: (TrackId, ClipId), i: usize) -> Option<Pos2> {
    let d = app
        .arrangement
        .auto
        .drawn
        .iter()
        .find(|d| (d.track, d.clip) == r)?;
    let a = clip_auto(app, r)?;
    let p = a.points.get(i)?;
    Some(d.frame.point(p.tick, p.value))
}

/// Where content tick `t` at value `v` of a drawn clip is on screen — for tests.
pub fn content_pos(app: &CantusApp, r: (TrackId, ClipId), t: Ticks, v: f32) -> Option<Pos2> {
    let d = app
        .arrangement
        .auto
        .drawn
        .iter()
        .find(|d| (d.track, d.clip) == r)?;
    Some(d.frame.point(t, v))
}

/// The automation section of a track's menu: automate its fader or pan (the strip's) or
/// its own level, and show or hide its lanes. Returns whether something was chosen.
pub(super) fn track_menu(app: &mut CantusApp, ui: &mut egui::Ui, id: TrackId) -> bool {
    use crate::kit::menu::{Check, Item};
    use crate::kit::text::Src;
    let Some(t) = app.project().track(id) else {
        return false;
    };
    let has = lane_count(t) > 0;
    let open = t.automation_open;
    let strip = t.strip;
    let mut items = vec![
        Item::Band(Src::Key("auto.menu.band")),
        Item::action(Src::Key("auto.track.volume")),
        Item::action(Src::Key("auto.track.pan")),
        Item::action(Src::Key("auto.track.level")),
    ];
    if has {
        items.push(Item::Action {
            label: Src::Key("auto.lane.show"),
            check: if open { Check::On } else { Check::Off },
            shortcut: None,
            submenu: false,
            enabled: true,
        });
    }
    let target = match crate::kit::widgets::menu_list(ui, &items, 220.0).1.clicked {
        Some(1) => strip
            .map(AutoTarget::StripGain)
            .unwrap_or(AutoTarget::TrackGain(id)),
        Some(2) => strip
            .map(AutoTarget::StripPan)
            .unwrap_or(AutoTarget::TrackPan(id)),
        Some(3) => AutoTarget::TrackGain(id),
        Some(4) => {
            app.exec(Command::SetTrackAutomationOpen { id, open: !open });
            return true;
        }
        _ => return false,
    };
    app.create_automation_clip(target);
    true
}
