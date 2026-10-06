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
//! cells go into one mesh with a level-of-detail rule, not one painter call each.
//!
//! ## Drawn as FL's rack (ADR 0024)
//!
//! Each row is FL's: the enable lamp, pan and level knobs, the route number in a sunken
//! well, the channel button (neutral at rest, the channel's colour when selected) and the
//! selector — and the steps are FL's keycaps, grey and maroon by beat, lit pale blue and
//! pink. The rack scrolls both ways, Shift+wheel is time, Ctrl+wheel zooms the steps and
//! Ctrl+Alt+wheel the rows, the middle button pans, and both bars are always there.

use super::clips_chrome::{self as chrome, Span, Toolbar, BAR};
use super::GRIDS;
use crate::app::CantusApp;
use crate::i18n::{t, tf};
use crate::kit::hint::{self, Caps};
use crate::kit::knob::KnobSize;
use crate::kit::text::Src;
use crate::kit::{self, Style};
use crate::theme;
use crate::views::piano_roll::Axis;
use cantus_core::{
    Clip, ClipId, Command, InstrumentParam, MergeKey, Note, NoteId, PadField, Pitch, Ticks, TrackId,
};
use egui::{pos2, vec2, PointerButton, Rect, Sense};

#[path = "channel_rack_paint.rs"]
mod paint;

/// Width of the channel column: the lamp, two knobs, the route well, the channel button
/// and the selector — none of them a four-pixel target.
const CHANNELS_W: f32 = 268.0;
const HEADER_H: f32 = 20.0;
const MIN_STEP_W: f32 = 3.0;
const MAX_STEP_W: f32 = 64.0;
const MIN_ROW_H: f32 = 16.0;
const MAX_ROW_H: f32 = 44.0;
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
    /// How far the rows are scrolled up, in points.
    pub scroll_y: f32,
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
    /// Buffers the painters reuse, so a steady frame allocates nothing.
    scratch: Scratch,
}

#[derive(Debug, Default)]
struct Scratch {
    at: Vec<Ticks>,
    tints: chrome::Tints,
    /// The toolbar's readouts, rebuilt when what they say changes.
    counts: ((usize, usize), String),
    span: ((i64, i64, usize), String),
    /// The tracks' axes for the pattern, and what they were built for.
    axes: Vec<Axis>,
    axes_key: Option<(u64, i64, i64, usize, u64)>,
}

impl Default for ChannelRackView {
    fn default() -> Self {
        ChannelRackView {
            step_w: 26.0,
            row_h: 30.0,
            scroll_steps: 0.0,
            scroll_y: 0.0,
            grid: Ticks::SIXTEENTH,
            velocity: 100,
            selected_pad: 0,
            fit: true,
            stroke: 0,
            paint: None,
            rename: None,
            geometry: None,
            scratch: Scratch::default(),
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
        // Note clips on the playing lane only: an audio clip or a take has no steps.
        let notes = |c: &&'a Clip| c.audio.is_none() && c.lane == 0;
        track
            .clips
            .iter()
            .filter(notes)
            .find(|c| c.start <= self.start && c.end() > self.start)
            .or_else(|| {
                track
                    .clips
                    .iter()
                    .filter(notes)
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
        audio: None,
        lane: 0,
    }
}

/// The rows the rack lays out for a project: one per track, plus one per pad for a kit
/// with several. Pure, so the layout and the tests agree by construction.
fn layout_rows(rows: &mut Vec<RackRow>, app: &CantusApp, axes: &[Axis], top: f32, row_h: f32) {
    rows.clear();
    let mut y = top;
    for (i, (track, axis)) in app.project().tracks.iter().zip(axes).enumerate() {
        let pads = !track.instrument.lanes().is_chromatic();
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

/// Where the rack's regions are this frame.
#[derive(Copy, Clone, Debug)]
struct Frame {
    channels: Rect,
    header: Rect,
    grid: Rect,
    hbar: Rect,
    vbar: Rect,
    /// The corners around the bars and over the channel column.
    corners: [Rect; 4],
}

impl Frame {
    fn of(full: Rect, s: &Style) -> Frame {
        let bar = s.s(BAR);
        let cw = s.s(CHANNELS_W).min(full.width() * 0.5);
        let (l, r) = (full.left(), full.right() - bar);
        let (top, bottom) = (full.top() + HEADER_H, full.bottom() - bar);
        let gx = l + cw;
        Frame {
            channels: Rect::from_min_max(pos2(l, top), pos2(gx, bottom)),
            header: Rect::from_min_max(pos2(gx, full.top()), pos2(r, top)),
            grid: Rect::from_min_max(pos2(gx, top), pos2(r, bottom)),
            hbar: Rect::from_min_max(pos2(gx, bottom), pos2(r, full.bottom())),
            vbar: Rect::from_min_max(pos2(r, top), pos2(full.right(), bottom)),
            corners: [
                Rect::from_min_max(full.min, pos2(gx, top)),
                Rect::from_min_max(pos2(r, full.top()), pos2(full.right(), top)),
                Rect::from_min_max(pos2(r, bottom), full.max),
                Rect::from_min_max(pos2(l, bottom), pos2(gx, full.bottom())),
            ],
        }
    }
}

/// An empty rack's one line, in a box of its own.
fn empty(ui: &mut egui::Ui, key: &str, s: &Style) {
    let r = ui.available_rect_before_wrap();
    kit::with_brush(ui.painter(), |b| {
        b.register(r, "empty", &[kit::Tok::PanelLight, kit::Tok::Text], &[]);
        b.rect(b.snap_rect(r), kit::Fill::Solid(s.t.panel));
        let tx = kit::text::Text::key(key, kit::text::Role::Title, s.t.text_dim)
            .policy(kit::text::Policy::Elide(kit::text::Elide::End));
        let _ = kit::text::paint(b, r.shrink(16.0), &tx, s.tk());
    });
}

pub fn show(app: &mut CantusApp, ui: &mut egui::Ui) {
    let s = Style::current();
    let Some((sel_track, sel_clip)) = app.selected_track.zip(app.selected_clip) else {
        empty(ui, "clip.none_selected", &s);
        return;
    };
    let Some(pattern) = app
        .project()
        .track(sel_track)
        .and_then(|t| t.clip(sel_clip))
        .map(|c| Pattern::of(c, app.rack.grid))
    else {
        app.selected_clip = None;
        return;
    };
    toolbar(app, ui, sel_track, sel_clip, pattern);
    if app.project().tracks.is_empty() {
        empty(ui, "rack.empty", &s);
        app.rack.geometry = None;
        return;
    }
    let full = ui.available_rect_before_wrap();
    if full.width() < 120.0 || full.height() < HEADER_H + 2.0 * s.s(BAR) + 8.0 {
        return;
    }
    let f = Frame::of(full, &s);
    let resp = ui.allocate_rect(f.grid, Sense::click_and_drag());
    let mut scratch = std::mem::take(&mut app.rack.scratch);
    refresh_axes(app, &pattern, &mut scratch);
    let n_rows = count_rows(app, &scratch.axes);
    navigate(app, ui, &f, pattern, n_rows);

    let mut geo = app.rack.geometry.take().unwrap_or(RackGeometry {
        grid_left: 0.0,
        grid_top: 0.0,
        step_w: 1.0,
        scroll_steps: 0.0,
        steps: 0,
        rows: Vec::new(),
    });
    geo.grid_left = f.grid.left();
    geo.grid_top = f.grid.top();
    geo.step_w = app.rack.step_w;
    geo.scroll_steps = app.rack.scroll_steps;
    geo.steps = pattern.count;
    let top = f.grid.top() - app.rack.scroll_y;
    layout_rows(&mut geo.rows, app, &scratch.axes, top, app.rack.row_h);
    {
        let a: &CantusApp = app;
        // The playhead's step, folded into the pattern when it repeats.
        let pos = a.engine.position();
        let folded = if pattern.repeats {
            let d = (pos - pattern.start).0;
            (d >= 0).then(|| pattern.start + Ticks(d.rem_euclid(pattern.window().0.max(1))))
        } else {
            Some(pos)
        };
        let playing = folded.and_then(|t| pattern.index_of(t));
        kit::with_brush(&ui.painter().with_clip_rect(f.grid), |b| {
            paint::grid(
                b,
                a,
                &geo,
                f.grid,
                &pattern,
                &scratch.axes,
                playing,
                &mut scratch.at,
                &s,
            )
        });
        kit::with_brush(&ui.painter().with_clip_rect(f.header), |b| {
            paint::header(b, a, &geo, f.header, &pattern, playing, &s)
        });
        kit::with_brush(ui.painter(), |b| {
            for c in f.corners {
                chrome::corner(b, c, &s);
            }
        });
    }
    app.rack.scratch = scratch;
    channels(app, ui, &f, &geo, pattern, &s);
    interact(app, ui, &resp, &geo, pattern);
    if resp.hovered() {
        hint::set_text(ui.ctx(), chrome::word("rack.hint"), "", Caps::NONE);
    }
    app.rack.geometry = Some(geo);
}

/// The rows' full height.
fn content_height(geo_rows: usize, row_h: f32) -> f32 {
    geo_rows as f32 * row_h
}

/// The wheel, the middle button and the two bars.
fn navigate(app: &mut CantusApp, ui: &mut egui::Ui, f: &Frame, pattern: Pattern, n_rows: usize) {
    let area = Rect::from_min_max(pos2(f.channels.left(), f.header.top()), f.grid.max);
    let over = ui.rect_contains_pointer(area);
    let pointer = ui.input(|i| i.pointer.hover_pos());
    let v = &mut app.rack;
    if v.fit {
        let fitted = f.grid.width() / pattern.count.max(1) as f32;
        v.step_w = fitted.clamp(MIN_STEP_W, MAX_STEP_W);
        v.scroll_steps = 0.0;
    }
    if over {
        let w = chrome::wheel(ui);
        if w.zoom_x != 1.0 {
            let at_x = pointer
                .map(|p| p.x.max(f.grid.left()))
                .unwrap_or(f.grid.left());
            let anchor = (at_x - f.grid.left()) / v.step_w + v.scroll_steps;
            v.fit = false;
            v.step_w = (v.step_w * w.zoom_x).clamp(MIN_STEP_W, MAX_STEP_W);
            v.scroll_steps = (anchor - (at_x - f.grid.left()) / v.step_w).max(0.0);
        }
        if w.zoom_y != 1.0 {
            let at_y = pointer.map(|p| p.y).unwrap_or(f.grid.top()) - f.grid.top();
            let before = v.row_h;
            v.row_h = (v.row_h * w.zoom_y).clamp(MIN_ROW_H, MAX_ROW_H);
            v.scroll_y = ((v.scroll_y + at_y) * v.row_h / before - at_y).max(0.0);
        }
        // Alt+wheel over a step is velocity, handled with the steps.
        if !w.alt && w.scroll != egui::Vec2::ZERO {
            // Shift+wheel arrives as `x`: there is nothing to ask about Shift.
            if w.scroll.x != 0.0 {
                v.fit = false;
                v.scroll_steps -= w.scroll.x / v.step_w;
            }
            v.scroll_y -= w.scroll.y;
        }
    }
    let (middle, origin, delta) = ui.input(|i| {
        (
            i.pointer.middle_down(),
            i.pointer.press_origin(),
            i.pointer.delta(),
        )
    });
    if middle && origin.is_some_and(|o| area.contains(o)) && delta != egui::Vec2::ZERO {
        ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
        if delta.x != 0.0 {
            v.fit = false;
            v.scroll_steps -= delta.x / v.step_w;
        }
        v.scroll_y -= delta.y;
    }
    let count = pattern.count as f32;
    let visible = f.grid.width() / v.step_w;
    v.scroll_steps = v.scroll_steps.clamp(0.0, (count - visible).max(0.0));
    let span = Span::new(v.scroll_steps, visible.min(count), count);
    let zoom = Some((f.grid.width() / MAX_STEP_W, f.grid.width() / MIN_STEP_W));
    if let Some(n) = chrome::bar(
        ui,
        f.hbar,
        ui.id().with("rack-hbar"),
        span,
        zoom,
        "views.nav.hbar",
    ) {
        v.fit = false;
        v.step_w = (f.grid.width() / n.len).clamp(MIN_STEP_W, MAX_STEP_W);
        v.scroll_steps = n.start.max(0.0);
    }
    let content = content_height(n_rows, v.row_h);
    let span = Span::new(v.scroll_y.max(0.0), f.grid.height(), content + v.row_h);
    let zoom = Some((
        f.grid.height() * v.row_h / MAX_ROW_H,
        f.grid.height() * v.row_h / MIN_ROW_H,
    ));
    if let Some(n) = chrome::bar(
        ui,
        f.vbar,
        ui.id().with("rack-vbar"),
        span,
        zoom,
        "views.nav.vbar",
    ) {
        let before = v.row_h;
        v.row_h = (v.row_h * span.len / n.len.max(1.0)).clamp(MIN_ROW_H, MAX_ROW_H);
        v.scroll_y = n.start * v.row_h / before;
    }
    let content = content_height(n_rows, v.row_h);
    v.scroll_y = v
        .scroll_y
        .clamp(0.0, (content + v.row_h - f.grid.height()).max(0.0));
}

/// How many rows the rack lays out, without laying them out.
fn count_rows(app: &CantusApp, axes: &[Axis]) -> usize {
    app.project()
        .tracks
        .iter()
        .zip(axes)
        .map(|(t, axis)| {
            if t.instrument.lanes().is_chromatic() || axis.len() == 1 {
                1
            } else {
                1 + axis.len()
            }
        })
        .sum()
}

/// The tracks' axes for this pattern, rebuilt when the project or the pattern changes
/// rather than every frame: a kit's axis names every one of its pads.
fn refresh_axes(app: &CantusApp, pattern: &Pattern, scratch: &mut Scratch) {
    let p = app.project();
    let key = (
        app.bus.revision(),
        pattern.start.0,
        pattern.step.0,
        pattern.count,
        p.tracks.first().map_or(0, |t| t.id.0) ^ p.tracks.len() as u64,
    );
    if scratch.axes_key == Some(key) && scratch.axes.len() == p.tracks.len() {
        return;
    }
    scratch.axes.clear();
    scratch
        .axes
        .extend(p.tracks.iter().map(|t| axis_of(t, pattern)));
    scratch.axes_key = Some(key);
}

/// The channel column: a row per track with its controls, and a row per pad under a kit.
fn channels(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    f: &Frame,
    geo: &RackGeometry,
    pattern: Pattern,
    s: &Style,
) {
    let clip = f.channels;
    let p = ui.painter().with_clip_rect(clip);
    let visible = |r: &RackRow| r.top + r.height > clip.top() && r.top < clip.bottom();
    let strip_of =
        |r: &RackRow| Rect::from_min_size(pos2(clip.left(), r.top), vec2(clip.width(), r.height));
    let pointer = ui
        .input(|i| i.pointer.hover_pos())
        .filter(|_| ui.rect_contains_pointer(clip));
    let mut scratch = std::mem::take(&mut app.rack.scratch);
    {
        let a: &CantusApp = app;
        kit::with_brush(&p, |b| {
            b.register(clip, "rack_channels", &[kit::Tok::PanelLight], &[]);
            b.rect(b.snap_rect(clip), kit::Fill::Solid(s.t.panel_light));
            for row in geo.rows.iter().filter(|r| visible(r)) {
                let Some(track) = a.project().tracks.get(row.track) else {
                    continue;
                };
                let Some(axis) = scratch.axes.get(row.track) else {
                    continue;
                };
                let rr = paint::row_rects(strip_of(row), row.kind, s);
                let tint = scratch.tints.of(theme::track_color(track.color), s);
                let mut digits = chrome::Digits::default();
                let hovered = pointer.is_some_and(|p| rr.button.contains(p));
                let look = match row.kind {
                    RowKind::Track => paint::RowLook {
                        name: &track.name,
                        route: Some(
                            match track.strip.and_then(|s| a.project().strip_number(s)) {
                                Some(n) => digits.fmt(n as i64),
                                None => "M",
                            },
                        ),
                        on: !track.muted,
                        selected: a.selected_track == Some(track.id)
                            && (row.lane.is_some() || axis.len() != 1),
                        tint,
                        button_hovered: hovered,
                    },
                    RowKind::Pad => {
                        let lane = row.lane.unwrap_or(0);
                        let kit_pads = track.instrument.pulsus();
                        let index = kit_pads.and_then(|k| k.index_of_pitch(axis.pitch(lane)));
                        let pad = index.and_then(|i| kit_pads.and_then(|k| k.pad(i)));
                        paint::RowLook {
                            name: axis.name_ref(lane),
                            route: None,
                            on: !pad.is_some_and(|p| p.muted),
                            selected: a.selected_track == Some(track.id)
                                && index == Some(a.rack.selected_pad),
                            tint,
                            button_hovered: hovered,
                        }
                    }
                };
                let renaming =
                    row.kind == RowKind::Pad
                        && a.rack.rename.as_ref().is_some_and(|(t, i, _)| {
                            *t == track.id
                                && track.instrument.pulsus().and_then(|k| {
                                    k.index_of_pitch(axis.pitch(row.lane.unwrap_or(0)))
                                }) == Some(*i)
                        });
                let look = if renaming {
                    paint::RowLook { name: "", ..look }
                } else {
                    look
                };
                paint::row(b, &rr, &look, s);
            }
        });
    }
    app.rack.scratch = scratch;

    for row in geo.rows.iter().filter(|r| visible(r)) {
        let Some(id) = app.project().tracks.get(row.track).map(|t| t.id) else {
            continue;
        };
        let rr = paint::row_rects(strip_of(row), row.kind, s);
        match row.kind {
            RowKind::Track => track_controls(app, ui, &rr, id, pattern),
            RowKind::Pad => {
                pad_controls(app, ui, &rr, id, row.track, row.lane.unwrap_or(0), pattern)
            }
        }
    }
}

/// A knob in a rack row, placed in `rect`: FL's small flat knob.
#[allow(clippy::too_many_arguments)]
fn row_knob(
    ui: &mut egui::Ui,
    rect: Rect,
    salt: impl std::hash::Hash + std::fmt::Debug,
    value: &mut f32,
    range: std::ops::RangeInclusive<f32>,
    default: f32,
    name: &'static str,
    unit: Unit,
) -> egui::Response {
    let mut child = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(rect)
            .id_salt(salt)
            .layout(egui::Layout::left_to_right(egui::Align::Center)),
    );
    let fmt = move |v: f32, b: &mut String| {
        use std::fmt::Write as _;
        let _ = match unit {
            Unit::Db => write!(b, "{v:+.1} dB"),
            Unit::Plain => write!(b, "{v:.0}"),
            Unit::Pan => {
                if v.abs() < 0.005 {
                    write!(b, "C")
                } else if v < 0.0 {
                    write!(b, "L{:.0}", -v * 100.0)
                } else {
                    write!(b, "R{:.0}", v * 100.0)
                }
            }
        };
    };
    kit::widgets::Param::new(value, range, default)
        .size(KnobSize::Mini)
        .name(name)
        .format(fmt)
        .show(&mut child)
}

/// What a row knob's value is in.
#[derive(Copy, Clone, Debug)]
enum Unit {
    Db,
    Pan,
    Plain,
}

/// A track row's controls: the lamp (the track's mute), pan and level (the track's own,
/// the same the mixer and the inspector edit), the route well, the channel button.
fn track_controls(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    rr: &paint::RowRects,
    id: TrackId,
    pattern: Pattern,
) {
    let Some((muted, mut pan, mut gain, strip)) = app
        .project()
        .track(id)
        .map(|t| (t.muted, t.pan, t.gain_db, t.strip))
    else {
        return;
    };
    let led = ui.interact(
        Rect::from_center_size(rr.led, vec2(16.0, 16.0)),
        ui.id().with(("rack-led", id.0)),
        Sense::click(),
    );
    if led.hovered() {
        hint::set_text(ui.ctx(), chrome::word("track.mute_tip"), "", Caps::NONE);
    }
    if led.clicked() {
        app.exec(Command::SetTrackMute { id, muted: !muted });
    }
    let r = row_knob(
        ui,
        rr.pan,
        ("rack-pan", id.0),
        &mut pan,
        -1.0..=1.0,
        0.0,
        chrome::word("track.pan"),
        Unit::Pan,
    );
    if r.changed() {
        app.exec_merging(
            Command::SetTrackPan { id, pan },
            MergeKey::of(&("pan", id.0)),
        );
    }
    if r.drag_stopped() {
        app.end_gesture();
    }
    let g = row_knob(
        ui,
        rr.vol,
        ("rack-vol", id.0),
        &mut gain,
        cantus_core::MIN_GAIN_DB..=cantus_core::MAX_GAIN_DB,
        0.0,
        chrome::word("track.gain"),
        Unit::Db,
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
    // The strip this channel feeds, and the place to change it: click, and the route
    // picker opens.
    if let Some(route) = rr.route {
        let resp = ui.interact(route, ui.id().with(("route", id.0)), Sense::click());
        if resp.hovered() {
            hint::set_text(
                ui.ctx(),
                chrome::word("track.route_tip"),
                "",
                Caps {
                    menu: true,
                    ..Caps::NONE
                },
            );
        }
        egui::Popup::menu(&resp).show(|ui| {
            crate::views::inspector::route_picker(app, ui, id, strip);
        });
    }
    // The name: click to make this the selected track — which is also how the inspector
    // is pointed at its instrument and the piano roll at its clip.
    let button = ui.interact(
        rr.button.union(rr.selector),
        ui.id().with(("rack-button", id.0)),
        Sense::click(),
    );
    if button.hovered() {
        // Round 20: what the button does now (the selector beside it selects).
        let tip = crate::i18n::t("swap.rack.channel_tip");
        hint::set_text(
            ui.ctx(),
            &tip,
            "",
            Caps {
                menu: true,
                ..Caps::NONE
            },
        );
    }
    // Round 20, as FL: a click opens or closes the channel's instrument; a right-click,
    // or the swap badge that shows on the button, swaps it for another.
    let swap = crate::picker::PickPurpose::ReplaceInstrument { track: id };
    crate::views::device_chrome::publish(
        ui.ctx(),
        crate::views::device_chrome::Spot::RackButton(id),
        rr.button,
    );
    let badge = kit::with_brush(&ui.painter().clone(), |b| {
        crate::views::device_chrome::swap_badge(
            app,
            ui,
            b,
            rr.button,
            ui.id().with(("rack-swap", id.0)),
            1.0,
            swap.clone(),
            "swap.tip.instrument",
        )
    });
    if button.clicked() && !badge {
        focus_track(app, id, pattern, 0);
        // The selector beside the button only selects (FL).
        let on_button = button
            .interact_pointer_pos()
            .is_some_and(|p| rr.button.contains(p));
        if on_button && app.instrument_window_key(id).is_some() {
            app.toggle_instrument_window_of(id);
        }
    }
    if button.secondary_clicked() {
        app.select_track(id);
        app.open_picker(swap);
    }
}

/// One pad of a kit with several: its own mute lamp, pan and level, and the name, renamed
/// in place (the name travels in the kit, so this renames the piano roll's lane too).
#[allow(clippy::too_many_arguments)]
fn pad_controls(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    rr: &paint::RowRects,
    id: TrackId,
    track_index: usize,
    lane: usize,
    pattern: Pattern,
) {
    let Some(pitch) = app
        .rack
        .scratch
        .axes
        .get(track_index)
        .map(|a| a.pitch(lane))
    else {
        return;
    };
    let Some((i, p)) = app.project().track(id).and_then(|t| {
        let k = t.instrument.pulsus()?;
        let i = k.index_of_pitch(pitch)?;
        Some((i, k.pad(i)?.clone()))
    }) else {
        return;
    };
    let led = ui.interact(
        Rect::from_center_size(rr.led, vec2(16.0, 16.0)),
        ui.id().with(("rack-pad-led", id.0, i)),
        Sense::click(),
    );
    if led.hovered() {
        hint::set_text(ui.ctx(), chrome::word("pad.mute"), "", Caps::NONE);
    }
    if led.clicked() {
        set_pad(
            app,
            id,
            i,
            PadField::Mute,
            if p.muted { 0.0 } else { 1.0 },
            None,
        );
    }
    let mut pan = p.pan;
    let r = row_knob(
        ui,
        rr.pan,
        ("rack-pad-pan", id.0, i),
        &mut pan,
        -1.0..=1.0,
        0.0,
        chrome::word("pad.pan"),
        Unit::Pan,
    );
    if r.changed() {
        set_pad(app, id, i, PadField::Pan, pan, Some(("padpan", i)));
    }
    if r.drag_stopped() {
        app.end_gesture();
    }
    let info = PadField::Level.info(p.voice);
    let mut level = p.level_db;
    let r = row_knob(
        ui,
        rr.vol,
        ("rack-pad-level", id.0, i),
        &mut level,
        info.min..=info.max,
        0.0,
        chrome::word("pad.level"),
        Unit::Db,
    );
    if r.changed() {
        set_pad(app, id, i, PadField::Level, level, Some(("padlevel", i)));
    }
    if r.drag_stopped() {
        app.end_gesture();
    }

    let renaming = app
        .rack
        .rename
        .as_ref()
        .is_some_and(|(t, pi, _)| *t == id && *pi == i);
    if renaming {
        let mut done = None;
        let mut child = ui.new_child(
            egui::UiBuilder::new()
                .max_rect(rr.button.shrink(2.0))
                .layout(egui::Layout::left_to_right(egui::Align::Center)),
        );
        if let Some((_, _, text)) = app.rack.rename.as_mut() {
            let edit =
                child.add(egui::TextEdit::singleline(text).desired_width(rr.button.width() - 8.0));
            edit.request_focus();
            if edit.lost_focus() || child.input(|i| i.key_pressed(egui::Key::Enter)) {
                done = Some(text.clone());
            }
            if child.input(|i| i.key_pressed(egui::Key::Escape)) {
                done = Some(String::new());
            }
        }
        if let Some(name) = done {
            app.rack.rename = None;
            if !name.trim().is_empty() {
                app.exec(Command::SetPadName {
                    track: id,
                    pad: p.id,
                    name,
                });
            }
        }
        return;
    }
    let button = ui.interact(
        rr.button.union(rr.selector),
        ui.id().with(("rack-pad-button", id.0, i)),
        Sense::click(),
    );
    if button.hovered() {
        hint::set_text(ui.ctx(), chrome::word("rack.pad_tip"), "", Caps::NONE);
    }
    if button.clicked() {
        focus_track(app, id, pattern, i);
    }
    if button.double_clicked() {
        app.rack.rename = Some((id, i, p.name.clone()));
    }
}

/// The toolbar: ▸, the clip and the bars the rack is editing, the grid, the velocity a
/// new step gets, and at the right end the counts and the fit key.
fn toolbar(app: &mut CantusApp, ui: &mut egui::Ui, track: TrackId, clip: ClipId, pattern: Pattern) {
    let id = ui.id().with("rack-toolbar");
    let mut bar = Toolbar::new(ui);
    if let Some(r) = bar.menu(ui, id.with("menu"), "rack.menu") {
        egui::Popup::menu(&r).frame(egui::Frame::NONE).show(|ui| {
            let items = [kit::menu::Item::action(Src::Key("rack.fit"))];
            if kit::widgets::menu_list(ui, &items, 220.0)
                .1
                .clicked
                .is_some()
            {
                app.rack.fit = true;
                ui.close();
            }
        });
    }
    let ts = app.project().time_signature;
    let (from, _, _) = ts.bbt(pattern.start);
    let (to, _, _) = ts.bbt(pattern.start + pattern.window() - Ticks(1));
    let key = (from, to, pattern.count);
    if app.rack.scratch.span.0 != key || app.rack.scratch.span.1.is_empty() {
        app.rack.scratch.span = (key, tf("rack.bars", &[("from", &from), ("to", &to)]));
    }
    {
        let a: &CantusApp = app;
        let name = a
            .project()
            .track(track)
            .and_then(|t| t.clip(clip))
            .map(|c| c.name.as_str())
            .unwrap_or("");
        bar.title(ui, Src::Str(name), Some(&a.rack.scratch.span.1));
    }
    bar.divider(ui);
    let grid_key = GRIDS
        .iter()
        .find(|(_, g)| *g == app.rack.grid.0)
        .map(|(k, _)| *k)
        .unwrap_or("grid.sixteenth");
    let caption = chrome::word("arrangement.grid");
    let field = bar.field(
        ui,
        id.with("grid"),
        Some(caption),
        chrome::word(grid_key),
        64.0,
        || t("rack.grid_tip"),
    );
    if let Some(r) = field {
        egui::Popup::menu(&r).frame(egui::Frame::NONE).show(|ui| {
            if let Some(g) = super::arrangement::grid_menu(ui, app.rack.grid) {
                app.rack.grid = g;
                app.rack.scroll_steps = 0.0;
                app.rack.fit = true;
                ui.close();
            }
        });
    }
    bar.divider(ui);
    // The velocity a step switched on by hand gets: a knob, its value in the hint and
    // in a readout beside it.
    let caption = chrome::word("rack.velocity");
    if let Some(r) = bar.take_left(
        kit::text::measure(
            ui.ctx(),
            &kit::text::Text::new(caption, kit::text::Role::Label, bar.s.t.text_dim),
            bar.s.tk(),
        )
        .x / bar.s.k
            + 2.0,
        22.0,
    ) {
        let s = bar.s;
        kit::with_brush(ui.painter(), |b| {
            b.register(r, "caption", &[kit::Tok::TextDim], &[]);
            let tx = kit::text::Text::new(caption, kit::text::Role::Label, s.t.text_dim)
                .align(egui::Align2::RIGHT_CENTER);
            let _ = kit::text::paint(b, r, &tx, s.tk());
        });
    }
    if let Some(r) = bar.take_left(paint::KNOB + 2.0, paint::KNOB + 2.0) {
        let mut vel = app.rack.velocity as f32;
        let resp = row_knob(
            ui,
            r,
            "rack-velocity",
            &mut vel,
            1.0..=127.0,
            100.0,
            caption,
            Unit::Plain,
        );
        if resp.changed() {
            app.rack.velocity = vel.round().clamp(1.0, 127.0) as u8;
        }
    }
    if let Some(r) = bar.take_left(36.0, 22.0) {
        let mut d = chrome::Digits::default();
        let s = bar.s;
        let v = d.fmt(app.rack.velocity as i64);
        kit::with_brush(ui.painter(), |b| {
            kit::well::readout(b, r, &s, kit::State::REST, v, true)
        });
    }
    let fit = bar.key_right(
        ui,
        id.with("fit"),
        crate::icons::Icon::Zoom,
        app.rack.fit,
        true,
        || t("rack.fit"),
    );
    if fit.is_some_and(|r| r.clicked()) {
        app.rack.fit = true;
    }
    let counts = (app.project().tracks.len(), pattern.count);
    if app.rack.scratch.counts.0 != counts || app.rack.scratch.counts.1.is_empty() {
        app.rack.scratch.counts = (
            counts,
            tf("rack.counts", &[("pads", &counts.0), ("steps", &counts.1)]),
        );
    }
    let text = std::mem::take(&mut app.rack.scratch.counts.1);
    bar.readout_right(ui, &text, 150.0, false);
    app.rack.scratch.counts.1 = text;
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

fn set_pad(
    app: &mut CantusApp,
    track: TrackId,
    pad: usize,
    field: PadField,
    value: f32,
    merge: Option<(&str, usize)>,
) {
    // The row is an index into the kit; the command names the pad itself.
    let Some(pad) = app
        .project()
        .track(track)
        .and_then(|t| t.instrument.pulsus())
        .and_then(|k| k.pad(pad))
        .map(|p| p.id)
    else {
        return;
    };
    let cmd = Command::SetInstrumentParam {
        track,
        param: InstrumentParam::Pulsus { pad, field },
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
