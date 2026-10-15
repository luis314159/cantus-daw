//! **With All Your Heart** — cinematic pop in F minor at 116 BPM (round 28's showcase).
//!
//! The title is the project's motto (Colossians 3:23). The song is built around **Chorda**,
//! the modelled grand: it opens alone, pedalled from an automation lane (its `pedal`
//! parameter, legato pedalling on every chord change), carries the verses, drives the
//! choruses and has the breakdown to itself. Around it: Gravis's sub under a follower that
//! ducks it from the kick, Calor's velvet pad through Concentus with an LFO on its filter
//! and Fluctus pumping it in the choruses, Lamina's suitcase panned by Tremor, Unda's pluck
//! arpeggio under a step sequence and a random pan through Phasis, Exemplar's nylon guitar
//! through Phasis's Phase 90, Unda's formant voice as the topline through **Tonus** hard
//! tune (F minor), Calor's sync lead doubling it in the last chorus through Flexus,
//! Exemplar's choir, Modulus bells through Anulus, an Unda riser through Translatio and a
//! sub drop. Pulsus II drums (no download needed): Impetus on the kick and the snare, the
//! drum bus through Discrimen, Comprimo and Tonsor. Master: Aequalis, Multiplex, Latitudo,
//! Tonsor, Culmen to −1 dBTP, Speculum to look at.
//!
//! Intro 8 · verse 16 · pre-chorus 8 · chorus 16 · breakdown 8 · chorus 16 · outro 8 = 80
//! bars, 2 min 46 s.

mod desk;
mod drums;
mod moves;
mod music;

use super::song::{self, colors, native, Feel, Part, Song};
use crate::i18n::t;
use cantus_core::{InstrumentKind as I, Project, StripId, TrackIcon};

pub const BPM: f64 = 116.0;

/// The sections: key, first bar, bars.
pub const FORM: [(&str, f64, f64); 7] = [
    ("intro", 0.0, 8.0),
    ("verse", 8.0, 16.0),
    ("prechorus", 24.0, 8.0),
    ("chorus", 32.0, 16.0),
    ("break", 48.0, 8.0),
    ("chorus", 56.0, 16.0),
    ("outro", 72.0, 8.0),
];

/// The last chorus starts here.
pub const FINAL: f64 = 56.0;

// The drums, General MIDI.
pub const KICK: u8 = 36;
pub const RIM: u8 = 37;
pub const SNARE: u8 = 38;
pub const CLAP: u8 = 39;
pub const HAT: u8 = 42;
pub const OPEN: u8 = 46;
pub const TOM_LO: u8 = 45;
pub const TOM_MID: u8 = 47;
pub const TOM_HI: u8 = 50;
pub const CRASH: u8 = 49;
pub const SHAKER: u8 = 70;

pub struct Band {
    pub kick: Part,
    pub snare: Part,
    pub hats: Part,
    pub perc: Part,
    pub fills: Part,
    pub bass: Part,
    pub piano: Part,
    pub keys: Part,
    pub pad: Part,
    pub arp: Part,
    pub guitar: Part,
    pub vox: Part,
    pub lead: Part,
    pub choir: Part,
    pub bells: Part,
    pub rise: Part,
    pub drop: Part,
}

/// The strips that are not a track's.
pub struct Desk {
    pub drums: StripId,
    pub music: StripId,
    pub verb: StripId,
    pub delay: StripId,
}

pub fn build() -> Project {
    let mut s = Song::new(&t("demos.song.heart"), BPM, 0x4EA2_7323);
    s.strips(20);
    let b = band(&mut s);
    let d = desk::desk(&mut s, &b);
    for (key, at, len) in FORM {
        music::section(&mut s, &b, key, at, len);
    }
    moves::automation(&mut s, &b, &d);
    moves::modulators(&mut s, &b);
    s.finish()
}

fn band(s: &mut Song) -> Band {
    let electro = song::kit("pulsus.kit.electro");
    let studio = song::kit("pulsus.kit.studio");
    let k909 = song::kit("pulsus.kit.909");
    let perc = song::kit("pulsus.kit.percussion");
    let drum = |s: &mut Song, name: &str, spec, color| s.part(&t(name), spec, color, TrackIcon::Drums);
    let kick = drum(s, "demos.track.kick", song::pads(&electro, &[KICK]), colors::KICK);
    let snare = drum(
        s,
        "demos.track.snare",
        song::pads_from(&[(&studio, SNARE), (&k909, CLAP)]),
        colors::SNARE,
    );
    let hats = drum(s, "demos.track.hats", song::pads(&k909, &[HAT, OPEN]), colors::HATS);
    let perc = drum(
        s,
        "demos.track.perc",
        song::pads_from(&[(&perc, SHAKER), (&studio, RIM)]),
        colors::PERC,
    );
    let fills = drum(
        s,
        "demos.track.toms",
        song::pads(&studio, &[TOM_LO, TOM_MID, TOM_HI, CRASH]),
        colors::PERC,
    );
    let bass = s.part(
        &t("demos.track.bass"),
        native(
            I::Bass,
            "bass.preset.square_sub",
            &[
                ("mode", 1.0),
                ("syn.wave", 0.0),
                ("syn.cutoff", 520.0),
                ("syn.env", 1.4),
                ("syn.f_decay", 220.0),
                ("syn.release", 90.0),
                ("sub.level", 0.6),
            ],
        ),
        colors::BASS,
        TrackIcon::Synth,
    );
    let piano = s.part(
        &t("demos.track.piano"),
        native(I::Chorda, "chorda.preset.pop", &[("width", 0.8)]),
        colors::KEYS,
        TrackIcon::Keys,
    );
    let keys = s.part(
        &t("demos.track.keys"),
        native(I::ElectricPiano, "epiano.preset.suitcase", &[]),
        colors::ORGAN,
        TrackIcon::Keys,
    );
    let pad = s.part(
        &t("demos.track.pad"),
        native(I::Analog, "calor.preset.velvet_pad", &[]),
        colors::PAD,
        TrackIcon::Synth,
    );
    let arp = s.part(
        &t("demos.track.arp"),
        native(I::Wavetable, "unda.preset.pluck_keys", &[]),
        colors::PLUCK,
        TrackIcon::Synth,
    );
    let guitar = s.part(
        &t("demos.track.guitar"),
        native(I::Sampler, "smp.preset.nylon", &[]),
        colors::BRASS,
        TrackIcon::Strings,
    );
    // The topline: one voice gliding into every note, so Tonus has slides to snap.
    let vox = s.part(
        &t("demos.track.vox"),
        native(
            I::Wavetable,
            "unda.preset.formant_talker",
            &[
                ("osc1.uni", 1.0),
                ("voice.mode", 1.0),
                ("voice.glide", 0.07),
                ("voice.glide_mode", 1.0),
            ],
        ),
        colors::LEAD,
        TrackIcon::Vocal,
    );
    let lead = s.part(
        &t("demos.track.lead"),
        native(I::Analog, "calor.preset.sync_lead", &[]),
        colors::LEAD,
        TrackIcon::Synth,
    );
    let choir = s.part(
        &t("demos.track.choir"),
        native(I::Sampler, "smp.preset.choir", &[]),
        colors::STRINGS,
        TrackIcon::Vocal,
    );
    let bells = s.part(
        &t("demos.track.bells"),
        native(I::Fm, "fm.preset.bell", &[]),
        colors::BELLS,
        TrackIcon::Keys,
    );
    let rise = s.part(
        &t("demos.section.rise"),
        native(I::Wavetable, "unda.preset.noise_sweep", &[]),
        colors::BELLS,
        TrackIcon::Fx,
    );
    let drop = s.part(
        &t("demos.track.drop"),
        native(I::Wavetable, "unda.preset.sub_drop", &[]),
        colors::BASS,
        TrackIcon::Fx,
    );
    let tight = Feel {
        jitter: 0.004,
        vel: 5.0,
        ..Feel::HUMAN
    };
    for p in [kick, snare, hats, perc, fills] {
        s.feel(p, tight);
    }
    s.feel(hats, tight.swung(0.08, 0.25));
    s.feel(perc, tight.swung(0.1, 0.25));
    s.feel(
        bass,
        Feel {
            jitter: 0.0,
            vel: 3.0,
            ..Feel::HUMAN
        },
    );
    s.feel(
        piano,
        Feel {
            jitter: 0.008,
            vel: 6.0,
            ..Feel::HUMAN
        },
    );
    for p in [arp, rise, drop] {
        s.feel(p, Feel::TIGHT);
    }
    Band {
        kick,
        snare,
        hats,
        perc,
        fills,
        bass,
        piano,
        keys,
        pad,
        arp,
        guitar,
        vox,
        lead,
        choir,
        bells,
        rise,
        drop,
    }
}
