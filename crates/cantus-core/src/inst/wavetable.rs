//! Unda — the wavetable / hybrid flagship synth: its parameter model. Owned by this
//! instrument's builder (ADR 0023). The DSP is `cantus_dsp::unda`, the unit
//! `cantus_engine::inst::wavetable`, the editor `cantus_ui::views::inst::wavetable`; the
//! document is `docs/instruments/unda.md`.
//!
//! Three wavetable oscillators (`osc1…3`), a sub and a noise source, two filters
//! (`flt1`, `flt2`, serial ↔ parallel by `flt.routing`), four envelopes (`env1` is the
//! amplifier), eight LFOs, the shared macros and matrix. Every value is in the unit a
//! musician reads; a value at its default changes no sample (the init patch is the
//! defaults). Keys are forever once shipped.
//!
//! Text keys: `osc{n}.table` is the path of an imported table (an asset the engine
//! decodes; `osc{n}.frame` holds its samples per cycle, read at import), `osc{n}.name`
//! its display name, and `lfo{n}.shape` a drawn LFO shape (`x,y;x,y;…`, x and y in
//! 0…1, at most [`LFO_POINTS`] points).

use super::{find, InstParamInfo};
use crate::effect::p;
use crate::model::TrackIcon;

pub const NAME_KEY: &str = "instrument.wavetable";
pub const ICON: TrackIcon = TrackIcon::Synth;
/// Offered in the instrument picker once it is built.
pub const READY: bool = true;

/// The factory wavetables, in `osc{n}.wt` order. Their label keys are `unda.wt.<name>`;
/// the DSP generates them in code (`cantus_dsp::unda::factory`).
pub const TABLES: &[&str] = &[
    "basic",
    "analog",
    "pwm",
    "harmonics",
    "formant",
    "vocal",
    "digital",
    "sync",
    "fm",
    "organ",
];

/// Warp modes, in `osc{n}.warp{k}` order. Label keys `unda.warp.<name>`.
pub const WARPS: &[&str] = &[
    "off",
    "sync",
    "bend_plus",
    "bend_minus",
    "asym_plus",
    "asym_minus",
    "pwm",
    "mirror",
    "flip",
    "quantize",
    "fold",
    "soft_clip",
    "hard_clip",
    "fm",
    "pm",
    "am",
    "rm",
];

/// What an FM/PM/AM/RM warp reads, in `osc{n}.warp{k}.src` order. Label keys
/// `unda.src.<name>`.
pub const WARP_SOURCES: &[&str] = &["osc1", "osc2", "osc3", "sub", "noise", "self"];

/// Where a source goes, in `…route` order. Label keys `unda.route.<name>`.
pub const ROUTES: &[&str] = &["f1", "f2", "both", "direct"];

/// Filter types, in `flt{n}.type` order. Label keys `unda.flt.<name>`.
pub const FILTERS: &[&str] = &[
    "svf_lp12",
    "svf_lp24",
    "svf_bp",
    "svf_hp12",
    "svf_hp24",
    "svf_notch",
    "ladder_lp24",
    "ladder_lp12",
    "ladder_bp",
    "ladder_hp24",
];

/// Unison detune laws, in `osc{n}.ucurve` order. Label keys `unda.ucurve.<name>`.
pub const UNISON_CURVES: &[&str] = &["super", "linear", "power", "random"];

/// Unison stacks, in `osc{n}.stack` order. Label keys `unda.stack.<name>`.
pub const STACKS: &[&str] = &[
    "off",
    "octave",
    "octave_fifth",
    "down_octave",
    "down_two",
    "fifths",
    "major",
    "minor",
];

/// Sub waves, in `sub.wave` order. Label keys `unda.sub.<name>`.
pub const SUB_WAVES: &[&str] = &["sine", "triangle", "saw", "square", "pulse"];

/// Oscillator engines, in `osc{n}.engine` order. Label keys `unda.engine.<name>`:
/// the wavetable, Partes (additive) and Grana (granular).
pub const ENGINES: &[&str] = &["wavetable", "partes", "grana"];

/// Grain windows, in `osc{n}.gwin` order. Label keys `unda.gwin.<name>`.
pub const GRAIN_WINDOWS: &[&str] = &["hann", "gauss", "tukey", "expodec"];

/// LFO shapes, in `lfo{n}.wave` order. Label keys `unda.lfo.<name>`.
pub const LFO_WAVES: &[&str] = &[
    "sine",
    "triangle",
    "saw_up",
    "saw_down",
    "square",
    "sample_hold",
    "smooth_random",
    "drawn",
];

/// LFO modes, in `lfo{n}.mode` order. Label keys `unda.lfo_mode.<name>`.
pub const LFO_MODES: &[&str] = &["retrig", "free", "envelope"];

/// Voice modes, in `voice.mode` order (the shared allocator's `PlayMode`).
pub const VOICE_MODES: &[&str] = &["poly", "mono", "legato"];

/// Oversampling, in `quality` order: the factor is `1 << index`.
pub const QUALITY: &[&str] = &["1x", "2x", "4x"];

/// The most points a drawn LFO shape holds.
pub const LFO_POINTS: usize = 16;
/// The most frames a table holds (more are thinned evenly at import).
pub const MAX_FRAMES: usize = 256;

pub const OSCS: usize = 3;
pub const ENVS: usize = 4;
pub const LFOS: usize = 8;
pub const FILTERS_N: usize = 2;

/// Every numeric parameter, generated per oscillator, envelope and LFO so the three
/// oscillators can never drift apart.
macro_rules! unda_params {
    (
        osc: [$(($o:literal, $on:expr, $src:expr)),*],
        flt: [$(($f:literal, $cut:expr)),*],
        env: [$(($e:literal, $a:expr, $d:expr, $s:expr, $r:expr)),*],
        lfo: [$($l:literal),*]
    ) => {
        &[
            p("gain", "inst.gain", -48.0, 12.0, 0.0, "dB", false, false),
            p("quality", "unda.quality", 0.0, 2.0, 1.0, "", false, true),
            p("voice.poly", "unda.poly", 1.0, 32.0, 16.0, "", false, true),
            p("voice.mode", "unda.mode", 0.0, 2.0, 0.0, "", false, true),
            p("voice.prio", "unda.prio", 0.0, 2.0, 0.0, "", false, true),
            p("voice.glide", "unda.glide", 0.0, 2.0, 0.0, "s", false, false),
            p("voice.glide_mode", "unda.glide_mode", 0.0, 1.0, 0.0, "", false, true),
            p("voice.bend", "unda.bend", 0.0, 24.0, 2.0, "st", false, true),
            p("voice.vel", "unda.vel", 0.0, 1.0, 0.5, "%", false, false),
            p("voice.drift", "unda.drift", 0.0, 1.0, 0.0, "%", false, false),
            p("voice.seed", "unda.seed", 0.0, 999.0, 0.0, "", false, true),
            p("flt.routing", "unda.routing", 0.0, 1.0, 0.0, "%", false, false),
            $(
                p(concat!("osc", $o, ".on"), "unda.on", 0.0, 1.0, $on, "", false, true),
                p(concat!("osc", $o, ".wt"), "unda.table", 0.0, 9.0, 0.0, "", false, true),
                p(concat!("osc", $o, ".frame"), "unda.frame", 16.0, 4096.0, 2048.0, "", false, true),
                p(concat!("osc", $o, ".pos"), "unda.pos", 0.0, 1.0, 0.0, "%", false, false),
                p(concat!("osc", $o, ".oct"), "unda.oct", -4.0, 4.0, 0.0, "oct", false, true),
                p(concat!("osc", $o, ".semi"), "unda.semi", -12.0, 12.0, 0.0, "st", false, true),
                p(concat!("osc", $o, ".fine"), "unda.fine", -100.0, 100.0, 0.0, "ct", false, false),
                p(concat!("osc", $o, ".pitch"), "unda.pitch", -48.0, 48.0, 0.0, "st", false, false),
                p(concat!("osc", $o, ".phase"), "unda.phase", 0.0, 360.0, 0.0, "°", false, false),
                p(concat!("osc", $o, ".rand"), "unda.rand", 0.0, 1.0, 1.0, "%", false, false),
                p(concat!("osc", $o, ".level"), "unda.level", -48.0, 6.0, 0.0, "dB", false, false),
                p(concat!("osc", $o, ".pan"), "unda.pan", -1.0, 1.0, 0.0, "", false, false),
                p(concat!("osc", $o, ".route"), "unda.route", 0.0, 3.0, 0.0, "", false, true),
                p(concat!("osc", $o, ".uni"), "unda.uni", 1.0, 16.0, 1.0, "", false, true),
                p(concat!("osc", $o, ".detune"), "unda.detune", 0.0, 100.0, 20.0, "ct", false, false),
                p(concat!("osc", $o, ".ucurve"), "unda.ucurve", 0.0, 3.0, 0.0, "", false, true),
                p(concat!("osc", $o, ".blend"), "unda.blend", 0.0, 1.0, 0.5, "%", false, false),
                p(concat!("osc", $o, ".width"), "unda.width", 0.0, 1.0, 1.0, "%", false, false),
                p(concat!("osc", $o, ".stack"), "unda.stack", 0.0, 7.0, 0.0, "", false, true),
                p(concat!("osc", $o, ".spread"), "unda.spread", -1.0, 1.0, 0.0, "%", false, false),
                p(concat!("osc", $o, ".warp1"), "unda.warp", 0.0, 16.0, 0.0, "", false, true),
                p(concat!("osc", $o, ".warp1.amt"), "unda.warp_amt", 0.0, 1.0, 0.0, "%", false, false),
                p(concat!("osc", $o, ".warp1.src"), "unda.warp_src", 0.0, 5.0, $src, "", false, true),
                p(concat!("osc", $o, ".warp2"), "unda.warp", 0.0, 16.0, 0.0, "", false, true),
                p(concat!("osc", $o, ".warp2.amt"), "unda.warp_amt", 0.0, 1.0, 0.0, "%", false, false),
                p(concat!("osc", $o, ".warp2.src"), "unda.warp_src", 0.0, 5.0, $src, "", false, true),
                p(concat!("osc", $o, ".engine"), "unda.engine", 0.0, 2.0, 0.0, "", false, true),
                p(concat!("osc", $o, ".partials"), "unda.partials", 1.0, 256.0, 64.0, "", false, true),
                p(concat!("osc", $o, ".tilt"), "unda.tilt", -12.0, 12.0, 0.0, "dB", false, false),
                p(concat!("osc", $o, ".oddeven"), "unda.oddeven", -1.0, 1.0, 0.0, "", false, false),
                p(concat!("osc", $o, ".stretch"), "unda.stretch", -0.1, 0.1, 0.0, "%", false, false),
                p(concat!("osc", $o, ".density"), "unda.density", 1.0, 200.0, 30.0, "Hz", true, false),
                p(concat!("osc", $o, ".glen"), "unda.glen", 0.005, 0.5, 0.08, "s", true, false),
                p(concat!("osc", $o, ".spray"), "unda.spray", 0.0, 0.5, 0.02, "s", false, false),
                p(concat!("osc", $o, ".gpitch"), "unda.gpitch", 0.0, 1200.0, 0.0, "ct", false, false),
                p(concat!("osc", $o, ".greverse"), "unda.greverse", 0.0, 1.0, 0.0, "%", false, false),
                p(concat!("osc", $o, ".gwin"), "unda.gwin", 0.0, 3.0, 0.0, "", false, true),
            )*
            p("sub.on", "unda.on", 0.0, 1.0, 0.0, "", false, true),
            p("sub.wave", "unda.sub_wave", 0.0, 4.0, 0.0, "", false, true),
            p("sub.oct", "unda.sub_oct", -3.0, 0.0, -1.0, "oct", false, true),
            p("sub.level", "unda.level", -48.0, 6.0, 0.0, "dB", false, false),
            p("sub.pan", "unda.pan", -1.0, 1.0, 0.0, "", false, false),
            p("sub.route", "unda.route", 0.0, 3.0, 3.0, "", false, true),
            p("noise.on", "unda.on", 0.0, 1.0, 0.0, "", false, true),
            p("noise.color", "unda.color", -1.0, 1.0, 0.0, "", false, false),
            p("noise.level", "unda.level", -48.0, 6.0, -12.0, "dB", false, false),
            p("noise.pan", "unda.pan", -1.0, 1.0, 0.0, "", false, false),
            p("noise.width", "unda.width", 0.0, 1.0, 1.0, "%", false, false),
            p("noise.route", "unda.route", 0.0, 3.0, 0.0, "", false, true),
            $(
                p(concat!("flt", $f, ".on"), "unda.on", 0.0, 1.0, 0.0, "", false, true),
                p(concat!("flt", $f, ".type"), "unda.flt_type", 0.0, 9.0, 0.0, "", false, true),
                p(concat!("flt", $f, ".cutoff"), "unda.cutoff", 16.0, 20_000.0, $cut, "Hz", true, false),
                p(concat!("flt", $f, ".res"), "unda.res", 0.0, 1.0, 0.1, "%", false, false),
                p(concat!("flt", $f, ".drive"), "unda.drive", 0.0, 36.0, 0.0, "dB", false, false),
                p(concat!("flt", $f, ".key"), "unda.keytrack", -1.0, 1.0, 0.0, "%", false, false),
                p(concat!("flt", $f, ".stereo"), "unda.stereo", -2.0, 2.0, 0.0, "oct", false, false),
                p(concat!("flt", $f, ".mix"), "unda.mix", 0.0, 1.0, 1.0, "%", false, false),
            )*
            $(
                p(concat!("env", $e, ".delay"), "unda.env_delay", 0.0, 10.0, 0.0, "s", false, false),
                p(concat!("env", $e, ".a"), "unda.attack", 0.0005, 20.0, $a, "s", true, false),
                p(concat!("env", $e, ".h"), "unda.hold", 0.0, 10.0, 0.0, "s", false, false),
                p(concat!("env", $e, ".d"), "unda.decay", 0.001, 30.0, $d, "s", true, false),
                p(concat!("env", $e, ".s"), "unda.sustain", 0.0, 1.0, $s, "%", false, false),
                p(concat!("env", $e, ".r"), "unda.release", 0.001, 30.0, $r, "s", true, false),
                p(concat!("env", $e, ".ac"), "unda.a_curve", -1.0, 1.0, 0.0, "", false, false),
                p(concat!("env", $e, ".dc"), "unda.d_curve", -1.0, 1.0, -0.6, "", false, false),
                p(concat!("env", $e, ".rc"), "unda.r_curve", -1.0, 1.0, -0.6, "", false, false),
            )*
            $(
                p(concat!("lfo", $l, ".wave"), "unda.lfo_wave", 0.0, 7.0, 0.0, "", false, true),
                p(concat!("lfo", $l, ".rate"), "unda.rate", 0.01, 50.0, 2.0, "Hz", true, false),
                p(concat!("lfo", $l, ".mode"), "unda.lfo_mode", 0.0, 2.0, 0.0, "", false, true),
                p(concat!("lfo", $l, ".phase"), "unda.phase", 0.0, 360.0, 0.0, "°", false, false),
                p(concat!("lfo", $l, ".delay"), "unda.lfo_delay", 0.0, 10.0, 0.0, "s", false, false),
                p(concat!("lfo", $l, ".rise"), "unda.rise", 0.0, 10.0, 0.0, "s", false, false),
                p(concat!("lfo", $l, ".smooth"), "unda.smooth", 0.0, 1.0, 0.0, "%", false, false),
            )*
        ]
    };
}

pub const PARAMS: &[InstParamInfo] = unda_params!(
    osc: [(1, 1.0, 1.0), (2, 0.0, 2.0), (3, 0.0, 0.0)],
    flt: [(1, 2_000.0), (2, 8_000.0)],
    env: [
        (1, 0.002, 0.6, 1.0, 0.12),
        (2, 0.002, 0.4, 0.0, 0.3),
        (3, 0.002, 0.4, 0.0, 0.3),
        (4, 0.002, 0.4, 0.0, 0.3)
    ],
    lfo: [1, 2, 3, 4, 5, 6, 7, 8]
);

pub fn params() -> &'static [InstParamInfo] {
    PARAMS
}

/// The modulation sources it produces (tags from `modulation::SOURCES`): every one.
pub const SOURCES: &[&str] = &[
    "env1",
    "env2",
    "env3",
    "env4",
    "lfo1",
    "lfo2",
    "lfo3",
    "lfo4",
    "lfo5",
    "lfo6",
    "lfo7",
    "lfo8",
    "vel",
    "relvel",
    "key",
    "rnd",
    "mpe.bend",
    "mpe.press",
    "mpe.slide",
    "macro1",
    "macro2",
    "macro3",
    "macro4",
    "macro5",
    "macro6",
    "macro7",
    "macro8",
    "mw",
    "at",
    "pb",
    "expr",
    "sus",
];

pub fn param(key: &str) -> Option<&'static InstParamInfo> {
    find(PARAMS, key).or_else(|| super::modulation::param(key))
}

/// `osc{n}.table` (a file), `osc{n}.name` (its name), `lfo{n}.shape` (a drawn shape).
pub fn is_text_key(key: &str) -> bool {
    super::modulation::is_text_key(key)
        || TABLE_KEYS.contains(&key)
        || NAME_KEYS.contains(&key)
        || SHAPE_KEYS.contains(&key)
}

/// The imported tables are the assets: the engine decodes them before the snapshot.
pub fn is_asset_key(key: &str) -> bool {
    TABLE_KEYS.contains(&key)
}

/// `TABLE_KEYS[n - 1]`: oscillator *n*'s imported table (a file path).
pub const TABLE_KEYS: [&str; OSCS] = ["osc1.table", "osc2.table", "osc3.table"];
/// `NAME_KEYS[n - 1]`: the imported table's display name.
pub const NAME_KEYS: [&str; OSCS] = ["osc1.name", "osc2.name", "osc3.name"];
/// `SHAPE_KEYS[n - 1]`: LFO *n*'s drawn shape.
pub const SHAPE_KEYS: [&str; LFOS] = [
    "lfo1.shape",
    "lfo2.shape",
    "lfo3.shape",
    "lfo4.shape",
    "lfo5.shape",
    "lfo6.shape",
    "lfo7.shape",
    "lfo8.shape",
];

/// A drawn LFO shape's points, parsed from its text without allocating: `(x, y)` in
/// 0…1, sorted by x, at most [`LFO_POINTS`]. Returns how many were read; malformed
/// pairs are skipped.
pub fn parse_shape(text: &str, out: &mut [(f32, f32); LFO_POINTS]) -> usize {
    let mut n = 0;
    for pair in text.split(';') {
        if n == LFO_POINTS {
            break;
        }
        let mut it = pair.split(',');
        let (Some(x), Some(y)) = (it.next(), it.next()) else {
            continue;
        };
        let (Ok(x), Ok(y)) = (x.trim().parse::<f32>(), y.trim().parse::<f32>()) else {
            continue;
        };
        if !(x.is_finite() && y.is_finite()) {
            continue;
        }
        let x = x.clamp(0.0, 1.0);
        if n > 0 && x < out[n - 1].0 {
            continue;
        }
        out[n] = (x, y.clamp(0.0, 1.0));
        n += 1;
    }
    n
}

/// A shape as its text, the inverse of [`parse_shape`] (three decimals).
pub fn format_shape(points: &[(f32, f32)]) -> String {
    points
        .iter()
        .take(LFO_POINTS)
        .map(|(x, y)| format!("{:.3},{:.3}", x.clamp(0.0, 1.0), y.clamp(0.0, 1.0)))
        .collect::<Vec<_>>()
        .join(";")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::inst::{InstrumentKind, NativeSpec};

    #[test]
    fn the_lists_match_the_ranges_they_index() {
        let kind = InstrumentKind::Wavetable;
        let max = |k: &str| kind.param(k).unwrap().max as usize;
        assert_eq!(max("osc1.wt") + 1, TABLES.len());
        assert_eq!(max("osc2.warp1") + 1, WARPS.len());
        assert_eq!(max("osc3.warp2.src") + 1, WARP_SOURCES.len());
        assert_eq!(max("osc1.route") + 1, ROUTES.len());
        assert_eq!(max("flt2.type") + 1, FILTERS.len());
        assert_eq!(max("osc1.ucurve") + 1, UNISON_CURVES.len());
        assert_eq!(max("osc1.stack") + 1, STACKS.len());
        assert_eq!(max("sub.wave") + 1, SUB_WAVES.len());
        assert_eq!(max("osc2.engine") + 1, ENGINES.len());
        assert_eq!(max("osc3.gwin") + 1, GRAIN_WINDOWS.len());
        assert_eq!(max("lfo8.wave") + 1, LFO_WAVES.len());
        assert_eq!(max("lfo1.mode") + 1, LFO_MODES.len());
        assert_eq!(max("voice.mode") + 1, VOICE_MODES.len());
        assert_eq!(max("quality") + 1, QUALITY.len());
        assert_eq!(SOURCES.len(), super::super::modulation::SOURCES.len());
    }

    #[test]
    fn tables_are_assets_and_shapes_are_text() {
        let kind = InstrumentKind::Wavetable;
        assert!(kind.is_asset_key("osc1.table") && kind.is_text_key("osc3.table"));
        assert!(kind.is_text_key("lfo8.shape") && !kind.is_asset_key("lfo8.shape"));
        assert!(kind.is_text_key("osc2.name") && !kind.is_asset_key("osc2.name"));
        let mut s = NativeSpec::new(kind);
        s.set_text("osc2.table", Some("a.wav".into())).unwrap();
        assert_eq!(
            s.assets().collect::<Vec<_>>(),
            vec![("osc2.table", "a.wav")]
        );
    }

    #[test]
    fn a_shape_round_trips_through_its_text() {
        let pts = [(0.0, 0.5), (0.25, 1.0), (0.75, 0.0), (1.0, 0.5)];
        let text = format_shape(&pts);
        let mut out = [(0.0, 0.0); LFO_POINTS];
        assert_eq!(parse_shape(&text, &mut out), 4);
        assert_eq!(&out[..4], &pts);
        assert_eq!(parse_shape("junk;0.5,x;0.2,0.3;0.1,0.9", &mut out), 1);
    }
}
