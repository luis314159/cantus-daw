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
//! bulb and record, the sends, and (Extra large) the insert slots, empty ones as darker
//! wells. Every strip has the same rows at the same heights ([`layout::fit`]), so a row
//! reads straight across the desk, and a **legend column** at the left names each row
//! once — the strips carry values and icons, never the same word forty times.
//!
//! **Sends are FL's**: the sends row of every strip answers "can the *selected* strip go
//! here?" — a white ▲ (click: send it here; Ctrl+click: route it here only), a hollow ▲
//! when it would close a loop, a lit ▲ where it already goes, and a knob where it already
//! sends (drag: the level). The selected strip itself shows where it goes, and its menu
//! lists every destination — built when it opens, never every frame.
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

#[path = "mixer_desk.rs"]
mod desk;
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
#[path = "mixer_strip.rs"]
mod strip;

use crate::app::CantusApp;
use crate::i18n::{self, t};
use crate::kit::{Brush, Style};
use cantus_core::{MixerStrip, StripId};
use layout::{Colourful, RowSet, WidthMode};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

pub use parts::phase_button;
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
}

impl Default for DeskPrefs {
    fn default() -> Self {
        DeskPrefs {
            width: WidthMode::default(),
            rows: RowSet::default(),
            ratio: layout::DEFAULT_RATIO,
            colourful: Colourful::default(),
        }
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
    pub ratio: String,
    pub menu: String,
    pub modes: String,
    pub wave: String,
    pub slot_empty: String,
    pub slot: String,
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
            ratio: t("mixer.hint.ratio"),
            menu: t("mixer.hint.menu"),
            modes: t("mixer.hint.modes"),
            wave: t("mixer.wave_tip"),
            slot_empty: t("mixer.hint.slot_empty"),
            slot: t("mixer.hint.slot"),
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
    /// An insert slot being dragged to a new place: its strip (or the master), the
    /// effect, and the slot it started in.
    slot_drag: Option<(StripId, cantus_core::EffectId, usize)>,
    /// The desk's one brush: a mesh per clip region, the text on top.
    brush: Brush,
    /// Reused for numbers and hint names, so a frame formats without allocating.
    pub(crate) buf: String,
    pub(crate) hint_buf: String,
    /// The last frame's counters (the frame-budget test reads them).
    laid_out: usize,
    lanes_filling: usize,
    desk_width: f32,
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

    /// The middle dock's horizontal scroll, and the desk's vertical one, in points.
    pub fn scroll(&self) -> (f32, f32) {
        (self.scroll_x, self.scroll_y)
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

/// Show the desk. Takes the view out of the app for the frame, so painting can borrow
/// the project while the view's own buffers are written.
pub fn show(app: &mut CantusApp, ui: &mut egui::Ui) {
    let mut view = std::mem::take(&mut app.mixer);
    view.frame(app, ui);
    // Anything the frame queued for the app (menus, renames) was applied inside; put the
    // view back.
    app.mixer = view;
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
}

impl MixerView {
    /// One frame of the desk.
    fn frame(&mut self, app: &mut CantusApp, ui: &mut egui::Ui) {
        self.words.refresh();
        self.prefs.follow(&app.layouts.current.name);
        self.desk.refresh(app);
        self.meters.begin_frame();
        let s = style();
        let prefs = self.prefs.current;
        let full = ui.available_rect_before_wrap();
        ui.allocate_rect(full, egui::Sense::hover());
        let fit = layout::fit(
            full.height() - size::TITLE - size::HBAR - size::TOP,
            prefs.width,
            prefs.rows,
            prefs.ratio,
            self.desk.most_inserts + 1,
        );
        let geo = self.geometry(full, &fit);
        self.follow_selection(app, &geo, &fit);
        paint::wheel(self, app, ui, &geo, &fit);
        // The wheel may have scrolled or changed the width: lay out again.
        let prefs = self.prefs.current;
        let fit = layout::fit(
            full.height() - size::TITLE - size::HBAR - size::TOP,
            prefs.width,
            prefs.rows,
            prefs.ratio,
            self.desk.most_inserts + 1,
        );
        let geo = self.geometry(full, &fit);
        self.clamp_scroll(&geo, &fit);
        let geo = self.geometry(full, &fit);
        self.desk_width = geo.middle.width();

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
        pass.clip = geo.rows;
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
        b.flush(&ui.painter().with_clip_rect(geo.rows));

        // 3. The middle, clipped to itself: only the strips that show are laid out.
        let clip = geo.middle.intersect(geo.rows);
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
        pass.clip = geo.rows;
        let mut x = geo.right.left();
        for i in 0..self.desk.right.len() {
            let item = self.desk.right[i];
            let w = desk::item_width(&item, geo.pitch);
            strip::item(self, app, ui, &mut b, &mut pass, item, x, w);
            x += w;
        }
        paint::dividers(&mut b, &s, &geo);
        b.flush(&ui.painter().with_clip_rect(geo.rows));
        self.brush = b;

        // Overlays: the name being typed, the open menu.
        self.rename_box(app, ui);
        menu::show(self, app, ui);

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
        let s = style();
        let w = at.width().max(132.0);
        let rect = egui::Rect::from_min_size(at.min, egui::vec2(w, 22.0));
        let id = egui::Id::new(("mixer-rename", self.rename_serial));
        let mut done: Option<bool> = None;
        egui::Area::new(id)
            .order(egui::Order::Foreground)
            .fixed_pos(rect.min)
            .constrain(true)
            .show(ui.ctx(), |ui| {
                let (r, _) = ui.allocate_exact_size(rect.size(), egui::Sense::hover());
                crate::kit::with_brush(ui.painter(), |b| {
                    crate::kit::well::paint(b, r, &s, crate::kit::State::LIT, None);
                });
                let Some((_, text)) = self.rename.as_mut() else {
                    return;
                };
                let edit = ui.put(
                    r.shrink2(egui::vec2(6.0, 2.0)),
                    egui::TextEdit::singleline(text)
                        .frame(egui::Frame::NONE)
                        .font(crate::kit::text::Role::Label.font(1.0))
                        .text_color(s.t.text)
                        .desired_width(r.width() - 12.0),
                );
                if !self.rename_focused {
                    edit.request_focus();
                    self.rename_focused = true;
                }
                if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                    done = Some(false);
                } else if edit.lost_focus() || ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                    done = Some(true);
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
