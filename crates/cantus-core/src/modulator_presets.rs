//! Controller presets (fxd step 3, ADRs 0147, 0148): ready settings for Tactus (the X-Y
//! pad) and Calculus (the formula controller), each with the links its routes take in
//! order — route 0 reads `links[0]`, route 1 `links[1]` and so on.
//!
//! **What a preset holds.** A pad's or a formula's settings (its look 0, its links empty)
//! and the axis and range each of the first routes reads. Applying one keeps everything
//! that is the modulator's own — its id, name, routes, amount, mute and the face's
//! colourway — and replaces only the source and the links.
//!
//! **Calculus's presets** share one plan for the knobs, so a knob means the same thing
//! wherever it can: `a` the **rate** (2^(Round(a·6) − 4) cycles a beat — a bar at 0.34, a
//! beat at 0.67, from one cycle in four bars to four in a beat; the envelope's attack and
//! the climb's length instead), `b` the **shape** (the waves' phase, the pulse's and the
//! gate's width, the random's sequence, the envelope's decay, the climb's bend) and `c`
//! the **depth**. The waves are bipolar (their centre is the parameter's own value); the
//! gate, the envelope and the climb rise from it. Every formula is in FL's syntax and
//! compiles (a test proves it, and samples each one's shape).
//!
//! Names and one-line descriptions are in the locales: `fxd.preset.<id>` and
//! `fxd.preset.<id>.what`.

use super::{
    formula, FormulaSource, ModKind, ModSource, Modulator, RouteLink, RouteRange, XyPath, XySource,
};

/// How a preset's route reads its controller: an output (an axis) and a range.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct PresetLink {
    pub output: u8,
    pub range: Option<RouteRange>,
}

/// A controller preset.
#[derive(Debug)]
pub struct ControllerPreset {
    /// Its locale keys: `fxd.preset.<id>` (the name) and `fxd.preset.<id>.what`.
    pub id: &'static str,
    /// [`ModKind::XyPad`] or [`ModKind::Formula`].
    pub kind: ModKind,
    /// Its settings (look 0, links empty).
    pub source: fn() -> ModSource,
    /// The links its routes take, in order; routes beyond them get none.
    pub links: &'static [PresetLink],
}

impl PartialEq for ControllerPreset {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

/// Output 0 (X, or the formula) and output 1 (Y), as they are.
const X: PresetLink = PresetLink {
    output: 0,
    range: None,
};
const Y: PresetLink = PresetLink {
    output: 1,
    range: None,
};

fn pad(x: f32, y: f32, inertia_ms: f32) -> XySource {
    XySource {
        x,
        y,
        inertia_ms,
        ..XySource::default()
    }
}

fn filter_sweep() -> ModSource {
    ModSource::XyPad(pad(0.5, 0.0, 80.0))
}

fn width_pan() -> ModSource {
    ModSource::XyPad(pad(0.5, 0.5, 120.0))
}

/// The figure-eight's points: a 1:2 Lissajous (∞), 32 of them over two bars.
pub const FIGURE_EIGHT_POINTS: usize = 32;

fn figure_eight() -> ModSource {
    let points = (0..FIGURE_EIGHT_POINTS)
        .map(|k| {
            let th = std::f32::consts::TAU * k as f32 / FIGURE_EIGHT_POINTS as f32;
            [
                0.5 + 0.4 * cantus_dsp::math::sin(th),
                0.5 + 0.4 * cantus_dsp::math::sin(2.0 * th),
            ]
        })
        .collect();
    ModSource::XyPad(XySource {
        path: Some(XyPath {
            points,
            beats: 8.0,
            smooth: true,
        }),
        play_path: true,
        ..pad(0.5, 0.5, 0.0)
    })
}

fn performance() -> ModSource {
    ModSource::XyPad(pad(0.5, 0.5, 400.0))
}

/// The cycles of the rate knob `a` at the song position, as formula text.
macro_rules! cyc {
    () => {
        "SongTime*2^(Round(a*6)-4)"
    };
}

/// The formula presets' texts.
pub const SINE: &str = concat!("0.5+0.5*c*Sin(2*Pi*(", cyc!(), "+b))");
pub const TRIANGLE: &str = concat!("0.5+c*(0.5-Abs(2*Frac(", cyc!(), "+b)-1))");
pub const SAW: &str = concat!("0.5+c*(Frac(", cyc!(), "+b)-0.5)");
pub const SQUARE: &str = concat!("0.5+c*((Frac(", cyc!(), "+b)<0.5)-0.5)");
pub const PULSE: &str = concat!("0.5+c*((Frac(", cyc!(), ")<b)-0.5)");
pub const RANDOM_BEAT: &str = concat!(
    "0.5+c*(SeededRand(Int(",
    cyc!(),
    ")+Round(b*100)*1000)-0.5)"
);
pub const GATE_BEATS: &str = concat!(
    "c*Min(Min(1,Frac(",
    cyc!(),
    ")*50),Max(0,Min(1,(b-Frac(",
    cyc!(),
    "))*50)))"
);
pub const ENV_BAR: &str =
    "c*Min(1,Frac(SongTime/4)/(a*0.5+0.001))*Exp(-Max(0,Frac(SongTime/4)-a*0.5)*(1+b*15))";
pub const CLIMB: &str = "c*Power(Min(1,SongTime/(4*Max(1,Round(a*64)))),4^(2*b-1))";

fn formula_of(text: &str, inputs: [f32; 3], bipolar: bool) -> ModSource {
    ModSource::Formula(FormulaSource {
        text: text.to_string(),
        inputs,
        bipolar,
        links: Vec::new(),
        look: 0,
    })
}

/// A bar a cycle, phase 0, full depth.
const BAR: [f32; 3] = [0.34, 0.0, 1.0];

fn f_sine() -> ModSource {
    formula_of(SINE, BAR, true)
}
fn f_triangle() -> ModSource {
    formula_of(TRIANGLE, BAR, true)
}
fn f_saw() -> ModSource {
    formula_of(SAW, BAR, true)
}
fn f_square() -> ModSource {
    formula_of(SQUARE, BAR, true)
}
fn f_pulse() -> ModSource {
    formula_of(PULSE, [0.67, 0.25, 1.0], true)
}
fn f_random_beat() -> ModSource {
    formula_of(RANDOM_BEAT, [0.67, 0.0, 1.0], true)
}
fn f_gate_beats() -> ModSource {
    formula_of(GATE_BEATS, [0.67, 0.5, 1.0], false)
}
fn f_env_bar() -> ModSource {
    formula_of(ENV_BAR, [0.05, 0.4, 1.0], false)
}
fn f_climb() -> ModSource {
    formula_of(CLIMB, [0.25, 0.5, 1.0], false)
}

/// Every preset, the pads' first.
pub static PRESETS: &[ControllerPreset] = &[
    ControllerPreset {
        id: "xy.filter_sweep",
        kind: ModKind::XyPad,
        source: filter_sweep,
        links: &[
            PresetLink {
                output: 0,
                range: Some(RouteRange {
                    from: 0.0,
                    to: 1.0,
                    curve: 0.0,
                }),
            },
            PresetLink {
                output: 1,
                range: Some(RouteRange {
                    from: 0.0,
                    to: 0.6,
                    curve: 0.4,
                }),
            },
        ],
    },
    ControllerPreset {
        id: "xy.width_pan",
        kind: ModKind::XyPad,
        source: width_pan,
        links: &[X, Y],
    },
    ControllerPreset {
        id: "xy.figure_eight",
        kind: ModKind::XyPad,
        source: figure_eight,
        links: &[X, Y],
    },
    ControllerPreset {
        id: "xy.performance",
        kind: ModKind::XyPad,
        source: performance,
        links: &[X, Y],
    },
    ControllerPreset {
        id: "formula.sine",
        kind: ModKind::Formula,
        source: f_sine,
        links: &[],
    },
    ControllerPreset {
        id: "formula.triangle",
        kind: ModKind::Formula,
        source: f_triangle,
        links: &[],
    },
    ControllerPreset {
        id: "formula.saw",
        kind: ModKind::Formula,
        source: f_saw,
        links: &[],
    },
    ControllerPreset {
        id: "formula.square",
        kind: ModKind::Formula,
        source: f_square,
        links: &[],
    },
    ControllerPreset {
        id: "formula.pulse",
        kind: ModKind::Formula,
        source: f_pulse,
        links: &[],
    },
    ControllerPreset {
        id: "formula.random_beat",
        kind: ModKind::Formula,
        source: f_random_beat,
        links: &[],
    },
    ControllerPreset {
        id: "formula.gate_beats",
        kind: ModKind::Formula,
        source: f_gate_beats,
        links: &[],
    },
    ControllerPreset {
        id: "formula.env_bar",
        kind: ModKind::Formula,
        source: f_env_bar,
        links: &[],
    },
    ControllerPreset {
        id: "formula.climb",
        kind: ModKind::Formula,
        source: f_climb,
        links: &[],
    },
];

/// The presets of one kind, in order.
pub fn of_kind(kind: ModKind) -> impl Iterator<Item = &'static ControllerPreset> {
    PRESETS.iter().filter(move |p| p.kind == kind)
}

/// The preset named `id`.
pub fn by_id(id: &str) -> Option<&'static ControllerPreset> {
    PRESETS.iter().find(|p| p.id == id)
}

/// The face's colourway a source keeps (0 for what has none).
fn look_of(src: &ModSource) -> u8 {
    match src {
        ModSource::XyPad(x) => x.look,
        ModSource::Formula(f) => f.look,
        _ => 0,
    }
}

fn set_look(src: &mut ModSource, look: u8) {
    match src {
        ModSource::XyPad(x) => x.look = look,
        ModSource::Formula(f) => f.look = look,
        _ => {}
    }
}

/// The modulator with preset `p` applied: its id, name, routes, amount and mute kept; the
/// source replaced by `p`'s (the face's look kept), its links rebuilt from `p.links` over
/// the routes in order. Valid whenever `m` is.
pub fn apply(m: &Modulator, p: &ControllerPreset) -> Modulator {
    let mut source = (p.source)();
    set_look(&mut source, look_of(&m.source));
    if let Some(links) = source.links_mut() {
        links.clear();
        links.extend(m.routes.iter().zip(p.links).map(|(r, l)| RouteLink {
            target: r.target.clone(),
            output: l.output.min(p.kind.outputs().saturating_sub(1)),
            range: l.range,
        }));
    }
    Modulator {
        source,
        ..m.clone()
    }
}

/// A source's settings as a preset compares them: no look, no links, and the inputs at
/// rest (where the hand or a knob left a pad's puck and a formula's knobs is playing,
/// not a setting: the header keeps the preset's name while it is played).
fn settings(src: &ModSource) -> ModSource {
    let mut s = src.clone();
    set_look(&mut s, 0);
    if let Some(l) = s.links_mut() {
        l.clear();
    }
    match &mut s {
        ModSource::XyPad(x) => {
            x.x = 0.0;
            x.y = 0.0;
        }
        ModSource::Formula(f) => f.inputs = [0.0; 3],
        _ => {}
    }
    s
}

/// Which preset the modulator's source is (its look, links and inputs aside), if any:
/// what the window's header names.
pub fn matching(m: &Modulator) -> Option<&'static ControllerPreset> {
    let kind = m.source.kind();
    if !ModKind::CONTROLLERS.contains(&kind) {
        return None;
    }
    let mine = settings(&m.source);
    of_kind(kind).find(|p| settings(&(p.source)()) == mine)
}

/// Whether every preset's formula compiles (what the tests hold; a cheap check for a
/// caller that wants it).
pub fn all_compile() -> bool {
    PRESETS.iter().all(|p| match (p.source)() {
        ModSource::Formula(f) => formula::Program::compile(&f.text).is_ok(),
        _ => true,
    })
}

#[cfg(test)]
#[path = "modulator_presets_tests.rs"]
mod tests;
