//! The Edit view: every control, in the order the signal flows — the three oscillators;
//! the mixer, the high-pass and the amplifier; the filter; the two envelopes; the LFOs and
//! the wheel; the voices, the chorus and the output; the sixteen cards. One kit card per
//! section in its function hue, cells inside, a small display where it helps (a cycle, a
//! shape), the switches as segmented controls and LED buttons. Taller than the window: the
//! shell scrolls it.

use super::*;
use crate::kit::Brush;
use egui::{Sense, Vec2};

pub const MODELS: [&str; 5] = [
    "calor.model.ladder",
    "calor.model.cascade",
    "calor.model.diode",
    "calor.model.svf",
    "calor.model.sk",
];
pub const SLOPES: [&str; 4] = [
    "calor.slope.6",
    "calor.slope.12",
    "calor.slope.18",
    "calor.slope.24",
];
pub const QUALITIES: [&str; 3] = [
    "calor.quality.draft",
    "calor.quality.good",
    "calor.quality.best",
];
pub const CHORUS: [&str; 4] = [
    "calor.chorus.off",
    "calor.chorus.one",
    "calor.chorus.two",
    "calor.chorus.both",
];
pub const VCF_REV: [&str; 3] = [
    "calor.vcf.rev.clean",
    "calor.vcf.rev.rough",
    "calor.vcf.rev.pushed",
];
pub const MID: [&str; 2] = ["calor.mid.notch", "calor.mid.band"];
const MODES: [&str; 3] = ["calor.mode.poly", "calor.mode.mono", "calor.mode.legato"];
const ASSIGN: [&str; 3] = [
    "calor.assign.rotate",
    "calor.assign.reuse",
    "calor.assign.lowest",
];
const PRIORITY: [&str; 3] = [
    "calor.priority.last",
    "calor.priority.low",
    "calor.priority.high",
];
const GLIDE_MODE: [&str; 2] = ["calor.glide.by_time", "calor.glide.by_rate"];
const GLIDE_CURVE: [&str; 2] = ["calor.glide.rc", "calor.glide.linear"];
const REVS: [&str; 3] = [
    "calor.rev.ideal",
    "calor.rev.vintage_a",
    "calor.rev.vintage_b",
];
const SUBS: [&str; 3] = ["calor.sub.sq1", "calor.sub.sq2", "calor.sub.pulse1"];
const NOISE: [&str; 2] = ["calor.noise.white", "calor.noise.pink"];
const HPF_POS: [&str; 3] = ["calor.hpf.pre", "calor.hpf.post", "calor.hpf.boost"];
const VCA_SRC: [&str; 2] = ["calor.vca.env2", "calor.vca.gate"];
const ENV_MODELS: [&str; 4] = [
    "calor.env.model.rc",
    "calor.env.model.ads",
    "calor.env.model.digital",
    "calor.env.model.oneshot",
];
const RETRIG: [&str; 2] = ["calor.retrig.current", "calor.retrig.reset"];
pub const LFO_SHAPES: [&str; 7] = [
    "calor.lfo.tri",
    "calor.lfo.sine",
    "calor.lfo.saw_up",
    "calor.lfo.saw_down",
    "calor.lfo.square",
    "calor.lfo.sh",
    "calor.lfo.smooth",
];
const LFO_RESTART: [&str; 3] = [
    "calor.restart.free",
    "calor.restart.gate",
    "calor.restart.random",
];
const PHASE: [&str; 2] = ["calor.phase.free", "calor.phase.reset"];

/// What a card's title bar holds at its right.
#[derive(Copy, Clone)]
enum Bar {
    None,
    /// A segmented control: key, labels, first value, width.
    Choice(&'static str, &'static [&'static str], f32, f32),
    /// A menu field with ‹ ›: key, labels, width.
    Stepper(&'static str, &'static [&'static str], f32),
}

impl Bar {
    fn width(self) -> f32 {
        match self {
            Bar::None => 0.0,
            Bar::Choice(_, _, _, w) | Bar::Stepper(_, _, w) => w,
        }
    }
}

/// What a card draws in its display and slots.
#[derive(Copy, Clone)]
enum Extra {
    None,
    Osc(usize),
    Filter,
    Env(usize),
    Lfo(usize),
    Output,
    Cards,
}

/// One card of the view.
struct Card<'a> {
    title: &'static str,
    f: Function,
    display: Option<f32>,
    bar: Bar,
    items: &'a [Item<'a>],
    frac: f32,
    extra: Extra,
}

fn row_height(cards: &[Card<'_>], width: f32, s: &Style) -> f32 {
    let fracs: Vec<f32> = cards.iter().map(|c| c.frac).collect();
    parts::widths(width, &fracs, s)
        .iter()
        .zip(cards)
        .map(|(w, c)| parts::card_height(c.items, *w, c.display, s))
        .fold(0.0, f32::max)
}

#[allow(clippy::too_many_arguments)]
fn draw_row(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    track: TrackId,
    spec: &NativeSpec,
    s: &Style,
    tel: &cantus_engine::Telemetry,
    area: Rect,
    y: f32,
    h: f32,
    cards: &[Card<'_>],
) {
    let fracs: Vec<f32> = cards.iter().map(|c| c.frac).collect();
    let rects = parts::row(area, y, h, &fracs, s);
    for (c, r) in cards.iter().zip(rects) {
        let st = s.function(c.f);
        let Some((cr, slots)) = parts::section(
            app,
            ui,
            track,
            spec,
            &st,
            r,
            Src::Key(c.title),
            c.display,
            c.bar.width(),
            c.items,
        ) else {
            continue;
        };
        match c.bar {
            Bar::None => {}
            Bar::Choice(key, labels, first, w) => {
                let r = parts::bar(&cr, &st, w);
                parts::choice_at(app, ui, track, spec, key, labels, first, r, &st);
            }
            Bar::Stepper(key, labels, w) => {
                let r = parts::bar(&cr, &st, w);
                parts::stepper_at(app, ui, track, spec, key, labels, r, &st);
            }
        }
        extra(app, ui, track, spec, &st, tel, c.extra, cr.display, &slots);
    }
}

#[allow(clippy::too_many_arguments)]
fn extra(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    track: TrackId,
    spec: &NativeSpec,
    s: &Style,
    tel: &cantus_engine::Telemetry,
    what: Extra,
    display: Option<Rect>,
    slots: &[Rect],
) {
    // The pictures in the display's look (round 32): its ink, its glass over them.
    let sd = &crate::views::inst::face::on_display(s);
    let paint = |ui: &egui::Ui, d: Rect, f: &dyn Fn(&mut Brush)| {
        crate::kit::with_brush(&ui.painter_at(d), |b| {
            f(b);
            crate::views::inst::face::cover(b, d, sd);
        });
    };
    match (what, display) {
        (Extra::Osc(n), Some(d)) => {
            let k = |f: &str| spec.get(&format!("osc{n}.{f}"));
            let levels = (k("saw"), k("pulse"), k("tri"), k("pw"));
            paint(ui, d, &|b| {
                draw::cycle(b, d, sd, levels);
            });
        }
        (Extra::Filter, Some(d)) => filter::display(app, ui, track, spec, tel, d, sd),
        (Extra::Env(e), Some(d)) => {
            let k = |f: &str| spec.get(&format!("env{e}.{f}"));
            let times = (k("a"), k("d"), k("s"), k("r"));
            let level = if e == 1 {
                tel[tel::ENV1]
            } else {
                tel[tel::ENV2]
            };
            paint(ui, d, &|b| draw::envelope(b, d, sd, times, level));
        }
        (Extra::Lfo(l), Some(d)) => {
            let k = |f: &str| spec.get(&format!("lfo{l}.{f}"));
            let (shape, uni, ph) = (
                k("shape").round() as usize,
                k("unipolar") >= 0.5,
                k("phase"),
            );
            paint(ui, d, &|b| draw::lfo(b, d, sd, shape, uni, ph));
        }
        (Extra::Output, _) => {
            if let Some(r) = slots.first() {
                slot_caption(ui, *r, &cpu_text(tel), s);
            }
        }
        (Extra::Cards, Some(d)) => {
            cards::grid(app, ui, track, spec, tel, d, sd, true);
            if let [words, tools, ..] = slots[..] {
                cards::condition_words(ui, words, spec, s);
                cards::tools(app, ui, track, spec, tools, s);
            }
        }
        _ => {}
    }
}

const fn cell(key: &'static str) -> Item<'static> {
    Item::Cell(key, KnobSize::M, Fmt::Unit)
}

const fn cell_as(key: &'static str, fmt: Fmt) -> Item<'static> {
    Item::Cell(key, KnobSize::M, fmt)
}

const fn choice(key: &'static str, labels: &'static [&'static str], w: f32) -> Item<'static> {
    Item::Choice {
        key,
        labels,
        first: 0.0,
        w,
        caption: true,
    }
}

/// An oscillator's cells: tuning, then its three levels and its width.
fn osc_items(n: usize) -> Vec<Item<'static>> {
    const KEYS: [[&str; 8]; 3] = [
        [
            "osc1.octave",
            "osc1.semi",
            "osc1.fine",
            "osc1.pw",
            "osc1.saw",
            "osc1.pulse",
            "osc1.tri",
            "osc1.pwm",
        ],
        [
            "osc2.octave",
            "osc2.semi",
            "osc2.fine",
            "osc2.pw",
            "osc2.saw",
            "osc2.pulse",
            "osc2.tri",
            "osc2.pwm",
        ],
        [
            "osc3.octave",
            "osc3.semi",
            "osc3.fine",
            "osc3.pw",
            "osc3.saw",
            "osc3.pulse",
            "osc3.tri",
            "osc3.pwm",
        ],
    ];
    let k = &KEYS[n - 1];
    let mut v = vec![
        cell_as(k[0], Fmt::Signed("oct")),
        cell_as(k[1], Fmt::Signed("st")),
        cell(k[2]),
        cell(k[3]),
        Item::Break,
        cell(k[4]),
        cell(k[5]),
        cell(k[6]),
        cell(k[7]),
    ];
    match n {
        2 => v.extend([
            Item::Break,
            Item::Toggle("osc2.sync", "calor.osc.sync", 96.0),
        ]),
        3 => v.extend([
            Item::Break,
            Item::Toggle("osc3.sync", "calor.osc.sync", 96.0),
            Item::Toggle("osc3.low", "calor.osc3.low", 100.0),
            Item::Toggle("osc3.kbd", "calor.osc3.kbd", 108.0),
        ]),
        _ => {}
    }
    v
}

pub fn show(app: &mut CantusApp, ui: &mut egui::Ui, track: TrackId, spec: &NativeSpec, s: &Style) {
    let tel = app.inst_telemetry();
    let width = ui.available_width();
    let g = parts::gutter(s);

    let osc: [Vec<Item>; 3] = [osc_items(1), osc_items(2), osc_items(3)];
    let row1: Vec<Card> = (0..3)
        .map(|i| Card {
            title: [
                "calor.section.osc1",
                "calor.section.osc2",
                "calor.section.osc3",
            ][i],
            f: Function::Source,
            display: Some(52.0),
            bar: Bar::Choice(["osc1.rev", "osc2.rev", "osc3.rev"][i], &REVS, 0.0, 180.0),
            items: &osc[i],
            frac: 1.0 / 3.0,
            extra: Extra::Osc(i + 1),
        })
        .collect();

    let mixer = [
        cell("mix.osc1"),
        cell("mix.osc2"),
        cell("mix.osc3"),
        cell("mix.sub"),
        cell("mix.noise"),
        cell("mix.feedback"),
        cell("mix.drive"),
        Item::Break,
        choice("sub.shape", &SUBS, 210.0),
        choice("noise.color", &NOISE, 130.0),
    ];
    let hpf = [
        cell("hpf.cutoff"),
        Item::Break,
        choice("hpf.pos", &HPF_POS, 160.0),
    ];
    let amp = [
        cell("vca.vel"),
        cell("vca.drive"),
        cell_as("vca.pan", Fmt::Pan),
        Item::Break,
        choice("vca.source", &VCA_SRC, 170.0),
    ];
    let row2 = [
        Card {
            title: "calor.section.mixer",
            f: Function::Source,
            display: None,
            bar: Bar::None,
            items: &mixer,
            frac: 0.5,
            extra: Extra::None,
        },
        Card {
            title: "calor.section.hpf",
            f: Function::Tone,
            display: None,
            bar: Bar::None,
            items: &hpf,
            frac: 0.18,
            extra: Extra::None,
        },
        Card {
            title: "calor.section.amp",
            f: Function::Dynamics,
            display: None,
            bar: Bar::None,
            items: &amp,
            frac: 0.32,
            extra: Extra::None,
        },
    ];

    let (col, n) = super::filter_column(spec);
    let svf =
        cantus_dsp::va::Model::from_index(spec.get("vcf.model")) == cantus_dsp::va::Model::Svf;
    let mut filt = vec![
        cell("vcf.cutoff"),
        cell("vcf.reso"),
        cell("vcf.drive"),
        cell("vcf.bass_comp"),
        cell("vcf.key"),
        cell("vcf.env"),
        cell("vcf.vel"),
        cell("vcf.lfo"),
        cell("vcf.fm"),
    ];
    if svf {
        filt.push(cell("vcf.morph"));
    }
    filt.push(Item::Column(&col[..n]));
    let row3 = [Card {
        title: "calor.section.filter",
        f: Function::Tone,
        display: Some(150.0),
        bar: Bar::Choice("vcf.model", &MODELS, 0.0, 330.0),
        items: &filt,
        frac: 1.0,
        extra: Extra::Filter,
    }];

    let env = |e: usize| -> [Item<'static>; 9] {
        const K: [[&str; 8]; 2] = [
            [
                "env1.a",
                "env1.d",
                "env1.s",
                "env1.r",
                "env1.kbd",
                "env1.vel",
                "env1.model",
                "env1.retrig",
            ],
            [
                "env2.a",
                "env2.d",
                "env2.s",
                "env2.r",
                "env2.kbd",
                "env2.vel",
                "env2.model",
                "env2.retrig",
            ],
        ];
        let k = &K[e - 1];
        [
            cell(k[0]),
            cell(k[1]),
            cell(k[2]),
            cell(k[3]),
            cell(k[4]),
            cell(k[5]),
            Item::Break,
            choice(k[6], &ENV_MODELS, 250.0),
            choice(k[7], &RETRIG, 220.0),
        ]
    };
    let (env1, env2) = (env(1), env(2));
    let row4 = [
        Card {
            title: "calor.section.env1",
            f: Function::Dynamics,
            display: Some(60.0),
            bar: Bar::None,
            items: &env1,
            frac: 0.5,
            extra: Extra::Env(1),
        },
        Card {
            title: "calor.section.env2",
            f: Function::Dynamics,
            display: Some(60.0),
            bar: Bar::None,
            items: &env2,
            frac: 0.5,
            extra: Extra::Env(2),
        },
    ];

    let lfo1 = [
        cell_as("lfo1.rate", Fmt::Hz),
        cell("lfo1.delay"),
        cell_as("lfo1.phase", Fmt::Degrees),
        cell("lfo1.pitch"),
        Item::Break,
        choice("lfo1.restart", &LFO_RESTART, 190.0),
        Item::Toggle("lfo1.unipolar", "calor.lfo.unipolar", 84.0),
    ];
    let lfo2 = [
        cell_as("lfo2.rate", Fmt::Hz),
        cell("lfo2.delay"),
        cell_as("lfo2.phase", Fmt::Degrees),
        Item::Break,
        choice("lfo2.restart", &LFO_RESTART, 190.0),
        Item::Toggle("lfo2.unipolar", "calor.lfo.unipolar", 84.0),
    ];
    let routes = [
        Item::Toggle("polymod.to_freq", "calor.polymod.to_freq", 124.0),
        Item::Toggle("polymod.to_pw", "calor.polymod.to_pw", 124.0),
        Item::Toggle("polymod.to_vcf", "calor.polymod.to_vcf", 124.0),
    ];
    let wheel = [
        cell("wheel.vibrato"),
        cell("wheel.mix"),
        cell("wheel.to_pw"),
        cell("wheel.to_vcf"),
        Item::Break,
        cell("polymod.env"),
        cell("polymod.osc3"),
        cell("xmod.amount"),
        Item::Column(&routes),
    ];
    let row5 = [
        Card {
            title: "calor.section.lfo1",
            f: Function::Mod,
            display: Some(48.0),
            bar: Bar::Stepper("lfo1.shape", &LFO_SHAPES, 150.0),
            items: &lfo1,
            frac: 0.3,
            extra: Extra::Lfo(1),
        },
        Card {
            title: "calor.section.lfo2",
            f: Function::Mod,
            display: Some(48.0),
            bar: Bar::Stepper("lfo2.shape", &LFO_SHAPES, 150.0),
            items: &lfo2,
            frac: 0.3,
            extra: Extra::Lfo(2),
        },
        Card {
            title: "calor.section.mod",
            f: Function::Mod,
            display: None,
            bar: Bar::None,
            items: &wheel,
            frac: 0.4,
            extra: Extra::None,
        },
    ];

    let glide = [
        choice("glide.mode", &GLIDE_MODE, 170.0),
        choice("glide.curve", &GLIDE_CURVE, 170.0),
    ];
    let voices = [
        choice("voice.mode", &MODES, 150.0),
        choice("voice.assign", &ASSIGN, 200.0),
        choice("voice.priority", &PRIORITY, 124.0),
        Item::Break,
        cell_as("voice.count", Fmt::Whole("")),
        cell_as("voice.stack", Fmt::Whole("")),
        cell("voice.stack_detune"),
        cell("voice.stack_spread"),
        cell_as("tune.transpose", Fmt::Signed("st")),
        cell("tune.fine"),
        Item::Break,
        cell("glide.time"),
        cell_as("bend.up", Fmt::Signed("st")),
        cell_as("bend.down", Fmt::Signed("st")),
        Item::Column(&glide),
        Item::Toggle("glide.legato", "calor.glide.legato", 110.0),
    ];
    let chorus = [
        Item::Choice {
            key: "chorus.mode",
            labels: &CHORUS,
            first: 0.0,
            w: 190.0,
            caption: false,
        },
        Item::Break,
        cell("chorus.mix"),
    ];
    let output = [
        choice("quality", &QUALITIES, 210.0),
        Item::Break,
        Item::Slot(210.0, 12.0),
        Item::Break,
        cell("gain"),
    ];
    let row6 = [
        Card {
            title: "calor.section.voices",
            f: Function::Output,
            display: None,
            bar: Bar::None,
            items: &voices,
            frac: 0.5,
            extra: Extra::None,
        },
        Card {
            title: "calor.section.chorus",
            f: Function::Space,
            display: None,
            bar: Bar::None,
            items: &chorus,
            frac: 0.22,
            extra: Extra::None,
        },
        Card {
            title: "calor.section.output",
            f: Function::Output,
            display: None,
            bar: Bar::None,
            items: &output,
            frac: 0.28,
            extra: Extra::Output,
        },
    ];

    let words = [
        Item::Slot(210.0, 34.0),
        choice("analog.phase", &PHASE, 210.0),
    ];
    let cards = [
        cell("analog.condition"),
        cell_as("analog.seed", Fmt::Whole("#")),
        cell("analog.drift"),
        cell_as("analog.drift_rate", Fmt::Hz),
        cell("analog.detune"),
        cell("analog.var_cutoff"),
        cell("analog.var_env"),
        cell("analog.var_pw"),
        cell("analog.var_glide"),
        cell("analog.tracking"),
        Item::Column(&words),
        Item::Break,
        Item::Slot(560.0, 22.0),
    ];
    let row7 = [Card {
        title: "calor.section.cards",
        f: Function::Output,
        display: Some(176.0),
        bar: Bar::None,
        items: &cards,
        frac: 1.0,
        extra: Extra::Cards,
    }];

    let rows: [&[Card]; 7] = [&row1, &row2, &row3, &row4, &row5, &row6, &row7];
    let heights: Vec<f32> = rows.iter().map(|r| row_height(r, width, s)).collect();
    let total = heights.iter().sum::<f32>() + g * (rows.len() - 1) as f32;
    let (area, _) = ui.allocate_exact_size(Vec2::new(width, total), Sense::hover());
    let mut y = area.top();
    for (r, h) in rows.iter().zip(&heights) {
        draw_row(app, ui, track, spec, s, &tel, area, y, *h, r);
        y += h + g;
    }
}
