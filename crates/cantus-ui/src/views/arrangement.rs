//! Arrangement view — FL's playlist, drawn with the kit (ADR 0024): track headers on the
//! left as coloured blocks, a timeline with clips on the right, the ruler with the loop
//! brace and the time selection above, and a scroll bar on each axis that is also a zoom.
//!
//! It scrolls both ways (it could not scroll vertically before, `cantus-audit.md` #3),
//! Shift+wheel scrolls time (#4), Ctrl+wheel zooms time and Ctrl+Alt+wheel the rows,
//! the middle button pans, and the header column's edge can be dragged. The painting is
//! in [`paint`]; this file is the state, the layout and the input.

use super::clips_chrome::{self as chrome, Digits, Tints};
use super::{beats_to_ticks, ticks_to_beats, GRIDS};
use crate::app::CantusApp;

use crate::i18n::{t, tf};
use crate::kit::hint::{self, Caps};
use crate::kit::menu::{Check, Item};
use crate::kit::text::Src;
use crate::kit::{self, Style};
use crate::theme;
use cantus_core::{ClipId, Command, LoopRegion, MergeKey, Ticks, TrackId};
use egui::{pos2, vec2, PointerButton, Pos2, Rect, Sense};

#[path = "arrangement_audio.rs"]
mod audio;
pub use self::audio::reversed_frame;
pub use audio::edit::CURVE_KEYS;
pub use audio::warp::Reversal;
pub use audio::{curve_icon, Handle as AudioHandle, Handles as AudioHandles};
// The warp editor reads clips the way the arrangement does (round 29, ADR 0061).
pub use audio::{warp as warp_api, Shown as AudioShown};

/// Automation lanes and the curves in their clips (ADR 0027).
#[path = "arrangement_auto.rs"]
pub mod autolanes;
/// A MIDI clip's content by zoom (round 39, des-views).
#[path = "arrangement_paint_clip.rs"]
pub mod clip_content;
/// The comp on the playing lane: seams, take numbers, the comps' chip (ADR 0058).
#[path = "arrangement_comp.rs"]
pub mod comp;
#[path = "arrangement_empty.rs"]
pub mod empty;
#[path = "arrangement_folders.rs"]
pub mod folders;
#[path = "arrangement_head.rs"]
pub mod head;
#[path = "arrangement_headers.rs"]
mod headers;
/// Round 39, *themes*: every theme easy to read, measured on what the painters paint.
#[cfg(test)]
#[path = "arrangement_legibility.rs"]
mod legibility;
/// The markers on the ruler (round 29, ADR 0060).
#[path = "arrangement_markers.rs"]
pub mod markers;
#[path = "arrangement_menus.rs"]
pub mod menus;
#[path = "arrangement_paint.rs"]
mod paint;
#[path = "arrangement_rec.rs"]
mod rec;
#[path = "arrangement_ruler.rs"]
pub mod ruler;
/// r39:el-arrange — the arrangement in Slate (ADR 0111, 0113): its inks, the headers, the
/// folders, the menus, the ruler and sections, the song map, the empty song.
#[path = "arrangement_slate.rs"]
pub(crate) mod slate;
#[path = "arrangement_songmap.rs"]
mod songmap;
#[path = "arrangement_takes.rs"]
mod takes;
/// The tempo track and the metre chips (round 29, ADR 0060).
#[path = "arrangement_tempo.rs"]
pub mod tempo;
/// r39:lo-arrange — the tool row and its tools (dir-a's `b-toolbar-*`).
#[path = "arrangement_tools.rs"]
pub mod tools;
pub use rec::{record_controls, RecordControls};

/// The ruler is two lanes, and that is what makes both gestures direct.
///
/// The **loop lane** on top is where dragging sets the loop brace, which is what a bare
/// drag in the ruler used to do. The **selection lane** below it is where dragging makes a
/// time selection. Putting the new gesture on top of the old one would have cost the loop
/// its direct manipulation; putting the selection behind a modifier (FL's `Ctrl+drag`)
/// would have hidden the thing nearly everything else in the playlist plan hangs off.
/// Reaper splits the ruler the same way, for the same reason.
const RULER_H: f32 = ruler::RULER_H;
const LOOP_LANE_H: f32 = 8.0;
/// How wide the grab zone on a header's bottom edge is.
const DIVIDER_GRIP: f32 = 5.0;
/// The header column: its width by default, and how far its edge may be dragged.
const HEADER_W: f32 = head::NORMAL;
const HEADER_MIN: f32 = head::MIN_W;
const HEADER_MAX: f32 = head::MAX_W;
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
    /// A track to scroll into view, with its lanes, on the next frame drawn (the
    /// inspector's *Show in the arrangement*, round 39).
    pub reveal: Option<TrackId>,
    /// The base row height. A track's own height is a multiple of this, so the interface
    /// scale and the per-track choice compose instead of fighting. Ctrl+Alt+wheel and the
    /// vertical bar's ends zoom it.
    pub track_height: f32,
    /// The header column's width as the user left it; its edge is a splitter. A window
    /// too narrow for it draws it narrower ([`Self::header_shown`]) and never stores that
    /// (round 39, fixes).
    pub header_width: f32,
    /// The header column's width as drawn last frame: [`Self::header_width`] within what
    /// the arrangement's own width allows.
    header_drawn: f32,
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
    // r39:el-arrange
    /// The headers' words, rebuilt only when the song or the language changes.
    words: head::Words,
    /// The folder selected, and the one being renamed.
    pub folder_selected: Option<cantus_core::FolderId>,
    folder_rename: Option<(cantus_core::FolderId, String)>,
    /// The page of a track's menu that is open.
    menu_page: Option<(TrackId, menus::Page)>,
    /// The track's own icon's picker.
    pub glyphs: super::track_glyphs::Picker,
    /// The ruler's memory (where play started) and the sections lane's.
    pub ruler: ruler::State,
    pub sections: ruler::sections::State,
    /// The song map's or the vertical thumb's drag.
    map_grab: Option<songmap::Grab>,
    /// Where the empty song's starters were drawn.
    pub empty: empty::Drawn,
    /// Whether the sections and markers lane is shown (the corner's lanes menu).
    pub show_sections: bool,
    // r39:lo-arrange
    /// The tool in hand (dir-a's radio set): view state, never an undo step.
    pub tool: tools::Tool,
    /// Follow: the view turns its page to keep the playhead in sight while the song plays.
    pub follow: bool,
    /// Where the playhead was when Follow last looked (in beats).
    follow_last: Option<f32>,
    /// The grid follows the zoom (the grid menu's "Follow the zoom").
    pub grid_auto: bool,
    /// A tool's gesture in progress (a clip drawn, a sweep, a range to zoom to).
    gesture: Option<tools::Gesture>,
}

/// What the painters reuse from frame to frame.
#[derive(Debug, Default)]
struct Scratch {
    /// A clip's repetitions of one note.
    at: Vec<Ticks>,
    /// A far clip's contour: the highest and lowest note a pixel column (r39:el-views).
    cols: Vec<(f32, f32)>,
    /// Each MIDI clip's notes summed up for the rows, and the revision and note count
    /// it was summed at (r39:el-views).
    summaries: std::collections::HashMap<ClipId, (u64, usize, clip_content::Summary)>,
    /// A clip's label, with its "(muted)" when it has one.
    label: String,
    /// "muted", in the interface's language, and the language it was read in.
    muted_word: String,
    locale: Option<crate::i18n::Locale>,
    tints: Tints,
    digits: Digits,
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
    /// A clip drag's landing under a tempo track (review 4): built when the drag's result
    /// or the song changes, not every frame — the revision, the anchor, the result.
    dropped: Option<(DropKey, Landing)>,
}

/// What a drag's landing was built for.
type DropKey = (u64, (TrackId, ClipId), ClipDragResult);

/// The clips a drag's release would leave, by clip.
type Landing = std::collections::HashMap<(TrackId, ClipId), cantus_core::Clip>;

impl Scratch {
    fn refresh_words(&mut self) {
        let now = crate::i18n::locale();
        if self.locale != Some(now) {
            self.locale = Some(now);
            self.muted_word = t("clip.muted_short");
            self.placeholders.clear();
            self.counts.clear();
        }
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
    /// r39:lo-arrange — the corner notch at the top right (des-arrange): the right edge
    /// drawn out with the clip repeating — its loop switched on, at the length it had.
    Loop,
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
            reveal: None,
            // 40 px, not 64. The header used to hold a fader, a pan bar and a meter — a
            // mixer strip inside a track header — and it is why only four tracks fitted on
            // screen. Those live in the inspector now, which is what buys the density.
            track_height: head::ROW,
            header_width: HEADER_W,
            header_drawn: HEADER_W,
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
            words: Default::default(),
            folder_selected: None,
            folder_rename: None,
            menu_page: None,
            glyphs: Default::default(),
            ruler: Default::default(),
            sections: Default::default(),
            map_grab: None,
            empty: Default::default(),
            show_sections: true,
            tool: tools::Tool::Select,
            follow: true,
            follow_last: None,
            grid_auto: false,
            gesture: None,
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
    /// r39:el-arrange — its keys (M S ● A; the last two empty on a narrow header) and its
    /// own icon's slot before the name.
    pub keys: [Rect; 4],
    pub glyph: Rect,
}

impl ArrangementView {
    /// Let go of every clip drag and audio handle's drag (another project replaces the one
    /// they were begun on — round 29's third review).
    pub(crate) fn let_go(&mut self) {
        self.drag = None;
        self.audio.drag = None;
    }

    /// Start renaming a track in place (a click on its name's letters does).
    pub fn begin_rename_track(&mut self, id: TrackId, name: String) {
        self.rename = Some((id, name));
    }

    /// The header column's width as drawn last frame (the stored one is
    /// [`Self::header_width`]).
    pub fn header_shown(&self) -> f32 {
        self.header_drawn
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

    /// Whether clips are being dragged to move (the Session pane lights its drop slot). // r33:launcher
    pub fn moving_clips(&self) -> bool {
        // r33:launcher
        matches!(&self.drag, Some(d) if d.mode == DragMode::Move) // r33:launcher
    } // r33:launcher

    /// Forget where the timeline was drawn (it was not drawn last pass: a hidden tab). // r33:launcher
    pub fn forget_geometry(&mut self) {
        // r33:launcher
        self.geometry = None; // r33:launcher
    } // r33:launcher

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

    /// A move of the selection in progress, `anchor` dragged by `delta` — for the shots
    /// (`examples/shoot_tempo.rs`), which cannot press a pointer: what a drag draws.
    #[doc(hidden)]
    pub fn shoot_move(&mut self, anchor: (TrackId, ClipId), start: Ticks, delta: Ticks) {
        self.drag = Some(ClipDrag {
            mode: DragMode::Move,
            anchor,
            anchor_start: start,
            anchor_length: Ticks::ZERO,
            press: egui::Pos2::ZERO,
            delta,
            rows: 0,
            copy: false,
        });
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
            DragMode::TrimEnd | DragMode::Loop if r == d.anchor => Some((Ticks::ZERO, d.delta, 0)),
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

    /// The drag in progress as the result its release would give.
    fn drag_result(&self) -> Option<((TrackId, ClipId), ClipDragResult)> {
        let d = self.drag.as_ref()?;
        let r = match d.mode {
            DragMode::Move => ClipDragResult::Move {
                delta: d.delta,
                rows: d.rows,
                copy: false,
            },
            DragMode::TrimStart => ClipDragResult::TrimStart { delta: d.delta },
            DragMode::TrimEnd | DragMode::Loop => ClipDragResult::TrimEnd { delta: d.delta },
            DragMode::Slip => ClipDragResult::Slip { delta: d.delta },
        };
        Some((d.anchor, r))
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
    /// Above the headers, beside the ruler: the plus, the count, the lanes key; the top
    /// lanes' own headers under it.
    corner: Rect,
    headers: Rect,
    /// r39:el-arrange — the song map, the panel's foot (the horizontal scroll), and the
    /// header column's foot beside it (the song's length).
    map: Rect,
    foot: Rect,
    /// The ruler with the loop band, the punch lane, the numbers and the sections lane.
    ruler: Rect,
    lanes: Rect,
    /// The header column's edge.
    split: Rect,
    /// The punch lane's height (zero while punch is off), between the loop lane and the
    /// bar numbers.
    punch_h: f32,
    /// The sections and markers lane's height (zero while hidden), the ruler's foot.
    markers_h: f32,
    /// The tempo track's lane, between the ruler and the tracks (empty while hidden).
    tempo: Rect,
}

impl Frame {
    /// The regions (round 39: the song map at the foot, the sections under the numbers).
    fn with_lanes(
        full: Rect,
        header_w: f32,
        punch_h: f32,
        markers_h: f32,
        tempo_h: f32,
        s: &Style,
    ) -> Frame {
        let hx = full.left() + header_w;
        let right = full.right();
        let map_h = s.s(songmap::MAP_H);
        let ruler = Rect::from_min_max(
            pos2(hx + 1.0, full.top()),
            pos2(right, full.top() + s.s(RULER_H) + punch_h + markers_h),
        );
        let tempo = Rect::from_min_max(
            pos2(ruler.left(), ruler.bottom()),
            pos2(right, ruler.bottom() + tempo_h),
        );
        let bottom = full.bottom() - map_h;
        let lanes = Rect::from_min_max(pos2(ruler.left(), tempo.bottom()), pos2(right, bottom));
        Frame {
            corner: Rect::from_min_max(full.min, pos2(hx, ruler.bottom())),
            headers: Rect::from_min_max(pos2(full.left(), tempo.bottom()), pos2(hx, bottom)),
            map: Rect::from_min_max(pos2(ruler.left(), bottom), pos2(right, full.bottom())),
            foot: Rect::from_min_max(pos2(full.left(), bottom), pos2(hx, full.bottom())),
            ruler,
            lanes,
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

/// The grid list as a kit menu, for the channel rack and the top bar's snap: returns the
/// grid chosen. The arrangement's and the piano roll's own grid menus are
/// [`tools::grid_menu`] (r39:lo-arrange: musical order, Triplets, Follow the zoom).
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
    app.forget_other_projects_lanes();
    // A sample from the browser lands here as an audio clip (ADR 0030).
    crate::samples::drop::publish(
        ui.ctx(),
        ui.max_rect(),
        crate::samples::drop::Target::Arrangement,
    );
    tools::toolbar(app, ui);
    let s = Style::current();
    let full = ui.available_rect_before_wrap();
    if full.width() < 60.0 || full.height() < s.s(RULER_H) + 2.0 * s.s(songmap::MAP_H) {
        return;
    }
    let punch_h = rec::punch_height(app, &s);
    let markers_h = markers::height(app, &s);
    let tempo_h = tempo::height(app, &s);
    // The automation lanes' order, once per edit.
    let mut auto_st = std::mem::take(&mut app.arrangement.auto);
    auto_st.refresh(app);
    app.arrangement.auto = auto_st;
    // The headers' and lanes' words, once per edit (r39:el-arrange).
    let mut words = std::mem::take(&mut app.arrangement.words);
    words.refresh(app);
    app.arrangement.words = words;
    let v = &mut app.arrangement;
    // What is drawn is clamped to the room there is; what is stored is not, so a first
    // frame in a small window leaves the user's width for when it grows (round 39, fixes).
    let shown = header_shown(v.header_width, full.width());
    v.header_drawn = shown;
    let f = Frame::with_lanes(full, shown, punch_h, markers_h, tempo_h, &s);
    navigate(app, ui, &f);
    // r39:lo-arrange — the grid that follows the zoom, and the page that follows the playhead.
    tools::follow_zoom(&mut app.arrangement);
    tools::follow_playhead(app, f.lanes);
    let mut geo = layout(app, &f);
    // A track asked for (the inspector's *Show in the arrangement*): its row and lanes
    // scrolled into view, 8 pt under the lanes' top, where they are not all seen.
    if let Some(t) = app.arrangement.reveal.take() {
        let row = app
            .project()
            .tracks
            .iter()
            .position(|x| x.id == t)
            .and_then(|i| geo.rows.get(i))
            .map(|r| (r.top, r.top + r.height + r.takes + r.auto + r.pitch));
        if let Some((top, bottom)) = row.filter(|(a, z)| z > a) {
            if top < f.lanes.top() || bottom > f.lanes.bottom() {
                let v = &mut app.arrangement;
                v.scroll_y = (v.scroll_y + top - f.lanes.top() - 8.0).max(0.0);
                geo = layout(app, &f);
            }
        }
    }
    timeline(app, ui, &f, &geo, &s);
    headers::headers(app, ui, &f, &geo, &s);
    corner(app, ui, &f, &s);
    // r39:el-arrange — the song map is the horizontal scroll; a thin thumb the vertical.
    songmap::show(app, ui, f.map, f.foot, f.lanes, &s);
    let content = content_height(app) + s.s(head::ADD_ROW);
    songmap::thumb(app, ui, f.lanes, content, &s);
    // The paint tool (B; round 26, ADR 0037), above the timeline while it is out.
    super::clip_browser::paint_tool(app, ui, f.lanes, &geo);
    let v = &mut app.arrangement;
    let mut edge = shown;
    kit::widgets::splitter(
        ui,
        f.split,
        ui.id().with("arrangement-headers-edge"),
        &mut edge,
        HEADER_W,
        HEADER_MIN..=HEADER_MAX,
    );
    // Only the user's own move of the edge is stored.
    if edge != shown {
        v.header_width = edge;
    }
    if ui.rect_contains_pointer(f.split.expand(2.0)) {
        let tip = crate::i18n::tf("arrange.splitter.tip", &[("w", &(shown.round() as i64))]);
        hint::set_text(
            ui.ctx(),
            &tip,
            chrome::word("arrange.splitter.fit"),
            Caps::NONE,
        );
    }
    // The track's own icon's picker, when open.
    super::track_glyphs::show(app, ui.ctx());
    // The geometry is published last, so a click this frame is aimed with what was drawn.
    let v = &mut app.arrangement;
    geo.scroll_beats = v.scroll_beats;
    v.geometry = Some(geo);
    app.session_view.arrangement_pass = Some(ui.ctx().cumulative_pass_nr()); // r33:launcher
}

/// The header column's width drawn in an arrangement `width` wide: the stored one within
/// [`HEADER_MIN`] and the most the arrangement's width allows (60 % of it, at most
/// [`HEADER_MAX`]).
fn header_shown(stored: f32, width: f32) -> f32 {
    let most = HEADER_MAX.min(width * 0.6).max(40.0);
    stored.clamp(HEADER_MIN.min(most), most)
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

/// The rows' full height at the current zoom: tracks, their lanes and folders' rows (a
/// closed folder's members at none).
fn content_height(app: &CantusApp) -> f32 {
    folders::content_height(app.project(), app.arrangement.track_height)
}

/// The wheel and the middle button (the song map and the thumb are their own).
fn navigate(app: &mut CantusApp, ui: &mut egui::Ui, f: &Frame) {
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

    // The rows never scroll past the add-track row.
    let content = content_height(app) + app.arrangement.track_height;
    let v = &mut app.arrangement;
    let most = (content - f.lanes.height()).max(0.0);
    v.scroll_y = v.scroll_y.clamp(0.0, most);
}

/// The rows where they are drawn this frame, into the geometry the last frame left.
fn layout(app: &mut CantusApp, f: &Frame) -> TimelineGeometry {
    let v = &mut app.arrangement;
    let mut geo = v.geometry.take().unwrap_or_default();
    geo.left = f.lanes.left();
    geo.width = f.lanes.width();
    geo.ruler_top = f.ruler.top();
    geo.loop_lane_bottom = f.ruler.top() + LOOP_LANE_H;
    geo.body_top = f.lanes.top();
    geo.lanes_bottom = f.lanes.bottom();
    // The punch lane under the loop lane; the sections and markers lane at the ruler's foot
    // (r39:el-arrange; ADR 0060, 0113).
    geo.punch =
        (f.punch_h > 0.0).then_some((geo.loop_lane_bottom, geo.loop_lane_bottom + f.punch_h));
    geo.markers = (f.markers_h > 0.0).then_some((f.ruler.bottom() - f.markers_h, f.ruler.bottom()));
    geo.tempo = (f.tempo.height() > 0.0).then_some((f.tempo.top(), f.tempo.bottom()));
    geo.px_per_beat = v.px_per_beat;
    geo.scroll_beats = v.scroll_beats;
    let (top, base) = (f.lanes.top() - v.scroll_y, v.track_height);
    let mut rows = std::mem::take(&mut geo.rows);
    let mut fr = std::mem::take(&mut geo.folders);
    folders::rows_into(&mut rows, &mut fr, app.project(), top, base);
    geo.rows = rows;
    geo.folders = fr;
    geo
}

/// The band under the loop (and punch) lane where the bar numbers are.
fn numbers_rect(geo: &TimelineGeometry, ruler: Rect) -> Rect {
    let top = geo.punch.map_or(geo.loop_lane_bottom, |p| p.1);
    let bottom = geo.markers.map_or(ruler.bottom(), |m| m.0);
    Rect::from_min_max(pos2(ruler.left(), top), pos2(ruler.right(), bottom))
}

/// The corner over the headers (r39:el-arrange): a plus in a well (the add-track menu),
/// "N tracks", the lanes key (what shows above the tracks); under it each top lane's own
/// header ("Sections").
fn corner(app: &mut CantusApp, ui: &mut egui::Ui, f: &Frame, s: &Style) {
    let ink = slate::Ink::of(s);
    let k = s.k;
    let row = Rect::from_min_max(
        f.corner.min,
        pos2(f.corner.right(), f.corner.top() + s.s(RULER_H)),
    );
    let plus = Rect::from_center_size(
        pos2(row.left() + 18.0 * k, row.center().y),
        vec2(20.0 * k, 18.0 * k),
    );
    let lanes_key = Rect::from_center_size(
        pos2(row.right() - 18.0 * k, row.center().y),
        vec2(20.0 * k, 18.0 * k),
    );
    let plus_r = ui.interact(plus, ui.id().with("arr-add-track"), Sense::click());
    let lanes_r = ui.interact(lanes_key, ui.id().with("arr-lanes-key"), Sense::click());
    super::device_chrome::publish(ui.ctx(), super::device_chrome::Spot::AddTrack, plus);
    let sections = Rect::from_min_max(
        pos2(f.corner.left(), f.corner.bottom() - f.markers_h),
        f.corner.max,
    );
    {
        let words = &app.arrangement.words;
        kit::with_brush(&ui.painter().with_clip_rect(f.corner), |b| {
            b.register(f.corner, "corner", &[kit::Tok::Toolbar], &[]);
            b.rect(b.snap_rect(f.corner), kit::Fill::Solid(ink.surface));
            let well = if plus_r.hovered() {
                ink.well_hi
            } else {
                ink.well
            };
            b.rrect(plus, s.r(3.0), kit::Fill::Solid(ink.edge));
            b.rrect(plus.shrink(b.px()), s.r(3.0), kit::Fill::Solid(well));
            let c = plus.center();
            let pc = if plus_r.hovered() { ink.ink1 } else { ink.ink2 };
            b.hline(c.x - 4.0 * k, c.x + 4.0 * k, c.y, 1.2 * k, pc);
            b.vline(c.x, c.y - 4.0 * k, c.y + 4.0 * k, 1.2 * k, pc);
            let count = Rect::from_min_max(
                pos2(plus.right() + 8.0 * k, row.top()),
                pos2(lanes_key.left() - 4.0 * k, row.bottom()),
            );
            let _ = slate::say(
                b,
                count,
                &words.count,
                slate::W::Regular,
                11.0,
                ink.ink3,
                egui::Align2::LEFT_CENTER,
                s,
            );
            let lc = if lanes_r.hovered() {
                ink.ink1
            } else {
                ink.ink3
            };
            for (i, w) in [10.0, 7.0, 10.0].iter().enumerate() {
                let y = lanes_key.center().y - 3.0 * k + 3.0 * k * i as f32;
                let x0 = lanes_key.center().x - 5.0 * k;
                b.hline(x0, x0 + w * k, y, 1.2 * k, lc);
            }
            if sections.height() > 0.0 {
                let g = slate::Ink::to(ink.surface, ink.base, 0.35);
                b.rect(b.snap_rect(sections), kit::Fill::Solid(g));
                let r = sections.shrink2(vec2(12.0 * k, 0.0));
                let _ = slate::say(
                    b,
                    r,
                    &words.sections_word,
                    slate::W::Regular,
                    11.0,
                    ink.ink3,
                    egui::Align2::LEFT_CENTER,
                    s,
                );
            }
        });
    }
    if plus_r.hovered() {
        hint::set_text(ui.ctx(), chrome::word("swap.add_track_tip"), "", Caps::NONE);
    }
    // A click: an instrument from the picker (round 20); a right-click: every kind of
    // track, and the instruments chosen last (ux-b, ADR 0110).
    if plus_r.clicked() {
        app.open_picker(crate::picker::PickPurpose::AddInstrument { index: None });
    }
    egui::Popup::context_menu(&plus_r)
        .frame(chrome::menu_frame())
        .show(|ui| menus::add_track(app, ui, None));
    if lanes_r.hovered() {
        hint::set_text(ui.ctx(), chrome::word("arrange.lanes.tip"), "", Caps::NONE);
    }
    egui::Popup::menu(&lanes_r)
        .frame(chrome::menu_frame())
        .show(|ui| lanes_menu(app, ui));
}

/// The top lanes' menu: what shows above the tracks.
fn lanes_menu(app: &mut CantusApp, ui: &mut egui::Ui) {
    use crate::keymap::Action;
    let tempo_on = app.tempo_lane_shown();
    let marker_key = app.keymap.display_for(Action::AddMarker);
    let tempo_key = app.keymap.display_for(Action::ToggleTempoTrack);
    let items = [
        Item::Action {
            label: Src::Key("arrange.lanes.sections"),
            check: if app.arrangement.show_sections {
                Check::On
            } else {
                Check::Off
            },
            shortcut: None,
            submenu: false,
            enabled: true,
        },
        Item::Action {
            label: Src::Key("arrange.lanes.tempo"),
            check: if tempo_on { Check::On } else { Check::Off },
            shortcut: (!tempo_key.is_empty()).then_some(tempo_key.as_str()),
            submenu: false,
            enabled: true,
        },
        Item::Separator,
        Item::Action {
            label: Src::Key("arrange.lanes.add_marker"),
            check: Check::None,
            shortcut: (!marker_key.is_empty()).then_some(marker_key.as_str()),
            submenu: false,
            enabled: true,
        },
        Item::action(Src::Key("arrange.lanes.add_section")),
    ];
    match kit::widgets::menu_list(ui, &items, 240.0).1.clicked {
        Some(0) => app.arrangement.show_sections = !app.arrangement.show_sections,
        Some(1) => app.run_action(Action::ToggleTempoTrack),
        Some(3) => app.run_action(Action::AddMarker),
        Some(4) => {
            app.arrangement.show_sections = true;
            let at = app.engine.position();
            let _ = ruler::sections::add_here(app, at);
        }
        _ => return,
    }
    ui.close();
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

/// Where the timeline was drawn, in screen coordinates.
///
/// Recorded every frame so that anything outside `show` — a test aiming at a clip, and
/// later anything that wants to scroll the view to a tick — can map between ticks and
/// pixels by *asking* rather than by mirroring the layout constants. A test that
/// recomputes the layout is a test that breaks the day a toolbar is added, which is
/// exactly what happened.
#[derive(Clone, Debug, Default, PartialEq)]
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
    /// r39:el-arrange — where each folder's own row was drawn (ADR 0113), and the lanes'
    /// bottom (above the song map).
    pub folders: Vec<folders::FolderRow>,
    pub lanes_bottom: f32,
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
            .position(|r| r.height > 0.0 && y >= r.top && y < r.top + r.height)
    }

    /// One past the bottom of the last row (a track's with its lanes, or a folder's).
    pub fn bottom(&self) -> f32 {
        let rows = self
            .rows
            .iter()
            .map(|r| r.top + r.height + r.takes + r.auto + r.pitch);
        let folders = self.folders.iter().map(|f| f.top + f.height);
        let most = rows.chain(folders).fold(f32::NEG_INFINITY, f32::max);
        if most.is_finite() {
            most
        } else {
            self.body_top
        }
    }
}

/// Where each track's row sits, given the base row height. One function, so the headers
/// and the timeline can never disagree about which stripe belongs to which track (round 39:
/// folders' rows among them, a closed folder's members at no height — ADR 0113).
pub fn rows_of(project: &cantus_core::Project, top: f32, base: f32) -> Vec<TrackRow> {
    let mut rows = Vec::with_capacity(project.tracks.len());
    let mut folders = Vec::new();
    folders::rows_into(&mut rows, &mut folders, project, top, base);
    rows
}

/// Hit test result: which clip the pointer is over, and where in it.
#[derive(Copy, Clone, Debug, PartialEq)]
struct Hit {
    track: TrackId,
    clip: ClipId,
    mode: DragMode,
    /// An audio clip's fade or gain handle, which a press there drags instead.
    handle: Option<AudioHandle>,
    /// r39:lo-arrange — where the clip is drawn, and its colour: the resize edge's light,
    /// the loop notch and the Slice tool's cut are drawn on it.
    body: Rect,
    hue: egui::Color32,
    /// Whether the clip has a loop notch (a note clip in a lane at least 14 high).
    notch: bool,
}

/// r39:lo-arrange — where a clip's loop notch is: 9 px at its top right corner.
fn notch_rect(body: Rect, k: f32) -> Rect {
    Rect::from_min_max(
        pos2(body.right() - 9.0 * k, body.top()),
        pos2(body.right(), body.top() + 9.0 * k),
    )
}

/// r39:lo-arrange — des-arrange's *Resize*: the edge under the pointer lights 4 px (the
/// clip's colour + 20 % white), and stays lit while it is dragged; a note clip under the
/// pointer shows its loop notch, lit when the pointer is on it.
fn edge_light(
    b: &mut kit::Brush,
    v: &ArrangementView,
    hover: Option<Hit>,
    anchor: Option<(Rect, egui::Color32)>,
    s: &Style,
) {
    let room = super::room::Room::of(&s.t);
    let w = 4.0 * s.k;
    let strip = |b: &mut kit::Brush, body: Rect, hue: egui::Color32, left: bool| {
        let r = if left {
            Rect::from_min_max(body.min, pos2(body.left() + w, body.bottom()))
        } else {
            Rect::from_min_max(pos2(body.right() - w, body.top()), body.max)
        };
        let r = b.snap_rect(r.intersect(body));
        b.rrect(
            r,
            s.r(2.0),
            kit::Fill::Solid(kit::color::mix(hue, room.white, 0.2)),
        );
    };
    let notch = |b: &mut kit::Brush, body: Rect, hot: bool| {
        let n = notch_rect(body, s.k);
        let ink = if hot {
            room.ink1
        } else {
            kit::color::with_alpha(room.ink1, 0.45)
        };
        b.polygon(&[n.left_top(), n.right_top(), n.right_bottom()], ink);
    };
    if let Some(d) = v.drag.as_ref() {
        if let Some((body, hue)) = anchor {
            match d.mode {
                DragMode::TrimStart => strip(b, body, hue, true),
                DragMode::TrimEnd => strip(b, body, hue, false),
                DragMode::Loop => {
                    strip(b, body, hue, false);
                    notch(b, body, true);
                }
                _ => {}
            }
        }
        return;
    }
    let tools_resize = matches!(v.tool, tools::Tool::Select | tools::Tool::Draw);
    let Some(h) = hover.filter(|h| h.handle.is_none() && tools_resize) else {
        return;
    };
    match h.mode {
        DragMode::TrimStart => strip(b, h.body, h.hue, true),
        DragMode::TrimEnd => strip(b, h.body, h.hue, false),
        _ => {}
    }
    if h.notch {
        notch(b, h.body, h.mode == DragMode::Loop);
    }
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
    /// r39:lo-arrange — where the clip a trim or a loop drag holds is drawn now, and its
    /// colour (its edge stays lit while it is dragged).
    anchor: Option<(Rect, egui::Color32)>,
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
    // What freezes hold (round 29, ADR 0064): their clips under a frosted veil.
    let held = super::freeze_view::held(app);
    audio::begin_frame(&mut scratch.audio);
    scratch.refresh_words();
    // Linked clips (ADR 0037): the groups' sizes and the selection's groups, once.
    // r39:el-views — the playhead while the song plays: the clips under it are lit.
    let playhead = app.engine.is_playing().then(|| app.engine.position());
    let marks = crate::links::LinkMarks::of(app.project(), &app.clip_selection());
    let mut counts = std::mem::take(&mut scratch.counts);
    let n_rows = geo.rows.len() as i32;
    let (vis_l, vis_r) = (geo.beats(lanes.left()), geo.beats(lanes.right()));
    // Under a tempo track a drag of unwarped audio re-derives the clips it moves (review
    // 3): the clips its release would leave are drawn, not tick arithmetic.
    let mut dropped = std::mem::take(&mut scratch.dropped);
    match v.drag_result() {
        Some((anchor, result))
            if !app.project().tempo_points.is_empty() && v.stretch_preview(anchor).is_none() =>
        {
            let key = (app.bus.revision(), anchor, result);
            if dropped.as_ref().is_none_or(|(k, _)| *k != key) {
                let landing = crate::clips::drag_preview(
                    app.project(),
                    &app.clip_selection(),
                    anchor,
                    result,
                );
                dropped = Some((key, landing));
            }
        }
        _ => dropped = None,
    }
    let landings = dropped.as_ref().map(|(_, m)| m);
    // r39:lo-arrange — des-arrange's clip states: the selected track's clips `lit`, the
    // clips of a track another track's solo silences `dim`.
    let any_solo = app.project().tracks.iter().any(|t| t.soloed);
    let moving = v.drag.as_ref().is_some_and(|d| d.mode == DragMode::Move);
    let ink = slate::Ink::of(s);
    for (i, track) in app.project().tracks.iter().enumerate() {
        let Some(here) = geo.rows.get(i).copied() else {
            continue;
        };
        // A closed folder's member is not drawn (r39:el-arrange).
        if here.height <= 0.0 {
            continue;
        }
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
            let landed = landings.and_then(|m| m.get(&r));
            let (start, length) = match landed {
                Some(c) => (c.start, c.length.max(Ticks::SIXTEENTH)),
                None => (
                    clip.start + dstart,
                    (clip.length + dlen).max(Ticks::SIXTEENTH),
                ),
            };
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
            // r39:lo-arrange — a clip being moved leaves its slot dashed (ink 3, 4/3) and
            // rides lifted, a shadow under it (des-arrange, *Moving a clip*).
            let lifted =
                moving && (dstart != Ticks::ZERO || drows != 0) && v.preview(sel, r).is_some();
            if lifted {
                let home = if clip.automation.is_some() || clip.lane > 0 {
                    target
                } else {
                    here
                };
                let (h0, h1) = (
                    geo.x(ticks_to_beats(clip.start)),
                    geo.x(ticks_to_beats(
                        clip.start + clip.length.max(Ticks::SIXTEENTH),
                    )),
                );
                let slot = Rect::from_min_max(
                    pos2(h0, home.top + inset),
                    pos2(h1.max(h0 + 2.0), home.top + home.height - inset),
                );
                let (slot, px) = (b.snap_rect(slot), b.px());
                paint::dashed_rect(b, slot, lanes, px, ink.ink3, 4.0 * s.k, 3.0 * s.k);
                let shade = if s.t.dark { 0.35 } else { 0.12 };
                let room = super::room::Room::of(&s.t);
                b.rrect_soft(
                    body.translate(egui::vec2(0.0, 3.0 * s.k)),
                    s.r(3.0),
                    kit::Fill::Solid(kit::color::with_alpha(room.black, shade)),
                    4.0 * s.k,
                );
            }
            let renaming = v.rename_clip.as_ref().is_some_and(|(c, _)| *c == r);
            let reserve = match clip.audio.as_ref() {
                Some(a) => audio::badge_width(
                    b,
                    a,
                    app.project().provenance.contains_key(&a.source), // r34:stems
                    &mut scratch.audio,
                    s,
                ),
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
                // r37:finish — quiet when the track above says the same from the same bar:
                // the first clip of a run keeps its name (the judge: a name always to be
                // found), the ones under it show their content at full height.
                quiet: !renaming
                    && clip.lane == 0
                    && !clip.name.is_empty()
                    && marks.count(clip).is_none()
                    && i > 0
                    && app.project().tracks[i - 1]
                        .clips
                        .iter()
                        .any(|c| c.lane == 0 && c.start == clip.start && c.name == clip.name),
                // r39:el-views — des-views' clip states.
                playing: playhead.is_some_and(|p| start <= p && p < start + length),
                near: geo.px_per_beat >= 20.0,
                lit: app.selected_track == Some(track.id) || lifted,
                dim: any_solo && !track.soloed,
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
                let (region, offset) =
                    match landed.and_then(|c| c.audio.as_ref().map(|a| (a, c.offset))) {
                        Some(l) => l,
                        None => (region, clip.offset + v.slip_preview(r, region.reverse)),
                    };
                let shown = audio::Shown {
                    track: track.id,
                    clip,
                    region,
                    start,
                    length,
                    offset,
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
                    app.project(),
                    s,
                );
            }
            if !clip.notes.is_empty() && inner.height() > 2.0 && inner.width() > 4.0 {
                // r39:el-views — the content by zoom (des-views' rules,
                // `arrangement_paint_clip.rs`).
                let ink = paint::note_ink(&look, s);
                let rev = app.bus.revision();
                let sum = match scratch.summaries.get(&clip.id) {
                    Some(&(r, n, sum)) if r == rev && n == clip.notes.len() => sum,
                    _ => {
                        let sum = clip_content::Summary::of(clip);
                        scratch
                            .summaries
                            .insert(clip.id, (rev, clip.notes.len(), sum));
                        sum
                    }
                };
                let m = clip_content::Midi {
                    sum,
                    clip,
                    shown,
                    inner,
                    left: inner.left().max(lanes.left()),
                    right: inner.right().min(lanes.right()),
                    vis: (vis_l, vis_r),
                    ink,
                    ground: paint::body_ground(&look, s),
                    drums: matches!(track.instrument, cantus_core::InstrumentSpec::Pulsus(_)),
                    roles: clip_content::Roles::of(track.instrument.pulsus()), // r39:lo-rest
                    px_per_beat: geo.px_per_beat,
                    attack: {
                        let (c, a) = if s.t.dark {
                            (s.t.light, 0.35)
                        } else {
                            (s.t.dark_shade, 0.18)
                        };
                        // r39:lo-arrange — a silenced clip's attacks quiet with its notes.
                        (c, if look.dim { a * 0.32 } else { a })
                    },
                };
                clip_content::midi(b, geo, &m, &mut scratch.at, &mut scratch.cols);
            }
            // A frozen track's clip (ADR 0064): in a render now, under a frosted veil.
            if clip.automation.is_none() && clip.lane == 0 && held.held_tracks.contains(&track.id) {
                super::freeze_view::veil_clip(b, body, lanes, s);
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
            // r39:lo-arrange — a note clip's loop notch (9 px at its top right).
            let notch = clip.audio.is_none()
                && clip.automation.is_none()
                && clip.lane == 0
                && body.height() >= 14.0 * s.k
                && body.width() >= 36.0 * s.k;
            if v.drag.as_ref().is_some_and(|d| {
                d.anchor == r
                    && matches!(
                        d.mode,
                        DragMode::TrimStart | DragMode::TrimEnd | DragMode::Loop
                    )
            }) {
                out.anchor = Some((body, color));
            }
            let test = |pos: Pos2| -> Option<Hit> {
                if clip.lane > 0 || !body.contains(pos) || !lanes.contains(pos) {
                    return None;
                }
                let hit = |mode, handle| Hit {
                    track: track.id,
                    clip: clip.id,
                    mode,
                    handle,
                    body,
                    hue: color,
                    notch,
                };
                if let Some(handle) = audio_h.and_then(|h| h.at(pos)) {
                    return Some(hit(DragMode::Move, Some(handle)));
                }
                if notch && notch_rect(body, s.k).contains(pos) {
                    return Some(hit(DragMode::Loop, None));
                }
                let zone = edge_zone(body.width());
                let mode = if pos.x < body.left() + zone {
                    DragMode::TrimStart
                } else if pos.x > body.right() - zone {
                    DragMode::TrimEnd
                } else {
                    DragMode::Move
                };
                Some(hit(mode, None))
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
    scratch.dropped = dropped;
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
    let mut ruler_st = std::mem::take(&mut app.arrangement.ruler);
    let hits = {
        let a: &CantusApp = app;
        // r39:el-arrange — the ruler in Slate, the sections and markers lane at its foot.
        let mut rst = std::mem::take(&mut ruler_st);
        kit::with_brush(&ui.painter().with_clip_rect(f.ruler), |b| {
            ruler::paint(
                b,
                a,
                &mut rst,
                view,
                f.ruler,
                numbers_rect(view, f.ruler),
                s,
            );
            rec::paint_punch(b, a, view, f.ruler, s);
            if let Some((top, bottom)) = view.markers {
                let lane =
                    Rect::from_min_max(pos2(f.ruler.left(), top), pos2(f.ruler.right(), bottom));
                ruler::sections::paint(b, a, view, lane, s);
                markers::paint(b, a, view, lane, s);
            }
        });
        ruler_st = rst;
        // The tempo track under the ruler, its name and tempo in the header column.
        if f.tempo.height() > 0.0 {
            let head = f.tempo_head();
            kit::with_brush(&ui.painter().with_clip_rect(f.tempo.union(head)), |b| {
                tempo::paint(b, a, view, f.tempo, head, s);
            });
        }
        kit::with_brush(&lanes_p, |b| {
            ruler::grid(b, a, view, f.lanes, s);
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
    app.arrangement.ruler = ruler_st;
    kit::with_brush(&lanes_p, |b| {
        takes::overlay(b, app, view, f.lanes, s);
        rec::paint_live(b, app, view, f.lanes, s);
        ruler::overlay(b, app, &app.arrangement, view, f.lanes, s);
        // r39:lo-arrange — the resize edge's light and the loop notch, on the clip the
        // pointer finds (des-arrange: nothing is drawn on a clip until then); the tools'.
        edge_light(b, &app.arrangement, hits.hover, hits.anchor, s);
        tools::paint(b, app, view, f.lanes, hits.hover, pointer, s);
    });
    rec::paint_live_waves(&lanes_p, app, view, f.lanes, s);
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
        ui.ctx().set_cursor_icon(audio::edit::cursor(handle));
        audio::edit::hint(ui.ctx(), app, (h.track, h.clip), handle, false);
    } else if let Some(h) = hit {
        if app.arrangement.drag.is_none() {
            ui.ctx().set_cursor_icon(match h.mode {
                DragMode::TrimStart | DragMode::TrimEnd | DragMode::Loop => {
                    egui::CursorIcon::ResizeHorizontal
                }
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
    // r39:lo-arrange — the tool in hand takes the lanes' clicks and drags first; the Select
    // tool's (and the Draw tool's on a clip) are the view's own below.
    let tool_cx = tools::Ctx {
        view,
        body_top,
        snap_to: &snap_to,
        mods,
        hit,
        at_press: hit_at_press,
    };
    tools::hover(app, ui, &tool_cx, pointer);
    let tool = app.arrangement.tool;
    let tool_click = resp.clicked()
        && resp
            .interact_pointer_pos()
            .is_some_and(|pos| tools::click(app, &tool_cx, pos));
    let tool_drag = resp.drag_started_by(PointerButton::Primary)
        && press.is_some_and(|pos| tools::drag_start(app, &tool_cx, pos));
    let tool_dragged = resp.dragged_by(PointerButton::Primary)
        && resp
            .interact_pointer_pos()
            .is_some_and(|pos| tools::dragged(app, &tool_cx, pos));
    if resp.drag_stopped() {
        tools::drag_stop(app, &tool_cx);
    }

    if resp.clicked() && !tool_click {
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
                // A selected warp marker is let go too (Del then acts on the selection).
                app.warp.selected = None;
                if !mods.command {
                    app.clear_clip_selection();
                }
                if let Some(tid) = track_at(app, pos.y) {
                    app.select_track(tid);
                }
            }
        }
    }
    // The other tools' clicks are theirs: a double-click is two of them (the Draw tool's on
    // an empty lane two clips, not a clip and a four-bar one under it).
    let doubles = tool == tools::Tool::Select || (tool == tools::Tool::Draw && hit.is_some());
    if resp.double_clicked() && doubles {
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
        // Where on the clip: *Warp from here* starts there (ADR 0061).
        app.warp.menu_tick = resp
            .interact_pointer_pos()
            .or(pointer)
            .map(|p| beats_to_ticks(view.beats(p.x).max(0.0)));
        app.arrangement.menu_at = match (hit, resp.interact_pointer_pos().or(pointer)) {
            (None, Some(pos)) if pos.y >= body_top => Some((
                beats_to_ticks(view.beats(pos.x).max(0.0)),
                track_at(app, pos.y),
            )),
            // The ruler (review 2): its menu's *Add a marker here* acts under the pointer
            // (on the grid: `menu_point`); no track under it.
            (None, Some(pos)) => Some((beats_to_ticks(view.beats(pos.x).max(0.0)), None)),
            _ => None,
        };
    }
    // r39:ux-b — Shift+F10 on the selected clip opens its menu (ADR 0110).
    let by_key = crate::menu_keys::take_clip(ui.ctx());
    if let Some((t, c)) = by_key {
        app.arrangement.menu_clip = Some((t, c));
        app.arrangement.audio.menu = None;
        app.arrangement.menu_at = None;
        clip_menu_opened(app, t, c);
    }
    let fade_menu = audio::edit::menu_fade(app.arrangement.audio.menu);
    let menu_target = app
        .arrangement
        .menu_clip
        .filter(|(t, c)| app.project().track(*t).and_then(|t| t.clip(*c)).is_some());
    let handle_menu = app.arrangement.audio.menu;
    let popup = egui::Popup::context_menu(&resp);
    let popup = if by_key.is_some() {
        popup.open_memory(Some(egui::containers::SetOpenCommand::Bool(true)))
    } else {
        popup
    };
    popup
        .frame(chrome::menu_frame())
        .show(|ui| match (menu_target, fade_menu) {
            (Some(r), Some(fade)) => audio::edit::fade_menu(app, ui, r, fade),
            // A crossfade's or an envelope's own menu (ADR 0070).
            (Some(r), None) if audio::edit::handle_menu(app, ui, r, handle_menu) => {}
            (Some((track, clip)), None) => clip_menu(app, ui, track, clip),
            (None, _) => timeline_menu(app, ui, true),
        });
    audio::edit::editor_window(app, ui.ctx());

    if resp.drag_started_by(PointerButton::Primary) && !tool_drag {
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
    if resp.dragged_by(PointerButton::Primary) && !tool_dragged {
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
                    DragMode::TrimEnd | DragMode::Loop => {
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
                        DragMode::TrimEnd | DragMode::Loop => Some(WindowEdit::TrimEnd),
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
    // r39:el-arrange — a time range being drawn says where and how long ("43.1 – 46.1 ·
    // 3 bars"); a clip being moved, where it lands.
    if let (Some(_), Some((a, b))) = (app.arrangement.selecting, app.time_selection) {
        let p = app.project();
        let (ab, abeat, _) = crate::views::bars::bbt(p, a);
        let (bb, bbeat, _) = crate::views::bars::bbt(p, b);
        let bars = crate::views::bars::bar_position(p, b) - crate::views::bars::bar_position(p, a);
        let text = crate::i18n::tf(
            "arrange.range.tip",
            &[
                ("from", &format!("{ab}.{abeat}")),
                ("to", &format!("{bb}.{bbeat}")),
                ("bars", &format!("{bars:.1}")),
            ],
        );
        hint::set_text(ui.ctx(), &text, "", Caps::NONE);
    }
    if let Some(d) = app
        .arrangement
        .drag
        .as_ref()
        .filter(|d| d.mode == DragMode::Move)
    {
        let p = app.project();
        let (bar, beat, _) = crate::views::bars::bbt(p, d.anchor_start + d.delta);
        let name = crate::clips::clip_of(p, d.anchor)
            .map(|c| c.name.clone())
            .unwrap_or_default();
        let text = crate::i18n::tf(
            "arrange.move.tip",
            &[("name", &name), ("at", &format!("{bar}.{beat}"))],
        );
        hint::set_text(
            ui.ctx(),
            &text,
            chrome::word("arrange.move.keys"),
            Caps::NONE,
        );
    }
    // r39:lo-arrange — a resize says the length it lands on (des-arrange: "Length 8 bars ·
    // Alt: stretch"); the loop notch says what it does.
    if let Some(d) = app.arrangement.drag.as_ref().filter(|d| {
        matches!(
            d.mode,
            DragMode::TrimStart | DragMode::TrimEnd | DragMode::Loop
        )
    }) {
        let p = app.project();
        let (from, to) = match d.mode {
            DragMode::TrimStart => (d.anchor_start + d.delta, d.anchor_start + d.anchor_length),
            _ => (d.anchor_start, d.anchor_start + d.anchor_length + d.delta),
        };
        let bars =
            crate::views::bars::bar_position(p, to) - crate::views::bars::bar_position(p, from);
        let bars = if (bars - bars.round()).abs() < 1e-3 {
            format!("{}", bars.round() as i64)
        } else {
            format!("{bars:.2}")
        };
        let text = crate::i18n::tf("arrange.resize.tip", &[("bars", &bars)]);
        hint::set_text(
            ui.ctx(),
            &text,
            chrome::word("arrange.resize.keys"),
            Caps::NONE,
        );
    } else if hit.is_some_and(|h| h.mode == DragMode::Loop)
        && matches!(tool, tools::Tool::Select | tools::Tool::Draw)
    {
        hint::set_text(ui.ctx(), chrome::word("arrange.notch.tip"), "", Caps::NONE);
    }
    if resp.drag_stopped() {
        // r39:ux-b — a loop drawn in the loop lane is heard looping at once (FL's and
        // Logic's way; ADR 0110): the switch goes on in the same step of undo as the brace.
        if let Some(a) = app.arrangement.looping {
            let p = app.project();
            let drawn = p
                .loop_region
                .as_ref()
                .is_some_and(|r| (r.start == a || r.end == a) && r.end > r.start);
            if drawn && !p.loop_enabled {
                app.exec_merging(
                    Command::SetLoopEnabled { enabled: true },
                    MergeKey::of(&"loop"),
                );
            }
        }
        app.arrangement.looping = None;
        app.arrangement.selecting = None;
        audio::edit::end(app);
        // Let go over the Session pane, a move puts the clips in its slots instead. // r33:launcher
        let session = |app: &mut CantusApp, d: &ClipDrag| {
            d.mode == DragMode::Move && crate::views::session::drop::from_arrangement(app, ui.ctx())
        }; // r33:launcher
        if let Some(d) = app.arrangement.drag.take().filter(|d| !session(app, d)) {
            // r33:launcher
            if d.mode == DragMode::Loop {
                // r39:lo-arrange — the loop notch: the clip repeats, drawn out to its end.
                tools::commit_loop(app, d.anchor, d.delta);
            } else {
                app.commit_clip_drag(
                    d.anchor,
                    match d.mode {
                        DragMode::Move => ClipDragResult::Move {
                            delta: d.delta,
                            rows: d.rows,
                            copy: d.copy,
                        },
                        DragMode::TrimStart => ClipDragResult::TrimStart { delta: d.delta },
                        DragMode::TrimEnd | DragMode::Loop => {
                            ClipDragResult::TrimEnd { delta: d.delta }
                        }
                        DragMode::Slip => ClipDragResult::Slip { delta: d.delta },
                    },
                );
            }
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
    let numbers = numbers_rect(view, f.ruler);
    tempo::metre_input(app, ui, view, numbers, s);
    tempo::input(app, ui, view, f.tempo, s);
    takes::input(app, ui, view, f.lanes, s);
    autolanes::input(app, ui, f.lanes, s);
    super::pitch_lane::input(app, ui, view, f.lanes, s);
    // r39:el-arrange — a new song's canvas says how to begin (over the timeline's response).
    if app.project().tracks.is_empty() && app.project().folders.is_empty() {
        empty::show(app, ui, f.lanes, s);
    }
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
    let _ = timeline_menu_after(app, ui, at_pointer, &[]);
}

/// r39:lo-arrange — the tool row's panel menu (dir-a's "…"): what the old row's keys did to
/// the selected clips (split, glue, duplicate, mute, repeat, make unique, the paint tool,
/// delete), the time selection's two, the snap, then the timeline's own entries — one menu.
pub(super) fn panel_menu(app: &mut CantusApp, ui: &mut egui::Ui) {
    use crate::keymap::Action;
    let snap = app.arrangement.snap;
    let entries = [
        Entry::Band("arrange.panel.clips"),
        Entry::Run(Action::SplitClip, "clip.split"),
        Entry::Run(Action::GlueClips, "clip.glue"),
        Entry::Run(Action::Duplicate, "clip.duplicate"),
        Entry::Run(Action::ToggleClipMute, "clip.mute"),
        Entry::Run(Action::ToggleClipLoop, "clip.loop_on"),
        Entry::Run(Action::MakeUnique, "clip.make_unique"),
        Entry::Run(Action::ToolPaint, "links.paint.key"),
        Entry::Run(Action::Delete, "clip.delete"),
        Entry::Separator,
        Entry::Run(Action::LoopToSelection, "action.loop-to-selection"),
        Entry::Run(Action::ZoomToSelection, "action.zoom-to-selection"),
        Entry::Toggle("arrange.panel.snap", snap),
        Entry::Separator,
    ];
    match timeline_menu_after(app, ui, false, &entries) {
        Some(12) => {
            app.arrangement.snap = !snap;
            ui.close();
        }
        Some(_) => ui.close(),
        None => {}
    }
}

/// The timeline's menu after `before` (another menu's own entries, one list): returns the
/// index of one of those clicked (an action of theirs has already run).
fn timeline_menu_after(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    at_pointer: bool,
    before: &[Entry<'_>],
) -> Option<usize> {
    use crate::keymap::Action;
    app.focus_arrangement();
    // The tempo map's entries (round 29, ADR 0060) in the same menu (review 2: a second
    // list drew a second menu box): where it was opened, or — the ▸ menu — the playhead.
    let at = menu_point(app, at_pointer);
    let (marker, metre) = if at_pointer {
        ("tempo.menu.add_marker", "tempo.menu.add_metre")
    } else {
        (
            "tempo.menu.add_marker_playhead",
            "tempo.menu.add_metre_playhead",
        )
    };
    let entries = [
        Entry::Shown(Action::Paste, "menu.paste"),
        Entry::Run(Action::SelectAll, "menu.select_all"),
        Entry::Separator,
        Entry::Own("swap.add_instrument"),
        Entry::Run(Action::AddAudioTrack, "rec.add_audio_track"),
        Entry::Separator,
        // Whether a paste, Duplicate or Alt-drag makes the same clip again (ADR 0037).
        Entry::Toggle("links.copies", app.settings.link_copies),
        // Round 31 (ADR 0070): automatic crossfades, and the clips' gain envelopes shown.
        Entry::Toggle("clipfx.menu.auto_crossfade", app.settings.auto_crossfade),
        Entry::Toggle("clipfx.menu.show_gain", app.settings.show_clip_gain),
        Entry::Separator,
        Entry::Own(marker),
        Entry::Own(metre),
        Entry::Toggle("tempo.view_menu", app.tempo_lane_shown()),
    ];
    let all: Vec<Entry<'_>> = before.iter().chain(&entries).copied().collect();
    let n = before.len();
    let clicked = menu(app, ui, &all)?;
    if clicked < n {
        return Some(clicked);
    }
    match Some(clicked - n) {
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
        Some(7) => {
            let on = !app.settings.auto_crossfade;
            app.set_auto_crossfade(on);
            ui.close();
        }
        Some(8) => {
            app.toggle_clip_gain();
            ui.close();
        }
        Some(10) => {
            if let Some(id) = app.add_marker(at) {
                let name = app.project().marker(id).map(|m| m.name.clone());
                markers::rename_now(app, id, name);
            }
            ui.close();
        }
        Some(11) => {
            tempo::add_metre_change(app, at);
            ui.close();
        }
        Some(12) => {
            app.toggle_tempo_lane();
            ui.close();
        }
        Some(_) => ui.close(),
        None => {}
    }
    None
}

/// Where the timeline menu's *Add a marker* and *Add a metre change* act: the point the
/// menu was opened on (a right-click, `at_pointer`), else the playhead.
pub fn menu_point(app: &CantusApp, at_pointer: bool) -> Ticks {
    let here = app.arrangement.menu_at.filter(|_| at_pointer);
    match here {
        // On the grid, as the marker lane's own entries (review 3).
        Some((t, _)) if app.arrangement.snap => {
            crate::views::bars::snap(app.project(), t.max(Ticks::ZERO), app.arrangement.grid)
        }
        Some((t, _)) => t.max(Ticks::ZERO),
        None => app.engine.position().max(Ticks::ZERO),
    }
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
    // What the menu acts on is this clip, whatever was chosen before it (review 3: a strip
    // or the master chosen on the desk took the clip menu's Bounce).
    app.inspector
        .follow(crate::views::inspector::Subject::Clip(track, clip));
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
    // Bounce in place (round 29, ADR 0064): the selected clips, to audio.
    entries.push(Entry::Run(Action::BounceInPlace, "bounce.clips"));
    entries.push(Entry::Run(Action::BounceSource, "bounce.clips_source"));
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
        // r36:ara — an ARA 2 plug-in to edit the clip (ADR 0097).
        super::ara_menu::clip_menu(app, ui, (track, clip));
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

/// The height a clip label's words need at full size (the label face's row).
fn label_row(ctx: &egui::Context, s: &Style) -> f32 {
    let font = kit::text::Role::Label.font(s.tk());
    ctx.fonts_mut(|f| f.row_height(&font)).max(1.0)
}
