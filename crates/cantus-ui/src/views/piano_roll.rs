//! Piano roll: the dense view. Everything is painted with primitives on the visible range
//! only (ADR 0003 guardrail), never one widget per note.

use super::{beats_to_ticks, ticks_to_beats, GRIDS};
use crate::app::CantusApp;
use crate::i18n::{t, tf};
use crate::theme;
use cantus_core::{pitch_name, ClipId, Command, MergeKey, Note, NoteId, Ticks, TrackId};
use egui::{
    pos2, vec2, Align2, Color32, CornerRadius, FontId, PointerButton, Pos2, Rect, Sense, Stroke,
    StrokeKind,
};

const KEYS_W: f32 = 56.0;
const RULER_H: f32 = 20.0;
const VEL_H: f32 = 70.0;
const RESIZE_GRIP: f32 = 7.0;

#[derive(Debug)]
pub struct PianoRollView {
    pub px_per_beat: f32,
    pub scroll_beats: f32,
    pub key_height: f32,
    /// Pitch shown at the top edge (fractional for smooth scrolling).
    pub top_pitch: f32,
    pub grid: Ticks,
    pub snap: bool,
    pub last_note_length: Ticks,
    drag: Option<NoteDrag>,
    marquee: Option<(Pos2, Pos2)>,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum DragKind {
    Move,
    Resize,
    Velocity,
}

#[derive(Debug)]
struct NoteDrag {
    kind: DragKind,
    originals: Vec<Note>,
    press: Pos2,
    track: TrackId,
    clip: ClipId,
}

impl Default for PianoRollView {
    fn default() -> Self {
        PianoRollView {
            px_per_beat: 48.0,
            scroll_beats: 0.0,
            key_height: 14.0,
            top_pitch: 84.0,
            grid: Ticks::SIXTEENTH,
            snap: true,
            last_note_length: Ticks::SIXTEENTH,
            drag: None,
            marquee: None,
        }
    }
}

struct Geo {
    grid: Rect,
    px_per_beat: f32,
    scroll_beats: f32,
    key_h: f32,
    top_pitch: f32,
}

impl Geo {
    fn x(&self, beats: f32) -> f32 {
        self.grid.left() + (beats - self.scroll_beats) * self.px_per_beat
    }
    fn beats(&self, x: f32) -> f32 {
        (x - self.grid.left()) / self.px_per_beat + self.scroll_beats
    }
    fn y(&self, pitch: f32) -> f32 {
        self.grid.top() + (self.top_pitch - pitch) * self.key_h
    }
    fn pitch(&self, y: f32) -> f32 {
        self.top_pitch - (y - self.grid.top()) / self.key_h
    }
    fn note_rect(&self, n: &Note) -> Rect {
        let x0 = self.x(ticks_to_beats(n.start));
        let x1 = self.x(ticks_to_beats(n.end()));
        let y0 = self.y(n.pitch as f32 + 1.0);
        Rect::from_min_max(
            pos2(x0, y0 + 1.0),
            pos2(x1.max(x0 + 2.0), y0 + self.key_h - 1.0),
        )
    }
}

fn is_black(pitch: u8) -> bool {
    matches!(pitch % 12, 1 | 3 | 6 | 8 | 10)
}

pub fn show(app: &mut CantusApp, ui: &mut egui::Ui) {
    let Some((track_id, clip_id)) = app.selected_track.zip(app.selected_clip) else {
        ui.centered_and_justified(|ui| {
            ui.label(egui::RichText::new(t("clip.none_selected")).color(theme::TEXT_DIM));
        });
        return;
    };
    let Some(clip) = app
        .project()
        .track(track_id)
        .and_then(|t| t.clip(clip_id))
        .cloned()
    else {
        app.selected_clip = None;
        return;
    };
    let track_color = app
        .project()
        .track(track_id)
        .map(|t| theme::track_color(t.color))
        .unwrap_or(theme::GOLD);

    toolbar(app, ui, &clip);

    let full = ui.available_rect_before_wrap();
    let grid = Rect::from_min_max(
        pos2(full.left() + KEYS_W, full.top() + RULER_H),
        pos2(full.right(), full.bottom() - VEL_H),
    );
    let keys = Rect::from_min_max(
        pos2(full.left(), grid.top()),
        pos2(grid.left(), grid.bottom()),
    );
    let ruler = Rect::from_min_max(
        pos2(grid.left(), full.top()),
        pos2(full.right(), grid.top()),
    );
    let vel = Rect::from_min_max(pos2(grid.left(), grid.bottom()), full.max);

    let resp = ui.allocate_rect(grid, Sense::click_and_drag());
    let vel_resp = ui.allocate_rect(vel, Sense::click_and_drag());
    let ruler_resp = ui.allocate_rect(ruler, Sense::click());

    // Zoom & scroll.
    if resp.hovered() || vel_resp.hovered() || ruler_resp.hovered() {
        let (zoom, scroll, shift) =
            ui.input(|i| (i.zoom_delta(), i.smooth_scroll_delta, i.modifiers.shift));
        let v = &mut app.piano_roll;
        if zoom != 1.0 {
            let hover_x = resp
                .hover_pos()
                .or(vel_resp.hover_pos())
                .map(|p| p.x)
                .unwrap_or(grid.left());
            let anchor = (hover_x - grid.left()) / v.px_per_beat + v.scroll_beats;
            v.px_per_beat = (v.px_per_beat * zoom).clamp(4.0, 800.0);
            v.scroll_beats = (anchor - (hover_x - grid.left()) / v.px_per_beat).max(0.0);
        } else if scroll != egui::Vec2::ZERO {
            if shift || scroll.x != 0.0 {
                let dx = if shift { scroll.y } else { scroll.x };
                v.scroll_beats = (v.scroll_beats - dx / v.px_per_beat).max(0.0);
            }
            if !shift && scroll.y != 0.0 {
                v.top_pitch = (v.top_pitch + scroll.y / v.key_height).clamp(12.0, 128.0);
            }
        }
    }
    let v = &app.piano_roll;
    let geo = Geo {
        grid,
        px_per_beat: v.px_per_beat,
        scroll_beats: v.scroll_beats,
        key_h: v.key_height,
        top_pitch: v.top_pitch,
    };

    paint_grid(app, ui, &geo, &clip);
    paint_keys(ui, &geo, keys);
    paint_ruler(app, ui, &geo, ruler, &clip);

    // Notes.
    let p = ui.painter().with_clip_rect(grid);
    let visible_start = beats_to_ticks(geo.beats(grid.left())) - Ticks::beats(64);
    let visible_end = beats_to_ticks(geo.beats(grid.right()));
    let lo_pitch = geo.pitch(grid.bottom()).floor() as i32 - 1;
    let hi_pitch = geo.pitch(grid.top()).ceil() as i32 + 1;
    let mut hit: Option<(NoteId, bool)> = None;
    let hover = resp.hover_pos();
    let mut drawn = 0usize;
    for n in &clip.notes {
        if n.start > visible_end {
            break; // notes are sorted by start
        }
        if n.end() < visible_start || (n.pitch as i32) < lo_pitch || (n.pitch as i32) > hi_pitch {
            continue;
        }
        let r = geo.note_rect(n);
        let selected = app.selected_notes.contains(&n.id);
        let alpha = 0.55 + 0.45 * (n.velocity as f32 / 127.0);
        let fill = if selected {
            theme::GOLD_BRIGHT
        } else {
            track_color
        }
        .gamma_multiply(alpha);
        p.rect_filled(r, CornerRadius::same(2), fill);
        p.rect_stroke(
            r,
            CornerRadius::same(2),
            Stroke::new(1.0, if selected { Color32::WHITE } else { theme::INK }),
            StrokeKind::Inside,
        );
        if r.width() > 28.0 && geo.key_h >= 11.0 {
            p.with_clip_rect(r).text(
                pos2(r.left() + 3.0, r.center().y),
                Align2::LEFT_CENTER,
                pitch_name(n.pitch),
                FontId::proportional(9.0),
                theme::INK,
            );
        }
        if let Some(pos) = hover {
            if r.contains(pos) {
                hit = Some((n.id, pos.x > r.right() - RESIZE_GRIP));
            }
        }
        drawn += 1;
    }
    app.piano_roll_notes_drawn = drawn;

    // Marquee.
    if let Some((a, b)) = app.piano_roll.marquee {
        let r = Rect::from_two_pos(a, b);
        p.rect_filled(r, 0.0, theme::SELECTION.gamma_multiply(0.15));
        p.rect_stroke(
            r,
            0.0,
            Stroke::new(1.0, theme::SELECTION),
            StrokeKind::Inside,
        );
    }

    // Playhead (clip-relative).
    let ph = app.engine.position() - clip.start;
    let ph_x = geo.x(ticks_to_beats(ph));
    if ph_x >= grid.left() && ph_x <= grid.right() {
        p.vline(ph_x, grid.y_range(), Stroke::new(1.5, theme::GOLD_BRIGHT));
    }

    velocity_lane(
        app,
        ui,
        &geo,
        vel,
        &vel_resp,
        &clip,
        track_color,
        track_id,
        clip_id,
    );
    interact(
        app,
        ui,
        &geo,
        &resp,
        &ruler_resp,
        &clip,
        hit,
        track_id,
        clip_id,
    );

    if hover.is_some() {
        ui.ctx()
            .set_cursor_icon(match (hit, app.piano_roll.drag.as_ref().map(|d| d.kind)) {
                (_, Some(DragKind::Resize)) | (Some((_, true)), None) => {
                    egui::CursorIcon::ResizeHorizontal
                }
                (Some(_), _) => egui::CursorIcon::Grab,
                _ => egui::CursorIcon::Default,
            });
    }
}

fn toolbar(app: &mut CantusApp, ui: &mut egui::Ui, clip: &cantus_core::Clip) {
    ui.horizontal(|ui| {
        ui.label(
            egui::RichText::new(t("pianoroll.title"))
                .strong()
                .color(theme::GOLD),
        );
        ui.label(egui::RichText::new(&clip.name).color(theme::TEXT_DIM));
        ui.separator();
        ui.label(t("pianoroll.grid"));
        let current = GRIDS
            .iter()
            .find(|(_, g)| *g == app.piano_roll.grid.0)
            .map(|(k, _)| t(k))
            .unwrap_or_default();
        egui::ComboBox::from_id_salt("grid")
            .selected_text(current)
            .width(70.0)
            .show_ui(ui, |ui| {
                for (key, g) in GRIDS {
                    if ui
                        .selectable_label(app.piano_roll.grid.0 == g, t(key))
                        .clicked()
                    {
                        app.piano_roll.grid = Ticks(g);
                    }
                }
            });
        ui.toggle_value(&mut app.piano_roll.snap, t("pianoroll.snap"));
        if ui.button(t("menu.quantize")).clicked() {
            app.quantize_selection();
        }
        ui.separator();
        ui.label(
            egui::RichText::new(tf("pianoroll.notes", &[("n", &clip.notes.len())]))
                .color(theme::TEXT_DIM),
        );
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.label(
                egui::RichText::new(t("pianoroll.hint"))
                    .small()
                    .color(theme::TEXT_DIM),
            );
        });
    });
}

fn paint_grid(app: &CantusApp, ui: &egui::Ui, geo: &Geo, clip: &cantus_core::Clip) {
    let p = ui.painter().with_clip_rect(geo.grid);
    let grid = geo.grid;
    p.rect_filled(grid, 0.0, theme::INK);
    // Pitch rows.
    let lo = geo.pitch(grid.bottom()).floor().max(0.0) as i32;
    let hi = geo.pitch(grid.top()).ceil().min(128.0) as i32;
    for pitch in lo..hi {
        let y1 = geo.y(pitch as f32);
        let y0 = y1 - geo.key_h;
        let r = Rect::from_min_max(pos2(grid.left(), y0), pos2(grid.right(), y1));
        p.rect_filled(
            r,
            0.0,
            if is_black(pitch as u8) {
                theme::ROW_BLACK
            } else {
                theme::ROW_WHITE
            },
        );
        if pitch % 12 == 0 {
            p.hline(grid.x_range(), y1, Stroke::new(1.0, theme::GRID_STRONG));
        }
    }
    // Clip bounds shading.
    let clip_end_x = geo.x(ticks_to_beats(clip.length));
    if clip_end_x < grid.right() {
        p.rect_filled(
            Rect::from_min_max(pos2(clip_end_x, grid.top()), grid.max),
            0.0,
            Color32::from_black_alpha(90),
        );
    }
    // Vertical grid lines at the snap grid; bars and beats stronger.
    let ts = app.project().time_signature;
    let bar_ticks = ts.ticks_per_bar();
    let beat_ticks = ts.ticks_per_beat();
    let step = app.piano_roll.grid;
    let min_px = 5.0;
    let mut step_ticks = step;
    while ticks_to_beats(step_ticks) * geo.px_per_beat < min_px {
        step_ticks = step_ticks * 2;
    }
    let first = beats_to_ticks(geo.beats(grid.left()).max(0.0)).snap_floor(step_ticks);
    let last = beats_to_ticks(geo.beats(grid.right()));
    let mut tick = first;
    while tick <= last {
        let x = geo.x(ticks_to_beats(tick));
        let color = if tick.0 % bar_ticks.0 == 0 {
            theme::GRID_BAR
        } else if tick.0 % beat_ticks.0 == 0 {
            theme::GRID_STRONG
        } else {
            theme::GRID
        };
        p.vline(x, grid.y_range(), Stroke::new(1.0, color));
        tick += step_ticks;
    }
}

fn paint_keys(ui: &egui::Ui, geo: &Geo, keys: Rect) {
    let p = ui.painter().with_clip_rect(keys);
    p.rect_filled(keys, 0.0, theme::PANEL);
    let lo = geo.pitch(keys.bottom()).floor().max(0.0) as i32;
    let hi = geo.pitch(keys.top()).ceil().min(128.0) as i32;
    for pitch in lo..hi {
        let y1 = geo.y(pitch as f32);
        let y0 = y1 - geo.key_h;
        let black = is_black(pitch as u8);
        let r = Rect::from_min_max(
            pos2(keys.left(), y0),
            pos2(
                if black {
                    keys.right() - 18.0
                } else {
                    keys.right()
                },
                y1,
            ),
        );
        p.rect_filled(
            r.shrink2(vec2(0.0, 0.5)),
            0.0,
            if black {
                theme::BLACK_KEY
            } else {
                theme::WHITE_KEY
            },
        );
        if pitch % 12 == 0 && geo.key_h >= 9.0 {
            p.text(
                pos2(keys.right() - 3.0, r.center().y),
                Align2::RIGHT_CENTER,
                pitch_name(pitch as u8),
                FontId::proportional(9.0),
                theme::INK,
            );
        }
    }
    p.vline(
        keys.right() - 0.5,
        keys.y_range(),
        Stroke::new(1.0, theme::GRID_STRONG),
    );
}

fn paint_ruler(app: &CantusApp, ui: &egui::Ui, geo: &Geo, ruler: Rect, clip: &cantus_core::Clip) {
    let p = ui.painter().with_clip_rect(ruler);
    p.rect_filled(ruler, 0.0, theme::PANEL);
    let ts = app.project().time_signature;
    let bar = ts.ticks_per_bar();
    let first = (beats_to_ticks(geo.beats(ruler.left()).max(0.0)) + clip.start).snap_floor(bar)
        - clip.start;
    let last = beats_to_ticks(geo.beats(ruler.right()));
    let mut tick = first;
    while tick <= last {
        let x = geo.x(ticks_to_beats(tick));
        let (b, _, _) = ts.bbt(tick + clip.start);
        p.vline(x, ruler.y_range(), Stroke::new(1.0, theme::GRID_STRONG));
        p.text(
            pos2(x + 3.0, ruler.center().y),
            Align2::LEFT_CENTER,
            b.to_string(),
            FontId::proportional(10.0),
            theme::TEXT_DIM,
        );
        tick += bar;
    }
    p.hline(
        ruler.x_range(),
        ruler.bottom() - 0.5,
        Stroke::new(1.0, theme::GRID_STRONG),
    );
}

#[allow(clippy::too_many_arguments)]
fn velocity_lane(
    app: &mut CantusApp,
    ui: &egui::Ui,
    geo: &Geo,
    vel: Rect,
    resp: &egui::Response,
    clip: &cantus_core::Clip,
    color: Color32,
    track: TrackId,
    clip_id: ClipId,
) {
    let p = ui.painter().with_clip_rect(vel);
    p.rect_filled(vel, 0.0, theme::PANEL);
    p.hline(
        vel.x_range(),
        vel.top() + 0.5,
        Stroke::new(1.0, theme::GRID_STRONG),
    );
    p.text(
        pos2(vel.left() + 4.0, vel.top() + 3.0),
        Align2::LEFT_TOP,
        t("pianoroll.velocity"),
        FontId::proportional(9.0),
        theme::TEXT_DIM,
    );
    let inner_top = vel.top() + 6.0;
    let inner_bottom = vel.bottom() - 4.0;
    let hover = resp.hover_pos();
    let mut hit = None;
    for n in &clip.notes {
        let x = geo.x(ticks_to_beats(n.start));
        if x < vel.left() - 4.0 || x > vel.right() {
            continue;
        }
        let h = (inner_bottom - inner_top) * n.velocity as f32 / 127.0;
        let r = Rect::from_min_max(pos2(x, inner_bottom - h), pos2(x + 4.0, inner_bottom));
        let selected = app.selected_notes.contains(&n.id);
        p.rect_filled(r, 0.0, if selected { theme::GOLD_BRIGHT } else { color });
        if let Some(pos) = hover {
            if (pos.x - x).abs() < 5.0 {
                hit = Some(n.id);
            }
        }
    }
    if resp.drag_started_by(PointerButton::Primary) {
        if let (Some(id), Some(press)) = (hit, ui.input(|i| i.pointer.press_origin())) {
            if !app.selected_notes.contains(&id) {
                app.selected_notes = vec![id];
            }
            let originals: Vec<Note> = clip
                .notes
                .iter()
                .filter(|n| app.selected_notes.contains(&n.id))
                .cloned()
                .collect();
            app.piano_roll.drag = Some(NoteDrag {
                kind: DragKind::Velocity,
                originals,
                press,
                track,
                clip: clip_id,
            });
        }
    }
    if resp.dragged_by(PointerButton::Primary) {
        if let (Some(pos), Some(d)) = (resp.interact_pointer_pos(), app.piano_roll.drag.as_ref()) {
            if d.kind == DragKind::Velocity {
                let dv = -(pos.y - d.press.y) / (inner_bottom - inner_top) * 127.0;
                let notes: Vec<Note> = d
                    .originals
                    .iter()
                    .map(|n| Note {
                        velocity: (n.velocity as f32 + dv).round().clamp(1.0, 127.0) as u8,
                        ..n.clone()
                    })
                    .collect();
                let (track, clip_id) = (d.track, d.clip);
                app.exec_merging(
                    Command::SetNotes {
                        track,
                        clip: clip_id,
                        notes,
                    },
                    MergeKey::of(&("vel", clip_id.0)),
                );
            }
        }
    }
    if resp.drag_stopped()
        && app.piano_roll.drag.as_ref().map(|d| d.kind) == Some(DragKind::Velocity)
    {
        app.piano_roll.drag = None;
        app.end_gesture();
    }
}

#[allow(clippy::too_many_arguments)]
fn interact(
    app: &mut CantusApp,
    ui: &egui::Ui,
    geo: &Geo,
    resp: &egui::Response,
    ruler_resp: &egui::Response,
    clip: &cantus_core::Clip,
    hit: Option<(NoteId, bool)>,
    track: TrackId,
    clip_id: ClipId,
) {
    let ctrl = ui.input(|i| i.modifiers.command);
    let snap_grid = app.piano_roll.grid;
    let snap = |t: Ticks, on: bool| if on { t.snap_floor(snap_grid) } else { t };

    if ruler_resp.clicked() {
        if let Some(pos) = ruler_resp.interact_pointer_pos() {
            let tick =
                beats_to_ticks(geo.beats(pos.x).max(0.0)).snap_floor(Ticks::QUARTER) + clip.start;
            app.seek(tick);
        }
    }

    if resp.clicked() {
        if let Some(pos) = resp.interact_pointer_pos() {
            match hit {
                Some((id, _)) => {
                    if ctrl {
                        if let Some(i) = app.selected_notes.iter().position(|n| *n == id) {
                            app.selected_notes.remove(i);
                        } else {
                            app.selected_notes.push(id);
                        }
                    } else {
                        app.selected_notes = vec![id];
                    }
                }
                None => {
                    let start = snap(beats_to_ticks(geo.beats(pos.x)), app.piano_roll.snap)
                        .max(Ticks::ZERO);
                    let pitch = geo.pitch(pos.y).floor().clamp(0.0, 127.0) as u8;
                    let length = if app.piano_roll.snap {
                        app.piano_roll.last_note_length.max(snap_grid)
                    } else {
                        app.piano_roll.last_note_length
                    };
                    let note = app.bus.new_note(start, length, pitch, 100);
                    let id = note.id;
                    if app.exec(Command::InsertNotes {
                        track,
                        clip: clip_id,
                        notes: vec![note],
                    }) {
                        app.selected_notes = vec![id];
                        app.audition(track, pitch);
                    }
                }
            }
        }
    }
    if resp.secondary_clicked() {
        if let Some((id, _)) = hit {
            let ids = if app.selected_notes.contains(&id) {
                app.selected_notes.clone()
            } else {
                vec![id]
            };
            app.exec(Command::RemoveNotes {
                track,
                clip: clip_id,
                ids,
            });
            app.selected_notes.clear();
        }
    }
    if resp.drag_started_by(PointerButton::Primary) {
        let press = ui
            .input(|i| i.pointer.press_origin())
            .unwrap_or(resp.interact_pointer_pos().unwrap_or_default());
        match hit {
            Some((id, resize)) => {
                if !app.selected_notes.contains(&id) {
                    app.selected_notes = vec![id];
                }
                let originals: Vec<Note> = clip
                    .notes
                    .iter()
                    .filter(|n| app.selected_notes.contains(&n.id))
                    .cloned()
                    .collect();
                app.piano_roll.drag = Some(NoteDrag {
                    kind: if resize {
                        DragKind::Resize
                    } else {
                        DragKind::Move
                    },
                    originals,
                    press,
                    track,
                    clip: clip_id,
                });
            }
            None => {
                app.piano_roll.marquee = Some((press, press));
                if !ctrl {
                    app.selected_notes.clear();
                }
            }
        }
    }
    if resp.dragged_by(PointerButton::Primary) {
        if let Some(pos) = resp.interact_pointer_pos() {
            if let Some(d) = app.piano_roll.drag.as_ref() {
                let d_beats = (pos.x - d.press.x) / geo.px_per_beat;
                let raw = beats_to_ticks(d_beats);
                let d_pitch = -((pos.y - d.press.y) / geo.key_h).round() as i32;
                let notes: Vec<Note> = match d.kind {
                    DragKind::Move => {
                        let dt = if app.piano_roll.snap {
                            raw.snap(snap_grid)
                        } else {
                            raw
                        };
                        d.originals
                            .iter()
                            .map(|n| Note {
                                start: (n.start + dt).max(Ticks::ZERO),
                                pitch: (n.pitch as i32 + d_pitch).clamp(0, 127) as u8,
                                ..n.clone()
                            })
                            .collect()
                    }
                    DragKind::Resize => d
                        .originals
                        .iter()
                        .map(|n| {
                            let end = if app.piano_roll.snap {
                                (n.end() + raw).snap(snap_grid)
                            } else {
                                n.end() + raw
                            };
                            let min_len = if app.piano_roll.snap {
                                snap_grid
                            } else {
                                Ticks(10)
                            };
                            Note {
                                length: (end - n.start).max(min_len),
                                ..n.clone()
                            }
                        })
                        .collect(),
                    DragKind::Velocity => return,
                };
                if let Some(first) = notes.first() {
                    if d.kind == DragKind::Resize {
                        app.piano_roll.last_note_length = first.length;
                    }
                }
                let (track, clip_id) = (d.track, d.clip);
                app.exec_merging(
                    Command::SetNotes {
                        track,
                        clip: clip_id,
                        notes,
                    },
                    MergeKey::of(&("notes", clip_id.0)),
                );
            } else if let Some((a, _)) = app.piano_roll.marquee {
                app.piano_roll.marquee = Some((a, pos));
                let r = Rect::from_two_pos(a, pos);
                let mut sel: Vec<NoteId> = if ctrl {
                    app.selected_notes.clone()
                } else {
                    Vec::new()
                };
                for n in &clip.notes {
                    if r.intersects(geo.note_rect(n)) && !sel.contains(&n.id) {
                        sel.push(n.id);
                    }
                }
                app.selected_notes = sel;
            }
        }
    }
    if resp.drag_stopped() {
        if let Some(d) = app.piano_roll.drag.take() {
            if d.kind == DragKind::Move {
                if let Some(first) = d.originals.first() {
                    if let Some(n) = clip.note(first.id) {
                        app.audition(track, n.pitch);
                    }
                }
            }
            app.end_gesture();
        }
        app.piano_roll.marquee = None;
    }
}
