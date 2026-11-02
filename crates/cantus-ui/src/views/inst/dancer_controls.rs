//! Saltator's controls (round 35, ADR 0087): every number in the unit a person reads — a
//! move and a mood by name, the travel by where it goes, speed as ¼×…4×, size as ×, the
//! position and the levels as %, the facing as left/right, colour in degrees — and
//! *React to ‹a track›*, the one control that is not a number.

use super::super::analog::parts::push_at;
use super::super::organ::parts::at;
use crate::app::CantusApp;
use crate::i18n::t;
use crate::kit::cell;
use crate::kit::hint::Caps;
use crate::kit::knob::KnobSize;
use crate::kit::text::Src;
use crate::kit::widgets::Param;
use crate::kit::{with_brush, Style};
use cantus_core::inst::dancer::{self, Mood, SPEEDS};
use cantus_core::modulator::Listen;
use cantus_core::{NativeSpec, Project, StripId, TrackId};
use egui::{Pos2, Rect, Sense, Vec2};
use std::fmt::Write as _;

/// The words of `step`'s values, from 0.
pub const STEPS: [&str; 4] = [
    "dance.step.in_place",
    "dance.step.left",
    "dance.step.right",
    "dance.step.wander",
];
/// The words of `view.mode`'s values, from 0.
pub const VIEWS: [&str; 3] = [
    "dance.view.floating",
    "dance.view.docked",
    "dance.view.hidden",
];
/// `move`'s words: the routine first, then every move.
pub fn move_words() -> Vec<&'static str> {
    std::iter::once("dance.to_the_beat")
        .chain(dancer::Move::ALL.iter().map(|m| m.label_key()))
        .collect()
}
/// `mood`'s words.
pub fn mood_words() -> Vec<&'static str> {
    Mood::ALL.iter().map(|m| m.label_key()).collect()
}

/// The words of `speed`'s values, from 0: ¼×, ½×, 1×, 2×, 4×.
pub const SPEED_WORDS: [&str; 5] = [
    "dance.speed.quarter",
    "dance.speed.half",
    "dance.speed.one",
    "dance.speed.two",
    "dance.speed.four",
];

/// A multiplier as a person writes it: ¼×, ½×, 1×, 2×, 4×.
pub fn speed_text(v: f32) -> String {
    let i = SPEEDS
        .iter()
        .position(|x| *x >= dancer::speed_of(v))
        .unwrap_or(SPEEDS.len() - 1);
    t(SPEED_WORDS[i])
}

/// Write `key`'s value `v` the way a person reads it.
pub fn readout(key: &str, v: f32, b: &mut String) {
    let _ = match key {
        "move" => write!(b, "{}", super::move_text(v)),
        "mood" => write!(
            b,
            "{}",
            t(Mood::from_index(v.round().max(0.0) as usize).label_key())
        ),
        "step" => write!(b, "{}", t(STEPS[(v.round().max(0.0) as usize).min(3)])),
        "speed" => write!(b, "{}", speed_text(v)),
        "size" => write!(b, "{v:.2}\u{d7}"),
        "turn" if v.abs() < 0.02 => write!(b, "{}", t("dance.facing.front")),
        "turn" if v < 0.0 => write!(b, "{} {:.0} %", t("dance.facing.left"), -v * 100.0),
        "turn" => write!(b, "{} {:.0} %", t("dance.facing.right"), v * 100.0),
        "spin" => write!(b, "{v:.0}\u{b0}"),
        "hue" => write!(b, "{v:+.0}\u{b0}"),
        "visible" | "view.top" => {
            write!(b, "{}", t(if v >= 0.5 { "dance.on" } else { "dance.off" }))
        }
        "view.mode" => write!(b, "{}", t(VIEWS[(v.round().max(0.0) as usize).min(2)])),
        _ => write!(b, "{:.0} %", v * 100.0),
    };
}

/// One number as a kit cell with its readout in words and units: range, taper and label
/// from its description, one undo step per drag, learn and modulation as every cell's.
pub fn cell_at(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    track: TrackId,
    spec: &NativeSpec,
    key: &str,
    s: &Style,
    size: KnobSize,
) -> egui::Response {
    let Some(info) = spec.kind.param(key) else {
        return ui.allocate_response(Vec2::ZERO, Sense::hover());
    };
    let mut v = spec.get(key);
    let mut rings = crate::views::inst::frame::rings_for(spec, key, s);
    let mut mod_rings = [(0.0, egui::Color32::TRANSPARENT); crate::views::modulator::MAX_RINGS];
    let w = crate::views::inst::frame::wire(
        app,
        track,
        key,
        &mut v,
        (info.min, info.max, info.logarithmic),
        s,
        &mut mod_rings,
    );
    rings.extend_from_slice(&mod_rings[..w.n]);
    let name = t(info.label_key);
    let pkey = info.key;
    let (r, choice) = Param::new(&mut v, info.min..=info.max, info.default)
        .size(size)
        .log(info.logarithmic)
        .name(&name)
        .style(*s)
        .rings(&rings)
        .offers(w.on, w.automated, w.offer)
        .cell(Src::Key(info.label_key))
        .format(move |v, b| readout(pkey, v, b))
        .show_full(ui);
    crate::views::inst::frame::wired(app, &w, choice, r.rect);
    if r.changed() {
        let v = if info.stepped { v.round() } else { v };
        app.set_inst_value(track, key, v, true);
    }
    if r.drag_stopped() || r.double_clicked() {
        app.end_gesture();
    }
    if spec.kind.modulatable(key)
        && egui::DragAndDrop::has_payload_of_type::<&'static str>(ui.ctx())
        && r.contains_pointer()
    {
        with_brush(ui.painter(), |b| {
            b.rrect_stroke(r.rect, s.r(4.0), s.s(1.5), s.hue)
        });
    }
    if let Some(src) = r
        .dnd_release_payload::<&'static str>()
        .filter(|_| spec.kind.modulatable(key))
    {
        app.connect_modulation(track, *src, key);
    }
    app.inst.dancer.placed.insert(pkey, r.rect);
    r
}

/// A row of cells across `rect`, centred, spread over its width.
#[allow(clippy::too_many_arguments)]
pub fn cells(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    rect: Rect,
    track: TrackId,
    spec: &NativeSpec,
    keys: &[&str],
    s: &Style,
    size: KnobSize,
) {
    let n = keys.len();
    if n == 0 {
        return;
    }
    let sz = cell::size(size) * s.k;
    let spare = rect.width() - sz.x * n as f32;
    let gap = if n > 1 {
        (spare / (n - 1) as f32).clamp(s.s(2.0), s.s(14.0))
    } else {
        0.0
    };
    let total = sz.x * n as f32 + gap * n.saturating_sub(1) as f32;
    let x0 = rect.left() + ((rect.width() - total) * 0.5).max(0.0);
    let y0 = rect.center().y - sz.y * 0.5;
    for (i, key) in keys.iter().enumerate() {
        let r = Rect::from_min_size(Pos2::new(x0 + i as f32 * (sz.x + gap), y0), sz);
        at(ui, r, ("dancer-cell", *key), |ui| {
            cell_at(app, ui, track, spec, key, s, size);
        });
    }
}

/// What a dancer can react to, in the order the field steps through: nothing, the whole
/// song (the master), then every strip but its own (named after the track that feeds it).
pub fn react_sources(project: &Project, track: TrackId) -> Vec<(Option<Listen>, String)> {
    let own = project.track(track).and_then(|t| t.strip);
    let mut v = vec![
        (None, t("dance.react.nothing")),
        (Some(Listen::Master), t("dance.react.song")),
    ];
    for strip in &project.strips {
        if Some(strip.id) == own {
            continue;
        }
        v.push((
            Some(Listen::Strip(strip.id)),
            strip_label(project, strip.id),
        ));
    }
    v
}

/// A strip as a person knows it: the track that feeds it, else its own name.
pub fn strip_label(project: &Project, strip: StripId) -> String {
    project
        .tracks
        .iter()
        .find(|t| t.strip == Some(strip))
        .map(|t| t.name.clone())
        .or_else(|| project.strip(strip).map(|s| s.name.clone()))
        .filter(|n| !n.trim().is_empty())
        .unwrap_or_else(|| t("dance.react.strip"))
}

/// Make the dancer on `track` react to `listen` (or stop): one undo step.
pub fn react_to(app: &mut CantusApp, track: TrackId, listen: Option<Listen>) -> bool {
    let depth = match app
        .project()
        .track(track)
        .and_then(|t| t.instrument.native())
    {
        Some(n) if n.get(dancer::REACT_KEY) > 0.0 => n.get(dancer::REACT_KEY),
        _ => dancer::REACT_DEPTH,
    };
    let id = app.bus.alloc_id();
    match dancer::react_command(app.project(), track, listen, depth, id) {
        Some(cmd) => app.exec(cmd),
        None => false,
    }
}

/// *React to ‹a track›*: a field naming what it listens to, ‹ › stepping through the
/// choices (one undo step each), a click going on to the next.
pub fn react_field(app: &mut CantusApp, ui: &mut egui::Ui, rect: Rect, track: TrackId, s: &Style) {
    let sources = react_sources(app.project(), track);
    let now = dancer::reacting_to(app.project(), track);
    let i = sources.iter().position(|(l, _)| *l == now).unwrap_or(0);
    let name = t("dance.react.title");
    let (resp, c) = crate::kit::widgets::MenuField::new(&sources[i].1)
        .steppers(true)
        .id(ui.id().with(("dance-react", track.0)))
        .name(&name)
        .caps(Caps {
            menu: true,
            ..Caps::NONE
        })
        .style(*s)
        .show_at(ui, rect);
    app.inst.dancer.placed.insert("react.to", rect);
    if resp.clicked() {
        let n = sources.len() as i32;
        let next = (i as i32 + if c.prev { -1 } else { 1 }).rem_euclid(n) as usize;
        react_to(app, track, sources[next].0);
    }
}

/// A push button with a word, for an action (Import character…).
pub fn push(ui: &mut egui::Ui, rect: Rect, s: &Style, label: &str, tip: &str) -> egui::Response {
    push_at(ui, rect, s, label, tip)
}
