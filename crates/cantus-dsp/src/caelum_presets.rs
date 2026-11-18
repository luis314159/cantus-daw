//! Caelum's starting preset and its factory spaces, as settings — what the listening
//! examples render and what the presets' file (a later phase) will hold, so the two cannot
//! drift: the file's test can compare itself with these.

use super::{CaelumSettings, Lines};

/// A named factory space.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Preset {
    /// The preset's name (English; the file names its translation).
    pub name: &'static str,
    /// One line on what it is for.
    pub about: &'static str,
    pub settings: CaelumSettings,
}

/// What an insert starts with (ADR 0071: the defaults stay a null, the insert sounds at
/// once): the classic octave shimmer, a third wet.
pub fn starting() -> CaelumSettings {
    octave_shimmer()
}

fn base() -> CaelumSettings {
    CaelumSettings::default()
}

/// A wide space whose tail climbs an octave and keeps climbing, softly.
pub fn octave_shimmer() -> CaelumSettings {
    CaelumSettings {
        size: 0.72,
        decay_s: 6.5,
        predelay_ms: 30.0,
        diffusion: 0.8,
        high_hz: 7_000.0,
        high_decay: 0.45,
        mod_depth: 0.35,
        mod_rate_hz: 0.35,
        shimmer: 0.55,
        pitch_a: 12.0,
        pitch_b_level: 0.0,
        shimmer_low_hz: 350.0,
        shimmer_high_hz: 9_000.0,
        mix: 0.35,
        ..base()
    }
}

/// A vast stone nave: an octave and a fifth above, dark walls, long bass.
pub fn fifth_cathedral() -> CaelumSettings {
    CaelumSettings {
        size: 0.92,
        decay_s: 11.0,
        predelay_ms: 55.0,
        diffusion: 0.85,
        high_hz: 4_500.0,
        high_decay: 0.4,
        low_hz: 220.0,
        low_decay: 1.3,
        mod_depth: 0.3,
        mod_rate_hz: 0.25,
        shimmer: 0.5,
        pitch_a: 12.0,
        pitch_b: 7.0,
        pitch_b_level: 0.7,
        shimmer_low_hz: 300.0,
        shimmer_high_hz: 7_500.0,
        mix: 0.4,
        ..base()
    }
}

/// The sound swells into the space over two seconds: a pad out of anything.
pub fn bloom_pad() -> CaelumSettings {
    CaelumSettings {
        size: 0.8,
        decay_s: 8.0,
        predelay_ms: 10.0,
        diffusion: 0.9,
        high_hz: 6_000.0,
        high_decay: 0.5,
        mod_depth: 0.55,
        mod_rate_hz: 0.3,
        shimmer: 0.2,
        pitch_a: 12.0,
        shimmer_low_hz: 400.0,
        shimmer_high_hz: 8_000.0,
        bloom_ms: 1_800.0,
        mix: 0.45,
        ..base()
    }
}

/// An endless space that swells in; press Freeze on a chord to hold it.
pub fn freeze_swell() -> CaelumSettings {
    CaelumSettings {
        size: 1.0,
        decay_s: 20.0,
        infinite: true,
        predelay_ms: 0.0,
        diffusion: 0.9,
        high_hz: 6_500.0,
        high_decay: 0.7,
        mod_depth: 0.45,
        mod_rate_hz: 0.2,
        shimmer: 0.3,
        pitch_a: 12.0,
        shimmer_low_hz: 400.0,
        shimmer_high_hz: 7_000.0,
        bloom_ms: 2_500.0,
        mix: 0.5,
        ..base()
    }
}

/// A singer's space that steps aside while the voice sings and blooms in the gaps.
pub fn ducked_vocal() -> CaelumSettings {
    CaelumSettings {
        size: 0.55,
        decay_s: 3.2,
        predelay_ms: 70.0,
        diffusion: 0.75,
        high_hz: 8_000.0,
        high_decay: 0.55,
        low_hz: 300.0,
        low_decay: 0.8,
        mod_depth: 0.3,
        mod_rate_hz: 0.5,
        shimmer: 0.15,
        pitch_a: 12.0,
        shimmer_low_hz: 600.0,
        shimmer_high_hz: 9_000.0,
        duck_db: 12.0,
        duck_thr_db: -32.0,
        duck_att_ms: 15.0,
        duck_rel_ms: 350.0,
        low_cut_hz: 180.0,
        mix: 0.32,
        ..base()
    }
}

/// An octave below: a dark cloud under the note.
pub fn sub_cloud() -> CaelumSettings {
    CaelumSettings {
        size: 0.85,
        decay_s: 9.0,
        predelay_ms: 20.0,
        high_hz: 3_500.0,
        high_decay: 0.35,
        shimmer: 0.4,
        pitch_a: -12.0,
        shimmer_low_hz: 60.0,
        shimmer_high_hz: 4_000.0,
        mod_depth: 0.4,
        mix: 0.4,
        ..base()
    }
}

/// Infinite, climbing: the octaves keep rising and fade into the top once you stop (its
/// Sustain at 60 %: about 11 dB a minute); while you play, it layers under its ceiling.
pub fn endless_ascent() -> CaelumSettings {
    CaelumSettings {
        size: 0.9,
        decay_s: 20.0,
        infinite: true,
        sustain: 0.6,
        diffusion: 0.85,
        high_hz: 8_000.0,
        high_decay: 0.6,
        mod_depth: 0.4,
        mod_rate_hz: 0.3,
        shimmer: 0.6,
        pitch_a: 12.0,
        shimmer_low_hz: 300.0,
        shimmer_high_hz: 6_500.0,
        mix: 0.45,
        ..base()
    }
}

/// A bright small plate, dense at once, with the faintest shimmer.
pub fn glass_plate() -> CaelumSettings {
    CaelumSettings {
        size: 0.3,
        decay_s: 2.2,
        predelay_ms: 5.0,
        diffusion: 1.0,
        high_hz: 11_000.0,
        high_decay: 0.8,
        mod_depth: 0.2,
        mod_rate_hz: 0.8,
        shimmer: 0.12,
        pitch_a: 12.0,
        shimmer_low_hz: 800.0,
        shimmer_high_hz: 14_000.0,
        mix: 0.3,
        ..base()
    }
}

/// A fourth and an octave: an open, ambiguous chord above anything.
pub fn fourth_rise() -> CaelumSettings {
    CaelumSettings {
        size: 0.75,
        decay_s: 7.0,
        predelay_ms: 40.0,
        shimmer: 0.45,
        pitch_a: 5.0,
        pitch_b: 12.0,
        pitch_b_level: 0.6,
        shimmer_low_hz: 300.0,
        shimmer_high_hz: 8_000.0,
        mod_depth: 0.35,
        mix: 0.38,
        ..base()
    }
}

/// A minute of dark space: no shimmer, the bass held, the top gone early.
pub fn black_sky() -> CaelumSettings {
    CaelumSettings {
        size: 1.0,
        decay_s: 40.0,
        predelay_ms: 80.0,
        diffusion: 0.9,
        high_hz: 2_500.0,
        high_decay: 0.25,
        low_hz: 180.0,
        low_decay: 1.4,
        mod_depth: 0.6,
        mod_rate_hz: 0.15,
        mix: 0.45,
        ..base()
    }
}

/// Eight lines, half the cost: a light shimmer for a busy session.
pub fn light_shimmer() -> CaelumSettings {
    CaelumSettings {
        lines: Lines::Eight,
        size: 0.6,
        decay_s: 4.5,
        predelay_ms: 25.0,
        shimmer: 0.4,
        pitch_a: 12.0,
        shimmer_low_hz: 400.0,
        shimmer_high_hz: 9_000.0,
        mix: 0.3,
        ..base()
    }
}

/// Every factory space, in the order the browser lists them.
pub fn factory() -> Vec<Preset> {
    let p = |name, about, settings| Preset {
        name,
        about,
        settings,
    };
    vec![
        p(
            "Octave Shimmer",
            "A wide space whose tail climbs an octave.",
            octave_shimmer(),
        ),
        p(
            "Fifth Cathedral",
            "A stone nave, an octave and a fifth above.",
            fifth_cathedral(),
        ),
        p(
            "Bloom Pad",
            "The sound swells into the space over two seconds.",
            bloom_pad(),
        ),
        p(
            "Freeze Swell",
            "Endless and swelling; Freeze holds a chord.",
            freeze_swell(),
        ),
        p(
            "Ducked Vocal Space",
            "Steps aside while the voice sings.",
            ducked_vocal(),
        ),
        p("Sub Cloud", "A dark cloud an octave below.", sub_cloud()),
        p(
            "Endless Ascent",
            "Infinite; the octaves climb and fade into the top.",
            endless_ascent(),
        ),
        p(
            "Glass Plate",
            "A bright small plate with a hint of shimmer.",
            glass_plate(),
        ),
        p(
            "Fourth Rise",
            "A fourth and an octave above anything.",
            fourth_rise(),
        ),
        p(
            "Black Sky",
            "A minute of dark space, no shimmer.",
            black_sky(),
        ),
        p(
            "Light Shimmer",
            "Eight lines, half the cost.",
            light_shimmer(),
        ),
    ]
}
