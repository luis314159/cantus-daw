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
pub const URL: &str = "https://github.com/lalmazan/cantus";
pub const MANUAL_URL: &str = "https://github.com/lalmazan/cantus/tree/master/docs/plugins";
pub const SUPPORT_URL: &str = "https://github.com/lalmazan/cantus/issues";
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

/// The family, in its order (ADR 0017), then the round-19 effects (ADR 0026).
pub static ALL: [Descriptor; 10] = [
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
