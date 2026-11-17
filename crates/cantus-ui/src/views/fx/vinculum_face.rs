//! Vinculum's faceplate (design sheet v1): the strip along its top (the maker's name, the
//! model's own name, the jewel lamps), the meter bridge (the two needle meters and the
//! window on the model's gain element between them) and the model's panel — the model
//! selector and the model's own stepped switches, each a real control.
//!
//! A click on an engraved position chooses it; a click on a knob steps it to the next
//! position (round again after the last); a click on a toggle throws it; each control's menu
//! offers automation and MIDI learn (the kit's `stepped_offer`); the help line says what it
//! does. Every word is measured, so the panel is as wide as its words in each language.

use super::super::controls;
use super::super::frame;
use super::super::frame::tr;
use super::super::host::{FxHost, KeySource, KeyTarget};
use super::art::{self, c};
use super::display::{Parts, Reading, View};
use super::meter::{self, Mark, Scale};
use super::parts;
use crate::kit::color::{mix, with_alpha};
use crate::kit::hint::{self, Caps};
use crate::kit::skin::racks as pal;
use crate::kit::text::{self as ktext, Role, Src, Text};
use crate::kit::{Brush, Style};
use cantus_core::EffectSpec;
use cantus_dsp::math::{cos, sin};
use egui::{pos2, vec2, Align2, Color32, Pos2, Rect, Sense, Vec2};

/// The model in force.
pub fn model(spec: &EffectSpec) -> usize {
    spec.get("model").round().clamp(0.0, 2.0) as usize
}

/// A control of the panel.
#[derive(Copy, Clone, Debug)]
enum Kind {
    /// A stepped knob: its positions' engraved words (locale keys).
    Rotary(&'static [&'static str]),
    /// A bat toggle: down, up.
    Toggle([&'static str; 2]),
}

#[derive(Copy, Clone, Debug)]
struct Ctl {
    key: &'static str,
    caption: &'static str,
    /// The value's words, `{values}.{i}`.
    values: &'static str,
    tip: &'static str,
    kind: Kind,
}

const MODELS: &[&str] = &[
    "fx.vinculum.model.0",
    "fx.vinculum.model.1",
    "fx.vinculum.model.2",
];
const RATIOS: &[&str] = &[
    "fx.vinculum.ratio.0",
    "fx.vinculum.ratio.1",
    "fx.vinculum.ratio.2",
];
const ATTACKS: &[&str] = &[
    "vinculum.pos.att.0",
    "vinculum.pos.att.1",
    "vinculum.pos.att.2",
    "vinculum.pos.att.3",
    "vinculum.pos.att.4",
    "vinculum.pos.att.5",
];
const RELEASES: &[&str] = &[
    "vinculum.pos.rel.0",
    "vinculum.pos.rel.1",
    "vinculum.pos.rel.2",
    "vinculum.pos.rel.3",
    "fx.vinculum.rel.4",
];
const TIMES: &[&str] = &[
    "vinculum.pos.time.0",
    "vinculum.pos.time.1",
    "vinculum.pos.time.2",
    "vinculum.pos.time.3",
    "vinculum.pos.time.4",
    "vinculum.pos.time.5",
];

const fn rotary(
    key: &'static str,
    values: &'static str,
    tip: &'static str,
    labels: &'static [&'static str],
) -> Ctl {
    Ctl {
        key,
        caption: values,
        values,
        tip,
        kind: Kind::Rotary(labels),
    }
}

const MODEL: Ctl = rotary("model", "fx.vinculum.model", "vinculum.tip.model", MODELS);
const RATIO: Ctl = rotary("ratio", "fx.vinculum.ratio", "vinculum.tip.ratio", RATIOS);
const ATTACK: Ctl = rotary("att", "fx.vinculum.att", "vinculum.tip.att", ATTACKS);
const RELEASE: Ctl = rotary("rel", "fx.vinculum.rel", "vinculum.tip.rel", RELEASES);
const TIME: Ctl = rotary("time", "fx.vinculum.time", "vinculum.tip.time", TIMES);
const CHARACTER: Ctl = Ctl {
    key: "char",
    caption: "fx.vinculum.char",
    values: "fx.vinculum.char",
    tip: "vinculum.tip.char",
    kind: Kind::Toggle(["fx.vinculum.char.0", "fx.vinculum.char.1"]),
};

/// Each model's panel, left to right.
fn controls_of(model: usize) -> &'static [Ctl] {
    match model {
        1 => &[MODEL, CHARACTER],
        2 => &[MODEL, TIME, CHARACTER],
        _ => &[MODEL, RATIO, ATTACK, RELEASE],
    }
}

/// The knob's radius and the gap to its engraved positions, at 100 %.
const KNOB: f32 = 33.0;
const RING: f32 = 10.0;
/// The toggle's size, at 100 %.
const BAT: f32 = 25.0;

/// The angle of position `i` of `n` (radians, 0 up, clockwise).
fn angle(i: usize, n: usize) -> f32 {
    let step = match n {
        0..=2 => 80.0f32,
        3 => 62.0,
        _ => 50.0,
    }
    .to_radians();
    (i as f32 - (n as f32 - 1.0) * 0.5) * step
}

fn dir(a: f32) -> Vec2 {
    vec2(sin(a), -cos(a))
}

/// Where an engraved position's word anchors round its knob.
fn label_align(a: f32) -> Align2 {
    let d = dir(a);
    let h = if d.x > 0.35 {
        egui::Align::Min
    } else if d.x < -0.35 {
        egui::Align::Max
    } else {
        egui::Align::Center
    };
    let v = if d.y < -0.35 {
        egui::Align::Max
    } else if d.y > 0.35 {
        egui::Align::Min
    } else {
        egui::Align::Center
    };
    Align2([h, v])
}

fn micro(ctx: &egui::Context, key: &str, k: f32) -> Vec2 {
    ktext::measure(ctx, &Text::key(key, Role::Micro, Color32::TRANSPARENT), k)
}

/// One control laid out round the origin: its words' places, its hit areas, its extent.
#[derive(Copy, Clone, Debug)]
struct Bay {
    ctl: Ctl,
    /// The extent round the knob's centre (relative), its width.
    rel: Rect,
    centre: Pos2,
    /// The positions' words (relative to the centre), one per position (at most six).
    labels: [Rect; 6],
    n: usize,
}

impl Bay {
    fn lay(ctx: &egui::Context, k: f32, ctl: Ctl) -> Bay {
        let mut labels = [Rect::NOTHING; 6];
        let (mut ext, n) = match ctl.kind {
            Kind::Rotary(words) => {
                let r = (KNOB + RING) * k;
                let mut ext = Rect::from_center_size(Pos2::ZERO, Vec2::splat(2.0 * KNOB * k));
                for (i, w) in words.iter().enumerate().take(6) {
                    let a = angle(i, words.len());
                    let at = Pos2::ZERO + dir(a) * r;
                    let rect = label_align(a).anchor_size(at, micro(ctx, w, k));
                    labels[i] = rect;
                    ext = ext.union(rect);
                }
                (ext, words.len())
            }
            Kind::Toggle(words) => {
                let h = BAT * k;
                let up = micro(ctx, words[1], k);
                let down = micro(ctx, words[0], k);
                labels[1] = Align2::CENTER_BOTTOM.anchor_size(pos2(0.0, -h * 1.45), up);
                labels[0] = Align2::CENTER_TOP.anchor_size(pos2(0.0, h * 1.45), down);
                let ext = Rect::from_center_size(Pos2::ZERO, vec2(h * 1.4, h * 2.6))
                    .union(labels[0])
                    .union(labels[1]);
                (ext, 2)
            }
        };
        // The caption and the value's words under it.
        let cap = micro(ctx, ctl.caption, k);
        let widest = (0..n)
            .map(|i| value_size(ctx, &ctl, i, k).x)
            .fold(cap.x, f32::max);
        let foot = ext.bottom() + 8.0 * k + cap.y + 4.0 * k + value_size(ctx, &ctl, 0, k).y;
        ext = ext.union(Rect::from_min_max(
            pos2(-widest * 0.5, ext.bottom()),
            pos2(widest * 0.5, foot),
        ));
        Bay {
            ctl,
            rel: ext.expand2(vec2(6.0 * k, 0.0)),
            centre: Pos2::ZERO,
            labels,
            n,
        }
    }

    fn width(&self) -> f32 {
        self.rel.width()
    }

    fn label(&self, i: usize) -> Rect {
        self.labels[i].translate(self.centre.to_vec2())
    }

    fn knob(&self, k: f32) -> Rect {
        match self.ctl.kind {
            Kind::Rotary(_) => Rect::from_center_size(self.centre, Vec2::splat(2.0 * KNOB * k)),
            Kind::Toggle(_) => {
                Rect::from_center_size(self.centre, vec2(BAT * k * 1.4, BAT * k * 2.6))
            }
        }
    }
}

/// The value's words of position `i`: "10 ms", "Auto", "2 · 0.2 ms, 0.8 s".
fn value_key(ctl: &Ctl, i: usize) -> String {
    format!("{}.{i}", ctl.values)
}

fn value_size(ctx: &egui::Context, ctl: &Ctl, i: usize, k: f32) -> Vec2 {
    let key = value_key(ctl, i);
    ktext::measure(
        ctx,
        &Text::key(&key, Role::Value, Color32::TRANSPARENT).caps(false),
        k,
    )
}

/// The model's panel: its controls laid in a row.
pub struct Panel {
    bays: [Option<Bay>; 4],
    /// The width the panel needs.
    pub width: f32,
    pub rect: Rect,
}

impl Panel {
    /// The model's controls measured in the interface's language.
    pub fn lay(ctx: &egui::Context, s: &Style, spec: &EffectSpec) -> Panel {
        let k = s.k;
        let mut bays = [None; 4];
        let mut width = 0.0;
        for (i, ctl) in controls_of(model(spec)).iter().enumerate() {
            let bay = Bay::lay(ctx, k, *ctl);
            width += bay.width() + 18.0 * k;
            bays[i] = Some(bay);
        }
        Panel {
            bays,
            width: width + 26.0 * k,
            rect: Rect::NOTHING,
        }
    }

    /// The controls spread across `r`, their knobs on one line.
    pub fn place(mut self, r: Rect) -> Panel {
        let used: f32 = self.bays.iter().flatten().map(Bay::width).sum();
        let n = self.bays.iter().flatten().count().max(1) as f32;
        let gap = ((r.width() - used) / (n + 1.0)).max(0.0);
        let top = self
            .bays
            .iter()
            .flatten()
            .map(|b| -b.rel.top())
            .fold(0.0, f32::max);
        let bottom = self
            .bays
            .iter()
            .flatten()
            .map(|b| b.rel.bottom())
            .fold(0.0, f32::max);
        let cy = r.center().y + (top - bottom) * 0.5;
        let mut x = r.left() + gap;
        for bay in self.bays.iter_mut().flatten() {
            bay.centre = pos2(x - bay.rel.left(), cy);
            x += bay.width() + gap;
        }
        self.rect = r;
        self
    }
}

/// A switch of the panel where the pointer finds it: its key, its knob (or bat), the words
/// of its positions.
#[derive(Copy, Clone, Debug)]
pub struct Switch {
    pub key: &'static str,
    pub knob: Rect,
    pub positions: [Rect; 6],
    pub n: usize,
}

impl Panel {
    /// The panel's switches as placed, left to right.
    pub fn switches(&self, k: f32) -> [Option<Switch>; 4] {
        self.bays.map(|b| {
            b.map(|bay| Switch {
                key: bay.ctl.key,
                knob: bay.knob(k),
                positions: std::array::from_fn(|i| {
                    if i < bay.n {
                        bay.label(i)
                    } else {
                        Rect::NOTHING
                    }
                }),
                n: bay.n,
            })
        })
    }
}

/// What the pointer does to each control this frame.
#[derive(Copy, Clone, Debug, Default)]
pub struct Press {
    hover: bool,
}

/// The panel's controls answer the pointer (see the module's words).
pub fn edit(host: &mut dyn FxHost, ui: &mut egui::Ui, s: &Style, panel: &Panel) -> [Press; 4] {
    let k = s.k;
    let mut out = [Press::default(); 4];
    for (j, bay) in panel.bays.iter().enumerate() {
        let Some(bay) = bay else { continue };
        let ctl = bay.ctl;
        let now = host.spec().get(ctl.key).round().max(0.0) as usize;
        let name = tr(ctl.caption);
        let id = ui.id().with(("vinculum-panel", ctl.key));
        let knob = ui.interact(bay.knob(k), id, Sense::click());
        let learnt = controls::stepped_offer(ui, s, host, ctl.key, name, &knob);
        let mut hover = knob.hovered();
        if knob.clicked() && !learnt {
            host.set(ctl.key, ((now + 1) % bay.n) as f32, None);
        }
        for i in 0..bay.n {
            let r = bay.label(i).expand(2.0 * k);
            let resp = ui.interact(r, id.with(i), Sense::click());
            let learnt = controls::stepped_offer(ui, s, host, ctl.key, name, &resp);
            if resp.clicked() && !learnt && i != now {
                host.set(ctl.key, i as f32, None);
            }
            hover |= resp.hovered();
        }
        if hover {
            hint::set_text(ui.ctx(), name, tr(ctl.tip), Caps::NONE);
        }
        out[j] = Press { hover };
    }
    out
}

/// The model's panel: each control, its engraved positions, its caption and its value.
pub fn panel(b: &mut Brush, s: &Style, panel: &Panel, press: &[Press; 4], spec: &EffectSpec) {
    let k = s.k;
    // A sunk sub-panel behind the switches: an engraved frame round them.
    let r = panel.rect;
    let p = art::plate(s);
    b.rrect(
        r,
        6.0 * k,
        crate::kit::Fill::Solid(with_alpha(c(p.foot), 0.18)),
    );
    // The sub-panel's four screws.
    for (j, q) in [
        r.left_top(),
        r.right_top(),
        r.left_bottom(),
        r.right_bottom(),
    ]
    .into_iter()
    .enumerate()
    {
        let inset = vec2(
            if j % 2 == 0 { 12.0 } else { -12.0 },
            if j < 2 { 14.0 } else { -14.0 },
        ) * k;
        art::screw(b, k, q + inset, 4.0 * k, art::hash(j as u32 + 40));
    }
    art::engrave_line(
        b,
        s,
        r.left_top() + vec2(8.0, 0.0) * k,
        r.right_top() - vec2(8.0, 0.0) * k,
        0.8 * k,
    );
    art::engrave_line(
        b,
        s,
        r.left_bottom() + vec2(8.0, 0.0) * k,
        r.right_bottom() - vec2(8.0, 0.0) * k,
        0.8 * k,
    );
    let leg = art::legend(s);
    // An engraved line between the model selector and the model's own switches.
    if let [Some(a), Some(z), ..] = panel.bays {
        let x = 0.5 * (a.centre.x + a.rel.right() + z.centre.x + z.rel.left());
        art::engrave_line(
            b,
            s,
            pos2(x, r.top() + 22.0 * k),
            pos2(x, r.bottom() - 22.0 * k),
            0.8 * k,
        );
    }
    for (bay, pr) in panel.bays.iter().zip(press) {
        let Some(bay) = bay else { continue };
        let ctl = bay.ctl;
        let v = spec.get(ctl.key).round().clamp(0.0, (bay.n - 1) as f32) as usize;
        match ctl.kind {
            Kind::Rotary(words) => {
                // The engraved ring: a tick at each position, the word beyond it.
                for (i, w) in words.iter().enumerate().take(bay.n) {
                    let a = angle(i, bay.n);
                    let d = dir(a);
                    art::engrave_line(
                        b,
                        s,
                        bay.centre + d * (KNOB + 1.5) * k,
                        bay.centre + d * (KNOB + 6.0) * k,
                        if i == v { 1.6 * k } else { 0.9 * k },
                    );
                    let at = bay.label(i);
                    art::engrave(
                        b,
                        s,
                        at.center(),
                        Align2::CENTER_CENTER,
                        &Text::key(w, Role::Micro, Color32::TRANSPARENT),
                        1.0,
                    );
                }
                art::knob(b, s, bay.centre, KNOB * k, angle(v, bay.n), pr.hover);
            }
            Kind::Toggle(words) => {
                for (i, word) in words.iter().enumerate() {
                    art::engrave(
                        b,
                        s,
                        bay.label(i).center(),
                        Align2::CENTER_CENTER,
                        &Text::key(word, Role::Micro, Color32::TRANSPARENT),
                        1.0,
                    );
                }
                art::toggle(b, s, bay.centre, BAT * k, v == 1, pr.hover);
            }
        }
        // The caption, and the value as it stands written under it in the plate's paint.
        let foot = match ctl.kind {
            Kind::Rotary(_) => bay
                .labels
                .iter()
                .take(bay.n)
                .map(|r| r.bottom())
                .fold(KNOB * k, f32::max),
            Kind::Toggle(_) => bay.labels[0].bottom(),
        };
        let y = bay.centre.y + foot + 8.0 * k;
        let cap = art::engrave_key(b, s, pos2(bay.centre.x, y), Align2::CENTER_TOP, ctl.caption);
        let key = value_key(&ctl, v);
        super::super::canvas::put(
            b,
            pos2(bay.centre.x, cap.bottom() + 4.0 * k),
            Align2::CENTER_TOP,
            &Text::key(&key, Role::Value, mix(leg, c(p.face), 0.15)).caps(false),
            k,
            None,
        );
    }
}

/// A jewel lamp on the strip: its word, its lens, whether it is lit, where it stands.
#[derive(Copy, Clone, Debug)]
pub struct Lamp {
    /// Its engraved word's locale key (`vinculum.d.key`, `.auto`, `.limit`, `.colour`).
    pub word: &'static str,
    lens: Color32,
    pub on: bool,
    /// The jewel's centre and its radius; the word's rect.
    jewel: Pos2,
    rad: f32,
    text: Rect,
}

impl Lamp {
    /// Where the pointer reaches it: the jewel and its word.
    pub fn hit(&self) -> Rect {
        self.text
            .union(Rect::from_center_size(
                self.jewel,
                Vec2::splat(self.rad * 2.0),
            ))
            .expand(self.rad * 0.6)
    }
}

/// The lamps shown for the model, right to left as they stand (the key, the Auto release,
/// the limit, the valve colour — each only where the model has it), measured in the
/// interface's language.
pub fn lamps(
    ctx: &egui::Context,
    s: &Style,
    r: Rect,
    spec: &EffectSpec,
    keyed: bool,
) -> [Option<Lamp>; 4] {
    let k = s.k;
    let m = model(spec);
    let limit = spec.get("char") >= 0.5;
    let all: [(&'static str, Color32, bool, bool); 4] = [
        ("vinculum.d.key", c(pal::LENS_AMBER), true, keyed),
        (
            "vinculum.d.auto",
            c(pal::LENS_GREEN),
            m == 0,
            spec.get("rel") >= 3.5,
        ),
        ("vinculum.d.limit", c(pal::LENS_RED), m != 0, limit),
        (
            "vinculum.d.colour",
            c(pal::LENS_ORANGE),
            m == 2,
            spec.get("colour") > 0.0,
        ),
    ];
    let mut out = [None; 4];
    let mut x = r.right();
    let rad = 7.0 * k;
    for (j, (word, lens, shown, on)) in all.into_iter().enumerate().rev() {
        if !shown {
            continue;
        }
        let text = Align2::RIGHT_CENTER.anchor_size(pos2(x, r.center().y), micro(ctx, word, k));
        let jewel = pos2(text.left() - 14.0 * k, r.center().y);
        out[j] = Some(Lamp {
            word,
            lens,
            on,
            jewel,
            rad,
            text,
        });
        x = text.left() - 14.0 * k - rad * 1.5 - 22.0 * k;
    }
    out
}

/// What the lamps remember between frames.
#[derive(Copy, Clone, Debug, Default)]
pub struct LampMemory {
    /// The VCA's release position before Auto was lit from its lamp.
    pub manual_rel: Option<f32>,
    /// The key's list while it is open under its lamp: the list's rect and its rows.
    pub menu: Option<(Rect, usize)>,
}

/// The lamps answer the pointer, each change one undo step: the key's lamp opens the key's
/// list (its own input, or a strip of the desk) under it — in a plug-in it throws the
/// sidechain switch; the Auto lamp lights Auto or gives back the release it had; the limit
/// lamp throws Compress or Limit. The colour's lamp only shows (the colour is a knob).
pub fn lamp_edit(
    host: &mut dyn FxHost,
    ui: &mut egui::Ui,
    s: &Style,
    lamps: &[Option<Lamp>; 4],
    key: Option<&KeySource>,
    mem: &mut LampMemory,
) {
    let k = s.k;
    let base = ui.id().with("vinculum-lamps");
    mem.menu = None;
    for lamp in lamps.iter().flatten() {
        let id = base.with(lamp.word);
        let hit = lamp.hit();
        let resp = ui.interact(hit, id, Sense::click());
        let tip = match lamp.word {
            "vinculum.d.key" => "vinculum.tip.key_lamp",
            "vinculum.d.auto" => "vinculum.tip.auto_lamp",
            "vinculum.d.limit" => "vinculum.tip.char",
            _ => "vinculum.tip.colour_lamp",
        };
        if resp.hovered() {
            hint::set_text(ui.ctx(), tr(lamp.word), tr(tip), Caps::NONE);
        }
        match lamp.word {
            "vinculum.d.key" => match key {
                Some(KeySource::Strips {
                    current, options, ..
                }) => {
                    let open_id = id.with("open");
                    let mut open = ui.data(|d| d.get_temp::<bool>(open_id)).unwrap_or(false);
                    if resp.clicked() {
                        open = !open;
                        ui.data_mut(|d| d.insert_temp(open_id, open));
                    }
                    if open {
                        let mut items = vec![frame::radio(
                            Src::Key("vinculum.key_own"),
                            current.is_none(),
                        )];
                        items.extend(options.iter().map(|(sid, text)| {
                            frame::radio(Src::Str(text), *current == Some(*sid))
                        }));
                        let w = 220.0;
                        let at = pos2(hit.right() - w * k, hit.bottom() + 2.0 * k);
                        let list = Rect::from_min_size(
                            at,
                            vec2(w * k, crate::kit::menu::height(&items, s.d(1.0))),
                        );
                        mem.menu = Some((list, items.len()));
                        let anchor = controls::Anchor { at, field: hit };
                        // The list closes itself on a choice, a click elsewhere or Esc.
                        if let Some(i) = controls::list(ui, s, id, anchor, w, &items) {
                            let target = if i == 0 {
                                None
                            } else {
                                options.get(i - 1).map(|o| o.0)
                            };
                            drop(items);
                            host.set_key_source(KeyTarget::Strip(target));
                        }
                    }
                }
                Some(KeySource::Switch(on)) if resp.clicked() => {
                    host.set_key_source(KeyTarget::Switch(!*on));
                }
                _ => {}
            },
            "vinculum.d.auto" => {
                if resp.clicked() {
                    let now = host.spec().get("rel");
                    if now >= 3.5 {
                        let back = mem.manual_rel.take().unwrap_or(1.0);
                        host.set("rel", back, None);
                    } else {
                        mem.manual_rel = Some(now);
                        host.set("rel", cantus_core::fx::vinculum::REL_AUTO, None);
                    }
                }
            }
            "vinculum.d.limit" if resp.clicked() => {
                let now = host.spec().get("char") >= 0.5;
                host.set("char", if now { 0.0 } else { 1.0 }, None);
            }
            _ => {}
        }
    }
}

/// The names engraved along the faceplate's top, and the jewel lamps at its right.
pub fn strip(
    b: &mut Brush,
    s: &Style,
    r: Rect,
    spec: &EffectSpec,
    now: &Reading,
    lamps: &[Option<Lamp>; 4],
) {
    let k = s.k;
    let m = model(spec);
    let ring = 9.0 * k;
    let e = pos2(r.left() + ring + 2.0 * k, r.center().y);
    art::emblem(b, s, e, ring);
    let name = art::engrave(
        b,
        s,
        pos2(e.x + ring * 2.4, r.center().y),
        Align2::LEFT_CENTER,
        &Text::key(
            "picker.dev.vinculum.name",
            Role::Title,
            Color32::TRANSPARENT,
        )
        .caps(true),
        1.45,
    );
    let rule_x = name.right() + 14.0 * k;
    art::engrave_line(
        b,
        s,
        pos2(rule_x, r.top() + 10.0 * k),
        pos2(rule_x, r.bottom() - 10.0 * k),
        1.0 * k,
    );
    let face_key = [
        "vinculum.face.vca",
        "vinculum.face.opto",
        "vinculum.face.varimu",
    ][m];
    art::engrave(
        b,
        s,
        pos2(rule_x + 14.0 * k, r.center().y),
        Align2::LEFT_CENTER,
        &Text::key(face_key, Role::Label, Color32::TRANSPARENT).caps(true),
        1.0,
    );
    // The lamps, from the right: the valve colour, the limit, the Auto release, the key.
    for lamp in lamps.iter().flatten() {
        art::engrave_key(
            b,
            s,
            pos2(lamp.text.right(), lamp.text.center().y),
            Align2::RIGHT_CENTER,
            lamp.word,
        );
        let lit = if lamp.on {
            if now.seen || lamp.word != "vinculum.d.key" {
                1.0
            } else {
                0.8
            }
        } else {
            0.0
        };
        art::jewel(b, k, lamp.jewel, lamp.rad, lamp.lens, lit);
    }
}

/// The meters' scales.
fn gr_scale() -> Scale {
    static MARKS: std::sync::OnceLock<Vec<Mark>> = std::sync::OnceLock::new();
    let marks = MARKS.get_or_init(|| {
        [
            (0.0f32, Some("0")),
            (1.0, Some("1")),
            (2.0, Some("2")),
            (3.0, Some("3")),
            (4.0, Some("4")),
            (5.0, None),
            (6.0, Some("6")),
            (8.0, None),
            (10.0, Some("10")),
            (12.5, None),
            (15.0, Some("15")),
            (20.0, Some("20")),
        ]
        .into_iter()
        .map(|(db, label)| Mark {
            at: meter::gr_pos(db),
            label,
        })
        .collect()
    });
    Scale {
        marks: marks.as_slice(),
        red: (0.0, meter::gr_pos(10.0)),
        title: "vinculum.d.gr",
        unit: "dB",
    }
}

fn out_scale() -> Scale {
    static MARKS: std::sync::OnceLock<Vec<Mark>> = std::sync::OnceLock::new();
    let marks = MARKS.get_or_init(|| {
        [
            (-40.0f32, Some("40")),
            (-30.0, Some("30")),
            (-24.0, None),
            (-20.0, Some("20")),
            (-15.0, None),
            (-12.0, Some("12")),
            (-9.0, None),
            (-6.0, Some("6")),
            (-3.0, Some("3")),
            (0.0, Some("0")),
        ]
        .into_iter()
        .map(|(db, label)| Mark {
            at: meter::out_pos(db),
            label,
        })
        .collect()
    });
    Scale {
        marks: marks.as_slice(),
        red: (meter::out_pos(-3.0), 1.0),
        title: "vinculum.d.output",
        unit: "dBFS",
    }
}

/// The meter bridge: the reduction's needle, the window on the gain element, the
/// output's needle.
pub fn bridge(b: &mut Brush, s: &Style, p: &Parts, spec: &EffectSpec, view: &View, now: &Reading) {
    let k = s.k;
    let lamp = s.skin.accent_in(true).unwrap_or_else(|| c(pal::LENS_AMBER));
    let gr_scale = gr_scale();
    let out_scale = out_scale();
    let gr_win = art::window(b, s, p.gr_meter, 5.0 * k);
    let peak = (now.seen || view.peak() > 0.05).then(|| meter::gr_pos(view.peak()));
    meter::meter(
        b,
        s,
        gr_win,
        lamp,
        &gr_scale,
        meter::gr_pos(view.needle_gr),
        peak,
        1.0,
    );
    let out_win = art::window(b, s, p.out_meter, 5.0 * k);
    meter::meter(
        b,
        s,
        out_win,
        lamp,
        &out_scale,
        meter::out_pos(view.needle_out),
        None,
        1.0,
    );
    let el = art::window(b, s, p.element, 6.0 * k);
    let gr = view.needle_gr;
    match model(spec) {
        1 => parts::opto(
            b,
            s,
            el,
            (gr / 7.0).clamp(0.0, 1.0),
            spec.get("char") >= 0.5,
        ),
        2 => parts::valves(
            b,
            s,
            el,
            0.55 + 0.45 * spec.get("colour").clamp(0.0, 1.0),
            (gr / 14.0).clamp(0.0, 0.8),
        ),
        _ => {
            let led = s.skin.accent2_in(true).unwrap_or_else(|| c(pal::LENS_RED));
            parts::vca(b, s, el, (gr / 8.0).clamp(0.0, 1.0), led)
        }
    }
    art::glass(b, el, k);
}
