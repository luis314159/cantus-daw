//! r40:pads — a pad's own mixer track, chosen from the rack's well (ADR 0117).
//!
//! FL's channel rack gives every channel a mixer-track number; ours gives every pad of a kit
//! one too, in the well left of its name (des-views: "3" under the kit's "3"). A click opens
//! the list: *the kit's track* (the default, where every pad starts), *the first free
//! track*, and every mixer track of the desk. The choice is one command and one undo step
//! (`cantus_core::padout::set_pad_track`); a pad with a chain keeps it.

use crate::app::CantusApp;
use crate::i18n::tf;
use crate::kit::hint::{self, Caps};
use crate::kit::menu::{Check, Item};
use crate::kit::text::Src;
use cantus_core::padout::{self, Refusal};
use cantus_core::{PadId, StripId, TrackId};
use egui::{Rect, Sense};

/// One choice of the list.
#[derive(Clone, Debug, PartialEq)]
pub struct Entry {
    pub label: String,
    /// `None` is the kit's own track.
    pub to: Option<StripId>,
    pub chosen: bool,
}

/// The list a pad's well opens: the kit's track, the first free one, every other mixer
/// track that can carry a pad, in desk order (each `number · name`) — each strip once.
pub fn entries(app: &CantusApp, track: TrackId, pad: PadId) -> Vec<Entry> {
    let p = app.project();
    let Some(t) = p.track(track) else {
        return Vec::new();
    };
    let Some(x) = t.instrument.pulsus().and_then(|k| k.by_id(pad)) else {
        return Vec::new();
    };
    let own = padout::has_own_track(p, t, x);
    let now = padout::pad_track(p, t, x);
    let kit = match t.strip.and_then(|s| p.strip_number(s)) {
        Some(n) => tf("pads.track_kit", &[("n", &n)]),
        None => crate::i18n::t("pads.track_kit_master"),
    };
    let mut out = vec![Entry {
        label: kit,
        to: None,
        chosen: !own,
    }];
    let free = p.first_free_strip();
    if let Some(free) = free {
        let n = p.strip_number(free).unwrap_or(0);
        out.push(Entry {
            label: tf("pads.track_free", &[("n", &n)]),
            to: Some(free),
            chosen: false,
        });
    }
    // Every other strip once: the kit's own and the first free one have their entries
    // above (review mid-round 40, C-8: "The kit's strip (3)" and "3 · Hats" were one).
    let listed = |id: StripId| Some(id) == t.strip || Some(id) == free;
    for s in p
        .strips
        .iter()
        .filter(|s| !listed(s.id) && padout::can_carry_pad(p, s.id))
    {
        let mut name = String::new();
        crate::views::inspector::strip_label(app, s.id, &mut name);
        let n = p.strip_number(s.id).unwrap_or(0);
        out.push(Entry {
            label: format!("{n} · {name}"),
            to: Some(s.id),
            chosen: own && now == Some(s.id),
        });
    }
    out
}

impl CantusApp {
    /// Send pad `pad` of `track`'s kit to mixer track `to` (`None`: back on the kit's),
    /// one undo step. `false`, with a notice, when it cannot go there; `false` and quiet
    /// when it is there already.
    pub fn set_pad_track(&mut self, track: TrackId, pad: PadId, to: Option<StripId>) -> bool {
        match padout::set_pad_track(self.project(), track, pad, to) {
            Ok(cmd) => {
                let ok = self.exec(cmd);
                if ok {
                    if let Some(s) = to {
                        self.selected_strip = Some(s);
                    }
                }
                ok
            }
            Err(Refusal::Nothing) => false,
            Err(r) => {
                self.pad_refused(track, r);
                false
            }
        }
    }
}

/// The pad's well on its row: the hint, and a click opens the list.
pub(super) fn well(app: &mut CantusApp, ui: &mut egui::Ui, rect: Rect, track: TrackId, pad: PadId) {
    let id = ui.id().with(("rack-pad-route", track.0, pad.0));
    let resp = ui.interact(rect, id, Sense::click());
    if resp.hovered() {
        hint::set_text(
            ui.ctx(),
            crate::views::clips_chrome::word("pads.track_tip"),
            "",
            Caps {
                menu: true,
                ..Caps::NONE
            },
        );
    }
    let popup = egui::Popup::menu(&resp);
    if !popup.is_open() && !resp.clicked() {
        return;
    }
    let list = entries(app, track, pad);
    let chosen = popup
        .show(|ui| {
            let items: Vec<Item<'_>> = list
                .iter()
                .map(|e| Item::Action {
                    label: Src::Str(&e.label),
                    check: Check::Radio(e.chosen),
                    shortcut: None,
                    submenu: false,
                    enabled: true,
                })
                .collect();
            // A long desk scrolls inside the window (the kit's menu sheet, `menu::fit`).
            crate::kit::menu::fit(ui, |ui| {
                crate::kit::widgets::menu_list(ui, &items, 200.0).1.clicked
            })
        })
        .and_then(|r| r.inner);
    if let Some(e) = chosen.and_then(|i| list.get(i)) {
        if !e.chosen {
            app.set_pad_track(track, pad, e.to);
        }
    }
}
