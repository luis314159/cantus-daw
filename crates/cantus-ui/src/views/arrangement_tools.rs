//! r39:lo-arrange — the arrangement's tool row and its tools (dir-a's `b-toolbar-*` sheets,
//! des-arrange's *Panel*): the panel's name, the tools as one radio set (Select, Draw, Slice,
//! Erase, Mute, Zoom), the grid's well with its menu in musical order (Triplets, "Follow the
//! zoom"), and at the right Follow and the panel's menu. The row folds as the sheet does: the
//! grid's word and Follow's word, then the name to its glyph, then the tools to the chosen
//! one and a chevron.
//!
//! A tool is the view's state, never the song's: choosing one is not an undo step. What a
//! tool does to the song is a `Command` like any edit — a clip drawn is `InsertClip`, a
//! slice `clips::split`'s batch, an erase `RemoveClip`, a mute `SetClipMute`; a drag of the
//! eraser or the mute tool over several clips is one merged step. The tool keys reach the
//! arrangement's tools while it is the view being worked in (FL's letters, the number row),
//! the piano roll's otherwise, as before.

use super::super::clips_chrome::{self as chrome, Toolbar};
use super::{ArrangementView, Hit, TimelineGeometry};
use crate::app::CantusApp;
use crate::i18n::{t, tf};
use crate::icons::Icon;
use crate::keymap::Action;
use crate::kit::hint::{self, Caps};
use crate::kit::menu::{Check, Item};
use crate::kit::text::Src;
use crate::kit::{self, Style};
use cantus_core::{ClipId, Command, MergeKey, Ticks, TrackId, PPQ};
use egui::{pos2, Pos2, Rect};

/// The arrangement's mouse tools, in the sheet's order.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub enum Tool {
    /// Select, move and resize clips; a drag on empty lanes selects (what the view always
    /// did, and still the default).
    #[default]
    Select,
    /// Click on an empty lane to add a clip, drag to stretch it; clips still move.
    Draw,
    /// Click a clip to split it at the pointer, on the grid.
    Slice,
    /// Click a clip, or drag over clips, to delete them.
    Erase,
    /// Click a clip, or drag over clips, to mute or unmute them.
    Mute,
    /// Click to zoom in at the pointer (Alt: out); drag across a range to fit it.
    Zoom,
}

impl Tool {
    pub const ALL: [Tool; 6] = [
        Tool::Select,
        Tool::Draw,
        Tool::Slice,
        Tool::Erase,
        Tool::Mute,
        Tool::Zoom,
    ];

    pub fn icon(self) -> Icon {
        match self {
            Tool::Select => Icon::Select,
            Tool::Draw => Icon::Draw,
            Tool::Slice => Icon::Slice,
            Tool::Erase => Icon::Delete,
            Tool::Mute => Icon::Mute,
            Tool::Zoom => Icon::Zoom,
        }
    }

    /// The tool's name (an i18n key).
    pub fn label_key(self) -> &'static str {
        match self {
            Tool::Select => "arrange.tool.select",
            Tool::Draw => "arrange.tool.draw",
            Tool::Slice => "arrange.tool.slice",
            Tool::Erase => "arrange.tool.erase",
            Tool::Mute => "arrange.tool.mute",
            Tool::Zoom => "arrange.tool.zoom",
        }
    }

    /// What the tool does, in the help line's words (the sheet's: "Draw — click to add a
    /// clip, drag to stretch it").
    pub fn hint_key(self) -> &'static str {
        match self {
            Tool::Select => "arrange.tool.select.hint",
            Tool::Draw => "arrange.tool.draw.hint",
            Tool::Slice => "arrange.tool.slice.hint",
            Tool::Erase => "arrange.tool.erase.hint",
            Tool::Mute => "arrange.tool.mute.hint",
            Tool::Zoom => "arrange.tool.zoom.hint",
        }
    }

    /// The keymap's action that picks it: the same actions as the piano roll's tools, so
    /// FL's letters and the number row reach both.
    pub fn action(self) -> Action {
        match self {
            Tool::Select => Action::ToolSelect,
            Tool::Draw => Action::ToolDraw,
            Tool::Slice => Action::ToolSlice,
            Tool::Erase => Action::ToolDelete,
            Tool::Mute => Action::ToolMute,
            Tool::Zoom => Action::ToolZoom,
        }
    }

    /// The tool an action picks in the arrangement, if it picks one.
    pub fn of_action(a: Action) -> Option<Tool> {
        Tool::ALL.into_iter().find(|t| t.action() == a)
    }
}

/// A tool's tooltip and help line: what it does, and the key bound to it now.
fn tool_tip(app: &CantusApp, tool: Tool) -> String {
    let keys = app.keymap.display_for(tool.action());
    if keys.is_empty() {
        t(tool.hint_key())
    } else {
        format!("{} · {keys}", t(tool.hint_key()))
    }
}

// ---- the grid -----------------------------------------------------------------------

/// The straight grid values in musical order: the menu's label, the field's name, the
/// value. Their triplets are two thirds of each.
const STRAIGHT: [(&str, &str, i64); 6] = [
    ("arrange.grid.menu.bar", "grid.bar", PPQ * 4),
    ("grid.half", "grid.half", PPQ * 2),
    ("arrange.grid.menu.beat", "grid.quarter", PPQ),
    ("grid.eighth", "grid.eighth", PPQ / 2),
    ("grid.sixteenth", "grid.sixteenth", PPQ / 4),
    ("grid.thirtysecond", "grid.thirtysecond", PPQ / 8),
];

/// The triplets' names, by the straight value they are two thirds of.
const TRIPLET_NAMES: [&str; 6] = [
    "arrange.grid.triplet_bar",
    "arrange.grid.triplet_half",
    "arrange.grid.triplet_beat",
    "grid.triplet8",
    "grid.triplet16",
    "arrange.grid.triplet32",
];

/// Where a grid value sits among the straight ones, and whether it is their triplet.
fn place(g: Ticks) -> Option<(usize, bool)> {
    if let Some(i) = STRAIGHT.iter().position(|(_, _, v)| *v == g.0) {
        return Some((i, false));
    }
    STRAIGHT
        .iter()
        .position(|(_, _, v)| v * 2 / 3 == g.0)
        .map(|i| (i, true))
}

/// Whether `g` is a triplet grid (two thirds of a straight value).
pub fn is_triplet(g: Ticks) -> bool {
    place(g).is_some_and(|(_, t)| t)
}

/// `g` with its triplets switched: a straight value to its triplet and back. A value of
/// neither kind is left alone.
pub fn toggle_triplets(g: Ticks) -> Ticks {
    match place(g) {
        Some((i, false)) => Ticks(STRAIGHT[i].2 * 2 / 3),
        Some((i, true)) => Ticks(STRAIGHT[i].2),
        None => g,
    }
}

/// The grid's name as the field shows it ("1 bar", "1/8T").
pub fn grid_name(g: Ticks) -> String {
    match place(g) {
        Some((i, false)) => t(STRAIGHT[i].1),
        Some((i, true)) => t(TRIPLET_NAMES[i]),
        None => t("grid.custom"),
    }
}

/// The grid "Follow the zoom" picks: the finest straight value whose lines stand at least
/// 12 points apart at `px_per_beat` (a bar when even bars are closer), its triplet when
/// `triplets`.
pub fn auto_grid(px_per_beat: f32, triplets: bool) -> Ticks {
    let ticks_px = px_per_beat / PPQ as f32;
    let v = STRAIGHT
        .iter()
        .rev()
        .map(|(_, _, v)| *v)
        .find(|v| *v as f32 * ticks_px >= 12.0)
        .unwrap_or(STRAIGHT[0].2);
    let g = Ticks(v);
    if triplets {
        toggle_triplets(g)
    } else {
        g
    }
}

/// What the grid menu was asked for.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum GridPick {
    /// A value (straight; the caller keeps the triplets as they were).
    Value(usize),
    Triplets,
    FollowZoom,
}

/// A view's grid as its toolbar and menu change it: the value, and whether it follows the
/// zoom.
pub fn apply(grid: &mut Ticks, auto: &mut bool, pick: GridPick) {
    match pick {
        GridPick::Value(i) => {
            let v = Ticks(STRAIGHT[i].2);
            *grid = if is_triplet(*grid) {
                toggle_triplets(v)
            } else {
                v
            };
            *auto = false;
        }
        GridPick::Triplets => *grid = toggle_triplets(*grid),
        GridPick::FollowZoom => *auto = !*auto,
    }
}

/// The grid menu (dir-a's sheet): the values in musical order with a check on the current
/// one, then Triplets with its key and "Follow the zoom". `triplets_key` is the key bound to
/// the triplets now.
pub fn grid_menu(
    ui: &mut egui::Ui,
    current: Ticks,
    auto: bool,
    triplets_key: Option<&str>,
) -> Option<GridPick> {
    let here = place(current).map(|(i, _)| i);
    let mut items: Vec<Item<'_>> = STRAIGHT
        .iter()
        .enumerate()
        .map(|(i, (label, _, _))| Item::Action {
            label: Src::Key(label),
            check: Check::Radio(!auto && here == Some(i)),
            shortcut: None,
            submenu: false,
            enabled: true,
        })
        .collect();
    items.push(Item::Separator);
    items.push(Item::Action {
        label: Src::Key("arrange.grid.triplets"),
        check: if is_triplet(current) {
            Check::On
        } else {
            Check::Off
        },
        shortcut: triplets_key,
        submenu: false,
        enabled: true,
    });
    items.push(Item::Action {
        label: Src::Key("arrange.grid.follow_zoom"),
        check: if auto { Check::On } else { Check::Off },
        shortcut: None,
        submenu: false,
        enabled: true,
    });
    let (_, clicks) = kit::widgets::menu_list(ui, &items, 168.0);
    let n = STRAIGHT.len();
    clicks.clicked.and_then(|i| match i {
        i if i < n => Some(GridPick::Value(i)),
        i if i == n + 1 => Some(GridPick::Triplets),
        i if i == n + 2 => Some(GridPick::FollowZoom),
        _ => None,
    })
}

/// The grid field's words: the value, "Auto · 1/8" while it follows the zoom, "Off" while
/// the snap is off.
pub fn field_value(grid: Ticks, auto: bool, snap: bool) -> String {
    if !snap {
        t("arrange.grid.off")
    } else if auto {
        tf("arrange.grid.auto", &[("grid", &grid_name(grid))])
    } else {
        grid_name(grid)
    }
}

// ---- the tool row -------------------------------------------------------------------

/// The arrangement's tool row (dir-a `b-toolbar-*`): its name, the tools, the grid; Follow
/// and the panel's menu at the right.
pub(super) fn toolbar(app: &mut CantusApp, ui: &mut egui::Ui) {
    let id = ui.id().with("arrangement-toolbar");
    let mut bar = Toolbar::new(ui).named(crate::kit::glyph::Glyph::Arrangement);
    // The right end first: the menu and Follow keep their places while the left folds.
    let menu = bar.menu_right(ui, id.with("menu"), || t("arrangement.menu"));
    if let Some(r) = menu {
        egui::Popup::menu(&r)
            .frame(chrome::menu_frame())
            .show(|ui| super::panel_menu(app, ui));
    }
    let follow = bar.follow_right(
        ui,
        id.with("follow"),
        app.arrangement.follow,
        "arrange.follow",
        || t("arrange.follow.tip"),
    );
    if follow.is_some_and(|r| r.clicked()) {
        app.arrangement.follow = !app.arrangement.follow;
        app.arrangement.follow_last = None;
    }
    bar.title(ui, Src::Key("arrangement.title"), None);
    let set = Tool::ALL.map(|t| (t.icon(), t.label_key()));
    let chosen = Tool::ALL
        .iter()
        .position(|t| *t == app.arrangement.tool)
        .unwrap_or(0);
    if let Some(i) = bar.tools(ui, id.with("tools"), &set, chosen, |i| {
        tool_tip(app, Tool::ALL[i])
    }) {
        app.arrangement.tool = Tool::ALL[i];
        app.focus_arrangement();
    }
    let v = &app.arrangement;
    let value = field_value(v.grid, v.grid_auto, v.snap);
    let caption = t("arrangement.grid");
    let field = bar.field(ui, id.with("grid"), Some(&caption), &value, 58.0, || {
        t("arrange.grid.tip")
    });
    if let Some(r) = field {
        let keys = app.keymap.display_for(Action::GridTriplets);
        let keys = (!keys.is_empty()).then_some(keys);
        egui::Popup::menu(&r).frame(egui::Frame::NONE).show(|ui| {
            let v = &mut app.arrangement;
            if let Some(pick) = grid_menu(ui, v.grid, v.grid_auto, keys.as_deref()) {
                apply(&mut v.grid, &mut v.grid_auto, pick);
                v.snap = true;
                ui.close();
            }
        });
    }
}

/// The grid "Follow the zoom" keeps, brought up to the zoom before the view is drawn (a
/// view state: nothing repaints for it).
pub(super) fn follow_zoom(v: &mut ArrangementView) {
    if v.grid_auto {
        let g = auto_grid(v.px_per_beat, is_triplet(v.grid));
        if g != v.grid {
            v.grid = g;
        }
    }
}

/// Follow (dir-a: "the view keeps the playhead in sight"): while the song plays, when the
/// playhead crosses the view's edge — or starts outside it — the view turns the page, the
/// playhead a little in from the left. A view the user scrolled away from the playhead is
/// left there until the playhead comes into it and leaves it again; nothing moves at rest.
pub(super) fn follow_playhead(app: &mut CantusApp, lanes: Rect) {
    let playing = app.engine.is_playing();
    let at = super::ticks_to_beats(app.engine.position());
    let v = &mut app.arrangement;
    if !v.follow || !playing || v.drag.is_some() || v.px_per_beat <= 0.0 {
        v.follow_last = None;
        return;
    }
    let shown = lanes.width() / v.px_per_beat;
    if let Some(to) = page(v.scroll_beats, shown, at, v.follow_last) {
        v.scroll_beats = to;
    }
    v.follow_last = Some(at);
}

/// Where Follow turns the page to, if it does: the view starts at `scroll` and shows
/// `shown` beats, the playhead is at `at` and was at `last` when Follow last looked.
pub fn page(scroll: f32, shown: f32, at: f32, last: Option<f32>) -> Option<f32> {
    let inside = |b: f32| b >= scroll && b < scroll + shown;
    let turn = !inside(at)
        && match last {
            None => true,
            // It was in view, or it jumped (a loop's wrap, a seek).
            Some(p) => inside(p) || (at - p).abs() > shown * 0.5,
        };
    turn.then(|| (at - shown * 0.05).max(0.0))
}

/// The tool keys (FL's letters, the number row) while the arrangement is worked in: true
/// when `action` picked one of its tools.
pub fn pick_by_key(app: &mut CantusApp, action: Action) -> bool {
    match Tool::of_action(action) {
        Some(tool) => {
            app.arrangement.tool = tool;
            true
        }
        None => false,
    }
}

// ---- what the tools do on the lanes --------------------------------------------------

/// A tool's gesture in progress.
#[derive(Copy, Clone, Debug, PartialEq)]
pub(super) enum Gesture {
    /// A clip being drawn: its track, where it starts, where its end is now. Nothing is in
    /// the song until the button comes up: one `InsertClip`, one undo step.
    Draw {
        track: TrackId,
        start: Ticks,
        end: Ticks,
    },
    /// The eraser (`erase`) or the mute tool drawn over clips; `mute` is what the mute tool
    /// sets (the opposite of the first clip it met).
    Sweep { erase: bool, mute: bool },
    /// A range being drawn to zoom to: where it began and where the pointer is.
    Zoom { from: f32, to: f32 },
}

/// What the timeline hands the tools this frame.
pub(super) struct Ctx<'a> {
    pub view: &'a TimelineGeometry,
    pub body_top: f32,
    /// The timeline's snap (`off_grid`: Ctrl held).
    pub snap_to: &'a dyn Fn(Ticks, bool) -> Ticks,
    pub mods: egui::Modifiers,
    /// The clip under the pointer now, and where the button went down.
    pub hit: Option<Hit>,
    pub at_press: Option<Hit>,
}

impl Ctx<'_> {
    fn tick(&self, x: f32) -> Ticks {
        super::beats_to_ticks(self.view.beats(x).max(0.0))
    }

    /// The track whose playing lane is at `y` (not its take, automation or pitch lanes).
    fn track_at(&self, app: &CantusApp, y: f32) -> Option<TrackId> {
        self.view
            .row_at(y)
            .and_then(|i| app.project().tracks.get(i).map(|t| t.id))
    }
}

/// The grid line at or before `t` (the Draw tool starts a clip in the cell it is clicked
/// in, FL's way), through the metre changes; `t` itself while the snap is off or Ctrl held.
fn floor(app: &CantusApp, t: Ticks, off_grid: bool) -> Ticks {
    let v = &app.arrangement;
    if off_grid || !v.snap {
        return t;
    }
    let bar = crate::views::bars::bar_start(app.project(), t);
    if v.grid == crate::views::bars::BAR_GRID {
        return bar;
    }
    let g = v.grid.0.max(1);
    bar + Ticks((t - bar).0 / g * g)
}

/// The shortest clip the Draw tool makes by a drag: a grid step, a sixteenth without one.
fn least(app: &CantusApp) -> Ticks {
    let v = &app.arrangement;
    if v.snap {
        v.grid.max(Ticks(1))
    } else {
        Ticks::SIXTEENTH
    }
}

/// A click on the lanes with the tool in hand: true when the tool took it (the Select
/// tool's click, and the Draw tool's on a clip, are the view's own).
pub(super) fn click(app: &mut CantusApp, c: &Ctx<'_>, pos: Pos2) -> bool {
    if pos.y < c.body_top {
        return false;
    }
    let tool = app.arrangement.tool;
    match tool {
        Tool::Select => false,
        Tool::Draw if c.hit.is_some() => false,
        Tool::Draw => {
            if let Some(track) = c.track_at(app, pos.y) {
                app.focus_arrangement();
                let start = floor(app, c.tick(pos.x), c.mods.command);
                let end = crate::views::bars::add_bars(app.project(), start, 1);
                app.create_clip(track, start, end - start);
            }
            true
        }
        Tool::Slice => {
            if let Some(h) = c.hit {
                app.focus_arrangement();
                let at = (c.snap_to)(c.tick(pos.x), c.mods.command);
                slice(app, (h.track, h.clip), at);
            }
            true
        }
        Tool::Erase | Tool::Mute => {
            if let Some(h) = c.hit {
                app.focus_arrangement();
                let erase = tool == Tool::Erase;
                let mute = !muted(app, (h.track, h.clip));
                sweep(app, (h.track, h.clip), erase, mute);
                app.end_gesture();
            }
            true
        }
        Tool::Zoom => {
            zoom_at(app, c, pos.x, if c.mods.alt { 0.5 } else { 2.0 });
            true
        }
    }
}

/// A drag begun on the lanes with the tool in hand: true when the tool took it.
pub(super) fn drag_start(app: &mut CantusApp, c: &Ctx<'_>, pos: Pos2) -> bool {
    if pos.y < c.body_top {
        return false;
    }
    let tool = app.arrangement.tool;
    let gesture = match tool {
        Tool::Select => return false,
        // On a clip the Draw tool moves and resizes it, as the Select tool does.
        Tool::Draw if c.at_press.is_some() => return false,
        Tool::Draw => c.track_at(app, pos.y).map(|track| {
            let start = floor(app, c.tick(pos.x), c.mods.command);
            Gesture::Draw {
                track,
                start,
                end: start + least(app),
            }
        }),
        Tool::Slice => None,
        Tool::Erase => Some(Gesture::Sweep {
            erase: true,
            mute: false,
        }),
        Tool::Mute => Some(Gesture::Sweep {
            erase: false,
            mute: c.at_press.is_none_or(|h| !muted(app, (h.track, h.clip))),
        }),
        Tool::Zoom => Some(Gesture::Zoom {
            from: pos.x,
            to: pos.x,
        }),
    };
    app.focus_arrangement();
    app.arrangement.gesture = gesture;
    if let (Some(Gesture::Sweep { erase, mute }), Some(h)) = (gesture, c.at_press) {
        sweep(app, (h.track, h.clip), erase, mute);
    }
    true
}

/// The pointer moved with a tool's gesture held: true while one is.
pub(super) fn dragged(app: &mut CantusApp, c: &Ctx<'_>, pos: Pos2) -> bool {
    let Some(g) = app.arrangement.gesture else {
        return false;
    };
    let g = match g {
        Gesture::Draw { track, start, .. } => {
            let at = (c.snap_to)(c.tick(pos.x), c.mods.command);
            Gesture::Draw {
                track,
                start,
                end: at.max(start + least(app)),
            }
        }
        Gesture::Sweep { erase, mute } => {
            if let Some(h) = c.hit {
                sweep(app, (h.track, h.clip), erase, mute);
            }
            g
        }
        Gesture::Zoom { from, .. } => Gesture::Zoom { from, to: pos.x },
    };
    app.arrangement.gesture = Some(g);
    true
}

/// The button came up: the tool's gesture ends (a drawn clip goes into the song, a sweep
/// closes its undo step, a range is zoomed to). True when there was one.
pub(super) fn drag_stop(app: &mut CantusApp, c: &Ctx<'_>) -> bool {
    let Some(g) = app.arrangement.gesture.take() else {
        return false;
    };
    match g {
        Gesture::Draw { track, start, end } => app.create_clip(track, start, end - start),
        Gesture::Sweep { .. } => app.end_gesture(),
        Gesture::Zoom { from, to } => {
            if (to - from).abs() < 4.0 {
                zoom_at(app, c, from, if c.mods.alt { 0.5 } else { 2.0 });
            } else {
                let (a, b) = (c.view.beats(from.min(to)), c.view.beats(from.max(to)));
                let v = &mut app.arrangement;
                v.px_per_beat =
                    (c.view.width / (b - a).max(0.01)).clamp(super::PX_MIN, super::PX_MAX);
                v.scroll_beats = a.max(0.0);
            }
        }
    }
    true
}

fn muted(app: &CantusApp, r: (TrackId, ClipId)) -> bool {
    crate::clips::clip_of(app.project(), r).is_some_and(|c| c.muted)
}

/// The eraser or the mute tool over one clip: one merged step for the whole drag.
fn sweep(app: &mut CantusApp, r: (TrackId, ClipId), erase: bool, mute: bool) {
    if erase {
        if crate::clips::clip_of(app.project(), r).is_some() {
            app.exec_merging(
                Command::RemoveClip {
                    track: r.0,
                    clip: r.1,
                },
                MergeKey::of(&"arrange-eraser"),
            );
            app.validate_selection();
        }
        return;
    }
    for cmd in crate::clips::set_mute(app.project(), &[r], mute) {
        app.exec_merging(cmd, MergeKey::of(&"arrange-mute-tool"));
    }
}

/// The Slice tool: the clip in two at `at` (one undo step, both halves selected).
fn slice(app: &mut CantusApp, r: (TrackId, ClipId), at: Ticks) {
    let (commands, select) = app.with_ids(|project, ids| {
        let Some(clip) = crate::clips::clip_of(project, r) else {
            return (Vec::new(), Vec::new());
        };
        let Some(cmds) = crate::clips::split(project, r.0, clip, at, ids) else {
            return (Vec::new(), Vec::new());
        };
        let mut select = vec![r];
        for c in &cmds {
            if let Command::InsertClip { clip, .. } = c {
                select.push((r.0, clip.id));
            }
        }
        (cmds, select)
    });
    app.exec_clip_batch("cmd.split_clip", commands, select);
}

/// Zoom time by `k` about the screen x `at` (what is under it stays under it).
fn zoom_at(app: &mut CantusApp, c: &Ctx<'_>, at: f32, k: f32) {
    let anchor = c.view.beats(at);
    let v = &mut app.arrangement;
    v.px_per_beat = (v.px_per_beat * k).clamp(super::PX_MIN, super::PX_MAX);
    v.scroll_beats = (anchor - (at - c.view.left) / v.px_per_beat).max(0.0);
}

/// The pointer's shape and the help line for the tool in hand, over the lanes (the Select
/// tool's are the view's own).
pub(super) fn hover(app: &CantusApp, ui: &egui::Ui, c: &Ctx<'_>, pointer: Option<Pos2>) {
    let tool = app.arrangement.tool;
    if pointer.is_none_or(|p| p.y < c.body_top) {
        return;
    }
    if tool == Tool::Select || (tool == Tool::Draw && c.hit.is_some()) {
        return;
    }
    let cursor = match tool {
        Tool::Zoom if c.mods.alt => egui::CursorIcon::ZoomOut,
        Tool::Zoom => egui::CursorIcon::ZoomIn,
        Tool::Draw | Tool::Slice => egui::CursorIcon::Crosshair,
        _ if c.hit.is_some() => egui::CursorIcon::PointingHand,
        _ => egui::CursorIcon::Default,
    };
    ui.ctx().set_cursor_icon(cursor);
    hint::set_text(ui.ctx(), chrome::word(tool.hint_key()), "", Caps::NONE);
}

/// What the tool in hand draws over the lanes: the clip being drawn, the Slice tool's cut
/// under the pointer, the range being zoomed to.
pub(super) fn paint(
    b: &mut kit::Brush,
    app: &CantusApp,
    view: &TimelineGeometry,
    lanes: Rect,
    hit: Option<Hit>,
    pointer: Option<Pos2>,
    s: &Style,
) {
    use super::slate::Ink;
    let ink = Ink::of(s);
    let v = &app.arrangement;
    match v.gesture {
        Some(Gesture::Draw { track, start, end }) => {
            let row = app
                .project()
                .tracks
                .iter()
                .position(|t| t.id == track)
                .and_then(|i| view.rows.get(i));
            if let Some(row) = row {
                let x0 = view.x(super::ticks_to_beats(start));
                let x1 = view.x(super::ticks_to_beats(end));
                let r = b.snap_rect(Rect::from_min_max(
                    pos2(x0, row.top + s.k),
                    pos2(x1.max(x0 + 2.0), row.top + row.height - 2.0 * s.k),
                ));
                b.rrect(r, s.r(3.0), kit::Fill::Solid(Ink::a(ink.accent, 0.22)));
                b.rrect_stroke(r, s.r(3.0), b.px(), ink.accent);
            }
        }
        Some(Gesture::Zoom { from, to }) => {
            let column = Rect::from_min_max(
                pos2(from.min(to), lanes.top()),
                pos2(from.max(to), lanes.bottom()),
            );
            b.rect(
                b.snap_rect(column),
                kit::Fill::Solid(Ink::a(ink.ink1, 0.07)),
            );
            for x in [column.left(), column.right()] {
                b.vline(x, column.top(), column.bottom(), b.px(), ink.ink3);
            }
        }
        _ => {}
    }
    if v.tool == Tool::Slice && v.gesture.is_none() {
        if let (Some(h), Some(p)) = (hit, pointer) {
            // Where the cut would fall: on the grid, as the click will put it.
            let t = super::beats_to_ticks(view.beats(p.x).max(0.0));
            let at = if v.snap {
                crate::views::bars::snap(app.project(), t, v.grid)
            } else {
                t
            };
            let x = view.x(super::ticks_to_beats(at));
            if x > h.body.left() && x < h.body.right() {
                b.vline(x, h.body.top(), h.body.bottom(), 1.5 * s.k, ink.ink1);
            }
        }
    }
}

/// The loop notch let go (des-arrange: "a 9-px corner notch at the top right loops the
/// clip"): the clip's repeat switched on at the length it had, and its end drawn out —
/// one undo step.
pub(super) fn commit_loop(app: &mut CantusApp, anchor: (TrackId, ClipId), delta: Ticks) {
    let link = app.settings.link_copies;
    let result = super::ClipDragResult::TrimEnd { delta };
    let (trim, _) = app.with_ids(|project, ids| {
        crate::clips::drag_commands(project, &[anchor], anchor, result, link, ids)
    });
    let mut commands = crate::clips::set_loop(app.project(), &[anchor], true);
    commands.extend(trim);
    app.exec_clip_batch("cmd.set_clip_loop", commands, Vec::new());
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Follow turns the page when the playhead leaves the view, not while a view the user
    /// scrolled away is left alone, and again when a loop wraps or a seek jumps.
    #[test]
    fn follow_turns_the_page_only_when_the_playhead_leaves_the_view() {
        // In view: nothing.
        assert_eq!(page(0.0, 16.0, 8.0, Some(7.9)), None);
        // Crossing the right edge: the page turns, the playhead a little in.
        let to = page(0.0, 16.0, 16.1, Some(15.9)).unwrap();
        assert!(to > 15.0 && to < 16.1, "{to}");
        // Scrolled away by the user (it was outside last frame too): left alone.
        assert_eq!(page(40.0, 16.0, 20.0, Some(19.9)), None);
        // A loop's wrap back to the start, out of view: the page turns back.
        assert_eq!(page(40.0, 16.0, 0.0, Some(55.9)), Some(0.0));
        // Playback starting outside the view.
        assert!(page(0.0, 16.0, 64.0, None).is_some());
    }

    /// The grid menu's values are in musical order and each has its triplet; Triplets
    /// switches between them; "Follow the zoom" picks finer values as the view zooms in.
    #[test]
    fn the_grid_has_its_triplets_and_follows_the_zoom() {
        for (_, _, v) in STRAIGHT {
            let g = Ticks(v);
            assert!(!is_triplet(g));
            let t = toggle_triplets(g);
            assert!(is_triplet(t), "{v}");
            assert_eq!(t.0 * 3, v * 2);
            assert_eq!(toggle_triplets(t), g);
        }
        assert!(
            STRAIGHT.windows(2).all(|w| w[0].2 > w[1].2),
            "musical order"
        );
        let (mut g, mut auto) = (Ticks(PPQ / 4), false);
        apply(&mut g, &mut auto, GridPick::Triplets);
        assert_eq!(g, Ticks(PPQ / 6));
        apply(&mut g, &mut auto, GridPick::Value(0));
        assert_eq!(g, Ticks(PPQ * 4 * 2 / 3), "a value keeps the triplets");
        apply(&mut g, &mut auto, GridPick::FollowZoom);
        assert!(auto);
        let far = auto_grid(4.0, false);
        let near = auto_grid(200.0, false);
        assert!(near < far, "{near:?} {far:?}");
        assert_eq!(far, Ticks(PPQ * 4));
        assert!(is_triplet(auto_grid(80.0, true)));
        // The lines it picks stand 12 points apart at least.
        for px in [8.0f32, 26.0, 60.0, 140.0, 400.0] {
            let g = auto_grid(px, false);
            let apart = g.0 as f32 / PPQ as f32 * px;
            assert!(apart >= 12.0 || g == Ticks(PPQ * 4), "{px}: {apart}");
        }
    }

    /// Every tool has its own key action, and the action finds the tool again.
    #[test]
    fn every_tool_is_picked_by_its_action() {
        for t in Tool::ALL {
            assert_eq!(Tool::of_action(t.action()), Some(t));
        }
        assert_eq!(Tool::of_action(Action::ToolPaint), None);
        assert_eq!(Tool::default(), Tool::Select);
    }
}
