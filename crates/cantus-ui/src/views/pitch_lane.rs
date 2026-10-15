//! **The pitch lane** (round 28, review 1; BACKLOG §3c item 4): an audio track's notes in a
//! lane under it in the arrangement, for small fixes without opening the editor. Each
//! note of its clips (the stored edit, or the take's notes as analysed) is a bar at the
//! pitch it sounds, where it plays; drag one up or down to move it (the editor's snap:
//! the key's scale, semitones, or free — Alt for cents), double-click it to pull its
//! centre onto its note (again to let it go). The same `SetClipPitch` the editor sends,
//! one undo step a gesture. Shown from the track's menu (*Pitch lane*, stored with the
//! track, `SetTrackPitchOpen`).

use super::arrangement::{TimelineGeometry, TrackRow};
use crate::app::CantusApp;
use crate::kit::color::{shade, with_alpha};
use crate::kit::hint::{self, Caps};
use crate::kit::menu::{Check, Item};
use crate::kit::text::{self, Elide, Policy, Role, Src, Text};
use crate::kit::{self, Brush, Fill, Style, Tok};
use crate::pitch::Take;
use cantus_core::pitch::{edit, PitchEdit, PitchOp};
use cantus_core::{ClipId, Command, Track, TrackId};
use egui::{pos2, Align2, Color32, Id, Pos2, Rect, Sense};

/// The lane's height, in track heights.
const HEIGHT: f32 = 1.5;

/// A gesture in a lane: a note's pitch dragged.
#[derive(Clone, Debug)]
pub struct LaneDrag {
    track: TrackId,
    clip: ClipId,
    note: u32,
    at_press: PitchEdit,
    from: Pos2,
    px_per_st: f32,
}

/// The lanes' state, kept on the app (`app.pitch.lane`).
#[derive(Debug, Default)]
pub struct LaneState {
    pub(crate) drag: Option<LaneDrag>,
    /// The note last pressed in a lane: (track, clip, note).
    pub selected: Option<(TrackId, ClipId, u32)>,
}

/// The height the lane adds under a track: shown, and the track has audio clips.
pub fn height(t: &Track, base: f32) -> f32 {
    if t.pitch_open && t.clips.iter().any(|c| c.audio.is_some()) {
        (base * HEIGHT).round().max(36.0)
    } else {
        0.0
    }
}

/// Where a row's lane is, across `lanes` (the timeline's width).
pub fn lane_rect(row: &TrackRow, lanes: Rect) -> Option<Rect> {
    (row.pitch > 0.0).then(|| {
        let top = row.top + row.height + row.takes + row.auto;
        Rect::from_min_max(
            pos2(lanes.left(), top),
            pos2(lanes.right(), top + row.pitch),
        )
    })
}

/// One note in a lane, on screen.
#[derive(Clone, Debug, PartialEq)]
pub struct LaneNote {
    pub track: TrackId,
    pub clip: ClipId,
    pub id: u32,
    pub rect: Rect,
    /// Points per semitone in its lane.
    pub px_per_st: f32,
}

/// A lane laid out: its rect, its notes, its pitch range, the spans still analysed.
#[derive(Clone, Debug)]
pub struct Lane {
    pub track: Option<TrackId>,
    pub rect: Rect,
    pub notes: Vec<LaneNote>,
    pub range: (f32, f32),
    pub pending: Vec<Rect>,
}

/// Every open lane in view, laid out (the painter and the input both ask).
pub fn layout(app: &CantusApp, geo: &TimelineGeometry, lanes: Rect) -> Vec<Lane> {
    let mut out = Vec::new();
    for (i, row) in geo.rows.iter().enumerate() {
        let Some(track) = app.project().tracks.get(i) else {
            continue;
        };
        let Some(rect) = lane_rect(row, lanes).filter(|r| r.intersects(lanes)) else {
            continue;
        };
        // Each clip's notes on the timeline, cut to its window.
        let mut found: Vec<(ClipId, u32, f32, f32, f32)> = Vec::new();
        let mut pending = Vec::new();
        for c in track
            .clips
            .iter()
            .filter(|c| c.audio.is_some() && c.lane == 0)
        {
            let (x0, x1) = (
                geo.x(super::ticks_to_beats(c.start)),
                geo.x(super::ticks_to_beats(c.end())),
            );
            if x1 < lanes.left() || x0 > lanes.right() {
                continue;
            }
            let take = app.pitch_take(track.id, c.id);
            let (Some(e), Some(tm)) = (
                app.pitch_edit(track.id, c.id, &take),
                app.clip_time(track.id, c.id),
            ) else {
                if matches!(take, Take::Pending) {
                    pending.push(Rect::from_min_max(
                        pos2(x0, rect.top()),
                        pos2(x1, rect.bottom()),
                    ));
                }
                continue;
            };
            for n in &e.notes {
                let (a, b) = (tm.tick(n.start as f64), tm.tick(n.end as f64));
                let (a, b) = (a.min(b).max(c.start), a.max(b).min(c.end()));
                if b <= a {
                    continue;
                }
                let (xa, xb) = (
                    geo.x(super::ticks_to_beats(a)),
                    geo.x(super::ticks_to_beats(b)),
                );
                found.push((c.id, n.id, xa, xb, n.out_centre()));
            }
        }
        let (lo, hi) = found.iter().fold((f32::MAX, f32::MIN), |(lo, hi), f| {
            (lo.min(f.4), hi.max(f.4))
        });
        let (lo, hi) = if lo <= hi {
            let mid = (lo + hi) * 0.5;
            let half = ((hi - lo) * 0.5 + 1.5).max(3.0);
            (mid - half, mid + half)
        } else {
            (57.0, 69.0)
        };
        let px_per_st = rect.height() / (hi - lo);
        let bar = (px_per_st * 0.8).clamp(3.0, 9.0);
        let notes = found
            .into_iter()
            .map(|(clip, id, xa, xb, m)| {
                let y = rect.bottom() - (m - lo) * px_per_st;
                LaneNote {
                    track: track.id,
                    clip,
                    id,
                    rect: Rect::from_min_max(
                        pos2(xa, y - bar * 0.5),
                        pos2(xb.max(xa + 2.0), y + bar * 0.5),
                    ),
                    px_per_st,
                }
            })
            .collect();
        out.push(Lane {
            track: Some(track.id),
            rect,
            notes,
            range: (lo, hi),
            pending,
        });
    }
    out
}

/// Paint every open lane: its ground, a line at each C, the notes in the track's colour.
pub fn paint(b: &mut Brush, app: &CantusApp, geo: &TimelineGeometry, lanes: Rect, s: &Style) {
    let t = &s.t;
    for lane in layout(app, geo, lanes) {
        let r = lane.rect;
        b.register(
            r,
            "pitch_lane",
            &[Tok::RowBlack, Tok::Grid, Tok::Selection],
            &[],
        );
        b.rect(b.snap_rect(r), Fill::Solid(shade(t.row_black, -0.04)));
        b.hline(
            r.left(),
            r.right(),
            r.top() + 0.5 * b.px(),
            b.px(),
            t.grid_strong,
        );
        let (lo, hi) = lane.range;
        let px = r.height() / (hi - lo);
        let mut c = (lo / 12.0).ceil() * 12.0;
        while c < hi {
            let y = r.bottom() - (c - lo) * px;
            b.hline(r.left(), r.right(), y, b.px(), with_alpha(t.grid, 0.8));
            c += 12.0;
        }
        let color = lane
            .track
            .and_then(|id| app.project().track(id))
            .map(|tr| crate::theme::track_color(tr.color))
            .unwrap_or(t.brand);
        for n in &lane.notes {
            let sel = app.pitch.lane.selected == Some((n.track, n.clip, n.id));
            let ink = super::pitch::paint_ink(color, sel, s);
            b.rrect(n.rect, n.rect.height() * 0.5, Fill::Solid(ink.0));
            b.rrect_stroke(n.rect, n.rect.height() * 0.5, b.px(), ink.1);
        }
        for p in &lane.pending {
            let tx = Text::key("pitch.analysing_short", Role::Label, t.text_dim)
                .align(Align2::LEFT_CENTER)
                .policy(Policy::Elide(Elide::End));
            let _ = text::paint(b, p.shrink2(egui::vec2(6.0 * s.k, 0.0)), &tx, s.tk());
        }
    }
}

/// The lane's name in the header column.
pub fn paint_header(b: &mut Brush, row: &TrackRow, headers: Rect, tint: Color32, s: &Style) {
    let Some(r) = lane_rect(row, headers) else {
        return;
    };
    b.register(r, "pitch_lane_header", &[Tok::Panel, Tok::TextDim], &[]);
    b.rect(b.snap_rect(r), Fill::Solid(shade(s.t.panel, -0.03)));
    b.rect(
        Rect::from_min_max(r.min, pos2(r.left() + 3.0 * s.k, r.bottom())),
        Fill::Solid(tint),
    );
    let tx = Text::key("pitch.lane.header", Role::Micro, s.t.text_dim)
        .align(Align2::LEFT_CENTER)
        .policy(Policy::Elide(Elide::End));
    let _ = text::paint(b, r.shrink2(egui::vec2(10.0 * s.k, 0.0)), &tx, s.tk());
}

/// The lanes' pointer: a press on a note selects it and drags its pitch, a double-click
/// pulls its centre onto its note.
pub fn input(app: &mut CantusApp, ui: &egui::Ui, geo: &TimelineGeometry, lanes: Rect, _s: &Style) {
    let laid = layout(app, geo, lanes);
    let pointer = ui.input(|i| i.pointer.hover_pos());
    let (pressed, down, alt) = ui.input(|i| {
        (
            i.pointer.primary_pressed(),
            i.pointer.primary_down(),
            i.modifiers.alt,
        )
    });
    for lane in &laid {
        let Some(track) = lane.track else {
            continue;
        };
        let resp = ui.interact(
            lane.rect,
            Id::new(("pitch-lane", track.0)),
            Sense::click_and_drag(),
        );
        let hit = |p: Pos2| {
            lane.notes
                .iter()
                .rev()
                .find(|n| n.rect.expand(3.0).contains(p))
                .cloned()
        };
        if resp.hovered() {
            let txt = match pointer.and_then(hit) {
                Some(n) => app
                    .project()
                    .track(n.track)
                    .and_then(|_| {
                        let take = app.pitch_take(n.track, n.clip);
                        app.pitch_edit(n.track, n.clip, &take)
                    })
                    .and_then(|e| {
                        e.note(n.id)
                            .map(|x| super::pitch::note_words(x.out_centre()))
                    })
                    .unwrap_or_default(),
                None => String::new(),
            };
            hint::set_text(
                ui.ctx(),
                &crate::i18n::t("pitch.lane.tip"),
                &txt,
                Caps::NONE,
            );
            if pointer.and_then(hit).is_some() {
                ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeVertical);
            }
        }
        let press = ui.input(|i| i.pointer.press_origin());
        if pressed && press.is_some_and(|p| lane.rect.contains(p)) {
            if let Some(n) = press.and_then(hit) {
                let take = app.pitch_take(n.track, n.clip);
                if let Some(e) = app.pitch_edit(n.track, n.clip, &take) {
                    app.pitch.lane.selected = Some((n.track, n.clip, n.id));
                    app.pitch.lane.drag = Some(LaneDrag {
                        track: n.track,
                        clip: n.clip,
                        note: n.id,
                        at_press: e,
                        from: press.expect("checked"),
                        px_per_st: n.px_per_st,
                    });
                }
            }
        }
        if resp.double_clicked() {
            if let Some(n) = resp.interact_pointer_pos().and_then(hit) {
                let take = app.pitch_take(n.track, n.clip);
                if let Some(mut e) = app.pitch_edit(n.track, n.clip, &take) {
                    let on = e.note(n.id).is_some_and(|x| x.centre == 1.0);
                    edit::set_field(
                        &mut e,
                        &[n.id],
                        edit::Field::Centre,
                        if on { 0.0 } else { 1.0 },
                    );
                    app.commit_pitch(n.track, n.clip, Some(e), PitchOp::Centre, false);
                }
            }
        }
    }
    if let (true, Some(p), Some(d)) = (down, pointer, app.pitch.lane.drag.clone()) {
        let delta = (d.from.y - p.y) / d.px_per_st.max(1e-3);
        if (d.from - p).length() > 3.0 {
            let mut e = d.at_press.clone();
            if let Some(n) = d.at_press.note(d.note) {
                let to = match (alt, app.pitch.view.snap) {
                    (true, _) | (_, super::pitch::Snap::Free) => {
                        n.pitch + (delta * 100.0).round() / 100.0
                    }
                    (_, super::pitch::Snap::Semitone) => n.pitch + delta.round(),
                    (_, super::pitch::Snap::Scale) => {
                        let key = d.at_press.key;
                        let steps = key.steps_to(n.pitch, key.nearest(n.pitch + delta));
                        let mut q = n.pitch;
                        for _ in 0..steps.abs() {
                            q = key.step(q, steps.signum());
                        }
                        q
                    }
                };
                edit::set_pitch(&mut e, d.note, to);
            }
            let now = app.stored_pitch(d.track, d.clip).cloned();
            if now.as_ref() != Some(&e) {
                app.commit_pitch(d.track, d.clip, Some(e), PitchOp::Pitch, true);
            }
        }
    }
    if !down && app.pitch.lane.drag.take().is_some() {
        app.end_gesture();
    }
}

/// The track's menu entry: *Pitch lane* (for a track with audio clips). Returns whether it
/// was chosen.
pub fn track_menu(app: &mut CantusApp, ui: &mut egui::Ui, id: TrackId) -> bool {
    let Some(t) = app.project().track(id) else {
        return false;
    };
    if !t.clips.iter().any(|c| c.audio.is_some()) {
        return false;
    }
    let open = t.pitch_open;
    let items = [
        Item::Band(Src::Key("pitch.lane.band")),
        Item::Action {
            label: Src::Key("pitch.lane.menu"),
            check: if open { Check::On } else { Check::Off },
            shortcut: None,
            submenu: false,
            enabled: true,
        },
    ];
    if kit::widgets::menu_list(ui, &items, 220.0).1.clicked == Some(1) {
        app.toggle_pitch_lane(id);
        return true;
    }
    false
}

impl CantusApp {
    /// Show or hide a track's pitch lane (one undo step).
    pub fn toggle_pitch_lane(&mut self, id: TrackId) {
        if let Some(open) = self.project().track(id).map(|t| t.pitch_open) {
            self.exec(Command::SetTrackPitchOpen { id, open: !open });
        }
    }
}
