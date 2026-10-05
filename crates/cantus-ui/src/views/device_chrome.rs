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
        self.inst.open == Some(track) || self.pulsus.open == Some(track)
    }

    /// Open `track`'s instrument window, or bring it back from the tray. A track whose
    /// instrument has no window of its own (Firmus, edited in the inspector) shows the
    /// inspector instead.
    pub fn open_instrument_window(&mut self, track: TrackId) {
        let Some(spec) = self.project().track(track).map(|t| t.instrument.clone()) else {
            return;
        };
        if let (Some(ctx), Some(key)) = (self.theme_ctx.clone(), self.instrument_window_key(track))
        {
            if self.instrument_window_open(track) {
                device::restore(&ctx, key);
                return;
            }
        }
        match spec {
            InstrumentSpec::Native(_) => self.open_inst_editor(track),
            InstrumentSpec::Pulsus(_) => super::pulsus_editor::open(self, track),
            _ => {
                self.select_track(track);
                if !self.workspace.is_visible(Pane::Inspector) {
                    self.toggle_pane(Pane::Inspector);
                }
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
    }

    /// F8: the selected track's instrument window — open it when it is closed, bring it
    /// back when it is minimised, close it when it shows.
    pub fn toggle_instrument_window(&mut self) {
        let Some(track) = self.selected_track else {
            return;
        };
        let minimised = match (self.theme_ctx.clone(), self.instrument_window_key(track)) {
            (Some(ctx), Some(key)) => device::is_minimised(&ctx, key),
            _ => false,
        };
        if self.instrument_window_open(track) && !minimised {
            self.close_instrument_window(track);
        } else {
            self.open_instrument_window(track);
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
                        .then(|| self.dock.get_window_state(path.surface).map(|w| w.rect()))
                        .flatten()
                        .filter(|r| r.is_positive());
                    let surface = if path.surface.is_main() {
                        self.dock.detach_tab(path, area)
                    } else {
                        path.surface
                    };
                    if let Some(w) = self.dock.get_window_state_mut(surface) {
                        w.set_position(area.min).set_size(area.size());
                    }
                    device::set_maximised(ctx, key, true, was);
                }
                Pending::Restore => {
                    match restore {
                        Some(r) => {
                            if let Some(w) = self.dock.get_window_state_mut(path.surface) {
                                w.set_position(r.min).set_size(r.size());
                            }
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
            } else if let Some(r) = self
                .dock
                .get_window_state(path.surface)
                .map(|w| w.rect())
                .filter(|r| r.is_positive())
            {
                device::remember_place(ctx, key, r);
            }
        }
    }

    /// A device tab was just put back in the dock: float it where it floated last, if it
    /// did.
    pub(crate) fn float_where_it_was(&mut self, ctx: &Context, key: DeviceKey) {
        let Some(rect) = device::place_of(ctx, key) else {
            return;
        };
        if let Some(path) = self.find(Pane::Device(key)).filter(|p| p.surface.is_main()) {
            let surface = self.dock.detach_tab(path, rect);
            if let Some(w) = self.dock.get_window_state_mut(surface) {
                w.set_position(rect.min).set_size(rect.size());
            }
        }
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
    let s = Style::current();
    let t = &s.t;
    let painter = ui.painter_at(rect);
    let mut end = left;
    for ((key, title, hue), (_, chip, close)) in items.iter().zip(&chips) {
        let chip = chip.intersect(rect);
        if chip.width() < 24.0 * k {
            break;
        }
        end = chip.right() + 6.0 * k;
        let resp = ui.interact(chip, ui.id().with(("device-tray", *key)), Sense::click());
        let x = ui.interact(*close, ui.id().with(("device-tray-x", *key)), Sense::click());
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
        with_brush(&painter, |b| {
            b.register(chip, "device_tray_chip", crate::kit::window_ctl::TOKENS, &[]);
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
    end
}
