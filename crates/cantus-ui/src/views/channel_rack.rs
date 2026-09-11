//! The channel rack: every track down the side, a step grid across, editing one pattern.
//!
//! ## A channel is a track
//!
//! FL's channel rack lists *channels* — each its own sound, its own level and pan, its own
//! route into the mixer. In Cantus that thing already exists and is called a track: it has
//! an instrument, a mixer strip, inserts, and clips. So the rack lists **every track**, one
//! row each, and a kit with several pads expands into a row per pad under its track. A
//! kick that needs its own strip is a track with a one-pad kit
//! (`PulsusParams::single`); a whole kit on one strip is a track with seven. Both are the
//! same type, the same rows and the same edits — which is what lets a musician choose
//! rather than us.
//!
//! Routing several tracks into one mixer channel is a *bus*, and buses are the next block
//! after this one. Each track already lands on its own strip today, and the strip number
//! is shown on the row so the route is visible before it is editable.
//!
//! ## A step *is* a note
//!
//! Toggling a step is `InsertNotes` / `RemoveNotes`, exactly as drawing in the piano roll
//! is, so it undoes, it saves, and it is the same music the piano roll shows. Switching
//! views in the middle of an edit converts nothing and loses nothing, because there is
//! nothing to convert. FL keeps a step sequencer and a piano roll as two representations
//! that have to be translated between; we do not have to, and a DAW that runs two data
//! models for one idea is a DAW with two of everything.
//!
//! ## A pattern is a window of time, across every track
//!
//! FL's channel rack edits "the current pattern", one per project at a time. Ours edits a
//! **pattern region**: the window of the selected clip — its start, and one period if it
//! repeats or its whole length if not — on *every* track. Each row shows what its track's
//! clip plays inside that window, and switching a step on in a row whose track has no
//! clip there creates one, the same size and with the same repeat as the selected clip,
//! in the same undo step. Clips stay the unit of reuse (duplicate, repeat, colour, name),
//! so there is still one timeline model; the rack is a way of looking at a bar of it
//! across the whole band.
//!
//! ## The grid is dense from the first line
//!
//! Rows × steps, redrawn every frame, is precisely the shape ADR 0011's rule is about: the
//! cells go into one [`egui::Mesh`] with a level-of-detail rule, not one painter call each.

use super::GRIDS;
use crate::app::CantusApp;
use crate::design::{self, space};
use crate::i18n::{t, tf};
use crate::icons;
use crate::theme;
use crate::views::piano_roll::Axis;
use crate::widgets::{Bar, Fader};
use cantus_core::{
    Clip, ClipId, Command, InstrumentParam, MergeKey, Note, NoteId, PadField, Pitch, Ticks, TrackId,
};
use egui::{pos2, vec2, Align2, PointerButton, Rect, Sense, Stroke, StrokeKind};

/// Width of the channel column: a colour stripe, an icon, a strip number, a name, a mute,
/// a level and a pan, none of them a four-pixel target.
const CHANNELS_W: f32 = 300.0;
const HEADER_H: f32 = 20.0;
const MIN_STEP_W: f32 = 3.0;
const MAX_STEP_W: f32 = 64.0;
const MIN_ROW_H: f32 = 16.0;
const MAX_ROW_H: f32 = 44.0;
/// How far a pad row is indented under its kit's row.
const PAD_INDENT: f32 = 22.0;
/// The pitch a step on a melodic row adds when the clip gives no better hint.
const MELODIC_STEP_PITCH: Pitch = 60;
/// A melodic track's single row edits *any* pitch; this marks that lane.
const ANY_PITCH: usize = usize::MAX;

#[derive(Debug)]
pub struct ChannelRackView {
    /// Width of one step cell, in points. Ctrl+wheel changes it.
    pub step_w: f32,
    pub row_h: f32,
    /// Leftmost visible step (fractional, for smooth scrolling).
    pub scroll_steps: f32,
    /// What one step is worth. Its own setting rather than the piano roll's, because a
    /// rack at 1/16 and a roll at 1/32 is a normal way to work.
    pub grid: Ticks,
    /// Velocity given to a step switched on by hand.
    pub velocity: u8,
    /// The pad of the *selected track* whose parameters the inspector is showing.
    pub selected_pad: usize,
    /// Fit the whole pattern to the width. On until the user zooms, and back on when the
    /// grid changes: a sixteen-bar pattern at a readable cell size is wider than any
    /// screen, and a pattern you have to scroll to see the end of is not a pattern.
    pub fit: bool,
    /// Gesture counter, so each paint stroke merges into one undo step.
    stroke: u64,
    /// While a stroke is running: whether it is switching steps on or off, and the last
    /// cell it touched. Decided at the press and never re-decided — a stroke that changed
    /// its mind halfway is a stroke you cannot aim.
    paint: Option<(bool, Option<(usize, usize)>)>,
    /// A pad being renamed in place: its track, its index, and the text so far.
    rename: Option<(TrackId, usize, String)>,
    geometry: Option<RackGeometry>,
}

impl Default for ChannelRackView {
    fn default() -> Self {
        ChannelRackView {
            step_w: 26.0,
            row_h: 28.0,
            scroll_steps: 0.0,
            grid: Ticks::SIXTEENTH,
            velocity: 100,
            selected_pad: 0,
            fit: true,
            stroke: 0,
            paint: None,
            rename: None,
            geometry: None,
        }
    }
}

impl ChannelRackView {
    /// Where the grid was drawn last frame, or `None` before the first one.
    ///
    /// Published for the same reason the arrangement publishes its timeline: a test that
    /// recomputes the layout breaks the day a toolbar is added, which is the day it is
    /// least useful.
    pub fn geometry(&self) -> Option<RackGeometry> {
        self.geometry.clone()
    }
}

/// What a row of the rack is.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum RowKind {
    /// A track's row, carrying the track's own controls. Editable when the track has one
    /// lane — a melodic track, or a one-pad kit — and a heading otherwise.
    Track,
    /// One pad of a kit with several, under its track's row.
    Pad,
}

/// One row of the rack, as drawn.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct RackRow {
    pub top: f32,
    pub height: f32,
    /// Index into `project.tracks`.
    pub track: usize,
    pub kind: RowKind,
    /// The lane (into that track's [`Axis`]) this row edits; [`ANY_PITCH`] for a melodic
    /// track's row; `None` for a kit's heading, which edits nothing.
    pub lane: Option<usize>,
}

/// Where the rack drew its grid, in screen coordinates.
#[derive(Clone, Debug, PartialEq)]
pub struct RackGeometry {
    pub grid_left: f32,
    pub grid_top: f32,
    pub step_w: f32,
    pub scroll_steps: f32,
    pub steps: usize,
    pub rows: Vec<RackRow>,
}

impl RackGeometry {
    /// The cell for a row and a step.
    pub fn cell(&self, row: usize, step: usize) -> Rect {
        let x = self.grid_left + (step as f32 - self.scroll_steps) * self.step_w;
        let (top, h) = self
            .rows
            .get(row)
            .map(|r| (r.top, r.height))
            .unwrap_or((self.grid_top, 0.0));
        Rect::from_min_size(pos2(x, top), vec2(self.step_w, h))
    }

    /// The first editable row for a track — its own row for a melodic track or a one-pad
    /// kit, its first (highest) pad row otherwise. What a test aims at.
    pub fn row_for_track(&self, track: usize) -> Option<usize> {
        self.rows
            .iter()
            .position(|r| r.track == track && r.lane.is_some())
    }

    /// The row editing a particular lane of a track.
    pub fn row_for_lane(&self, track: usize, lane: usize) -> Option<usize> {
        self.rows
            .iter()
            .position(|r| r.track == track && r.lane == Some(lane))
    }

    fn step_at(&self, x: f32) -> Option<usize> {
        let s = (x - self.grid_left) / self.step_w + self.scroll_steps;
        (s >= 0.0 && (s as usize) < self.steps).then_some(s as usize)
    }

    fn row_at(&self, y: f32) -> Option<usize> {
        self.rows
            .iter()
            .position(|r| y >= r.top && y < r.top + r.height)
    }
}

/// The pattern region: a window of the timeline, in steps.
///
/// Taken from the **selected clip** — its start, and one period if it repeats or its whole
/// length if not — so the rack and the arrangement agree about how long the thing being
/// edited is. Every row maps its own clip into this window through the clip's timeline
/// positions, which is the same mapping the engine uses.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Pattern {
    pub start: Ticks,
    pub step: Ticks,
    pub count: usize,
    /// Whether the selected clip repeats, so a clip the rack creates on another track
    /// repeats the same way.
    pub repeats: bool,
}

impl Pattern {
    pub fn of(clip: &Clip, grid: Ticks) -> Pattern {
        let step = if grid.0 > 0 { grid } else { Ticks::SIXTEENTH };
        let window = clip.period().unwrap_or(clip.length).max(step);
        Pattern {
            start: clip.start,
            step,
            count: ((window.0 + step.0 - 1) / step.0).max(1) as usize,
            repeats: clip.period().is_some(),
        }
    }

    pub fn window(&self) -> Ticks {
        self.step * self.count as i64
    }

    /// The timeline tick a step starts at.
    pub fn tick(&self, step: usize) -> Ticks {
        self.start + self.step * step as i64
    }

    /// Which step a timeline tick falls in, if any.
    pub fn index_of(&self, tick: Ticks) -> Option<usize> {
        let d = (tick - self.start).0;
        if d < 0 {
            return None;
        }
        let i = (d / self.step.0) as usize;
        (i < self.count).then_some(i)
    }

    /// The clip on a track that plays inside this window: the one containing the window's
    /// start, or failing that the first one overlapping it.
    pub fn clip_on<'a>(&self, track: &'a cantus_core::Track) -> Option<&'a Clip> {
        let end = self.start + self.window();
        track
            .clips
            .iter()
            .find(|c| c.start <= self.start && c.end() > self.start)
            .or_else(|| {
                track
                    .clips
                    .iter()
                    .find(|c| c.start < end && c.end() > self.start)
            })
    }
}

fn axis_of(track: &cantus_core::Track, pattern: &Pattern) -> Axis {
    match pattern.clip_on(track) {
        Some(c) => Axis::build(&track.instrument, c),
        None => Axis::build(&track.instrument, &empty_clip()),
    }
}

fn empty_clip() -> Clip {
    Clip {
        id: ClipId(0),
        name: String::new(),
        start: Ticks::ZERO,
        length: Ticks::ZERO,
        offset: Ticks::ZERO,
        content_length: None,
        muted: false,
        color: None,
        notes: Vec::new(),
    }
}

/// The rows the rack lays out for a project: one per track, plus one per pad for a kit
/// with several. Pure, so the layout and the tests agree by construction.
fn layout_rows(app: &CantusApp, pattern: &Pattern, top: f32, row_h: f32) -> Vec<RackRow> {
    let mut rows = Vec::new();
    let mut y = top;
    for (i, track) in app.project().tracks.iter().enumerate() {
        let pads = !track.instrument.lanes().is_chromatic();
        let axis = axis_of(track, pattern);
        let single = !pads || axis.len() == 1;
        rows.push(RackRow {
            top: y,
            height: row_h,
            track: i,
            kind: RowKind::Track,
            lane: match (single, pads) {
                (true, true) => Some(0),
                (true, false) => Some(ANY_PITCH),
                (false, _) => None,
            },
        });
        y += row_h;
        if !single {
            // Highest pad first, as in the piano roll: kick at the bottom.
            for lane in (0..axis.len()).rev() {
                rows.push(RackRow {
                    top: y,
                    height: row_h,
                    track: i,
                    kind: RowKind::Pad,
                    lane: Some(lane),
                });
                y += row_h;
            }
        }
    }
    rows
}

/// The pitch a row edits: `Some(None)` for a melodic row (any pitch), `None` for a heading.
fn row_pitch(app: &CantusApp, pattern: &Pattern, row: &RackRow) -> Option<Option<Pitch>> {
    let lane = row.lane?;
    if lane == ANY_PITCH {
        return Some(None);
    }
    let track = app.project().tracks.get(row.track)?;
    Some(Some(axis_of(track, pattern).pitch(lane)))
}

pub fn show(app: &mut CantusApp, ui: &mut egui::Ui) {
    let Some((sel_track, sel_clip)) = app.selected_track.zip(app.selected_clip) else {
        ui.centered_and_justified(|ui| {
            ui.label(egui::RichText::new(t("clip.none_selected")).color(theme::p().text_dim));
        });
        return;
    };
    let Some(clip) = app
        .project()
        .track(sel_track)
        .and_then(|t| t.clip(sel_clip))
        .cloned()
    else {
        app.selected_clip = None;
        return;
    };
    let pattern = Pattern::of(&clip, app.rack.grid);

    toolbar(app, ui, &clip, pattern);
    if app.project().tracks.is_empty() {
        ui.centered_and_justified(|ui| {
            ui.label(egui::RichText::new(t("rack.empty")).color(theme::p().text_dim));
        });
        app.rack.geometry = None;
        return;
    }

    let full = ui.available_rect_before_wrap();
    let grid_rect = Rect::from_min_max(
        pos2(full.left() + CHANNELS_W, full.top() + HEADER_H),
        pos2(full.right(), full.bottom()),
    );
    let channels_rect = Rect::from_min_max(
        pos2(full.left(), grid_rect.top()),
        pos2(grid_rect.left(), full.bottom()),
    );
    let header = Rect::from_min_max(grid_rect.min - vec2(0.0, HEADER_H), grid_rect.right_top());

    let resp = ui.allocate_rect(grid_rect, Sense::click_and_drag());
    zoom_and_scroll(app, ui, &resp, grid_rect, pattern);
    if app.rack.fit {
        let fitted = grid_rect.width() / pattern.count.max(1) as f32;
        app.rack.step_w = fitted.clamp(MIN_STEP_W, MAX_STEP_W);
        app.rack.scroll_steps = 0.0;
    }

    let rows = layout_rows(app, &pattern, grid_rect.top(), app.rack.row_h);
    let geo = RackGeometry {
        grid_left: grid_rect.left(),
        grid_top: grid_rect.top(),
        step_w: app.rack.step_w,
        scroll_steps: app.rack.scroll_steps,
        steps: pattern.count,
        rows,
    };
    app.rack.geometry = Some(geo.clone());

    paint_grid(app, ui, &geo, grid_rect, header, pattern);
    channels(app, ui, channels_rect, &geo, pattern);
    interact(app, ui, &resp, &geo, pattern);
}

fn zoom_and_scroll(
    app: &mut CantusApp,
    ui: &egui::Ui,
    resp: &egui::Response,
    grid: Rect,
    pattern: Pattern,
) {
    if !resp.hovered() {
        return;
    }
    let (zoom, scroll, mods) = ui.input(|i| (i.zoom_delta(), i.smooth_scroll_delta, i.modifiers));
    let v = &mut app.rack;
    if zoom != 1.0 {
        v.fit = false;
        v.step_w = (v.step_w * zoom).clamp(MIN_STEP_W, MAX_STEP_W);
        v.row_h = (v.row_h * zoom).clamp(MIN_ROW_H, MAX_ROW_H);
    } else if scroll != egui::Vec2::ZERO && !mods.alt {
        let dx = if mods.shift { scroll.y } else { scroll.x };
        let visible = grid.width() / v.step_w;
        let max = (pattern.count as f32 - visible).max(0.0);
        v.scroll_steps = (v.scroll_steps - dx / v.step_w).clamp(0.0, max);
    }
}

/// How much detail a step cell is worth at its size on screen (ADR 0011).
///
/// Below the thresholds the cells stop being individually visible, so the empty ones stop
/// being drawn at all — the column shading behind them says the same thing for the price
/// of one rectangle per column — and a lit step becomes a plain quad instead of a
/// velocity-height bar inside a frame.
#[derive(Copy, Clone, Debug, PartialEq)]
struct StepLod {
    cells: bool,
    velocity: bool,
    gap: f32,
}

impl StepLod {
    const CELLS_ABOVE: f32 = 4.0;
    const VELOCITY_ABOVE_W: f32 = 7.0;
    const VELOCITY_ABOVE_H: f32 = 12.0;

    fn for_size(w: f32, h: f32) -> StepLod {
        StepLod {
            cells: w >= Self::CELLS_ABOVE && h >= Self::CELLS_ABOVE,
            velocity: w >= Self::VELOCITY_ABOVE_W && h >= Self::VELOCITY_ABOVE_H,
            gap: if w >= 8.0 { 1.5 } else { 0.0 },
        }
    }
}

fn paint_grid(
    app: &CantusApp,
    ui: &egui::Ui,
    geo: &RackGeometry,
    grid: Rect,
    header: Rect,
    pattern: Pattern,
) {
    let pal = theme::p();
    let p = ui.painter().with_clip_rect(grid);
    p.rect_filled(grid, 0.0, pal.ink);

    let first = geo.scroll_steps.floor().max(0.0) as usize;
    let visible = (grid.width() / geo.step_w).ceil() as usize + 2;
    let last = (first + visible).min(pattern.count);
    let ts = app.project().time_signature;
    let steps_per_beat = (ts.ticks_per_beat().0 / pattern.step.0).max(1) as usize;
    let steps_per_bar = (ts.ticks_per_bar().0 / pattern.step.0).max(1) as usize;

    // Column shading first: alternate beats, one rectangle per beat, which says where the
    // beat is at any zoom — and stands in for the cells once they are too small to draw.
    for step in first..last {
        let x = geo.grid_left + (step as f32 - geo.scroll_steps) * geo.step_w;
        let strong = step % steps_per_bar == 0;
        let beat = step % steps_per_beat == 0;
        if beat && (step / steps_per_beat) % 2 == 0 {
            let col = Rect::from_min_size(
                pos2(x, grid.top()),
                vec2(geo.step_w * steps_per_beat as f32, grid.height()),
            );
            p.rect_filled(col, 0.0, pal.row_white.gamma_multiply(0.35));
        }
        if strong {
            p.vline(x, grid.y_range(), Stroke::new(1.0, pal.grid_bar));
        } else if beat {
            p.vline(x, grid.y_range(), Stroke::new(1.0, pal.grid));
        }
    }

    // Every cell of the rows × steps grid goes into one mesh (ADR 0011).
    let row_h = geo.rows.first().map(|r| r.height).unwrap_or(MIN_ROW_H);
    let lod = StepLod::for_size(geo.step_w, row_h);
    let mut cells = egui::Mesh::default();
    let mut at = Vec::new();
    for (ri, row) in geo.rows.iter().enumerate() {
        if row.top > grid.bottom() {
            break;
        }
        let Some(pitch) = row_pitch(app, &pattern, row) else {
            // A kit's heading: a quiet band across the grid, no cells.
            let band =
                Rect::from_min_size(pos2(grid.left(), row.top), vec2(grid.width(), row.height));
            p.rect_filled(band, 0.0, pal.panel.gamma_multiply(0.6));
            continue;
        };
        if lod.cells {
            for step in first..last {
                let r = geo.cell(ri, step).shrink(lod.gap);
                cells.add_colored_rect(r, pal.panel.gamma_multiply(0.9));
            }
        }
        let Some(track) = app.project().tracks.get(row.track) else {
            continue;
        };
        let Some(clip) = pattern.clip_on(track) else {
            continue;
        };
        let color = theme::track_color(clip.color.unwrap_or(track.color));
        for n in &clip.notes {
            if pitch.is_some_and(|pp| pp != n.pitch) {
                continue;
            }
            clip.timeline_positions(n.start, &mut at);
            for on in at.iter().copied() {
                let Some(step) = pattern.index_of(on) else {
                    continue;
                };
                if step < first || step >= last {
                    continue;
                }
                let cell = geo.cell(ri, step).shrink(lod.gap);
                let fill = if n.muted || clip.muted {
                    pal.text_dim
                } else {
                    color
                };
                let lit = if lod.velocity {
                    // Velocity is the height of the lit part — the graph editor's first
                    // column, always on screen rather than behind a key.
                    let h = cell.height() * (0.28 + 0.72 * n.velocity as f32 / 127.0);
                    Rect::from_min_max(pos2(cell.left(), cell.bottom() - h), cell.max)
                } else {
                    cell
                };
                cells.add_colored_rect(lit, fill);
            }
        }
    }
    if !cells.is_empty() {
        p.add(egui::Shape::Mesh(std::sync::Arc::new(cells)));
    }

    // Row separators: one line per row, not one per cell; a track's row gets the strong
    // line so the tracks read as groups.
    for row in &geo.rows {
        if row.top > grid.bottom() {
            break;
        }
        let strong = row.kind == RowKind::Track;
        p.hline(
            grid.x_range(),
            row.top,
            Stroke::new(1.0, if strong { pal.grid_strong } else { pal.grid }),
        );
    }

    // The header numbers the bars, thinning out by doubling as the piano roll's ruler does.
    let hp = ui.painter().with_clip_rect(header);
    hp.rect_filled(header, 0.0, pal.panel);
    let mut bars_per_label = 1usize;
    while geo.step_w * ((steps_per_bar * bars_per_label) as f32) < 40.0 {
        bars_per_label *= 2;
    }
    let label_every = steps_per_bar * bars_per_label;
    for step in first..last {
        if step % label_every != 0 {
            continue;
        }
        let x = geo.grid_left + (step as f32 - geo.scroll_steps) * geo.step_w;
        let (bar, _, _) = ts.bbt(pattern.tick(step));
        hp.text(
            pos2(x + 3.0, header.center().y),
            Align2::LEFT_CENTER,
            bar.to_string(),
            design::paint::ruler(),
            pal.text_dim,
        );
    }
    hp.hline(
        header.x_range(),
        header.bottom() - 0.5,
        Stroke::new(1.0, pal.grid_strong),
    );

    // Playhead: which step is sounding. A repeating pattern folds the playhead into its
    // window, so the marker keeps running round while the song plays on.
    let pos = app.engine.position();
    let folded = if pattern.repeats {
        let d = (pos - pattern.start).0;
        (d >= 0).then(|| pattern.start + Ticks(d.rem_euclid(pattern.window().0.max(1))))
    } else {
        Some(pos)
    };
    if let Some(step) = folded.and_then(|t| pattern.index_of(t)) {
        let x = geo.grid_left + (step as f32 - geo.scroll_steps) * geo.step_w;
        let col = Rect::from_min_size(pos2(x, grid.top()), vec2(geo.step_w, grid.height()));
        p.rect_stroke(
            col,
            egui::CornerRadius::ZERO,
            Stroke::new(1.5, pal.gold_bright),
            StrokeKind::Inside,
        );
    }
}

/// The channel column: a row per track with its controls, and a row per pad under a kit.
fn channels(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    rect: Rect,
    geo: &RackGeometry,
    pattern: Pattern,
) {
    let pal = theme::p();
    ui.painter().rect_filled(rect, 0.0, pal.panel);
    let ids: Vec<TrackId> = app.project().tracks.iter().map(|t| t.id).collect();
    for (ri, row) in geo.rows.iter().enumerate() {
        if row.top > rect.bottom() {
            break;
        }
        let Some(id) = ids.get(row.track).copied() else {
            continue;
        };
        let strip = Rect::from_min_size(pos2(rect.left(), row.top), vec2(rect.width(), row.height));
        let selected_track = app.selected_track == Some(id);
        let bg = match row.kind {
            RowKind::Track if selected_track => pal.panel_raised,
            RowKind::Track => pal.panel,
            RowKind::Pad if ri % 2 == 1 => pal.row_black.gamma_multiply(0.5),
            RowKind::Pad => pal.panel,
        };
        ui.painter().rect_filled(strip, 0.0, bg);
        ui.painter().hline(
            strip.x_range(),
            strip.top(),
            Stroke::new(
                1.0,
                if row.kind == RowKind::Track {
                    pal.grid_strong
                } else {
                    pal.grid
                },
            ),
        );
        match row.kind {
            RowKind::Track => track_row(app, ui, strip, id, row.track, pattern, selected_track),
            RowKind::Pad => pad_row(app, ui, strip, id, row.lane.unwrap_or(0), pattern),
        }
    }
}

/// A track's row: colour, icon, the mixer strip it lands on, mute, name, level, pan.
///
/// These are the *track's* controls — `SetTrackMute`, `SetTrackGain`, `SetTrackPan` — the
/// same ones the mixer and the inspector edit, so what the rack says a channel does is
/// what the strip says it does.
fn track_row(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    strip: Rect,
    id: TrackId,
    index: usize,
    pattern: Pattern,
    selected: bool,
) {
    let pal = theme::p();
    let Some(track) = app.project().track(id).cloned() else {
        return;
    };
    let p = ui.painter();
    p.rect_filled(
        Rect::from_min_size(strip.min, vec2(5.0, strip.height())),
        0.0,
        theme::track_color(track.color),
    );
    let icon = Rect::from_center_size(
        pos2(strip.left() + 17.0, strip.center().y),
        vec2(14.0, 14.0),
    );
    icons::paint_track_icon(
        p,
        icon,
        track.icon(),
        if selected {
            pal.gold_bright
        } else {
            pal.text_dim
        },
    );
    // The mixer strip this channel lands on. Every track is its own strip today; when
    // buses arrive this is where the route will be chosen, so it is already where the eye
    // learns to look for it.
    let strip_no = Rect::from_center_size(
        pos2(strip.left() + 38.0, strip.center().y),
        vec2(20.0, 16.0),
    );
    p.rect_filled(
        strip_no,
        pal.corner_radius,
        pal.grid_strong.gamma_multiply(0.6),
    );
    p.text(
        strip_no.center(),
        Align2::CENTER_CENTER,
        (index + 1).to_string(),
        design::paint::ruler(),
        pal.text_dim,
    );

    let inner = Rect::from_min_max(
        pos2(strip.left() + 52.0, strip.top() + 1.0),
        pos2(strip.right() - space::S2, strip.bottom() - 1.0),
    );
    let mut child = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(inner)
            .layout(egui::Layout::left_to_right(egui::Align::Center)),
    );
    let ui = &mut child;
    ui.spacing_mut().item_spacing.x = space::S1;

    let m = ui.add(
        egui::Button::new(
            egui::RichText::new(t("track.mute"))
                .text_style(design::caption())
                .color(if track.muted {
                    pal.danger
                } else {
                    pal.text_dim
                }),
        )
        .min_size(vec2(18.0, 16.0))
        .fill(if track.muted {
            pal.danger.gamma_multiply(0.2)
        } else {
            egui::Color32::TRANSPARENT
        }),
    );
    if m.on_hover_text(t("track.mute_tip")).clicked() {
        app.exec(Command::SetTrackMute {
            id,
            muted: !track.muted,
        });
    }

    // The name: click to make this the selected track — which is also how the inspector
    // is pointed at its instrument and the piano roll at its clip.
    let name = ui.add(
        egui::Label::new(
            egui::RichText::new(&track.name)
                .text_style(design::strong())
                .color(if selected { pal.gold_bright } else { pal.text }),
        )
        .truncate()
        .sense(Sense::click()),
    );
    if name.on_hover_text(t("rack.channel_tip")).clicked() {
        focus_track(app, id, pattern, 0);
    }

    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
        let mut pan = track.pan;
        let r = ui.add(Bar::pan(&mut pan).size(40.0).name(t("track.pan")));
        if r.changed() {
            app.exec_merging(
                Command::SetTrackPan { id, pan },
                MergeKey::of(&("pan", id.0)),
            );
        }
        if r.drag_stopped() {
            app.end_gesture();
        }
        let mut gain = track.gain_db;
        let g = ui.add(
            Fader::new(&mut gain)
                .horizontal(64.0)
                .compact()
                .name(&track.name),
        );
        if g.changed() {
            app.exec_merging(
                Command::SetTrackGain { id, gain_db: gain },
                MergeKey::of(&("gain", id.0)),
            );
        }
        if g.drag_stopped() {
            app.end_gesture();
        }
    });
}

/// Make `id` the selected track, on the clip it has in the pattern's window, and point the
/// inspector at `pad`.
fn focus_track(app: &mut CantusApp, id: TrackId, pattern: Pattern, pad: usize) {
    let clip = app
        .project()
        .track(id)
        .and_then(|t| pattern.clip_on(t).map(|c| c.id));
    match clip {
        Some(c) => app.select_clip(id, c),
        None => app.select_track(id),
    }
    app.rack.selected_pad = pad;
    app.focus_piano_roll();
}

/// One pad of a kit with several: indented, with the pad's own mute, name, level and pan.
fn pad_row(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    strip: Rect,
    id: TrackId,
    lane: usize,
    pattern: Pattern,
) {
    let pal = theme::p();
    let Some(track) = app.project().track(id).cloned() else {
        return;
    };
    let axis = axis_of(&track, &pattern);
    let pitch = axis.pitch(lane);
    let pad_index = track
        .instrument
        .pulsus()
        .and_then(|k| k.index_of_pitch(pitch));
    let pad = pad_index.and_then(|i| track.instrument.pulsus().and_then(|k| k.pad(i)).cloned());
    let selected = app.selected_track == Some(id) && pad_index == Some(app.rack.selected_pad);

    let inner = Rect::from_min_max(
        pos2(strip.left() + PAD_INDENT, strip.top() + 1.0),
        pos2(strip.right() - space::S2, strip.bottom() - 1.0),
    );
    let mut child = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(inner)
            .layout(egui::Layout::left_to_right(egui::Align::Center)),
    );
    let ui = &mut child;
    ui.spacing_mut().item_spacing.x = space::S1;

    if let (Some(i), Some(p)) = (pad_index, pad.as_ref()) {
        let muted = p.muted;
        let r = ui.add(
            egui::Button::new(
                egui::RichText::new(t("track.mute"))
                    .text_style(design::caption())
                    .color(if muted { pal.danger } else { pal.text_dim }),
            )
            .min_size(vec2(18.0, 16.0))
            .fill(if muted {
                pal.danger.gamma_multiply(0.2)
            } else {
                egui::Color32::TRANSPARENT
            }),
        );
        if r.on_hover_text(t("pad.mute")).clicked() {
            set_pad(
                app,
                id,
                i,
                PadField::Mute,
                if muted { 0.0 } else { 1.0 },
                None,
            );
        }
    }

    // The name, renamed in place: a pad's name travels in the kit, so this renames the
    // piano roll's lane and every clip on the track at once.
    let renaming = app
        .rack
        .rename
        .as_ref()
        .is_some_and(|(t, i, _)| *t == id && Some(*i) == pad_index);
    if renaming {
        let mut done = None;
        if let Some((_, _, text)) = app.rack.rename.as_mut() {
            let edit = ui.add(
                egui::TextEdit::singleline(text)
                    .desired_width(96.0)
                    .font(design::strong()),
            );
            edit.request_focus();
            if edit.lost_focus() || ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                done = Some(text.clone());
            }
            if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                done = Some(String::new());
            }
        }
        if let Some(name) = done {
            app.rack.rename = None;
            if let Some(i) = pad_index {
                if !name.trim().is_empty() {
                    app.exec(Command::SetPadName {
                        track: id,
                        pad: i as u8,
                        name,
                    });
                }
            }
        }
    } else {
        let label = ui.add(
            egui::Label::new(
                egui::RichText::new(axis.name(lane))
                    .text_style(design::strong())
                    .color(if selected { pal.gold_bright } else { pal.text }),
            )
            .truncate()
            .sense(Sense::click()),
        );
        if label.clicked() {
            focus_track(app, id, pattern, pad_index.unwrap_or(0));
        }
        if label.double_clicked() {
            if let Some(i) = pad_index {
                app.rack.rename = Some((id, i, axis.name(lane)));
            }
        }
        label.on_hover_text(t("rack.pad_tip"));
    }

    let Some((i, p)) = pad_index.zip(pad) else {
        return;
    };
    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
        let mut pan = p.pan;
        let r = ui.add(Bar::pan(&mut pan).size(40.0).name(t("pad.pan")));
        if r.changed() {
            set_pad(app, id, i, PadField::Pan, pan, Some(("padpan", i)));
        }
        if r.drag_stopped() {
            app.end_gesture();
        }
        let info = PadField::Level.info(p.voice);
        let mut level = p.level_db;
        let r = ui.add(
            Bar::new(&mut level, info.min..=info.max, 0.0)
                .size(64.0)
                .name(t("pad.level")),
        );
        if r.changed() {
            set_pad(app, id, i, PadField::Level, level, Some(("padlevel", i)));
        }
        if r.drag_stopped() {
            app.end_gesture();
        }
    });
}

fn set_pad(
    app: &mut CantusApp,
    track: TrackId,
    pad: usize,
    field: PadField,
    value: f32,
    merge: Option<(&str, usize)>,
) {
    let cmd = Command::SetInstrumentParam {
        track,
        param: InstrumentParam::Pulsus {
            pad: pad as u8,
            field,
        },
        value,
    };
    match merge {
        Some(key) => {
            app.exec_merging(cmd, MergeKey::of(&(key.0, track.0, key.1)));
        }
        None => {
            app.exec(cmd);
        }
    }
}

fn interact(
    app: &mut CantusApp,
    ui: &egui::Ui,
    resp: &egui::Response,
    geo: &RackGeometry,
    pattern: Pattern,
) {
    let (mods, alt_wheel) = ui.input(|i| {
        (
            i.modifiers,
            if i.modifiers.alt {
                i.smooth_scroll_delta.y
            } else {
                0.0
            },
        )
    });
    let cell_at = |pos: egui::Pos2| -> Option<(usize, usize)> {
        let row = geo.row_at(pos.y)?;
        geo.rows[row].lane?;
        let step = geo.step_at(pos.x)?;
        Some((row, step))
    };

    // Alt + wheel over a step: velocity. The same gesture as the piano roll's, because it
    // is the same edit.
    if alt_wheel != 0.0 && resp.hovered() {
        if let Some((row, step)) = resp.hover_pos().and_then(cell_at) {
            let delta = (alt_wheel / 10.0).round().clamp(-16.0, 16.0) as i32;
            if delta != 0 {
                if let Some((track, clip, ids)) = step_notes(app, geo, &pattern, row, step) {
                    let notes: Vec<Note> = app
                        .project()
                        .track(track)
                        .and_then(|t| t.clip(clip))
                        .map(|c| {
                            c.notes
                                .iter()
                                .filter(|n| ids.contains(&n.id))
                                .map(|n| Note {
                                    velocity: (n.velocity as i32 + delta).clamp(1, 127) as u8,
                                    ..n.clone()
                                })
                                .collect()
                        })
                        .unwrap_or_default();
                    if let Some(first) = notes.first() {
                        app.rack.velocity = first.velocity;
                        app.exec_merging(
                            Command::SetNotes { track, clip, notes },
                            MergeKey::of(&("rackvel", clip.0)),
                        );
                    }
                }
            }
        }
    }

    if resp.clicked() || resp.secondary_clicked() {
        let on = !resp.secondary_clicked();
        if let Some((row, step)) = resp.interact_pointer_pos().and_then(cell_at) {
            app.focus_piano_roll();
            app.rack.stroke += 1;
            set_step(app, geo, pattern, row, step, on);
        }
    }

    // A stroke decides at the press whether it switches steps on or off, then paints that.
    if resp.drag_started_by(PointerButton::Primary)
        || resp.drag_started_by(PointerButton::Secondary)
    {
        let on = !mods.command && !resp.dragged_by(PointerButton::Secondary);
        app.rack.stroke += 1;
        app.rack.paint = Some((on, None));
    }
    if resp.dragged() {
        if let Some(pos) = resp.interact_pointer_pos() {
            if let (Some((on, last)), Some((row, step))) = (app.rack.paint, cell_at(pos)) {
                if last != Some((row, step)) {
                    app.rack.paint = Some((on, Some((row, step))));
                    set_step(app, geo, pattern, row, step, on);
                }
            }
        }
    }
    if resp.drag_stopped() {
        app.rack.paint = None;
        app.end_gesture();
    }
}

/// The notes a row's clip plays inside a step: `(track, clip, note ids)`, or `None` when
/// the track has no clip in the window.
fn step_notes(
    app: &CantusApp,
    geo: &RackGeometry,
    pattern: &Pattern,
    row: usize,
    step: usize,
) -> Option<(TrackId, ClipId, Vec<NoteId>)> {
    let r = geo.rows.get(row)?;
    let pitch = row_pitch(app, pattern, r)?;
    let track = app.project().tracks.get(r.track)?;
    let clip = pattern.clip_on(track)?;
    let (from, to) = (pattern.tick(step), pattern.tick(step + 1));
    let mut at = Vec::new();
    let ids = clip
        .notes
        .iter()
        .filter(|n| pitch.is_none_or(|p| p == n.pitch))
        .filter(|n| {
            clip.timeline_positions(n.start, &mut at);
            at.iter().any(|on| *on >= from && *on < to)
        })
        .map(|n| n.id)
        .collect();
    Some((track.id, clip.id, ids))
}

/// Switch a step on or off. **This is `InsertNotes` / `RemoveNotes` and nothing else** —
/// the rack has no data model of its own to keep in step with the piano roll's.
///
/// On a track with no clip in the window, switching a step on first creates one — the
/// pattern's size, repeating if the pattern does — in the same undo step, so the first
/// hit on a fresh channel is one gesture and one `Ctrl+Z`.
fn set_step(
    app: &mut CantusApp,
    geo: &RackGeometry,
    pattern: Pattern,
    row: usize,
    step: usize,
    on: bool,
) {
    let Some(r) = geo.rows.get(row).copied() else {
        return;
    };
    let Some(pitch) = row_pitch(app, &pattern, &r) else {
        return;
    };
    let Some(track) = app.project().tracks.get(r.track).cloned() else {
        return;
    };
    let key = MergeKey::of(&("rack", app.rack.stroke));
    let existing = step_notes(app, geo, &pattern, row, step);

    if !on {
        let Some((tid, cid, ids)) = existing else {
            return;
        };
        if ids.is_empty() {
            return;
        }
        app.exec_merging(
            Command::Batch {
                label_key: "cmd.step_off".into(),
                commands: vec![Command::RemoveNotes {
                    track: tid,
                    clip: cid,
                    ids,
                }],
            },
            key,
        );
        return;
    }

    if existing.as_ref().is_some_and(|(_, _, ids)| !ids.is_empty()) {
        return;
    }
    // A melodic row adds what the clip already plays — its lowest note, so a bass line
    // stays a bass line — and middle C otherwise.
    let pitch = match pitch {
        Some(p) => p,
        None => pattern
            .clip_on(&track)
            .and_then(|c| c.notes.iter().map(|n| n.pitch).min())
            .unwrap_or(MELODIC_STEP_PITCH),
    };
    // A step is a hit, so its length is a short mark rather than the grid.
    let length = Ticks::SIXTEENTH.min(pattern.step);
    let velocity = app.rack.velocity;
    let mut commands = Vec::with_capacity(2);
    let (clip_id, content) = match existing {
        Some((_, cid, _)) => {
            let content = track
                .clip(cid)
                .map(|c| c.content_of(pattern.tick(step)))
                .unwrap_or(pattern.tick(step) - pattern.start);
            (cid, content)
        }
        None => {
            let mut clip = app
                .bus
                .new_clip(track.name.clone(), pattern.start, pattern.window());
            if pattern.repeats {
                clip.content_length = Some(pattern.window());
            }
            let cid = clip.id;
            commands.push(Command::InsertClip {
                track: track.id,
                clip: Box::new(clip),
            });
            // A fresh clip's content starts at zero at its own start.
            (cid, pattern.tick(step) - pattern.start)
        }
    };
    // The note goes where the step *sounds*: the clip's content tick for that point on
    // the timeline, which is the mapping the engine plays back.
    let note = app.bus.new_note(content, length, pitch, velocity);
    commands.push(Command::InsertNotes {
        track: track.id,
        clip: clip_id,
        notes: vec![note],
    });
    app.exec_merging(
        Command::Batch {
            label_key: "cmd.step_on".into(),
            commands,
        },
        key,
    );
}

fn toolbar(app: &mut CantusApp, ui: &mut egui::Ui, clip: &Clip, pattern: Pattern) {
    ui.horizontal(|ui| {
        ui.label(
            egui::RichText::new(t("rack.title"))
                .strong()
                .color(theme::p().gold),
        );
        // Which bars the rack is editing, taken from the selected clip.
        let ts = app.project().time_signature;
        let (from, _, _) = ts.bbt(pattern.start);
        let (to, _, _) = ts.bbt(pattern.start + pattern.window() - Ticks(1));
        ui.label(
            egui::RichText::new(tf(
                "rack.pattern",
                &[("clip", &clip.name), ("from", &from), ("to", &to)],
            ))
            .text_style(design::mono_small())
            .color(theme::p().text_dim),
        );
        ui.separator();
        ui.label(egui::RichText::new(t("arrangement.grid")).text_style(design::caption()));
        let current = GRIDS
            .iter()
            .find(|(_, g)| *g == app.rack.grid.0)
            .map(|(k, _)| t(k))
            .unwrap_or_default();
        egui::ComboBox::from_id_salt("rack-grid")
            .selected_text(current)
            .width(70.0)
            .show_ui(ui, |ui| {
                for (key, g) in GRIDS {
                    if ui.selectable_label(app.rack.grid.0 == g, t(key)).clicked() {
                        app.rack.grid = Ticks(g);
                        app.rack.scroll_steps = 0.0;
                        app.rack.fit = true;
                    }
                }
            });
        ui.separator();
        ui.label(egui::RichText::new(t("rack.velocity")).text_style(design::caption()));
        let mut vel = app.rack.velocity as f32;
        let r = ui.add(
            Bar::new(&mut vel, 1.0..=127.0, 1.0)
                .size(80.0)
                .name(t("rack.velocity")),
        );
        if r.changed() {
            app.rack.velocity = vel.round().clamp(1.0, 127.0) as u8;
        }
        ui.label(
            egui::RichText::new(format!("{:>3}", app.rack.velocity))
                .text_style(design::mono_small())
                .color(theme::p().text_dim),
        );
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.label(
                egui::RichText::new(tf(
                    "rack.counts",
                    &[
                        ("pads", &app.project().tracks.len()),
                        ("steps", &pattern.count),
                    ],
                ))
                .text_style(design::caption())
                .color(theme::p().text_dim),
            );
        });
    });
}
