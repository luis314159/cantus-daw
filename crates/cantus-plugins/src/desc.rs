//! Who each exported plug-in is: its names, its ids in both standards, and what it says
//! it is to a host's browser.
//!
//! **Every id here is forever.** A host stores a CLAP id or a VST3 class id in every
//! project that uses the plug-in; changing one orphans those projects. The golden test
//! (`tests/ids.rs`) holds them.
//!
//! The VST3 class ids follow a fixed rule so that nobody has to invent one: the first 16
//! bytes of SHA-256 of `cantus/vst3/<slug>`, read as four big-endian 32-bit words (the
//! order `vst3::uid` takes, and the order a class id is printed in `moduleinfo.json`).

use cantus_core::EffectKind;

/// The vendor every host shows.
pub const VENDOR: &str = "Cantus";
pub const URL: &str = "https://github.com/luis314159/cantus-daw";
pub const MANUAL_URL: &str = "https://github.com/luis314159/cantus-daw/tree/master/docs/plugins";
pub const SUPPORT_URL: &str = "https://github.com/luis314159/cantus-daw/issues";
pub const EMAIL: &str = "";
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// One exported plug-in.
#[derive(Debug)]
pub struct Descriptor {
    pub kind: EffectKind,
    /// The plug-in's own name: `Porta`.
    pub name: &'static str,
    /// What a host lists: `Cantus Porta`. Also the bundle's file name.
    pub full_name: &'static str,
    /// Lower-case, for cargo features and file names: `porta`.
    pub slug: &'static str,
    /// The CLAP id (reverse domain of the project's home).
    pub clap_id: &'static str,
    /// The VST3 class id, as four words (see the module's rule).
    pub vst3_cid: [u32; 4],
    /// CLAP features, the standard's words (`plugin-features.h`).
    pub clap_features: &'static [&'static str],
    /// VST3 sub-categories (`Fx|Dynamics`).
    pub vst3_categories: &'static str,
    /// One line for a host's browser.
    pub description: &'static str,
    /// The cargo feature that compiles it in.
    pub compiled: bool,
}

impl Descriptor {
    /// The VST3 class id as `moduleinfo.json` and hosts print it: 32 hex digits.
    pub fn vst3_cid_hex(&self) -> String {
        let [a, b, c, d] = self.vst3_cid;
        format!("{a:08X}{b:08X}{c:08X}{d:08X}")
    }
}

const FX: &str = "audio-effect";
const STEREO: &str = "stereo";

/// The family, in its order (ADR 0017), then the round-19 effects (ADR 0026), then
/// round 28's.
pub static ALL: [Descriptor; 34] = [
    // fx3:contract: 33 with wave 3's four; fxa:loquax: 34 with the vocoder
    Descriptor {
        kind: EffectKind::Eq,
        name: "Aequalis",
        full_name: "Cantus Aequalis",
        slug: "aequalis",
        clap_id: "com.github.lalmazan.cantus.aequalis",
        vst3_cid: [0x1DAE669B, 0x75E1E0F8, 0x46E42D56, 0xF8B5D7FB],
        clap_features: &[FX, "equalizer", "filter", STEREO],
        vst3_categories: "Fx|EQ",
        description:
            "Parametric EQ: 24 bands, zero-latency, natural and linear phase, dynamics per band.",
        compiled: cfg!(feature = "eq"),
    },
    Descriptor {
        kind: EffectKind::Compressor,
        name: "Comprimo",
        full_name: "Cantus Comprimo",
        slug: "comprimo",
        clap_id: "com.github.lalmazan.cantus.comprimo",
        vst3_cid: [0x5CC2F9F9, 0xEAF424B6, 0xB13F59FB, 0x1D1143BD],
        clap_features: &[FX, "compressor", STEREO],
        vst3_categories: "Fx|Dynamics",
        description:
            "Compressor: nine styles with a visible detector, key EQ, lookahead, oversampling.",
        compiled: cfg!(feature = "compressor"),
    },
    Descriptor {
        kind: EffectKind::Limiter,
        name: "Terminus",
        full_name: "Cantus Terminus",
        slug: "terminus",
        clap_id: "com.github.lalmazan.cantus.terminus",
        vst3_cid: [0xFB32BF56, 0x01CFBF33, 0xCAF38252, 0x3A865DCE],
        clap_features: &[FX, "limiter", "mastering", STEREO],
        vst3_categories: "Fx|Mastering",
        description: "True-peak limiter with loudness metering to BS.1770-4, clipper and dither.",
        compiled: cfg!(feature = "limiter"),
    },
    Descriptor {
        kind: EffectKind::Space,
        name: "Spatium",
        full_name: "Cantus Spatium",
        slug: "spatium",
        clap_id: "com.github.lalmazan.cantus.spatium",
        vst3_cid: [0x84E7F366, 0xF9ACED98, 0x2F79C20B, 0x744AC27F],
        clap_features: &[FX, "reverb", STEREO],
        vst3_categories: "Fx|Reverb",
        description:
            "Reverb: a 16-line network with its decay fitted per octave, rooms, plates and springs.",
        compiled: cfg!(feature = "space"),
    },
    Descriptor {
        kind: EffectKind::Multiband,
        name: "Multiplex",
        full_name: "Cantus Multiplex",
        slug: "multiplex",
        clap_id: "com.github.lalmazan.cantus.multiplex",
        vst3_cid: [0x1F743387, 0x03DD6A8B, 0x3B7FABD0, 0x76E5DCF4],
        clap_features: &[FX, "compressor", "expander", "mastering", STEREO],
        vst3_categories: "Fx|Dynamics",
        description: "Multiband dynamics: up to eight bands, compress and expand above and below.",
        compiled: cfg!(feature = "multiband"),
    },
    Descriptor {
        kind: EffectKind::DeEsser,
        name: "Sibilus",
        full_name: "Cantus Sibilus",
        slug: "sibilus",
        clap_id: "com.github.lalmazan.cantus.sibilus",
        vst3_cid: [0x21F9E779, 0x4FEDE019, 0x877FB741, 0xE677030C],
        clap_features: &[FX, "de-esser", STEREO],
        vst3_categories: "Fx|Dynamics",
        description: "De-esser that tells an s from a bright sound: wide, split or band-only.",
        compiled: cfg!(feature = "deesser"),
    },
    Descriptor {
        kind: EffectKind::Gate,
        name: "Porta",
        full_name: "Cantus Porta",
        slug: "porta",
        clap_id: "com.github.lalmazan.cantus.porta",
        vst3_cid: [0xF79263DA, 0xC4FE4C2B, 0x64256272, 0xA7C26116],
        clap_features: &[FX, "gate", "expander", STEREO],
        vst3_categories: "Fx|Dynamics",
        description: "Gate and expander: six styles, hysteresis, lookahead, key filters.",
        compiled: cfg!(feature = "gate"),
    },
    Descriptor {
        kind: EffectKind::Stereo,
        name: "Latitudo",
        full_name: "Cantus Latitudo",
        slug: "latitudo",
        clap_id: "com.github.lalmazan.cantus.latitudo",
        vst3_cid: [0xEC13BB3A, 0x4A5F71E7, 0x1468741C, 0xD2C29ACE],
        clap_features: &[FX, "utility", "mastering", STEREO],
        vst3_categories: "Fx|Spatial",
        description:
            "Stereo and mid/side: width by band, phase-coherent mono bass, M/S tone, vectorscope.",
        compiled: cfg!(feature = "stereo"),
    },
    Descriptor {
        kind: EffectKind::Maximizer,
        name: "Culmen",
        full_name: "Cantus Culmen",
        slug: "culmen",
        clap_id: "com.github.lalmazan.cantus.culmen",
        vst3_cid: [0x5A15F5E7, 0x18155BDA, 0xA4E01DAE, 0xB6CACEC3],
        clap_features: &[FX, "limiter", "compressor", "mastering", STEREO],
        vst3_categories: "Fx|Mastering",
        description: "Mastering maximizer: three bands and a full band of two-sided dynamics into a true-peak brickwall.",
        compiled: cfg!(feature = "maximizer"),
    },
    Descriptor {
        kind: EffectKind::Saturator,
        name: "Fervor",
        full_name: "Cantus Fervor",
        slug: "fervor",
        clap_id: "com.github.lalmazan.cantus.fervor",
        vst3_cid: [0xC1072D15, 0xF4D9B4EB, 0xD8CBE322, 0x8AD22E23],
        clap_features: &[FX, "distortion", "mastering", STEREO],
        vst3_categories: "Fx|Distortion",
        description:
            "Saturation and colour: tape, tube, overdrive, fuzz, clip, fold and a drawn curve, oversampled.",
        compiled: cfg!(feature = "saturator"),
    },
    Descriptor {
        kind: EffectKind::Chorus,
        name: "Concentus",
        full_name: "Cantus Concentus",
        slug: "concentus",
        clap_id: "com.github.lalmazan.cantus.concentus",
        vst3_cid: [0xFD2C6C34, 0x40D7C253, 0x7A8189D0, 0x3B59D849],
        clap_features: &[FX, "chorus", STEREO],
        vst3_categories: "Fx|Modulation",
        description: "Chorus: the classic ensembles and wider voices of its own.",
        compiled: cfg!(feature = "concentus"),
    },
    Descriptor {
        kind: EffectKind::Flanger,
        name: "Flexus",
        full_name: "Cantus Flexus",
        slug: "flexus",
        clap_id: "com.github.lalmazan.cantus.flexus",
        vst3_cid: [0x88EF5F4F, 0x7F8034B7, 0x687D995D, 0x3E5912AE],
        clap_features: &[FX, "flanger", STEREO],
        vst3_categories: "Fx|Modulation",
        description: "Flanger that sweeps through zero, as two tape machines did.",
        compiled: cfg!(feature = "flexus"),
    },
    Descriptor {
        kind: EffectKind::Phaser,
        name: "Phasis",
        full_name: "Cantus Phasis",
        slug: "phasis",
        clap_id: "com.github.lalmazan.cantus.phasis",
        vst3_cid: [0x8E7D004D, 0x715CB0DC, 0x9BE8E58F, 0xFD66B11E],
        clap_features: &[FX, "phaser", STEREO],
        vst3_categories: "Fx|Modulation",
        description: "Phaser: sweeping notches from a chain of all-pass stages.",
        compiled: cfg!(feature = "phasis"),
    },
    Descriptor {
        kind: EffectKind::Tremolo,
        name: "Tremor",
        full_name: "Cantus Tremor",
        slug: "tremor",
        clap_id: "com.github.lalmazan.cantus.tremor",
        vst3_cid: [0x673BE03A, 0xACAD1CED, 0xD8FFE225, 0x0D2C4581],
        clap_features: &[FX, "tremolo", STEREO],
        vst3_categories: "Fx|Modulation",
        description: "Tremolo and auto-pan in time with the song.",
        compiled: cfg!(feature = "tremor"),
    },
    Descriptor {
        kind: EffectKind::Vibrato,
        name: "Vibrans",
        full_name: "Cantus Vibrans",
        slug: "vibrans",
        clap_id: "com.github.lalmazan.cantus.vibrans",
        vst3_cid: [0x21194EE9, 0x19E40ADD, 0xB984BE53, 0xCFF3C198],
        clap_features: &[FX, "vibrato", STEREO],
        vst3_categories: "Fx|Modulation",
        description: "Vibrato: pitch that wavers like a voice, a tape or a rotary speaker.",
        compiled: cfg!(feature = "vibrans"),
    },
    Descriptor {
        kind: EffectKind::RingMod,
        name: "Anulus",
        full_name: "Cantus Anulus",
        slug: "anulus",
        clap_id: "com.github.lalmazan.cantus.anulus",
        vst3_cid: [0xD5E55083, 0x797456BF, 0xD37B1AC9, 0xD278EA51],
        clap_features: &[FX, "ring-modulator", STEREO],
        vst3_categories: "Fx|Modulation",
        description: "Ring modulator: bells, metal and robot voices.",
        compiled: cfg!(feature = "anulus"),
    },
    Descriptor {
        kind: EffectKind::FreqShift,
        name: "Translatio",
        full_name: "Cantus Translatio",
        slug: "translatio",
        clap_id: "com.github.lalmazan.cantus.translatio",
        vst3_cid: [0xA8853B75, 0x63E0B21F, 0x04157A5F, 0xA3B23872],
        clap_features: &[FX, "frequency-shifter", STEREO],
        vst3_categories: "Fx|Modulation",
        description: "Frequency shifter: every partial moved by the same number of hertz.",
        compiled: cfg!(feature = "translatio"),
    },
    Descriptor {
        kind: EffectKind::VolumeShaper,
        name: "Fluctus",
        full_name: "Cantus Fluctus",
        slug: "fluctus",
        clap_id: "com.github.lalmazan.cantus.fluctus",
        vst3_cid: [0x599FE72F, 0x7636D3BA, 0x20D9E034, 0x4584E724],
        clap_features: &[FX, "tremolo", "mixing", STEREO],
        vst3_categories: "Fx|Modulation",
        description: "Volume shaper: pumping, gating and swells drawn over a beat.",
        compiled: cfg!(feature = "fluctus"),
    },
    Descriptor {
        kind: EffectKind::Clipper,
        name: "Tonsor",
        full_name: "Cantus Tonsor",
        slug: "tonsor",
        clap_id: "com.github.lalmazan.cantus.tonsor",
        vst3_cid: [0x496542F9, 0x94F62726, 0x9D2426F6, 0x05EFCA20],
        clap_features: &[FX, "limiter", "distortion", "mastering", STEREO],
        vst3_categories: "Fx|Dynamics",
        description: "Clipper: soft or hard, oversampled, under a true-peak ceiling.",
        compiled: cfg!(feature = "tonsor"),
    },
    Descriptor {
        kind: EffectKind::Transient,
        name: "Impetus",
        full_name: "Cantus Impetus",
        slug: "impetus",
        clap_id: "com.github.lalmazan.cantus.impetus",
        vst3_cid: [0x4A617E6B, 0x08572AE4, 0x8163D60C, 0xA326A3E0],
        clap_features: &[FX, "transient-shaper", STEREO],
        vst3_categories: "Fx|Dynamics",
        description: "Transient shaper: more or less attack and sustain, at any level.",
        compiled: cfg!(feature = "impetus"),
    },
    Descriptor {
        kind: EffectKind::TransientSplit,
        name: "Discrimen",
        full_name: "Cantus Discrimen",
        slug: "discrimen",
        clap_id: "com.github.lalmazan.cantus.discrimen",
        vst3_cid: [0x258467C3, 0x841A85F4, 0xF833DA71, 0x37868CD9],
        clap_features: &[FX, "transient-shaper", "utility", STEREO],
        vst3_categories: "Fx|Dynamics",
        description: "Transient splitter: the attack and the body, each on its own.",
        compiled: cfg!(feature = "discrimen"),
    },
    Descriptor {
        kind: EffectKind::Analyzer,
        name: "Speculum",
        full_name: "Cantus Speculum",
        slug: "speculum",
        clap_id: "com.github.lalmazan.cantus.speculum",
        vst3_cid: [0xCBA1953D, 0xC34A0CF3, 0x5419D4DA, 0x1196A31A],
        clap_features: &[FX, "analyzer", "utility", STEREO],
        vst3_categories: "Fx|Analyzer",
        description: "Analyser: spectrum, waveform, phase and loudness, measured to the standards.",
        compiled: cfg!(feature = "speculum"),
    },
    Descriptor {
        kind: EffectKind::AutoTune,
        name: "Tonus",
        full_name: "Cantus Tonus",
        slug: "tonus",
        clap_id: "com.github.lalmazan.cantus.tonus",
        vst3_cid: [0x5844B71E, 0xB6558FB8, 0xC2F9E7F2, 0x6DD70EC0],
        clap_features: &[FX, "pitch-correction", STEREO],
        vst3_categories: "Fx|Pitch Shift",
        description: "Real-time pitch correction to a key and scale, a set of notes or the notes you play, from natural to hard-tuned, formants kept.",
        compiled: cfg!(feature = "tonus"),
    },
    // r39:conv — Aula (ADR 0112): Cantus's own responses by its `ir` parameter (a file is
    // the DAW's: a plug-in's state holds numbers only).
    Descriptor {
        kind: EffectKind::Aula,
        name: "Aula",
        full_name: "Cantus Aula",
        slug: "aula",
        clap_id: "com.github.lalmazan.cantus.aula",
        vst3_cid: [0xE572AB2F, 0x65BBF685, 0x49C41E7C, 0xC17FCA6C],
        clap_features: &[FX, "reverb", STEREO],
        vst3_categories: "Fx|Reverb",
        description: "Convolution reverb: rooms, halls, plates, springs and chambers made by Cantus, zero latency, shaped by stretch, length, decay, damping and cuts.",
        compiled: cfg!(feature = "aula"),
    },
    // fx:contract — the fx side round's five (provisional ADRs 0130–0133); ids are
    // forever once released, by the module's rule.
    Descriptor {
        kind: EffectKind::AutoFilter, // fx:contract
        name: "Cribrum",
        full_name: "Cantus Cribrum",
        slug: "cribrum",
        clap_id: "com.github.lalmazan.cantus.cribrum",
        vst3_cid: [0x8280FB5B, 0xB86C9415, 0x7A1AD8E5, 0x364DDEC1],
        clap_features: &[FX, "filter", STEREO],
        vst3_categories: "Fx|Filter",
        description: "Auto filter: analog filter models moved by an envelope, a synced LFO or a follower.",
        compiled: cfg!(feature = "cribrum"),
    },
    Descriptor {
        kind: EffectKind::Pedal, // fx:contract
        name: "Fragor",
        full_name: "Cantus Fragor",
        slug: "fragor",
        clap_id: "com.github.lalmazan.cantus.fragor",
        vst3_cid: [0x60CC77ED, 0x2C45A210, 0x3147B7A4, 0x0E1527BE],
        clap_features: &[FX, "distortion", STEREO],
        vst3_categories: "Fx|Distortion",
        description: "Distortion and fuzz pedals, modelled from their circuits.",
        compiled: cfg!(feature = "fragor"),
    },
    Descriptor {
        kind: EffectKind::TimeShaper, // fx:contract
        name: "Lapsus",
        full_name: "Cantus Lapsus",
        slug: "lapsus",
        clap_id: "com.github.lalmazan.cantus.lapsus",
        vst3_cid: [0x83B3C333, 0x81E2521A, 0x89A21D39, 0xEAA7BF03],
        clap_features: &[FX, "glitch", STEREO],
        vst3_categories: "Fx|Modulation",
        description: "Time and volume shaper: tape stops, half-time, stutters and scratches, synced to the song.",
        compiled: cfg!(feature = "lapsus"),
    },
    Descriptor {
        kind: EffectKind::LoFi, // fx:contract
        name: "Vetus",
        full_name: "Cantus Vetus",
        slug: "vetus",
        clap_id: "com.github.lalmazan.cantus.vetus",
        vst3_cid: [0xAF6ED6C2, 0x8D427D1A, 0xFC4FB59B, 0x1B6199E3],
        clap_features: &[FX, "distortion", STEREO],
        vst3_categories: "Fx|Distortion",
        description: "Lo-fi: fewer bits, a lower sample rate, vinyl and tape wear.",
        compiled: cfg!(feature = "vetus"),
    },
    Descriptor {
        kind: EffectKind::Utility, // fx:contract
        name: "Libra",
        full_name: "Cantus Libra",
        slug: "libra",
        clap_id: "com.github.lalmazan.cantus.libra",
        vst3_cid: [0x035BD1F4, 0x20D86106, 0x99631CB5, 0xBC149313],
        clap_features: &[FX, "utility", STEREO],
        vst3_categories: "Fx|Tools",
        description: "Utility: gain, polarity, channel operations, alignment and a tuner.",
        compiled: cfg!(feature = "libra"),
    },
    // fx3:contract — the fx side round's wave 3 (provisional ADRs 0134–0137); ids are
    // forever once released, by the module's rule.
    Descriptor {
        kind: EffectKind::Board, // fx3:contract
        name: "Tabula",
        full_name: "Cantus Tabula",
        slug: "tabula",
        clap_id: "com.github.lalmazan.cantus.tabula",
        vst3_cid: [0xA28BD282, 0xBA48590F, 0x35C8BC62, 0x0CCF7313],
        clap_features: &[FX, "multi-effects", STEREO],
        vst3_categories: "Fx",
        description: "Pedalboard: Cantus's own effects as pedals in any order, an A/B split and eight macros.",
        compiled: cfg!(feature = "tabula"),
    },
    Descriptor {
        kind: EffectKind::PitchShift, // fx3:contract
        name: "Saltus",
        full_name: "Cantus Saltus",
        slug: "saltus",
        clap_id: "com.github.lalmazan.cantus.saltus",
        vst3_cid: [0xCECA3718, 0xF4DDB4E5, 0xDBABF3C7, 0xFE35322C],
        clap_features: &[FX, "pitch-shifter", STEREO],
        vst3_categories: "Fx|Pitch Shift",
        description: "Pitch: an octaver, a pitch shifter and harmoniser, and a whammy.",
        compiled: cfg!(feature = "saltus"),
    },
    Descriptor {
        kind: EffectKind::Rotary, // fx3:contract
        name: "Rota",
        full_name: "Cantus Rota",
        slug: "rota",
        clap_id: "com.github.lalmazan.cantus.rota",
        vst3_cid: [0xB3A52B72, 0x37C662B9, 0x7C8B54DF, 0x80048288],
        clap_features: &[FX, "modulation", STEREO],
        vst3_categories: "Fx|Modulation",
        description: "Rotary speaker: horn and drum, slow and fast, heard through two microphones.",
        compiled: cfg!(feature = "rota"),
    },
    Descriptor {
        kind: EffectKind::Amp, // fx3:contract
        name: "Arca",
        full_name: "Cantus Arca",
        slug: "arca",
        clap_id: "com.github.lalmazan.cantus.arca",
        vst3_cid: [0x84B3FE43, 0x91BC4B10, 0xB2BE25F4, 0xBCF6F1BC],
        clap_features: &[FX, "distortion", STEREO],
        vst3_categories: "Fx|Distortion",
        description: "Amp and cabinet: preamp, tone stack and power amp from their circuits, the cabinet by impulse response.",
        compiled: cfg!(feature = "arca"),
    },
    // fxa:loquax — the fx side round's wave A (provisional ADR 0139). Its carrier is the
    // sidechain input (the plug-in's own switch set to the external key) or the notes the
    // host sends to its note port, which play its synthesiser.
    Descriptor {
        kind: EffectKind::Vocoder,
        name: "Loquax",
        full_name: "Cantus Loquax",
        slug: "loquax",
        clap_id: "com.github.lalmazan.cantus.loquax",
        vst3_cid: [0x5B1E7A39, 0xC4D2486F, 0x9A03E1B7, 0x2F6C85D4],
        clap_features: &[FX, "filter", STEREO],
        vst3_categories: "Fx|Filter",
        description: "Vocoder: a voice speaks through a synthesiser played by notes or through the sidechain, in 8 to 64 bands.",
        compiled: cfg!(feature = "loquax"),
    },
];

/// The plug-ins this binary carries (the cargo features it was built with).
pub fn compiled() -> impl Iterator<Item = &'static Descriptor> {
    ALL.iter().filter(|d| d.compiled)
}

/// The descriptor of a kind, whether or not it is compiled in.
pub fn of(kind: EffectKind) -> Option<&'static Descriptor> {
    ALL.iter().find(|d| d.kind == kind)
}

pub fn by_clap_id(id: &str) -> Option<&'static Descriptor> {
    compiled().find(|d| d.clap_id == id)
}

pub fn by_slug(slug: &str) -> Option<&'static Descriptor> {
    ALL.iter().find(|d| d.slug == slug)
}
