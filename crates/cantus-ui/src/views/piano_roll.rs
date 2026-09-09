//! Piano roll: the dense view. Everything is painted with primitives on the visible range
//! only (ADR 0003 guardrail), never one widget per note.
//!
//! Mouse tools follow FL Studio's toolbar and letter shortcuts (P B E D T C Y); the number
//! keys 1–7 work too, and always, even while the typing keyboard plays notes. Hit testing always uses the *press origin*, never the current
//! pointer position, so a drag that leaves a note's rectangle still acts on that note.

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
const RESIZE_GRIP: f32 = 8.0;

/// Mouse tools. FL letters (when the typing piano is off) or number keys 1–7.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Tool {
    Draw,
    Paint,
    Select,
    Delete,
    Mute,
    Slice,
    Playback,
}

impl Tool {
    pub const ALL: [Tool; 7] = [
        Tool::Draw,
        Tool::Paint,
        Tool::Select,
        Tool::Delete,
        Tool::Mute,
        Tool::Slice,
        Tool::Playback,
    ];

    pub fn label_key(self) -> &'static str {
        match self {
            Tool::Draw => "tool.draw",
            Tool::Paint => "tool.paint",
            Tool::Select => "tool.select",
            Tool::Delete => "tool.delete",
            Tool::Mute => "tool.mute",
            Tool::Slice => "tool.slice",
            Tool::Playback => "tool.playback",
        }
    }

    pub fn hint_key(self) -> &'static str {
        match self {
            Tool::Draw => "tool.draw.hint",
            Tool::Paint => "tool.paint.hint",
            Tool::Select => "tool.select.hint",
            Tool::Delete => "tool.delete.hint",
            Tool::Mute => "tool.mute.hint",
            Tool::Slice => "tool.slice.hint",
            Tool::Playback => "tool.playback.hint",
        }
    }

    pub fn icon(self) -> crate::icons::Icon {
        use crate::icons::Icon;
        match self {
            Tool::Draw => Icon::Draw,
            Tool::Paint => Icon::Paint,
            Tool::Select => Icon::Select,
            Tool::Delete => Icon::Delete,
            Tool::Mute => Icon::Mute,
            Tool::Slice => Icon::Slice,
            Tool::Playback => Icon::Playback,
        }
    }

    /// Number-key shortcut (works even while the typing piano is on).
    pub fn number(self) -> u8 {
        Tool::ALL.iter().position(|t| *t == self).unwrap_or(0) as u8 + 1
    }

    /// FL Studio letter shortcut.
    pub fn shortcut(self) -> char {
        match self {
            Tool::Draw => 'P',
            Tool::Paint => 'B',
            Tool::Select => 'E',
            Tool::Delete => 'D',
            Tool::Mute => 'T',
            Tool::Slice => 'C',
            Tool::Playback => 'Y',
        }
    }

    pub fn key(self) -> egui::Key {
        match self {
            Tool::Draw => egui::Key::P,
            Tool::Paint => egui::Key::B,
            Tool::Select => egui::Key::E,
            Tool::Delete => egui::Key::D,
            Tool::Mute => egui::Key::T,
            Tool::Slice => egui::Key::C,
            Tool::Playback => egui::Key::Y,
        }
    }
}

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
    pub tool: Tool,
    drag: Option<NoteDrag>,
    marquee: Option<(Pos2, Pos2)>,
    /// Slice tool: x position of the cut line while dragging.
    slice_x: Option<f32>,
    /// Paint tool: last cell painted in this stroke.
    paint_last: Option<(Ticks, u8)>,
    /// Gesture counter so each paint/erase stroke merges into its own undo step.
    stroke: u64,
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
            tool: Tool::Draw,
            drag: None,
            marquee: None,
            slice_x: None,
            paint_last: None,
            stroke: 0,
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
    fn pitch_at(&self, y: f32) -> u8 {
        self.pitch(y).floor().clamp(0.0, 127.0) as u8
    }
    fn tick_at(&self, x: f32) -> Ticks {
        beats_to_ticks(self.beats(x))
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
    /// Note under `pos` and whether the pointer is on its resize grip.
    fn hit(&self, notes: &[Note], pos: Pos2) -> Option<(NoteId, bool)> {
        // Later notes are drawn on top, so search from the end.
        notes.iter().rev().find_map(|n| {
            let r = self.note_rect(n);
            if r.contains(pos) {
                Some((n.id, pos.x > r.right() - RESIZE_GRIP.min(r.width() * 0.5)))
            } else {
                None
            }
        })
    }
}

fn is_black(pitch: u8) -> bool {
    matches!(pitch % 12, 1 | 3 | 6 | 8 | 10)
}

pub fn show(app: &mut CantusApp, ui: &mut egui::Ui) {
    let Some((track_id, clip_id)) = app.selected_track.zip(app.selected_clip) else {
        ui.centered_and_justified(|ui| {
            ui.label(egui::RichText::new(t("clip.none_selected")).color(theme::p().text_dim));
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
        .unwrap_or(theme::p().gold);

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
    let keys_resp = ui.allocate_rect(keys, Sense::click());

    let (zoom, scroll, mods) = ui.input(|i| (i.zoom_delta(), i.smooth_scroll_delta, i.modifiers));

    // Zoom & scroll (alt+wheel is velocity, handled below).
    if (resp.hovered() || vel_resp.hovered() || ruler_resp.hovered()) && !mods.alt {
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
            if mods.shift || scroll.x != 0.0 {
                let dx = if mods.shift { scroll.y } else { scroll.x };
                v.scroll_beats = (v.scroll_beats - dx / v.px_per_beat).max(0.0);
            }
            if !mods.shift && scroll.y != 0.0 {
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
        let base = if selected {
            theme::p().gold_bright
        } else {
            track_color
        };
        if n.muted {
            p.rect_filled(r, CornerRadius::same(2), theme::p().panel_raised);
            p.rect_stroke(
                r,
                CornerRadius::same(2),
                Stroke::new(1.0, base.gamma_multiply(0.7)),
                StrokeKind::Inside,
            );
        } else {
            p.rect_filled(r, CornerRadius::same(2), base.gamma_multiply(alpha));
            p.rect_stroke(
                r,
                CornerRadius::same(2),
                Stroke::new(
                    1.0,
                    if selected {
                        Color32::WHITE
                    } else {
                        theme::p().ink
                    },
                ),
                StrokeKind::Inside,
            );
        }
        if r.width() > 28.0 && geo.key_h >= 11.0 {
            p.with_clip_rect(r).text(
                pos2(r.left() + 3.0, r.center().y),
                Align2::LEFT_CENTER,
                pitch_name(n.pitch),
                FontId::proportional(9.0),
                if n.muted {
                    theme::p().text_dim
                } else {
                    theme::p().ink
                },
            );
        }
        drawn += 1;
    }
    app.piano_roll_notes_drawn = drawn;

    // Notes being recorded: from their start to the playhead.
    if app.recorder.is_recording() {
        let ph_beats = ticks_to_beats(app.engine.position() - clip.start);
        for pending in app.recorder.pending() {
            let x0 = geo.x(ticks_to_beats(pending.start - clip.start));
            let x1 = geo.x(ph_beats).max(x0 + 2.0);
            let y0 = geo.y(pending.pitch as f32 + 1.0);
            let r = Rect::from_min_max(pos2(x0, y0 + 1.0), pos2(x1, y0 + geo.key_h - 1.0));
            p.rect_filled(
                r,
                CornerRadius::same(2),
                theme::p().danger.gamma_multiply(0.8),
            );
        }
    }

    // Marquee.
    if let Some((a, b)) = app.piano_roll.marquee {
        let r = Rect::from_two_pos(a, b);
        p.rect_filled(r, 0.0, theme::p().selection.gamma_multiply(0.15));
        p.rect_stroke(
            r,
            0.0,
            Stroke::new(1.0, theme::p().selection),
            StrokeKind::Inside,
        );
    }
    // Slice line.
    if let Some(x) = app.piano_roll.slice_x {
        p.vline(x, grid.y_range(), Stroke::new(1.5, theme::p().danger));
    }

    // Playhead (clip-relative).
    let ph = app.engine.position() - clip.start;
    let ph_x = geo.x(ticks_to_beats(ph));
    if ph_x >= grid.left() && ph_x <= grid.right() {
        p.vline(
            ph_x,
            grid.y_range(),
            Stroke::new(1.5, theme::p().gold_bright),
        );
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
    if keys_resp.clicked() {
        if let Some(pos) = keys_resp.interact_pointer_pos() {
            app.audition(track_id, geo.pitch_at(pos.y));
        }
    }
    interact(app, ui, &geo, &resp, &ruler_resp, &clip, track_id, clip_id);

    // Cursor.
    if let Some(pos) = resp.hover_pos() {
        let hit = geo.hit(&clip.notes, pos);
        let dragging = app.piano_roll.drag.as_ref().map(|d| d.kind);
        let icon = match (app.piano_roll.tool, hit, dragging) {
            (_, _, Some(DragKind::Resize)) => egui::CursorIcon::ResizeHorizontal,
            (_, _, Some(DragKind::Move)) => egui::CursorIcon::Grabbing,
            (Tool::Draw | Tool::Select, Some((_, true)), None) => {
                egui::CursorIcon::ResizeHorizontal
            }
            (Tool::Draw | Tool::Select, Some(_), None) => egui::CursorIcon::Grab,
            (Tool::Delete, _, _) => egui::CursorIcon::NotAllowed,
            (Tool::Slice, _, _) => egui::CursorIcon::VerticalText,
            (Tool::Paint, _, _) | (Tool::Draw, None, _) => egui::CursorIcon::Crosshair,
            (Tool::Playback, _, _) | (Tool::Mute, _, _) => egui::CursorIcon::PointingHand,
            _ => egui::CursorIcon::Default,
        };
        ui.ctx().set_cursor_icon(icon);
    }
}

fn toolbar(app: &mut CantusApp, ui: &mut egui::Ui, clip: &cantus_core::Clip) {
    ui.horizontal(|ui| {
        ui.label(
            egui::RichText::new(t("pianoroll.title"))
                .strong()
                .color(theme::p().gold),
        );
        ui.label(egui::RichText::new(&clip.name).color(theme::p().text_dim));
        ui.separator();
        for tool in Tool::ALL {
            let selected = app.piano_roll.tool == tool;
            let tip = format!(
                "{} ({} / {})",
                t(tool.label_key()),
                tool.shortcut(),
                tool.number()
            );
            if crate::icons::button(ui, tool.icon(), selected, tip).clicked() {
                app.piano_roll.tool = tool;
            }
        }
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
                .color(theme::p().text_dim),
        );
    });
}

fn paint_grid(app: &CantusApp, ui: &egui::Ui, geo: &Geo, clip: &cantus_core::Clip) {
    let p = ui.painter().with_clip_rect(geo.grid);
    let grid = geo.grid;
    p.rect_filled(grid, 0.0, theme::p().ink);
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
                theme::p().row_black
            } else {
                theme::p().row_white
            },
        );
        if pitch % 12 == 0 {
            p.hline(grid.x_range(), y1, Stroke::new(1.0, theme::p().grid_strong));
        }
    }
    let clip_end_x = geo.x(ticks_to_beats(clip.length));
    if clip_end_x < grid.right() {
        p.rect_filled(
            Rect::from_min_max(pos2(clip_end_x, grid.top()), grid.max),
            0.0,
            Color32::from_black_alpha(90),
        );
    }
    let ts = app.project().time_signature;
    let bar_ticks = ts.ticks_per_bar();
    let beat_ticks = ts.ticks_per_beat();
    let mut step_ticks = app.piano_roll.grid;
    while ticks_to_beats(step_ticks) * geo.px_per_beat < 5.0 {
        step_ticks = step_ticks * 2;
    }
    let first = beats_to_ticks(geo.beats(grid.left()).max(0.0)).snap_floor(step_ticks);
    let last = beats_to_ticks(geo.beats(grid.right()));
    let mut tick = first;
    while tick <= last {
        let x = geo.x(ticks_to_beats(tick));
        let color = if tick.0 % bar_ticks.0 == 0 {
            theme::p().grid_bar
        } else if tick.0 % beat_ticks.0 == 0 {
            theme::p().grid_strong
        } else {
            theme::p().grid
        };
        p.vline(x, grid.y_range(), Stroke::new(1.0, color));
        tick += step_ticks;
    }
}

fn paint_keys(ui: &egui::Ui, geo: &Geo, keys: Rect) {
    let p = ui.painter().with_clip_rect(keys);
    p.rect_filled(keys, 0.0, theme::p().panel);
    let lo = geo.pitch(keys.bottom()).floor().max(0.0) as i32;
    let hi = geo.pitch(keys.top()).ceil().min(128.0) as i32;
    for pitch in lo..hi {
        let y1 = geo.y(pitch as f32);
        let y0 = y1 - geo.key_h;
        let black = is_black(pitch as u8);
        let right = if black {
            keys.right() - 18.0
        } else {
            keys.right()
        };
        let r = Rect::from_min_max(pos2(keys.left(), y0), pos2(right, y1));
        p.rect_filled(
            r.shrink2(vec2(0.0, 0.5)),
            0.0,
            if black {
                theme::p().black_key
            } else {
                theme::p().white_key
            },
        );
        if pitch % 12 == 0 && geo.key_h >= 9.0 {
            p.text(
                pos2(keys.right() - 3.0, r.center().y),
                Align2::RIGHT_CENTER,
                pitch_name(pitch as u8),
                FontId::proportional(9.0),
                theme::p().ink,
            );
        }
    }
    p.vline(
        keys.right() - 0.5,
        keys.y_range(),
        Stroke::new(1.0, theme::p().grid_strong),
    );
}

fn paint_ruler(app: &CantusApp, ui: &egui::Ui, geo: &Geo, ruler: Rect, clip: &cantus_core::Clip) {
    let p = ui.painter().with_clip_rect(ruler);
    p.rect_filled(ruler, 0.0, theme::p().panel);
    let ts = app.project().time_signature;
    let bar = ts.ticks_per_bar();
    let first = (beats_to_ticks(geo.beats(ruler.left()).max(0.0)) + clip.start).snap_floor(bar)
        - clip.start;
    let last = beats_to_ticks(geo.beats(ruler.right()));
    let mut tick = first;
    while tick <= last {
        let x = geo.x(ticks_to_beats(tick));
        let (b, _, _) = ts.bbt(tick + clip.start);
        p.vline(x, ruler.y_range(), Stroke::new(1.0, theme::p().grid_strong));
        p.text(
            pos2(x + 3.0, ruler.center().y),
            Align2::LEFT_CENTER,
            b.to_string(),
            FontId::proportional(10.0),
            theme::p().text_dim,
        );
        tick += bar;
    }
    p.hline(
        ruler.x_range(),
        ruler.bottom() - 0.5,
        Stroke::new(1.0, theme::p().grid_strong),
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
    p.rect_filled(vel, 0.0, theme::p().panel);
    p.hline(
        vel.x_range(),
        vel.top() + 0.5,
        Stroke::new(1.0, theme::p().grid_strong),
    );
    p.text(
        pos2(vel.left() + 4.0, vel.top() + 3.0),
        Align2::LEFT_TOP,
        t("pianoroll.velocity"),
        FontId::proportional(9.0),
        theme::p().text_dim,
    );
    let inner_top = vel.top() + 6.0;
    let inner_bottom = vel.bottom() - 4.0;
    let press = ui.input(|i| i.pointer.press_origin());
    let hit_at = |x: f32| -> Option<NoteId> {
        clip.notes
            .iter()
            .rev()
            .find(|n| (geo.x(ticks_to_beats(n.start)) - x).abs() < 5.0)
            .map(|n| n.id)
    };
    for n in &clip.notes {
        let x = geo.x(ticks_to_beats(n.start));
        if x < vel.left() - 4.0 || x > vel.right() {
            continue;
        }
        let h = (inner_bottom - inner_top) * n.velocity as f32 / 127.0;
        let r = Rect::from_min_max(pos2(x, inner_bottom - h), pos2(x + 4.0, inner_bottom));
        let selected = app.selected_notes.contains(&n.id);
        p.rect_filled(
            r,
            0.0,
            if selected {
                theme::p().gold_bright
            } else if n.muted {
                theme::p().text_dim
            } else {
                color
            },
        );
    }
    if resp.drag_started_by(PointerButton::Primary) {
        if let Some(press) = press {
            if let Some(id) = hit_at(press.x) {
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
    track: TrackId,
    clip_id: ClipId,
) {
    let (mods, press, alt_wheel) = ui.input(|i| {
        (
            i.modifiers,
            i.pointer.press_origin(),
            if i.modifiers.alt {
                i.smooth_scroll_delta.y
            } else {
                0.0
            },
        )
    });
    let ctrl = mods.command;
    let grid_ticks = app.piano_roll.grid;
    let snapping = app.piano_roll.snap && !mods.alt;
    let snap_floor = |t: Ticks| {
        if snapping {
            t.snap_floor(grid_ticks)
        } else {
            t
        }
    };
    let snap_round = |t: Ticks| if snapping { t.snap(grid_ticks) } else { t };
    let tool = app.piano_roll.tool;
    let hover = resp.hover_pos();
    let hover_hit = hover.and_then(|p| geo.hit(&clip.notes, p));
    let press_hit = press.and_then(|p| geo.hit(&clip.notes, p));

    if ruler_resp.clicked() {
        if let Some(pos) = ruler_resp.interact_pointer_pos() {
            let tick = geo
                .tick_at(pos.x)
                .max(Ticks::ZERO)
                .snap_floor(Ticks::QUARTER)
                + clip.start;
            app.seek(tick);
        }
    }

    // Alt + wheel over a note: velocity.
    if alt_wheel != 0.0 && resp.hovered() {
        if let Some((id, _)) = hover_hit {
            let ids: Vec<NoteId> = if app.selected_notes.contains(&id) {
                app.selected_notes.clone()
            } else {
                vec![id]
            };
            let delta = (alt_wheel / 10.0).round().clamp(-16.0, 16.0) as i32;
            if delta != 0 {
                let notes: Vec<Note> = clip
                    .notes
                    .iter()
                    .filter(|n| ids.contains(&n.id))
                    .map(|n| Note {
                        velocity: (n.velocity as i32 + delta).clamp(1, 127) as u8,
                        ..n.clone()
                    })
                    .collect();
                app.exec_merging(
                    Command::SetNotes {
                        track,
                        clip: clip_id,
                        notes,
                    },
                    MergeKey::of(&("wheelvel", clip_id.0)),
                );
            }
        }
    }

    // ---- clicks -------------------------------------------------------------------------
    if resp.clicked() {
        if let Some(pos) = resp.interact_pointer_pos() {
            let hit = geo.hit(&clip.notes, pos);
            match tool {
                Tool::Draw | Tool::Paint => match hit {
                    Some((id, _)) => {
                        select_note(app, id, ctrl);
                        if let Some(n) = clip.note(id) {
                            app.audition(track, n.pitch);
                        }
                    }
                    None if !ctrl => {
                        let start = snap_floor(geo.tick_at(pos.x)).max(Ticks::ZERO);
                        let pitch = geo.pitch_at(pos.y);
                        let length = if snapping {
                            app.piano_roll.last_note_length.max(grid_ticks)
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
                    None => app.selected_notes.clear(),
                },
                Tool::Select => match hit {
                    Some((id, _)) => select_note(app, id, ctrl),
                    None if !ctrl => app.selected_notes.clear(),
                    None => {}
                },
                Tool::Delete => {
                    if let Some((id, _)) = hit {
                        app.erase_notes(vec![id], None);
                    }
                }
                Tool::Mute => {
                    if let Some((id, _)) = hit {
                        let ids = if app.selected_notes.contains(&id) {
                            app.selected_notes.clone()
                        } else {
                            vec![id]
                        };
                        app.toggle_mute_notes(ids);
                    }
                }
                Tool::Slice => {
                    let tick = snap_round(geo.tick_at(pos.x));
                    app.slice_at(tick);
                }
                Tool::Playback => {
                    let pitch = hit
                        .and_then(|(id, _)| clip.note(id).map(|n| n.pitch))
                        .unwrap_or_else(|| geo.pitch_at(pos.y));
                    app.audition(track, pitch);
                }
            }
        }
    }
    // Right-click deletes in every editing tool (FL convention).
    if resp.secondary_clicked() {
        if let Some((id, _)) = resp
            .interact_pointer_pos()
            .and_then(|p| geo.hit(&clip.notes, p))
        {
            let ids = if app.selected_notes.contains(&id) {
                app.selected_notes.clone()
            } else {
                vec![id]
            };
            app.erase_notes(ids, None);
        }
    }

    // ---- drags --------------------------------------------------------------------------
    if resp.drag_started_by(PointerButton::Primary) {
        let press = press.unwrap_or_default();
        match tool {
            Tool::Draw | Tool::Select => match press_hit {
                Some((id, resize)) if !(ctrl && tool == Tool::Select) => {
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
                _ => {
                    app.piano_roll.marquee = Some((press, press));
                    if !ctrl {
                        app.selected_notes.clear();
                    }
                }
            },
            Tool::Paint => {
                app.piano_roll.stroke += 1;
                app.piano_roll.paint_last = None;
                paint_cell(app, geo, clip, track, clip_id, press, snapping, grid_ticks);
            }
            Tool::Delete => {
                app.piano_roll.stroke += 1;
                if let Some((id, _)) = press_hit {
                    let key = MergeKey::of(&("erase", app.piano_roll.stroke));
                    app.erase_notes(vec![id], Some(key));
                }
            }
            Tool::Slice => {
                let x = geo.x(ticks_to_beats(snap_round(geo.tick_at(press.x))));
                app.piano_roll.slice_x = Some(x);
            }
            Tool::Mute | Tool::Playback => {}
        }
    }
    if resp.dragged_by(PointerButton::Primary) {
        if let Some(pos) = resp.interact_pointer_pos() {
            match tool {
                Tool::Draw | Tool::Select => {
                    if let Some(d) = app.piano_roll.drag.as_ref() {
                        let d_beats = (pos.x - d.press.x) / geo.px_per_beat;
                        let raw = beats_to_ticks(d_beats);
                        let d_pitch = -((pos.y - d.press.y) / geo.key_h).round() as i32;
                        let notes: Vec<Note> = match d.kind {
                            DragKind::Move => {
                                // Shift locks pitch, Ctrl locks time (FL convention).
                                let dt = if ctrl { Ticks::ZERO } else { snap_round(raw) };
                                let dp = if mods.shift { 0 } else { d_pitch };
                                d.originals
                                    .iter()
                                    .map(|n| Note {
                                        start: (n.start + dt).max(Ticks::ZERO),
                                        pitch: (n.pitch as i32 + dp).clamp(0, 127) as u8,
                                        ..n.clone()
                                    })
                                    .collect()
                            }
                            DragKind::Resize => d
                                .originals
                                .iter()
                                .map(|n| {
                                    let end = snap_round(n.end() + raw);
                                    let min_len = if snapping { grid_ticks } else { Ticks(10) };
                                    Note {
                                        length: (end - n.start).max(min_len),
                                        ..n.clone()
                                    }
                                })
                                .collect(),
                            DragKind::Velocity => return,
                        };
                        if d.kind == DragKind::Resize {
                            if let Some(first) = notes.first() {
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
                Tool::Paint => {
                    paint_cell(app, geo, clip, track, clip_id, pos, snapping, grid_ticks)
                }
                Tool::Delete => {
                    if let Some((id, _)) = hover_hit {
                        let key = MergeKey::of(&("erase", app.piano_roll.stroke));
                        app.erase_notes(vec![id], Some(key));
                    }
                }
                Tool::Slice => {
                    let x = geo.x(ticks_to_beats(snap_round(geo.tick_at(pos.x))));
                    app.piano_roll.slice_x = Some(x);
                }
                Tool::Mute | Tool::Playback => {}
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
        }
        if let Some(x) = app.piano_roll.slice_x.take() {
            let tick = geo.tick_at(x);
            app.slice_at(tick);
        }
        app.piano_roll.marquee = None;
        app.piano_roll.paint_last = None;
        app.end_gesture();
    }
}

fn select_note(app: &mut CantusApp, id: NoteId, toggle: bool) {
    if toggle {
        if let Some(i) = app.selected_notes.iter().position(|n| *n == id) {
            app.selected_notes.remove(i);
        } else {
            app.selected_notes.push(id);
        }
    } else {
        app.selected_notes = vec![id];
    }
}

/// Paint tool: add a grid-length note at the cell under `pos` unless that cell was the
/// last one painted in this stroke or already holds a note at that pitch.
#[allow(clippy::too_many_arguments)]
fn paint_cell(
    app: &mut CantusApp,
    geo: &Geo,
    clip: &cantus_core::Clip,
    track: TrackId,
    clip_id: ClipId,
    pos: Pos2,
    snapping: bool,
    grid_ticks: Ticks,
) {
    let raw = geo.tick_at(pos.x);
    let start = if snapping {
        raw.snap_floor(grid_ticks)
    } else {
        raw
    }
    .max(Ticks::ZERO);
    let pitch = geo.pitch_at(pos.y);
    if app.piano_roll.paint_last == Some((start, pitch)) {
        return;
    }
    app.piano_roll.paint_last = Some((start, pitch));
    let occupied = clip
        .notes
        .iter()
        .any(|n| n.pitch == pitch && n.start <= start && n.end() > start);
    if occupied {
        return;
    }
    let length = if snapping {
        grid_ticks
    } else {
        app.piano_roll.last_note_length
    };
    let note = app.bus.new_note(start, length, pitch, 100);
    let key = MergeKey::of(&("paint", app.piano_roll.stroke));
    app.exec_merging(
        Command::Batch {
            label_key: "cmd.paint".into(),
            commands: vec![Command::InsertNotes {
                track,
                clip: clip_id,
                notes: vec![note],
            }],
        },
        key,
    );
}
