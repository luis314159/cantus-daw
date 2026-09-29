//! The shipped pads and kits.
//!
//! Pulsus I's seven pads keep their numbers exactly (a kit built from them in code is a
//! model 2 kit, but the pads are the same). Pulsus II's pads use the new voices; the
//! factory kits combine them.

use super::{DrumPad, PadVoice, PulsusParams};
use crate::model::Pitch;

impl DrumPad {
    /// The shipped kick, on General MIDI 36.
    pub fn kick() -> DrumPad {
        let mut kick = DrumPad::new(36, "Kick", PadVoice::Kick);
        kick.tone = 2_400.0;
        kick
    }

    /// The shipped rim (GM 37): the snare shape, short and high.
    pub fn rim() -> DrumPad {
        let mut rim = DrumPad::new(37, "Rim", PadVoice::Snare);
        rim.tune = 14.0;
        rim.decay = 0.035;
        rim.mix = 0.25;
        rim.tone = 3_200.0;
        rim.noise_decay = 0.03;
        rim.level_db = -5.0;
        rim
    }

    /// The shipped snare, on GM 38.
    pub fn snare() -> DrumPad {
        let mut snare = DrumPad::new(38, "Snare", PadVoice::Snare);
        snare.decay = 0.13;
        snare.noise_decay = 0.19;
        snare.tone = 1_900.0;
        snare.mix = 0.55;
        snare
    }

    /// The shipped closed hat (GM 42), in choke group 1 with the open one.
    pub fn closed_hat() -> DrumPad {
        let mut closed = DrumPad::new(42, "Closed hat", PadVoice::HiHat);
        closed.decay = 0.055;
        closed.tone = 9_500.0;
        closed.choke = 1;
        closed.level_db = -3.0;
        closed
    }

    /// The shipped low tom (GM 45): the snare shape, all body, tuned down.
    pub fn low_tom() -> DrumPad {
        let mut tom = DrumPad::new(45, "Low tom", PadVoice::Snare);
        tom.tune = -9.0;
        tom.decay = 0.42;
        tom.mix = 0.06;
        tom.level_db = -2.0;
        tom
    }

    /// The shipped open hat (GM 46), in choke group 1 with the closed one. It follows the
    /// hi-hat pedal once one moves, so a closing pedal closes it.
    pub fn open_hat() -> DrumPad {
        let mut open = DrumPad::new(46, "Open hat", PadVoice::HiHat);
        open.decay = 0.42;
        open.tone = 9_500.0;
        open.choke = 1;
        open.level_db = -4.0;
        open
    }

    /// The shipped high tom (GM 50).
    pub fn high_tom() -> DrumPad {
        let mut tom = DrumPad::new(50, "High tom", PadVoice::Snare);
        tom.tune = -1.0;
        tom.decay = 0.32;
        tom.mix = 0.06;
        tom.level_db = -2.0;
        tom
    }

    // ---- Pulsus II ------------------------------------------------------------------

    /// A bridged-T 808 kick: a resonator that rings down on its own, a little sweep, the
    /// sigh as it fades. Tune 0 is F1; +2 puts it on G1 (49 Hz).
    pub fn kick_808() -> DrumPad {
        let mut k = DrumPad::new(36, "808 kick", PadVoice::Kick);
        k.model = 1;
        k.tune = 2.0;
        k.decay = 0.9;
        k.pitch_drop = 4.0;
        k.pitch_decay = 0.018;
        k.click = 0.45;
        k.tone = 1_600.0;
        k.sigh = 0.35;
        k.drive = 0.1;
        k
    }

    /// A 909 kick: a saw shaped to a near-sine with a deep, fast pitch fall and a click of
    /// noise and pulse.
    pub fn kick_909() -> DrumPad {
        let mut k = DrumPad::new(36, "909 kick", PadVoice::Kick);
        k.model = 2;
        k.tune = 3.0;
        k.decay = 0.45;
        k.pitch_drop = 26.0;
        k.pitch_decay = 0.028;
        k.click = 0.55;
        k.tone = 4_000.0;
        k.drive = 0.25;
        k
    }

    /// A modal snare: the head's modes, wires that rattle while it moves, a snappy tail.
    pub fn snare_modal() -> DrumPad {
        let mut s = DrumPad::new(38, "Snare", PadVoice::Snare);
        s.model = 1;
        s.decay = 0.2;
        s.noise_decay = 0.24;
        s.tone = 2_600.0;
        s.mix = 0.5;
        s.wires = 0.6;
        s.position = 0.35;
        s.hardness = 0.65;
        s
    }

    /// A clap (GM 39): four hands ten milliseconds apart, then the room.
    pub fn clap() -> DrumPad {
        let mut c = DrumPad::new(39, "Clap", PadVoice::Clap);
        c.decay = 0.28;
        c.tone = 1_150.0;
        c.level_db = -1.0;
        c
    }

    /// A modal tom on `pitch`, `tune` semitones from A3 (220 Hz).
    pub fn tom(pitch: Pitch, name: &str, tune: f32) -> DrumPad {
        let mut t = DrumPad::new(pitch, name, PadVoice::Tom);
        t.tune = tune;
        t.decay = 0.7;
        t.shell = 0.65;
        t.level_db = -2.0;
        t
    }

    /// The GM pedal hat (44): the foot's chick, in the hats' choke group.
    pub fn pedal_hat() -> DrumPad {
        let mut p = DrumPad::closed_hat();
        p.pitch = 44;
        p.name = "Pedal hat".into();
        p.decay = 0.03;
        p.tone = 8_000.0;
        p.level_db = -6.0;
        p
    }

    /// An 808 cymbal (GM 49): two bands of the six squares, each with its own decays.
    pub fn crash() -> DrumPad {
        let mut c = DrumPad::new(49, "Crash", PadVoice::HiHat);
        c.model = 1;
        c.decay = 1.8;
        c.tone = 7_100.0;
        c.hp_cutoff = 3_000.0;
        c.band_mix = 0.55;
        c.level_db = -5.0;
        c
    }

    /// A modal ride (GM 51): a dense inharmonic bank and a shimmer that swells.
    pub fn ride() -> DrumPad {
        let mut r = DrumPad::new(51, "Ride", PadVoice::HiHat);
        r.model = 3;
        r.decay = 2.6;
        r.tone = 6_500.0;
        r.hp_cutoff = 2_500.0;
        r.swell_ms = 50.0;
        r.level_db = -7.0;
        r
    }

    /// The 808 cowbell (GM 56): two squares at 540 and 800 Hz through a band-pass.
    pub fn cowbell() -> DrumPad {
        let mut c = DrumPad::new(56, "Cowbell", PadVoice::HiHat);
        c.model = 2;
        c.decay = 0.35;
        c.tone = 2_600.0;
        c.level_db = -8.0;
        c
    }

    /// Claves (GM 75): a hard wooden bar.
    pub fn claves() -> DrumPad {
        let mut c = DrumPad::tom(75, "Claves", 40.0);
        c.material = 2;
        c.decay = 0.12;
        c.hardness = 0.9;
        c.position = 0.1;
        c.level_db = -6.0;
        c
    }
}

impl PulsusParams {
    /// The full Pulsus II kit on the General MIDI map: every voice, the hats choking each
    /// other and following the pedal, modal toms, an 808 crash and a modal ride.
    pub fn studio() -> PulsusParams {
        let mut open = DrumPad::open_hat();
        open.pedal = true;
        open.decay = 0.12;
        open.open_decay = 0.9;
        open.openness = 1.0;
        PulsusParams::of(vec![
            DrumPad::kick(),
            DrumPad::rim(),
            DrumPad::snare_modal(),
            DrumPad::clap(),
            DrumPad::closed_hat(),
            DrumPad::pedal_hat(),
            DrumPad::tom(45, "Low tom", -7.0),
            open,
            DrumPad::tom(47, "Mid tom", -3.0),
            DrumPad::crash(),
            DrumPad::tom(50, "High tom", 2.0),
            DrumPad::ride(),
            DrumPad::cowbell(),
            DrumPad::claves(),
        ])
    }
}

/// A factory kit: a label key, its tags (`pulsus.kit.tag.*`), and the kit.
#[derive(Clone, Debug)]
pub struct FactoryKit {
    pub label_key: &'static str,
    pub tags: &'static [&'static str],
    pub kit: PulsusParams,
}

fn named(mut pad: DrumPad, pitch: Pitch, name: &str) -> DrumPad {
    pad.pitch = pitch;
    pad.name = name.to_string();
    pad
}

impl PulsusParams {
    /// The factory kits, from the classic seven pads to the full studio kit, the two drum
    /// machines and four styles. Each is a real kit on the General MIDI map.
    pub fn factory() -> Vec<FactoryKit> {
        let tom = |pitch, name: &str, tune, shell, damping| {
            let mut t = DrumPad::tom(pitch, name, tune);
            t.shell = shell;
            t.damping = damping;
            t
        };
        let shaker = || {
            let mut s = DrumPad::closed_hat();
            s.pitch = 70;
            s.name = "Shaker".into();
            s.choke = 0;
            s.decay = 0.035;
            s.tone = 12_000.0;
            s.hp_cutoff = 9_000.0;
            s.level_db = -9.0;
            s
        };
        let machine_808 = {
            let mut snare = DrumPad::snare();
            snare.tune = 2.0;
            snare.tone = 2_800.0;
            snare.mix = 0.6;
            let mut rim = named(DrumPad::claves(), 37, "Rim");
            rim.tune = 30.0;
            rim.decay = 0.05;
            let mut open = DrumPad::open_hat();
            open.decay = 0.5;
            PulsusParams::of(vec![
                DrumPad::kick_808(),
                rim,
                snare,
                DrumPad::clap(),
                DrumPad::closed_hat(),
                tom(45, "Low tom", -7.0, 1.0, 1.0),
                open,
                tom(47, "Mid tom", -2.0, 1.0, 1.0),
                DrumPad::crash(),
                tom(50, "High tom", 3.0, 1.0, 1.0),
                DrumPad::cowbell(),
                shaker(),
                DrumPad::claves(),
            ])
        };
        let machine_909 = {
            let mut snare = DrumPad::snare();
            snare.tone = 3_500.0;
            snare.mix = 0.65;
            snare.decay = 0.1;
            let mut closed = DrumPad::closed_hat();
            closed.tone = 10_500.0;
            let mut crash = DrumPad::crash();
            crash.tone = 8_000.0;
            PulsusParams::of(vec![
                DrumPad::kick_909(),
                DrumPad::rim(),
                snare,
                DrumPad::clap(),
                closed,
                tom(45, "Low tom", -5.0, 0.6, 0.6),
                DrumPad::open_hat(),
                tom(47, "Mid tom", 0.0, 0.6, 0.6),
                crash,
                tom(50, "High tom", 5.0, 0.6, 0.6),
                DrumPad::ride(),
            ])
        };
        let boom_bap = {
            let mut kick = DrumPad::kick();
            kick.tune = -1.0;
            kick.drive = 0.45;
            kick.filter = 1;
            kick.cutoff = 3_500.0;
            let mut snare = DrumPad::snare();
            snare.tone = 1_500.0;
            snare.decay = 0.16;
            snare.drive = 0.3;
            snare.filter = 1;
            snare.cutoff = 6_000.0;
            let mut closed = DrumPad::closed_hat();
            closed.decay = 0.04;
            closed.filter = 1;
            closed.cutoff = 7_000.0;
            closed.vel_level = 140.0;
            let mut open = DrumPad::open_hat();
            open.filter = 1;
            open.cutoff = 7_000.0;
            PulsusParams::of(vec![kick, DrumPad::rim(), snare, closed, open, shaker()])
        };
        let electro = {
            let mut kick = DrumPad::kick_909();
            kick.drive = 0.6;
            kick.drive_mode = 2;
            let mut snare = DrumPad::snare_modal();
            snare.decay = 0.12;
            snare.noise_decay = 0.15;
            let mut clap = DrumPad::clap();
            clap.bursts = 3;
            clap.spacing_ms = 7.0;
            let mut closed = DrumPad::closed_hat();
            closed.tone = 11_000.0;
            let mut laser = tom(45, "Low tom", -5.0, 0.2, 0.3);
            laser.glide = 4.0;
            laser.pitch_env = 7.0;
            let mut laser2 = tom(47, "Mid tom", 2.0, 0.2, 0.3);
            laser2.glide = 4.0;
            laser2.pitch_env = 7.0;
            PulsusParams::of(vec![
                kick,
                snare,
                clap,
                closed,
                laser,
                DrumPad::open_hat(),
                laser2,
                DrumPad::cowbell(),
            ])
        };
        let percussion = {
            let mut bell = tom(53, "Bell", 24.0, 0.5, 0.3);
            bell.material = 4;
            bell.decay = 1.6;
            bell.hardness = 0.8;
            let mut block = tom(76, "Woodblock", 29.0, 0.5, 0.7);
            block.material = 2;
            block.decay = 0.15;
            block.hardness = 0.85;
            PulsusParams::of(vec![
                DrumPad::clap(),
                tom(60, "Bongo", 16.0, 0.8, 0.6),
                tom(61, "Bongo", 21.0, 0.8, 0.6),
                tom(62, "Conga", 5.0, 0.8, 0.5),
                tom(63, "Conga", 9.0, 0.8, 0.5),
                bell,
                DrumPad::cowbell(),
                shaker(),
                DrumPad::claves(),
                block,
            ])
        };
        let trap = {
            let mut kick = DrumPad::kick_808();
            kick.decay = 2.6;
            kick.tune = 0.0;
            kick.polyphony = 1;
            kick.choke_fade_ms = 12.0;
            kick.drive = 0.35;
            let mut closed = DrumPad::closed_hat();
            closed.decay = 0.03;
            closed.polyphony = 1;
            closed.tone = 11_000.0;
            let mut clap = DrumPad::clap();
            clap.decay = 0.4;
            PulsusParams::of(vec![
                kick,
                DrumPad::rim(),
                DrumPad::snare_modal(),
                clap,
                closed,
                DrumPad::open_hat(),
                DrumPad::crash(),
            ])
        };
        const ELECTRONIC: &[&str] = &["pulsus.kit.tag.electronic"];
        vec![
            FactoryKit {
                label_key: "pulsus.kit.classic",
                tags: ELECTRONIC,
                kit: PulsusParams::default(),
            },
            FactoryKit {
                label_key: "pulsus.kit.studio",
                tags: &["pulsus.kit.tag.acoustic"],
                kit: PulsusParams::studio(),
            },
            FactoryKit {
                label_key: "pulsus.kit.808",
                tags: &["pulsus.kit.tag.vintage", "pulsus.kit.tag.electronic"],
                kit: machine_808,
            },
            FactoryKit {
                label_key: "pulsus.kit.909",
                tags: &["pulsus.kit.tag.vintage", "pulsus.kit.tag.electronic"],
                kit: machine_909,
            },
            FactoryKit {
                label_key: "pulsus.kit.boom_bap",
                tags: &["pulsus.kit.tag.vintage"],
                kit: boom_bap,
            },
            FactoryKit {
                label_key: "pulsus.kit.electro",
                tags: ELECTRONIC,
                kit: electro,
            },
            FactoryKit {
                label_key: "pulsus.kit.percussion",
                tags: &["pulsus.kit.tag.percussion"],
                kit: percussion,
            },
            FactoryKit {
                label_key: "pulsus.kit.trap",
                tags: ELECTRONIC,
                kit: trap,
            },
        ]
    }
}
