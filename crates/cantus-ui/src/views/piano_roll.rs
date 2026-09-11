//! Piano roll: the dense view. Everything is painted with primitives on the visible range
//! only (ADR 0003 guardrail), never one widget per note.
//!
//! Mouse tools follow FL Studio's toolbar and letter shortcuts (P B E D T C Y); the number
//! keys 1–7 work too, and always, even while the typing keyboard plays notes. Hit testing always uses the *press origin*, never the current
//! pointer position, so a drag that leaves a note's rectangle still acts on that note.

use super::{beats_to_ticks, ticks_to_beats, GRIDS};
use crate::app::CantusApp;
use crate::design;
use crate::i18n::{t, tf};
use crate::theme;
use cantus_core::{
    pitch_name, Clip, ClipId, Command, InstrumentSpec, MergeKey, Note, NoteId, Pitch, Ticks,
    TrackId,
};
use egui::{
    pos2, vec2, Align2, Color32, CornerRadius, PointerButton, Pos2, Rect, Sense, Stroke, StrokeKind,
};

const KEYS_W: f32 = 56.0;
/// Wider when the lanes are named: "Closed hat" does not fit in the width of "F#2".
const PAD_KEYS_W: f32 = 104.0;
/// Tallest a kit's row grows when the rows are stretched to fill the grid.
const PAD_ROW_MAX: f32 = 44.0;
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

    /// The keymap action that selects this tool, so the toolbar can show whatever
    /// shortcut is actually bound rather than a hard-coded letter.
    pub fn action(self) -> crate::keymap::Action {
        use crate::keymap::Action;
        match self {
            Tool::Draw => Action::ToolDraw,
            Tool::Paint => Action::ToolPaint,
            Tool::Select => Action::ToolSelect,
            Tool::Delete => Action::ToolDelete,
            Tool::Mute => Action::ToolMute,
            Tool::Slice => Action::ToolSlice,
            Tool::Playback => Action::ToolPlayback,
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

/// The piano roll's vertical axis: what each row is, for the instrument being edited.
///
/// A keyboard has 128 rows and calls them `C4`; a kit has one row per pad and calls them
/// `Kick`. The view asks the instrument (`InstrumentSpec::lanes`) rather than assuming,
/// which is what makes drum mode a property of the kit instead of a switch the user has
/// to find.
///
/// **The clip has a say too.** A kit's rows are its pads — but a clip may hold notes on
/// pitches the kit has no pad for, most often because the track used to be a synth. Those
/// get a row of their own, named the way a keyboard would name it, so that changing a
/// track's instrument can never make music *invisible*. Editing them is how you get them
/// back.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Axis {
    /// Ascending pitches, one per row. **Empty means chromatic**, where row and pitch are
    /// the same number and no vector is built at all — the common case must not allocate
    /// 128 entries every frame.
    pitches: Vec<Pitch>,
    names: Vec<String>,
    /// The instrument named these rows, so this is a kit and not a keyboard.
    pads: bool,
}

impl Axis {
    pub fn build(instrument: &InstrumentSpec, clip: &Clip) -> Axis {
        let lanes = instrument.lanes();
        if lanes.is_chromatic() {
            return Axis::default();
        }
        let mut pitches: Vec<Pitch> = (0..lanes.count()).filter_map(|i| lanes.pitch(i)).collect();
        // Anything in the clip the kit cannot play still gets a row.
        for n in &clip.notes {
            if !pitches.contains(&n.pitch) {
                pitches.push(n.pitch);
            }
        }
        pitches.sort_unstable();
        let names = pitches
            .iter()
            .map(|p| match lanes.lane_of(*p) {
                Some(lane) => lanes.name(lane),
                None => pitch_name(*p),
            })
            .collect();
        Axis {
            pitches,
            names,
            pads: true,
        }
    }

    /// Whether every pitch is a row. True for a keyboard.
    pub fn is_chromatic(&self) -> bool {
        self.pitches.is_empty()
    }

    /// Whether the rows are named pads — the one thing the drawing branches on.
    pub fn pads(&self) -> bool {
        self.pads
    }

    pub fn len(&self) -> usize {
        if self.pitches.is_empty() {
            128
        } else {
            self.pitches.len()
        }
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// The pitch a row plays. Out-of-range rows clamp to the nearest, so a pointer just
    /// past the top of the grid edits the top row rather than nothing.
    pub fn pitch(&self, lane: usize) -> Pitch {
        if self.pitches.is_empty() {
            lane.min(127) as Pitch
        } else {
            self.pitches[lane.min(self.pitches.len() - 1)]
        }
    }

    pub fn lane_of(&self, pitch: Pitch) -> Option<usize> {
        if self.pitches.is_empty() {
            Some(pitch as usize)
        } else {
            self.pitches.iter().position(|p| *p == pitch)
        }
    }

    pub fn name(&self, lane: usize) -> String {
        if self.pitches.is_empty() {
            pitch_name(lane.min(127) as Pitch)
        } else {
            self.names
                .get(lane)
                .cloned()
                .unwrap_or_else(|| pitch_name(self.pitch(lane)))
        }
    }

    /// Move a pitch by `rows`, in **rows** rather than semitones.
    ///
    /// On a keyboard the two are the same thing. On a kit they are emphatically not: one
    /// row above a snare is the next pad, and a "semitone" is a pitch the kit has no drum
    /// for. Every vertical move — dragging, transposing — goes through here, so the two
    /// axes can never disagree about what up means.
    pub fn moved(&self, pitch: Pitch, rows: i32) -> Pitch {
        let Some(lane) = self.lane_of(pitch) else {
            return pitch;
        };
        let target = (lane as i32 + rows).clamp(0, self.len() as i32 - 1);
        self.pitch(target as usize)
    }
}

#[derive(Debug)]
pub struct PianoRollView {
    pub px_per_beat: f32,
    pub scroll_beats: f32,
    pub key_height: f32,
    /// The **row** at the top edge, fractional for smooth scrolling.
    ///
    /// Rows, not pitches: on a kit the two are different numbers and the scroll position
    /// has to mean the same thing in both. On a keyboard they coincide, which is why the
    /// default reads as a pitch.
    pub top_lane: f32,
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
            top_lane: 84.0,
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

struct Geo<'a> {
    grid: Rect,
    px_per_beat: f32,
    scroll_beats: f32,
    key_h: f32,
    top_lane: f32,
    axis: &'a Axis,
}

impl Geo<'_> {
    fn x(&self, beats: f32) -> f32 {
        self.grid.left() + (beats - self.scroll_beats) * self.px_per_beat
    }
    fn beats(&self, x: f32) -> f32 {
        (x - self.grid.left()) / self.px_per_beat + self.scroll_beats
    }
    /// Screen y of a row boundary. Row `l` occupies `y(l + 1) ..= y(l)`.
    fn y(&self, lane: f32) -> f32 {
        self.grid.top() + (self.top_lane - lane) * self.key_h
    }
    fn lane(&self, y: f32) -> f32 {
        self.top_lane - (y - self.grid.top()) / self.key_h
    }
    fn lane_at(&self, y: f32) -> usize {
        self.lane(y)
            .floor()
            .clamp(0.0, self.axis.len() as f32 - 1.0) as usize
    }
    /// Screen y of the top of the row a pitch lives on, or `None` when this axis has no
    /// row for it — a live note being recorded onto a pitch the kit cannot play.
    fn y_of_pitch(&self, pitch: Pitch) -> Option<f32> {
        self.axis.lane_of(pitch).map(|l| self.y(l as f32 + 1.0))
    }
    fn pitch_at(&self, y: f32) -> Pitch {
        self.axis.pitch(self.lane_at(y))
    }
    fn tick_at(&self, x: f32) -> Ticks {
        beats_to_ticks(self.beats(x))
    }
    fn note_rect(&self, n: &Note) -> Rect {
        let x0 = self.x(ticks_to_beats(n.start));
        let x1 = self.x(ticks_to_beats(n.end()));
        // Every pitch in the clip has a row (`Axis::build`), so the fallback is only ever
        // reached by a note that is not in the clip yet.
        let y0 = match self.y_of_pitch(n.pitch) {
            Some(y) => y,
            None => return Rect::NOTHING,
        };
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

    // The vertical axis comes from the *instrument*, once per frame, before anything is
    // laid out: it decides how wide the key strip has to be as well as what is in it.
    let axis = app
        .project()
        .track(track_id)
        .map(|t| Axis::build(&t.instrument, &clip))
        .unwrap_or_default();
    let keys_w = if axis.pads() { PAD_KEYS_W } else { KEYS_W };

    let full = ui.available_rect_before_wrap();
    let grid = Rect::from_min_max(
        pos2(full.left() + keys_w, full.top() + RULER_H),
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
            v.px_per_beat = (v.px_per_beat * zoom).clamp(MIN_PX_PER_BEAT, MAX_PX_PER_BEAT);
            v.scroll_beats = (anchor - (hover_x - grid.left()) / v.px_per_beat).max(0.0);
        } else if scroll != egui::Vec2::ZERO {
            if mods.shift || scroll.x != 0.0 {
                let dx = if mods.shift { scroll.y } else { scroll.x };
                v.scroll_beats = (v.scroll_beats - dx / v.px_per_beat).max(0.0);
            }
            if !mods.shift && scroll.y != 0.0 {
                // The top of the scroll range is one row past the last, so the highest row
                // can reach the top of the grid; the bottom keeps one row on screen.
                let top = axis.len() as f32;
                let row_h = if axis.pads() {
                    (grid.height() / top.max(1.0)).clamp(v.key_height, PAD_ROW_MAX)
                } else {
                    v.key_height
                };
                v.top_lane = (v.top_lane + scroll.y / row_h).clamp(1.0, top);
            }
        }
    }
    // A kit has a dozen rows, a keyboard 128, and the stored scroll position is in rows:
    // switching between them has to land somewhere sensible rather than off the end.
    let max_top = axis.len() as f32;
    if app.piano_roll.top_lane > max_top || app.piano_roll.top_lane < 1.0 {
        app.piano_roll.top_lane = max_top;
    }
    // A kit's rows grow to fill the grid. Seven pads at keyboard height are a strip along
    // the top of an empty canvas; seven pads at a sixth of the height each are a drum
    // editor. The stored `key_height` stays the keyboard's zoom, untouched.
    let key_h = if axis.pads() {
        let fit = grid.height() / axis.len().max(1) as f32;
        fit.clamp(app.piano_roll.key_height, PAD_ROW_MAX)
    } else {
        app.piano_roll.key_height
    };
    if axis.pads() && key_h * axis.len() as f32 <= grid.height() + 0.5 {
        // Everything fits: pin the top row to the top so nothing scrolls off.
        app.piano_roll.top_lane = max_top;
    }
    let v = &app.piano_roll;
    let geo = Geo {
        grid,
        px_per_beat: v.px_per_beat,
        scroll_beats: v.scroll_beats,
        key_h,
        top_lane: v.top_lane,
        axis: &axis,
    };

    paint_grid(app, ui, &geo, &clip);
    paint_keys(ui, &geo, keys);
    paint_ruler(app, ui, &geo, ruler, &clip);

    // Notes.
    let p = ui.painter().with_clip_rect(grid);
    let visible_start = beats_to_ticks(geo.beats(grid.left())) - Ticks::beats(64);
    let visible_end = beats_to_ticks(geo.beats(grid.right()));
    let lo_lane = geo.lane(grid.bottom()).floor() as i32 - 1;
    let hi_lane = geo.lane(grid.top()).ceil() as i32 + 1;
    let mut drawn = 0usize;
    // Notes small enough to need no corners and no outline go into one mesh instead of
    // one shape each. epaint anti-aliases every filled shape by expanding it into a
    // feathered ring — about twenty triangles for a rectangle — and at this size the
    // feathering is invisible anyway. Two triangles per note, one shape for the lot, and
    // the tessellator has almost nothing to do.
    let mut bulk = egui::Mesh::default();
    for n in &clip.notes {
        if n.start > visible_end {
            break; // notes are sorted by start
        }
        let Some(lane) = axis.lane_of(n.pitch) else {
            continue;
        };
        if n.end() < visible_start || (lane as i32) < lo_lane || (lane as i32) > hi_lane {
            continue;
        }
        let r = geo.note_rect(n);
        let selected = app.selected_notes.contains(&n.id);
        let alpha = 0.55 + 0.45 * (n.velocity as f32 / 127.0);
        // Selection has to survive any theme and any track colour. The accent does not:
        // on a light theme the gold sits right next to the warm track colours, and the
        // white outline that made it obvious on ink disappeared into the page. The
        // palette's `selection` is the one colour reserved for exactly this.
        let base = if selected {
            theme::p().selection
        } else {
            track_color
        };
        // Level of detail. Zoomed out, a note can be a fraction of a pixel wide, and
        // rounding its corners and stroking its outline costs ~30 triangles to render
        // detail no one can see. Below the thresholds a note is a single quad: two
        // triangles, same colour on screen, an order of magnitude less work.
        let lod = NoteLod::for_rect(r);
        if lod.bulk {
            let fill = if selected {
                theme::p().selection
            } else if n.muted {
                base.gamma_multiply(0.35)
            } else {
                base.gamma_multiply(alpha)
            };
            bulk.add_colored_rect(r, fill);
            drawn += 1;
            continue;
        }
        if n.muted {
            p.rect_filled(r, lod.radius, theme::p().panel_raised);
            if lod.stroke {
                p.rect_stroke(
                    r,
                    lod.radius,
                    Stroke::new(1.0, base.gamma_multiply(0.7)),
                    StrokeKind::Inside,
                );
            } else {
                // The outline is what says "muted" at this size, so keep the colour
                // rather than the shape: a dimmed fill reads the same way.
                p.rect_filled(r, lod.radius, base.gamma_multiply(0.35));
            }
        } else {
            p.rect_filled(r, lod.radius, base.gamma_multiply(alpha));
            if lod.stroke {
                p.rect_stroke(
                    r,
                    lod.radius,
                    Stroke::new(
                        1.0,
                        if selected {
                            // Maximum contrast against the canvas, whatever the canvas is.
                            theme::p().text
                        } else {
                            theme::p().ink
                        },
                    ),
                    StrokeKind::Inside,
                );
            } else if selected {
                // Too small for an outline: the selection colour fills it instead.
                p.rect_filled(r, lod.radius, theme::p().selection);
            }
        }
        if r.width() > 28.0 && geo.key_h >= 11.0 {
            p.with_clip_rect(r).text(
                pos2(r.left() + 3.0, r.center().y),
                Align2::LEFT_CENTER,
                axis.name(lane),
                design::paint::ruler(),
                if n.muted {
                    theme::p().text_dim
                } else {
                    theme::p().ink
                },
            );
        }
        drawn += 1;
    }
    if !bulk.is_empty() {
        p.add(egui::Shape::Mesh(std::sync::Arc::new(bulk)));
    }
    app.piano_roll_notes_drawn = drawn;

    // Notes being recorded: from their start to the playhead.
    if app.recorder.is_recording() {
        let ph_beats = ticks_to_beats(clip.content_of(app.engine.position()));
        for pending in app.recorder.pending() {
            let x0 = geo.x(ticks_to_beats(clip.content_of(pending.start)));
            let x1 = geo.x(ph_beats).max(x0 + 2.0);
            // A live note on a pitch this kit has no pad for has nowhere to be drawn. It
            // is not lost: committing the take gives it a row of its own (`Axis::build`).
            let Some(y0) = geo.y_of_pitch(pending.pitch) else {
                continue;
            };
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

    // Playhead, in the clip's content frame: a slipped clip shows the playhead where the
    // music is, and a repeating one shows it inside the period it is playing.
    let ph = clip
        .content_shown_at(app.engine.position())
        .unwrap_or_else(|| clip.content_of(app.engine.position()));
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

/// How much detail a note is worth at its current size on screen.
///
/// This is the difference between the piano roll running at 60 fps when zoomed out and
/// running at 9: rounded corners and a stroked outline cost about thirty triangles per
/// note, and below a few pixels neither is visible at all.
#[derive(Copy, Clone, Debug, PartialEq)]
struct NoteLod {
    radius: CornerRadius,
    stroke: bool,
    /// Draw this note into the shared mesh rather than as a shape of its own.
    bulk: bool,
}

impl NoteLod {
    /// Corners stop reading below this size, so stop paying for them.
    const ROUND_ABOVE: f32 = 7.0;
    /// A 1-px outline inside a rectangle this small eats the fill it is supposed to frame.
    const STROKE_ABOVE_W: f32 = 3.5;
    const STROKE_ABOVE_H: f32 = 5.0;

    fn for_rect(r: Rect) -> NoteLod {
        let (w, h) = (r.width(), r.height());
        let radius = if w >= Self::ROUND_ABOVE && h >= Self::ROUND_ABOVE {
            CornerRadius::same(2)
        } else {
            CornerRadius::ZERO
        };
        let stroke = w >= Self::STROKE_ABOVE_W && h >= Self::STROKE_ABOVE_H;
        NoteLod {
            radius,
            stroke,
            // A plain square-cornered fill is exactly what a mesh quad draws, so anything
            // that needs neither a corner nor an outline can join the batch.
            bulk: radius == CornerRadius::ZERO && !stroke,
        }
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
            // Show the shortcut that is bound right now, not the one that shipped.
            let keys = app.keymap.display_for(tool.action());
            let tip = if keys.is_empty() {
                t(tool.label_key())
            } else {
                format!("{} ({keys})", t(tool.label_key()))
            };
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

fn paint_grid(app: &CantusApp, ui: &egui::Ui, geo: &Geo<'_>, clip: &cantus_core::Clip) {
    let p = ui.painter().with_clip_rect(geo.grid);
    let grid = geo.grid;
    p.rect_filled(grid, 0.0, theme::p().ink);
    let lo = geo.lane(grid.bottom()).floor().max(0.0) as i32;
    let hi = (geo.lane(grid.top()).ceil() as i32).min(geo.axis.len() as i32);
    for lane in lo..hi {
        let y1 = geo.y(lane as f32);
        let y0 = y1 - geo.key_h;
        let r = Rect::from_min_max(pos2(grid.left(), y0), pos2(grid.right(), y1));
        // A keyboard shades its accidentals; a kit has no accidentals, so its rows simply
        // alternate. Striping a kit by `pitch % 12` would draw a piano's pattern across
        // drums that have nothing to do with it.
        let dark = if geo.axis.pads() {
            lane % 2 == 1
        } else {
            is_black(geo.axis.pitch(lane as usize))
        };
        p.rect_filled(
            r,
            0.0,
            if dark {
                theme::p().row_black
            } else {
                theme::p().row_white
            },
        );
        let octave = !geo.axis.pads() && geo.axis.pitch(lane as usize) % 12 == 0;
        if octave {
            p.hline(grid.x_range(), y1, Stroke::new(1.0, theme::p().grid_strong));
        }
    }
    // Past the end of the clip. A black scrim was invisible on a dark theme and a heavy
    // grey slab on a light one; painting the panel colour instead says "not part of this
    // clip" as one step of the elevation scale, and reads the same way in any theme.
    //
    // Both edges, not just the end: with a content offset the window can start well into
    // the content, and what is before it is exactly as much "not part of this clip" as
    // what is after.
    let window = clip.period().map_or(clip.length, |p| p.min(clip.length));
    let start_x = geo.x(ticks_to_beats(clip.content_start()));
    if start_x > grid.left() {
        p.rect_filled(
            Rect::from_min_max(grid.min, pos2(start_x, grid.bottom())),
            0.0,
            theme::p().panel,
        );
    }
    let clip_end_x = geo.x(ticks_to_beats(clip.content_start() + window));
    if clip_end_x < grid.right() {
        p.rect_filled(
            Rect::from_min_max(pos2(clip_end_x, grid.top()), grid.max),
            0.0,
            theme::p().panel,
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

fn paint_keys(ui: &egui::Ui, geo: &Geo<'_>, keys: Rect) {
    let p = ui.painter().with_clip_rect(keys);
    p.rect_filled(keys, 0.0, theme::p().panel);
    let lo = geo.lane(keys.bottom()).floor().max(0.0) as i32;
    let hi = (geo.lane(keys.top()).ceil() as i32).min(geo.axis.len() as i32);
    for lane in lo..hi {
        let y1 = geo.y(lane as f32);
        let y0 = y1 - geo.key_h;
        let r = Rect::from_min_max(pos2(keys.left(), y0), pos2(keys.right(), y1));
        if geo.axis.pads() {
            // Named rows: no black keys and no shortened key — every pad is one full-width
            // lane with its name in it, which is the whole point of asking the instrument.
            p.rect_filled(
                r.shrink2(vec2(0.0, 0.5)),
                0.0,
                if lane % 2 == 1 {
                    theme::p().panel_raised
                } else {
                    theme::p().white_key
                },
            );
            if geo.key_h >= 9.0 {
                p.with_clip_rect(r).text(
                    pos2(keys.left() + 5.0, r.center().y),
                    Align2::LEFT_CENTER,
                    geo.axis.name(lane as usize),
                    design::paint::label(),
                    theme::on_light_key(&theme::p()),
                );
            }
            continue;
        }
        let pitch = geo.axis.pitch(lane as usize);
        let black = is_black(pitch);
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
                pitch_name(pitch),
                design::paint::ruler(),
                theme::on_light_key(&theme::p()),
            );
        }
    }
    p.vline(
        keys.right() - 0.5,
        keys.y_range(),
        Stroke::new(1.0, theme::p().grid_strong),
    );
}

/// Zoom limits, in pixels per beat.
///
/// The lower bound used to be 4.0, which showed about 80 bars — the limit was there
/// because drawing that many notes was ruinous. Since notes below a few pixels became two
/// triangles in a shared mesh (ADR 0011), 20 000 of them cost about 7 ms, so the limit can
/// be what a musician actually wants: at 0.5 a 1 300-px grid shows some 650 bars, which is
/// most of an hour of music, and the whole piece fits on screen.
const MIN_PX_PER_BEAT: f32 = 0.5;
const MAX_PX_PER_BEAT: f32 = 800.0;

/// Least space between two bar numbers in the ruler. Wide enough for four digits and a
/// gap, so the numbers stay separate at any zoom.
const RULER_LABEL_GAP: f32 = 44.0;

fn paint_ruler(
    app: &CantusApp,
    ui: &egui::Ui,
    geo: &Geo<'_>,
    ruler: Rect,
    clip: &cantus_core::Clip,
) {
    let p = ui.painter().with_clip_rect(ruler);
    p.rect_filled(ruler, 0.0, theme::p().panel);
    let ts = app.project().time_signature;
    let bar = ts.ticks_per_bar();
    // Zoomed out, a bar can be two pixels wide, and one number per bar becomes a smear.
    // Thin them out by *doubling* rather than by whatever fits, so the numbers shown stay
    // ones a musician counts in — every bar, every 2, every 4 — and never every 7th.
    let bar_px = ticks_to_beats(bar) * geo.px_per_beat;
    let mut bars_per_label = 1i64;
    while bar_px * (bars_per_label as f32) < RULER_LABEL_GAP {
        bars_per_label *= 2;
    }
    let step = bar * bars_per_label;
    // Bar numbers are the *arrangement's*, so the piano roll and the timeline agree even
    // when the clip's content is offset from where it sits.
    let to_timeline = clip.start - clip.offset;
    let first = (beats_to_ticks(geo.beats(ruler.left()).max(0.0)) + to_timeline).snap_floor(step)
        - to_timeline;
    let last = beats_to_ticks(geo.beats(ruler.right()));
    let mut tick = first;
    while tick <= last {
        let x = geo.x(ticks_to_beats(tick));
        let (b, _, _) = ts.bbt(tick + to_timeline);
        p.vline(x, ruler.y_range(), Stroke::new(1.0, theme::p().grid_strong));
        p.text(
            pos2(x + 3.0, ruler.center().y),
            Align2::LEFT_CENTER,
            b.to_string(),
            design::paint::ruler(),
            theme::p().text_dim,
        );
        tick += step;
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
    geo: &Geo<'_>,
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
        design::paint::caption(),
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
    // One mesh for every bar, for the same reason the notes use one: these are plain
    // square-cornered rectangles, and letting epaint anti-alias each of them separately
    // costs about ten triangles apiece to feather an edge nobody can see.
    let mut bars = egui::Mesh::default();
    for n in &clip.notes {
        let x = geo.x(ticks_to_beats(n.start));
        if x < vel.left() - 4.0 || x > vel.right() {
            continue;
        }
        let h = (inner_bottom - inner_top) * n.velocity as f32 / 127.0;
        let r = Rect::from_min_max(pos2(x, inner_bottom - h), pos2(x + 4.0, inner_bottom));
        let selected = app.selected_notes.contains(&n.id);
        bars.add_colored_rect(
            r,
            if selected {
                theme::p().gold_bright
            } else if n.muted {
                theme::p().text_dim
            } else {
                color
            },
        );
    }
    if !bars.is_empty() {
        p.add(egui::Shape::Mesh(std::sync::Arc::new(bars)));
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
    geo: &Geo<'_>,
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
                + (clip.start - clip.offset);
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
                        let length = drawn_note_length(
                            geo.axis,
                            snapping,
                            grid_ticks,
                            app.piano_roll.last_note_length,
                        );
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
                        // Rows, not semitones: on a kit one row up is the next pad.
                        let d_rows = -((pos.y - d.press.y) / geo.key_h).round() as i32;
                        let notes: Vec<Note> = match d.kind {
                            DragKind::Move => {
                                // Shift locks pitch, Ctrl locks time (FL convention).
                                let dt = if ctrl { Ticks::ZERO } else { snap_round(raw) };
                                let dp = if mods.shift { 0 } else { d_rows };
                                d.originals
                                    .iter()
                                    .map(|n| Note {
                                        start: (n.start + dt).max(Ticks::ZERO),
                                        pitch: geo.axis.moved(n.pitch, dp),
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

/// How long a note drawn by hand should be.
///
/// On a kit it is a fixed short mark, and deliberately not the grid: a drum has no
/// note-off worth aiming at, so its length carries no musical information, and a note as
/// long as the grid would tile the row into a solid block the moment somebody used a
/// coarse grid. A 1/16 stays legible at every zoom and reads as a *hit*.
fn drawn_note_length(axis: &Axis, snapping: bool, grid: Ticks, last: Ticks) -> Ticks {
    if axis.pads() {
        Ticks::SIXTEENTH.min(grid)
    } else if snapping {
        last.max(grid)
    } else {
        last
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
    geo: &Geo<'_>,
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
    let length = drawn_note_length(geo.axis, snapping, grid_ticks, grid_ticks);
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
