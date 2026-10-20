//! Arrangement view — FL's playlist, drawn with the kit (ADR 0024): track headers on the
//! left as coloured blocks, a timeline with clips on the right, the ruler with the loop
//! brace and the time selection above, and a scroll bar on each axis that is also a zoom.
//!
//! It scrolls both ways (it could not scroll vertically before, `cantus-audit.md` #3),
//! Shift+wheel scrolls time (#4), Ctrl+wheel zooms time and Ctrl+Alt+wheel the rows,
//! the middle button pans, and the header column's edge can be dragged. The painting is
//! in [`paint`]; this file is the state, the layout and the input.

use super::clips_chrome::{self as chrome, Digits, Span, Tints, Toolbar, BAR};
use super::{beats_to_ticks, ticks_to_beats, GRIDS};
use crate::app::CantusApp;

use crate::i18n::{t, tf};
use crate::icons::{self, Icon};
use crate::kit::hint::{self, Caps};
use crate::kit::menu::{Check, Item};
use crate::kit::text::Src;
use crate::kit::{self, Style};
use crate::theme;
use cantus_core::{
    ClipId, Command, LoopRegion, MergeKey, Ticks, TrackId, MAX_TRACK_HEIGHT, MIN_TRACK_HEIGHT,
};
use egui::{pos2, vec2, PointerButton, Pos2, Rect, Sense};

#[path = "arrangement_audio.rs"]
mod audio;
pub use audio::edit::CURVE_KEYS;
pub use audio::{curve_icon, Handle as AudioHandle, Handles as AudioHandles};
// The warp editor reads clips the way the arrangement does (round 29, ADR 0061).
pub use audio::{warp as warp_api, Shown as AudioShown};

/// Automation lanes and the curves in their clips (ADR 0027).
#[path = "arrangement_auto.rs"]
pub mod autolanes;
/// The comp on the playing lane: seams, take numbers, the comps' chip (ADR 0058).
#[path = "arrangement_comp.rs"]
pub mod comp;
/// The markers on the ruler (round 29, ADR 0060).
#[path = "arrangement_markers.rs"]
pub mod markers;
#[path = "arrangement_paint.rs"]
mod paint;
#[path = "arrangement_rec.rs"]
mod rec;
#[path = "arrangement_takes.rs"]
mod takes;
/// The tempo track and the metre chips (round 29, ADR 0060).
#[path = "arrangement_tempo.rs"]
pub mod tempo;
pub use rec::{record_controls, RecordControls};

/// The ruler is two lanes, and that is what makes both gestures direct.
///
/// The **loop lane** on top is where dragging sets the loop brace, which is what a bare
/// drag in the ruler used to do. The **selection lane** below it is where dragging makes a
/// time selection. Putting the new gesture on top of the old one would have cost the loop
/// its direct manipulation; putting the selection behind a modifier (FL's `Ctrl+drag`)
/// would have hidden the thing nearly everything else in the playlist plan hangs off.
/// Reaper splits the ruler the same way, for the same reason.
const RULER_H: f32 = 26.0;
const LOOP_LANE_H: f32 = 8.0;
/// How wide the grab zone on a header's bottom edge is.
const DIVIDER_GRIP: f32 = 5.0;
/// The header column: its width by default, and how far its edge may be dragged.
const HEADER_W: f32 = 220.0;
const HEADER_MIN: f32 = 150.0;
const HEADER_MAX: f32 = 420.0;
/// The base row height's range: the rows' zoom.
const BASE_MIN: f32 = 22.0;
const BASE_MAX: f32 = 120.0;
/// The time zoom's range, in points per beat.
const PX_MIN: f32 = 2.0;
const PX_MAX: f32 = 400.0;

#[derive(Debug)]
pub struct ArrangementView {
    /// The tempo track's lane (round 29, ADR 0060).
    pub tempo: tempo::State,
    /// The marker lane (round 29, ADR 0060).
    pub markers: markers::State,
    pub px_per_beat: f32,
    pub scroll_beats: f32,
    /// How far the rows are scrolled up, in points.
    pub scroll_y: f32,
    /// The base row height. A track's own height is a multiple of this, so the interface
    /// scale and the per-track choice compose instead of fighting. Ctrl+Alt+wheel and the
    /// vertical bar's ends zoom it.
    pub track_height: f32,
    /// The header column's width; its edge is a splitter.
    pub header_width: f32,
    /// A header divider being dragged: the track, and the height it started at.
    divider: Option<(TrackId, f32)>,
    /// A track header being dragged to reorder: the track, and where it would land.
    reorder: Option<(TrackId, usize)>,
    /// A time selection being dragged in the ruler: where it started.
    selecting: Option<Ticks>,
    /// A loop brace being dragged in the loop lane: where it started.
    looping: Option<Ticks>,
    /// What a clip drag snaps to, chosen from the toolbar. Ctrl bypasses it for one drag.
    pub grid: Ticks,
    /// Whether the grid is on at all. The toolbar toggle, mirroring the piano roll's.
    pub snap: bool,
    drag: Option<ClipDrag>,
    /// Rubber band over empty timeline: press origin and current pointer.
    marquee: Option<(egui::Pos2, egui::Pos2)>,
    /// The clip being renamed in place, and the text so far.
    rename_clip: Option<((TrackId, ClipId), String)>,
    /// Where the timeline was drawn last frame (`TimelineGeometry`).
    geometry: Option<TimelineGeometry>,
    /// Which clip the context menu is about, captured when the right button goes down.
    ///
    /// It cannot be re-read from the pointer while the menu is open: the menu appears
    /// next to the pointer, so moving towards it takes the pointer off the clip, and a
    /// menu that only exists while its own target is hovered closes before it can be used.
    menu_clip: Option<(TrackId, ClipId)>,
    /// Where the empty-timeline menu was opened — the tick under the pointer and the track
    /// — so its *Paste* lands there, not at the playhead (ADR 0036). Captured with
    /// [`Self::menu_clip`]; used once.
    pub menu_at: Option<(Ticks, Option<TrackId>)>,
    /// The track being renamed in place, and the text so far (a click on its name's
    /// letters opens it: round 27, ADR 0051).
    rename: Option<(TrackId, String)>,
    /// Where each visible header and its name's letters were drawn this frame: the input
    /// reads it (a click on the letters renames), and tests aim with it.
    hits: Vec<HeaderHit>,
    /// A fade or gain handle being dragged, and the handle a menu was opened on.
    audio: audio::edit::State,
    /// Buffers the painters reuse, so a steady frame allocates nothing.
    scratch: Scratch,
    /// The automation lanes: their order, what was drawn, a point being dragged.
    pub auto: autolanes::State,
}

/// What the painters reuse from frame to frame.
#[derive(Debug, Default)]
struct Scratch {
    /// A clip's repetitions of one note.
    at: Vec<Ticks>,
    /// A clip's label, with its "(muted)" when it has one.
    label: String,
    /// "muted", in the interface's language, and the language it was read in.
    muted_word: String,
    locale: Option<crate::i18n::Locale>,
    tints: Tints,
    digits: Digits,
    /// The toolbar's readouts, rebuilt only when what they say changes.
    selected: (usize, String),
    span: (Option<(Ticks, Ticks)>, String),
    /// "Track 5", "Track 6"…: the empty rows below the last track, named as FL names
    /// them, built once per language and count.
    placeholders: Vec<String>,
    /// Audio clips' buffers, and the handles drawn this frame.
    audio: audio::Scratch,
    /// "×2", "×3"…: how many copies share a linked clip (ADR 0037), built once per
    /// language and count.
    counts: Vec<String>,
    /// The height a clip label's words need at full size, this frame.
    label_row: f32,
}

impl Scratch {
    fn refresh_words(&mut self) {
        let now = crate::i18n::locale();
        if self.locale != Some(now) {
            self.locale = Some(now);
            self.muted_word = t("clip.muted_short");
            self.selected.1.clear();
            self.span.1.clear();
            self.placeholders.clear();
            self.counts.clear();
        }
    }

    /// The placeholder name of track number `n` (1-based).
    fn placeholder(&mut self, n: usize) -> &str {
        while self.placeholders.len() < n {
            let k = self.placeholders.len() + 1;
            self.placeholders.push(tf("track.new_name", &[("n", &k)]));
        }
        &self.placeholders[n - 1]
    }
}

/// What a drag on a clip is doing. Decided at the press, from where in the clip the
/// pointer went down and which modifier is held, and then never re-decided — a gesture
/// that changes meaning halfway through is a gesture you cannot aim.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum DragMode {
    /// Move the whole selection along the timeline and across tracks.
    Move,
    /// Trim the left edge: the window moves, the content stays put.
    TrimStart,
    /// Trim the right edge.
    TrimEnd,
    /// Slip: the window stays, the content moves under it.
    Slip,
}

/// A drag in progress. **Nothing is committed until the mouse comes up.**
///
/// The old code issued a merged `SetClipBounds` on every frame of the drag, which meant a
/// cross-track move could not be expressed at all (it is a different command) and undo
/// depended on gesture merging behaving. Previewing instead — drawing the clips where they
/// would land and committing one batch on release — makes move, copy, trim and slip one
/// code path, and gives exactly one undo step whatever the pointer did on the way.
#[derive(Debug)]
struct ClipDrag {
    mode: DragMode,
    /// The clip under the pointer. Trims and slips act on this one; a move acts on the
    /// whole selection.
    anchor: (TrackId, ClipId),
    anchor_start: Ticks,
    anchor_length: Ticks,
    press: egui::Pos2,
    /// Snapped offset along the timeline, updated as the pointer moves.
    delta: Ticks,
    /// How many tracks down (negative: up) the selection would land.
    rows: i32,
    /// Ctrl: leave the originals where they are.
    copy: bool,
}

impl Default for ArrangementView {
    fn default() -> Self {
        ArrangementView {
            tempo: tempo::State::default(),
            markers: markers::State::default(),
            px_per_beat: 26.0,
            scroll_beats: 0.0,
            scroll_y: 0.0,
            // 40 px, not 64. The header used to hold a fader, a pan bar and a meter — a
            // mixer strip inside a track header — and it is why only four tracks fitted on
            // screen. Those live in the inspector now, which is what buys the density.
            track_height: 40.0,
            header_width: HEADER_W,
            divider: None,
            reorder: None,
            selecting: None,
            looping: None,
            // A bar, which is where clips land nine times out of ten. The toolbar's grid
            // picker is there for the tenth.
            grid: Ticks::beats(4),
            snap: true,
            drag: None,
            marquee: None,
            menu_clip: None,
            menu_at: None,
            rename_clip: None,
            geometry: None,
            rename: None,
            hits: Vec::new(),
            audio: Default::default(),
            scratch: Scratch::default(),
            auto: Default::default(),
        }
    }
}

/// Where a track header was drawn this frame (round 27, ADR 0051).
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct HeaderHit {
    pub track: TrackId,
    /// The header's row.
    pub row: Rect,
    /// The name's letters, on the name line's height: a click there renames. `None` while
    /// the name is being edited.
    pub letters: Option<Rect>,
}

impl ArrangementView {
    /// Start renaming a track in place (a click on its name's letters does).
    pub fn begin_rename_track(&mut self, id: TrackId, name: String) {
        self.rename = Some((id, name));
    }

    /// Where each visible header and its name's letters were drawn this frame.
    pub fn header_hits(&self) -> &[HeaderHit] {
        &self.hits
    }

    /// The track being renamed, if any.
    pub fn renaming_track(&self) -> Option<TrackId> {
        self.rename.as_ref().map(|(t, _)| *t)
    }

    /// Whether a clip is being renamed.
    pub fn renaming_clip(&self) -> bool {
        self.rename_clip.is_some()
    }

    /// A header being dragged to reorder: the track and the row its drop line marks (its
    /// own row while it is held outside the arrangement: no line).
    pub fn reorder_target(&self) -> Option<(TrackId, usize)> {
        self.reorder
    }

    /// Where the timeline was drawn last frame, or `None` before the first one.
    pub fn geometry(&self) -> Option<TimelineGeometry> {
        self.geometry.clone()
    }

    /// Start renaming a clip in place.
    /// Where an audio clip's fade and gain handles were drawn last frame (published
    /// geometry: tests aim with it), or `None` when the clip was not drawn with them.
    pub fn audio_handles(&self, clip: (TrackId, ClipId)) -> Option<AudioHandles> {
        self.scratch
            .audio
            .handles
            .iter()
            .find(|(r, _)| *r == clip)
            .map(|(_, h)| *h)
    }

    /// Whether a fade or gain handle is being dragged.
    pub fn audio_dragging(&self) -> bool {
        self.audio.drag.is_some()
    }

    pub fn begin_rename_clip(&mut self, clip: (TrackId, ClipId), name: String) {
        self.rename_clip = Some((clip, name));
    }

    /// The rubber band being dragged, if any.
    fn marquee_rect(&self) -> Option<(Pos2, Pos2)> {
        self.marquee
    }

    /// Where a clip would be drawn while a drag is in progress: its start, its length and
    /// how many tracks down it has moved. `None` when this clip is not part of the drag.
    fn preview(&self, selected: bool, r: (TrackId, ClipId)) -> Option<(Ticks, Ticks, i32)> {
        let d = self.drag.as_ref()?;
        match d.mode {
            DragMode::Move => {
                if !selected && r != d.anchor {
                    return None;
                }
                Some((d.delta, Ticks::ZERO, d.rows))
            }
            DragMode::TrimStart if r == d.anchor => Some((d.delta, -d.delta, 0)),
            DragMode::TrimEnd if r == d.anchor => Some((Ticks::ZERO, d.delta, 0)),
            DragMode::Slip if r == d.anchor => Some((Ticks::ZERO, Ticks::ZERO, 0)),
            _ => None,
        }
    }

    /// How far the window's offset has moved in the drag's preview: a slip moves it, and
    /// so does a trim of the edge where the content is anchored (the left edge, or the
    /// right one of a reversed audio clip, whose window plays backwards).
    fn slip_preview(&self, r: (TrackId, ClipId), reversed: bool) -> Ticks {
        match &self.drag {
            Some(d) if d.anchor == r => match (d.mode, reversed) {
                (DragMode::Slip, false) => -d.delta,
                (DragMode::Slip, true) => d.delta,
                (DragMode::TrimStart, false) => d.delta,
                (DragMode::TrimEnd, true) => -d.delta,
                _ => Ticks::ZERO,
            },
            _ => Ticks::ZERO,
        }
    }

    /// The right-edge drag of clip `r` with Alt (round 29, ADR 0061): how far, while it
    /// stretches the clip — the painter previews the stretched waveform.
    pub fn stretch_preview(&self, r: (TrackId, ClipId)) -> Option<Ticks> {
        let d = self.drag.as_ref()?;
        (d.anchor == r && d.mode == DragMode::TrimEnd && self.audio.mods.alt).then_some(d.delta)
    }
}

/// Where the view's regions are this frame.
#[derive(Copy, Clone, Debug)]
struct Frame {
    /// Above the headers, beside the scroll bar and the ruler: the add-track key.
    corner: Rect,
    headers: Rect,
    /// The horizontal scroll and zoom bar, above the ruler (FL).
    hbar: Rect,
    ruler: Rect,
    lanes: Rect,
    /// The vertical bar, right of the lanes, and the corner above it.
    vbar: Rect,
    vcorner: Rect,
    /// The header column's edge.
    split: Rect,
    /// The punch lane's height (zero while punch is off), between the loop lane and the
    /// bar numbers.
    punch_h: f32,
    /// The marker lane's height (zero without markers), under the loop lane (ADR 0060).
    markers_h: f32,
    /// The tempo track's lane, between the ruler and the tracks (empty while hidden).
    tempo: Rect,
}

impl Frame {
    /// The regions, with the marker lane in the ruler and the tempo lane under it (round
    /// 29, ADR 0060).
    fn with_lanes(
        full: Rect,
        header_w: f32,
        punch_h: f32,
        markers_h: f32,
        tempo_h: f32,
        s: &Style,
    ) -> Frame {
        let bar = s.s(BAR);
        let hx = full.left() + header_w;
        let right = full.right() - bar;
        let hbar = Rect::from_min_max(pos2(hx + 1.0, full.top()), pos2(right, full.top() + bar));
        let ruler = Rect::from_min_max(
            pos2(hbar.left(), hbar.bottom()),
            pos2(right, hbar.bottom() + RULER_H + punch_h + markers_h),
        );
        let tempo = Rect::from_min_max(
            pos2(hbar.left(), ruler.bottom()),
            pos2(right, ruler.bottom() + tempo_h),
        );
        let lanes = Rect::from_min_max(
            pos2(hbar.left(), tempo.bottom()),
            pos2(right, full.bottom()),
        );
        Frame {
            corner: Rect::from_min_max(full.min, pos2(hx, ruler.bottom())),
            headers: Rect::from_min_max(pos2(full.left(), tempo.bottom()), pos2(hx, full.bottom())),
            hbar,
            ruler,
            lanes,
            vbar: Rect::from_min_max(pos2(right, tempo.bottom()), full.max),
            vcorner: Rect::from_min_max(
                pos2(right, full.top()),
                pos2(full.right(), tempo.bottom()),
            ),
            split: Rect::from_center_size(
                pos2(hx + 0.5, (full.top() + full.bottom()) * 0.5),
                vec2(s.s(kit::chrome::SPLITTER), full.height()),
            ),
            punch_h,
            markers_h,
            tempo,
        }
    }

    /// The tempo lane's header: the header column beside it.
    fn tempo_head(&self) -> Rect {
        Rect::from_min_max(
            pos2(self.corner.left(), self.tempo.top()),
            pos2(self.corner.right(), self.tempo.bottom()),
        )
    }
}

/// A key's tooltip: the action's name and whatever key is bound to it *right now*
/// (`Keymap::display_for`), so the toolbar teaches the shortcut rather than competing
/// with it. Built only while the key is hovered.
fn tip(app: &CantusApp, action: crate::keymap::Action, label: &str) -> String {
    let keys = app.keymap.display_for(action);
    if keys.is_empty() {
        t(label)
    } else {
        format!("{} ({keys})", t(label))
    }
}

/// The clip toolbar: the same shape as the piano roll's, for the same reason.
///
/// Everything here was already reachable from a menu and a shortcut, and that is not the
/// same as being to hand. A row of keys above the thing they act on is how a musician
/// finds out an operation exists at all.
fn toolbar(app: &mut CantusApp, ui: &mut egui::Ui) {
    use crate::keymap::Action;
    let linked = crate::links::selection_is_linked(app.project(), &app.clip_selection());
    let (muted, looping, color, selected) = {
        let a: &CantusApp = app;
        let first = if a.selected_clips.is_empty() {
            a.selected_track.zip(a.selected_clip)
        } else {
            a.selected_clips.first().copied()
        };
        let n = if a.selected_clips.is_empty() {
            usize::from(first.is_some())
        } else {
            a.selected_clips.len()
        };
        let (muted, looping, color) = first
            .and_then(|(t, c)| a.project().track(t)?.clip(c))
            .map(|c| (c.muted, c.content_length.is_some(), c.color))
            .unwrap_or((false, false, None));
        (muted, looping, color, n)
    };
    let id = ui.id().with("arrangement-toolbar");
    let mut bar = Toolbar::new(ui);
    if let Some(r) = bar.menu(ui, id.with("menu"), "arrangement.menu") {
        egui::Popup::menu(&r)
            .frame(egui::Frame::NONE)
            .show(|ui| timeline_menu(app, ui, false));
    }
    bar.title(ui, Src::Key("arrangement.title"), None);
    bar.divider(ui);
    // `lit` is state, never a press: mute and repeat light up because the clip *is*
    // muted or repeating, and the rest never light up at all.
    for (action, icon, label, lit) in [
        (Action::SplitClip, Icon::Slice, "clip.split", false),
        (Action::GlueClips, Icon::Glue, "clip.glue", false),
        (Action::Duplicate, Icon::Duplicate, "clip.duplicate", false),
        (Action::ToggleClipMute, Icon::Mute, "clip.mute", muted),
        (Action::ToggleClipLoop, Icon::Loop, "clip.loop_on", looping),
    ] {
        let key = bar.key(ui, id.with(label), icon, lit, true, || {
            tip(app, action, label)
        });
        if key.is_some_and(|k| k.clicked()) {
            app.run_action(action);
        }
    }
    // Lit while the selection is a linked clip (ADR 0037); a press makes it unique.
    let link = bar.key(ui, id.with("link"), Icon::Link, linked, linked, || {
        tip(app, Action::MakeUnique, "clip.link_key")
    });
    if link.is_some_and(|k| k.clicked()) {
        app.focus_arrangement();
        app.run_action(Action::MakeUnique);
    }
    // The paint tool (B): lit while it is out; its source is the clip browser's tile.
    let painting = app.clip_browser.painting();
    let paint = bar.key(ui, id.with("paint"), Icon::Paint, painting, true, || {
        tip(app, Action::ToolPaint, "links.paint.key")
    });
    if paint.is_some_and(|k| k.clicked()) {
        app.focus_arrangement();
        app.toggle_arrangement_paint();
    }
    // The colour key shows the colour rather than an icon of one.
    let swatch = bar.swatch(ui, id.with("colour"), color.map(theme::track_color), || {
        tip(app, Action::ClipColorNext, "clip.color")
    });
    if let Some(r) = swatch {
        egui::Popup::menu(&r)
            .frame(chrome::menu_frame())
            .show(|ui| color_menu(app, ui, false));
    }
    let del = bar.key(ui, id.with("delete"), Icon::Delete, false, true, || {
        tip(app, Action::Delete, "clip.delete")
    });
    if del.is_some_and(|k| k.clicked()) {
        app.focus_arrangement();
        app.delete_clip_selection();
    }
    bar.divider(ui);

    // The same list the piano roll offers, with the same names: two grid pickers that
    // disagree about what "1/8" is called would be a small cruelty.
    let grid_key = grid_key(app.arrangement.grid);
    let caption = t("arrangement.grid");
    let field = bar.field(
        ui,
        id.with("grid"),
        Some(&caption),
        &t(grid_key),
        64.0,
        || t("arrangement.grid_tip"),
    );
    if let Some(r) = field {
        egui::Popup::menu(&r).frame(egui::Frame::NONE).show(|ui| {
            if let Some(g) = grid_menu(ui, app.arrangement.grid) {
                app.arrangement.grid = g;
                ui.close();
            }
        });
    }
    let snap = bar.led(
        ui,
        id.with("snap"),
        app.arrangement.snap,
        "pianoroll.snap",
        58.0,
        || t("arrangement.snap_tip"),
    );
    if snap.is_some_and(|r| r.clicked()) {
        app.arrangement.snap = !app.arrangement.snap;
    }

    // The right end: what is selected, and what can be done with a time selection.
    let bars_per = app.project().time_signature.ticks_per_bar().0 as f32;
    let time_selection = app.time_selection;
    // The selection's length in bars, through the metre changes (ADR 0060).
    let span_bars = time_selection.map(|(from, to)| {
        if crate::views::bars::map(app.project()).is_one_metre() {
            (to - from).0 as f32 / bars_per
        } else {
            (crate::views::bars::bar_position(app.project(), to)
                - crate::views::bars::bar_position(app.project(), from)) as f32
        }
    });
    let scratch = &mut app.arrangement.scratch;
    scratch.refresh_words();
    if scratch.selected.0 != selected || scratch.selected.1.is_empty() {
        scratch.selected = (selected, tf("arrangement.selected", &[("n", &selected)]));
    }
    if scratch.span.0 != time_selection || scratch.span.1.is_empty() {
        // The time selection's length in bars, monospaced so it does not shift as it is
        // dragged. Shown even when empty, so the readout is a place the eye learns.
        let text = match time_selection {
            Some((from, to)) => {
                let _ = (from, to);
                let bars = span_bars.unwrap_or(0.0);
                tf("arrangement.selection", &[("bars", &format!("{bars:.2}"))])
            }
            None => t("arrangement.no_selection"),
        };
        scratch.span = (time_selection, text);
    }
    let (sel_text, span_text) = (
        std::mem::take(&mut scratch.selected.1),
        std::mem::take(&mut scratch.span.1),
    );
    bar.readout_right(ui, &sel_text, 84.0, false);
    bar.readout_right(ui, &span_text, 120.0, false);
    app.arrangement.scratch.selected.1 = sel_text;
    app.arrangement.scratch.span.1 = span_text;
    let has_selection = app.time_selection.is_some();
    for (action, icon, key) in [
        (
            Action::LoopToSelection,
            Icon::Loop,
            "action.loop-to-selection",
        ),
        (
            Action::ZoomToSelection,
            Icon::Zoom,
            "action.zoom-to-selection",
        ),
    ] {
        let r = bar.key_right(ui, id.with(key), icon, false, has_selection, || {
            tip(app, action, key)
        });
        if r.is_some_and(|r| r.clicked()) {
            app.run_action(action);
        }
    }
}

/// The i18n key of a grid's name.
fn grid_key(grid: Ticks) -> &'static str {
    GRIDS
        .iter()
        .find(|(_, g)| *g == grid.0)
        .map(|(k, _)| *k)
        .unwrap_or("grid.custom")
}

/// The grid list, as a kit menu: returns the grid chosen.
pub(crate) fn grid_menu(ui: &mut egui::Ui, current: Ticks) -> Option<Ticks> {
    let items: [Item<'_>; 8] = std::array::from_fn(|i| Item::Action {
        label: Src::Key(GRIDS[i].0),
        check: Check::Radio(GRIDS[i].1 == current.0),
        shortcut: None,
        submenu: false,
        enabled: true,
    });
    let (_, clicks) = kit::widgets::menu_list(ui, &items, 132.0);
    clicks.clicked.map(|i| Ticks(GRIDS[i].1))
}

/// The clip palette: a row of swatches and "follow the track".
/// The clip colours: "follow the track", then the palette. In the clip's menu (`panel`)
/// the row is a part of the menu — its panel under it, the swatches at the items' inset
/// (final review: they lay on the window under the menu's last band).
fn color_menu(app: &mut CantusApp, ui: &mut egui::Ui, panel: bool) {
    let s = Style::current();
    if panel {
        let (rect, _) =
            ui.allocate_exact_size(vec2(MENU_W, s.d(kit::menu::ROW + 8.0)), Sense::hover());
        kit::with_brush(ui.painter(), |b| kit::menu::paint(b, rect, &s, &[], None));
        let inner = rect.shrink2(vec2(s.s(8.0), s.s(4.0)));
        let mut row = ui.new_child(
            egui::UiBuilder::new()
                .max_rect(inner)
                .layout(egui::Layout::left_to_right(egui::Align::Center)),
        );
        row.spacing_mut().item_spacing.x = s.s(4.0);
        swatches(app, &mut row, &s);
    } else {
        ui.horizontal(|ui| swatches(app, ui, &s));
    }
}

fn swatches(app: &mut CantusApp, ui: &mut egui::Ui, s: &Style) {
    {
        // First, "follow the track": the swatch with the diagonal, as on the toolbar key.
        let (r, resp) = ui.allocate_exact_size(vec2(s.s(18.0), s.s(18.0)), Sense::click());
        kit::with_brush(ui.painter(), |b| {
            b.register(r, "swatch", &[kit::Tok::Edge, kit::Tok::MenuText], &[]);
            b.rrect_stroke(r.shrink(0.5), s.r(2.0), b.px(), s.t.edge);
            b.segment(r.left_bottom(), r.right_top(), 1.5, s.t.menu_text, false);
            if resp.hovered() {
                b.rrect_stroke(r.expand(1.0), s.r(3.0), 1.5, s.t.menu_text);
            }
        });
        let resp = if resp.hovered() {
            let tip = t("clip.color_reset");
            hint::set_text(ui.ctx(), &tip, "", Caps::NONE);
            resp.on_hover_text(tip)
        } else {
            resp
        };
        if resp.clicked() {
            app.set_clip_color(None);
            ui.close();
        }
        for c in cantus_core::Color::PALETTE {
            let (r, resp) = ui.allocate_exact_size(vec2(s.s(18.0), s.s(18.0)), Sense::click());
            let fill = theme::track_color(c);
            kit::with_brush(ui.painter(), |b| {
                b.register(r, "swatch", &[kit::Tok::Edge, kit::Tok::MenuText], &[]);
                b.rrect(r, s.r(2.0), kit::Fill::Solid(s.t.edge));
                b.rrect(r.shrink(b.px()), s.r(2.0), kit::Fill::Solid(fill));
                if resp.hovered() {
                    b.rrect_stroke(r.expand(1.0), s.r(3.0), 1.5, s.t.menu_text);
                }
            });
            if resp.clicked() {
                app.set_clip_color(Some(c));
                ui.close();
            }
        }
    }
}

pub fn show(app: &mut CantusApp, ui: &mut egui::Ui) {
    // A sample from the browser lands here as an audio clip (ADR 0030).
    crate::samples::drop::publish(
        ui.ctx(),
        ui.max_rect(),
        crate::samples::drop::Target::Arrangement,
    );
    toolbar(app, ui);
    let s = Style::current();
    let full = ui.available_rect_before_wrap();
    if full.width() < 60.0 || full.height() < RULER_H + 2.0 * s.s(BAR) {
        return;
    }
    let punch_h = rec::punch_height(app, &s);
    let markers_h = markers::height(app, &s);
    let tempo_h = tempo::height(app, &s);
    // The automation lanes' order, once per edit.
    let mut auto_st = std::mem::take(&mut app.arrangement.auto);
    auto_st.refresh(app);
    app.arrangement.auto = auto_st;
    let v = &mut app.arrangement;
    let most = HEADER_MAX.min(full.width() * 0.6).max(40.0);
    v.header_width = v.header_width.clamp(HEADER_MIN.min(most), most);
    let f = Frame::with_lanes(full, v.header_width, punch_h, markers_h, tempo_h, &s);
    navigate(app, ui, &f, &s);
    let mut geo = layout(app, &f);
    timeline(app, ui, &f, &geo, &s);
    headers(app, ui, &f, &geo, &s);
    corner(app, ui, &f, &s);
    // The paint tool (B; round 26, ADR 0037), above the timeline while it is out.
    super::clip_browser::paint_tool(app, ui, f.lanes, &geo);
    let v = &mut app.arrangement;
    kit::widgets::splitter(
        ui,
        f.split,
        ui.id().with("arrangement-headers-edge"),
        &mut v.header_width,
        HEADER_W,
        HEADER_MIN..=HEADER_MAX,
    );
    // The geometry is published last, so a click this frame is aimed with what was drawn.
    geo.scroll_beats = v.scroll_beats;
    v.geometry = Some(geo);
}

/// The song's length in beats, rounded up to a bar, plus room to keep writing.
fn song_beats(app: &CantusApp) -> f32 {
    let map = crate::views::bars::map(app.project());
    if !map.is_one_metre() {
        // Through the metre changes (ADR 0060): to the bar line at or after the end, and
        // eight bars more.
        let end = app.project().content_end();
        let at = if map.bar_start(end) == end {
            end
        } else {
            map.bar_end(end)
        };
        let (bar, _, _) = map.bbt(at);
        return ticks_to_beats(map.bar_tick(bar + 8));
    }
    let ts = app.project().time_signature;
    let per_bar = ts.numerator as f32 * 4.0 / ts.denominator as f32;
    let end = ticks_to_beats(app.project().content_end());
    ((end / per_bar).ceil() + 8.0) * per_bar
}

/// The rows' full height at the current zoom.
fn content_height(app: &CantusApp) -> f32 {
    let base = app.arrangement.track_height;
    app.project()
        .tracks
        .iter()
        .map(|t| {
            (base * t.height()).round()
                + takes::height(t, base)
                + super::pitch_lane::height(t, base)
        })
        .sum()
}

/// The wheel, the middle button and the two bars.
fn navigate(app: &mut CantusApp, ui: &mut egui::Ui, f: &Frame, s: &Style) {
    let over_time = ui.rect_contains_pointer(Rect::from_min_max(f.ruler.min, f.lanes.max));
    let over_rows = ui.rect_contains_pointer(f.headers);
    let pointer = ui.input(|i| i.pointer.hover_pos());
    if over_time || over_rows {
        let w = chrome::wheel(ui);
        let v = &mut app.arrangement;
        if w.zoom_x != 1.0 {
            // Zoom about the pointer, so what is under it stays under it.
            let at_x = pointer
                .filter(|_| over_time)
                .map(|p| p.x)
                .unwrap_or(f.lanes.left());
            let anchor = (at_x - f.lanes.left()) / v.px_per_beat + v.scroll_beats;
            v.px_per_beat = (v.px_per_beat * w.zoom_x).clamp(PX_MIN, PX_MAX);
            v.scroll_beats = (anchor - (at_x - f.lanes.left()) / v.px_per_beat).max(0.0);
        }
        if w.zoom_y != 1.0 {
            let at_y = pointer.map(|p| p.y).unwrap_or(f.lanes.top()) - f.lanes.top();
            let before = v.track_height;
            v.track_height = (v.track_height * w.zoom_y).clamp(BASE_MIN, BASE_MAX);
            let k = v.track_height / before;
            v.scroll_y = ((v.scroll_y + at_y) * k - at_y).max(0.0);
        }
        if !w.alt && w.scroll != egui::Vec2::ZERO {
            // Shift+wheel arrives as `x`: there is nothing to ask about Shift.
            v.scroll_beats = (v.scroll_beats - w.scroll.x / v.px_per_beat).max(0.0);
            v.scroll_y -= w.scroll.y;
        }
    }
    // Middle-drag pans both ways, from wherever it started over the view.
    let (middle, origin, delta) = ui.input(|i| {
        (
            i.pointer.middle_down(),
            i.pointer.press_origin(),
            i.pointer.delta(),
        )
    });
    let area = Rect::from_min_max(pos2(f.headers.left(), f.ruler.top()), f.lanes.max);
    if middle && origin.is_some_and(|o| area.contains(o)) && delta != egui::Vec2::ZERO {
        ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
        let v = &mut app.arrangement;
        v.scroll_beats = (v.scroll_beats - delta.x / v.px_per_beat).max(0.0);
        v.scroll_y -= delta.y;
    }

    // The bars: scroll by the thumb, zoom by its ends.
    let total = song_beats(app)
        .max(app.arrangement.scroll_beats + f.lanes.width() / app.arrangement.px_per_beat);
    let v = &mut app.arrangement;
    let visible = f.lanes.width() / v.px_per_beat;
    let span = Span::new(v.scroll_beats, visible, total);
    let zoom = Some((f.lanes.width() / PX_MAX, f.lanes.width() / PX_MIN));
    if let Some(n) = chrome::bar(
        ui,
        f.hbar,
        ui.id().with("arr-hbar"),
        span,
        zoom,
        "views.nav.hbar",
    ) {
        v.scroll_beats = n.start.max(0.0);
        v.px_per_beat = (f.lanes.width() / n.len).clamp(PX_MIN, PX_MAX);
    }
    let content = content_height(app);
    let v = &mut app.arrangement;
    let tail = v.track_height;
    let span = Span::new(v.scroll_y.max(0.0), f.lanes.height(), content + tail);
    // The span is in points at today's row height: showing more of the song means
    // smaller rows, down to `BASE_MIN`.
    let zoom = Some((
        f.lanes.height() * v.track_height / BASE_MAX,
        f.lanes.height() * v.track_height / BASE_MIN,
    ));
    if let Some(n) = chrome::bar(
        ui,
        f.vbar,
        ui.id().with("arr-vbar"),
        span,
        zoom,
        "views.nav.vbar",
    ) {
        let k = span.len / n.len.max(1.0);
        let before = v.track_height;
        v.track_height = (v.track_height * k).clamp(BASE_MIN, BASE_MAX);
        v.scroll_y = n.start * (v.track_height / before);
    }
    let content = content_height(app);
    let v = &mut app.arrangement;
    let most = (content + v.track_height - f.lanes.height()).max(0.0);
    v.scroll_y = v.scroll_y.clamp(0.0, most);
    kit::with_brush(ui.painter(), |b| chrome::corner(b, f.vcorner, s));
}

/// The rows where they are drawn this frame, into the geometry the last frame left.
fn layout(app: &mut CantusApp, f: &Frame) -> TimelineGeometry {
    let v = &mut app.arrangement;
    let mut geo = v.geometry.take().unwrap_or(TimelineGeometry {
        left: 0.0,
        width: 0.0,
        ruler_top: 0.0,
        loop_lane_bottom: 0.0,
        body_top: 0.0,
        px_per_beat: 1.0,
        scroll_beats: 0.0,
        rows: Vec::new(),
        punch: None,
        markers: None,
        tempo: None,
    });
    geo.left = f.lanes.left();
    geo.width = f.lanes.width();
    geo.ruler_top = f.ruler.top();
    geo.loop_lane_bottom = f.ruler.top() + LOOP_LANE_H;
    geo.body_top = f.lanes.top();
    // The marker lane under the loop lane, the punch lane under it (ADR 0060).
    geo.markers =
        (f.markers_h > 0.0).then_some((geo.loop_lane_bottom, geo.loop_lane_bottom + f.markers_h));
    let punch_top = geo.loop_lane_bottom + f.markers_h;
    geo.punch = (f.punch_h > 0.0).then_some((punch_top, punch_top + f.punch_h));
    geo.tempo = (f.tempo.height() > 0.0).then_some((f.tempo.top(), f.tempo.bottom()));
    geo.px_per_beat = v.px_per_beat;
    geo.scroll_beats = v.scroll_beats;
    let (top, base) = (f.lanes.top() - v.scroll_y, v.track_height);
    rows_into(&mut geo.rows, app.project(), top, base);
    geo
}

/// The add-track key, over the headers.
fn corner(app: &mut CantusApp, ui: &mut egui::Ui, f: &Frame, s: &Style) {
    kit::with_brush(ui.painter(), |b| {
        b.register(f.corner, "corner", &[kit::Tok::Panel], &[]);
        b.rect(b.snap_rect(f.corner), kit::Fill::Solid(s.t.panel));
    });
    let key = Rect::from_min_size(
        pos2(f.corner.left() + s.s(6.0), f.corner.bottom() - s.s(28.0)),
        vec2(s.s(26.0), s.s(24.0)),
    );
    let resp = ui.interact(key, ui.id().with("arr-add-track"), Sense::click());
    super::device_chrome::publish(ui.ctx(), super::device_chrome::Spot::AddTrack, key);
    let st = kit::State::of(&resp, false);
    kit::with_brush(ui.painter(), |b| {
        kit::cap::toolbar_key(
            b,
            key,
            s,
            st,
            kit::cap::KeyFace::Glyph(kit::glyph::Glyph::Plus),
        )
    });
    let resp = if resp.hovered() {
        let tip = t("swap.add_track_tip");
        hint::set_text(ui.ctx(), &tip, "", Caps::NONE);
        resp.on_hover_text(tip)
    } else {
        resp
    };
    // Round 20: the key opens the device picker on the instruments; the new track hosts
    // the one chosen.
    if resp.clicked() {
        app.open_picker(crate::picker::PickPurpose::AddInstrument { index: None });
    }
    // Right-click: an instrument from the picker, a plain track, or an audio track.
    egui::Popup::context_menu(&resp)
        .frame(chrome::menu_frame())
        .show(|ui| {
            let items = [
                Item::action(Src::Key("swap.add_instrument")),
                Item::action(Src::Key("swap.add_plain_track")),
                Item::action(Src::Key("rec.add_audio_track")),
            ];
            match kit::widgets::menu_list(ui, &items, 220.0).1.clicked {
                Some(0) => {
                    app.open_picker(crate::picker::PickPurpose::AddInstrument { index: None });
                    ui.close();
                }
                Some(1) => {
                    app.add_track();
                    ui.close();
                }
                Some(2) => {
                    app.add_audio_track();
                    ui.close();
                }
                _ => {}
            }
        });
}

/// The track headers.
///
/// **What is *not* here is the point.** Headers used to carry a fader, a pan bar and a
/// meter — a mixer strip inside a track header, in 64 px, which is why only four tracks
/// fitted on a screen. Those live in the inspector. What is left is what a header is
/// *for*: which track this is (colour, number, icon, name) and whether it is sounding
/// (the mute and solo lamps, a hairline meter) — FL's coloured block, in 40 px.
fn headers(app: &mut CantusApp, ui: &mut egui::Ui, f: &Frame, geo: &TimelineGeometry, s: &Style) {
    let clip = f.headers;
    let p = ui.painter().with_clip_rect(clip);
    let pointer = ui
        .input(|i| i.pointer.hover_pos())
        .filter(|_| ui.rect_contains_pointer(clip));
    let visible = |row: &TrackRow| {
        row.top + row.height + row.takes + row.auto + row.pitch > clip.top()
            && row.top < clip.bottom()
    };
    let row_rect = |row: &TrackRow| {
        Rect::from_min_size(pos2(clip.left(), row.top), vec2(clip.width(), row.height))
    };
    let near = |c: Pos2| pointer.is_some_and(|p| p.distance(c) <= s.s(paint::LED));

    // Paint, with the project borrowed and nothing edited.
    let mut scratch = std::mem::take(&mut app.arrangement.scratch);
    // The name's words at full size need this tall a line; a shorter one scales them.
    let body_row = name_row(ui.ctx(), s);
    let mut hits = std::mem::take(&mut app.arrangement.hits);
    hits.clear();
    {
        let a: &CantusApp = app;
        let any_solo = a.project().tracks.iter().any(|t| t.soloed);
        let bar = crate::views::bars::bar_len(a.project(), a.project().content_end())
            .0
            .max(1) as f32;
        let song_len = ((a.project().content_end().0 as f32 / bar).ceil() * bar).max(bar);
        let meters = a.engine.meters();
        let v = &a.arrangement;
        kit::with_brush(&p, |b| {
            b.register(clip, "headers", &[kit::Tok::Panel], &[]);
            b.rect(b.snap_rect(clip), kit::Fill::Solid(s.t.panel));
            for (i, row) in geo.rows.iter().enumerate() {
                let Some(track) = a.project().tracks.get(i).filter(|_| visible(row)) else {
                    continue;
                };
                let mut hr = paint::header_rects(row_rect(row), s);
                let rr = rec::layout(&mut hr, track, s);
                let tint = scratch.tints.of(theme::track_color(track.color), s);
                let grip = Rect::from_min_max(
                    pos2(hr.row.left(), hr.row.bottom() - DIVIDER_GRIP),
                    pos2(hr.row.right(), hr.row.bottom() + DIVIDER_GRIP),
                );
                let level = if i < meters.track_count {
                    ((cantus_core::linear_to_db(meters.tracks[i]) + 60.0) / 60.0).clamp(0.0, 1.0)
                } else {
                    0.0
                };
                let st = paint::HeaderState {
                    selected: a.selected_track == Some(track.id),
                    hovered_mute: near(hr.mute),
                    hovered_solo: near(hr.solo),
                    silenced: any_solo && !track.soloed,
                    renaming: v.rename.as_ref().is_some_and(|(r, _)| *r == track.id),
                    drop_above: v.reorder.is_some_and(|(d, to)| d != track.id && to == i),
                    divider: v.divider.is_some_and(|(d, _)| d == track.id)
                        || (v.drag.is_none() && pointer.is_some_and(|p| grip.contains(p))),
                    level,
                    fit: (hr.name.height() / body_row).min(1.0),
                };
                let letters =
                    paint::header(b, a, i, &hr, st, tint, song_len, &mut scratch.digits, s);
                hits.push(HeaderHit {
                    track: track.id,
                    row: hr.row,
                    letters,
                });
                rec::paint_header(b, a, track, &rr, tint, pointer, s);
                takes::paint_headers(b, a, track, row, clip, tint, s);
                autolanes::paint_headers(b, a, i, track, row, clip, tint, s);
                super::pitch_lane::paint_header(b, row, clip, tint.large, s);
            }
            // Below the last track, FL's empty rows: "Track 5" in a placeholder's faint
            // text on a flat well, so the column reads as a list with room in it.
            let base = v.track_height;
            let mut top = geo.bottom();
            let mut n = geo.rows.len() + 1;
            scratch.refresh_words();
            while top < clip.bottom() && base >= 8.0 {
                let row = Rect::from_min_size(pos2(clip.left(), top), vec2(clip.width(), base));
                if row.bottom() > clip.top() {
                    let label = scratch.placeholder(n);
                    paint::empty_header(b, row, label, s);
                }
                top += base;
                n += 1;
            }
        });
        // The icons are the house's vector icons, drawn over the headers' mesh; the one
        // under the pointer is raised into the instrument chip that swaps it (round 20).
        for (i, row) in geo.rows.iter().enumerate() {
            let Some(track) = a.project().tracks.get(i).filter(|_| visible(row)) else {
                continue;
            };
            let hr = paint::header_rects(row_rect(row), s);
            let tint = scratch.tints.of(theme::track_color(track.color), s);
            let over = pointer.is_some_and(|q| hr.icon.expand(s.s(3.0)).contains(q));
            kit::with_brush(&p, |b| {
                super::device_chrome::chip_plate(b, hr.icon, over, tint.on_large, s.k)
            });
            icons::paint_track_icon(&p, hr.icon, track.icon(), tint.on_large);
            // A hosted instrument that stopped working (round 29, ADR 0063): its sign, and
            // why as the icon's tip (review #34).
            super::plugin_notice::paint_header_stopped(&p, hr.icon, a, track.id, s);
            if over && a.plugins.instrument_crashed(track.id).is_some() {
                let tip = super::plugin_notice::tip(
                    a,
                    super::plugin_notice::Target::Instrument(track.id),
                );
                let heading = crate::i18n::t("isolation.stopped_card");
                crate::kit::hint::set_text(p.ctx(), &heading, &tip, crate::kit::hint::Caps::NONE);
            }
        }
    }
    app.arrangement.scratch = scratch;
    app.arrangement.hits = hits;

    // Input.
    let base = app.arrangement.track_height;
    let mut to_delete = None;
    let mut open_name = None;
    for (i, row) in geo.rows.iter().enumerate() {
        if !visible(row) {
            continue;
        }
        let Some((id, muted, soloed, height)) = app
            .project()
            .tracks
            .get(i)
            .map(|t| (t.id, t.muted, t.soloed, t.height()))
        else {
            continue;
        };
        let mut hr = paint::header_rects(row_rect(row), s);
        let rr = app
            .project()
            .tracks
            .get(i)
            .map(|t| rec::layout(&mut hr, t, s));
        // The interactive area is shifted down by the grip: a row owns the divider *below*
        // it and not the one above, so a press exactly on the line goes to the row whose
        // height it changes. With the plain rect, both rows contained the line and the
        // later one won, which turned every divider drag into a reorder of the row below.
        let hit = Rect::from_min_max(
            pos2(hr.row.left(), row.top + DIVIDER_GRIP),
            pos2(hr.row.right(), row.top + row.height + DIVIDER_GRIP),
        )
        .intersect(clip);
        let resp = ui.interact(
            hit,
            ui.id().with(("arr-header", id.0)),
            Sense::click_and_drag(),
        );
        let lamp = |c: Pos2| Rect::from_center_size(c, egui::Vec2::splat(s.s(paint::LED + 6.0)));
        let m = ui.interact(
            lamp(hr.mute),
            ui.id().with(("arr-mute", id.0)),
            Sense::click(),
        );
        let so = ui.interact(
            lamp(hr.solo),
            ui.id().with(("arr-solo", id.0)),
            Sense::click(),
        );
        // Round 27 (ADR 0051): the name's letters rename on a click — an I-beam over them
        // and the hint says so; the rest of the header opens the instrument on a
        // double-click. The lamps, the icon and the arm lamp write their own hints after.
        let letters = app
            .arrangement
            .hits
            .iter()
            .find(|h| h.track == id)
            .and_then(|h| h.letters);
        // The letters are a widget of their own, sensed after this row and the rows above:
        // on a short row they reach into the row above's band and this row's resize grip,
        // and there too a click on them is this track's name (review 1).
        let name = letters.map(|l| {
            ui.interact(
                l.intersect(clip),
                ui.id().with(("arr-name", id.0)),
                Sense::click_and_drag(),
            )
        });
        let on_name = name.as_ref().is_some_and(|n| n.hovered());
        if (resp.hovered() || on_name)
            && app.arrangement.drag.is_none()
            && app.arrangement.reorder.is_none()
        {
            let tip = if on_name {
                ui.ctx().set_cursor_icon(egui::CursorIcon::Text);
                "track.header.name_tip"
            } else {
                "track.header.tip"
            };
            if let Some(t) = app.project().track(id) {
                let caps = Caps {
                    menu: true,
                    typed: on_name,
                    ..Caps::NONE
                };
                hint::set_text(ui.ctx(), &t.name, chrome::word(tip), caps);
            }
        }
        for (r, key) in [(&m, "track.mute_tip"), (&so, "track.solo_tip")] {
            if r.hovered() {
                hint::set_text(ui.ctx(), chrome::word(key), "", Caps::NONE);
            }
        }
        // MIDI learn mode (round 29, ADR 0062): the lamps are learnable.
        crate::controllers::learn_spot(
            app,
            ui,
            m.rect,
            cantus_core::controller::MappingTarget::TrackMute(id),
        );
        crate::controllers::learn_spot(
            app,
            ui,
            so.rect,
            cantus_core::controller::MappingTarget::TrackSolo(id),
        );
        if m.clicked() {
            app.exec(Command::SetTrackMute { id, muted: !muted });
        }
        if so.clicked() {
            app.exec(Command::SetTrackSolo {
                id,
                soloed: !soloed,
            });
        }
        rename_in_place(app, ui, id, hr.name);
        super::device_chrome::chip_input(app, ui, hr.icon, id, s.k);
        // A device dragged from the browsing panel onto the header swaps its instrument.
        let swap = crate::picker::PickPurpose::ReplaceInstrument { track: id };
        super::device_chrome::drop_target(app, ui, &resp, swap);
        if let Some(rr) = &rr {
            rec::header_input(app, ui, id, rr, s);
        }
        takes::header_input(app, ui, id, row, clip, s);
        autolanes::header_input(app, ui, i, id, row, clip);

        let grip = Rect::from_min_max(
            pos2(hr.row.left(), row.top + row.height - DIVIDER_GRIP),
            pos2(hr.row.right(), row.top + row.height + DIVIDER_GRIP),
        );
        if resp.hover_pos().is_some_and(|p| grip.contains(p))
            || app.arrangement.divider.map(|(d, _)| d) == Some(id)
        {
            ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeVertical);
        }
        // A click selects; on the letters it also opens the name, selected, in place (the
        // owner's rule of 2026-10-14: only there, and never by typing). A double-click
        // anywhere else opens the instrument — FL's channel button.
        if resp.clicked() {
            app.select_track(id);
        }
        if name.as_ref().is_some_and(|n| n.clicked()) {
            app.select_track(id);
            // Opened once every row has run: another track's open name commits first,
            // whether its row comes before this one or after.
            open_name = Some(id);
        }
        if resp.double_clicked() {
            app.open_track_instrument(id);
        }
        // A drag from the letters reorders, as from the rest of the header (never a resize:
        // the letters are not the grip).
        let from_name = name.as_ref().filter(|n| {
            n.drag_started_by(PointerButton::Primary)
                || n.dragged_by(PointerButton::Primary)
                || n.drag_stopped()
        });
        let d = from_name.unwrap_or(&resp);
        // A drag that starts on the divider resizes; anywhere else it reorders. Decided at
        // the **press origin**, never at the pointer: egui reports a drag only once the
        // pointer has travelled, by which time it has left the five-pixel grip.
        if d.drag_started_by(PointerButton::Primary) {
            let origin = ui.input(|i| i.pointer.press_origin());
            if from_name.is_none() && origin.is_some_and(|p| grip.contains(p)) {
                app.arrangement.divider = Some((id, height));
            } else {
                app.select_track(id);
                app.arrangement.reorder = Some((id, i));
                // Round 25 (answer 11): the track is also a payload the mixer takes — on a
                // strip it routes there.
                egui::DragAndDrop::set_payload(ui.ctx(), crate::routing_actions::TrackDrag(id));
            }
        }
        if d.dragged_by(PointerButton::Primary) {
            if let Some(pos) = d.interact_pointer_pos() {
                if let Some((did, start_h)) = app.arrangement.divider {
                    if did == id {
                        let origin = ui
                            .input(|i| i.pointer.press_origin())
                            .map(|p| p.y)
                            .unwrap_or(pos.y);
                        let height = start_h + (pos.y - origin) / base;
                        app.exec_merging(
                            Command::SetTrackHeight {
                                id,
                                height: height.clamp(MIN_TRACK_HEIGHT, MAX_TRACK_HEIGHT),
                            },
                            MergeKey::of(&("height", id.0)),
                        );
                    }
                } else if app.arrangement.reorder.map(|(d, _)| d) == Some(id) {
                    // Outside the headers and lanes (held over the mixer, which routes
                    // it): no drop line here — the track would stay where it is.
                    let home = f.headers.union(f.lanes);
                    let to = if home.contains(pos) {
                        geo.rows
                            .iter()
                            .position(|r| pos.y < r.top + r.height * 0.5)
                            .unwrap_or(geo.rows.len().saturating_sub(1))
                    } else {
                        i
                    };
                    app.arrangement.reorder = Some((id, to));
                }
            }
        }
        if d.drag_stopped() {
            app.arrangement.divider = None;
            if let Some((did, to)) = app.arrangement.reorder.take() {
                // Released outside the arrangement (on the mixer, which routed it): the
                // track stays where it is.
                let home = f.headers.union(f.lanes);
                let inside = ui
                    .input(|i| i.pointer.latest_pos())
                    .is_none_or(|p| home.contains(p));
                if did == id && to != i && inside {
                    app.exec(Command::MoveTrack { id: did, index: to });
                }
            }
            app.end_gesture();
        }
        for r in std::iter::once(&resp).chain(name.as_ref()) {
            egui::Popup::context_menu(r)
                .frame(chrome::menu_frame())
                .show(|ui| {
                    track_menu(app, ui, id, &mut to_delete);
                });
        }
    }
    // A click on a name's letters (round 27, ADR 0051 §2): a name still open on another
    // track — its row ran before the click reached its field — commits as a click away
    // would, then this one opens, selected, in the same click.
    if let Some(id) = open_name {
        match app.arrangement.rename.take() {
            Some((rid, text)) if rid != id => commit_track_name(app, rid, &text),
            same => app.arrangement.rename = same,
        }
        if app.arrangement.rename.is_none() {
            if let Some(name) = app.project().track(id).map(|t| t.name.clone()) {
                app.arrangement.rename = Some((id, name));
            }
        }
    }
    if let Some(id) = to_delete {
        app.delete_track(id);
    }
}

/// Rename `id` to `text` (trimmed), unless that is empty or its name already: one
/// `SetTrackName`, one undo step; an untouched edit runs nothing (ADR 0024 §15).
fn commit_track_name(app: &mut CantusApp, id: TrackId, text: &str) {
    let new_name = text.trim();
    if !new_name.is_empty() && app.project().track(id).is_some_and(|t| t.name != new_name) {
        app.exec(Command::SetTrackName {
            id,
            name: new_name.to_string(),
        });
    }
}

/// The name editor over a header's name, while that track is being renamed.
fn rename_in_place(app: &mut CantusApp, ui: &mut egui::Ui, id: TrackId, at: Rect) {
    let Some((rid, name)) = app.arrangement.rename.as_mut().filter(|(r, _)| *r == id) else {
        return;
    };
    let rid = *rid;
    // The kit's field (round 26): the name selected as it opens; Enter or a click
    // elsewhere renames, Esc leaves the name as it was.
    // Its words as large as the line allows: on a short row a full-size field overflowed
    // into the rows round it.
    let fit = (at.height() / name_row(ui.ctx(), &Style::current())).min(1.0);
    let out = kit::field::Field::new(egui::Id::new(("track-rename", rid.0)))
        .scale(fit)
        .opens_when_shown()
        .show(ui, at, name);
    if out.event == Some(kit::field::Event::Cancel) {
        app.arrangement.rename = None;
        return;
    }
    if out.event == Some(kit::field::Event::Commit) {
        if let Some((_, text)) = app.arrangement.rename.take() {
            commit_track_name(app, rid, &text);
        }
    }
}

/// The track's own menu, in the kit's menu: height presets, an icon, and delete.
///
/// Presets as well as the divider because a divider is for *aiming* and a preset is for
/// *agreeing*: getting six tracks to exactly the same height by hand is a job nobody
/// should be given.
fn track_menu(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    id: TrackId,
    to_delete: &mut Option<TrackId>,
) {
    // Recording first: arm, input, takes (`arrangement_rec`).
    if rec::track_menu(app, ui, id) {
        ui.close();
        return;
    }
    // Automation: its fader, pan or level, and its lanes (ADR 0027).
    if autolanes::track_menu(app, ui, id) {
        ui.close();
        return;
    }
    // The pitch lane (round 28, ADR 0054).
    if super::pitch_lane::track_menu(app, ui, id) {
        ui.close();
        return;
    }
    // MIDI controllers (round 29, ADR 0062): learn the track's volume, pan, mute, solo, arm.
    if crate::controllers::track_menu(app, ui, id) {
        ui.close();
        return;
    }
    let current = app.project().track(id).map(|t| t.height()).unwrap_or(1.0);
    let width = 220.0;
    let heights: [Item<'_>; 6] = [
        Item::Band(Src::Key("track.height")),
        radio(
            TRACK_HEIGHTS[0].0,
            (current - TRACK_HEIGHTS[0].1).abs() < 0.01,
        ),
        radio(
            TRACK_HEIGHTS[1].0,
            (current - TRACK_HEIGHTS[1].1).abs() < 0.01,
        ),
        radio(
            TRACK_HEIGHTS[2].0,
            (current - TRACK_HEIGHTS[2].1).abs() < 0.01,
        ),
        radio(
            TRACK_HEIGHTS[3].0,
            (current - TRACK_HEIGHTS[3].1).abs() < 0.01,
        ),
        Item::action(Src::Key("track.height_all")),
    ];
    match kit::widgets::menu_list(ui, &heights, width).1.clicked {
        Some(i @ 1..=4) => {
            app.exec(Command::SetTrackHeight {
                id,
                height: TRACK_HEIGHTS[i - 1].1,
            });
            ui.close();
        }
        Some(5) => {
            app.set_all_track_heights(current);
            ui.close();
        }
        _ => {}
    }
    kit::widgets::menu_list(ui, &[Item::Band(Src::Key("track.icon"))], width);
    let s = Style::current();
    let chosen = app.project().track(id).and_then(|t| t.icon);
    ui.horizontal(|ui| {
        for icon in cantus_core::TrackIcon::ALL {
            let (rect, resp) = ui.allocate_exact_size(vec2(s.s(26.0), s.s(26.0)), Sense::click());
            let on = chosen == Some(icon);
            let st = kit::State::of(&resp, on);
            kit::with_brush(ui.painter(), |b| {
                kit::cap::toolbar_key(b, rect, &s, st, kit::cap::KeyFace::Text(""))
            });
            let ink = if on {
                s.t.highlight_glyph
            } else if resp.hovered() {
                s.t.text
            } else {
                s.t.text_dim
            };
            icons::paint_track_icon(ui.painter(), rect.shrink(s.s(6.0)), icon, ink);
            if resp.on_hover_text(t(icon.label_key())).clicked() {
                app.exec(Command::SetTrackIcon {
                    id,
                    icon: Some(icon),
                });
                ui.close();
            }
        }
    });
    let rest = [
        Item::action(Src::Key("track.icon_auto")),
        Item::Separator,
        Item::action(Src::Key("track.delete")),
    ];
    match kit::widgets::menu_list(ui, &rest, width).1.clicked {
        Some(0) => {
            app.exec(Command::SetTrackIcon { id, icon: None });
            ui.close();
        }
        Some(2) => {
            *to_delete = Some(id);
            ui.close();
        }
        _ => {}
    }
}

/// A menu row with a radio mark.
fn radio(key: &str, on: bool) -> Item<'_> {
    Item::Action {
        label: Src::Key(key),
        check: Check::Radio(on),
        shortcut: None,
        submenu: false,
        enabled: true,
    }
}

/// The height presets, as multiples of the base row. Small enough to scan a long song,
/// tall enough to read a clip's contents, and two steps in between.
pub const TRACK_HEIGHTS: [(&str, f32); 4] = [
    ("track.height_small", 0.6),
    ("track.height_medium", 1.0),
    ("track.height_large", 1.8),
    ("track.height_huge", 3.0),
];

/// Where the timeline was drawn, in screen coordinates.
///
/// Recorded every frame so that anything outside `show` — a test aiming at a clip, and
/// later anything that wants to scroll the view to a tick — can map between ticks and
/// pixels by *asking* rather than by mirroring the layout constants. A test that
/// recomputes the layout is a test that breaks the day a toolbar is added, which is
/// exactly what happened.
#[derive(Clone, Debug, PartialEq)]
pub struct TimelineGeometry {
    pub left: f32,
    /// How wide the timeline was drawn. Zoom-to-selection needs it and cannot recompute
    /// it: the panel's width is whatever the layout gave it this frame.
    pub width: f32,
    /// The top of the ruler; the loop lane runs from here to `loop_lane_bottom`, the
    /// selection lane from there to `body_top`.
    pub ruler_top: f32,
    pub loop_lane_bottom: f32,
    pub body_top: f32,
    pub px_per_beat: f32,
    pub scroll_beats: f32,
    /// Where each track's row was drawn — scrolled, so a row above the view has a top
    /// above `body_top`. Rows are not all the same size, so a single `track_height` cannot
    /// answer "where is row 3", and every caller that multiplied would get it wrong.
    pub rows: Vec<TrackRow>,
    /// The punch lane's top and bottom, while punch is on (ADR 0025): the red brace.
    pub punch: Option<(f32, f32)>,
    /// The marker lane's top and bottom, while the song has markers (ADR 0060).
    pub markers: Option<(f32, f32)>,
    /// The tempo track's top and bottom, while it is shown (ADR 0060).
    pub tempo: Option<(f32, f32)>,
}

#[derive(Copy, Clone, Debug, PartialEq)]
pub struct TrackRow {
    pub top: f32,
    /// The playing lane's height: the track's row.
    pub height: f32,
    /// The take lanes' height below it, while they are shown (zero otherwise).
    pub takes: f32,
    /// The automation lanes' height below those (ADR 0027), while shown.
    pub auto: f32,
    /// The pitch lane's height below those (round 28, ADR 0054), while shown.
    pub pitch: f32,
}

impl TimelineGeometry {
    /// Screen x of a position in beats.
    pub fn x(&self, beats: f32) -> f32 {
        self.left + (beats - self.scroll_beats) * self.px_per_beat
    }

    /// Position in beats of a screen x.
    pub fn beats(&self, x: f32) -> f32 {
        (x - self.left) / self.px_per_beat + self.scroll_beats
    }

    /// Screen y of the middle of a track's row.
    pub fn row_y(&self, row: usize) -> f32 {
        match self.rows.get(row) {
            Some(r) => r.top + r.height * 0.5,
            None => self.body_top,
        }
    }

    /// Which row a screen y falls in.
    pub fn row_at(&self, y: f32) -> Option<usize> {
        self.rows
            .iter()
            .position(|r| y >= r.top && y < r.top + r.height)
    }

    /// One past the bottom of the last row.
    pub fn bottom(&self) -> f32 {
        self.rows
            .last()
            .map(|r| r.top + r.height + r.takes + r.auto + r.pitch)
            .unwrap_or(self.body_top)
    }
}

/// Where each track's row sits, given the base row height. One function, so the headers
/// and the timeline can never disagree about which stripe belongs to which track — which
/// they would the first time one of them forgot that heights vary.
pub fn rows_of(project: &cantus_core::Project, top: f32, base: f32) -> Vec<TrackRow> {
    let mut rows = Vec::with_capacity(project.tracks.len());
    rows_into(&mut rows, project, top, base);
    rows
}

/// [`rows_of`] into a buffer the caller keeps, so a frame allocates nothing.
fn rows_into(rows: &mut Vec<TrackRow>, project: &cantus_core::Project, top: f32, base: f32) {
    rows.clear();
    let mut y = top;
    for t in &project.tracks {
        let height = (base * t.height()).round();
        let takes = takes::height(t, base);
        let auto = autolanes::height(t, base);
        let pitch = super::pitch_lane::height(t, base);
        rows.push(TrackRow {
            top: y,
            height,
            takes,
            auto,
            pitch,
        });
        y += height + takes + auto + pitch;
    }
}

/// Hit test result: which clip the pointer is over, and where in it.
#[derive(Copy, Clone, Debug, PartialEq)]
struct Hit {
    track: TrackId,
    clip: ClipId,
    mode: DragMode,
    /// An audio clip's fade or gain handle, which a press there drags instead.
    handle: Option<AudioHandle>,
}

/// How wide the trim zone at each end of a clip is. Wide enough to aim at, never more
/// than a third of the clip, so a very short clip can still be moved.
fn edge_zone(width: f32) -> f32 {
    (width / 3.0).min(8.0)
}

/// What the clip pass found under the pointer, now and at the press.
#[derive(Default)]
struct ClipHits {
    hover: Option<Hit>,
    at_press: Option<Hit>,
    /// Where the clip being renamed was drawn.
    renaming: Option<Rect>,
}

/// Paint every clip that is on screen — its chrome and, in one pass over its notes, the
/// miniature — and hit-test the pointer against them. Rows and clips off screen cost a
/// comparison each; notes past the right edge are not visited (notes are sorted).
#[allow(clippy::too_many_arguments)]
fn clips(
    b: &mut kit::Brush,
    app: &CantusApp,
    geo: &TimelineGeometry,
    lanes: Rect,
    scratch: &mut Scratch,
    auto_st: &mut autolanes::State,
    pointer: Option<Pos2>,
    press: Option<Pos2>,
    s: &Style,
) -> ClipHits {
    let v = &app.arrangement;
    let mut out = ClipHits::default();
    // Where audio plays on the timeline (ADR 0060).
    let tempo_map = crate::views::bars::map(app.project());
    audio::begin_frame(&mut scratch.audio);
    scratch.refresh_words();
    // Linked clips (ADR 0037): the groups' sizes and the selection's groups, once.
    let marks = crate::links::LinkMarks::of(app.project(), &app.clip_selection());
    let mut counts = std::mem::take(&mut scratch.counts);
    let n_rows = geo.rows.len() as i32;
    let (vis_l, vis_r) = (geo.beats(lanes.left()), geo.beats(lanes.right()));
    for (i, track) in app.project().tracks.iter().enumerate() {
        let Some(here) = geo.rows.get(i).copied() else {
            continue;
        };
        let track_color = theme::track_color(track.color);
        for clip in &track.clips {
            let r = (track.id, clip.id);
            let sel = paint::is_selected(app, r);
            let (dstart, dlen, drows) = v.preview(sel, r).unwrap_or((Ticks::ZERO, Ticks::ZERO, 0));
            // A clip being dragged across tracks takes the *target* row's shape: a
            // preview that kept its old height would be a lie about where it goes.
            let target =
                if let Some(lane) = autolanes::clip_row(auto_st, v.track_height, i, &here, clip) {
                    // An automation clip: in its lane, which a drag never leaves (ADR 0027).
                    match lane {
                        Some(r) => r,
                        None => continue,
                    }
                } else if clip.lane > 0 {
                    // A take lane's clip: drawn in its lane under the track, when shown.
                    match takes::lane_row(&here, clip.lane, v.track_height) {
                        Some(r) => r,
                        None => continue,
                    }
                } else {
                    geo.rows
                        .get((i as i32 + drows).clamp(0, n_rows - 1) as usize)
                        .copied()
                        .unwrap_or(here)
                };
            if target.top + target.height < lanes.top() || target.top > lanes.bottom() {
                continue;
            }
            let start = clip.start + dstart;
            let length = (clip.length + dlen).max(Ticks::SIXTEENTH);
            let x0 = geo.x(ticks_to_beats(start));
            let x1 = geo.x(ticks_to_beats(start + length));
            if x1 < lanes.left() || x0 > lanes.right() {
                continue;
            }
            // The chrome scales with the row: at 24 px there is no room for a margin.
            let inset = (target.height * 0.04).clamp(1.0, 2.0);
            let body = Rect::from_min_max(
                pos2(x0, target.top + inset),
                pos2(x1.max(x0 + 2.0), target.top + target.height - inset),
            );
            let color = clip.color.map(theme::track_color).unwrap_or(track_color);
            let renaming = v.rename_clip.as_ref().is_some_and(|(c, _)| *c == r);
            let reserve = match clip.audio.as_ref() {
                Some(a) => audio::badge_width(b, a, &mut scratch.audio, s),
                None => 0.0,
            };
            let look = paint::ClipLook {
                selected: sel,
                muted: clip.muted,
                tint: scratch.tints.of(color, s),
                renaming,
                reserve,
                sibling: marks.is_sibling(clip, sel),
                label_row: scratch.label_row,
            };
            let linked = marks.count(clip).map(|n| {
                while counts.len() < n {
                    let k = counts.len() + 1;
                    counts.push(tf("links.count", &[("n", &k)]));
                }
                counts[n - 1].as_str()
            });
            let name = paint::clip_label(scratch, &clip.name, clip.muted);
            let inner = paint::clip_chrome(b, body, lanes, name, linked, look, s);
            if renaming {
                out.renaming = Some(body);
            }
            // An audio clip: its waveform, fades, gain and handles (ADR 0025).
            let audio_h = clip.audio.as_ref().and_then(|region| {
                let dragging = v.audio.drag.as_ref().filter(|d| d.r == r).map(|d| d.handle);
                let shown = audio::Shown {
                    track: track.id,
                    clip,
                    region,
                    start,
                    length,
                    offset: clip.offset + v.slip_preview(r, region.reverse),
                    map: &tempo_map,
                };
                let near = pointer.is_some_and(|p| body.contains(p));
                let show = sel || near || dragging.is_some();
                audio::paint(
                    b,
                    app,
                    geo,
                    lanes,
                    &shown,
                    body,
                    inner,
                    &look,
                    pointer,
                    dragging,
                    show,
                    &mut scratch.audio,
                    s,
                )
            });
            // The miniature, in the clip's own content frame so a slipped, trimmed or
            // repeating clip shows what it actually plays. Geometry only: cloning the clip
            // here would copy every note in it, once per clip, every frame.
            let shown = cantus_core::ClipWindow {
                start,
                length,
                offset: clip.offset + v.slip_preview(r, false),
                content_length: clip.content_length,
            };
            // An automation clip's body is its curve (ADR 0027).
            if clip.automation.is_some() {
                let ground = if s.t.dark {
                    s.t.row_black
                } else {
                    s.t.row_white
                };
                autolanes::paint_body(
                    b,
                    auto_st,
                    r,
                    clip,
                    shown,
                    inner,
                    body,
                    lanes,
                    geo,
                    paint::note_ink(&look, s),
                    ground,
                    sel,
                    pointer,
                    s,
                );
            }
            if !clip.notes.is_empty() && inner.height() > 2.0 && inner.width() > 4.0 {
                let (lo, hi) = clip.notes.iter().fold((127u8, 0u8), |(lo, hi), n| {
                    (lo.min(n.pitch), hi.max(n.pitch))
                });
                let span = (hi - lo).max(12) as f32;
                let ink = paint::note_ink(&look, s);
                let half = (inner.height() / span * 0.5).clamp(0.75, 2.0);
                let (left, right) = (
                    inner.left().max(lanes.left()),
                    inner.right().min(lanes.right()),
                );
                let last_on = beats_to_ticks(vis_r.max(0.0)) - start + shown.offset;
                // Notes are sorted by start: a clip that does not repeat starts the pass
                // at the first note that can reach the left edge (a long lookback, for
                // held notes), so scrolling far into a long clip costs no more.
                let skip = if shown.period().is_none() {
                    let first_on =
                        beats_to_ticks(vis_l.max(0.0)) - start + shown.offset - Ticks::beats(64);
                    clip.notes.partition_point(|n| n.start < first_on)
                } else {
                    0
                };
                for n in &clip.notes[skip..] {
                    if shown.period().is_none() && n.start > last_on {
                        break; // sorted by start: the rest are off the right edge
                    }
                    shown.timeline_positions(n.start, &mut scratch.at);
                    for on in scratch.at.iter().copied() {
                        let b0 = ticks_to_beats(on);
                        let b1 = ticks_to_beats(on + n.length);
                        if b1 < vis_l || b0 > vis_r {
                            continue;
                        }
                        let (nx0, nx1) = (geo.x(b0).max(left), geo.x(b1).min(right));
                        if nx1 <= nx0 {
                            continue;
                        }
                        let ny = inner.bottom() - (n.pitch - lo) as f32 / span * inner.height();
                        b.rect(
                            Rect::from_min_max(
                                pos2(nx0, ny - half),
                                pos2(nx1.max(nx0 + 1.0), ny + half),
                            ),
                            kit::Fill::Solid(ink),
                        );
                    }
                }
            }
            // Repeat marks: a hairline where the content starts again.
            if let Some(period) = shown.period() {
                let mut d = period;
                while d < length {
                    let x = geo.x(ticks_to_beats(start + d));
                    if x > lanes.right() {
                        break;
                    }
                    b.vline(
                        x,
                        inner.top(),
                        body.bottom() - 1.0,
                        b.px(),
                        kit::color::with_alpha(paint::note_ink(&look, s), 0.55),
                    );
                    d += period;
                }
            }
            let test = |pos: Pos2| -> Option<Hit> {
                if clip.lane > 0 || !body.contains(pos) || !lanes.contains(pos) {
                    return None;
                }
                if let Some(handle) = audio_h.and_then(|h| h.at(pos)) {
                    return Some(Hit {
                        track: track.id,
                        clip: clip.id,
                        mode: DragMode::Move,
                        handle: Some(handle),
                    });
                }
                let zone = edge_zone(body.width());
                let mode = if pos.x < body.left() + zone {
                    DragMode::TrimStart
                } else if pos.x > body.right() - zone {
                    DragMode::TrimEnd
                } else {
                    DragMode::Move
                };
                Some(Hit {
                    track: track.id,
                    clip: clip.id,
                    mode,
                    handle: None,
                })
            };
            if let Some(h) = pointer.and_then(test) {
                out.hover = Some(h);
            }
            if let Some(h) = press.and_then(test) {
                out.at_press = Some(h);
            }
        }
    }
    scratch.counts = counts;
    out
}

fn timeline(app: &mut CantusApp, ui: &mut egui::Ui, f: &Frame, geo: &TimelineGeometry, s: &Style) {
    let rect = Rect::from_min_max(f.ruler.min, f.lanes.max);
    let resp = ui.allocate_rect(rect, Sense::click_and_drag());
    let body_top = f.lanes.top();
    let view = geo;
    let pointer = resp.hover_pos();
    let press_origin = ui.input(|i| i.pointer.press_origin());

    // ---- paint: the ruler, the lanes and the clips, then what goes over them ----------
    let mut scratch = std::mem::take(&mut app.arrangement.scratch);
    scratch.refresh_words();
    // A clip label's words need this tall a line at full size (short clips scale them).
    scratch.label_row = label_row(ui.ctx(), s);
    let mut auto_st = std::mem::take(&mut app.arrangement.auto);
    auto_st.drawn.clear();
    let lanes_p = ui.painter().with_clip_rect(f.lanes);
    let hits = {
        let a: &CantusApp = app;
        kit::with_brush(&ui.painter().with_clip_rect(f.ruler), |b| {
            paint::ruler(b, a, view, f.ruler, s);
            rec::paint_punch(b, a, view, f.ruler, s);
            if let Some((top, bottom)) = view.markers {
                let lane =
                    Rect::from_min_max(pos2(f.ruler.left(), top), pos2(f.ruler.right(), bottom));
                markers::paint(b, a, view, lane, s);
            }
        });
        // The tempo track under the ruler, its name and tempo in the header column.
        if f.tempo.height() > 0.0 {
            let head = f.tempo_head();
            kit::with_brush(&ui.painter().with_clip_rect(f.tempo.union(head)), |b| {
                tempo::paint(b, a, view, f.tempo, head, s);
            });
        }
        kit::with_brush(&lanes_p, |b| {
            paint::lanes(b, a, view, f.lanes, s);
            takes::grounds(b, a, view, f.lanes, s);
            autolanes::grounds(b, &auto_st, a.arrangement.track_height, view, f.lanes, s);
            super::pitch_lane::paint(b, a, view, f.lanes, s);
            markers::paint_lines(b, a, view, f.lanes, s);
            clips(
                b,
                a,
                view,
                f.lanes,
                &mut scratch,
                &mut auto_st,
                pointer,
                press_origin,
                s,
            )
        })
    };
    app.arrangement.scratch = scratch;
    app.arrangement.auto = auto_st;
    kit::with_brush(&lanes_p, |b| {
        takes::overlay(b, app, view, f.lanes, s);
        rec::paint_live(b, app, view, f.lanes, s);
        paint::overlay(b, app, &app.arrangement, view, f.lanes, s)
    });
    rec::paint_live_waves(&lanes_p, app, view, f.lanes, s);
    if app.project().tracks.is_empty() {
        kit::with_brush(&lanes_p, |b| {
            let tx = kit::text::Text::key("track.add", kit::text::Role::Title, s.t.text_dim);
            let _ = kit::text::paint(b, f.lanes.shrink(20.0), &tx, s.tk());
        });
    }
    let hit = hits.hover;
    let hit_at_press = hits.at_press;

    // Renaming happens on the clip itself rather than in a dialog: the name is a label on
    // a rectangle, and typing it anywhere else means looking away from the thing you are
    // naming. `F2` starts it; Enter and clicking away both commit.
    if let Some(body) = hits.renaming {
        let h = s.s(20.0).min(body.height());
        let well = Rect::from_min_size(
            pos2(body.left() + 2.0, body.top() + 1.0),
            vec2((body.width() - 4.0).max(40.0), h),
        );
        let mut done = None;
        if let Some((target, name)) = app.arrangement.rename_clip.as_mut() {
            let target = *target;
            // The kit's field (round 26): Enter or a click elsewhere renames, Esc leaves
            // the name (an empty name is an abandoned edit to `rename_clip`).
            let out = kit::field::Field::new(egui::Id::new(("clip-rename", target.1 .0)))
                .opens_when_shown()
                .show(ui, well, name);
            match out.event {
                Some(kit::field::Event::Commit) => done = Some((target, name.clone())),
                Some(kit::field::Event::Cancel) => done = Some((target, String::new())),
                None => {}
            }
        }
        if let Some((target, name)) = done {
            app.arrangement.rename_clip = None;
            app.rename_clip(target, name);
        }
    }

    // ---- the pointer's shape, and the hint for where it is ----------------------------
    if let Some(pos) = pointer {
        if pos.y < body_top && app.arrangement.drag.is_none() {
            ui.ctx()
                .set_cursor_icon(if pos.y < rect.top() + LOOP_LANE_H {
                    egui::CursorIcon::ResizeHorizontal
                } else {
                    egui::CursorIcon::Text
                });
            hint::set_text(
                ui.ctx(),
                chrome::word("arrangement.ruler_tip"),
                "",
                Caps::NONE,
            );
        } else if hit.is_none() {
            hint::set_text(ui.ctx(), chrome::word("clip.new_tip"), "", Caps::NONE);
        } else if let Some(h) = hit {
            // The clip's name, then its essentials (the whole list is the idle hint's
            // tooltip): the name reads here even where the clip is too short to show it
            // (round 27, ADR 0051).
            let caps = Caps {
                menu: true,
                ..Caps::NONE
            };
            let tip = chrome::word("arrangement.clip_hint");
            match crate::clips::clip_of(app.project(), (h.track, h.clip))
                .map(|c| c.name.as_str())
                .filter(|n| !n.is_empty())
            {
                Some(name) => hint::set_text(ui.ctx(), name, tip, caps),
                None => hint::set_text(ui.ctx(), tip, "", caps),
            }
        }
    }
    // An audio clip's handle: its own pointer and hint (live while it is dragged).
    if let Some(d) = app.arrangement.audio.drag.as_ref() {
        let (r, handle) = (d.r, d.handle);
        audio::edit::hint(ui.ctx(), app, r, handle, true);
    } else if let Some((h, handle)) = hit.and_then(|h| h.handle.map(|x| (h, x))) {
        ui.ctx().set_cursor_icon(match handle {
            AudioHandle::Gain => egui::CursorIcon::ResizeVertical,
            _ => egui::CursorIcon::ResizeHorizontal,
        });
        audio::edit::hint(ui.ctx(), app, (h.track, h.clip), handle, false);
    } else if let Some(h) = hit {
        if app.arrangement.drag.is_none() {
            ui.ctx().set_cursor_icon(match h.mode {
                DragMode::TrimStart | DragMode::TrimEnd => egui::CursorIcon::ResizeHorizontal,
                _ => egui::CursorIcon::Grab,
            });
        }
    }

    // ---- interaction ------------------------------------------------------------------
    let press = press_origin;
    let mods = ui.input(|i| i.modifiers);
    audio::warp::modifiers(app, mods);
    let track_at = |app: &CantusApp, y: f32| -> Option<TrackId> {
        view.row_at(y)
            .and_then(|i| app.project().tracks.get(i).map(|t| t.id))
    };
    // The arrangement's modifiers are not the piano roll's, and deliberately so. There is
    // no pitch axis here, so Shift and Alt are free for the two things a clip drag needs
    // that a note drag does not — slipping the content, and copying instead of moving —
    // and Alt+drag to copy is what a musician arriving from any of the big four expects.
    // That leaves the grid bypass on Ctrl. Ctrl+*click* still extends the selection:
    // egui only reports a click when the pointer did not travel, so the two never meet.
    let grid = app.arrangement.grid;
    let snap_on = app.arrangement.snap;
    // Through the metre changes (ADR 0060).
    let map = crate::views::bars::map(app.project());
    let one_bar = app.project().time_signature.ticks_per_bar();
    let snap_to = move |t: Ticks, off_grid: bool| {
        if off_grid || !snap_on {
            t
        } else if grid == crate::views::bars::BAR_GRID {
            if map.is_one_metre() {
                t.snap(one_bar)
            } else {
                map.snap_bar(t)
            }
        } else {
            map.snap(t, grid.max(Ticks(1)))
        }
    };

    if resp.clicked() {
        if let Some(pos) = resp.interact_pointer_pos() {
            if pos.y < body_top {
                let beats = view.beats(pos.x).max(0.0);
                app.seek(snap_to(beats_to_ticks(beats), mods.command));
                if pos.y >= rect.top() + LOOP_LANE_H {
                    app.focus_arrangement();
                    app.clear_time_selection();
                }
            } else if let Some(h) = hit {
                // A warp marker clicked is selected (Del removes it; ADR 0061).
                audio::warp::clicked(app, (h.track, h.clip), h.handle);
                app.focus_arrangement();
                if mods.command {
                    app.toggle_clip_selection(h.track, h.clip);
                } else {
                    app.select_clip(h.track, h.clip);
                }
            } else {
                app.focus_arrangement();
                if !mods.command {
                    app.clear_clip_selection();
                }
                if let Some(tid) = track_at(app, pos.y) {
                    app.select_track(tid);
                }
            }
        }
    }
    if resp.double_clicked() {
        if let Some((h, handle)) = hit.and_then(|h| h.handle.map(|x| (h, x))) {
            audio::edit::reset(app, (h.track, h.clip), handle);
        } else if let Some(h) = hit {
            // FL's double-click (round 27, ADR 0051): the clip's editor, on the clip, in
            // front and focused — the piano roll, the channel rack for a step pattern,
            // the automation editor (ADR 0027), the inspector for audio.
            app.open_clip_editor(h.track, h.clip);
        }
        if let Some(pos) = resp.interact_pointer_pos() {
            if pos.y >= body_top && hit.is_none() {
                if let Some(tid) = track_at(app, pos.y) {
                    let beats = view.beats(pos.x).max(0.0);
                    // A bar's start and four bars, through the metre changes (ADR 0060).
                    let start = crate::views::bars::bar_start(app.project(), beats_to_ticks(beats));
                    let end = crate::views::bars::bars_from(app.project(), start, 4);
                    app.create_clip(tid, start, end - start);
                }
            }
        }
    }
    // A context menu, not an immediate delete: right-clicking a clip away with no warning
    // is a hard thing to undo your way out of when it was the wrong clip. It is opened on
    // the clip that was under the pointer when the button went down, and drawn every frame
    // after that whatever the pointer is doing (see `menu_clip`).
    if ui.input(|i| i.pointer.button_pressed(egui::PointerButton::Secondary)) && resp.hovered() {
        app.arrangement.menu_clip = hit.map(|h| (h.track, h.clip));
        app.arrangement.audio.menu = hit.and_then(|h| h.handle);
        app.arrangement.menu_at = match (hit, resp.interact_pointer_pos().or(pointer)) {
            (None, Some(pos)) if pos.y >= body_top => Some((
                beats_to_ticks(view.beats(pos.x).max(0.0)),
                track_at(app, pos.y),
            )),
            _ => None,
        };
    }
    let fade_menu = audio::edit::menu_fade(app.arrangement.audio.menu);
    let menu_target = app
        .arrangement
        .menu_clip
        .filter(|(t, c)| app.project().track(*t).and_then(|t| t.clip(*c)).is_some());
    egui::Popup::context_menu(&resp)
        .frame(chrome::menu_frame())
        .show(|ui| match (menu_target, fade_menu) {
            (Some(r), Some(fade)) => audio::edit::fade_menu(app, ui, r, fade),
            (Some((track, clip)), None) => clip_menu(app, ui, track, clip),
            (None, _) => timeline_menu(app, ui, true),
        });

    if resp.drag_started_by(PointerButton::Primary) {
        if let Some(pos) = press {
            if pos.y < body_top {
                app.arrangement.drag = None;
                let at = snap_to(beats_to_ticks(view.beats(pos.x).max(0.0)), mods.command);
                if pos.y < rect.top() + LOOP_LANE_H {
                    app.arrangement.looping = Some(at);
                } else {
                    app.focus_arrangement();
                    app.arrangement.selecting = Some(at);
                }
            } else if let Some((h, handle)) = hit_at_press.and_then(|h| h.handle.map(|x| (h, x))) {
                // A fade or the gain of an audio clip: shaped live, one undo step.
                app.focus_arrangement();
                if !paint::is_selected(app, (h.track, h.clip)) {
                    app.select_clip(h.track, h.clip);
                }
                app.arrangement.drag = None;
                audio::edit::begin(app, (h.track, h.clip), handle, pos, view.px_per_beat);
            } else if let Some(h) = hit_at_press {
                app.focus_arrangement();
                if !paint::is_selected(app, (h.track, h.clip)) {
                    app.select_clip(h.track, h.clip);
                }
                let clip = app
                    .project()
                    .track(h.track)
                    .and_then(|t| t.clip(h.clip))
                    .map(|c| (c.start, c.length));
                if let Some((anchor_start, anchor_length)) = clip {
                    // Shift turns a move into a slip: the window stays, the music inside
                    // it slides. Alt copies instead of moving; Ctrl ignores the grid.
                    let mode = if mods.shift && h.mode == DragMode::Move {
                        DragMode::Slip
                    } else {
                        h.mode
                    };
                    app.arrangement.drag = Some(ClipDrag {
                        mode,
                        anchor: (h.track, h.clip),
                        anchor_start,
                        anchor_length,
                        press: pos,
                        delta: Ticks::ZERO,
                        rows: 0,
                        copy: mods.alt && mode == DragMode::Move,
                    });
                }
            } else {
                app.arrangement.marquee = Some((pos, pos));
            }
        }
    }
    if resp.dragged_by(PointerButton::Primary) {
        if let Some(pos) = resp.interact_pointer_pos() {
            if app.arrangement.audio.drag.is_some() {
                audio::edit::update(app, pos, mods.shift);
            } else if let Some(d) = app.arrangement.drag.as_ref() {
                let raw = beats_to_ticks((pos.x - d.press.x) / view.px_per_beat);
                let (mode, anchor_start, anchor_length, press_y) =
                    (d.mode, d.anchor_start, d.anchor_length, d.press.y);
                // Snap the *result*, not the movement: the user is aiming the edge at a
                // bar line, not measuring how far they dragged.
                let delta = match mode {
                    DragMode::TrimEnd => {
                        snap_to(anchor_start + anchor_length + raw, mods.command)
                            - (anchor_start + anchor_length)
                    }
                    _ => snap_to(anchor_start + raw, mods.command) - anchor_start,
                };
                // An audio clip's window stays on its file (ADR 0025).
                let anchor = d.anchor;
                let delta = {
                    use crate::audio_clips::{clamp_edit, WindowEdit};
                    let edit = match mode {
                        DragMode::TrimStart => Some(WindowEdit::TrimStart),
                        DragMode::TrimEnd => Some(WindowEdit::TrimEnd),
                        DragMode::Slip => Some(WindowEdit::Slip),
                        DragMode::Move => None,
                    };
                    match (edit, crate::clips::clip_of(app.project(), anchor)) {
                        // Alt on the right edge stretches the clip (ADR 0061): past the file.
                        (Some(WindowEdit::TrimEnd), Some(_)) if audio::warp::stretching(app) => {
                            delta
                        }
                        (Some(e), Some(c)) => clamp_edit(app.project(), c, e, delta),
                        _ => delta,
                    }
                };
                let rows = match mode {
                    DragMode::Move => {
                        let from = view.row_at(press_y).unwrap_or(0) as i32;
                        let to = view.row_at(pos.y).unwrap_or(from as usize) as i32;
                        to - from
                    }
                    _ => 0,
                };
                if let Some(d) = app.arrangement.drag.as_mut() {
                    d.delta = delta;
                    d.rows = rows;
                    d.copy = mods.alt && mode == DragMode::Move;
                }
            } else if let Some(m) = app.arrangement.marquee.as_mut() {
                m.1 = pos;
            } else if let Some(a) = app.arrangement.looping {
                let b = snap_to(beats_to_ticks(view.beats(pos.x).max(0.0)), mods.command);
                let (s, e) = if a <= b { (a, b) } else { (b, a) };
                if e > s {
                    app.exec_merging(
                        Command::SetLoopRegion {
                            region: Some(LoopRegion { start: s, end: e }),
                        },
                        MergeKey::of(&"loop"),
                    );
                }
            } else if let Some(a) = app.arrangement.selecting {
                // A selection is not a command: nothing about the piece changed, so there
                // is nothing to undo. It is the same kind of state as which clips are
                // highlighted.
                let b = snap_to(beats_to_ticks(view.beats(pos.x).max(0.0)), mods.command);
                app.set_time_selection(a, b);
            }
        }
    }
    if resp.drag_stopped() {
        app.arrangement.looping = None;
        app.arrangement.selecting = None;
        audio::edit::end(app);
        if let Some(d) = app.arrangement.drag.take() {
            app.commit_clip_drag(
                d.anchor,
                match d.mode {
                    DragMode::Move => ClipDragResult::Move {
                        delta: d.delta,
                        rows: d.rows,
                        copy: d.copy,
                    },
                    DragMode::TrimStart => ClipDragResult::TrimStart { delta: d.delta },
                    DragMode::TrimEnd => ClipDragResult::TrimEnd { delta: d.delta },
                    DragMode::Slip => ClipDragResult::Slip { delta: d.delta },
                },
            );
        } else if let Some((a, b)) = app.arrangement.marquee.take() {
            let band = Rect::from_two_pos(a, b);
            let mut picked = Vec::new();
            let base = app.arrangement.track_height;
            for (i, track) in app.project().tracks.iter().enumerate() {
                let Some(r) = view.rows.get(i).copied() else {
                    continue;
                };
                let track_row = Rect::from_min_max(
                    pos2(rect.left(), r.top + 2.0),
                    pos2(rect.right(), r.top + r.height - 2.0),
                );
                for clip in track.clips.iter().filter(|c| c.lane == 0) {
                    // Automation clips are picked in their lanes.
                    let row = match autolanes::clip_row(&app.arrangement.auto, base, i, &r, clip) {
                        Some(Some(l)) => Rect::from_min_max(
                            pos2(rect.left(), l.top + 2.0),
                            pos2(rect.right(), l.top + l.height - 2.0),
                        ),
                        Some(None) => continue,
                        None => track_row,
                    };
                    if !band.intersects(row) {
                        continue;
                    }
                    let x0 = view.x(ticks_to_beats(clip.start));
                    let x1 = view.x(ticks_to_beats(clip.end()));
                    if band.intersects(Rect::from_min_max(
                        pos2(x0, row.top()),
                        pos2(x1, row.bottom()),
                    )) {
                        picked.push((track.id, clip.id));
                    }
                }
            }
            app.focus_arrangement();
            app.set_clip_selection(picked);
        }
        app.end_gesture();
    }
    // Registered after the timeline's own response, so they are on top of it: the punch
    // lane's brace and the take lanes' swipes are theirs, not a clip drag's.
    rec::punch_input(app, ui, view, f.ruler, s);
    // The marker lane, the metre chips and the tempo track (round 29, ADR 0060).
    if let Some((top, bottom)) = view.markers {
        let lane = Rect::from_min_max(pos2(f.ruler.left(), top), pos2(f.ruler.right(), bottom));
        markers::input(app, ui, view, lane, s);
    }
    let numbers_top = view
        .punch
        .map(|p| p.1)
        .or(view.markers.map(|m| m.1))
        .unwrap_or(view.loop_lane_bottom);
    let numbers = Rect::from_min_max(pos2(f.ruler.left(), numbers_top), f.ruler.max);
    tempo::metre_input(app, ui, view, numbers, s);
    tempo::input(app, ui, view, f.tempo, s);
    takes::input(app, ui, view, f.lanes, s);
    autolanes::input(app, ui, f.lanes, s);
    super::pitch_lane::input(app, ui, view, f.lanes, s);
}

/// What a finished clip drag asks the application to do.
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum ClipDragResult {
    Move { delta: Ticks, rows: i32, copy: bool },
    TrimStart { delta: Ticks },
    TrimEnd { delta: Ticks },
    Slip { delta: Ticks },
}

/// How wide the view's menus are.
const MENU_W: f32 = 240.0;

/// One entry of a menu of actions: run an action, or something of the view's own.
#[derive(Copy, Clone)]
enum Entry<'a> {
    Run(crate::keymap::Action, &'a str),
    Own(&'a str),
    /// Something of the view's own that does what an action does, somewhere particular:
    /// the action's key is shown, the caller runs it.
    Shown(crate::keymap::Action, &'a str),
    Band(&'a str),
    /// Something of the view's own with a check mark: a setting switched on or off.
    Toggle(&'a str, bool),
    Separator,
}

/// A whole menu as one kit menu — FL 2026's light rows, one frame — with each action's
/// bound key on the right. Returns the entry clicked; an action has already run.
///
/// Going through `run_action` is ADR 0010's rule — a menu and a shortcut must reach the
/// same code — and showing the binding is how the shortcut gets discovered at all.
fn menu(app: &mut CantusApp, ui: &mut egui::Ui, entries: &[Entry<'_>]) -> Option<usize> {
    let keys: Vec<Option<String>> = entries
        .iter()
        .map(|e| match e {
            Entry::Run(a, _) | Entry::Shown(a, _) => {
                app.keymap.bindings_for(*a).first().map(|b| b.display())
            }
            _ => None,
        })
        .collect();
    let items: Vec<Item<'_>> = entries
        .iter()
        .zip(&keys)
        .map(|(e, k)| match *e {
            Entry::Run(_, label) | Entry::Own(label) | Entry::Shown(_, label) => Item::Action {
                label: Src::Key(label),
                check: Check::None,
                shortcut: k.as_deref(),
                submenu: false,
                enabled: true,
            },
            Entry::Band(label) => Item::Band(Src::Key(label)),
            Entry::Toggle(label, on) => Item::Action {
                label: Src::Key(label),
                check: if on { Check::On } else { Check::Off },
                shortcut: None,
                submenu: false,
                enabled: true,
            },
            Entry::Separator => Item::Separator,
        })
        .collect();
    let clicked = kit::widgets::menu_list(ui, &items, MENU_W).1.clicked?;
    if let Entry::Run(a, _) = entries[clicked] {
        app.run_action(a);
    }
    Some(clicked)
}

/// Right-clicking empty timeline, and the view's ▸ menu. Small on purpose: the things
/// worth reaching for there are the clipboard and a new track. `at_pointer` is the
/// right-click (its *Paste* lands where the menu was opened); the ▸ menu pastes at the
/// playhead, as the keys do.
fn timeline_menu(app: &mut CantusApp, ui: &mut egui::Ui, at_pointer: bool) {
    use crate::keymap::Action;
    app.focus_arrangement();
    let entries = [
        Entry::Shown(Action::Paste, "menu.paste"),
        Entry::Run(Action::SelectAll, "menu.select_all"),
        Entry::Separator,
        Entry::Own("swap.add_instrument"),
        Entry::Run(Action::AddAudioTrack, "rec.add_audio_track"),
        Entry::Separator,
        // Whether a paste, Duplicate or Alt-drag makes the same clip again (ADR 0037).
        Entry::Toggle("links.copies", app.settings.link_copies),
    ];
    match menu(app, ui, &entries) {
        Some(0) => {
            if !at_pointer {
                app.arrangement.menu_at = None;
            }
            paste_at_menu_point(app);
            ui.close();
        }
        Some(3) => {
            app.open_picker(crate::picker::PickPurpose::AddInstrument { index: None });
            ui.close();
        }
        Some(6) => {
            app.toggle_link_copies();
            ui.close();
        }
        Some(_) => ui.close(),
        None => {}
    }
    // The tempo track, markers and metre (round 29, ADR 0060): a list of their own.
    markers::timeline_menu(app, ui);
}

/// The empty-timeline menu's *Paste*: at the point the menu was opened on (snapped down to
/// the grid when snap is on), the topmost clip on the track under it — or, with no point,
/// at the playhead as the keys paste. Through `run_action`, like every menu entry
/// (ADR 0010); the point is used once.
pub fn paste_at_menu_point(app: &mut CantusApp) {
    app.clipboard.anchor = app.arrangement.menu_at.take();
    app.focus_arrangement();
    app.run_action(crate::keymap::Action::Paste);
}

/// A menu entry that runs an action, with whatever key is bound to it shown on the
/// right: one row of the kit's menu, for menus built elsewhere (the menu bar's Clip menu).
pub fn action_item(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    action: crate::keymap::Action,
    label_key: &str,
) -> bool {
    let clicked = menu(app, ui, &[Entry::Run(action, label_key)]).is_some();
    clicked
}

/// What opening a clip's right-click menu does before it draws: the clip joins the
/// selection (or becomes it, when it was not in it), the arrangement takes the edit
/// focus, and the clips — not a time range drawn earlier — are what Cut and Copy take.
pub fn clip_menu_opened(app: &mut CantusApp, track: TrackId, clip: ClipId) {
    if !paint::is_selected(app, (track, clip)) {
        app.select_clip(track, clip);
    } else {
        // The clip right-clicked is the one the menu's "this clip" means and the one the
        // keys then act on (Alt+L links the selection to it): the edited clip.
        app.edit_clip_keeping_selection((track, clip));
    }
    app.last_selection = crate::clipboard::SelKind::Clips;
    app.focus_arrangement();
}

/// The clip's own menu. Every entry is something `run_action` can also do, so a shortcut
/// can never reach less than the menu (ADR 0010); the colours sit under it in a row.
fn clip_menu(app: &mut CantusApp, ui: &mut egui::Ui, track: TrackId, clip: ClipId) {
    use crate::keymap::Action;
    clip_menu_opened(app, track, clip);
    // One menu: its sections' panels meet, the colour row's too.
    ui.spacing_mut().item_spacing.y = 0.0;
    let (muted, looping, is_audio) = app
        .project()
        .track(track)
        .and_then(|t| t.clip(clip))
        .map(|c| (c.muted, c.content_length.is_some(), c.audio.is_some()))
        .unwrap_or((false, false, false));
    let mut entries = vec![
        Entry::Run(Action::SplitClip, "clip.split"),
        Entry::Run(Action::GlueClips, "clip.glue"),
        Entry::Run(Action::Duplicate, "clip.duplicate"),
        Entry::Run(Action::Cut, "menu.cut"),
        Entry::Run(Action::Copy, "menu.copy"),
        Entry::Run(
            Action::ToggleClipMute,
            if muted { "clip.unmute" } else { "clip.mute" },
        ),
    ];
    // Linked clips (ADR 0037): a shared clip offers to leave its group and to show the
    // rest of it; a selection of several offers to become this clip.
    let selection = app.clip_selection();
    let linked = crate::links::selection_is_linked(app.project(), &selection);
    if linked {
        entries.push(Entry::Run(Action::MakeUnique, "clip.make_unique"));
        entries.push(Entry::Run(Action::SelectLinked, "clip.select_linked"));
    }
    // The right-clicked clip is the edited clip now (`clip_menu_opened`), so the entry and
    // its key are one action with one target.
    if selection.len() > 1 {
        entries.push(Entry::Run(Action::LinkClips, "clip.link_here"));
    }
    // A group or clip chosen by name (round 26): the picker.
    entries.push(Entry::Run(Action::LinkTo, "clip.link_to"));
    // An audio clip plays its file once: it has no repeat (ADR 0025).
    if !is_audio {
        entries.push(Entry::Run(
            Action::ToggleClipLoop,
            if looping {
                "clip.loop_off"
            } else {
                "clip.loop_on"
            },
        ));
    }
    entries.push(Entry::Separator);
    entries.push(Entry::Own("clip.delete"));
    let delete = entries.len() - 1;
    if !is_audio {
        entries.push(Entry::Band("clip.color"));
    }
    match menu(app, ui, &entries) {
        Some(i) if i == delete => {
            app.delete_clip_selection();
            ui.close();
        }
        Some(_) => ui.close(),
        None => {}
    }
    if is_audio {
        // The audio clip's own section: curves, normalise, gain, reverse, the file.
        audio::edit::clip_menu(app, ui, (track, clip));
        // Warping (round 29, ADR 0061): a list of its own.
        audio::warp::clip_menu(app, ui, (track, clip));
        let _ = menu(app, ui, &[Entry::Band("clip.color")]);
    }
    color_menu(app, ui, true);
    // An automation clip's section: open it, FL's articulators (ADR 0027).
    autolanes::clip_menu(app, ui, (track, clip));
}

/// Normalise the selected audio clips (their loudest sample to −0.1 dBFS, through clip
/// gain); the menu and the inspector both come here. Returns how many were set.
pub fn normalize_selection(app: &mut CantusApp) -> usize {
    audio::edit::normalize(app)
}

/// The height a track name's words need at full size (the body face's row).
fn name_row(ctx: &egui::Context, s: &Style) -> f32 {
    let font = kit::text::Role::Body.font(s.tk());
    ctx.fonts_mut(|f| f.row_height(&font)).max(1.0)
}

/// The height a clip label's words need at full size (the label face's row).
fn label_row(ctx: &egui::Context, s: &Style) -> f32 {
    let font = kit::text::Role::Label.font(s.tk());
    ctx.fonts_mut(|f| f.row_height(&font)).max(1.0)
}
