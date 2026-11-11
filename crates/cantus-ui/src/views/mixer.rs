//! The mixer desk (ADR 0024 §6): FL's strip, drawn by hand with the kit, fit first and
//! then scrolled.
//!
//! A desk has channels before anything is plugged into them, and so does this: a project
//! opens with sixteen strips and `+` adds more. A track feeds a strip (`Track::strip`); a
//! strip feeds the master or another strip (`MixerStrip::output`), which is what a bus is;
//! and a strip can *send* a post-fader copy into other strips (`MixerStrip::sends`), which
//! is what a return is. Faders, pans, EQ, sends and inserts here are the strip's own, and
//! the same commands the inspector issues, so the two views always agree.
//!
//! ## The strip
//!
//! Top to bottom: the colour number cell, the name (running up the strip below 36 pt of
//! width, elided in the middle above), the meter, the mute and solo lamps, pan, the fader,
//! (Wide and up) the fader's value, phase / swap / mono, width, the three EQ bars, the FX
//! bulb and record, (Extra large) the chain in one line, (Wide and up) the output chip,
//! (Extra large) the strip's own sends, FL's send row, and the cable band. The inserts
//! live in the inspector (round 25, ADR 0041 §9): Extra large is level, pan, sends and
//! routing. Every strip has the same rows at the same heights ([`layout::fit`]), so a row
//! reads straight across the desk, and a **legend column** at the left names each row
//! once — the strips carry values and icons, never the same word forty times.
//!
//! **Sends are FL's**: the sends row of every strip answers "can the *selected* strip go
//! here?" — a white ▲ (click: send it here; Ctrl+click: route it here only), a hollow ▲
//! when it would close a loop, a lit ▲ where it already goes, and a knob where it already
//! sends (drag: the level). The selected strip itself shows where it goes, and its menu
//! lists every destination — built when it opens, never every frame.
//!
//! **Cables** (round 25, [`cables`]): under the strips, every route drawn — the output
//! solid, a send thinner with its level on a pill (*PRE* when pre-fader), a key dashed in
//! the dynamics hue, a doubled signal with a warning mark — for the selection by default,
//! all of them or none from the title row's switch; the key beside it opens the view's
//! options (whose cables — out, in, both —, which kinds, the send levels, the outputs into
//! the master, quick views).
//!
//! ## The desk
//!
//! - **Width modes** Compact 24 / Wide 48 / Extra large 76 pt, from the title row or
//!   Ctrl+wheel over the desk, kept per window layout.
//! - **Fit first, then scroll**: rows drop in the ADR's order when the desk is short,
//!   each row family can be switched off in the ▾ menu, and when even that does not fit
//!   the desk scrolls vertically with the wheel and a visible scrollbar.
//! - The plain wheel over the numbers, names and meters scrolls the strips sideways;
//!   Shift+wheel moves the strip under the pointer; the wheel moves a fader or a knob only
//!   with Ctrl held.
//! - The meter and the fader share the flexible height; the 3-dot handle in the legend
//!   sets the split, and a double-click resets it.
//! - The master and the left dock are pinned at the left, the right dock (returns) at the
//!   right; the middle scrolls, and the selected strip is scrolled into view.
//! - Selection is FL's: click selects a strip (and the first track on it, so the inspector
//!   follows), Ctrl+click adds it to a multi-selection, Shift+click selects a run, and a
//!   fader dragged inside a multi-selection moves every selected fader by the same amount.
//!   The selection lights the fader caps and grooves lime and outlines the number cells.
//!   *Colourful mixer* tints whole strips in their colour.
//!
//! ## Cost (ADR 0024 §13)
//!
//! Only visible strips are laid out, metered and interacted with; everything the desk
//! derives from the project is worked out once per revision ([`desk`]); the whole desk is
//! painted through one [`Brush`] (a mesh per clip region, text on top) with every meter
//! in the same batched mesh; strings come from a per-language table built once
//! ([`Words`]); nothing is cloned or formatted a frame in steady state.

#[path = "mixer_cables.rs"]
mod cables;
#[path = "mixer_desk.rs"]
pub(crate) mod desk;
#[path = "mixer_drag.rs"]
mod drag;
#[path = "mixer_layout.rs"]
pub mod layout;
#[path = "mixer_look.rs"]
pub(crate) mod look; // r39:el-mixer
#[path = "mixer_menu.rs"]
mod menu;
#[path = "mixer_meters.rs"]
mod meters;
#[path = "mixer_paint.rs"]
mod paint;
#[path = "mixer_parts.rs"]
mod parts;
#[path = "mixer_wires.rs"]
pub(crate) mod wires; // r39:el-cables
pub(crate) use desk::short_name as desk_short_name;
pub use parts::chain_hint; // r39:el-mixer
#[path = "mixer_routehere.rs"]
mod routehere;
#[path = "mixer_rows.rs"]
mod rows; // r39:el-mixer
#[path = "mixer_strip.rs"]
mod strip;
#[path = "mixer_vca.rs"] // r36:mix
pub(crate) mod vca_ui; // r36:mix - VCA strips and their marks (ADR 0093)

use crate::app::CantusApp;
use crate::i18n::{self, t};
use crate::kit::{Brush, Style};
use cantus_core::{MixerStrip, StripId};
use layout::{CableOpts, CableView, Colourful, RowSet, WidthMode};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

pub use cables::Drawn as DrawnCable;
pub use parts::phase_button;
pub use vca_ui::open_snapshots; // r36:mix

/// Where the cable legend under ROUTING was drawn last frame (tests hold it whole and
/// apart): the row's cell in the legend column, the row's name, the three line samples
/// (with their words in the full key; alone, beside the name, in the compact one), and
/// whether the band was too short for the full key (round 26, ADR 0043 §8).
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct CableKey {
    pub cell: egui::Rect,
    pub name: egui::Rect,
    pub marks: Option<[egui::Rect; 3]>,
    pub compact: bool,
    /// The band's height handle (the 3 dots at the cell's right).
    pub handle: egui::Rect,
}
pub use strip::Control;

/// Where a control of the desk was last frame (`strip` `None` is the master), for tests
/// and tools that point at it.
pub fn control_rect(
    ctx: &egui::Context,
    strip: Option<StripId>,
    control: Control,
) -> Option<egui::Rect> {
    ctx.read_response(strip::pid(strip, control.part()))
        .map(|r| r.rect)
}

/// Where Compact's dot of a strip's send `k` was drawn last frame (round 39).
pub fn send_dot_rect(ctx: &egui::Context, strip: StripId, k: u8) -> Option<egui::Rect> {
    ctx.read_response(strip::pid(Some(strip), strip::Part::SendRow(k)).with("dot"))
        .map(|r| r.rect)
}

/// Where the key glyph of a strip's keyed insert `k` was drawn last frame (round 39).
pub fn key_rect(ctx: &egui::Context, strip: StripId, k: usize) -> Option<egui::Rect> {
    ctx.read_response(strip::pid(Some(strip), strip::Part::Chain).with(("key", k)))
        .map(|r| r.rect)
}

/// Where the tap chip of a strip's own send row `row` (Extra large) was drawn last frame.
pub fn tap_chip_rect(ctx: &egui::Context, strip: StripId, row: u8) -> Option<egui::Rect> {
    ctx.read_response(strip::pid(Some(strip), strip::Part::SendRow(row)).with("tap"))
        .map(|r| r.rect)
}

/// Where the selected send cable's tap chip (beside its pill) was drawn last frame.
pub fn pill_tap_rect(ctx: &egui::Context, from: StripId, to: StripId) -> Option<egui::Rect> {
    ctx.read_response(egui::Id::new(("mixer-cable-pill", from.0, to.0)).with("tap"))
        .map(|r| r.rect)
}

/// Where a send cable's level pill was drawn last frame.
pub fn pill_rect(ctx: &egui::Context, from: StripId, to: StripId) -> Option<egui::Rect> {
    ctx.read_response(egui::Id::new(("mixer-cable-pill", from.0, to.0)))
        .map(|r| r.rect)
}

/// Where the input jack a lit cable (`DrawnCable::edge`) plugs into was sensed last frame
/// (r39:el-cables: the jack is the cable's end; pulling it moves the route). `source`
/// is kept for callers; only the destination's end is sensed.
pub fn end_mark_rect(ctx: &egui::Context, edge: u32, source: bool) -> Option<egui::Rect> {
    ctx.read_response(egui::Id::new(("mixer-cable-end", edge, source)))
        .map(|r| r.rect)
}

/// Where a key of the desk's title row was drawn last frame: `menu`, `add`, `width`,
/// `wave`, `cables` (the view switch), `cable-options`, `takes`, `take`, or a desk-wide
/// key by its i18n key (`mixer.solo_off`, `mixer.unmute_all`).
pub fn title_rect(ctx: &egui::Context, key: &str) -> Option<egui::Rect> {
    ctx.read_response(egui::Id::new(("mixer-title", key)))
        .map(|r| r.rect)
}

/// `120 Hz`, `1.5 kHz` — the way a frequency is said, not stored.
pub fn hz_text(hz: f32) -> String {
    if hz >= 1_000.0 {
        format!("{:.1} kHz", hz / 1_000.0)
    } else {
        format!("{hz:.0} Hz")
    }
}

/// How the desk is set up in one window layout: saved per layout, so the mixing layout
/// can be wide and the tracking one compact.
#[derive(Copy, Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct DeskPrefs {
    pub width: WidthMode,
    pub rows: RowSet,
    /// The meter's share of the flexible height.
    pub ratio: f32,
    pub colourful: Colourful,
    /// Which cables the band draws (round 25: the selection's by default).
    pub cables: CableView,
    /// The cable band's height in points at 100 % (the legend's handle).
    pub band: f32,
    /// The cable view's options (the title row's options menu): whose cables, which
    /// kinds, how much detail. Absent from a desk saved before them: the default.
    pub cable_opts: CableOpts,
    /// The selected strip's inserts as a panel at the desk's right (FL's insert panel;
    /// r37:finish, ADR 0103 §48). Absent from older desks: off.
    pub inserts: bool,
    /// Read since round 39 (the owner's choices of 2026-11-09): a desk saved before has
    /// its cables hidden at rest and FL's row off once, as today's desks start.
    #[serde(default)]
    pub look39: bool,
}

impl Default for DeskPrefs {
    fn default() -> Self {
        DeskPrefs {
            width: WidthMode::default(),
            rows: RowSet::default(),
            ratio: layout::DEFAULT_RATIO,
            colourful: Colourful::default(),
            cables: CableView::default(),
            band: layout::BAND,
            cable_opts: CableOpts::default(),
            inserts: false,
            look39: true,
        }
    }
}

impl DeskPrefs {
    /// A desk saved before round 39, as today's start: the cables hidden at rest (the
    /// header's Cables key shows them), FL's "from selected" row off (Rows brings it back).
    pub fn to_look39(mut self) -> DeskPrefs {
        if !self.look39 {
            self.cables = CableView::Off;
            self.rows.set(layout::Row::Sends, false);
            self.look39 = true;
        }
        self
    }
}

impl DeskPrefs {
    /// The rows on: the ▾ menu's switches, and the cable band while the cable view is.
    pub fn shown(&self) -> RowSet {
        let mut r = self.rows;
        r.set(layout::Row::Cables, self.cables != CableView::Off);
        r
    }
}

/// The desk set-ups of every layout, and where they are kept.
#[derive(Debug, Default)]
struct Prefs {
    by_layout: BTreeMap<String, DeskPrefs>,
    /// `mixer.json` beside `layouts.json`; `None` (tests, screenshots) keeps them in
    /// memory only.
    path: Option<PathBuf>,
    /// The layout the desk was last shown in, to notice a switch or a *save as*.
    layout: String,
    current: DeskPrefs,
}

impl Prefs {
    /// The set-up for `layout`: its own when it has one; a layout just saved under a new
    /// name takes the one in use.
    fn follow(&mut self, layout: &str) {
        if self.layout == layout {
            return;
        }
        match self.by_layout.get(layout) {
            Some(p) => self.current = *p,
            None => {
                self.by_layout.insert(layout.to_string(), self.current);
            }
        }
        self.layout.clear();
        self.layout.push_str(layout);
    }

    /// Change the set-up in use, and keep it.
    fn set(&mut self, p: DeskPrefs) {
        if p == self.current && self.by_layout.get(&self.layout) == Some(&p) {
            return;
        }
        self.current = p;
        self.save();
    }

    /// Write the set-up in use into the map and, when the app gave one, the file.
    fn save(&mut self) {
        self.by_layout.insert(self.layout.clone(), self.current);
        if let Some(path) = &self.path {
            if let Ok(text) = serde_json::to_string_pretty(&self.by_layout) {
                if let Some(dir) = path.parent() {
                    let _ = std::fs::create_dir_all(dir);
                }
                if let Err(e) = std::fs::write(path, text) {
                    log::warn!("cannot write {}: {e}", path.display());
                }
            }
        }
    }
}

/// What a text box on the desk is renaming.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub(crate) enum Renaming {
    Strip(StripId),
    Group(cantus_core::GroupId),
    /// A mixer snapshot's name (round 36, ADR 0093). // r36:mix
    Snapshot(cantus_core::mix_snap::MixSnapId), // r36:mix
}

/// Every word the desk shows or hints with, looked up once per language instead of once
/// a frame (`i18n::t` returns a fresh `String` each call).
#[derive(Debug, Default)]
pub(crate) struct Words {
    locale: Option<i18n::Locale>,
    pub mute: String,
    pub solo: String,
    pub pan: String,
    pub balance: String,
    pub volume: String,
    pub width: String,
    pub phase: String,
    pub swap: String,
    pub mono: String,
    pub stop: String,
    pub level: String,
    pub peak: String,
    pub eq: [String; 3],
    pub send_free: String,
    pub send_loop: String,
    pub send_lit: String,
    pub send_knob: String,
    pub route: String,
    pub add_strip: String,
    pub vca: String,      // r36:mix
    pub snap: String,     // r36:mix
    pub snap_tip: String, // r36:mix
    pub master: String,
    /// The master's number and name: a click shows its chain, a right-click its menu.
    pub master_head: String,
    pub modes: String,
    pub chain: String,
    pub chain_empty: String,
    pub output: String,
    pub send_row: String,
    pub send_add: String,
    pub send_more: String,
    pub port: String,
    pub cable: String,
    pub cable_opts: String,
    pub pill: String,
    pub chip: String,
    pub send_off: String,
    pub pre: String,
    pub kind: [String; 3],
    pub tap: [String; 2],
    pub group: String,
    pub name: String,
    pub takes: String,
    // Round 39 (des-mixer).
    pub sends_in: String,
    pub strips_n: String,
    pub lufs: String,
    pub out_main: String,
    pub master_short: String,
    pub soloed: String,
    pub clear: String,
    pub clear_tip: String,
    pub rows: String,
    pub rows_tip: String,
    pub kind_track: String,
    pub kind_bus: String,
    pub kind_return: String,
    pub more_right: String,
    /// FL's next empty strips' hint (round 39, lo-mixer).
    pub next_strip: String,
    pub more_left: String,
    pub routes: String,
    pub tag_out: String,
    pub tag_send: String,
    pub tag_key: String,
    pub cables: String,
    pub cables_tip: String,
    pub key_from: String,
    pub send_level: String,
}

impl Words {
    fn refresh(&mut self) {
        let now = i18n::locale();
        if self.locale == Some(now) {
            return;
        }
        *self = Words {
            locale: Some(now),
            mute: t("mixer.mute_strip"),
            solo: t("mixer.solo_strip"),
            pan: t("track.pan"),
            balance: t("mixer.hint.balance"),
            volume: t("mixer.legend.fader"),
            width: t("track.width"),
            phase: t("track.phase_tip"),
            swap: t("mixer.swap_tip"),
            mono: t("track.mono_tip"),
            stop: t("take.stop_tip"),
            level: t("mixer.legend.meter"),
            peak: t("mixer.hint.peak"),
            eq: [
                t("inspector.eq_band_low"),
                t("inspector.eq_band_mid"),
                t("inspector.eq_band_high"),
            ],
            send_free: t("mixer.hint.send_free"),
            send_loop: t("mixer.hint.send_loop"),
            send_lit: t("mixer.hint.send_lit"),
            send_knob: t("mixer.hint.send_knob"),
            route: t("mixer.hint.route"),
            add_strip: t("mixer.add_strip"),
            vca: t("mix.vca.plate"),     // r36:mix
            snap: t("mix.snap.key"),     // r36:mix
            snap_tip: t("mix.snap.tip"), // r36:mix
            master: t("mixer.master"),
            master_head: t("master.head_tip"),
            modes: t("mixer.hint.modes"),
            chain: t("mixer.hint.chain"),
            chain_empty: t("mixer.hint.chain_empty"),
            output: t("mixer.hint.output"),
            send_row: t("mixer.hint.send_row"),
            send_add: t("mixer.hint.send_add"),
            send_more: t("mixer.hint.send_more"),
            port: t("cables.hint.port"),
            cable: t("cables.hint.cable"),
            cable_opts: t("cables.hint.options"),
            pill: t("cables.hint.pill"),
            chip: t("cables.hint.chip"),
            send_off: t("cables.send_off"),
            pre: t("routing.tap.pre"),
            kind: [
                t("cables.kind.output"),
                t("cables.kind.send"),
                t("cables.kind.key"),
            ],
            tap: [t("cables.tap.post"), t("cables.tap.pre")],
            group: t("mixer.hint.group"),
            name: t("mixer.hint.name"),
            takes: t("mixer.takes"),
            sends_in: t("mixer.look.sends_in"),
            strips_n: t("mixer.look.strips"),
            lufs: t("mixer.look.lufs"),
            out_main: t("mixer.look.out_main"),
            master_short: t("mixer.look.master"),
            soloed: t("mixer.look.soloed"),
            clear: t("mixer.look.clear"),
            clear_tip: t("mixer.look.clear_tip"),
            rows: t("mixer.look.rows"),
            rows_tip: t("mixer.look.rows_tip"),
            kind_track: t("mixer.look.track"),
            kind_bus: t("mixer.look.bus"),
            kind_return: t("mixer.look.return"),
            more_right: t("mixer.look.more_right"),
            next_strip: t("lo_mixer.next_strip"),
            more_left: t("mixer.look.more_left"),
            routes: t("mixer.look.routes"),
            tag_out: t("mixer.look.tag_out"),
            tag_send: t("mixer.look.tag_send"),
            tag_key: t("mixer.look.tag_key"),
            cables: t("mixer.look.cables"),
            cables_tip: t("mixer.look.cables_tip"),
            key_from: t("mixer.look.key_from"),
            send_level: t("mixer.look.send_knob"),
        };
    }
}

/// The desk's own state: meters, the derived desk, the set-up per layout, scrolling,
/// menus, and the buffers a frame paints through.
#[derive(Default)]
pub struct MixerView {
    meters: meters::Meters,
    desk: desk::Desk,
    prefs: Prefs,
    pub(crate) words: Words,
    /// The name being typed, and what it renames.
    rename: Option<(Renaming, String)>,
    /// Where the renamed strip's name showed this frame (set while painting it).
    pub(crate) rename_at: Option<egui::Rect>,
    rename_focused: bool,
    rename_serial: u64,
    /// The middle dock's scroll, and the whole desk's vertical scroll, in points.
    scroll_x: f32,
    scroll_y: f32,
    /// The selection the desk last scrolled to.
    followed: Option<StripId>,
    /// Wheel notches not yet turned into a step (a trackpad sends fractions).
    notch_acc: f32,
    /// Ctrl+wheel notches this frame, for the fader or knob under the pointer.
    pub(crate) ctrl_notches: f32,
    popups: menu::Popups,
    /// The desk as it was before the A/B compare put a snapshot on it (round 36). // r36:mix
    pub(crate) snap_ab: Option<(
        cantus_core::mix_snap::MixSnapId,
        Box<cantus_core::mix_snap::MixState>,
        u64,
    )>, // r36:mix: the bus revision right after its recall
    /// The cable band's state: the selected cable, what was drawn this frame (round 25,
    /// `mixer_cables`).
    band: cables::Band,
    /// A cable, a cable's end, a track, a strip or a group being dragged (round 25,
    /// `mixer_drag`, the interaction builder's).
    drag: drag::Drag,
    /// Why the last click on a from-selected arrow did nothing (its strip, the words),
    /// said in the hint bar while the pointer stays on that arrow.
    pub(crate) arrow_said: Option<(StripId, String)>,
    /// The desk's one brush: a mesh per clip region, the text on top.
    brush: Brush,
    /// Reused for numbers and hint names, so a frame formats without allocating.
    pub(crate) buf: String,
    pub(crate) hint_buf: String,
    /// Reused for the strips' EQ curves (round 39).
    pub(crate) pts: Vec<egui::Pos2>,
    /// Compact: the send whose knob is open (its strip, its index).
    pub(crate) send_pop: Option<(StripId, usize)>,
    /// A keyed insert's key under the pointer: the source strip lit (this frame's ask, and
    /// last frame's, which the strips draw).
    pub(crate) key_lit: Option<StripId>,
    pub(crate) key_shown: Option<StripId>,
    /// The track icons the strip heads drew this frame, and where (round 39; tests read
    /// them through [`MixerView::head_icon`]).
    pub(crate) icons_drawn: Vec<(StripId, egui::Rect, &'static str)>,
    /// The room the workspace's tab row leaves right of its tabs, when the desk is docked
    /// under one (round 39, des-mixer: the tabs, the density and the header share one 28-pt
    /// row). Set by the workspace before every frame; `None` (floating, narrow, a test's
    /// bare desk): the header has its own row.
    pub(crate) bar_room: Option<egui::Rect>,
    /// How many of FL's next empty strips the desk painted last frame (round 39).
    pub(crate) ghosts_drawn: usize,
    /// The last frame's counters (the frame-budget test reads them).
    laid_out: usize,
    desk_width: f32,
    /// The egui pass the desk was last drawn on: a gap means it left the screen in
    /// between (its pane closed, another tab over it) — the clean "the desk was hidden"
    /// signal an armed route lets go on (round 26, ADR 0044 §10).
    shown_pass: u64,
    /// Where the cable legend went last frame (`CableKey`).
    pub(crate) cable_key: Option<CableKey>,
    /// The desk's rows area last frame (where anything of the desk is visible).
    rows_seen: Option<egui::Rect>,
    /// The right dock's rect in the last frame (r37:finish, tests).
    right_seen: Option<egui::Rect>,
    /// The middle's rect in the last frame (r39:fix2, tests).
    middle_seen: Option<egui::Rect>,
    /// One digit's width in the overflow pill's face (10 pt SemiBold), measured each frame:
    /// the pill's slot is as wide as its count needs (r39:fix2).
    digit_w: f32,
}

impl std::fmt::Debug for MixerView {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MixerView")
            .field("prefs", &self.prefs.current)
            .field("scroll", &(self.scroll_x, self.scroll_y))
            .field("rename", &self.rename)
            .finish_non_exhaustive()
    }
}

impl MixerView {
    /// Where the head of `strip` drew its track's icon last frame, and which icon (round
    /// 39, ADR 0113 §4).
    pub fn next_strips_shown(&self) -> usize {
        self.ghosts_drawn
    }

    /// Where the head of `strip` drew its track's icon last frame, and which icon (round
    /// 39, ADR 0113 §4).
    pub fn head_icon(&self, strip: StripId) -> Option<(egui::Rect, &'static str)> {
        self.icons_drawn
            .iter()
            .find(|(s, _, _)| *s == strip)
            .map(|(_, r, g)| (*r, *g))
    }

    /// Start renaming a strip in place (the menu's *Rename…* and a double-click).
    /// Whether a strip or a group is being renamed.
    pub fn renaming(&self) -> bool {
        self.rename.is_some()
    }

    pub fn begin_rename(&mut self, strip: &MixerStrip) {
        self.rename = Some((Renaming::Strip(strip.id), strip.name.clone()));
    }

    /// Keep the desk's set-ups in `mixer.json` beside the layouts file: read it now, and
    /// write it whenever a set-up changes. The app calls this once, at start-up; without
    /// it (tests, screenshots) the set-ups live in memory only.
    pub fn load_prefs(&mut self, layouts_path: &Path) {
        let path = layouts_path.with_file_name("mixer.json");
        if let Ok(text) = std::fs::read_to_string(&path) {
            match serde_json::from_str::<BTreeMap<String, DeskPrefs>>(&text) {
                Ok(mut map) => {
                    for p in map.values_mut() {
                        *p = p.to_look39();
                    }
                    self.prefs.by_layout = map;
                }
                Err(e) => log::warn!("mixer.json is not usable ({e}); using the defaults"),
            }
        }
        self.prefs.layout.clear();
        self.prefs.path = Some(path);
    }

    /// The desk's set-up in the current layout.
    pub fn prefs(&self) -> DeskPrefs {
        self.prefs.current
    }

    /// Change the set-up (and keep it, when the app gave a file).
    pub fn set_prefs(&mut self, p: DeskPrefs) {
        self.prefs.set(p);
    }

    /// Strips laid out in the last frame: only the visible ones (ADR 0024 §13).
    pub fn strips_laid_out(&self) -> usize {
        self.laid_out
    }

    /// The middle dock's width in the last frame, in points.
    pub fn desk_width(&self) -> f32 {
        self.desk_width
    }

    /// Cables the band drew in the last frame (the frame-budget test reads it).
    pub fn cables_drawn(&self) -> usize {
        self.band.drawn.len()
    }

    /// What the band drew in the last frame, for tests: each cable's route, its two ends
    /// on screen, and whether it was lit.
    pub fn cables(&self) -> &[cables::Drawn] {
        &self.band.drawn
    }

    /// A drawn cable's points on screen (r39:el-cables: its curve or subway route).
    pub fn cable_path(&self, d: &cables::Drawn) -> &[egui::Pos2] {
        self.band.path(d)
    }

    /// A point on a drawn cable, half way along it (where a test points at it).
    pub fn cable_point(&self, d: &cables::Drawn) -> egui::Pos2 {
        wires::halfway(self.band.path(d))
    }

    /// How many pieces of Flow's gleam the band drew in the last frame.
    pub fn cable_gleams(&self) -> u32 {
        self.band.gleams
    }

    /// The desk's rows area in the last frame: what of the desk is visible.
    pub fn rows_seen(&self) -> egui::Rect {
        self.rows_seen.unwrap_or(egui::Rect::NOTHING)
    }

    /// The right dock in the last frame (its strips: the returns docked right). // r37:finish
    pub fn right_dock_seen(&self) -> egui::Rect {
        self.right_seen.unwrap_or(egui::Rect::NOTHING)
    }

    /// The middle of the desk in the last frame: where its scrolling strips show
    /// (r39:fix2).
    pub fn middle_seen(&self) -> egui::Rect {
        self.middle_seen.unwrap_or(egui::Rect::NOTHING)
    }

    /// Where the cable legend went in the last frame (`None` while the band is off).
    pub fn cable_key(&self) -> Option<CableKey> {
        self.cable_key
    }

    /// Where the band's level pills went in the last frame (tests hold that none overlaps
    /// another or a port).
    pub fn cable_pills(&self) -> &[egui::Rect] {
        self.band.pills()
    }

    /// The cable selected on the desk (a click on it; Delete removes it).
    pub fn selected_cable(&self) -> Option<cantus_core::routing::Route> {
        self.band.selected
    }

    /// The selected cable Delete may remove: one the band drew on the last pass (never a
    /// cable hidden by the *Off* view, scrolled out of sight, or on a desk not on screen).
    pub fn deletable_cable(&self) -> Option<cantus_core::routing::Route> {
        let sel = self.band.selected?;
        let fresh = self.band.pass + 1 >= self.band.keys_pass;
        (fresh && self.band.drawn.iter().any(|d| d.route == sel)).then_some(sel)
    }

    /// Whether Esc belongs to the desk now: a drag in progress (or a route armed by a
    /// click) cancels, a selected cable is let go — the keymap's *Clear selection* stays
    /// out of it.
    pub fn claims_escape(&self) -> bool {
        // Only a desk on screen (drawn on the last pass) — a hidden one never eats Esc.
        let shown = self.band.pass + 1 >= self.band.keys_pass;
        shown && (drag::active(self) || self.band.selected.is_some())
    }

    /// Open the selected strip's route menu on the next frame (the keymap's *Route menu*,
    /// Shift+F10).
    pub fn want_route_menu(&mut self) {
        self.drag.route_menu = true;
    }

    /// The keymap runs on this egui pass (see [`Self::deletable_cable`]).
    pub fn keys_on_pass(&mut self, pass: u64) {
        self.band.keys_pass = pass;
    }

    /// Forget the selected cable (it was removed, or the selection moved on).
    pub fn clear_cable_selection(&mut self) {
        self.band.selected = None;
    }

    /// The middle dock's horizontal scroll, and the desk's vertical one, in points.
    pub fn scroll(&self) -> (f32, f32) {
        (self.scroll_x, self.scroll_y)
    }

    /// The strip a click armed a route from (click mode: the next click on a strip makes
    /// the route), if any.
    pub fn armed(&self) -> Option<StripId> {
        drag::armed(self)
    }

    /// The strip a keyed insert's key under the pointer lights (round 39).
    pub fn key_lit_strip(&self) -> Option<StripId> {
        self.key_lit.or(self.key_shown)
    }

    /// Select a cable as a click on it does (tools and pictures).
    pub fn select_cable(&mut self, route: Option<cantus_core::routing::Route>) {
        self.band.selected = route;
    }

    /// How many strips the multi-selection holds (1 for a single selection), without
    /// building the list `CantusApp::strip_selection` returns.
    pub(crate) fn selection_len(&self, app: &CantusApp) -> usize {
        match app.selected_strip {
            Some(s) if app.selected_strips.contains(&s) => app.selected_strips.len(),
            Some(_) => 1,
            None => 0,
        }
    }
}

/// Whether `id` is in the strip selection (the multi-selection while the selected strip
/// is part of it, else the selected strip alone) — without allocating.
pub(crate) fn in_selection(app: &CantusApp, id: StripId) -> bool {
    match app.selected_strip {
        Some(s) if app.selected_strips.contains(&s) => app.selected_strips.contains(&id),
        Some(s) => s == id,
        None => false,
    }
}

/// Tests of the cable band (round 25's): the desk with the selection's cables shown, as the
/// header's Cables key leaves it (round 39: nothing is drawn at rest).
pub fn with_cables(mut app: CantusApp) -> CantusApp {
    let mut p = app.mixer.prefs.current;
    p.cables = CableView::Selected;
    app.mixer.prefs.current = p;
    app
}

/// Screenshots and tests: put the desk in a width mode without keeping it.
pub fn set_width_mode(app: &mut CantusApp, mode: WidthMode) {
    let p = DeskPrefs {
        width: mode,
        ..app.mixer.prefs.current
    };
    app.mixer.prefs.current = p;
    let layout = app.mixer.prefs.layout.clone();
    app.mixer.prefs.by_layout.insert(layout, p);
}

/// Screenshots and tests: show or hide the insert panel. // r37:finish
pub fn set_inserts_panel(app: &mut CantusApp, on: bool) {
    let p = DeskPrefs {
        inserts: on,
        ..app.mixer.prefs.current
    };
    app.mixer.prefs.current = p;
    let layout = app.mixer.prefs.layout.clone();
    app.mixer.prefs.by_layout.insert(layout, p);
}

/// Screenshots and tests: show or hide a family of the desk's rows.
pub fn set_row(app: &mut CantusApp, row: layout::Row, on: bool) {
    let mut p = app.mixer.prefs.current;
    p.rows.set(row, on);
    app.mixer.prefs.current = p;
    let layout = app.mixer.prefs.layout.clone();
    app.mixer.prefs.by_layout.insert(layout, p);
}

/// Screenshots and tests: open a strip's menu (`None`: the desk's) with its pointer at
/// `at`, and optionally one of its submenus by row.
pub fn open_menu(app: &mut CantusApp, strip: Option<StripId>, at: egui::Pos2) {
    let m = match strip {
        Some(id) if id.is_master() => menu::Menu::Master,
        Some(id) => menu::Menu::Strip(id),
        None => menu::Menu::Desk,
    };
    app.mixer.popups.open(m, at);
}

/// Screenshots and tests: open an insert's menu (as a right-click on its name in the
/// strip's chain line does) with its pointer at `at`.
pub fn open_slot_menu(
    app: &mut CantusApp,
    strip: StripId,
    effect: cantus_core::EffectId,
    at: egui::Pos2,
) {
    app.mixer.popups.open(menu::Menu::Slot(strip, effect), at);
}

/// Screenshots and tests: open the cable view's options menu under its title-row key
/// (as a click on it does; the key must have been drawn on an earlier frame), or at `at`.
pub fn open_cable_options(app: &mut CantusApp, ctx: &egui::Context, at: egui::Pos2) {
    let key = title_rect(ctx, "cable-options");
    let at = key.map_or(at, |r| r.left_bottom());
    app.mixer.popups.open_from(menu::Menu::Cables, at, key);
}

/// Show the desk. Takes the view out of the app for the frame, so painting can borrow
/// the project while the view's own buffers are written.
pub fn show(app: &mut CantusApp, ui: &mut egui::Ui) {
    let mut view = std::mem::take(&mut app.mixer);
    view.frame(app, ui);
    // Anything the frame queued for the app (menus, renames) was applied inside; put the
    // view back.
    app.mixer = view;
}

/// Pin the cable band to the desk's foot while the desk is scrolled away from it (round
/// 26, ADR 0043 §8): the routing — the ports, the cables, the legend's key — never
/// scrolls out of sight; the strips' rows above it scroll under its edge. At the foot of
/// the scroll the band is where it would be anyway.
fn pin_band(mut fit: layout::Fit, mut geo: Geo) -> (layout::Fit, Geo) {
    let i = layout::Row::Cables.index();
    let Some((a, z)) = fit.rows[i] else {
        return (fit, geo);
    };
    let (top, bottom) = (geo.y0 + a, geo.y0 + z);
    if bottom <= geo.rows.bottom() + 0.5 && top >= geo.rows.top() - 0.5 {
        return (fit, geo);
    }
    let h = z - a;
    let foot = geo.rows.bottom();
    let top = (foot - h).max(geo.rows.top());
    fit.rows[i] = Some((top - geo.y0, foot - geo.y0));
    geo.strips = egui::Rect::from_min_max(geo.rows.min, egui::pos2(geo.rows.right(), top));
    geo.pinned = true;
    (fit, geo)
}

/// The owner id the desk opens `views::route_menu` menus under.
pub(crate) fn route_owner() -> egui::Id {
    egui::Id::new("mixer.route_menu")
}

/// The style the desk paints with: the installed tokens at 100 %.
fn style() -> Style {
    Style::current()
}

/// The desk's fixed sizes, in points at 100 %.
pub(crate) mod size {
    /// The title row: the desk's menu, `+`, the width modes, the title.
    pub const TITLE: f32 = 28.0;
    /// The desk's left inset (des-mixer: the master at x 8; no legend column).
    pub const LEGEND: f32 = 8.0;
    /// The band under the strips holding the horizontal scrollbar (strips end 4 above
    /// the desk's foot; the 3-pt bar sits there).
    pub const HBAR: f32 = 4.0;
    /// The column at the right holding the vertical scrollbar, when the desk scrolls.
    pub const VBAR: f32 = 8.0;
    /// Between the docks and the middle (des-mixer: 8; the right dock's seam 6 left of it).
    pub const DIVIDER: f32 = 8.0;
    /// The slot each side of a scrolling middle where its overflow's pill sits: its widest
    /// (a two-digit count; [`super::MixerView::slot_for`] fits it to its count).
    pub const OVERFLOW: f32 = 36.0;
    /// The clearance each side of the pill in its slot.
    pub const SLOT_PAD: f32 = 2.0;
    /// Above the first row.
    pub const TOP: f32 = 4.0;
    /// The master strip, per width mode (its body and the 8 pt before the middle are
    /// `DIVIDER`'s).
    pub fn master(mode: super::WidthMode) -> f32 {
        super::look::master_w(mode)
    }
}

impl Geo {
    /// The header's room in the tab row, when it has one.
    fn with_title(mut self, bar: Option<egui::Rect>) -> Geo {
        if let Some(r) = bar {
            self.title = r;
        }
        self
    }
}

/// Where everything is this frame.
#[derive(Copy, Clone, Debug)]
pub(crate) struct Geo {
    pub full: egui::Rect,
    pub title: egui::Rect,
    /// The strips' area (under the title, above the scrollbar band).
    pub rows: egui::Rect,
    pub legend: egui::Rect,
    /// The pinned left dock (master first) and right dock.
    pub left: egui::Rect,
    pub right: egui::Rect,
    /// The scrolling middle.
    pub middle: egui::Rect,
    pub hbar: egui::Rect,
    pub vbar: Option<egui::Rect>,
    /// The screen y of the top of the first row.
    pub y0: f32,
    pub pitch: f32,
    pub master_w: f32,
    /// The middle's content width.
    pub content_w: f32,
    /// Where the strips' rows show: `rows`, less the cable band when it is pinned to the
    /// desk's foot.
    pub strips: egui::Rect,
    /// The cable band is pinned to the desk's foot (the desk scrolls; the routing stays).
    pub pinned: bool,
    /// The header sits in the workspace's tab row (`title` is that room).
    pub merged: bool,
    /// The middle scrolls (its strips do not all fit).
    pub scrolls: bool,
    /// The widths of the overflow's slots left and right of the middle, where its pills
    /// sit: 0 on a side with no strip hidden beyond it (r39:fix2 — a slot only where
    /// there is something to count, so no empty slot takes a strip's room).
    pub slot_l: f32,
    pub slot_r: f32,
    /// The rows' right edge, left of the vertical scrollbar when there is one: the band
    /// stops there, so the bar's track stays the bar's (second review, 9).
    pub inner_right: f32,
}

impl MixerView {
    /// One frame of the desk.
    fn frame(&mut self, app: &mut CantusApp, ui: &mut egui::Ui) {
        let now = ui.ctx().cumulative_pass_nr();
        if self.shown_pass != 0 && self.shown_pass + 1 < now {
            drag::desk_was_hidden(self, ui.ctx());
        }
        self.shown_pass = now;
        self.words.refresh();
        self.prefs.follow(&app.layouts.current.name);
        self.desk.refresh(app);
        self.meters.begin_frame();
        let s = style();
        let outer = ui.available_rect_before_wrap();
        // r37:finish — the insert panel at the desk's right, when it is on and there is
        // room for it beside a useful desk (ADR 0103 §48).
        let pw = super::mixer_inserts::WIDTH * s.k;
        let panel = (self.prefs.current.inserts && outer.width() >= pw + 480.0 * s.k).then(|| {
            egui::Rect::from_min_max(egui::pos2(outer.right() - pw, outer.top()), outer.max)
        });
        let full = match panel {
            Some(p) => egui::Rect::from_min_max(
                outer.min,
                egui::pos2(p.left() - size::DIVIDER, outer.bottom()),
            ),
            None => outer,
        };
        // The docks never cover each other (final review): where the master, the left
        // dock, the right dock and a strip of the middle do not fit side by side, the right
        // dock scrolls with the middle, after its last strip.
        let pitch = self.prefs.current.width.pitch();
        let pinned = size::LEGEND
            + size::master(self.prefs.current.width)
            + desk::items_width(&self.desk.left, pitch)
            + self.desk.right_docked_width(pitch)
            + 2.0 * size::DIVIDER
            + pitch
            + size::VBAR;
        if self.desk.set_unpinned(pinned > full.width()) {
            self.desk.refresh(app);
        }
        ui.allocate_rect(outer, egui::Sense::hover());
        if let Some(p) = panel {
            super::mixer_inserts::panel(app, ui, p);
        }
        self.digit_w = look::width_of(ui.ctx(), &s, "0", 10.0, look::W::SemiBold);
        // Every layout of the frame pins the band where it is drawn, so the wheel reads
        // the band under the pointer, not a row hidden under it (second review, 8).
        let (fit, geo) = self.layout(full);
        self.follow_selection(app, &geo, &fit);
        paint::wheel(self, app, ui, &geo, &fit);
        // The wheel may have scrolled or changed the width: lay out again.
        let prefs = self.prefs.current;
        let (fit, geo) = self.layout(full);
        self.clamp_scroll(&geo, &fit);
        let (fit, geo) = self.layout(full);
        self.desk_width = geo.middle.width();
        self.rows_seen = Some(geo.rows);
        self.right_seen = Some(geo.right); // r37:finish
        self.middle_seen = Some(geo.middle); // r39:fix2

        let mut b = std::mem::take(&mut self.brush);
        b.begin(ui.ctx());
        let mut pass = strip::Pass::new(&s, &fit, &geo, ui, prefs, app.settings.mixer_meter_wave);
        // Option C: the strip under the pointer, else the selected one, lights its
        // destinations; a keyed insert's key under the pointer lights its source.
        pass.lit = ui
            .ctx()
            .pointer_hover_pos()
            .filter(|q| geo.strips.contains(*q))
            .and_then(|q| paint::strip_at(self, &geo, q))
            .or(app.selected_strip);
        self.key_shown = self.key_lit.take();
        self.icons_drawn.clear();

        // 1. The ground, the title row and the scrollbars: nothing else overlaps them.
        paint::ground(&mut b, &s, &geo);
        if geo.merged {
            // The header in the tab row above the desk: its own clip, over the dock's bar.
            b.flush(ui.painter());
            let mut bar = ui.new_child(egui::UiBuilder::new().max_rect(geo.title));
            bar.set_clip_rect(geo.title);
            paint::title(self, app, &mut bar, &mut b, &s, &geo);
            b.flush(bar.painter());
        } else {
            paint::title(self, app, ui, &mut b, &s, &geo);
        }
        paint::scrollbars(self, ui, &mut b, &s, &geo, &fit);
        b.flush(ui.painter());

        // 2. The legend and the left dock: the master first, so the keyboard reaches it
        //    first, as the eye does.
        self.laid_out = 0;
        pass.clip = geo.strips;
        let mut x = geo.left.left();
        parts::master(self, app, ui, &mut b, &mut pass, x, geo.master_w);
        x += geo.master_w;
        for i in 0..self.desk.left.len() {
            let item = self.desk.left[i];
            let w = desk::item_width(&item, geo.pitch);
            strip::item(self, app, ui, &mut b, &mut pass, item, x, w);
            x += w;
        }
        b.flush(&ui.painter().with_clip_rect(geo.strips));

        // 3. The middle, clipped to itself: only the strips that show are laid out.
        let clip = geo.middle.intersect(geo.strips);
        pass.clip = clip;
        let mut x = geo.middle.left() - self.scroll_x;
        for i in 0..self.desk.middle.len() {
            let item = self.desk.middle[i];
            let w = desk::item_width(&item, geo.pitch);
            if x + w >= clip.left() && x <= clip.right() {
                strip::item(self, app, ui, &mut b, &mut pass, item, x, w);
            }
            x += w;
        }
        let add = egui::Rect::from_min_max(
            egui::pos2(x + 2.0, geo.y0),
            egui::pos2(x + 2.0 + desk::ADD, geo.y0 + fit.height),
        );
        if add.left() <= clip.right() {
            parts::add_button(self, app, ui, &mut b, &pass, add);
        }
        b.flush(&ui.painter().with_clip_rect(clip));

        // 4. The right dock (returns), and the dock dividers.
        pass.clip = geo.strips;
        let mut x = geo.right.left();
        for i in 0..self.desk.right.len() {
            let item = self.desk.right[i];
            let w = desk::item_width(&item, geo.pitch);
            strip::item(self, app, ui, &mut b, &mut pass, item, x, w);
            x += w;
        }
        // r39:lo-mixer — FL's next inserts up to the panel's edge, where the desk does not
        // scroll (it fits): after the right dock, else after the middle's add column.
        self.ghosts_drawn = 0;
        if !geo.scrolls {
            let content = desk::items_width(&self.desk.middle, geo.pitch) + desk::ADD + 6.0;
            let from = if geo.right.width() > 0.0 {
                geo.right.right()
            } else {
                geo.middle.left() + content + size::DIVIDER
            };
            let fits = content <= geo.middle.width() + 0.5;
            if fits {
                pass.clip = geo.strips;
                self.ghosts_drawn =
                    paint::ghosts(self, app, ui, &mut b, &pass, from, geo.inner_right);
            }
        }
        paint::dividers(&mut b, &s, &geo);
        paint::overflow(self, ui, &mut b, &s, &geo); // r39:el-mixer
        if geo.pinned {
            // The strips run under a pinned band: they stop at its edge (one more draw
            // call, only while the desk is scrolled away from its foot).
            b.flush(&ui.painter().with_clip_rect(geo.strips));
        }

        // 5. The cable band across the whole desk (round 25: `mixer_cables`, the desk
        //    builder's), then the drag in progress (`mixer_drag`, the interaction
        //    builder's): the rubber band, auto-scroll, the drop. Same clip as the right
        //    dock, so they share its flush: the band adds no draw call (ADR 0041 §12).
        pass.clip = geo.rows;
        // r39:el-mixer — no legend column: the band's cell is its row under the master.
        self.cable_key = fit.row(layout::Row::Cables).map(|(a, z)| {
            let cell = egui::Rect::from_min_max(
                egui::pos2(geo.left.left(), geo.y0 + a),
                egui::pos2(geo.left.left() + geo.master_w, geo.y0 + z),
            );
            CableKey {
                cell,
                name: cell,
                marks: None,
                compact: true,
                handle: egui::Rect::NOTHING,
            }
        });
        cables::band(self, app, ui, &mut b, &pass, &geo);
        drag::finish(self, app, ui, &mut b, &s, &geo);
        b.flush(&ui.painter().with_clip_rect(geo.rows));
        self.brush = b;

        // Overlays: the name being typed, the open menu, the route menu (round 25).
        self.rename_box(app, ui);
        menu::show(self, app, ui);
        crate::views::route_menu::show(app, ui, route_owner());

        // The next frame only while something moves, and at 60 Hz at most (§13) — through
        // the ticker: egui takes its predicted frame off a delayed request, so 16 ms asked
        // of it directly drew at the screen's rate (round 38, as round 37 found elsewhere).
        if self.meters.moving() {
            crate::views::workspace_ticker::moving(
                ui.ctx(),
                crate::views::workspace_ticker::Reason::Meters,
            );
        }
        if app.mix_recorder.status().is_some() {
            crate::views::workspace_ticker::wake_after(
                ui.ctx(),
                std::time::Duration::from_millis(33),
            );
        }
    }

    /// Where the rows go in `full`, for the set-up in use.
    fn fit(&self, full: egui::Rect) -> layout::Fit {
        let prefs = self.prefs.current;
        layout::fit(
            full.height() - self.title_h() - size::HBAR - size::TOP,
            prefs.width,
            prefs.shown(),
            prefs.ratio,
            // The level pills need a deeper band (round 25's); des-mixer's is the sockets'.
            layout::Wants {
                send_rows: 0,
                // r39:el-cables — the sockets' row and the channel the cables hang in
                // (Subway's lanes may ask for more).
                band: wires::JACK_ROW + self.band.channel.max(wires::CHANNEL),
            },
        )
    }

    /// The rows and where everything goes, the band pinned to the desk's foot while the
    /// desk is scrolled away from it.
    fn layout(&self, full: egui::Rect) -> (layout::Fit, Geo) {
        let fit = self.fit(full);
        let geo = self.geometry(full, &fit);
        pin_band(fit, geo)
    }

    /// Lay the desk out in `full`.
    fn geometry(&self, full: egui::Rect, fit: &layout::Fit) -> Geo {
        use egui::{pos2, Rect};
        let prefs = self.prefs.current;
        let pitch = prefs.width.pitch();
        let title = Rect::from_min_max(full.min, pos2(full.right(), full.top() + self.title_h()));
        let rows = Rect::from_min_max(
            pos2(full.left(), title.bottom()),
            pos2(
                full.right(),
                (full.bottom() - size::HBAR).max(title.bottom()),
            ),
        );
        let vbar = fit.scrolls.then(|| {
            Rect::from_min_max(
                pos2(rows.right() - 5.0, rows.top() + 4.0),
                pos2(rows.right() - 2.0, rows.bottom() - 4.0),
            )
        });
        // r39:el-mixer — the right dock 8 pt in from the edge (its last pitch carries 2).
        let right_edge = rows.right() - 6.0 - if vbar.is_some() { size::VBAR } else { 0.0 };
        let legend = Rect::from_min_max(rows.min, pos2(rows.left() + size::LEGEND, rows.bottom()));
        let master_w = size::master(prefs.width);
        let left_w = master_w + desk::items_width(&self.desk.left, pitch);
        let left = Rect::from_min_max(
            pos2(legend.right(), rows.top()),
            pos2(legend.right() + left_w, rows.bottom()),
        );
        let right_w = desk::items_width(&self.desk.right, pitch);
        let mid_l = left.right() + size::DIVIDER;
        // r37:finish — where the middle's strips all fit, the right dock sits just after
        // them, not across an empty gap at the window's edge (the judge: dead space before
        // the buses); pinned at the edge only while the middle scrolls.
        let strips_w = desk::items_width(&self.desk.middle, pitch);
        let content = strips_w + desk::ADD + 6.0;
        let end = if right_w > 0.0 {
            right_edge - right_w - size::DIVIDER
        } else {
            right_edge
        };
        // r37:finish — where the middle scrolls, it shows whole strips: its width a whole
        // number of strips, the dock just after them (a strip cut by the dock read as a
        // stray half strip). r39:lo-mixer — and a slot of its own for the overflow's pill
        // ("‹ 6", "6 ›"), so the pill never covers a strip's meter. r39:fix2 — a slot only
        // on a side with strips hidden beyond it: at the start no slot on the left (it was
        // an empty 36 pt that took a strip's room), and scrolled to the end none on the
        // right; the right slot takes what is left of the room after the whole strips, so
        // the right dock stays at the window's edge and no gap a strip could fill is left.
        let avail = (end - mid_l).max(0.0);
        let scrolls = mid_l + content > end + 0.5;
        let whole = |w: f32| (w.max(0.0) / pitch).floor() * pitch;
        let roomy = scrolls && avail - 2.0 * size::OVERFLOW >= pitch;
        // r39:fix2 — each slot as wide as its pill: the count's digits (the pill is its
        // number, the chevron and 20 pt), so a one-digit count leaves a strip the room a
        // two-digit one needs.
        let hidden_left = self.strips_between(pitch, 0.0, self.scroll_x + 1.0);
        let slot_l = if roomy && self.scroll_x > 0.5 {
            self.slot_for(hidden_left)
        } else {
            0.0
        };
        let hidden_right = strips_w - self.scroll_x > whole(avail - slot_l) + 0.5;
        let (mid_l, mid_w, slot_r) = if roomy && hidden_right {
            // The count beside the room a one-digit slot leaves; again if it has two.
            let mut slot = self.slot_for(1);
            let mut mid_w = whole(avail - slot_l - slot);
            for _ in 0..2 {
                let n = self.strips_between(pitch, self.scroll_x + mid_w - 1.0, f32::MAX);
                let need = self.slot_for(n);
                if need <= slot {
                    break;
                }
                slot = need;
                mid_w = whole(avail - slot_l - slot);
            }
            (mid_l + slot_l, mid_w, avail - slot_l - mid_w)
        } else if roomy {
            // Scrolled to the end: the middle runs to the dock (the strip cut at its left
            // edge sits under the fade, beside its pill).
            (mid_l + slot_l, avail - slot_l, 0.0)
        } else if scrolls && right_w > 0.0 {
            // Never past the window's edge (review: under one strip of room, a strip's
            // width pushed the dock off screen): the edge's place is the most.
            (mid_l, whole(avail), 0.0)
        } else {
            (mid_l, avail.min(content), 0.0)
        };
        let dock_l = if right_w > 0.0 {
            (mid_l + mid_w + slot_r + size::DIVIDER).min(right_edge - right_w)
        } else {
            right_edge
        };
        let right = Rect::from_min_max(
            pos2(dock_l, rows.top()),
            pos2(dock_l + right_w, rows.bottom()),
        );
        let mid_r = if right_w > 0.0 || roomy {
            mid_l + mid_w
        } else {
            right_edge
        };
        let middle = Rect::from_min_max(
            pos2(mid_l, rows.top()),
            pos2(mid_r.max(mid_l), rows.bottom()),
        );
        let content_w = desk::items_width(&self.desk.middle, pitch) + desk::ADD + 6.0;
        let hbar = Rect::from_min_max(
            pos2(middle.left(), rows.bottom()),
            pos2(middle.right(), rows.bottom() + 3.0),
        );
        Geo {
            full,
            title,
            rows,
            legend,
            left,
            right,
            middle,
            hbar,
            vbar,
            y0: rows.top() + size::TOP - self.scroll_y,
            pitch,
            master_w,
            content_w,
            strips: rows,
            pinned: false,
            merged: self.bar_room.is_some(),
            scrolls,
            slot_l,
            slot_r,
            inner_right: right_edge,
        }
        .with_title(self.bar_room)
    }

    /// The header's own row: none when it sits in the workspace's tab row.
    fn title_h(&self) -> f32 {
        if self.bar_room.is_some() {
            0.0
        } else {
            size::TITLE
        }
    }

    /// How many of the middle's strips lie wholly at or after `from` and end before `to`
    /// (scrolled content coordinates): what an overflow pill counts.
    fn strips_between(&self, pitch: f32, from: f32, to: f32) -> usize {
        let mut x = 0.0;
        let mut n = 0;
        for item in &self.desk.middle {
            let w = desk::item_width(item, pitch);
            if matches!(item, desk::Item::Strip(..)) && x >= from && x + w <= to {
                n += 1;
            }
            x += w;
        }
        n
    }

    /// The overflow slot a count `n` needs: its pill (the digits, the chevron and 20 pt, as
    /// `paint::overflow` draws it) and [`size::SLOT_PAD`] each side: 30 pt for one digit,
    /// 36 ([`size::OVERFLOW`]) for two in the shipped face (r39:fix2).
    pub(crate) fn slot_for(&self, n: usize) -> f32 {
        let digits = if n >= 100 {
            3.0
        } else if n >= 10 {
            2.0
        } else {
            1.0
        };
        let digit = if self.digit_w > 0.0 {
            self.digit_w
        } else {
            6.0
        };
        ((digits * digit).round() + 20.0 + 2.0 * size::SLOT_PAD).max(28.0)
    }

    /// Bring a strip selected elsewhere (the arrangement, the inspector, a route) into
    /// view.
    fn follow_selection(&mut self, app: &CantusApp, geo: &Geo, fit: &layout::Fit) {
        if app.selected_strip != self.followed {
            self.followed = app.selected_strip;
            if let Some(sel) = app.selected_strip {
                let mut x = 0.0;
                let mut at = None;
                for item in &self.desk.middle {
                    let w = desk::item_width(item, geo.pitch);
                    if matches!(item, desk::Item::Strip(id, _) if *id == sel) {
                        at = Some((x, w));
                        break;
                    }
                    x += w;
                }
                // r39:fix2 — the middle's width follows the scroll (the left overflow slot
                // shows once strips are hidden on the left): settle it in a few passes, so
                // the strip is in view with the slot the scroll brings.
                if let Some((x, w)) = at {
                    let mut width = geo.middle.width();
                    for _ in 0..3 {
                        if x < self.scroll_x {
                            self.scroll_x = x;
                        } else if x + w > self.scroll_x + width {
                            self.scroll_x = x + w - width;
                        } else {
                            break;
                        }
                        width = self.geometry(geo.full, fit).middle.width();
                    }
                }
            }
        }
        // Clamped by the middle as the scroll lays it out (narrower once scrolled).
        let now = self.geometry(geo.full, fit);
        self.clamp_scroll(&now, fit);
    }

    fn clamp_scroll(&mut self, geo: &Geo, fit: &layout::Fit) {
        let (mx, my) = self.scroll_max(geo, fit);
        self.scroll_x = self.scroll_x.clamp(0.0, mx);
        self.scroll_y = self.scroll_y.clamp(0.0, my);
    }

    /// Scroll limits: (horizontal, vertical).
    pub(crate) fn scroll_max(&self, geo: &Geo, fit: &layout::Fit) -> (f32, f32) {
        (
            (geo.content_w - geo.middle.width()).max(0.0),
            (fit.height + size::TOP - geo.rows.height()).max(0.0),
        )
    }
}

impl MixerView {
    /// Start renaming a group's separator in place.
    pub(crate) fn begin_group_rename(&mut self, app: &CantusApp, id: cantus_core::GroupId) {
        if let Some(g) = app.project().strip_group(id) {
            self.rename = Some((Renaming::Group(id), g.name.clone()));
        }
    }

    /// The text box a rename types into: a well laid over the strip's number and name,
    /// wide enough for a name even on a 24-pt strip. Enter or a click elsewhere keeps the
    /// name, Escape drops it.
    fn rename_box(&mut self, app: &mut CantusApp, ui: &mut egui::Ui) {
        let Some((what, _)) = &self.rename else {
            return;
        };
        let what = *what;
        let Some(at) = self.rename_at.take() else {
            // Not on screen this frame. Asked for this frame (the name is painted before
            // the double-click is seen), the box opens on the next; once shown, a strip
            // scrolled or folded away drops the rename.
            if self.rename_focused {
                self.rename = None;
                self.rename_focused = false;
            }
            return;
        };
        // Opening on a strip that shows its track's name: that name, selected (final
        // review: the field opened empty).
        if !self.rename_focused {
            if let Some((Renaming::Strip(sid), text)) = self.rename.as_mut() {
                if text.is_empty() {
                    if let Some(i) = app.project().strip_index(*sid) {
                        *text = self.desk.name(app, i).to_string();
                    }
                }
            }
        }
        // A popover under the cell it renames (a strip's number, a group's separator):
        // framed, over the desk, the next strips' numbers left in sight.
        let pad = 4.0;
        let w = at.width().max(132.0);
        let rect = egui::Rect::from_min_size(
            egui::pos2(at.left(), at.bottom() + 2.0 + pad),
            egui::vec2(w, 22.0),
        );
        let id = egui::Id::new(("mixer-rename", self.rename_serial));
        let mut done: Option<bool> = None;
        egui::Area::new(id)
            .order(egui::Order::Foreground)
            .fixed_pos(rect.min - egui::vec2(pad, pad))
            .constrain(true)
            .show(ui.ctx(), |ui| {
                let (outer, _) = ui.allocate_exact_size(
                    rect.size() + egui::vec2(2.0 * pad, 2.0 * pad),
                    egui::Sense::hover(),
                );
                let s = style();
                crate::kit::with_brush(ui.painter(), |b| {
                    crate::kit::menu::paint(b, outer, &s, &[], None)
                });
                let r = outer.shrink(pad);
                let Some((_, text)) = self.rename.as_mut() else {
                    return;
                };
                // The kit's field (round 26): the name selected as it opens; Enter or a
                // click elsewhere keeps it, Esc drops it.
                let out = crate::kit::field::Field::new(id.with("field"))
                    .role(crate::kit::text::Role::Label)
                    .opens_when_shown()
                    .show(ui, r, text);
                self.rename_focused = true;
                match out.event {
                    // Untouched: nothing to set (a strip keeps following its track).
                    Some(crate::kit::field::Event::Commit) => done = Some(out.edited),
                    Some(crate::kit::field::Event::Cancel) => done = Some(false),
                    None => {}
                }
            });
        let Some(keep) = done else {
            return;
        };
        let Some((_, name)) = self.rename.take() else {
            return;
        };
        self.rename_focused = false;
        self.rename_serial = self.rename_serial.wrapping_add(1);
        if !keep {
            return;
        }
        let name = name.trim().to_string();
        match what {
            Renaming::Strip(id) => {
                let old = app.project().strip(id).map(|s| s.name.as_str());
                if old.is_some_and(|o| o != name) {
                    app.exec(cantus_core::Command::SetStripName { id, name });
                }
            }
            Renaming::Group(id) => {
                let old = app.project().strip_group(id).map(|g| g.name.as_str());
                if !name.is_empty() && old.is_some_and(|o| o != name) {
                    app.exec(cantus_core::Command::SetStripGroupName { id, name });
                }
            }
            Renaming::Snapshot(id) => {
                // r36:mix
                let old = app.project().mix.snapshot(id).map(|s| s.name.as_str());
                if !name.is_empty() && old.is_some_and(|o| o != name) {
                    app.exec(cantus_core::Command::Mix(
                        cantus_core::mix_cmd::MixCommand::RenameSnapshot { id, name },
                    ));
                }
            }
        }
    }
}
