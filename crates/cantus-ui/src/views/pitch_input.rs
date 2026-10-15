//! The pitch editor's input: moving around (the piano roll's wheel), the ruler (play from
//! where it is clicked), and the gestures on the blobs.

use super::{notes_in, Geo, Gesture, Snap, Tool};
use crate::app::CantusApp;
use crate::kit::hint::{self, Caps};
use crate::views::clips_chrome as chrome;
use cantus_core::pitch::curves::time_points;
use cantus_core::pitch::edit;
use cantus_core::pitch::{PitchEdit, PitchOp};
use cantus_core::{ClipId, TrackId};
use cantus_dsp::psola::TimeMap;
use cantus_engine::pitch::TakeAnalysis;
use egui::{CursorIcon, Id, Pos2, Rect, Sense};

/// The widest and closest the time axis zooms, points per frame.
const ZOOM_MIN: f64 = 1e-5;
const ZOOM_MAX: f64 = 0.5;
/// How near a blob's end a press stretches it, points.
const EDGE: f32 = 5.0;

/// The wheel and the middle button over the rows and the keys: scroll the rows, Shift for
/// time, Ctrl to zoom time and Ctrl+Alt the rows (the piano roll's keys).
pub(super) fn navigate(app: &mut CantusApp, ui: &egui::Ui, grid: Rect, keys: Rect) {
    let area = grid.union(keys);
    let Some(p) = ui
        .input(|i| i.pointer.hover_pos())
        .filter(|p| area.contains(*p))
    else {
        return;
    };
    let w = chrome::wheel(ui);
    let v = &mut app.pitch.view;
    if v.px_per_frame <= 0.0 {
        return;
    }
    if w.zoom_x != 1.0 {
        let at = v.scroll + (p.x - grid.left()) as f64 / v.px_per_frame;
        v.px_per_frame = (v.px_per_frame * w.zoom_x as f64).clamp(ZOOM_MIN, ZOOM_MAX);
        v.scroll = at - (p.x - grid.left()) as f64 / v.px_per_frame;
    }
    if w.zoom_y != 1.0 {
        let at = v.top - (p.y - grid.top()) / v.row_h;
        v.row_h = (v.row_h * w.zoom_y).clamp(super::ROW_MIN, super::ROW_MAX);
        v.top = at + (p.y - grid.top()) / v.row_h;
    }
    if w.zoom_x == 1.0 && w.zoom_y == 1.0 {
        v.scroll -= w.scroll.x as f64 / v.px_per_frame;
        v.top += w.scroll.y / v.row_h;
    }
    if ui.input(|i| i.pointer.middle_down()) {
        let d = ui.input(|i| i.pointer.delta());
        v.scroll -= d.x as f64 / v.px_per_frame;
        v.top += d.y / v.row_h;
        ui.ctx().set_cursor_icon(CursorIcon::Grabbing);
    }
    v.top = v.top.clamp(12.0, 132.0);
}

/// The scroll bars: time in seconds across the take, the rows from the top; their ends
/// zoom (the piano roll's bars).
pub(super) fn bars(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    grid: Rect,
    hbar: Rect,
    vbar: Rect,
    edit: &PitchEdit,
) {
    use chrome::Span;
    let v = &mut app.pitch.view;
    if v.px_per_frame <= 0.0 {
        return;
    }
    let rate = edit.rate.max(1) as f64;
    let total = (edit.frames as f64 / rate) as f32;
    let shown = (grid.width() as f64 / v.px_per_frame / rate) as f32;
    let start = (v.scroll / rate) as f32;
    let span = Span::new(start.max(0.0), shown, total.max(start + shown));
    let zoom = Some((
        (grid.width() as f64 / ZOOM_MAX / rate) as f32,
        (grid.width() as f64 / ZOOM_MIN / rate) as f32,
    ));
    if let Some(n) = chrome::bar(ui, hbar, ui.id().with("pitch-hbar"), span, zoom, "views.nav.hbar") {
        v.scroll = n.start as f64 * rate;
        v.px_per_frame = (grid.width() as f64 / (n.len as f64 * rate)).clamp(ZOOM_MIN, ZOOM_MAX);
    }
    // Rows from the top: the bar's start is how far below the highest the top row is.
    const HIGHEST: f32 = 132.0;
    let rows = HIGHEST - 12.0;
    let shown = grid.height() / v.row_h;
    let span = Span::new((HIGHEST - v.top).max(0.0), shown.min(rows), rows);
    let zoom = Some((grid.height() / super::ROW_MAX, grid.height() / super::ROW_MIN));
    if let Some(n) = chrome::bar(ui, vbar, ui.id().with("pitch-vbar"), span, zoom, "views.nav.vbar") {
        v.row_h = (grid.height() / n.len).clamp(super::ROW_MIN, super::ROW_MAX);
        v.top = (HIGHEST - n.start).clamp(12.0, HIGHEST);
    }
}

/// Which part of a blob is under a point: its body, or one of its ends.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum Part {
    Body,
    Left,
    Right,
}

fn hit(blobs: &[(u32, Rect)], p: Pos2, k: f32) -> Option<(u32, Part)> {
    let (id, r) = blobs.iter().rev().find(|(_, r)| r.expand2(egui::vec2(1.0, 2.0)).contains(p))?;
    let e = (EDGE * k).min(r.width() / 4.0);
    let part = if p.x - r.left() <= e {
        Part::Left
    } else if r.right() - p.x <= e {
        Part::Right
    } else {
        Part::Body
    };
    Some((*id, part))
}

/// The canvas's pointer and keys.
#[allow(clippy::too_many_arguments)]
pub(super) fn canvas(
    app: &mut CantusApp,
    ui: &egui::Ui,
    track: TrackId,
    clip: ClipId,
    current: &PitchEdit,
    analysis: Option<&TakeAnalysis>,
    geo: &Geo,
) {
    let k = crate::kit::Style::current().k;
    let resp = ui.interact(geo.grid, Id::new(("pitch-canvas", clip.0)), Sense::click_and_drag());
    let ruler = ui.interact(geo.ruler, Id::new(("pitch-ruler", clip.0)), Sense::click_and_drag());
    // The ruler: the playhead goes where it is clicked or dragged.
    if ruler.clicked() || ruler.dragged() {
        if let (Some(p), Some(tm)) = (ruler.interact_pointer_pos(), app.clip_time(track, clip)) {
            app.seek(tm.tick(geo.frame(p.x)));
        }
    }
    if ruler.hovered() {
        hint::set_text(ui.ctx(), &crate::i18n::t("pitch.ruler_tip"), "", Caps::NONE);
    }
    let blobs = app.pitch.view.blobs.clone();
    let pointer = ui.input(|i| i.pointer.hover_pos());
    let hover = pointer.filter(|p| geo.grid.contains(*p)).and_then(|p| hit(&blobs, p, k));
    let tool = app.pitch.view.tool;
    if resp.hovered() {
        let icon = match (tool, hover.map(|h| h.1), &app.pitch.view.gesture) {
            (_, _, Some(Gesture::Pitch { .. })) => CursorIcon::ResizeVertical,
            (_, _, Some(Gesture::Move { .. } | Gesture::Stretch { .. })) => {
                CursorIcon::ResizeHorizontal
            }
            (Tool::Split, Some(_), _) => CursorIcon::VerticalText,
            (Tool::Edit, Some(Part::Left | Part::Right), _) => CursorIcon::ResizeHorizontal,
            (Tool::Edit, Some(Part::Body), _) => CursorIcon::Grab,
            _ => CursorIcon::Default,
        };
        ui.ctx().set_cursor_icon(icon);
        match hover.and_then(|(id, _)| current.note(id)) {
            Some(n) => {
                let txt = describe(n, current.rate);
                hint::set_text(ui.ctx(), &txt, "", Caps::NONE);
            }
            None => hint::set_text(ui.ctx(), &crate::i18n::t("pitch.canvas_tip"), "", Caps::NONE),
        }
    }
    let (pressed, down, released, shift, alt) = ui.input(|i| {
        (
            i.pointer.primary_pressed(),
            i.pointer.primary_down(),
            i.pointer.primary_released(),
            i.modifiers.shift,
            i.modifiers.alt,
        )
    });
    let press_pos = ui.input(|i| i.pointer.press_origin());
    if pressed && press_pos.is_some_and(|p| geo.grid.contains(p)) {
        let p = press_pos.expect("checked");
        let hit_now = hit(&blobs, p, k);
        match (tool, hit_now) {
            (Tool::Split, Some((id, _))) => {
                let map = TimeMap::new(time_points(current)).unwrap_or_default();
                let at = map.source(geo.frame(p.x)).max(0.0) as u64;
                let mut e = current.clone();
                let frames = analysis.map(|a| a.frames_view());
                if edit::split(&mut e, id, at, frames.as_ref()) {
                    app.commit_pitch(track, clip, Some(e), PitchOp::Split, false);
                }
            }
            (Tool::Edit, Some((id, part))) => {
                let v = &mut app.pitch.view;
                if shift {
                    if let Some(i) = v.selected.iter().position(|s| *s == id) {
                        v.selected.remove(i);
                    } else {
                        v.selected.push(id);
                    }
                } else if !v.selected.contains(&id) {
                    v.selected = vec![id];
                }
                let at_press = current.clone();
                v.gesture = Some(match part {
                    Part::Body => Gesture::Press { id, at_press, from: p },
                    Part::Left | Part::Right => Gesture::Stretch {
                        id,
                        left: part == Part::Left,
                        at_press,
                        from: p,
                    },
                });
            }
            (_, None) => {
                let v = &mut app.pitch.view;
                if !shift {
                    v.selected.clear();
                }
                v.gesture = Some(Gesture::Marquee { from: p, to: p });
            }
        }
    }
    if down {
        if let Some(p) = pointer {
            drag(app, track, clip, geo, p, alt, &blobs, shift);
        }
    }
    if released || (!down && app.pitch.view.gesture.is_some()) {
        let was = app.pitch.view.gesture.take();
        if matches!(
            was,
            Some(Gesture::Pitch { .. } | Gesture::Move { .. } | Gesture::Stretch { .. })
        ) {
            app.end_gesture();
        }
    }
    // A double-click on a blob pulls its centre onto its note, or lets it go.
    if resp.double_clicked() && tool == Tool::Edit {
        if let Some((id, _)) = resp.interact_pointer_pos().and_then(|p| hit(&blobs, p, k)) {
            let mut e = current.clone();
            let ids = selection_or(app, id);
            let all = ids.iter().all(|i| e.note(*i).is_some_and(|n| n.centre == 1.0));
            edit::set_field(&mut e, &ids, edit::Field::Centre, if all { 0.0 } else { 1.0 });
            app.commit_pitch(track, clip, Some(e), PitchOp::Centre, false);
        }
    }
    if resp.hovered() || resp.has_focus() || app.pitch.view.geo.is_some() && pointer.is_some_and(|p| geo.panel.contains(p)) {
        keys(app, ui, track, clip, current);
    }
}

/// The selection, or `id` when it is not in it.
fn selection_or(app: &CantusApp, id: u32) -> Vec<u32> {
    let sel = &app.pitch.view.selected;
    if sel.contains(&id) {
        sel.clone()
    } else {
        vec![id]
    }
}

/// A note in words, for the help line: its name, how far off it sounds, its length.
fn describe(n: &cantus_core::pitch::PitchNote, rate: u32) -> String {
    let c = n.out_centre();
    let near = c.round();
    let cents = ((c - near) * 100.0).round() as i32;
    let secs = (n.end - n.start) as f64 / rate.max(1) as f64;
    crate::i18n::tf(
        "pitch.note_tip",
        &[
            ("note", &cantus_core::pitch_name(near.clamp(0.0, 127.0) as u8)),
            ("cents", &format!("{cents:+}")),
            ("secs", &format!("{secs:.2}")),
        ],
    )
}

/// The press moved: choose the direction, then move, stretch or select.
#[allow(clippy::too_many_arguments)]
fn drag(
    app: &mut CantusApp,
    track: TrackId,
    clip: ClipId,
    geo: &Geo,
    p: Pos2,
    alt: bool,
    blobs: &[(u32, Rect)],
    shift: bool,
) {
    let Some(g) = app.pitch.view.gesture.clone() else {
        return;
    };
    let sel = app.pitch.view.selected.clone();
    match g {
        Gesture::Press { id, at_press, from } => {
            let d = p - from;
            if d.length() > 4.0 {
                app.pitch.view.gesture = Some(if d.y.abs() >= d.x.abs() {
                    Gesture::Pitch { id, at_press, from }
                } else {
                    Gesture::Move { at_press, from }
                });
                drag(app, track, clip, geo, p, alt, blobs, shift);
            }
        }
        Gesture::Pitch { id, at_press, from } => {
            let delta = (from.y - p.y) / geo.row_h;
            let snap = if alt { Snap::Free } else { app.pitch.view.snap };
            let mut e = at_press.clone();
            let anchor = at_press.note(id).map_or(0.0, |n| n.pitch);
            for n in at_press.notes.iter().filter(|n| sel.contains(&n.id)) {
                let to = match snap {
                    Snap::Free => n.pitch + (delta * 100.0).round() / 100.0,
                    Snap::Semitone => n.pitch + delta.round(),
                    Snap::Scale => {
                        // The anchor lands on the key's note nearest the pointer; the
                        // others follow by as many scale steps.
                        let target = at_press.key.nearest(anchor + delta);
                        let steps = scale_steps(&at_press, anchor, target);
                        step_scale(&at_press, n.pitch, steps)
                    }
                };
                edit::set_pitch(&mut e, n.id, to);
            }
            if e != *current_of(app, track, clip).as_ref().unwrap_or(&at_press) {
                app.commit_pitch(track, clip, Some(e), PitchOp::Pitch, true);
            }
        }
        Gesture::Move { at_press, from, .. } => {
            let delta = ((p.x - from.x) as f64 / geo.px_per_frame).round() as i64;
            let mut e = at_press.clone();
            let mut ids: Vec<u32> = at_press
                .notes
                .iter()
                .filter(|n| sel.contains(&n.id))
                .map(|n| n.id)
                .collect();
            if delta > 0 {
                ids.reverse();
            }
            for id in ids {
                edit::shift_timing(&mut e, id, delta);
            }
            if e != *current_of(app, track, clip).as_ref().unwrap_or(&at_press) {
                app.commit_pitch(track, clip, Some(e), PitchOp::Timing, true);
            }
        }
        Gesture::Stretch {
            id,
            left,
            at_press,
            from,
        } => {
            let delta = ((p.x - from.x) as f64 / geo.px_per_frame).round() as i64;
            let mut e = at_press.clone();
            if let Some(n) = at_press.note(id) {
                let (s, en) = (n.start as i64, n.end as i64);
                let (s, en) = if left { (s + delta, en) } else { (s, en + delta) };
                edit::set_timing(&mut e, id, s.max(0) as u64, en.max(0) as u64);
            }
            if e != *current_of(app, track, clip).as_ref().unwrap_or(&at_press) {
                app.commit_pitch(track, clip, Some(e), PitchOp::Timing, true);
            }
        }
        Gesture::Marquee { from, .. } => {
            app.pitch.view.gesture = Some(Gesture::Marquee { from, to: p });
            let mut picked = notes_in(blobs, Rect::from_two_pos(from, p));
            if shift {
                for id in sel {
                    if !picked.contains(&id) {
                        picked.push(id);
                    }
                }
            }
            app.pitch.view.selected = picked;
        }
    }
}

fn current_of(app: &CantusApp, track: TrackId, clip: ClipId) -> Option<PitchEdit> {
    app.stored_pitch(track, clip).cloned()
}

/// How many scale steps lie from `a` to `b` (both on the scale), signed.
fn scale_steps(e: &PitchEdit, a: f32, b: f32) -> i32 {
    let (lo, hi) = (a.min(b).round() as i32, a.max(b).round() as i32);
    let n = (lo + 1..=hi).filter(|m| e.key.contains(*m)).count() as i32;
    if b >= a {
        n
    } else {
        -n
    }
}

/// `p` moved `steps` steps of the key's scale (a fractional offset kept).
fn step_scale(e: &PitchEdit, p: f32, steps: i32) -> f32 {
    if steps == 0 {
        return p;
    }
    let base = e.key.nearest(p);
    e.key.degrees_from(base, steps) + (p - base)
}

/// The keys: ↑/↓ move the selection a step (Alt: 10 cents), Ctrl+A selects every note,
/// Ctrl+J joins the selection, Esc lets it go.
fn keys(app: &mut CantusApp, ui: &egui::Ui, track: TrackId, clip: ClipId, current: &PitchEdit) {
    let (up, down, all, join, esc, alt) = ui.input(|i| {
        (
            i.key_pressed(egui::Key::ArrowUp),
            i.key_pressed(egui::Key::ArrowDown),
            i.modifiers.command && i.key_pressed(egui::Key::A),
            i.modifiers.command && i.key_pressed(egui::Key::J),
            i.key_pressed(egui::Key::Escape),
            i.modifiers.alt,
        )
    });
    if all {
        app.pitch.view.selected = current.notes.iter().map(|n| n.id).collect();
    }
    if esc {
        app.pitch.view.selected.clear();
    }
    let sel = app.pitch.view.selected.clone();
    if join && sel.len() >= 2 {
        let mut e = current.clone();
        if edit::join(&mut e, &sel) {
            app.pitch.view.selected.truncate(1);
            app.commit_pitch(track, clip, Some(e), PitchOp::Join, false);
        }
    }
    if (up || down) && !sel.is_empty() {
        let dir = if up { 1 } else { -1 };
        let mut e = current.clone();
        for n in current.notes.iter().filter(|n| sel.contains(&n.id)) {
            let to = if alt {
                n.pitch + 0.1 * dir as f32
            } else {
                match app.pitch.view.snap {
                    Snap::Scale => step_scale(current, n.pitch, dir),
                    _ => n.pitch + dir as f32,
                }
            };
            edit::set_pitch(&mut e, n.id, to);
        }
        app.commit_pitch(track, clip, Some(e), PitchOp::Pitch, false);
    }
}
