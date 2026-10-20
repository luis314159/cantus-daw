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
#[path = "mixer_lanes.rs"]
mod lanes;
#[path = "mixer_layout.rs"]
pub mod layout;
#[path = "mixer_menu.rs"]
mod menu;
#[path = "mixer_meters.rs"]
mod meters;
#[path = "mixer_paint.rs"]
mod paint;
#[path = "mixer_parts.rs"]
mod parts;
#[path = "mixer_routehere.rs"]
mod routehere;
#[path = "mixer_strip.rs"]
mod strip;

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

/// Where the end mark of a lit cable (`DrawnCable::edge`) was drawn last frame: its
/// destination's (`source == false`) or, on the selected cable, its source's.
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
        }
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
    pub fx: String,
    pub fx_off: String,
    pub record: String,
    pub record_mix: String,
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
    pub master: String,
    /// The master's number and name: a click shows its chain, a right-click its menu.
    pub master_head: String,
    pub pan_law: String,
    /// The pan law's chip on the master, "{law} dB law" (and its short form "{law} dB").
    pub pan_law_chip: String,
    pub pan_law_short: String,
    pub ratio: String,
    pub menu: String,
    pub modes: String,
    pub wave: String,
    pub chain: String,
    pub chain_empty: String,
    pub output: String,
    pub send_row: String,
    pub send_add: String,
    pub send_more: String,
    pub port: String,
    pub cable: String,
    pub cable_view: String,
    pub cable_opts: String,
    pub pill: String,
    pub chip: String,
    pub band: String,
    /// The band's handle while the desk holds the band at its least.
    pub band_short: String,
    pub send_off: String,
    pub pre: String,
    pub kind: [String; 3],
    pub tap: [String; 2],
    pub group: String,
    pub solo_off: String,
    pub unmute_all: String,
    pub gr: String,
    pub name: String,
    pub takes: String,
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
            fx: t("mixer.fx_tip"),
            fx_off: t("mixer.fx_off"),
            record: t("mixer.hint.record"),
            record_mix: t("take.record_mix"),
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
            master: t("mixer.master"),
            master_head: t("master.head_tip"),
            pan_law: t("panlaw.tip"),
            pan_law_chip: t("mixer.pan_law.chip"),
            pan_law_short: t("mixer.pan_law.short"),
            ratio: t("mixer.hint.ratio"),
            menu: t("mixer.hint.menu"),
            modes: t("mixer.hint.modes"),
            wave: t("mixer.wave_tip"),
            chain: t("mixer.hint.chain"),
            chain_empty: t("mixer.hint.chain_empty"),
            output: t("mixer.hint.output"),
            send_row: t("mixer.hint.send_row"),
            send_add: t("mixer.hint.send_add"),
            send_more: t("mixer.hint.send_more"),
            port: t("cables.hint.port"),
            cable: t("cables.hint.cable"),
            cable_view: t("cables.hint.view"),
            cable_opts: t("cables.hint.options"),
            pill: t("cables.hint.pill"),
            chip: t("cables.hint.chip"),
            band: t("cables.hint.band"),
            band_short: t("cables.hint.band_short"),
            send_off: t("cables.send_off"),
            pre: t("routing.tap.pre"),
            kind: [
                t("cables.kind.output"),
                t("cables.kind.send"),
                t("cables.kind.key"),
            ],
            tap: [t("cables.tap.post"), t("cables.tap.pre")],
            group: t("mixer.hint.group"),
            solo_off: t("mixer.solo_off"),
            unmute_all: t("mixer.unmute_all"),
            gr: t("mixer.hint.gr"),
            name: t("mixer.hint.name"),
            takes: t("mixer.takes"),
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
    /// The last frame's counters (the frame-budget test reads them).
    laid_out: usize,
    lanes_filling: usize,
    desk_width: f32,
    /// The egui pass the desk was last drawn on: a gap means it left the screen in
    /// between (its pane closed, another tab over it) — the clean "the desk was hidden"
    /// signal an armed route lets go on (round 26, ADR 0044 §10).
    shown_pass: u64,
    /// Where the cable legend went last frame (`CableKey`).
    pub(crate) cable_key: Option<CableKey>,
    /// The desk's rows area last frame (where anything of the desk is visible).
    rows_seen: Option<egui::Rect>,
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
                Ok(map) => self.prefs.by_layout = map,
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

    /// Gain-reduction lanes showing reduction in the last frame.
    pub fn gr_lanes_filling(&self) -> usize {
        self.lanes_filling
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

    /// The desk's rows area in the last frame: what of the desk is visible.
    pub fn rows_seen(&self) -> egui::Rect {
        self.rows_seen.unwrap_or(egui::Rect::NOTHING)
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
    /// The legend column.
    pub const LEGEND: f32 = 84.0;
    /// The band under the strips holding the horizontal scrollbar.
    pub const HBAR: f32 = 12.0;
    /// The column at the right holding the vertical scrollbar, when the desk scrolls.
    pub const VBAR: f32 = 10.0;
    /// FL's dock divider.
    pub const DIVIDER: f32 = 6.0;
    /// Above the first row.
    pub const TOP: f32 = 4.0;
    /// The master strip, per width mode.
    pub fn master(mode: super::WidthMode) -> f32 {
        match mode {
            super::WidthMode::Compact => 50.0,
            super::WidthMode::Wide => 56.0,
            super::WidthMode::Xl => 76.0,
        }
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
        let full = ui.available_rect_before_wrap();
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
        ui.allocate_rect(full, egui::Sense::hover());
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

        let mut b = std::mem::take(&mut self.brush);
        b.begin(ui.ctx());
        let mut pass = strip::Pass::new(&s, &fit, &geo, ui, prefs, app.settings.mixer_meter_wave);

        // 1. The ground, the title row and the scrollbars: nothing else overlaps them.
        paint::ground(&mut b, &s, &geo);
        paint::title(self, app, ui, &mut b, &s, &geo);
        paint::scrollbars(self, ui, &mut b, &s, &geo, &fit);
        b.flush(ui.painter());

        // 2. The legend and the left dock: the master first, so the keyboard reaches it
        //    first, as the eye does.
        self.laid_out = 0;
        self.lanes_filling = 0;
        pass.clip = geo.strips;
        paint::legend(self, ui, &mut b, &s, &geo, &fit);
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
        paint::dividers(&mut b, &s, &geo);
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
        paint::band_legend(self, ui, &mut b, &s, &geo, &fit);
        cables::band(self, app, ui, &mut b, &pass, &geo);
        drag::finish(self, app, ui, &mut b, &s, &geo);
        b.flush(&ui.painter().with_clip_rect(geo.rows));
        self.brush = b;

        // Overlays: the name being typed, the open menu, the route menu (round 25).
        self.rename_box(app, ui);
        menu::show(self, app, ui);
        crate::views::route_menu::show(app, ui, route_owner());

        // The next frame only while something moves, and at 60 Hz at most (§13).
        if self.meters.moving() {
            ui.ctx()
                .request_repaint_after(std::time::Duration::from_millis(16));
        }
        if app.mix_recorder.status().is_some() {
            ui.ctx()
                .request_repaint_after(std::time::Duration::from_millis(33));
        }
    }

    /// Where the rows go in `full`, for the set-up in use.
    fn fit(&self, full: egui::Rect) -> layout::Fit {
        let prefs = self.prefs.current;
        layout::fit(
            full.height() - size::TITLE - size::HBAR - size::TOP,
            prefs.width,
            prefs.shown(),
            prefs.ratio,
            layout::Wants {
                send_rows: layout::send_rows_for(self.desk.most_sends),
                band: prefs.band,
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
        let title = Rect::from_min_max(full.min, pos2(full.right(), full.top() + size::TITLE));
        let rows = Rect::from_min_max(
            pos2(full.left(), title.bottom()),
            pos2(
                full.right(),
                (full.bottom() - size::HBAR).max(title.bottom()),
            ),
        );
        let vbar = fit.scrolls.then(|| {
            Rect::from_min_max(
                pos2(rows.right() - size::VBAR + 2.0, rows.top() + 2.0),
                pos2(rows.right() - 2.0, rows.bottom() - 2.0),
            )
        });
        let right_edge = rows.right() - if vbar.is_some() { size::VBAR } else { 0.0 };
        let legend = Rect::from_min_max(rows.min, pos2(rows.left() + size::LEGEND, rows.bottom()));
        let master_w = size::master(prefs.width);
        let left_w = master_w + desk::items_width(&self.desk.left, pitch);
        let left = Rect::from_min_max(
            pos2(legend.right(), rows.top()),
            pos2(legend.right() + left_w, rows.bottom()),
        );
        let right_w = desk::items_width(&self.desk.right, pitch);
        let right = Rect::from_min_max(
            pos2(right_edge - right_w, rows.top()),
            pos2(right_edge, rows.bottom()),
        );
        let mid_l = left.right() + size::DIVIDER;
        let mid_r = if right_w > 0.0 {
            right.left() - size::DIVIDER
        } else {
            right_edge
        };
        let middle = Rect::from_min_max(
            pos2(mid_l, rows.top()),
            pos2(mid_r.max(mid_l), rows.bottom()),
        );
        let content_w = desk::items_width(&self.desk.middle, pitch) + desk::ADD + 6.0;
        let hbar = Rect::from_min_max(
            pos2(middle.left() + 2.0, rows.bottom() + 3.0),
            pos2(middle.right() - 2.0, rows.bottom() + 9.0),
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
            inner_right: right_edge,
        }
    }

    /// Bring a strip selected elsewhere (the arrangement, the inspector, a route) into
    /// view.
    fn follow_selection(&mut self, app: &CantusApp, geo: &Geo, fit: &layout::Fit) {
        if app.selected_strip != self.followed {
            self.followed = app.selected_strip;
            if let Some(sel) = app.selected_strip {
                let mut x = 0.0;
                for item in &self.desk.middle {
                    let w = desk::item_width(item, geo.pitch);
                    if matches!(item, desk::Item::Strip(id, _) if *id == sel) {
                        if x < self.scroll_x {
                            self.scroll_x = x;
                        } else if x + w > self.scroll_x + geo.middle.width() {
                            self.scroll_x = x + w - geo.middle.width();
                        }
                        break;
                    }
                    x += w;
                }
            }
        }
        self.clamp_scroll(geo, fit);
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
        }
    }
}
