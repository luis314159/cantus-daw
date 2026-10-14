//! Device windows that are easy to close, minimise and swap (round 20, ADR 0029 §device-ux).
//!
//! The owner, 2026-10-05: *"it is hard to know you have to unfold an option to close or
//! minimise an instrument — I want something more graphical"*, and swapping an instrument
//! or an effect for another must be easy and intuitive. What this module adds, on the
//! kit's window buttons ([`crate::kit::window_ctl`]):
//!
//! - **Every device window has its buttons** in its title bar — swap, minimise, maximise /
//!   restore, close — instruments and Pulsus II (dock tabs) and effects and hosted
//!   plug-ins (floating areas) alike. [`ctl`] says which, [`on_event`] does what they ask.
//! - **The device tray** in the status bar: a minimised device leaves the screen and a chip
//!   (its glyph in its hue, its name, a ×) brings it back with one click ([`tray`]).
//! - **Esc** closes the device clicked last; **F8** opens or closes the selected track's
//!   instrument (and brings a minimised one back) — [`CantusApp::toggle_instrument_window`].
//! - **Where devices float is remembered** per device for the session: closed and opened
//!   again, it comes back where it was; maximised and restored, likewise
//!   ([`Workspace::device_places`]).
//! - **Swap** opens the device picker ([`crate::picker`]) for the device in place; what is
//!   kept when the choice is made is `CantusApp::apply_pick`'s.

use super::workspace::{Pane, Workspace};
use super::workspace_device::{self as device, DeviceKey, Pending};
use crate::app::CantusApp;
use crate::i18n::tf;
use crate::kit::color::shade;
use crate::kit::glyph::{self, Glyph};
use crate::kit::shell::Event;
use crate::kit::text::{self, Elide, Policy, Role, Text};
use crate::kit::window_ctl::WindowCtl;
use crate::kit::{cap, with_brush, Fill, State, Style};
use crate::picker::PickPurpose;
use cantus_core::{InstrumentSpec, TrackId};
use egui::{Align2, Context, Pos2, Rect, Sense, Ui, Vec2};

/// The buttons a device window shows in Cantus: all four, the maximise key showing
/// *restore* when it is maximised. `swap_tip` is the i18n key of the swap cap's tooltip.
pub fn ctl(ctx: &Context, key: DeviceKey, swap_tip: &'static str) -> WindowCtl {
    WindowCtl {
        swap: Some(swap_tip),
        minimise: true,
        maximised: Some(device::is_maximised(ctx, key)),
        close: true,
    }
}

/// What a window button asked of the editor that owns the window.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Asked {
    /// Nothing for the editor: the registry or the picker took care of it.
    Nothing,
    /// Close the editor.
    Close,
}

/// Act on one of the shell's events for the device `key`: minimise and maximise go to the
/// registry, swap opens the picker for `swap`, close is the editor's to do (it knows how
/// to close itself). Every other event is not a window button's: `Nothing`.
pub fn on_event(
    app: &mut CantusApp,
    ctx: &Context,
    key: DeviceKey,
    ev: Event,
    swap: PickPurpose,
) -> Asked {
    match ev {
        Event::Close => Asked::Close,
        Event::Minimise => {
            device::minimise(ctx, key);
            Asked::Nothing
        }
        Event::Maximise => {
            device::toggle_maximise(ctx, key);
            Asked::Nothing
        }
        Event::Swap => {
            app.open_picker(swap);
            Asked::Nothing
        }
        _ => Asked::Nothing,
    }
}

// ---- the keys: Esc and F8 -----------------------------------------------------------------

impl CantusApp {
    /// Esc: close the device window clicked last, if one is on screen. Returns whether one
    /// was closed (so Esc does nothing else).
    pub fn close_focused_device(&mut self) -> bool {
        let Some(ctx) = self.theme_ctx.clone() else {
            return false;
        };
        match device::focused(&ctx) {
            Some(key) => {
                device::mark_closed(&ctx, key);
                ctx.request_repaint();
                true
            }
            None => false,
        }
    }

    /// The device key of `track`'s instrument window, if its instrument has one: a native
    /// instrument's editor or Pulsus's.
    pub fn instrument_window_key(&self, track: TrackId) -> Option<DeviceKey> {
        match &self.project().track(track)?.instrument {
            InstrumentSpec::Native(_) => Some(super::inst::editor_id(track).value()),
            InstrumentSpec::Pulsus(_) => Some(super::pulsus_editor::editor_id(track).value()),
            _ => None,
        }
    }

    /// Whether `track`'s instrument window is open (shown or minimised).
    pub fn instrument_window_open(&self, track: TrackId) -> bool {
        self.inst.open == Some(track)
            || self.pulsus.open == Some(track)
            || self.plugins.instrument_window_open(track)
    }

    /// Open `track`'s instrument window, or bring it back from the tray and to the front
    /// (round 27, ADR 0051). A track whose instrument has no window of its own (Firmus,
    /// edited in the inspector) shows the inspector instead.
    pub fn open_instrument_window(&mut self, track: TrackId) {
        let Some(spec) = self.project().track(track).map(|t| t.instrument.clone()) else {
            return;
        };
        if let (Some(ctx), Some(key)) = (self.theme_ctx.clone(), self.instrument_window_key(track))
        {
            if self.instrument_window_open(track) {
                // Round 27 (ADR 0051): back from the tray, in front, focused.
                device::present(&ctx, key);
                return;
            }
        }
        match spec {
            InstrumentSpec::Native(_) => self.open_inst_editor(track),
            InstrumentSpec::Pulsus(_) => super::pulsus_editor::open(self, track),
            // A hosted instrument's window is the plug-in's own (ADR 0028).
            InstrumentSpec::External(_) => self.open_hosted_window(track),
            _ => {
                self.select_track(track);
                // Possibly from inside a tab (the channel rack): deferred to the dock.
                self.show_pane(Pane::Inspector);
            }
        }
    }

    /// Close `track`'s instrument window.
    pub fn close_instrument_window(&mut self, track: TrackId) {
        if self.inst.open == Some(track) {
            self.close_inst_editor();
        }
        if self.pulsus.open == Some(track) {
            super::pulsus_editor::close(self);
        }
        self.plugins.close_instrument_window(track);
    }

    /// Whether `track`'s instrument window is on screen: open and not minimised.
    pub fn instrument_window_shown(&self, track: TrackId) -> bool {
        let minimised = match (&self.theme_ctx, self.instrument_window_key(track)) {
            (Some(ctx), Some(key)) => device::is_minimised(ctx, key),
            // A hosted plug-in's own window, minimised to the taskbar (round 27).
            _ => self.plugins.instrument_window_minimised(track),
        };
        self.instrument_window_open(track) && !minimised
    }

    /// Whether `track`'s instrument window is open **and in front** (round 27, ADR 0051
    /// §3): not minimised, its tab the one showing, no other window over it — for a hosted
    /// plug-in, its own window not minimised and not covered.
    pub fn instrument_window_in_front(&self, track: TrackId) -> bool {
        if !self.instrument_window_open(track) {
            return false;
        }
        match (&self.theme_ctx, self.instrument_window_key(track)) {
            (Some(ctx), Some(key)) => device::in_front(ctx, key),
            (None, Some(_)) => self.instrument_window_shown(track),
            _ => self.plugins.instrument_window_frontmost(track),
        }
    }

    /// `track`'s instrument window as the user sees it (F8, and a click on the rack's
    /// channel button): closed → opened; open but not in front (minimised, behind another
    /// tab, under another window) → brought back and to the front; in front → closed.
    pub fn toggle_instrument_window_of(&mut self, track: TrackId) {
        if self.instrument_window_in_front(track) {
            self.close_instrument_window(track);
        } else {
            self.open_instrument_window(track);
        }
    }

    /// F8: the selected track's instrument window, open or closed.
    pub fn toggle_instrument_window(&mut self) {
        if let Some(track) = self.selected_track {
            self.toggle_instrument_window_of(track);
        }
    }
}

// ---- the dock: where tabs float, maximise and restore -------------------------------------

impl Workspace {
    /// Round 20, called before the dock is laid out: carry out the maximise and restore
    /// the device tabs' buttons asked for, and remember where each device tab floats (a
    /// docked one is forgotten, and docks again when it reopens).
    pub(crate) fn device_places(&mut self, ctx: &Context, area: Rect) {
        device::begin_frame(ctx, area);
        let mut pending = Vec::new();
        device::take_pending(ctx, &mut pending);
        for (key, what, restore) in pending {
            let pane = Pane::Device(key);
            let Some(path) = self.find(pane) else {
                continue;
            };
            match what {
                Pending::Maximise => {
                    let was = (!path.surface.is_main())
                        .then(|| self.float_rect(ctx, path))
                        .flatten();
                    self.refloat(pane, area);
                    device::set_maximised(ctx, key, true, was);
                }
                Pending::Restore => {
                    match restore {
                        Some(r) => {
                            self.refloat(pane, r);
                            device::settle(ctx, key, r.min);
                        }
                        // It was docked: back into the dock.
                        None => {
                            self.hide(pane);
                            self.reveal(pane);
                        }
                    }
                    device::set_maximised(ctx, key, false, None);
                }
            }
            ctx.request_repaint();
        }
        let mut settles = Vec::new();
        device::take_settles(ctx, &mut settles);
        for (key, at) in settles {
            let path = self
                .find(Pane::Device(key))
                .filter(|p| !p.surface.is_main());
            if let Some(w) = path.and_then(|p| self.dock.get_window_state_mut(p.surface)) {
                w.set_position(at);
            }
        }
        // Where each device tab is: floating windows are remembered, docked ones forgotten.
        let tabs: Vec<(DeviceKey, egui_dock::TabPath)> = self
            .dock
            .iter_all_tabs()
            .filter_map(|(path, p)| match p {
                Pane::Device(k) => Some((*k, path)),
                _ => None,
            })
            .collect();
        for (key, path) in tabs {
            if device::is_maximised(ctx, key) {
                continue;
            }
            if path.surface.is_main() {
                device::forget_place(ctx, key);
            } else if let Some(r) = self.float_rect(ctx, path) {
                device::remember_place(ctx, key, r);
            }
        }
    }

    /// Float `pane` in a fresh window at `rect`. A window that already floats is made
    /// again rather than moved: egui keeps an existing window's place over a new position.
    fn refloat(&mut self, pane: Pane, rect: Rect) {
        if self.find(pane).is_some_and(|p| !p.surface.is_main()) {
            self.hide(pane);
            self.reveal(pane);
        }
        let Some(path) = self.find(pane).filter(|p| p.surface.is_main()) else {
            return;
        };
        let surface = self.dock.detach_tab(path, rect);
        if let Some(w) = self.dock.get_window_state_mut(surface) {
            w.set_position(rect.min).set_size(rect.size());
        }
    }

    /// A device tab was just put back in the dock: float it where it floated last, if it
    /// did. The first time, a device whose leaf is shorter than the device can be drawn in
    /// (its minimum size; the arrangement's half of the window rarely is tall enough for an
    /// instrument) floats at its own size over the workspace instead of being squeezed
    /// until its cards overlap; dragged into the dock, it stays docked.
    pub(crate) fn float_where_it_was(&mut self, ctx: &Context, key: DeviceKey) {
        let area = self.area;
        let rect = device::place_of(ctx, key).or_else(|| {
            let (size, min) = device::sizes(ctx, key)?;
            let path = self.find(Pane::Device(key))?;
            let leaf = self.dock.leaf(path.node_path()).ok()?.rect;
            let short = !leaf.is_positive() || leaf.height() < min.y || leaf.width() < min.x;
            (short && area.is_positive()).then(|| {
                let size = size
                    .min(area.size() - Vec2::splat(16.0))
                    .max(min.min(area.size()));
                Rect::from_center_size(area.center(), size)
            })
        });
        let Some(rect) = rect else {
            return;
        };
        if let Some(path) = self.find(Pane::Device(key)).filter(|p| p.surface.is_main()) {
            let surface = self.dock.detach_tab(path, rect);
            if let Some(w) = self.dock.get_window_state_mut(surface) {
                w.set_position(rect.min).set_size(rect.size());
            }
            device::settle(ctx, key, rect.min);
        }
    }
}

impl Workspace {
    /// Where a floating tab's window is, as `set_position` and `set_size` take it back: the
    /// window's corner and its content's size, from its leaf (egui_dock 0.21 never fills in
    /// `WindowState::rect`).
    fn float_rect(&self, ctx: &Context, path: egui_dock::TabPath) -> Option<Rect> {
        let leaf = self.dock.leaf(path.node_path()).ok()?.rect;
        // The window's frame sits around its leaf by the window margin.
        let m = ctx.global_style().spacing.window_margin;
        let corner = leaf.min - Vec2::new(m.leftf(), m.topf());
        let r = Rect::from_min_size(corner, leaf.size());
        r.is_positive().then_some(r)
    }
}

// ---- the device tray ----------------------------------------------------------------------

/// A chip's widest name at 100 %.
const CHIP_NAME_W: f32 = 150.0;

/// Where the tray's chips are, laid out from `left` in the status bar's `rect`: (key,
/// chip, its ×). Published for tests.
pub fn tray_layout(
    ctx: &Context,
    rect: Rect,
    left: f32,
    k: f32,
    items: &[(DeviceKey, String, crate::kit::Function)],
    out: &mut Vec<(DeviceKey, Rect, Rect)>,
) {
    out.clear();
    let s = Style::current();
    let mut x = left;
    let h = (rect.height() - 4.0 * k).max(12.0 * k);
    for (key, title, _) in items {
        let name = Text::new(title, Role::Label, s.t.text);
        let w = text::measure(ctx, &name, s.tk()).x.min(CHIP_NAME_W * k);
        let chip = Rect::from_min_size(
            Pos2::new(x, rect.center().y - h * 0.5),
            Vec2::new(8.0 * k + 12.0 * k + 5.0 * k + w + 4.0 * k + h, h),
        );
        let close = Rect::from_min_max(Pos2::new(chip.right() - h, chip.top()), chip.max);
        out.push((*key, chip, close));
        x = chip.right() + 4.0 * k;
    }
}

/// The device tray in the status bar, from `left`: a chip for each minimised device —
/// its mark in its hue, its name, a × — a click brings it back, the × closes it. Returns
/// where the tray ends (the status line starts there).
pub fn tray(ui: &mut Ui, rect: Rect, left: f32, k: f32) -> f32 {
    let ctx = ui.ctx().clone();
    let mut items = Vec::new();
    device::minimised(&ctx, &mut items);
    if items.is_empty() {
        return left;
    }
    let mut chips = Vec::with_capacity(items.len());
    tray_layout(&ctx, rect, left, k, &items, &mut chips);
    let mut end = left;
    let mut painted = Vec::with_capacity(items.len());
    for ((key, title, hue), (_, chip, close)) in items.iter().zip(&chips) {
        let chip = chip.intersect(rect);
        if chip.width() < 24.0 * k {
            break;
        }
        end = chip.right() + 6.0 * k;
        publish(&ctx, Spot::TrayChip(*key), chip);
        publish(&ctx, Spot::TrayClose(*key), *close);
        let resp = ui.interact(chip, ui.id().with(("device-tray", *key)), Sense::click());
        let x = ui.interact(
            *close,
            ui.id().with(("device-tray-x", *key)),
            Sense::click(),
        );
        let over_x = x.hovered();
        if x.clicked() {
            device::mark_closed(&ctx, *key);
        } else if resp.clicked() {
            device::restore(&ctx, *key);
        }
        if over_x {
            let tip = tf("swap.tray.close", &[("name", title)]);
            crate::kit::hint::set_text(&ctx, &tip, "", crate::kit::hint::Caps::NONE);
            let _ = x.on_hover_text(tip);
        } else if resp.hovered() {
            let tip = tf("swap.tray.restore", &[("name", title)]);
            crate::kit::hint::set_text(&ctx, &tip, "", crate::kit::hint::Caps::NONE);
            let _ = resp.clone().on_hover_text(tip);
        }
        let st = State {
            hovered: resp.hovered(),
            active: resp.is_pointer_button_down_on() && !over_x,
            ..State::REST
        };
        painted.push(TrayChip {
            chip,
            close: *close,
            title: title.clone(),
            hue: *hue,
            st,
            over_x,
        });
    }
    TRAY.with(|t| *t.borrow_mut() = painted);
    end
}

/// A chip as [`tray`] sensed it, for [`tray_paint`].
struct TrayChip {
    chip: Rect,
    close: Rect,
    title: String,
    hue: crate::kit::Function,
    st: State,
    over_x: bool,
}

thread_local! {
    static TRAY: std::cell::RefCell<Vec<TrayChip>> = const { std::cell::RefCell::new(Vec::new()) };
}

/// Paint the chips [`tray`] laid out this frame, over the status bar's ground (which the
/// bar paints after it has asked the tray where its status line starts).
pub fn tray_paint(ui: &Ui, rect: Rect, k: f32) {
    let chips = TRAY.with(|t| std::mem::take(&mut *t.borrow_mut()));
    if chips.is_empty() {
        return;
    }
    let s = Style::current();
    let t = &s.t;
    let painter = ui.painter_at(rect);
    for c in &chips {
        let (chip, close, st, over_x, hue) = (c.chip, &c.close, c.st, c.over_x, &c.hue);
        let title = &c.title;
        with_brush(&painter, |b| {
            b.register(
                chip,
                "device_tray_chip",
                crate::kit::window_ctl::TOKENS,
                &[],
            );
            let look = cap::CapLook {
                base: Some(shade(t.panel, if t.dark { 0.08 } else { 0.06 })),
                radius: Some(3.0),
                ..cap::CapLook::default()
            };
            cap::paint(b, b.snap_rect(chip), &s, st, &look);
            // The device's mark in its hue: which device this is, at a glance.
            let mark = Rect::from_center_size(
                Pos2::new(chip.left() + 8.0 * k + 6.0 * k, chip.center().y),
                Vec2::splat(12.0 * k),
            );
            glyph::paint(b, mark, Glyph::Device, t.hue(*hue));
            let name = Rect::from_min_max(
                Pos2::new(mark.right() + 5.0 * k, chip.top()),
                Pos2::new(close.left() - 2.0 * k, chip.bottom()),
            );
            let ink = if st.hovered { t.text } else { t.text_dim };
            let label = Text::new(title, Role::Label, ink)
                .align(Align2::LEFT_CENTER)
                .policy(Policy::Elide(Elide::Middle));
            let _ = text::paint(b, name, &label, s.tk());
            if over_x {
                b.rrect(close.shrink(2.0 * k), 2.0 * k, Fill::Solid(t.danger));
            }
            let xc = if over_x {
                crate::kit::color::on_fill(t.danger)
            } else {
                ink
            };
            glyph::paint(b, close.shrink(5.0 * k), Glyph::Close, xc);
        });
    }
}

// ---- entry points: the small affordances the views share ----------------------------------

/// The swap badge's side at 100 %: a square at the right end of a slot or a button.
pub const BADGE: f32 = 16.0;

/// Where the swap badge of `rect` goes: a square at its right end, inset by 1 pt.
pub fn badge_rect(rect: Rect, k: f32) -> Rect {
    let side = (BADGE * k).min(rect.height() - 2.0).max(8.0);
    Rect::from_center_size(
        Pos2::new(rect.right() - side * 0.5 - 2.0 * k, rect.center().y),
        Vec2::splat(side),
    )
}

/// The swap badge over a slot, a channel button or a header chip (round 20): shown while
/// the pointer is over `rect` — a raised square with the swap glyph — and a click on it
/// opens the picker for `purpose`. Registered after the slot's own response, so it takes
/// the click where it is. Paints into `b`; returns whether it was clicked.
#[allow(clippy::too_many_arguments)]
pub fn swap_badge(
    app: &mut CantusApp,
    ui: &Ui,
    b: &mut crate::kit::Brush,
    rect: Rect,
    id: egui::Id,
    k: f32,
    purpose: PickPurpose,
    tip_key: &'static str,
) -> bool {
    if !ui.rect_contains_pointer(rect) || ui.ctx().dragged_id().is_some() {
        return false;
    }
    let r = badge_rect(rect, k);
    let resp = ui.interact(r, id, Sense::click());
    let s = Style::current();
    let t = &s.t;
    let st = State {
        hovered: resp.hovered(),
        active: resp.is_pointer_button_down_on(),
        ..State::REST
    };
    let look = cap::CapLook {
        base: Some(shade(t.panel_raised, if t.dark { 0.06 } else { 0.03 })),
        radius: Some(2.5),
        ..cap::CapLook::default()
    };
    b.register(r, "swap_badge", crate::kit::window_ctl::TOKENS, &[]);
    cap::paint(b, b.snap_rect(r), &s, st, &look);
    let ink = if st.hovered { t.text } else { t.text_dim };
    glyph::paint(b, r.shrink(r.width() * 0.16), Glyph::Replace, ink);
    if resp.hovered() {
        let tip = crate::i18n::t(tip_key);
        crate::kit::hint::set_text(ui.ctx(), &tip, "", crate::kit::hint::Caps::NONE);
        let _ = resp.clone().on_hover_text(tip);
    }
    if resp.clicked() {
        app.open_picker(purpose);
        return true;
    }
    false
}

/// The track header's instrument chip (round 20): the instrument's icon, raised into a
/// key while the pointer is on it, with a ▾ that says it opens a choice. A click opens
/// the picker to swap the instrument ([`chip_input`]). Paints the key into `b`; the icon
/// itself is drawn over it by the caller.
pub fn chip_plate(
    b: &mut crate::kit::Brush,
    icon: Rect,
    hovered: bool,
    ink: egui::Color32,
    k: f32,
) {
    if !hovered {
        return;
    }
    let s = Style::current();
    let t = &s.t;
    let r = icon.expand(3.0 * k);
    b.register(r, "instrument_chip", crate::kit::window_ctl::TOKENS, &[]);
    b.rrect(
        b.snap_rect(r),
        3.0 * k,
        Fill::Solid(crate::kit::color::with_alpha(t.dark_shade, 0.3)),
    );
    b.rrect_stroke(
        b.snap_rect(r),
        3.0 * k,
        b.px(),
        crate::kit::color::with_alpha(ink, 0.6),
    );
    // The ▾ inside the key's corner: it opens a choice.
    let down = Rect::from_center_size(
        Pos2::new(r.right() - 3.0 * k, r.bottom() - 3.0 * k),
        Vec2::splat(6.0 * k),
    );
    glyph::paint(b, down, Glyph::Down, ink);
}

/// The instrument chip's input: a click opens the picker to swap `track`'s instrument.
pub fn chip_input(app: &mut CantusApp, ui: &Ui, icon: Rect, track: TrackId, k: f32) {
    let r = icon.expand(3.0 * k);
    publish(ui.ctx(), Spot::Chip(track), r);
    let resp = ui.interact(
        r,
        ui.id().with(("instrument-chip", track.0)),
        Sense::click(),
    );
    if resp.hovered() {
        let name = app
            .project()
            .track(track)
            .map(|t| crate::swap::instrument_name(&t.instrument))
            .unwrap_or_default();
        let tip = tf("swap.chip.tip", &[("name", &name)]);
        crate::kit::hint::set_text(ui.ctx(), &tip, "", crate::kit::hint::Caps::NONE);
        let _ = resp.clone().on_hover_text(tip);
    }
    if resp.clicked() {
        app.select_track(track);
        app.open_picker(PickPurpose::ReplaceInstrument { track });
    }
}

// ---- published geometry -------------------------------------------------------------------

/// A place that opens the picker or a device, published for tests: where it was drawn
/// last.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum Spot {
    /// The track header's instrument chip.
    Chip(TrackId),
    /// The channel rack's channel button.
    RackButton(TrackId),
    /// The inspector's instrument field.
    InspectorInstrument(TrackId),
    /// The inspector's empty insert tile `n` (0-based).
    InspectorEmptySlot(usize),
    /// The arrangement's add-track key.
    AddTrack,
    /// A minimised device's chip in the tray, and its ×.
    TrayChip(DeviceKey),
    TrayClose(DeviceKey),
}

type Spots = std::sync::Arc<std::sync::Mutex<std::collections::HashMap<Spot, Rect>>>;

fn spots(ctx: &Context) -> Spots {
    ctx.data_mut(|d| {
        d.get_temp_mut_or_insert_with(egui::Id::new("cantus-device-spots"), Spots::default)
            .clone()
    })
}

/// Publish where `spot` was drawn this frame.
pub fn publish(ctx: &Context, spot: Spot, rect: Rect) {
    let s = spots(ctx);
    let mut g = s.lock().unwrap_or_else(|e| e.into_inner());
    g.insert(spot, rect);
}

/// Where `spot` was drawn last, if it was.
pub fn spot(ctx: &Context, spot: Spot) -> Option<Rect> {
    let s = spots(ctx);
    let g = s.lock().unwrap_or_else(|e| e.into_inner());
    g.get(&spot).copied()
}

// ---- drop targets -------------------------------------------------------------------------

/// A drop target for a device dragged out of the browsing panel (ADR 0029 §6): outlined in
/// the selection colour while a device that fits hovers it, and `apply_pick` with
/// `purpose` when it is let go there. Returns whether a device was dropped.
pub fn drop_target(
    app: &mut CantusApp,
    ui: &Ui,
    resp: &egui::Response,
    purpose: PickPurpose,
) -> bool {
    use crate::picker::DevicePayload;
    if let Some(p) = resp.dnd_hover_payload::<DevicePayload>() {
        if crate::swap::available(&purpose, &p.0) {
            let t = Style::current().t;
            with_brush(ui.painter(), |b| {
                b.rrect_stroke(b.snap_rect(resp.rect.shrink(1.0)), 3.0, 2.0, t.selection)
            });
        }
    }
    if let Some(p) = resp.dnd_release_payload::<DevicePayload>() {
        if crate::swap::available(&purpose, &p.0) {
            app.apply_pick(purpose, p.0.clone());
            return true;
        }
    }
    false
}
