//! What every popup audit asks of a list it opened (beta.14's dropdown and menu audits):
//! opaque — the probe colour painted over every layer beneath it shows through no point of
//! its sheet; inside the window; its words neither overlapping nor cut at their sides (cut
//! at the head or foot only at the sheet's edge, where a long list scrolls).

#![allow(dead_code)]

use super::paths_rig::*;
use cantus_ui::kit::appearance::Mode;
use egui::{LayerId, Order, Rect};
use std::collections::HashSet;

/// The looks every dropdown is opened in: (preset, light).
pub const LOOKS: [(&str, bool); 3] = [
    ("slate_night", false),
    ("slate_paper", true),
    ("contrast_light", true),
];

/// Install `look` (a preset, in the light or the dark mode).
pub fn wear(r: &mut Rig, look: &str, light: bool) {
    r.app.appearance.mode = if light { Mode::Light } else { Mode::Dark };
    if light {
        r.app.appearance.light.base = format!("preset:{look}");
    } else {
        r.app.appearance.dark.base = format!("preset:{look}");
    }
    r.app.apply_appearance();
    r.idle(4);
}

/// The floating layers drawn now, and their rects.
pub fn floating(r: &Rig) -> Vec<(LayerId, Rect)> {
    r.ctx.memory(|m| {
        let mut v: Vec<(LayerId, Rect)> = m
            .areas()
            .visible_layer_ids()
            .into_iter()
            .filter(|l| matches!(l.order, Order::Foreground | Order::Tooltip))
            .filter_map(|l| Some((l, m.area_rect(l.id)?)))
            .filter(|(_, r)| r.is_positive())
            .collect();
        v.sort_by_key(|(l, _)| l.id.value());
        v
    })
}

/// One finding: which list, where, what is wrong.
#[derive(Debug)]
pub struct Finding {
    pub look: &'static str,
    pub screen: String,
    pub field: String,
    pub what: String,
}

impl Finding {
    pub fn line(&self) -> String {
        format!(
            "{} {} {}: {}",
            self.look, self.screen, self.field, self.what
        )
    }
}

/// The probe colour: nothing of the app is this magenta.
pub const PROBE: egui::Color32 = egui::Color32::from_rgb(255, 0, 255);

/// The layers opened since `was`.
pub fn opened_since(r: &Rig, was: &[(LayerId, Rect)]) -> Vec<(LayerId, Rect)> {
    floating(r)
        .into_iter()
        .filter(|(l, _)| !was.iter().any(|(w, _)| w == l))
        .collect()
}

/// Ask of the list `sheet` (drawn by `layers`) what every popup owes: opaque, inside the
/// window, legible. Its findings are named `look`, `screen`, `name`.
pub fn inspect(
    r: &mut Rig,
    layers: &[(LayerId, Rect)],
    sheet: Rect,
    look: &'static str,
    screen: &str,
    name: &str,
    out: &mut Vec<Finding>,
) {
    let mut bad = |what: String| {
        out.push(Finding {
            look,
            screen: screen.to_string(),
            field: name.to_string(),
            what,
        })
    };
    let window = r.ctx.content_rect().expand(0.5);
    if !window.contains_rect(sheet) {
        bad(format!("runs off the window: {sheet:?} in {window:?}"));
    }
    // Opaque: the probe painted over every layer drawn beneath the list (and over the
    // panels), for a frame: nothing of it may show inside the list's sheet.
    let keep: HashSet<LayerId> = layers.iter().map(|(l, _)| *l).collect();
    let rank = |l: &LayerId, i: usize| (l.order as usize, i);
    let below: Vec<LayerId> = r.ctx.memory(|m| {
        let order: Vec<LayerId> = m.layer_ids().collect();
        let lowest = order
            .iter()
            .enumerate()
            .filter(|(_, l)| keep.contains(l))
            .map(|(i, l)| rank(l, i))
            .min()
            .unwrap_or((usize::MAX, 0));
        order
            .iter()
            .enumerate()
            .filter(|(i, l)| !keep.contains(l) && rank(l, *i) < lowest)
            .map(|(_, l)| *l)
            .collect()
    });
    r.probe = Some(PROBE);
    r.probe_layers = below;
    r.idle(1);
    let c = r.canvas();
    let texts = r.texts_above_probe();
    r.probe = None;
    r.probe_layers.clear();
    r.idle(1);
    let ppp = r.ctx.pixels_per_point();
    let probe = sheet.shrink(3.0);
    let (mut through, mut all) = (0, 0);
    let mut y = probe.top();
    while y < probe.bottom() {
        let mut x = probe.left();
        while x < probe.right() {
            let p = c.pixel((x * ppp) as usize, (y * ppp) as usize);
            all += 1;
            if p[0] > 200 && p[1] < 60 && p[2] > 200 {
                through += 1;
            }
            x += 4.0;
        }
        y += 4.0;
    }
    if through > 0 {
        bad(format!(
            "see-through: the probe shows through {through} of {all} points of {sheet:?}"
        ));
    }
    // Legible: its words, seen inside the sheet.
    let words: Vec<_> = texts
        .iter()
        .filter(|t| !t.text.trim().is_empty() && t.seen.is_positive() && sheet.intersects(t.seen))
        .collect();
    for t in &words {
        if t.seen.width() + 0.5 < t.rect.width() {
            bad(format!(
                "“{}” cut at its side: {:?} seen of {:?}",
                t.text, t.seen, t.rect
            ));
        }
        // Cut at its head or foot only at the sheet's edge, where a long list scrolls.
        let at_edge = t.seen.top() - sheet.top() < 14.0 || sheet.bottom() - t.seen.bottom() < 14.0;
        if t.seen.height() + 0.5 < t.rect.height() && !at_edge {
            bad(format!(
                "“{}” cut: {:?} seen of {:?}",
                t.text, t.seen, t.rect
            ));
        }
    }
    for (i, a) in words.iter().enumerate() {
        for b in &words[i + 1..] {
            let x = a.seen.intersect(b.seen);
            if x.width() > 2.0 && x.height() > 3.0 {
                bad(format!("“{}” and “{}” overlap at {x:?}", a.text, b.text));
            }
        }
    }
}
