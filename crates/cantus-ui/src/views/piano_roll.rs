//! Piano roll: the dense view, drawn with the kit (ADR 0024). Everything is painted on
//! the visible range only (ADR 0003 guardrail), never one widget per note — and, since
//! round 18, without copying the clip: the view paints with the project borrowed and acts
//! on the pointer afterwards (`piano_roll_paint.rs`).
//!
//! Mouse tools follow FL Studio's toolbar and letter shortcuts (P B E D T C Y); the number
//! keys 1–7 work too, and always, even while the typing keyboard plays notes. Hit testing
//! always uses the *press origin*, never the current pointer position, so a drag that
//! leaves a note's rectangle still acts on that note.
//!
//! Moving around is FL's: the wheel scrolls both ways (Shift+wheel is time), Ctrl+wheel
//! zooms time and Ctrl+Alt+wheel the rows, the middle button pans, and the bars on top
//! and at the right scroll by their thumb and zoom by its ends.

use super::clips_chrome::{self as chrome, Span, Toolbar, BAR};
use super::{beats_to_ticks, ticks_to_beats, GRIDS};
use crate::app::CantusApp;
use crate::i18n::{t, tf};
use crate::icons::Icon;
use crate::kit::hint::{self, Caps};
use crate::kit::text::Src;
use crate::kit::{self, Style};
use crate::theme;
use cantus_core::{
    pitch_name, Clip, ClipId, Command, InstrumentSpec, MergeKey, Note, NoteId, Pitch, Ticks,
    TrackId,
};
use egui::{pos2, vec2, PointerButton, Pos2, Rect, Sense};

/// The event lanes under the notes (ADR 0068).
#[path = "piano_roll_lanes.rs"]
pub mod lanes;
use lanes::LaneRow;
#[path = "piano_roll_paint.rs"]
mod paint;
/// The song's key in the piano roll (round 31, ADR 0069).
#[path = "piano_roll_scale.rs"]
pub mod scale;

const KEYS_W: f32 = 64.0;
/// Wider when the lanes are named: "Closed hat" does not fit in the width of "F#2".
const PAD_KEYS_W: f32 = 104.0;
/// Tallest a kit's row grows when the rows are stretched to fill the grid.
const PAD_ROW_MAX: f32 = 44.0;
const RULER_H: f32 = 20.0;
/// The velocity lane: its height by default and how far its splitter may move it.
const VEL_H: f32 = 80.0;
const VEL_MIN: f32 = 36.0;
const VEL_MAX: f32 = 520.0;
/// The splitter between the grid and the velocity lane.
const SPLIT_H: f32 = 6.0;
/// The rows' zoom range, in points per row.
const KEY_MIN: f32 = 6.0;
const KEY_MAX: f32 = 40.0;
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

    /// The tool's essentials (`<hint_key>.short`): what the grid says under the pointer.
    pub fn short_hint_key(self) -> &'static str {
        match self {
            Tool::Draw => "tool.draw.hint.short",
            Tool::Paint => "tool.paint.hint.short",
            Tool::Select => "tool.select.hint.short",
            Tool::Delete => "tool.delete.hint.short",
            Tool::Mute => "tool.mute.hint.short",
            Tool::Slice => "tool.slice.hint.short",
            Tool::Playback => "tool.playback.hint.short",
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
                // An Exemplar kit's names in the interface's language (round 28); a
                // Pulsus pad keeps the name it was given.
                Some(lane) if matches!(lanes, cantus_core::Lanes::Keys(_)) => {
                    crate::views::drum_names::row_name(&lanes.name(lane), *p)
                }
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

impl Axis {
    /// A row's name without allocating: the kit's pad names, or a keyboard's pitch names
    /// from a table built once.
    pub fn name_ref(&self, lane: usize) -> &str {
        if self.pitches.is_empty() {
            pitch_label(lane.min(127) as Pitch)
        } else {
            match self.names.get(lane) {
                Some(n) => n,
                None => pitch_label(self.pitch(lane)),
            }
        }
    }
}

/// "C4", "F#2": every pitch's name, built once, so the keyboard and the notes can label
/// themselves every frame without formatting a string.
pub fn pitch_label(p: Pitch) -> &'static str {
    static NAMES: std::sync::OnceLock<Vec<String>> = std::sync::OnceLock::new();
    &NAMES.get_or_init(|| (0..=127u8).map(pitch_name).collect())[p.min(127) as usize]
}

/// The selected notes as a sorted set, rebuilt only when the selection changes. The
/// selection is a list, and asking a list once per note is quadratic: with everything
/// selected on the stress project that was 400 million comparisons a frame.
#[derive(Debug, Default)]
pub(crate) struct SelSet {
    sig: (usize, u64),
    sorted: Vec<NoteId>,
}

impl SelSet {
    /// Below this many, a plain scan is quicker than keeping a copy.
    const SMALL: usize = 16;

    fn update(&mut self, ids: &[NoteId]) {
        if ids.len() <= Self::SMALL {
            return;
        }
        let sig = (
            ids.len(),
            ids.iter().fold(0u64, |a, i| {
                a.wrapping_mul(0x100_0000_01b3).wrapping_add(i.0)
            }),
        );
        if sig != self.sig {
            self.sorted.clear();
            self.sorted.extend_from_slice(ids);
            self.sorted.sort_unstable();
            self.sig = sig;
        }
    }

    pub(crate) fn contains(&self, ids: &[NoteId], id: NoteId) -> bool {
        if ids.len() <= Self::SMALL {
            ids.contains(&id)
        } else {
            self.sorted.binary_search(&id).is_ok()
        }
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
    /// The velocity lane's height; its splitter moves it.
    pub vel_h: f32,
    drag: Option<NoteDrag>,
    marquee: Option<(Pos2, Pos2)>,
    /// Slice tool: x position of the cut line while dragging.
    slice_x: Option<f32>,
    /// Paint tool: last cell painted in this stroke.
    paint_last: Option<(Ticks, u8)>,
    /// Gesture counter so each paint/erase stroke merges into its own undo step.
    stroke: u64,
    sel: SelSet,
    /// The axis last built, and the edit and clip it was built for.
    /// The axis and what it was built for — the language too (a kit's rows are named in
    /// it; sound review 2).
    axis: Option<((u64, TrackId, ClipId, crate::i18n::Locale), Axis)>,
    /// The grid's size last frame, for zoom-to-notes.
    last_grid: (f32, f32),
    /// The toolbar's note count, rebuilt when the count or the language changes (review of
    /// fix3: it kept the old language after a switch).
    count: (usize, crate::i18n::Locale, String),
    /// A clip opened from the arrangement, to fit to the grid the next time it is laid out
    /// ([`fit_clip`]).
    fit_pending: Option<(TrackId, ClipId)>,
    /// The event lanes' own state (ADR 0068).
    pub lanes: lanes::LanesView,
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
    /// A move's clip lanes when it began, when they follow the notes (ADR 0068).
    lanes: Vec<cantus_core::ClipLane>,
}

impl PianoRollView {
    /// The note grid's size when it was last laid out, in points (zero before).
    pub fn grid_size(&self) -> (f32, f32) {
        self.last_grid
    }
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
            vel_h: VEL_H,
            drag: None,
            marquee: None,
            slice_x: None,
            paint_last: None,
            stroke: 0,
            sel: SelSet::default(),
            axis: None,
            last_grid: (0.0, 0.0),
            count: (usize::MAX, crate::i18n::Locale::En, String::new()),
            fit_pending: None,
            lanes: lanes::LanesView::default(),
        }
    }
}

pub(crate) struct Geo<'a> {
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

pub(crate) fn is_black(pitch: u8) -> bool {
    matches!(pitch % 12, 1 | 3 | 6 | 8 | 10)
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

/// Where the view's regions are this frame.
#[derive(Copy, Clone, Debug)]
struct Frame {
    /// Above the keyboard: FL's note-colour tab.
    corner: Rect,
    hbar: Rect,
    ruler: Rect,
    keys: Rect,
    grid: Rect,
    vbar: Rect,
    vcorner: Rect,
    split: Rect,
    /// The velocity lane, and the cell beside it naming what it edits.
    vel: Rect,
    vel_label: Rect,
    vel_corner: Rect,
}

impl Frame {
    fn of(full: Rect, keys_w: f32, vel_h: f32, s: &Style) -> Frame {
        let bar = s.s(BAR);
        let (l, r) = (full.left(), full.right() - bar);
        let kx = l + keys_w;
        let vel_h = vel_h.clamp(VEL_MIN, (full.height() * 0.6).max(VEL_MIN));
        let hbar = Rect::from_min_max(pos2(kx, full.top()), pos2(r, full.top() + bar));
        let ruler = Rect::from_min_max(pos2(kx, hbar.bottom()), pos2(r, hbar.bottom() + RULER_H));
        let split_top = (full.bottom() - vel_h - SPLIT_H).max(ruler.bottom() + 40.0);
        let grid = Rect::from_min_max(pos2(kx, ruler.bottom()), pos2(r, split_top));
        let split = Rect::from_min_max(pos2(l, split_top), pos2(full.right(), split_top + SPLIT_H));
        let vel = Rect::from_min_max(pos2(kx, split.bottom()), pos2(r, full.bottom()));
        Frame {
            corner: Rect::from_min_max(full.min, pos2(kx, ruler.bottom())),
            hbar,
            ruler,
            keys: Rect::from_min_max(pos2(l, grid.top()), pos2(kx, grid.bottom())),
            grid,
            vbar: Rect::from_min_max(pos2(r, ruler.top()), pos2(full.right(), grid.bottom())),
            vcorner: Rect::from_min_max(pos2(r, full.top()), pos2(full.right(), ruler.top())),
            split,
            vel,
            vel_label: Rect::from_min_max(pos2(l, vel.top()), pos2(kx, vel.bottom())),
            vel_corner: Rect::from_min_max(pos2(r, vel.top()), full.max),
        }
    }
}

/// A press of any button inside the editor's area hands it the edit focus. The rack and
/// the piano roll edit the same notes, so both claim [`crate::app::EditFocus::PianoRoll`]; the
/// arrangement and the mixer say so themselves.
pub fn claim_focus(app: &mut CantusApp, ui: &egui::Ui) {
    if ui.input(|i| i.pointer.any_pressed()) && ui.rect_contains_pointer(ui.max_rect()) {
        app.focus_piano_roll();
    }
}

/// The clip being edited, borrowed from the project.
fn clip_of(app: &CantusApp, track: TrackId, clip: ClipId) -> Option<&Clip> {
    app.project().track(track)?.clip(clip)
}

pub fn show(app: &mut CantusApp, ui: &mut egui::Ui) {
    let s = Style::current();
    // Before the early returns, so an empty or audio clip's piano roll claims it too.
    claim_focus(app, ui);
    let Some((track_id, clip_id)) = app.selected_track.zip(app.selected_clip) else {
        empty(ui, "clip.none_selected", &s);
        return;
    };
    match clip_of(app, track_id, clip_id) {
        None => {
            app.selected_clip = None;
            return;
        }
        // An audio clip in its Warp mode (round 29, ADR 0061).
        Some(c) if c.audio.is_some() && app.warp.view.open => {
            super::warp::show(app, ui, track_id, clip_id);
            return;
        }
        // An audio clip: its notes in the pitch editor (round 28, ADR 0054); its fades,
        // gain and file stay in the inspector (ADR 0025 §1).
        Some(c) if c.audio.is_some() => {
            super::pitch::show(app, ui, track_id, clip_id);
            return;
        }
        // An automation clip: its curve in the large editor (ADR 0027).
        Some(c) if c.automation.is_some() => {
            super::auto_editor::show(app, ui, track_id, clip_id);
            return;
        }
        Some(_) => {}
    }
    toolbar(app, ui, track_id, clip_id);

    // The vertical axis comes from the *instrument*, once per frame, before anything is
    // laid out: it decides how wide the key strip has to be as well as what is in it.
    // Built once per edit, not once per frame: a kit's axis names every pad.
    let key = (app.bus.revision(), track_id, clip_id, crate::i18n::locale());
    let axis = match app.piano_roll.axis.take() {
        Some((k, a)) if k == key => a,
        _ => {
            let a: &CantusApp = app;
            match (a.project().track(track_id), clip_of(a, track_id, clip_id)) {
                // r31 midi: folded to the key when asked.
                (Some(t), Some(c)) => scale::fold(a, Axis::build(&t.instrument, c), c),
                _ => Axis::default(),
            }
        }
    };
    let keys_w = s.s(if axis.pads() { PAD_KEYS_W } else { KEYS_W });
    let full = ui.available_rect_before_wrap();
    if full.height() < 120.0 || full.width() < keys_w + 80.0 {
        return;
    }
    let f = Frame::of(full, keys_w, app.piano_roll.vel_h, &s);
    // The lanes under the grid (ADR 0068): velocity, the release, the clip's own lanes.
    let lane_rows = clip_of(app, track_id, clip_id)
        .map(|c| lanes::rows(&app.piano_roll.lanes, c))
        .unwrap_or_default();
    let (rows, max_scroll) = lanes::layout(
        f.vel,
        f.vel_label,
        &lane_rows,
        app.piano_roll.lanes.scroll,
        &s,
    );
    app.piano_roll.last_grid = (f.grid.width(), f.grid.height());
    // A clip opened from the arrangement (round 27, ADR 0051): fitted to this grid before
    // it is drawn, so the first frame shows it.
    if app.piano_roll.fit_pending == Some((track_id, clip_id)) {
        app.piano_roll.fit_pending = None;
        fit_now(app, &axis, track_id, clip_id);
    }

    let resp = ui.allocate_rect(f.grid, Sense::click_and_drag());
    let row_resps: Vec<egui::Response> = rows
        .iter()
        .map(|r| ui.allocate_rect(r.body, Sense::click_and_drag()))
        .collect();
    let ruler_resp = ui.allocate_rect(f.ruler, Sense::click());
    let keys_resp = ui.allocate_rect(f.keys, Sense::click_and_drag());
    navigate(app, ui, &f, &axis);
    split(app, ui, &f, &s);

    // A kit's rows grow to fill the grid. Seven pads at keyboard height are a strip along
    // the top of an empty canvas; seven pads at a sixth of the height each are a drum
    // editor. The stored `key_height` stays the keyboard's zoom, untouched.
    let max_top = axis.len() as f32;
    let v = &mut app.piano_roll;
    if v.top_lane > max_top || v.top_lane < 1.0 {
        v.top_lane = max_top;
    }
    let key_h = row_height(&axis, f.grid, v.key_height);
    if axis.pads() && key_h * axis.len() as f32 <= f.grid.height() + 0.5 {
        // Everything fits: pin the top row to the top so nothing scrolls off.
        v.top_lane = max_top;
    }
    let geo = Geo {
        grid: f.grid,
        px_per_beat: v.px_per_beat,
        scroll_beats: v.scroll_beats,
        key_h,
        top_lane: v.top_lane,
        axis: &axis,
    };
    let selected = std::mem::take(&mut app.selected_notes);
    app.piano_roll.sel.update(&selected);
    app.selected_notes = selected;

    // ---- paint: everything read, nothing edited ---------------------------------------
    let hover_key = keys_resp.hover_pos().map(|p| key_at(&geo, f.keys, p));
    let hits = {
        let a: &CantusApp = app;
        let v = &a.piano_roll;
        let clip = clip_of(a, track_id, clip_id).expect("checked above");
        let track_color = a
            .project()
            .track(track_id)
            .map(|t| theme::track_color(t.color))
            .unwrap_or(s.t.brand);
        let ph = clip
            .content_shown_at(a.engine.position())
            .unwrap_or_else(|| clip.content_of(a.engine.position()));
        let ph_x = geo.x(ticks_to_beats(ph));
        let grid_p = ui.painter().with_clip_rect(f.grid);
        let drawn = kit::with_brush(&grid_p, |b| {
            paint::grid(b, a, &geo, clip, v.grid, &s);
            scale::rows(b, a, &geo, &s); // r31 midi: the key's rows lit
            let drawn = paint::notes(b, &geo, clip, &v.sel, &a.selected_notes, track_color, &s);
            paint::recording(b, a, &geo, clip, &s);
            if let Some((p0, p1)) = v.marquee {
                let r = Rect::from_two_pos(p0, p1);
                b.rect(
                    r,
                    kit::Fill::Solid(kit::color::with_alpha(s.t.selection, 0.15)),
                );
                b.rrect_stroke(r, 0.0, b.px(), s.t.selection);
            }
            if let Some(x) = v.slice_x {
                b.vline(x, f.grid.top(), f.grid.bottom(), 1.5 * s.k, s.t.danger);
            }
            if ph_x >= f.grid.left() && ph_x <= f.grid.right() {
                b.vline(ph_x, f.grid.top(), f.grid.bottom(), 1.5 * s.k, s.t.playhead);
            }
            drawn
        });
        kit::with_brush(&ui.painter().with_clip_rect(f.keys), |b| {
            paint::keys(b, &geo, f.keys, hover_key, &s);
            scale::keys(b, a, &geo, f.keys, &s); // r31 midi: the key on the keyboard
        });
        kit::with_brush(&ui.painter().with_clip_rect(f.ruler), |b| {
            paint::ruler(b, a, &geo, f.ruler, clip, Some(ph_x), &s)
        });
        // The lanes, each in its own row (ADR 0068), the playhead through them.
        for rr in &rows {
            kit::with_brush(&ui.painter().with_clip_rect(rr.body), |b| {
                let field = match rr.row {
                    LaneRow::Velocity => Some(paint::NoteField::Velocity),
                    LaneRow::Release => Some(paint::NoteField::Release),
                    LaneRow::Ctl(_) => None,
                };
                match (field, rr.row) {
                    (Some(field), _) => paint::velocity(
                        b,
                        a,
                        &geo,
                        rr.body,
                        clip,
                        &v.sel,
                        &a.selected_notes,
                        track_color,
                        v.grid,
                        field,
                        &s,
                    ),
                    (None, LaneRow::Ctl(k)) => {
                        if let Some(l) = clip.lanes.iter().find(|l| l.kind == k) {
                            lanes::paint_ctl(
                                b,
                                a,
                                &geo,
                                rr.body,
                                clip,
                                l,
                                v.lanes.selected(k),
                                track_id,
                                track_color,
                                v.grid,
                                &s,
                            );
                        }
                    }
                    _ => {}
                }
                if ph_x >= rr.body.left() && ph_x <= rr.body.right() {
                    b.vline(ph_x, rr.body.top(), rr.body.bottom(), s.k, s.t.playhead);
                }
                lanes::paint_name(b, rr.row, rr.body, &s);
            });
        }
        let pointer = resp.hover_pos();
        let press = ui.input(|i| i.pointer.press_origin());
        let in_notes_lane = |p: &Pos2| {
            rows.iter()
                .any(|r| !matches!(r.row, LaneRow::Ctl(_)) && r.body.contains(*p))
        };
        Hits {
            drawn,
            hover: pointer.and_then(|p| geo.hit(&clip.notes, p)),
            press: press.and_then(|p| geo.hit(&clip.notes, p)),
            vel_press: press.filter(in_notes_lane).and_then(|p| {
                clip.notes
                    .iter()
                    .rev()
                    .find(|n| (geo.x(ticks_to_beats(n.start)) - p.x).abs() < 5.0)
                    .map(|n| n.id)
            }),
            to_timeline: clip.start - clip.offset,
            track_color,
        }
    };
    app.piano_roll_notes_drawn = hits.drawn;
    chrome_parts(app, ui, &f, &hits, &s);
    kit::with_brush(&ui.painter().with_clip_rect(f.vel_label), |b| {
        lanes::paint_cells(b, &app.piano_roll.lanes, &rows, &s)
    });

    // ---- act ------------------------------------------------------------------------
    for (rr, r) in rows.iter().zip(&row_resps) {
        match rr.row {
            LaneRow::Velocity | LaneRow::Release => {
                let field = if rr.row == LaneRow::Velocity {
                    paint::NoteField::Velocity
                } else {
                    paint::NoteField::Release
                };
                velocity_input(app, ui, &geo, rr.body, r, &hits, track_id, clip_id, field);
            }
            LaneRow::Ctl(k) => lanes::ctl_input(app, ui, &geo, rr.body, r, k, track_id, clip_id),
        }
    }
    lanes::cell_input(app, ui, &rows, max_scroll, f.vel_label, track_id, clip_id);
    if keys_resp.clicked() {
        if let Some(pos) = keys_resp.interact_pointer_pos() {
            app.audition(track_id, geo.axis.pitch(key_at(&geo, f.keys, pos)));
        }
    }
    if keys_resp.hovered() {
        hint::set_text(ui.ctx(), chrome::word("pianoroll.keys_tip"), "", Caps::NONE);
    }
    interact(app, ui, &geo, &resp, &ruler_resp, &hits, track_id, clip_id);

    // Cursor.
    if resp.hover_pos().is_some() {
        let dragging = app.piano_roll.drag.as_ref().map(|d| d.kind);
        let icon = match (app.piano_roll.tool, hits.hover, dragging) {
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
        // The tool's essentials (final review: the whole Draw list was cut at a laptop's
        // width); the whole is the idle hint and the hint panel's tooltip.
        hint::set_text(
            ui.ctx(),
            chrome::word(app.piano_roll.tool.short_hint_key()),
            "",
            Caps::NONE,
        );
    }
    app.piano_roll.axis = Some((key, axis));
}

/// What the painting pass found, for the input pass.
struct Hits {
    drawn: usize,
    /// The note under the pointer now, and whether it is on the resize grip.
    hover: Option<(NoteId, bool)>,
    /// The note under the pointer when the button went down.
    press: Option<(NoteId, bool)>,
    /// The velocity stem nearest the press, in the lane.
    vel_press: Option<NoteId>,
    /// Content ticks to the arrangement's: the ruler counts the arrangement's bars.
    to_timeline: Ticks,
    track_color: egui::Color32,
}

/// An empty view's one line, in a box of its own.
fn empty(ui: &mut egui::Ui, key: &str, s: &Style) {
    let r = ui.available_rect_before_wrap();
    kit::with_brush(ui.painter(), |b| {
        b.register(r, "empty", &[kit::Tok::Panel, kit::Tok::TextDim], &[]);
        b.rect(b.snap_rect(r), kit::Fill::Solid(s.t.panel));
        let tx = kit::text::Text::key(key, kit::text::Role::Title, s.t.text_dim)
            .policy(kit::text::Policy::Elide(kit::text::Elide::End));
        let _ = kit::text::paint(b, r.shrink(16.0), &tx, s.tk());
    });
}

/// How tall a row is: the keyboard's zoom, or for a kit, what fills the grid.
fn row_height(axis: &Axis, grid: Rect, key_height: f32) -> f32 {
    if axis.pads() {
        let fit = grid.height() / axis.len().max(1) as f32;
        fit.clamp(key_height, PAD_ROW_MAX)
    } else {
        key_height
    }
}

/// The row a point on the keyboard plays: on a black key's row, the white key beside it
/// when the point is past the black key's end, as on a piano.
fn key_at(geo: &Geo<'_>, keys: Rect, p: Pos2) -> usize {
    let lane = geo.lane_at(p.y);
    if geo.axis.pads() || p.x < keys.left() + keys.width() * 0.60 {
        return lane;
    }
    let pitch = geo.axis.pitch(lane);
    if !is_black(pitch) {
        return lane;
    }
    // Upper half of the black row belongs to the white key above, lower half below.
    let mid = geo.y(lane as f32 + 0.5);
    let up = p.y < mid;
    let target = if up { lane + 1 } else { lane.saturating_sub(1) };
    target.min(geo.axis.len() - 1)
}

/// The wheel, the middle button and the two bars.
fn navigate(app: &mut CantusApp, ui: &mut egui::Ui, f: &Frame, axis: &Axis) {
    let area = Rect::from_min_max(pos2(f.keys.left(), f.ruler.top()), f.vel.max);
    let over = ui.rect_contains_pointer(area) && !ui.rect_contains_pointer(f.split);
    let pointer = ui.input(|i| i.pointer.hover_pos());
    let rows = axis.len() as f32;
    let key_h = row_height(axis, f.grid, app.piano_roll.key_height);
    if over {
        let w = chrome::wheel(ui);
        let v = &mut app.piano_roll;
        if w.zoom_x != 1.0 {
            let at_x = pointer
                .map(|p| p.x.max(f.grid.left()))
                .unwrap_or(f.grid.left());
            let anchor = (at_x - f.grid.left()) / v.px_per_beat + v.scroll_beats;
            v.px_per_beat = (v.px_per_beat * w.zoom_x).clamp(MIN_PX_PER_BEAT, MAX_PX_PER_BEAT);
            v.scroll_beats = (anchor - (at_x - f.grid.left()) / v.px_per_beat).max(0.0);
        }
        if w.zoom_y != 1.0 && !axis.pads() {
            // Keep the row under the pointer under it.
            let at_y = pointer.map(|p| p.y).unwrap_or(f.grid.center().y) - f.grid.top();
            let lane_at = v.top_lane - at_y / v.key_height;
            v.key_height = (v.key_height * w.zoom_y).clamp(KEY_MIN, KEY_MAX);
            v.top_lane = (lane_at + at_y / v.key_height).clamp(1.0, rows);
        }
        // Alt+wheel over a note is velocity, handled with the notes.
        if !w.alt && w.scroll != egui::Vec2::ZERO {
            // Shift+wheel arrives as `x`: there is nothing to ask about Shift.
            v.scroll_beats = (v.scroll_beats - w.scroll.x / v.px_per_beat).max(0.0);
            v.top_lane = (v.top_lane + w.scroll.y / key_h).clamp(1.0, rows);
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
        let v = &mut app.piano_roll;
        v.scroll_beats = (v.scroll_beats - delta.x / v.px_per_beat).max(0.0);
        v.top_lane = (v.top_lane + delta.y / key_h).clamp(1.0, rows);
    }

    // The bars.
    let total = content_beats(app)
        .max(app.piano_roll.scroll_beats + f.grid.width() / app.piano_roll.px_per_beat);
    let v = &mut app.piano_roll;
    let span = Span::new(v.scroll_beats, f.grid.width() / v.px_per_beat, total);
    let zoom = Some((
        f.grid.width() / MAX_PX_PER_BEAT,
        f.grid.width() / MIN_PX_PER_BEAT,
    ));
    if let Some(n) = chrome::bar(
        ui,
        f.hbar,
        ui.id().with("pr-hbar"),
        span,
        zoom,
        "views.nav.hbar",
    ) {
        v.scroll_beats = n.start.max(0.0);
        v.px_per_beat = (f.grid.width() / n.len).clamp(MIN_PX_PER_BEAT, MAX_PX_PER_BEAT);
    }
    // Rows count from the top here: the bar's start is the top row's distance from the
    // highest one.
    let shown = (f.grid.height() / key_h).min(rows);
    let span = Span::new((rows - v.top_lane).max(0.0), shown, rows);
    let zoom = (!axis.pads()).then(|| (f.grid.height() / KEY_MAX, f.grid.height() / KEY_MIN));
    if let Some(n) = chrome::bar(
        ui,
        f.vbar,
        ui.id().with("pr-vbar"),
        span,
        zoom,
        "views.nav.vbar",
    ) {
        if !axis.pads() {
            v.key_height = (f.grid.height() / n.len).clamp(KEY_MIN, KEY_MAX);
        }
        v.top_lane = (rows - n.start).clamp(1.0, rows);
    }
}

/// How far the piano roll can scroll: the clip's content, and four bars past it.
fn content_beats(app: &CantusApp) -> f32 {
    let clip = app
        .selected_track
        .zip(app.selected_clip)
        .and_then(|(t, c)| clip_of(app, t, c));
    // The bar where the clip's content starts, through the metre changes (ADR 0060).
    let bar = ticks_to_beats(
        clip.map_or(app.project().time_signature.ticks_per_bar(), |c| {
            crate::views::bars::bar_len(app.project(), c.start - c.offset)
        }),
    );
    let end = clip
        .map(|c| {
            let notes = c.notes.last().map(|n| n.end()).unwrap_or(Ticks::ZERO);
            ticks_to_beats(c.content_start() + c.length).max(ticks_to_beats(notes))
        })
        .unwrap_or(0.0);
    end + 4.0 * bar
}

/// The splitter over the velocity lane: drag to resize it, right-click to reset,
/// double-click to toggle its last two heights (FL's event-lane handle).
fn split(app: &mut CantusApp, ui: &mut egui::Ui, f: &Frame, s: &Style) {
    let id = ui.id().with("pr-vel-split");
    let resp = ui.interact(f.split, id, Sense::click_and_drag());
    let v = &mut app.piano_roll;
    if resp.dragged_by(PointerButton::Primary) {
        v.vel_h = (v.vel_h - resp.drag_delta().y).clamp(VEL_MIN, VEL_MAX);
    }
    if resp.secondary_clicked() {
        v.vel_h = VEL_H;
    }
    if resp.double_clicked() {
        let last = ui.data(|d| d.get_temp::<f32>(id)).unwrap_or(VEL_MIN);
        ui.data_mut(|d| d.insert_temp(id, v.vel_h));
        v.vel_h = last;
    }
    if resp.hovered() || resp.dragged() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeVertical);
        hint::set_text(
            ui.ctx(),
            chrome::word("pianoroll.split_tip"),
            "",
            Caps::NONE,
        );
    }
    let st = kit::State::of(&resp, false);
    kit::with_brush(ui.painter(), |b| {
        b.register(f.split, "split_ground", &[kit::Tok::Panel], &[]);
        b.rect(b.snap_rect(f.split), kit::Fill::Solid(s.t.panel));
        kit::chrome::splitter(b, f.split, s, st);
    });
}

/// The small parts around the grid: FL's note-colour tab over the keyboard, the corners
/// beside the bars, and the cell naming what the lower lane edits.
fn chrome_parts(app: &CantusApp, ui: &egui::Ui, f: &Frame, hits: &Hits, s: &Style) {
    let t = &s.t;
    let ink = paint::NoteInk::of(hits.track_color, s);
    kit::with_brush(ui.painter(), |b| {
        chrome::corner(b, f.vcorner, s);
        chrome::corner(b, f.vel_corner, s);
        b.register(f.corner, "note_tab", &[kit::Tok::Panel], &[]);
        b.rect(b.snap_rect(f.corner), kit::Fill::Solid(t.panel));
        // The tab: the note colour, slanted at its right edge (FL).
        let c = f.corner.shrink2(vec2(s.s(4.0), s.s(4.0)));
        let slant = c.height().min(s.s(18.0));
        b.polygon(
            &[
                pos2(c.left(), c.bottom()),
                pos2(c.left(), c.top() + s.s(3.0)),
                pos2(c.right() - slant, c.top() + s.s(3.0)),
                pos2(c.right(), c.bottom()),
            ],
            ink.outline,
        );
        let inner = c.shrink(1.0);
        b.polygon(
            &[
                pos2(inner.left(), inner.bottom()),
                pos2(inner.left(), inner.top() + s.s(3.0)),
                pos2(inner.right() - slant, inner.top() + s.s(3.0)),
                pos2(inner.right(), inner.bottom()),
            ],
            ink.bottom,
        );
    });
    let _ = app;
}

/// A tool key's tooltip: its name and the shortcut bound right now.
fn tool_tip(app: &CantusApp, tool: Tool) -> String {
    let keys = app.keymap.display_for(tool.action());
    if keys.is_empty() {
        t(tool.label_key())
    } else {
        format!("{} ({keys})", t(tool.label_key()))
    }
}

/// The toolbar: FL's piano-roll row — ▸, the clip and its track, the tools as keys, the
/// grid and snap, quantize, zoom to the notes, and the count at the right end.
fn toolbar(app: &mut CantusApp, ui: &mut egui::Ui, track: TrackId, clip: ClipId) {
    use crate::keymap::Action;
    let id = ui.id().with("pianoroll-toolbar");
    let mut bar = Toolbar::new(ui);
    if let Some(r) = bar.menu(ui, id.with("menu"), "pianoroll.menu") {
        egui::Popup::menu(&r).show(|ui| {
            for (action, key) in [
                (Action::SelectAll, "menu.select_all"),
                (Action::Quantize, "menu.quantize"),
                (Action::TransposeOctaveUp, "action.transpose-octave-up"),
                (Action::TransposeOctaveDown, "action.transpose-octave-down"),
            ] {
                if super::arrangement::action_item(app, ui, action, key) {
                    ui.close();
                }
            }
        });
    }
    {
        let a: &CantusApp = app;
        let clip_name = clip_of(a, track, clip)
            .map(|c| c.name.as_str())
            .unwrap_or("");
        let track_name = a.project().track(track).map(|t| t.name.as_str());
        bar.title(ui, Src::Str(clip_name), track_name);
    }
    bar.divider(ui);
    for tool in Tool::ALL {
        let lit = app.piano_roll.tool == tool;
        let r = bar.key(
            ui,
            id.with(tool.label_key()),
            tool.icon(),
            lit,
            true,
            || tool_tip(app, tool),
        );
        if r.is_some_and(|r| r.clicked()) {
            app.piano_roll.tool = tool;
        }
    }
    bar.divider(ui);
    let grid_key = GRIDS
        .iter()
        .find(|(_, g)| *g == app.piano_roll.grid.0)
        .map(|(k, _)| *k)
        .unwrap_or("grid.sixteenth");
    let caption = t("pianoroll.grid");
    let field = bar.field(
        ui,
        id.with("grid"),
        Some(&caption),
        &t(grid_key),
        64.0,
        || t("pianoroll.grid_tip"),
    );
    if let Some(r) = field {
        egui::Popup::menu(&r).frame(egui::Frame::NONE).show(|ui| {
            if let Some(g) = super::arrangement::grid_menu(ui, app.piano_roll.grid) {
                app.piano_roll.grid = g;
                ui.close();
            }
        });
    }
    let snap = bar.led(
        ui,
        id.with("snap"),
        app.piano_roll.snap,
        "pianoroll.snap",
        58.0,
        || t("pianoroll.snap_tip"),
    );
    if snap.is_some_and(|r| r.clicked()) {
        app.piano_roll.snap = !app.piano_roll.snap;
    }
    scale::toolbar(app, ui, &mut bar, id); // r31 midi: the key, Scale, Fold, Snap
    let q = t("menu.quantize");
    let quant = bar.word(ui, id.with("quantize"), &q, 72.0, || {
        let keys = app.keymap.display_for(Action::Quantize);
        if keys.is_empty() {
            t("menu.quantize")
        } else {
            format!("{} ({keys})", t("menu.quantize"))
        }
    });
    if quant.is_some_and(|r| r.clicked()) {
        app.quantize_selection();
    }
    let zoom = bar.key(ui, id.with("zoom"), Icon::Zoom, false, true, || {
        t("pianoroll.zoom_notes")
    });
    if zoom.is_some_and(|r| r.clicked()) {
        zoom_to_notes(app, track, clip);
    }
    let n = clip_of(app, track, clip).map_or(0, |c| c.notes.len());
    let v = &mut app.piano_roll;
    let lang = crate::i18n::locale();
    if (v.count.0, v.count.1) != (n, lang) {
        v.count = (n, lang, tf("pianoroll.notes", &[("n", &n)]));
    }
    let text = std::mem::take(&mut v.count.2);
    bar.readout_right(ui, &text, 96.0, false);
    app.piano_roll.count.2 = text;
}

/// A clip opened from the arrangement (round 27, ADR 0051): its window — one period of a
/// clip that repeats, else its length — across the grid from the content's start, and its
/// notes' rows in the middle, the keys made smaller only when they would not fit. Done the
/// next time the piano roll is laid out, where the grid's size is known (a piano roll
/// behind another tab has none yet).
pub fn fit_clip(app: &mut CantusApp, track: TrackId, clip: ClipId) {
    app.piano_roll.fit_pending = Some((track, clip));
    if let Some(c) = clip_of(app, track, clip) {
        app.piano_roll.scroll_beats = ticks_to_beats(c.content_start()).max(0.0);
    }
}

/// [`fit_clip`] on the grid just laid out.
fn fit_now(app: &mut CantusApp, axis: &Axis, track: TrackId, clip: ClipId) {
    let Some(c) = clip_of(app, track, clip) else {
        return;
    };
    let beats = ticks_to_beats(c.period().unwrap_or(c.length)).max(0.25);
    let start = ticks_to_beats(c.content_start()).max(0.0);
    let lanes = c.notes.iter().fold(None, |acc: Option<(usize, usize)>, n| {
        let Some(l) = axis.lane_of(n.pitch) else {
            return acc;
        };
        Some(acc.map_or((l, l), |(a, b)| (a.min(l), b.max(l))))
    });
    let rows = axis.len() as f32;
    let chromatic = axis.is_chromatic();
    let (w, h) = app.piano_roll.last_grid;
    let v = &mut app.piano_roll;
    if w > 0.0 {
        v.px_per_beat = (w * 0.95 / beats).clamp(MIN_PX_PER_BEAT, MAX_PX_PER_BEAT);
    }
    v.scroll_beats = start;
    // A kit's rows already fill the grid; a keyboard's are centred on the notes.
    if let (true, Some((lo, hi)), true) = (chromatic, lanes, h > 0.0) {
        let span = (hi - lo) as f32 + 3.0;
        if span * v.key_height > h {
            v.key_height = (h / span).clamp(KEY_MIN, KEY_MAX);
        }
        let shown = h / v.key_height;
        let mid = (lo + hi) as f32 * 0.5;
        v.top_lane = (mid + shown * 0.5 + 0.5).clamp(1.0, rows.max(1.0));
    }
}

/// Show the selected notes — or every note when none is selected — filling the grid:
/// the piano roll's zoom to selection. It runs on the next frame's layout, which is the
/// grid this frame drew.
fn zoom_to_notes(app: &mut CantusApp, track: TrackId, clip: ClipId) {
    let Some(c) = clip_of(app, track, clip) else {
        return;
    };
    let sel = &app.selected_notes;
    let mut span: Option<(Ticks, Ticks, Pitch, Pitch)> = None;
    for n in c
        .notes
        .iter()
        .filter(|n| sel.is_empty() || sel.contains(&n.id))
    {
        span = Some(match span {
            None => (n.start, n.end(), n.pitch, n.pitch),
            Some((a, b, lo, hi)) => (
                a.min(n.start),
                b.max(n.end()),
                lo.min(n.pitch),
                hi.max(n.pitch),
            ),
        });
    }
    let Some((from, to, lo, hi)) = span else {
        return;
    };
    let chromatic = app
        .project()
        .track(track)
        .is_some_and(|t| t.instrument.is_chromatic());
    let (w, h) = app.piano_roll.last_grid;
    if w <= 0.0 || h <= 0.0 {
        return;
    }
    let beats = (ticks_to_beats(to) - ticks_to_beats(from)).max(0.25);
    let v = &mut app.piano_roll;
    v.px_per_beat = (w * 0.9 / beats).clamp(MIN_PX_PER_BEAT, MAX_PX_PER_BEAT);
    v.scroll_beats = (ticks_to_beats(from) - beats * 0.05).max(0.0);
    // Rows are pitches only on a keyboard; a kit's rows already fill the grid.
    if chromatic {
        let rows = (hi - lo) as f32 + 3.0;
        v.key_height = (h / rows).clamp(KEY_MIN, KEY_MAX);
        v.top_lane = hi as f32 + 2.0;
    }
}

/// Dragging a velocity stem, in the lane under the grid.
#[allow(clippy::too_many_arguments)]
fn velocity_input(
    app: &mut CantusApp,
    ui: &egui::Ui,
    geo: &Geo<'_>,
    vel: Rect,
    resp: &egui::Response,
    hits: &Hits,
    track: TrackId,
    clip_id: ClipId,
    field: paint::NoteField,
) {
    let (top, bottom) = (vel.top() + 8.0, vel.bottom() - 3.0);
    if resp.hovered() {
        hint::set_text(
            ui.ctx(),
            chrome::word(match field {
                paint::NoteField::Velocity => "pianoroll.velocity_tip",
                paint::NoteField::Release => "automodes.lane.release_tip",
            }),
            "",
            Caps::NONE,
        );
    }
    if resp.drag_started_by(PointerButton::Primary) {
        let press = ui.input(|i| i.pointer.press_origin());
        if let (Some(press), Some(id)) = (press, hits.vel_press) {
            if !app.selected_notes.contains(&id) {
                app.selected_notes = vec![id];
            }
            let originals: Vec<Note> = clip_of(app, track, clip_id)
                .map(|c| {
                    c.notes
                        .iter()
                        .filter(|n| app.selected_notes.contains(&n.id))
                        .cloned()
                        .collect()
                })
                .unwrap_or_default();
            app.piano_roll.drag = Some(NoteDrag {
                kind: DragKind::Velocity,
                originals,
                press,
                track,
                clip: clip_id,
                lanes: Vec::new(),
            });
        }
    }
    if resp.dragged_by(PointerButton::Primary) {
        if let (Some(pos), Some(d)) = (resp.interact_pointer_pos(), app.piano_roll.drag.as_ref()) {
            if d.kind == DragKind::Velocity {
                let dv = -(pos.y - d.press.y) / (bottom - top) * 127.0;
                let notes: Vec<Note> = d
                    .originals
                    .iter()
                    .map(|n| match field {
                        paint::NoteField::Velocity => Note {
                            velocity: (n.velocity as f32 + dv).round().clamp(1.0, 127.0) as u8,
                            ..n.clone()
                        },
                        // A release drawn from none starts where the press is (ADR 0068).
                        paint::NoteField::Release => {
                            let from = if n.release == 0 {
                                (bottom - d.press.y) / (bottom - top) * 127.0
                            } else {
                                n.release as f32
                            };
                            Note {
                                release: (from + dv).round().clamp(1.0, 127.0) as u8,
                                ..n.clone()
                            }
                        }
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
    let _ = geo;
}

/// The notes of `ids` in the clip, with `f` applied: for the edits that rewrite a few
/// notes (velocity by the wheel, a drag). Borrowed only for the copy.
fn edited(
    app: &CantusApp,
    track: TrackId,
    clip: ClipId,
    ids: &[NoteId],
    f: impl Fn(&Note) -> Note,
) -> Vec<Note> {
    clip_of(app, track, clip)
        .map(|c| {
            c.notes
                .iter()
                .filter(|n| ids.contains(&n.id))
                .map(f)
                .collect()
        })
        .unwrap_or_default()
}

#[allow(clippy::too_many_arguments)]
fn interact(
    app: &mut CantusApp,
    ui: &egui::Ui,
    geo: &Geo<'_>,
    resp: &egui::Response,
    ruler_resp: &egui::Response,
    hits: &Hits,
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
    // Content ticks, snapped through the timeline's bars when the metre changes (ADR 0060).
    let to_tl = hits.to_timeline;
    let snapper = crate::views::bars::Snapper::of(app.project());
    let snap_floor = |t: Ticks| {
        if snapping {
            snapper.content_floor(t, grid_ticks, to_tl)
        } else {
            t
        }
    };
    let snap_round = |t: Ticks| {
        if snapping {
            snapper.content(t, grid_ticks, to_tl)
        } else {
            t
        }
    };
    let tool = app.piano_roll.tool;
    let (hover_hit, press_hit) = (hits.hover, hits.press);
    let pitch_of = |app: &CantusApp, id: NoteId| {
        clip_of(app, track, clip_id)
            .and_then(|c| c.note(id))
            .map(|n| n.pitch)
    };

    if ruler_resp.clicked() {
        if let Some(pos) = ruler_resp.interact_pointer_pos() {
            let tick = geo.tick_at(pos.x).max(Ticks::ZERO);
            // The beat under the click, through the metre changes (ADR 0060).
            let tick = crate::views::bars::map(app.project())
                .snap_floor(tick + hits.to_timeline, Ticks::QUARTER);
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
                let notes = edited(app, track, clip_id, &ids, |n| Note {
                    velocity: (n.velocity as i32 + delta).clamp(1, 127) as u8,
                    ..n.clone()
                });
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
            let hit = hover_hit
                .filter(|_| resp.hover_pos() == Some(pos))
                .or_else(|| clip_of(app, track, clip_id).and_then(|c| geo.hit(&c.notes, pos)));
            match tool {
                Tool::Draw | Tool::Paint => match hit {
                    Some((id, _)) => {
                        select_note(app, id, ctrl);
                        if let Some(p) = pitch_of(app, id) {
                            app.audition(track, p);
                        }
                    }
                    None if !ctrl => {
                        let start = snap_floor(geo.tick_at(pos.x)).max(Ticks::ZERO);
                        let pitch = scale::snap(app, geo.axis, geo.pitch_at(pos.y)); // r31 midi
                        let length = drawn_note_length(
                            geo.axis,
                            snapping,
                            crate::views::bars::grid_len(app.project(), grid_ticks, to_tl + start),
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
                        .and_then(|(id, _)| pitch_of(app, id))
                        .unwrap_or_else(|| geo.pitch_at(pos.y));
                    app.audition(track, pitch);
                }
            }
        }
    }
    // Right-click deletes in every editing tool (FL convention).
    if resp.secondary_clicked() {
        let hit = resp
            .interact_pointer_pos()
            .and_then(|p| clip_of(app, track, clip_id).and_then(|c| geo.hit(&c.notes, p)));
        if let Some((id, _)) = hit {
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
                    let ids = app.selected_notes.clone();
                    let originals = edited(app, track, clip_id, &ids, Note::clone);
                    // The lanes under them move with them (ADR 0068), when they follow.
                    let lanes = match clip_of(app, track, clip_id) {
                        Some(c) if !resize && app.piano_roll.lanes.follow => c.lanes.clone(),
                        _ => Vec::new(),
                    };
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
                        lanes,
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
                paint_cell(app, geo, track, clip_id, press, snapping, grid_ticks);
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
                                        pitch: scale::moved_with(
                                            app,
                                            geo.axis,
                                            &d.originals,
                                            n.pitch,
                                            dp,
                                        ), // r31 midi
                                        ..n.clone()
                                    })
                                    .collect()
                            }
                            DragKind::Resize => d
                                .originals
                                .iter()
                                .map(|n| {
                                    let end = snap_round(n.end() + raw);
                                    let min_len = if snapping {
                                        crate::views::bars::grid_len(
                                            app.project(),
                                            grid_ticks,
                                            to_tl + n.start,
                                        )
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
                        if d.kind == DragKind::Resize {
                            if let Some(first) = notes.first() {
                                app.piano_roll.last_note_length = first.length;
                            }
                        }
                        let (track, clip_id) = (d.track, d.clip);
                        // The controllers under the notes move with them (ADR 0068): the
                        // stretch from the first note's start to the last's end, by as much.
                        let lanes_cmd = (d.kind == DragKind::Move && !d.lanes.is_empty())
                            .then(|| {
                                let a = d.originals.iter().map(|n| n.start).min()?;
                                let b = d.originals.iter().map(Note::end).max()?;
                                let moved = notes.iter().map(|n| n.start).min()?;
                                Some(Command::SetClipLanes {
                                    track,
                                    clip: clip_id,
                                    lanes: cantus_core::clip_lanes::edit::shift(
                                        &d.lanes,
                                        a,
                                        b,
                                        moved - a,
                                    ),
                                })
                            })
                            .flatten();
                        let set = Command::SetNotes {
                            track,
                            clip: clip_id,
                            notes,
                        };
                        let cmd = match lanes_cmd {
                            Some(l) => Command::Batch {
                                label_key: set.label_key().to_string(),
                                commands: vec![set, l],
                            },
                            None => set,
                        };
                        app.exec_merging(cmd, MergeKey::of(&("notes", clip_id.0)));
                    } else if let Some((a, _)) = app.piano_roll.marquee {
                        app.piano_roll.marquee = Some((a, pos));
                        let r = Rect::from_two_pos(a, pos);
                        let mut sel: Vec<NoteId> = if ctrl {
                            app.selected_notes.clone()
                        } else {
                            Vec::new()
                        };
                        if let Some(c) = clip_of(app, track, clip_id) {
                            for n in &c.notes {
                                if r.intersects(geo.note_rect(n)) && !sel.contains(&n.id) {
                                    sel.push(n.id);
                                }
                            }
                        }
                        app.selected_notes = sel;
                    }
                }
                Tool::Paint => paint_cell(app, geo, track, clip_id, pos, snapping, grid_ticks),
                Tool::Delete => {
                    let hit = clip_of(app, track, clip_id).and_then(|c| geo.hit(&c.notes, pos));
                    if let Some((id, _)) = hit {
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
                    if let Some(p) = pitch_of(app, first.id) {
                        app.audition(track, p);
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
    track: TrackId,
    clip_id: ClipId,
    pos: Pos2,
    snapping: bool,
    grid_ticks: Ticks,
) {
    let raw = geo.tick_at(pos.x);
    let to_tl = clip_of(app, track, clip_id).map_or(Ticks::ZERO, |c| c.start - c.offset);
    let start = if snapping {
        crate::views::bars::snap_content_floor(app.project(), raw, grid_ticks, to_tl)
    } else {
        raw
    }
    .max(Ticks::ZERO);
    let pitch = scale::snap(app, geo.axis, geo.pitch_at(pos.y)); // r31 midi: snap to scale
    if app.piano_roll.paint_last == Some((start, pitch)) {
        return;
    }
    app.piano_roll.paint_last = Some((start, pitch));
    let occupied = clip_of(app, track, clip_id).is_some_and(|c| {
        c.notes
            .iter()
            .any(|n| n.pitch == pitch && n.start <= start && n.end() > start)
    });
    if occupied {
        return;
    }
    let grid_here = crate::views::bars::grid_len(app.project(), grid_ticks, to_tl + start);
    let length = drawn_note_length(geo.axis, snapping, grid_here, grid_here);
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
