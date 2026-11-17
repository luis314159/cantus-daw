//! Native insert effects: their *parameters*, not their DSP (see `cantus-dsp`).
//!
//! Effects use a generic parameter model — a kind plus `key → f32` — so the UI, the command
//! layer and the AI bridge treat every effect (and, later, every plugin) the same way.

pub use crate::external::{ExternalPlugin, PluginFormat, PluginState};
use crate::fx;
use crate::model::{EffectId, StripId, TrackId};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum EffectKind {
    Delay,
    Reverb,
    /// Aequalis, the parametric EQ: [`EQ_BANDS`] bands addressed as `b{n}.{field}`.
    Eq,
    /// Comprimo, the compressor. Can be keyed from another strip ([`EffectSlot::sidechain`]).
    Compressor,
    /// Terminus, the limiter (`fx::limiter`).
    Limiter,
    /// Spatium, the reverb (`fx::space`). The older `Reverb` stays for the projects that
    /// use it.
    Space,
    /// Multiplex, the multiband dynamics (`fx::multiband`).
    Multiband,
    /// Sibilus, the de-esser (`fx::deesser`).
    DeEsser,
    /// Porta, the gate and expander (`fx::gate`).
    Gate,
    /// Fervor, saturation and colour: tape, tube, overdrive, a drawn waveshaper
    /// (`fx::saturator`, ADR 0026).
    Saturator,
    /// Latitudo, stereo control: width, mid/side, mono bass, balance (`fx::stereo`).
    Stereo,
    /// Culmen, the mastering maximizer: multiband dynamics into a limiter
    /// (`fx::maximizer`), FL's Maximus to our bar.
    Maximizer,
    /// A plug-in made to a market standard (CLAP, VST3), hosted (ADR 0019). What it is and
    /// its parameters are in [`EffectSpec::external`]; `params` stays empty.
    External,
    /// Concentus, the chorus (Juno, Dimension) (`fx::concentus`, round 28).
    Chorus,
    /// Flexus, the through-zero flanger (`fx::flexus`, round 28).
    Flanger,
    /// Phasis, the phaser (`fx::phasis`, round 28).
    Phaser,
    /// Tremor, tremolo and auto-pan (`fx::tremor`, round 28).
    Tremolo,
    /// Vibrans, the vibrato (`fx::vibrans`, round 28).
    Vibrato,
    /// Anulus, the ring modulator (`fx::anulus`, round 28).
    RingMod,
    /// Translatio, the frequency shifter (`fx::translatio`, round 28).
    FreqShift,
    /// Fluctus, the LFO Tool-style volume shaper (`fx::fluctus`, round 28).
    VolumeShaper,
    /// Tonsor, the clipper, with true peak (`fx::tonsor`, round 28).
    Clipper,
    /// Impetus, the transient processor (`fx::impetus`, round 28).
    Transient,
    /// Discrimen, the transient splitter (`fx::discrimen`, round 28).
    TransientSplit,
    /// Speculum, the analyser (Wave Candy / SPAN class) (`fx::speculum`, round 28).
    Analyzer,
    /// Tonus, the real-time auto-tuner (`fx::tonus`, round 28).
    AutoTune,
    /// Aula, the convolution reverb (`fx::aula`, round 39, ADR 0112). // r39:conv
    Aula,
    // fx:contract — the fx side round's five (provisional ADRs 0130–0133).
    /// Cribrum, the auto filter (`fx::cribrum`, fx side round). // fx:contract
    AutoFilter, // fx:contract
    /// Fragor, the distortion and fuzz pedals (`fx::fragor`, fx side round). // fx:contract
    Pedal, // fx:contract
    /// Lapsus, the time and volume shaper (`fx::lapsus`, fx side round). // fx:contract
    TimeShaper, // fx:contract
    /// Vetus, lo-fi: bits, rate, vinyl, tape wear (`fx::vetus`, fx side round). // fx:contract
    LoFi, // fx:contract
    /// Libra, the utility: gain, polarity, channels, alignment (`fx::libra`). // fx:contract
    Utility, // fx:contract
    // fx3:contract — the side round's wave 3 (provisional ADRs 0134–0137).
    /// Tabula, the pedalboard: native units as pedals ([`EffectSpec::board`]). // fx3:contract
    Board, // fx3:contract
    /// Saltus, pitch: octaver, shifter, harmoniser, whammy (`fx::saltus`). // fx3:contract
    PitchShift, // fx3:contract
    /// Rota, the rotary speaker (`fx::rota`). // fx3:contract
    Rotary, // fx3:contract
    /// Arca, amp and cabinet (`fx::arca`). // fx3:contract
    Amp, // fx3:contract
    // fxa:contract — the side round's wave A (provisional ADRs 0139–0141).
    /// Loquax, the vocoder (`fx::loquax`). // fxa:contract
    Vocoder, // fxa:contract
    /// Fractus, the glitch sequencer and grain cloud (`fx::fractus`, [`EffectSpec::fractus`]). // fxa:contract
    Glitch, // fxa:contract
    /// Caelum, the shimmer reverb (`fx::caelum`). // fxa:contract
    Shimmer, // fxa:contract
    // fxc:contract — the side round's wave C (provisional ADRs 0145–0146).
    /// Purus, the real-time repair unit: de-hum, de-click and de-crackle, de-reverb (`fx::purus`). // fxc:contract
    Repair, // fxc:contract
    /// Comparatio, the reference comparator: a reference track at the mix's loudness, A/B and blind (`fx::comparatio`, its file in [`EffectSpec::ir`]). // fxc:contract
    Reference, // fxc:contract
}

pub use fx::compressor::COMPRESSOR_PARAMS;
pub use fx::eq::{eq_key, BandField, EQ_BANDS, EQ_BAND_FIELDS};

/// Display/range metadata for one effect parameter.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct EffectParamInfo {
    pub key: &'static str,
    pub label_key: &'static str,
    pub min: f32,
    pub max: f32,
    pub default: f32,
    pub unit: &'static str,
    pub logarithmic: bool,
    pub stepped: bool,
}

#[allow(clippy::too_many_arguments)]
pub(crate) const fn p(
    key: &'static str,
    label_key: &'static str,
    min: f32,
    max: f32,
    default: f32,
    unit: &'static str,
    logarithmic: bool,
    stepped: bool,
) -> EffectParamInfo {
    EffectParamInfo {
        key,
        label_key,
        min,
        max,
        default,
        unit,
        logarithmic,
        stepped,
    }
}

pub const DELAY_PARAMS: &[EffectParamInfo] = &[
    p(
        "time_ms",
        "fx.delay.time",
        1.0,
        2000.0,
        375.0,
        "ms",
        true,
        false,
    ),
    p(
        "feedback",
        "fx.delay.feedback",
        0.0,
        0.95,
        0.4,
        "",
        false,
        false,
    ),
    p(
        "tone_hz",
        "fx.delay.tone",
        200.0,
        20_000.0,
        6_000.0,
        "Hz",
        true,
        false,
    ),
    p(
        "ping_pong",
        "fx.delay.ping_pong",
        0.0,
        1.0,
        1.0,
        "",
        false,
        true,
    ),
    p("mix", "fx.mix", 0.0, 1.0, 0.3, "", false, false),
];

pub const REVERB_PARAMS: &[EffectParamInfo] = &[
    p("size", "fx.reverb.size", 0.0, 1.0, 0.7, "", false, false),
    p(
        "damping",
        "fx.reverb.damping",
        0.0,
        1.0,
        0.4,
        "",
        false,
        false,
    ),
    p("width", "fx.reverb.width", 0.0, 1.0, 1.0, "", false, false),
    p(
        "predelay_ms",
        "fx.reverb.predelay",
        0.0,
        100.0,
        10.0,
        "ms",
        false,
        false,
    ),
    p("mix", "fx.mix", 0.0, 1.0, 0.25, "", false, false),
];

impl EffectKind {
    pub const ALL: [EffectKind; 40] = [
        // fxc:contract: 40 with wave C's two (wave B's three come before them at the merge)
        // fxa:contract: 38 with wave A's three
        // fx3:contract: 35 with wave 3's four
        // fx:contract: 31 with the side round's five
        // r39:conv: 26 with Aula
        EffectKind::Eq,
        EffectKind::Compressor,
        EffectKind::Limiter,
        EffectKind::Space,
        EffectKind::Multiband,
        EffectKind::DeEsser,
        EffectKind::Gate,
        EffectKind::Saturator,
        EffectKind::Stereo,
        EffectKind::Maximizer,
        EffectKind::Delay,
        EffectKind::Reverb,
        EffectKind::Chorus,
        EffectKind::Flanger,
        EffectKind::Phaser,
        EffectKind::Tremolo,
        EffectKind::Vibrato,
        EffectKind::RingMod,
        EffectKind::FreqShift,
        EffectKind::VolumeShaper,
        EffectKind::Clipper,
        EffectKind::Transient,
        EffectKind::TransientSplit,
        EffectKind::Analyzer,
        EffectKind::AutoTune,
        EffectKind::Aula,       // r39:conv
        EffectKind::AutoFilter, // fx:contract
        EffectKind::Pedal,      // fx:contract
        EffectKind::TimeShaper, // fx:contract
        EffectKind::LoFi,       // fx:contract
        EffectKind::Utility,    // fx:contract
        EffectKind::Board,      // fx3:contract
        EffectKind::PitchShift, // fx3:contract
        EffectKind::Rotary,     // fx3:contract
        EffectKind::Amp,        // fx3:contract
        EffectKind::Vocoder,    // fxa:contract
        EffectKind::Glitch,     // fxa:contract
        EffectKind::Shimmer,    // fxa:contract
        EffectKind::Repair,     // fxc:contract
        EffectKind::Reference,  // fxc:contract
    ];

    /// Whether the menus and the picker offer it for a new insert: the older Reverb is kept
    /// only for the projects that use it (Spatium is the reverb). // r37:library
    pub fn offered(self) -> bool {
        self != EffectKind::Reverb // r37:library
    }

    pub fn name_key(self) -> &'static str {
        match self {
            EffectKind::Delay => "effect.delay",
            EffectKind::Reverb => "effect.reverb",
            EffectKind::Eq => fx::eq::NAME_KEY,
            EffectKind::Compressor => fx::compressor::NAME_KEY,
            EffectKind::Limiter => fx::limiter::NAME_KEY,
            EffectKind::Space => fx::space::NAME_KEY,
            EffectKind::Multiband => fx::multiband::NAME_KEY,
            EffectKind::DeEsser => fx::deesser::NAME_KEY,
            EffectKind::Gate => fx::gate::NAME_KEY,
            EffectKind::Saturator => fx::saturator::NAME_KEY,
            EffectKind::Stereo => fx::stereo::NAME_KEY,
            EffectKind::Maximizer => fx::maximizer::NAME_KEY,
            EffectKind::External => crate::external::NAME_KEY,
            EffectKind::Chorus => fx::concentus::NAME_KEY,
            EffectKind::Flanger => fx::flexus::NAME_KEY,
            EffectKind::Phaser => fx::phasis::NAME_KEY,
            EffectKind::Tremolo => fx::tremor::NAME_KEY,
            EffectKind::Vibrato => fx::vibrans::NAME_KEY,
            EffectKind::RingMod => fx::anulus::NAME_KEY,
            EffectKind::FreqShift => fx::translatio::NAME_KEY,
            EffectKind::VolumeShaper => fx::fluctus::NAME_KEY,
            EffectKind::Clipper => fx::tonsor::NAME_KEY,
            EffectKind::Transient => fx::impetus::NAME_KEY,
            EffectKind::TransientSplit => fx::discrimen::NAME_KEY,
            EffectKind::Analyzer => fx::speculum::NAME_KEY,
            EffectKind::AutoTune => fx::tonus::NAME_KEY,
            EffectKind::Aula => fx::aula::NAME_KEY, // r39:conv
            EffectKind::AutoFilter => fx::cribrum::NAME_KEY, // fx:contract
            EffectKind::Pedal => fx::fragor::NAME_KEY, // fx:contract
            EffectKind::TimeShaper => fx::lapsus::NAME_KEY, // fx:contract
            EffectKind::LoFi => fx::vetus::NAME_KEY, // fx:contract
            EffectKind::Utility => fx::libra::NAME_KEY, // fx:contract
            EffectKind::Board => fx::tabula::NAME_KEY, // fx3:contract
            EffectKind::PitchShift => fx::saltus::NAME_KEY, // fx3:contract
            EffectKind::Rotary => fx::rota::NAME_KEY, // fx3:contract
            EffectKind::Amp => fx::arca::NAME_KEY,  // fx3:contract
            EffectKind::Vocoder => fx::loquax::NAME_KEY, // fxa:contract
            EffectKind::Glitch => fx::fractus::NAME_KEY, // fxa:contract
            EffectKind::Shimmer => fx::caelum::NAME_KEY, // fxa:contract
            EffectKind::Repair => fx::purus::NAME_KEY, // fxc:contract
            EffectKind::Reference => fx::comparatio::NAME_KEY, // fxc:contract
        }
    }

    pub fn params(self) -> &'static [EffectParamInfo] {
        match self {
            EffectKind::Delay => DELAY_PARAMS,
            EffectKind::Reverb => REVERB_PARAMS,
            EffectKind::Eq => fx::eq::params(),
            EffectKind::Compressor => fx::compressor::params(),
            EffectKind::Limiter => fx::limiter::params(),
            EffectKind::Space => fx::space::params(),
            EffectKind::Multiband => fx::multiband::params(),
            EffectKind::DeEsser => fx::deesser::params(),
            EffectKind::Gate => fx::gate::params(),
            EffectKind::Saturator => fx::saturator::params(),
            EffectKind::Stereo => fx::stereo::params(),
            EffectKind::Maximizer => fx::maximizer::params(),
            // The plug-in's own, by its ids, in `EffectSpec::external`.
            EffectKind::External => &[],
            EffectKind::Chorus => fx::concentus::params(),
            EffectKind::Flanger => fx::flexus::params(),
            EffectKind::Phaser => fx::phasis::params(),
            EffectKind::Tremolo => fx::tremor::params(),
            EffectKind::Vibrato => fx::vibrans::params(),
            EffectKind::RingMod => fx::anulus::params(),
            EffectKind::FreqShift => fx::translatio::params(),
            EffectKind::VolumeShaper => fx::fluctus::params(),
            EffectKind::Clipper => fx::tonsor::params(),
            EffectKind::Transient => fx::impetus::params(),
            EffectKind::TransientSplit => fx::discrimen::params(),
            EffectKind::Analyzer => fx::speculum::params(),
            EffectKind::AutoTune => fx::tonus::params(),
            EffectKind::Aula => fx::aula::params(), // r39:conv
            EffectKind::AutoFilter => fx::cribrum::params(), // fx:contract
            EffectKind::Pedal => fx::fragor::params(), // fx:contract
            EffectKind::TimeShaper => fx::lapsus::params(), // fx:contract
            EffectKind::LoFi => fx::vetus::params(), // fx:contract
            EffectKind::Utility => fx::libra::params(), // fx:contract
            EffectKind::Board => fx::tabula::params(), // fx3:contract
            EffectKind::PitchShift => fx::saltus::params(), // fx3:contract
            EffectKind::Rotary => fx::rota::params(), // fx3:contract
            EffectKind::Amp => fx::arca::params(),  // fx3:contract
            EffectKind::Vocoder => fx::loquax::params(), // fxa:contract
            EffectKind::Glitch => fx::fractus::params(), // fxa:contract
            EffectKind::Shimmer => fx::caelum::params(), // fxa:contract
            EffectKind::Repair => fx::purus::params(), // fxc:contract
            EffectKind::Reference => fx::comparatio::params(), // fxc:contract
        }
    }

    /// A kind with a large parameter space stores only what differs from the defaults,
    /// so an EQ with two bands is a few lines in the project file and not three hundred.
    pub fn sparse(self) -> bool {
        match self {
            EffectKind::Delay | EffectKind::Reverb => false,
            EffectKind::Eq => fx::eq::SPARSE,
            EffectKind::Compressor => fx::compressor::SPARSE,
            EffectKind::Limiter => fx::limiter::SPARSE,
            EffectKind::Space => fx::space::SPARSE,
            EffectKind::Multiband => fx::multiband::SPARSE,
            EffectKind::DeEsser => fx::deesser::SPARSE,
            EffectKind::Gate => fx::gate::SPARSE,
            EffectKind::Saturator => fx::saturator::SPARSE,
            EffectKind::Stereo => fx::stereo::SPARSE,
            EffectKind::Maximizer => fx::maximizer::SPARSE,
            EffectKind::External => true,
            EffectKind::Chorus => fx::concentus::SPARSE,
            EffectKind::Flanger => fx::flexus::SPARSE,
            EffectKind::Phaser => fx::phasis::SPARSE,
            EffectKind::Tremolo => fx::tremor::SPARSE,
            EffectKind::Vibrato => fx::vibrans::SPARSE,
            EffectKind::RingMod => fx::anulus::SPARSE,
            EffectKind::FreqShift => fx::translatio::SPARSE,
            EffectKind::VolumeShaper => fx::fluctus::SPARSE,
            EffectKind::Clipper => fx::tonsor::SPARSE,
            EffectKind::Transient => fx::impetus::SPARSE,
            EffectKind::TransientSplit => fx::discrimen::SPARSE,
            EffectKind::Analyzer => fx::speculum::SPARSE,
            EffectKind::AutoTune => fx::tonus::SPARSE,
            EffectKind::Aula => fx::aula::SPARSE, // r39:conv
            EffectKind::AutoFilter => fx::cribrum::SPARSE, // fx:contract
            EffectKind::Pedal => fx::fragor::SPARSE, // fx:contract
            EffectKind::TimeShaper => fx::lapsus::SPARSE, // fx:contract
            EffectKind::LoFi => fx::vetus::SPARSE, // fx:contract
            EffectKind::Utility => fx::libra::SPARSE, // fx:contract
            EffectKind::Board => fx::tabula::SPARSE, // fx3:contract
            EffectKind::PitchShift => fx::saltus::SPARSE, // fx3:contract
            EffectKind::Rotary => fx::rota::SPARSE, // fx3:contract
            EffectKind::Amp => fx::arca::SPARSE,  // fx3:contract
            EffectKind::Vocoder => fx::loquax::SPARSE, // fxa:contract
            EffectKind::Glitch => fx::fractus::SPARSE, // fxa:contract
            EffectKind::Shimmer => fx::caelum::SPARSE, // fxa:contract
            EffectKind::Repair => fx::purus::SPARSE, // fxc:contract
            EffectKind::Reference => fx::comparatio::SPARSE, // fxc:contract
        }
    }

    /// A kind with an editor of its own, rather than a row of knobs per parameter. Every
    /// kind but the old Reverb (Delay has had its window since round 39, ADR 0115).
    pub fn has_editor(self) -> bool {
        !matches!(self, EffectKind::Reverb)
    }

    /// A kind that plays an impulse response from a file ([`EffectSpec::ir`],
    /// `Command::SetEffectIr`): Aula, and Arca's cabinet once its builder says so.
    /// // fx3:contract
    pub fn takes_ir(self) -> bool {
        match self {
            EffectKind::Aula => true,
            EffectKind::Amp => true, // fx3:arca — the cabinet's user IR
            EffectKind::Reference => true, // fxc:comparatio — the reference track
            _ => false,
        }
    }

    /// A kind whose detector can listen to another strip.
    pub fn takes_sidechain(self) -> bool {
        match self {
            EffectKind::Delay | EffectKind::Reverb => false,
            EffectKind::Eq => fx::eq::TAKES_SIDECHAIN,
            EffectKind::Compressor => fx::compressor::TAKES_SIDECHAIN,
            EffectKind::Limiter => fx::limiter::TAKES_SIDECHAIN,
            EffectKind::Space => fx::space::TAKES_SIDECHAIN,
            EffectKind::Multiband => fx::multiband::TAKES_SIDECHAIN,
            EffectKind::DeEsser => fx::deesser::TAKES_SIDECHAIN,
            EffectKind::Gate => fx::gate::TAKES_SIDECHAIN,
            EffectKind::Saturator => fx::saturator::TAKES_SIDECHAIN,
            EffectKind::Stereo => fx::stereo::TAKES_SIDECHAIN,
            EffectKind::Maximizer => fx::maximizer::TAKES_SIDECHAIN,
            // Per plug-in, not per kind: `EffectSpec::takes_sidechain`.
            EffectKind::External => false,
            EffectKind::Chorus => fx::concentus::TAKES_SIDECHAIN,
            EffectKind::Flanger => fx::flexus::TAKES_SIDECHAIN,
            EffectKind::Phaser => fx::phasis::TAKES_SIDECHAIN,
            EffectKind::Tremolo => fx::tremor::TAKES_SIDECHAIN,
            EffectKind::Vibrato => fx::vibrans::TAKES_SIDECHAIN,
            EffectKind::RingMod => fx::anulus::TAKES_SIDECHAIN,
            EffectKind::FreqShift => fx::translatio::TAKES_SIDECHAIN,
            EffectKind::VolumeShaper => fx::fluctus::TAKES_SIDECHAIN,
            EffectKind::Clipper => fx::tonsor::TAKES_SIDECHAIN,
            EffectKind::Transient => fx::impetus::TAKES_SIDECHAIN,
            EffectKind::TransientSplit => fx::discrimen::TAKES_SIDECHAIN,
            EffectKind::Analyzer => fx::speculum::TAKES_SIDECHAIN,
            EffectKind::AutoTune => fx::tonus::TAKES_SIDECHAIN,
            EffectKind::Aula => fx::aula::TAKES_SIDECHAIN, // r39:conv
            EffectKind::AutoFilter => fx::cribrum::TAKES_SIDECHAIN, // fx:contract
            EffectKind::Pedal => fx::fragor::TAKES_SIDECHAIN, // fx:contract
            EffectKind::TimeShaper => fx::lapsus::TAKES_SIDECHAIN, // fx:contract
            EffectKind::LoFi => fx::vetus::TAKES_SIDECHAIN, // fx:contract
            EffectKind::Utility => fx::libra::TAKES_SIDECHAIN, // fx:contract
            EffectKind::Board => fx::tabula::TAKES_SIDECHAIN, // fx3:contract
            EffectKind::PitchShift => fx::saltus::TAKES_SIDECHAIN, // fx3:contract
            EffectKind::Rotary => fx::rota::TAKES_SIDECHAIN, // fx3:contract
            EffectKind::Amp => fx::arca::TAKES_SIDECHAIN,  // fx3:contract
            EffectKind::Vocoder => fx::loquax::TAKES_SIDECHAIN, // fxa:contract
            EffectKind::Glitch => fx::fractus::TAKES_SIDECHAIN, // fxa:contract
            EffectKind::Shimmer => fx::caelum::TAKES_SIDECHAIN, // fxa:contract
            EffectKind::Repair => fx::purus::TAKES_SIDECHAIN, // fxc:contract
            EffectKind::Reference => fx::comparatio::TAKES_SIDECHAIN, // fxc:contract
        }
    }

    /// fxa:loquax — a key that is the editor's look, not the sound (Loquax's microphone,
    /// ADR 0139): kept per insert like any key and never heard. A preset that names it sets
    /// it (each factory preset suggests its object; a user's preset keeps the one it was
    /// saved with), one that does not leaves it; A/B leaves it; and a value off a preset's
    /// does not make the preset "modified".
    pub fn is_look(self, key: &str) -> bool {
        match self {
            EffectKind::Vocoder => fx::loquax::LOOK.contains(&key),
            _ => false,
        }
    }

    /// A kind that can hear a track's notes ([`EffectSlot::midi_from`]).
    pub fn takes_notes(self) -> bool {
        match self {
            EffectKind::Gate => fx::gate::TAKES_NOTES,
            EffectKind::Chorus => fx::concentus::TAKES_NOTES,
            EffectKind::Flanger => fx::flexus::TAKES_NOTES,
            EffectKind::Phaser => fx::phasis::TAKES_NOTES,
            EffectKind::Tremolo => fx::tremor::TAKES_NOTES,
            EffectKind::Vibrato => fx::vibrans::TAKES_NOTES,
            EffectKind::RingMod => fx::anulus::TAKES_NOTES,
            EffectKind::FreqShift => fx::translatio::TAKES_NOTES,
            EffectKind::VolumeShaper => fx::fluctus::TAKES_NOTES,
            EffectKind::Clipper => fx::tonsor::TAKES_NOTES,
            EffectKind::Transient => fx::impetus::TAKES_NOTES,
            EffectKind::TransientSplit => fx::discrimen::TAKES_NOTES,
            EffectKind::Analyzer => fx::speculum::TAKES_NOTES,
            EffectKind::AutoTune => fx::tonus::TAKES_NOTES,
            EffectKind::Aula => fx::aula::TAKES_NOTES, // r39:conv
            EffectKind::AutoFilter => fx::cribrum::TAKES_NOTES, // fx:contract
            EffectKind::Pedal => fx::fragor::TAKES_NOTES, // fx:contract
            EffectKind::TimeShaper => fx::lapsus::TAKES_NOTES, // fx:contract
            EffectKind::LoFi => fx::vetus::TAKES_NOTES, // fx:contract
            EffectKind::Utility => fx::libra::TAKES_NOTES, // fx:contract
            EffectKind::Board => fx::tabula::TAKES_NOTES, // fx3:contract
            EffectKind::PitchShift => fx::saltus::TAKES_NOTES, // fx3:contract
            EffectKind::Rotary => fx::rota::TAKES_NOTES, // fx3:contract
            EffectKind::Amp => fx::arca::TAKES_NOTES,  // fx3:contract
            EffectKind::Vocoder => fx::loquax::TAKES_NOTES, // fxa:contract
            EffectKind::Glitch => fx::fractus::TAKES_NOTES, // fxa:contract
            EffectKind::Shimmer => fx::caelum::TAKES_NOTES, // fxa:contract
            EffectKind::Repair => fx::purus::TAKES_NOTES, // fxc:contract
            EffectKind::Reference => fx::comparatio::TAKES_NOTES, // fxc:contract
            _ => false,
        }
    }

    /// The locale prefix of a stepped parameter's choices (`{prefix}.{i}`), by key, for
    /// the kinds that keep that table in their own module (round 28 on): what the
    /// exported plug-in shows a host (`cantus_plugins::params`). Empty for the older
    /// kinds, whose prefixes `cantus_plugins::params` lists itself.
    pub fn choices(self) -> &'static [(&'static str, &'static str)] {
        match self {
            EffectKind::Chorus => fx::concentus::CHOICES,
            EffectKind::Flanger => fx::flexus::CHOICES,
            EffectKind::Phaser => fx::phasis::CHOICES,
            EffectKind::Tremolo => fx::tremor::CHOICES,
            EffectKind::Vibrato => fx::vibrans::CHOICES,
            EffectKind::RingMod => fx::anulus::CHOICES,
            EffectKind::FreqShift => fx::translatio::CHOICES,
            EffectKind::VolumeShaper => fx::fluctus::CHOICES,
            EffectKind::Clipper => fx::tonsor::CHOICES,
            EffectKind::Transient => fx::impetus::CHOICES,
            EffectKind::TransientSplit => fx::discrimen::CHOICES,
            EffectKind::Analyzer => fx::speculum::CHOICES,
            EffectKind::AutoTune => fx::tonus::CHOICES,
            EffectKind::Aula => fx::aula::CHOICES, // r39:conv
            EffectKind::AutoFilter => fx::cribrum::CHOICES, // fx:contract
            EffectKind::Pedal => fx::fragor::CHOICES, // fx:contract
            EffectKind::TimeShaper => fx::lapsus::CHOICES, // fx:contract
            EffectKind::LoFi => fx::vetus::CHOICES, // fx:contract
            EffectKind::Utility => fx::libra::CHOICES, // fx:contract
            EffectKind::Board => fx::tabula::CHOICES, // fx3:contract
            EffectKind::PitchShift => fx::saltus::CHOICES, // fx3:contract
            EffectKind::Rotary => fx::rota::CHOICES, // fx3:contract
            EffectKind::Amp => fx::arca::CHOICES,  // fx3:contract
            EffectKind::Vocoder => fx::loquax::CHOICES, // fxa:contract
            EffectKind::Glitch => fx::fractus::CHOICES, // fxa:contract
            EffectKind::Shimmer => fx::caelum::CHOICES, // fxa:contract
            EffectKind::Repair => fx::purus::CHOICES, // fxc:contract
            EffectKind::Reference => fx::comparatio::CHOICES, // fxc:contract
            _ => &[],
        }
    }

    pub fn param(self, key: &str) -> Option<&'static EffectParamInfo> {
        self.params().iter().find(|i| i.key == key)
    }
}

/// An effect's kind and parameter values. Unknown keys are ignored on load; missing keys
/// read as their default, so adding a parameter never breaks an old file.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct EffectSpec {
    pub kind: EffectKind,
    pub params: BTreeMap<String, f32>,
    /// The hosted plug-in, for [`EffectKind::External`]; `None` for every native kind, and
    /// then not written, so a native effect's JSON is what it always was.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub external: Option<Box<ExternalPlugin>>,
    /// Aula's response from a file (round 39, ADR 0112; schema 60); `None` for every other
    /// kind and for Aula playing one of Cantus's own, and then not written. // r39:conv
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ir: Option<Box<fx::aula::IrFile>>,
    /// Lapsus's patterns where they differ from its factory bank (ADR 0132, provisional);
    /// `None` for every other kind and for a Lapsus on the factory bank, and then not
    /// written. // fx:lapsus
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lapsus: Option<Box<fx::lapsus::Bank>>,
    /// Tabula's pedals and macros (ADR 0134, provisional; schema 63); `None` for every other
    /// kind and for an empty board, and then not written. Its pedals' parameters are
    /// addressed `p{id}.{key}` through the routed API ([`EffectSpec::param`]). // fx3:contract
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub board: Option<Box<fx::tabula::Board>>,
    /// Fractus's painted lanes (ADR 0140, provisional; schema 64); `None` for every other
    /// kind and for a pattern with nothing painted, and then not written. // fxa:contract
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fractus: Option<Box<fx::fractus::Pattern>>,
    /// Comparatio's references (ADR 0146 phase B; schema 66): the list and the Aequalis its
    /// match EQ was sent to; the one compared is [`EffectSpec::ir`]. `None` for every other
    /// kind and for an empty list, and then not written. // fxc:comparatio
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub refs: Option<Box<fx::comparatio::References>>,
}

impl EffectSpec {
    pub fn new(kind: EffectKind) -> Self {
        let params = if kind.sparse() {
            BTreeMap::new()
        } else {
            kind.params()
                .iter()
                .map(|i| (i.key.to_string(), i.default))
                .collect()
        };
        EffectSpec {
            kind,
            params,
            external: None,
            ir: None,      // r39:conv
            lapsus: None,  // fx:lapsus
            board: None,   // fx3:contract
            fractus: None, // fxa:contract
            refs: None,    // fxc:comparatio
        }
    }

    /// A hosted plug-in's spec.
    pub fn external(plugin: ExternalPlugin) -> Self {
        EffectSpec {
            kind: EffectKind::External,
            params: BTreeMap::new(),
            external: Some(Box::new(plugin)),
            ir: None,      // r39:conv
            lapsus: None,  // fx:lapsus
            board: None,   // fx3:contract
            fractus: None, // fxa:contract
            refs: None,    // fxc:comparatio
        }
    }

    /// Whether this effect can be keyed from another strip: by kind for the native ones,
    /// by what the scan found for a hosted plug-in (a second audio input), and for a board
    /// when one of its pedals can (fx3:contract).
    pub fn takes_sidechain(&self) -> bool {
        self.kind.takes_sidechain()
            || self.external.as_ref().is_some_and(|e| e.sidechain)
            || self.pedal_specs().any(|s| s.takes_sidechain())
    }

    /// Whether this effect can hear a track's notes: by kind, and for a board when one of
    /// its pedals can (fx3:contract).
    pub fn takes_notes(&self) -> bool {
        self.kind.takes_notes() || self.pedal_specs().any(|s| s.takes_notes())
    }

    // ---- the routed API (fx3:contract, ADR 0134): a board's `p{id}.{key}` is its pedal's
    // ---- `key`; every other key, and every key of every other kind, is the spec's own.

    /// The pedals' specs of a board (none for every other kind).
    fn pedal_specs(&self) -> impl Iterator<Item = &EffectSpec> {
        self.pedals().iter().map(|p| &p.spec)
    }

    /// The spec that holds `key` and its key there: a board's pedal for `p{id}.…`, this
    /// spec otherwise; `None` for a pedal the board does not hold. No allocation.
    fn leaf<'k>(&self, key: &'k str) -> Option<(&EffectSpec, &'k str)> {
        if self.kind == EffectKind::Board {
            if let Some((id, rest)) = fx::tabula::split_key(key) {
                let p = self.board.as_deref()?.pedal(id)?;
                return Some((&p.spec, rest));
            }
        }
        Some((self, key))
    }

    fn leaf_mut<'k>(&mut self, key: &'k str) -> Option<(&mut EffectSpec, &'k str)> {
        if self.kind == EffectKind::Board {
            if let Some((id, rest)) = fx::tabula::split_key(key) {
                let p = self.board.as_deref_mut()?.pedal_mut(id)?;
                return Some((&mut p.spec, rest));
            }
        }
        Some((self, key))
    }

    /// The kind and the key inside it that `key` addresses (a board's pedal's own, for
    /// `p{id}.…`) — what a choice's words or a readout are looked up by.
    pub fn resolve<'k>(&self, key: &'k str) -> Option<(EffectKind, &'k str)> {
        self.leaf(key).map(|(s, k)| (s.kind, k))
    }

    /// The parameter `key` addresses: the kind's, or a board's pedal's.
    pub fn param(&self, key: &str) -> Option<&'static EffectParamInfo> {
        let (s, k) = self.leaf(key)?;
        s.kind.param(k)
    }

    /// A parameter's value: as stored, else its default (0 for a key nothing has).
    pub fn get(&self, key: &str) -> f32 {
        let Some((s, k)) = self.leaf(key) else {
            return 0.0;
        };
        match s.params.get(k) {
            Some(v) => *v,
            None => s.kind.param(k).map(|i| i.default).unwrap_or(0.0),
        }
    }

    /// A parameter's value as stored, `None` when it is not (it reads as its default).
    pub fn stored(&self, key: &str) -> Option<f32> {
        let (s, k) = self.leaf(key)?;
        s.params.get(k).copied()
    }

    /// Set a parameter, clamping to its range. `None` for a key the effect does not have.
    pub fn set(&mut self, key: &str, value: f32) -> Option<f32> {
        let (s, k) = self.leaf_mut(key)?;
        let info = s.kind.param(k)?;
        let v = if value.is_finite() {
            value.clamp(info.min, info.max)
        } else {
            info.default
        };
        let v = if info.stepped { v.round() } else { v };
        s.params.insert(k.to_string(), v);
        Some(v)
    }

    /// Take a parameter back to "never set"; the value it had stored, if any.
    pub fn clear(&mut self, key: &str) -> Option<f32> {
        let (s, k) = self.leaf_mut(key)?;
        s.params.remove(k)
    }

    /// The stored value of `key`, to write in place: never inserts (the audio thread's
    /// automation and macros write through this). `None` when it is not stored.
    pub fn value_mut(&mut self, key: &str) -> Option<&mut f32> {
        let (s, k) = self.leaf_mut(key)?;
        s.params.get_mut(k)
    }

    /// Store `key` at its current value so later writes are writes ([`Self::value_mut`]),
    /// never insertions. Allocates: off the audio thread. `false` for a key the effect does
    /// not have.
    pub fn ensure(&mut self, key: &str) -> bool {
        let Some((s, k)) = self.leaf_mut(key) else {
            return false;
        };
        if s.kind.param(k).is_none() {
            return false;
        }
        if !s.params.contains_key(k) {
            let v = s.get(k);
            s.params.insert(k.to_string(), v);
        }
        true
    }

    /// Every key the spec addresses: its kind's, then each pedal's `p{id}.{key}` in signal
    /// order. Allocates.
    pub fn param_keys(&self) -> Vec<String> {
        let mut v: Vec<String> = self.kind.params().iter().map(|i| i.key.into()).collect();
        for p in self.pedals() {
            v.extend(
                p.spec
                    .kind
                    .params()
                    .iter()
                    .map(|i| fx::tabula::pedal_key(p.id, i.key)),
            );
        }
        v
    }

    /// A board's pedals in signal order (none for every other kind).
    pub fn pedals(&self) -> &[fx::tabula::Pedal] {
        match self.board.as_deref() {
            Some(b) if self.kind == EffectKind::Board => &b.pedals,
            _ => &[],
        }
    }

    /// The stored values by their keys as the routed API names them: the spec's own map,
    /// with a board's pedals' stored values added as `p{id}.{key}` (borrowed when there
    /// are none — every kind but a board with pedals).
    pub fn flat_params(&self) -> std::borrow::Cow<'_, BTreeMap<String, f32>> {
        if self.pedals().is_empty() {
            return std::borrow::Cow::Borrowed(&self.params);
        }
        let mut m = self.params.clone();
        for p in self.pedals() {
            for (k, v) in &p.spec.params {
                m.insert(fx::tabula::pedal_key(p.id, k), *v);
            }
        }
        std::borrow::Cow::Owned(m)
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct EffectSlot {
    pub id: EffectId,
    pub bypass: bool,
    pub spec: EffectSpec,
    /// The slot's own dry/wet, 0…1, outside the effect: at 1 the effect's output is what
    /// leaves the slot, at 0 the slot is a wire. FL has this knob on every slot, and it
    /// is what makes any effect a parallel one without a second strip. Unity is skipped
    /// entirely on the audio thread, so an old file renders exactly as it did.
    #[serde(default = "unit_mix")]
    pub mix: f32,
    /// The strip whose post-fader output this effect's detector listens to, instead of its
    /// own input — a sidechain, chosen inside the effect with no send to set up. A route
    /// like an output or a send: it orders the strips and it cannot close a loop.
    #[serde(default)]
    pub sidechain: Option<StripId>,
    /// The track whose notes this effect hears — a gate opened by a MIDI part, chosen
    /// inside the effect (ADR 0022). Not a route for audio: it orders nothing, and a track
    /// that is removed simply stops sending (undoing the removal brings it back). The
    /// track's own mute silences its instrument, not its notes. `None` (the default, and
    /// every file before schema 14) sends nothing, and the effect renders as before.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub midi_from: Option<TrackId>,
}

fn unit_mix() -> f32 {
    1.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_and_clamping() {
        for kind in EffectKind::ALL {
            let mut s = EffectSpec::new(kind);
            for info in kind.params() {
                assert_eq!(s.get(info.key), info.default);
                assert_eq!(s.set(info.key, info.max + 1.0), Some(info.max));
                assert_eq!(s.set(info.key, f32::NAN), Some(info.default));
            }
            assert!(s.set("nope", 1.0).is_none());
        }
    }

    #[test]
    fn the_eq_names_its_bands_stably_and_stores_only_what_moved() {
        let mut s = EffectSpec::new(EffectKind::Eq);
        assert!(s.params.is_empty(), "a new EQ writes nothing");
        assert_eq!(
            EffectKind::Eq.params().len(),
            EQ_BANDS * EQ_BAND_FIELDS.len() + fx::eq::EQ_GLOBALS.len()
        );
        assert_eq!(s.get("b23.freq"), 1_000.0);
        assert_eq!(s.set("b3.gain", 40.0), Some(30.0));
        assert_eq!(
            s.set("b3.shape", 2.4),
            Some(2.0),
            "a shape is a whole number"
        );
        assert!(
            s.set("b24.gain", 1.0).is_none(),
            "there are 24 bands, 0 to 23"
        );
        assert_eq!(s.params.len(), 2);
    }

    /// fx3:contract — a board's `p{id}.{key}` reaches its pedal: read, set (clamped),
    /// stored, cleared, written in place, ensured; a missing pedal is nothing, and the
    /// board's own keys and every other kind's are what they always were.
    #[test]
    fn a_boards_pedal_keys_are_routed_to_its_pedals() {
        use fx::tabula::{Board, Pedal, PedalId};
        let mut s = EffectSpec::new(EffectKind::Board);
        assert!(s.param("p0.time_ms").is_none(), "no pedal yet");
        assert_eq!(s.get("p0.time_ms"), 0.0);
        assert!(s.set("p0.time_ms", 1.0).is_none());
        s.board = Some(Box::new(Board {
            pedals: vec![
                Pedal::new(PedalId(0), EffectSpec::new(EffectKind::Delay)),
                Pedal::new(PedalId(12), EffectSpec::new(EffectKind::Eq)),
            ],
            macros: Vec::new(),
        }));
        assert_eq!(
            s.param("p0.time_ms"),
            EffectKind::Delay.param("time_ms"),
            "the child's info"
        );
        assert_eq!(s.resolve("p12.b3.gain"), Some((EffectKind::Eq, "b3.gain")));
        assert_eq!(s.resolve("out"), Some((EffectKind::Board, "out")));
        assert_eq!(s.get("p12.b3.gain"), 0.0);
        assert_eq!(s.stored("p12.b3.gain"), None, "a sparse EQ stores nothing");
        assert_eq!(s.set("p12.b3.gain", 40.0), Some(30.0));
        assert_eq!(s.stored("p12.b3.gain"), Some(30.0));
        *s.value_mut("p12.b3.gain").unwrap() = 6.0;
        assert_eq!(s.get("p12.b3.gain"), 6.0);
        assert_eq!(s.clear("p12.b3.gain"), Some(6.0));
        assert!(s.value_mut("p12.b3.gain").is_none(), "never inserts");
        assert!(s.ensure("p12.b3.gain"));
        assert_eq!(s.value_mut("p12.b3.gain").copied(), Some(0.0));
        assert!(!s.ensure("p12.nope") && !s.ensure("p5.out"));
        assert!(s.param("p5.out").is_none(), "no pedal 5");
        assert_eq!(s.set("ab", 2.0), Some(1.0), "the board's own");
        let keys = s.param_keys();
        assert_eq!(
            keys.len(),
            12 + DELAY_PARAMS.len() + EffectKind::Eq.params().len()
        );
        assert!(keys.iter().all(|k| s.param(k).is_some()));
        let flat = s.flat_params();
        assert_eq!(flat.get("p0.time_ms"), Some(&375.0));
        assert_eq!(flat.get("ab"), Some(&1.0));
        // The board takes a key or notes when a pedal does (Aequalis's dynamic bands are keyed).
        assert_eq!(s.takes_sidechain(), EffectKind::Eq.takes_sidechain());
        assert!(!EffectSpec::new(EffectKind::Board).takes_sidechain());
        assert_eq!(
            s.takes_notes(),
            EffectKind::Delay.takes_notes() || EffectKind::Eq.takes_notes()
        );
        // Every other kind: a `p`-key is its own (Fluctus has `p0.on`).
        let f = EffectSpec::new(EffectKind::VolumeShaper);
        assert_eq!(
            f.param("p0.on"),
            EffectKind::VolumeShaper.param("p0.on"),
            "not routed"
        );
        let json = serde_json::to_string(&EffectSpec::new(EffectKind::Delay)).unwrap();
        assert!(!json.contains("board"), "a native spec's JSON is unchanged");
    }

    #[test]
    fn missing_key_reads_default() {
        let mut s = EffectSpec::new(EffectKind::Delay);
        s.params.remove("mix");
        assert_eq!(s.get("mix"), 0.3);
        let json = serde_json::to_string(&s).unwrap();
        let back: EffectSpec = serde_json::from_str(&json).unwrap();
        assert_eq!(back, s);
    }
}
