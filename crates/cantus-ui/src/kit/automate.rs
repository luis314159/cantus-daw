//! A parameter's own menu and its automation mark (ADR 0027), shared by every control that
//! is bound to a parameter — the kit's [`Param`](super::widgets::Param), the effects'
//! cells, the mixer's faders — so right-clicking any knob in Cantus offers the same thing
//! FL's does: *Create automation clip* (or *Edit automation* once there is one), type a
//! value, reset.
//!
//! The kit knows nothing of projects: the caller says whether the parameter is automated
//! and acts on the choice it gets back.

use super::menu::Item;
use super::text::Src;
use super::{Brush, Fill, Style};
use egui::{Pos2, Rect, Response, Ui};

/// What a parameter's menu offers.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub struct Offer {
    /// A lane drives the parameter already: the first entry edits it.
    pub automated: bool,
    /// The control can be typed into.
    pub typing: bool,
}

/// What was chosen.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Choice {
    /// Create an automation clip, or (automated) open the one there is.
    Automate,
    Type,
    Reset,
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
fn frame() -> egui::Frame {
    let s = Style::current();
    egui::Frame::NONE
        .fill(s.t.menu)
        .corner_radius(egui::CornerRadius::same(s.r(3.0).round() as u8))
        .inner_margin(egui::Margin::same(3))
}

/// The automated mark: a small lit dot at a control's upper right, in the selection's
/// colour, ringed so it reads on any face — "a clip moves this".
pub fn mark(b: &mut Brush, rect: Rect, s: &Style) {
    let r = 2.6 * s.k;
    let c = Pos2::new(rect.right() - r - 1.5 * s.k, rect.top() + r + 1.5 * s.k);
    b.circle(c, r + b.px(), Fill::Solid(s.t.panel));
    b.circle(c, r, Fill::Solid(s.t.selection));
}
