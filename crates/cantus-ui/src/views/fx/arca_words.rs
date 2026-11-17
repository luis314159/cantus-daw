//! Arca's words and the facts its window shows (ADR 0137, provisional): each amp's
//! character, its valves, its supply and bias, read from the models the DSP plays — the
//! window never states a figure the circuit does not have.

use super::super::frame;
use cantus_core::EffectSpec;
use cantus_dsp::arca::power::Bias;
use cantus_dsp::arca::AmpModel;

/// The amp choices' names (0: none).
pub const MODELS: [&str; 6] = [
    "fx.arca.model.0",
    "fx.arca.model.1",
    "fx.arca.model.2",
    "fx.arca.model.3",
    "fx.arca.model.4",
    "fx.arca.model.5",
];

/// The cabinet choices' names (0: none, 6: the loaded file).
pub const CABS: [&str; 7] = [
    "fx.arca.cab.0",
    "fx.arca.cab.1",
    "fx.arca.cab.2",
    "fx.arca.cab.3",
    "fx.arca.cab.4",
    "fx.arca.cab.5",
    "fx.arca.cab.6",
];

/// The microphones' names.
pub const MICS: [&str; 3] = ["fx.arca.mic.0", "fx.arca.mic.1", "fx.arca.mic.2"];

/// The oversampling choices' names.
pub const OS: [&str; 3] = ["fx.arca.os.0", "fx.arca.os.1", "fx.arca.os.2"];

/// What an amp is, as the chassis says it.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Character {
    /// Its character in a line (a locale key).
    pub what: &'static str,
    /// The power valves: their type and how many.
    pub power_valve: &'static str,
    pub power_count: usize,
    /// Rated power (W).
    pub watts: f32,
    /// A valve rectifier (it sags more) or a solid-state one.
    pub valve_rectifier: bool,
    /// Cathode bias (class A, the bias moving with the signal) or a fixed bias.
    pub cathode_bias: bool,
    /// The presence knob works as a cut (the top-boost amp's, turned the other way).
    pub cut: bool,
}

/// The facts of `m`, from its power amp's values.
pub fn character(m: AmpModel) -> Character {
    let p = m.power();
    let (what, power_valve) = match m {
        AmpModel::Clean => ("arca.what.clean", "6V6GT"),
        AmpModel::Crunch => ("arca.what.crunch", "EL34"),
        AmpModel::Chime => ("arca.what.chime", "EL84"),
        AmpModel::HighGain => ("arca.what.high_gain", "6L6GC"),
        AmpModel::Bass => ("arca.what.bass", "5881"),
    };
    Character {
        what,
        power_valve,
        power_count: (2.0 * p.per_side).round() as usize,
        watts: p.watts as f32,
        // A valve rectifier's resistance is a hundred ohms and more; a diode's, tens.
        valve_rectifier: p.r_supply >= 100.0,
        cathode_bias: matches!(p.bias, Bias::Cathode { .. }),
        cut: m.has_cut(),
    }
}

/// The preamp valve of circuit `i` of `m` (the bass amp's input is the milder 12AY7).
pub fn pre_valve(m: AmpModel, i: usize) -> &'static str {
    if m == AmpModel::Bass && i == 0 {
        "12AY7"
    } else {
        "12AX7"
    }
}

/// The amp chosen by `spec`, or none.
pub fn model(spec: &EffectSpec) -> Option<AmpModel> {
    match spec.get("model").round().max(0.0) as usize {
        0 => None,
        i => Some(AmpModel::from_index(i - 1)),
    }
}

/// The inspector's line: the amp and the cabinet ("Crunch · 4×12 vintage"), or that neither
/// is on.
pub fn summary(spec: &EffectSpec) -> String {
    let m = (spec.get("model").round().max(0.0) as usize).min(5);
    let c = (spec.get("cab").round().max(0.0) as usize).min(6);
    match (m, c) {
        (0, 0) => frame::tr("arca.off").to_string(),
        (0, c) => format!("{} \u{b7} {}", frame::tr("arca.no_amp"), cab_name(spec, c)),
        (m, 0) => format!(
            "{} \u{b7} {}",
            frame::tr(MODELS[m]),
            frame::tr("arca.no_cab")
        ),
        (m, c) => format!("{} \u{b7} {}", frame::tr(MODELS[m]), cab_name(spec, c)),
    }
}

/// A cabinet choice's name; the loaded file by its own name.
pub fn cab_name(spec: &EffectSpec, c: usize) -> String {
    match (&spec.ir, c) {
        (Some(f), 6) => f.name().to_string(),
        _ => frame::tr(CABS[c.min(6)]).to_string(),
    }
}

/// A knob cell's width (points at 100 %) for its label `key`: the medium knob's own, or
/// wider when the word needs it in the interface's language (never elided).
pub fn cell_width(ui: &egui::Ui, key: &str) -> f32 {
    use crate::kit::text::{self as ktext, Role, Text};
    let t = Text::key(key, Role::Label, egui::Color32::TRANSPARENT).caps(true);
    (ktext::measure(ui.ctx(), &t, 1.0).x + 16.0).max(64.0)
}
