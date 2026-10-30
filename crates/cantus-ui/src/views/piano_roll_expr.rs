//! **Per-note expression in the piano roll** (round 33, ADR 0080): Bitwig's and Logic's
//! idea in FL's look. On the notes, each one's bend is a line through it (where it bends to,
//! in rows), its pressure the line's weight, its timbre the line's colour — each switchable
//! from the lane's + menu. Under the roll, a row per kind shows the selected notes' curves
//! (all the clip's when none is selected) over their own spans and edits them: the pencil
//! draws (thinned as it goes), the line tool draws a straight stretch, Shift snaps a bend to
//! whole semitones, and a right-click offers smooth, thin, scale, flatten and clear. Every
//! gesture is one `SetNoteExpr` — one undo step, named in the history.

use super::lanes::LaneTool;
use super::{ticks_to_beats, Geo};
use crate::app::CantusApp;
use crate::i18n::t;
use crate::kit::{self, Brush, Fill, Style};
use cantus_core::model::note_expr::{ops, with_curve, ExprEdit, ExprKind, ExprPoint};
use cantus_core::{Clip, ClipId, Note, NoteId, Ticks, TrackId};
use egui::{pos2, Pos2, Rect, Sense, Ui};

/// A stroke under way in an expression row.
#[derive(Clone, Debug)]
pub struct Stroke {
    kind: ExprKind,
    /// `(content tick, stored value)` so far (the pencil), or the press (the line).
    points: Vec<(i64, i64)>,
    line: bool,
}

/// The value axis of a row: the bend about a centre line over ±`range` semitones (the
/// widest bend shown, rounded up to 2, 12, 24, 48 or 96), pressure and timbre 0 to 1.
#[derive(Copy, Clone, Debug)]
struct Axis {
    kind: ExprKind,
    top: f32,
    bottom: f32,
    range: f64,
}

impl Axis {
    fn of(kind: ExprKind, body: Rect, notes: &[&Note], s: &Style) -> Axis {
        let widest = notes
            .iter()
            .filter_map(|n| n.expr.as_deref())
            .flat_map(|e| e.bend.iter())
            .map(|p| ExprKind::Bend.read(p.1).abs())
            .fold(0.0f64, f64::max);
        let range = [2.0, 12.0, 24.0, 48.0, 96.0]
            .into_iter()
            .find(|r| widest <= *r)
            .unwrap_or(128.0);
        Axis {
            kind,
            top: body.top() + 6.0 * s.k,
            bottom: body.bottom() - 5.0 * s.k,
            range,
        }
    }

    /// 0…1 up the row for a stored value.
    fn unit(&self, raw: i64) -> f32 {
        let v = self.kind.read(raw);
        match self.kind {
            ExprKind::Bend => (0.5 + v / (2.0 * self.range)) as f32,
            _ => v as f32,
        }
        .clamp(0.0, 1.0)
    }

    fn y(&self, raw: i64) -> f32 {
        self.bottom - self.unit(raw) * (self.bottom - self.top)
    }

    /// The stored value at screen `y`.
    fn raw(&self, y: f32) -> i64 {
        let u = f64::from(((self.bottom - y) / (self.bottom - self.top).max(1.0)).clamp(0.0, 1.0));
        let v = match self.kind {
            ExprKind::Bend => (u - 0.5) * 2.0 * self.range,
            _ => u,
        };
        self.kind.store(v)
    }
}

/// The notes a row edits and shows: the selection, else every note of the clip.
fn targets<'a>(app: &CantusApp, clip: &'a Clip) -> Vec<&'a Note> {
    let sel: Vec<&Note> = clip
        .notes
        .iter()
        .filter(|n| app.selected_notes.contains(&n.id))
        .collect();
    if sel.is_empty() {
        clip.notes.iter().collect()
    } else {
        sel
    }
}

/// One row: its ground and centre line, each target note's span and its curve over it.
#[allow(clippy::too_many_arguments)]
pub(crate) fn paint(
    b: &mut Brush,
    app: &CantusApp,
    geo: &Geo<'_>,
    body: Rect,
    clip: &Clip,
    kind: ExprKind,
    ink: egui::Color32,
    grid: Ticks,
    s: &Style,
) {
    let t = &s.t;
    let body = b.snap_rect(body);
    b.register(
        body,
        "expression_lane",
        &[kit::Tok::RowBlack, kit::Tok::Grid],
        &[],
    );
    b.rect(body, Fill::Solid(kit::color::mix(t.row_black, t.ink, 0.25)));
    super::paint::ladder(b, app, geo, body, clip, grid, s);
    let notes = targets(app, clip);
    let ax = Axis::of(kind, body, &notes, s);
    let rest = ax.y(kind.rest());
    b.hline(body.left(), body.right(), rest, b.px(), t.grid_strong);
    for n in &notes {
        let x0 = geo.x(ticks_to_beats(n.start));
        let x1 = geo.x(ticks_to_beats(n.end()));
        if x1 < body.left() || x0 > body.right() {
            continue;
        }
        // The note's span, faint, so a note without a curve is still a place to draw.
        b.rect(
            Rect::from_min_max(pos2(x0, ax.bottom + 1.0), pos2(x1, ax.bottom + 3.0 * s.k)),
            Fill::Solid(kit::color::with_alpha(ink, 0.5)),
        );
        let Some(c) = n
            .expr
            .as_deref()
            .map(|e| e.curve(kind))
            .filter(|c| !c.is_empty())
        else {
            continue;
        };
        let pts = curve_points(c, n, geo, x0, x1, |raw| ax.y(raw));
        b.polyline(&pts, 1.5 * s.k, ink);
        // Its points, where they are far enough apart to be told apart (a played curve is
        // a line, not a string of beads).
        let mut last = f32::NEG_INFINITY;
        for p in c.iter().filter(|p| p.0 >= 0 && p.0 <= n.length.0) {
            let x = geo.x(ticks_to_beats(n.start + Ticks(p.0)));
            if x - last >= 8.0 * s.k {
                b.dot(pos2(x, ax.y(p.1)), 2.0 * s.k, t.text);
                last = x;
            }
        }
    }
}

/// A curve as screen points across its note, a point every few pixels.
fn curve_points(
    c: &[ExprPoint],
    n: &Note,
    geo: &Geo<'_>,
    x0: f32,
    x1: f32,
    y: impl Fn(i64) -> f32,
) -> Vec<Pos2> {
    let steps = (((x1 - x0) / 3.0).ceil() as usize).clamp(1, 512);
    (0..=steps)
        .map(|i| {
            let at = n.length.0 * i as i64 / steps as i64;
            let raw = cantus_core::model::note_expr::raw_at(c, at).unwrap_or(0);
            pos2(geo.x(ticks_to_beats(n.start + Ticks(at))), y(raw))
        })
        .collect()
}

/// What the notes show of their expression: the bend as a line through each note, in rows
/// (a bend of a semitone reaches the next row), pressure as its weight, timbre as its
/// colour.
pub(crate) fn paint_on_notes(
    b: &mut Brush,
    geo: &Geo<'_>,
    clip: &Clip,
    show: [bool; 3],
    s: &Style,
) {
    if !show.iter().any(|x| *x) {
        return;
    }
    let t = &s.t;
    for n in clip.notes.iter().filter(|n| n.expr.is_some() && !n.muted) {
        let Some(e) = n.expr.as_deref() else {
            continue;
        };
        let r = geo.note_rect(n);
        if r == Rect::NOTHING || r.width() < 3.0 {
            continue;
        }
        let mid = r.center().y;
        let bend = if show[0] { e.bend.as_slice() } else { &[] };
        let pts = curve_points(bend, n, geo, r.left(), r.right(), |raw| {
            mid - ExprKind::Bend.read(raw) as f32 * geo.key_h
        });
        let at = |c: &[ExprPoint], kind: ExprKind| {
            cantus_core::model::note_expr::raw_at(c, n.length.0 / 2)
                .map(|raw| kind.read(raw) as f32)
        };
        let weight = if show[1] {
            at(&e.pressure, ExprKind::Pressure).unwrap_or(0.3)
        } else {
            0.3
        };
        let colour = if show[2] {
            at(&e.timbre, ExprKind::Timbre).unwrap_or(0.0)
        } else {
            0.0
        };
        let c = kit::color::mix(t.text, t.highlight, colour);
        b.polyline(&pts, (1.0 + 2.5 * weight) * s.k, c);
    }
}

/// A row's gestures: draw with the pencil or the line, Shift snapping a bend to whole
/// semitones; a right-click's menu of edits.
#[allow(clippy::too_many_arguments)]
pub(crate) fn input(
    app: &mut CantusApp,
    ui: &mut Ui,
    geo: &Geo<'_>,
    body: Rect,
    resp: &egui::Response,
    kind: ExprKind,
    track: TrackId,
    clip_id: ClipId,
) {
    let s = Style::current();
    let Some(clip) = app
        .project()
        .track(track)
        .and_then(|t| t.clip(clip_id))
        .cloned()
    else {
        return;
    };
    let notes: Vec<Note> = targets(app, &clip).into_iter().cloned().collect();
    let refs: Vec<&Note> = notes.iter().collect();
    let ax = Axis::of(kind, body, &refs, &s);
    let snap = ui.input(|i| i.modifiers.shift) && kind == ExprKind::Bend;
    let at = |p: Pos2| {
        let tick = geo.tick_at(p.x).0;
        let raw = ax.raw(p.y);
        (tick, if snap { ops::snap_semitone(raw) } else { raw })
    };
    if resp.drag_started_by(egui::PointerButton::Primary) {
        if let Some(p) = resp.interact_pointer_pos() {
            app.piano_roll.lanes.expr_drag = Some(Stroke {
                kind,
                points: vec![at(p)],
                line: app.piano_roll.lanes.tool != LaneTool::Pencil,
            });
        }
    }
    if resp.dragged_by(egui::PointerButton::Primary) {
        if let (Some(p), Some(d)) = (
            resp.interact_pointer_pos(),
            app.piano_roll.lanes.expr_drag.as_mut(),
        ) {
            let v = at(p);
            if d.line {
                d.points.truncate(1);
                d.points.push(v);
            } else if d.points.last().is_none_or(|l| l.0 != v.0) {
                d.points.push(v);
            }
        }
    }
    if resp.drag_stopped() || resp.clicked() {
        let stroke = app.piano_roll.lanes.expr_drag.take().or_else(|| {
            resp.interact_pointer_pos().map(|p| Stroke {
                kind,
                points: vec![at(p)],
                line: false,
            })
        });
        if let Some(st) = stroke.filter(|st| st.kind == kind) {
            commit(app, track, clip_id, &notes, &st);
        }
    }
    egui::Popup::context_menu(resp)
        .frame(crate::views::clips_chrome::menu_frame())
        .show(|ui| {
            let items = [
                (ExprEdit::Smooth, "mpe.edit.smooth"),
                (ExprEdit::Thin, "mpe.edit.thin"),
                (ExprEdit::Scale, "mpe.edit.less"),
                (ExprEdit::Scale, "mpe.edit.more"),
                (ExprEdit::Flatten, "mpe.edit.flatten"),
                (ExprEdit::Clear, "mpe.edit.clear"),
            ];
            let words: Vec<String> = items.iter().map(|(_, k)| t(k)).collect();
            let refs: Vec<kit::menu::Item<'_>> = words
                .iter()
                .map(|w| kit::menu::Item::Action {
                    label: kit::text::Src::Str(w),
                    check: kit::menu::Check::None,
                    shortcut: None,
                    submenu: false,
                    enabled: true,
                })
                .collect();
            if let Some(i) = kit::widgets::menu_list(ui, &refs, 200.0).1.clicked {
                let (edit, key) = items[i];
                edit_curves(
                    app,
                    track,
                    clip_id,
                    &notes,
                    kind,
                    edit,
                    key == "mpe.edit.more",
                );
                ui.close();
            }
        });
    let _ = Sense::hover();
}

/// The stroke into each target note it crosses: one undo step.
fn commit(app: &mut CantusApp, track: TrackId, clip: ClipId, notes: &[Note], st: &Stroke) {
    let mut pts = st.points.clone();
    pts.sort_by_key(|p| p.0);
    pts.dedup_by_key(|p| p.0);
    let (Some(a), Some(z)) = (pts.first().copied(), pts.last().copied()) else {
        return;
    };
    // A click: the value from there to the note's end.
    let single = pts.len() == 1;
    let ids: Vec<NoteId> = notes
        .iter()
        .filter(|n| {
            let (s0, s1) = (n.start.0, n.end().0);
            if single {
                a.0 >= s0 && a.0 < s1
            } else {
                z.0 >= s0 && a.0 < s1
            }
        })
        .map(|n| n.id)
        .collect();
    let kind = st.kind;
    let cmd = cantus_core::model::note_expr::cmd::command_for(
        app.project(),
        track,
        clip,
        &ids,
        ExprEdit::Draw,
        |n| {
            let mut rel: Vec<ExprPoint> = pts
                .iter()
                .filter(|p| p.0 >= n.start.0 && p.0 <= n.end().0)
                .map(|p| ExprPoint(p.0 - n.start.0, p.1))
                .collect();
            if single {
                rel = vec![
                    ExprPoint((a.0 - n.start.0).max(0), a.1),
                    ExprPoint(n.length.0, a.1),
                ];
            }
            if rel.is_empty() {
                return n.expr.clone();
            }
            let rel = ops::thin(&rel, ops::default_tolerance(kind));
            let curve = n.expr.as_deref().map(|e| e.curve(kind)).unwrap_or(&[]);
            with_curve(n.expr.as_deref(), kind, ops::draw(curve, &rel))
        },
    );
    if let Some(c) = cmd {
        app.exec(c);
    }
}

/// A menu edit of the target notes' curves of `kind`: one undo step.
fn edit_curves(
    app: &mut CantusApp,
    track: TrackId,
    clip: ClipId,
    notes: &[Note],
    kind: ExprKind,
    edit: ExprEdit,
    more: bool,
) {
    let ids: Vec<NoteId> = notes.iter().map(|n| n.id).collect();
    let cmd = cantus_core::model::note_expr::cmd::command_for(
        app.project(),
        track,
        clip,
        &ids,
        edit,
        |n| {
            let e = n.expr.as_deref()?;
            let c = e.curve(kind);
            if c.is_empty() {
                return n.expr.clone();
            }
            let next = match edit {
                ExprEdit::Smooth => ops::smooth(c, kind, 4),
                ExprEdit::Thin => ops::thin(c, ops::default_tolerance(kind) * 4),
                ExprEdit::Scale => ops::scale(c, kind, if more { 1.5 } else { 0.5 }),
                ExprEdit::Flatten => ops::flatten(c, kind, n.length.0),
                _ => Vec::new(),
            };
            with_curve(Some(e), kind, next)
        },
    );
    if let Some(c) = cmd {
        app.exec(c);
    }
}
