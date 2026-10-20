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
//! **MIDI learn** (round 29, ADR 0062): *Learn MIDI controller* and *Forget MIDI
//! controller* in every parameter's menu, a mark on a mapped control, and the learn mode
//! (Ableton's MIDI Map mode): every learnable control outlined, a click on one answers
//! [`Choice::Learn`] and its value does not move.
//!
//! The kit knows nothing of projects: the caller says whether the parameter is automated,
//! modulated or mapped and acts on the choice it gets back.

use super::menu::Item;
use super::text::Src;
use super::{Brush, Fill, Style};
use egui::{Pos2, Rect, Response, Ui};

/// Every parameter control drawn on this thread while a probe runs (review 1): its name and
/// whether it offered modulation. A test opens each editor with the probe on and finds any
/// control that does not offer *Modulate…*; off (the app), it costs a flag read.
pub mod probe {
    use std::cell::RefCell;

    thread_local! {
        static SEEN: RefCell<Option<Vec<(String, bool)>>> = const { RefCell::new(None) };
        static LIVE: RefCell<Option<Vec<(String, &'static str)>>> = const { RefCell::new(None) };
        static LEARN: RefCell<Option<Vec<(String, bool)>>> = const { RefCell::new(None) };
    }

    /// Start recording (what was recorded before is dropped).
    pub fn start() {
        SEEN.with(|s| *s.borrow_mut() = Some(Vec::new()));
        LIVE.with(|s| *s.borrow_mut() = Some(Vec::new()));
        LEARN.with(|s| *s.borrow_mut() = Some(Vec::new()));
    }

    /// A parameter control was drawn, `learnable` when its menu offered *Learn MIDI
    /// controller* (round 29, ADR 0062).
    pub fn note_learn(name: &str, learnable: bool) {
        LEARN.with(|s| {
            if let Some(v) = s.borrow_mut().as_mut() {
                v.push((name.to_string(), learnable));
            }
        });
    }

    /// The learn offers noted since [`start`] (and stop noting them).
    pub fn take_learn() -> Vec<(String, bool)> {
        LEARN.with(|s| s.borrow_mut().take()).unwrap_or_default()
    }

    /// A control showed where modulation has it now, the way `how` names (review of fix4:
    /// which mark the inspector's pinned strip draws for each control).
    pub fn note_live(name: &str, how: &'static str) {
        LIVE.with(|s| {
            if let Some(v) = s.borrow_mut().as_mut() {
                v.push((name.to_string(), how));
            }
        });
    }

    /// The live marks noted since [`start`] (and stop noting them).
    pub fn take_live() -> Vec<(String, &'static str)> {
        LIVE.with(|s| s.borrow_mut().take()).unwrap_or_default()
    }

    /// Stop recording and hand back what was drawn.
    pub fn take() -> Vec<(String, bool)> {
        SEEN.with(|s| s.borrow_mut().take()).unwrap_or_default()
    }

    /// A parameter control was drawn, `modulated` when it offered modulation.
    pub fn note(name: &str, modulated: bool) {
        SEEN.with(|s| {
            if let Some(v) = s.borrow_mut().as_mut() {
                v.push((name.to_string(), modulated));
            }
        });
    }
}

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
    /// What MIDI learn knows of it (round 29, ADR 0062); `None` where it cannot be learnt.
    pub learn: Option<LearnOffer>,
}

/// A learnable control's MIDI mapping (round 29, ADR 0062).
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub struct LearnOffer {
    /// A controller drives it: the mark, and *Forget* in its menu.
    pub mapped: bool,
    /// The learn mode is on: outlined, and a click learns.
    pub mode: bool,
    /// It is waiting for a hardware control to move.
    pub armed: bool,
}

impl Offer {
    /// What MIDI learn knows of the control, when it can be learnt.
    pub fn learn(&self) -> Option<LearnOffer> {
        self.modulation.and_then(|m| m.learn)
    }
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
    /// Learn the next hardware control moved for it (its menu, or a click in learn mode).
    Learn,
    /// Take its MIDI mapping away.
    Forget,
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
                label: Src::Key("mods.param.modulate"),
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
        // MIDI learn (round 29, ADR 0062).
        if let Some(l) = offer.learn() {
            items.push(Item::action(Src::Key(if l.mapped {
                "ctl.param.relearn"
            } else {
                "ctl.param.learn"
            })));
            choices.push(Choice::Learn);
            if l.mapped {
                items.push(Item::action(Src::Key("ctl.param.forget")));
                choices.push(Choice::Forget);
            }
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
    // In learn mode a drag moves nothing (the click learns).
    if m.learn.is_some_and(|l| l.mode) {
        return true;
    }
    m.mapping || (m.depth.is_some() && ui.input(|i| i.modifiers.alt))
}

/// What a control's modulation did this frame: a handle dropped on it, a depth drag's new
/// depth (Shift for fine) or its end. `None` when none of these happened.
pub fn modulation_choice(ui: &Ui, resp: &Response, offer: Option<Offer>) -> Option<Choice> {
    let m = offer.and_then(|o| o.modulation)?;
    if let Some(l) = m.learn.filter(|l| l.mode) {
        return learn_spot(ui, resp.rect, resp.id, l).then_some(Choice::Learn);
    }
    // Only a modulator's handle is ours: egui's release takes whatever is carried, and a
    // control also takes other drops (an instrument's source chip, a track for routing).
    if handle_in_flight(&resp.ctx) {
        if let Some(h) = resp.dnd_release_payload::<ModHandle>() {
            return Some(Choice::Connect(h.0));
        }
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

/// The mapped mark (round 29, ADR 0062): a small dot in the highlight's colour at the
/// control's lower right — "a hardware control moves this".
pub fn learn_mark(b: &mut Brush, rect: Rect, s: &Style) {
    let r = 2.6 * s.k;
    let c = Pos2::new(rect.right() - r - 1.5 * s.k, rect.bottom() - r - 1.5 * s.k);
    b.circle(c, r + b.px(), Fill::Solid(s.t.panel));
    b.circle(c, r, Fill::Solid(s.t.highlight));
}

/// A learnable spot in learn mode (round 29, ADR 0062): outlined above whatever is drawn
/// there — faint when free, in the highlight's colour when mapped, lit while it waits for a
/// hardware control — and covered so the control under it does not move: `true` when it was
/// clicked. Every learnable control of the kit goes through here; buttons that are not
/// parameters (a mute, the transport's) call it themselves.
pub fn learn_spot(ui: &Ui, rect: Rect, id: egui::Id, l: LearnOffer) -> bool {
    learn_spot_clipped(ui, rect, ui.clip_rect(), id, l)
}

/// [`learn_spot`] shown and clickable only inside `clip`: a control scrolled half under a
/// view's dock or ruler is outlined and covered where it shows, never over what is on top.
pub fn learn_spot_clipped(ui: &Ui, rect: Rect, clip: Rect, id: egui::Id, l: LearnOffer) -> bool {
    let clip = clip.intersect(ui.clip_rect());
    let shown = rect.intersect(clip);
    if shown.width() <= 0.0 || shown.height() <= 0.0 {
        return false;
    }
    let cover = ui.interact(
        shown,
        id.with("cantus-learn"),
        egui::Sense::click_and_drag(),
    );
    let s = Style::current();
    // Above the control's own painting: a sublayer of the layer it is drawn in.
    let parent = ui.layer_id();
    let layer = egui::LayerId::new(parent.order, parent.id.with("cantus-learn"));
    ui.ctx().set_sublayer(parent, layer);
    let painter = ui.ctx().layer_painter(layer).with_clip_rect(clip);
    super::with_brush(&painter, |b| {
        let r = rect.shrink(b.px());
        let (fill, line, w) = if l.armed {
            (s.t.led.gamma_multiply(0.22), s.t.led, s.s(2.0))
        } else if l.mapped {
            (s.t.highlight.gamma_multiply(0.18), s.t.highlight, s.s(1.5))
        } else if cover.hovered() {
            (s.t.focus.gamma_multiply(0.16), s.t.focus, s.s(1.5))
        } else {
            (s.t.focus.gamma_multiply(0.12), s.t.focus, s.s(1.0))
        };
        b.rrect(r, s.r(3.0), Fill::Solid(fill));
        b.rrect_stroke(r, s.r(3.0), w, line);
    });
    if cover.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    cover.clicked()
}

/// The mapped mark over a control drawn by its caller (a lamp, a key), on a layer above it,
/// shown only inside `clip`.
pub fn learn_mark_over(ui: &Ui, rect: Rect, clip: Rect) {
    let s = Style::current();
    let parent = ui.layer_id();
    let layer = egui::LayerId::new(parent.order, parent.id.with("cantus-learn"));
    ui.ctx().set_sublayer(parent, layer);
    let painter = ui
        .ctx()
        .layer_painter(layer)
        .with_clip_rect(clip.intersect(ui.clip_rect()));
    super::with_brush(&painter, |b| learn_mark(b, rect, &s));
}

/// A handle hovering over a control that would take it: the drop target's outline.
pub fn drop_target(b: &mut Brush, rect: Rect, s: &Style) {
    b.rrect_stroke(rect.shrink(b.px()), s.r(3.0), s.s(1.5), s.t.hue_mod);
}

/// Whether a modulator's handle is being dragged anywhere now.
pub fn handle_in_flight(ctx: &egui::Context) -> bool {
    egui::DragAndDrop::has_payload_of_type::<ModHandle>(ctx)
}
