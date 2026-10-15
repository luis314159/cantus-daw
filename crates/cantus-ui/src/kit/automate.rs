//! A parameter's own menu and its automation mark (ADR 0027), shared by every control that
//! is bound to a parameter — the kit's [`Param`](super::widgets::Param), the effects'
//! cells, the mixer's faders — so right-clicking any knob in Cantus offers the same thing
//! FL's does: *Create automation clip* (or *Edit automation* once there is one), type a
//! value, reset — and *Modulate…* (ADR 0057).
//!
//! **Modulation on the control itself** (ADR 0057, Bitwig's way): a modulator's handle
//! ([`ModHandle`], dragged from the Modulators panel) dropped on a control connects it; with
//! **Alt** held, a drag on a modulated control edits the depth of its route instead of its
//! value — and while a modulator is *mapping*, a plain drag does. The control reports these
//! as [`Choice`]s; the caller knows which parameter it is.
//!
//! The kit knows nothing of projects: the caller says whether the parameter is automated
//! or modulated and acts on the choice it gets back.

use super::menu::Item;
use super::text::Src;
use super::{Brush, Fill, Style};
use egui::{Pos2, Rect, Response, Ui};

/// What a parameter's menu offers.
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub struct Offer {
    /// A lane drives the parameter already: the first entry edits it.
    pub automated: bool,
    /// The control can be typed into.
    pub typing: bool,
    /// It can be modulated (ADR 0057): *Modulate…*, the handle's drop and the depth drag.
    pub modulation: Option<ModOffer>,
}

/// What a modulatable control knows of its modulation (ADR 0057).
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub struct ModOffer {
    /// The depth of the route a depth drag edits (the mapping modulator's, else the
    /// first): `None` when nothing modulates it and nothing is mapping.
    pub depth: Option<f32>,
    /// A modulator is mapping (Bitwig's mode): a plain drag edits its depth here.
    pub mapping: bool,
    /// The parameter's position now, modulation in (0…1), for the live dot.
    pub live: Option<f32>,
}

impl Offer {
    /// The menu of a parameter that can be automated and modulated.
    pub fn full(automated: bool, typing: bool, m: ModOffer) -> Offer {
        Offer {
            automated,
            typing,
            modulation: Some(m),
        }
    }
}

/// A modulator's handle on its way to a control (egui's drag and drop payload): the
/// modulator's id.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct ModHandle(pub u64);

/// What was chosen.
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum Choice {
    /// Create an automation clip, or (automated) open the one there is.
    Automate,
    Type,
    Reset,
    /// Open the *Modulate…* list for this parameter (ADR 0057).
    Modulate,
    /// A depth drag: the route's new depth (−1…1).
    Depth(f32),
    /// The depth drag let go: the end of its undo step.
    DepthDone,
    /// A modulator's handle was dropped on the control: connect it.
    Connect(u64),
}

/// The menu on a right-click of `resp`; the choice, the frame it is made.
pub fn menu(ui: &Ui, resp: &Response, offer: Offer) -> Option<Choice> {
    let mut chosen = None;
    egui::Popup::context_menu(resp).frame(frame()).show(|ui| {
        let mut items = vec![Item::action(Src::Key(if offer.automated {
            "auto.param.open"
        } else {
            "auto.param.create"
        }))];
        let mut choices = vec![Choice::Automate];
        if offer.modulation.is_some() {
            items.push(Item::Action {
                label: Src::Key("mod.param.modulate"),
                check: super::menu::Check::None,
                shortcut: None,
                submenu: true,
                enabled: true,
            });
            choices.push(Choice::Modulate);
        }
        if offer.typing {
            items.push(Item::action(Src::Key("auto.param.type")));
            choices.push(Choice::Type);
        }
        items.push(Item::Separator);
        choices.push(Choice::Reset);
        items.push(Item::action(Src::Key("auto.param.reset")));
        if let Some(i) = super::widgets::menu_list(ui, &items, 220.0).1.clicked {
            // The separator is not a choice: rows after it shift by one.
            let k = if i >= items.len() - 1 {
                choices.len() - 1
            } else {
                i
            };
            chosen = choices.get(k).copied();
            ui.close();
        }
    });
    let _ = ui;
    chosen
}

/// The menus' frame: the kit's menu face.
pub fn frame() -> egui::Frame {
    let s = Style::current();
    egui::Frame::NONE
        .fill(s.t.menu)
        .corner_radius(egui::CornerRadius::same(s.r(3.0).round() as u8))
        .inner_margin(egui::Margin::same(3))
}

/// Pointer travel for a whole depth, as a knob's whole travel.
const DEPTH_TRAVEL_PX: f32 = 200.0;

/// Whether this frame's drag edits a modulation depth rather than the value (Alt held on
/// a modulated control, or a modulator mapping).
pub fn depth_drag(ui: &Ui, offer: Option<Offer>) -> bool {
    let Some(m) = offer.and_then(|o| o.modulation) else {
        return false;
    };
    m.mapping || (m.depth.is_some() && ui.input(|i| i.modifiers.alt))
}

/// What a control's modulation did this frame: a handle dropped on it, a depth drag's new
/// depth (Shift for fine) or its end. `None` when none of these happened.
pub fn modulation_choice(ui: &Ui, resp: &Response, offer: Option<Offer>) -> Option<Choice> {
    let m = offer.and_then(|o| o.modulation)?;
    if let Some(h) = resp.dnd_release_payload::<ModHandle>() {
        return Some(Choice::Connect(h.0));
    }
    if !depth_drag(ui, offer) {
        return None;
    }
    if resp.drag_stopped() {
        return Some(Choice::DepthDone);
    }
    if !resp.dragged() {
        return None;
    }
    let fine = if ui.input(|i| i.modifiers.shift) {
        0.2
    } else {
        1.0
    };
    let d = resp.drag_delta();
    let delta = (d.x - d.y) * fine / DEPTH_TRAVEL_PX;
    (delta != 0.0).then(|| Choice::Depth((m.depth.unwrap_or(0.0) + delta).clamp(-1.0, 1.0)))
}

/// The automated mark: a small lit dot at a control's upper right, in the selection's
/// colour, ringed so it reads on any face — "a clip moves this".
pub fn mark(b: &mut Brush, rect: Rect, s: &Style) {
    let r = 2.6 * s.k;
    let c = Pos2::new(rect.right() - r - 1.5 * s.k, rect.top() + r + 1.5 * s.k);
    b.circle(c, r + b.px(), Fill::Solid(s.t.panel));
    b.circle(c, r, Fill::Solid(s.t.selection));
}

/// The modulated mark, opposite the automated one: a small dot in the modulation hue at
/// the control's upper left — "a modulator moves this".
pub fn mod_mark(b: &mut Brush, rect: Rect, s: &Style) {
    let r = 2.6 * s.k;
    let c = Pos2::new(rect.left() + r + 1.5 * s.k, rect.top() + r + 1.5 * s.k);
    b.circle(c, r + b.px(), Fill::Solid(s.t.panel));
    b.circle(c, r, Fill::Solid(s.t.hue_mod));
}

/// A handle hovering over a control that would take it: the drop target's outline.
pub fn drop_target(b: &mut Brush, rect: Rect, s: &Style) {
    b.rrect_stroke(rect.shrink(b.px()), s.r(3.0), s.s(1.5), s.t.hue_mod);
}

/// Whether a modulator's handle is being dragged anywhere now.
pub fn handle_in_flight(ctx: &egui::Context) -> bool {
    egui::DragAndDrop::has_payload_of_type::<ModHandle>(ctx)
}
