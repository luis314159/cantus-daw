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

/// `kit` balanced as a mixed record has it (r37:drums, ADR 0100 §11): its kick (note 36)
/// driven by `drive` more — the same peak, more body, so the groove's loudness rises under
/// Pulsus's single-pad rule (the kick holds most of a groove's loudness) — and its hats
/// (42/44/46) `hats_db` and its snare (38) `snare_db` up, nearer the kick than the 20-odd
/// and 12 LU under it they sat. The kits everyone else builds on
/// ([`PulsusParams::default`], [`PulsusParams::studio`]) are not touched.
fn mixed(mut kit: PulsusParams, drive: f32, hats_db: f32, snare_db: f32) -> PulsusParams {
    for p in &mut kit.pads {
        match p.pitch {
            36 => p.drive = (p.drive + drive).min(1.0),
            38 => p.level_db += snare_db,
            42 | 44 | 46 => p.level_db += hats_db,
            _ => {}
        }
    }
    kit
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
        // r37:drums — two more styles, synthesised by Pulsus itself (ADR 0100 §8).
        let house = {
            let mut kick = DrumPad::kick_909();
            kick.decay = 0.38;
            kick.tune = -1.0;
            kick.click = 0.35;
            let mut clap = DrumPad::clap();
            clap.decay = 0.45;
            clap.bursts = 4;
            clap.spacing_ms = 9.0;
            let mut closed = DrumPad::closed_hat();
            closed.tone = 10_000.0;
            closed.decay = 0.045;
            let mut open = DrumPad::open_hat();
            open.decay = 0.32;
            open.tone = 9_000.0;
            let mut ride = DrumPad::ride();
            ride.level_db = -4.0;
            let mut shake = shaker();
            shake.level_db = -11.0;
            PulsusParams::of(vec![
                kick,
                DrumPad::rim(),
                DrumPad::snare(),
                clap,
                closed,
                open,
                ride,
                shake,
            ])
        };
        let lofi = {
            let dull = |mut p: DrumPad, cutoff: f32| {
                p.filter = 1;
                p.cutoff = cutoff;
                p
            };
            let mut kick = dull(DrumPad::kick(), 2_200.0);
            kick.drive = 0.55;
            kick.decay = 0.42;
            let mut snare = dull(DrumPad::snare(), 4_000.0);
            snare.decay = 0.18;
            snare.drive = 0.4;
            snare.tone = 1_300.0;
            let mut closed = dull(DrumPad::closed_hat(), 5_000.0);
            closed.decay = 0.05;
            closed.vel_level = 140.0;
            let mut open = dull(DrumPad::open_hat(), 5_000.0);
            open.decay = 0.28;
            let mut rim = dull(DrumPad::rim(), 3_500.0);
            rim.level_db = -3.0;
            let mut shake = dull(shaker(), 9_000.0);
            shake.level_db = -5.0;
            PulsusParams::of(vec![kick, rim, snare, closed, open, shake])
        };
        // r37:drums — six more styles, synthesised by Pulsus itself (ADR 0100 §8).
        let rock = {
            let mut kick = DrumPad::kick();
            kick.tune = -2.0;
            kick.click = 0.5;
            kick.drive = 0.2;
            let mut snare = DrumPad::snare_modal();
            snare.decay = 0.25;
            snare.tone = 2_200.0;
            let mut crash = DrumPad::crash();
            crash.decay = 2.2;
            PulsusParams::of(vec![
                kick,
                DrumPad::rim(),
                snare,
                DrumPad::closed_hat(),
                DrumPad::pedal_hat(),
                tom(43, "Floor tom", -9.0, 0.9, 0.3),
                DrumPad::open_hat(),
                tom(47, "Mid tom", -3.0, 0.9, 0.3),
                crash,
                tom(50, "High tom", 2.0, 0.9, 0.3),
                DrumPad::ride(),
            ])
        };
        let pop = {
            let mut kick = DrumPad::kick_909();
            kick.decay = 0.3;
            kick.click = 0.4;
            let mut snare = DrumPad::snare();
            snare.tone = 2_800.0;
            snare.mix = 0.55;
            let mut clap = DrumPad::clap();
            clap.level_db = -3.0;
            let mut closed = DrumPad::closed_hat();
            closed.tone = 11_000.0;
            PulsusParams::of(vec![
                kick,
                snare,
                clap,
                closed,
                DrumPad::open_hat(),
                DrumPad::crash(),
                shaker(),
            ])
        };
        let rnb = {
            let mut kick = DrumPad::kick_808();
            kick.decay = 0.9;
            kick.tune = -2.0;
            let mut rim = DrumPad::rim();
            rim.tune = 4.0;
            let mut snare = DrumPad::snare_modal();
            snare.decay = 0.14;
            snare.level_db = -2.0;
            let mut clap = DrumPad::clap();
            clap.decay = 0.3;
            clap.spacing_ms = 11.0;
            let mut closed = DrumPad::closed_hat();
            closed.decay = 0.03;
            closed.tone = 9_500.0;
            PulsusParams::of(vec![
                kick,
                rim,
                snare,
                clap,
                closed,
                DrumPad::open_hat(),
                shaker(),
                DrumPad::claves(),
            ])
        };
        let reggaeton = {
            let mut kick = DrumPad::kick_808();
            kick.decay = 0.5;
            kick.drive = 0.3;
            let mut snare = DrumPad::snare();
            snare.decay = 0.08;
            snare.tone = 3_000.0;
            snare.mix = 0.7;
            let mut timbale = tom(65, "Timbale", 14.0, 0.4, 0.5);
            timbale.material = 4;
            timbale.decay = 0.4;
            PulsusParams::of(vec![
                kick,
                DrumPad::rim(),
                snare,
                DrumPad::clap(),
                DrumPad::closed_hat(),
                DrumPad::open_hat(),
                DrumPad::cowbell(),
                tom(62, "Conga", 5.0, 0.8, 0.5),
                tom(63, "Conga", 9.0, 0.8, 0.5),
                timbale,
                shaker(),
            ])
        };
        let cinematic = {
            let mut kick = DrumPad::kick();
            kick.tune = -5.0;
            kick.decay = 1.2;
            kick.drive = 0.3;
            let boom = |pitch, name: &str, t| {
                let mut x = tom(pitch, name, t, 1.0, 0.2);
                x.decay = 1.6;
                x
            };
            let mut snare = DrumPad::snare_modal();
            snare.decay = 0.5;
            let mut crash = DrumPad::crash();
            crash.decay = 3.5;
            PulsusParams::of(vec![
                kick,
                snare,
                boom(41, "Low boom", -12.0),
                boom(43, "Boom", -9.0),
                boom(45, "High boom", -6.0),
                crash,
                DrumPad::ride(),
            ])
        };
        let jazz = {
            let mut kick = DrumPad::kick();
            kick.decay = 0.3;
            // r37:drums — a jazz kick under the brushes (its peak under the brush snare's)
            // but driven for body, the kit's gain up by what the brush leaves (ADR 0100 §11).
            kick.level_db = -4.0;
            kick.drive = 0.6;
            let mut brush = DrumPad::snare();
            brush.name = "Brush snare".into();
            brush.mix = 0.85;
            brush.noise_decay = 0.3;
            brush.decay = 0.2;
            brush.level_db = -3.0;
            let mut ride = DrumPad::ride();
            ride.decay = 3.0;
            let mut crash = DrumPad::crash();
            crash.decay = 2.5;
            crash.level_db = -4.0;
            PulsusParams::of(vec![
                kick,
                DrumPad::rim(),
                brush,
                DrumPad::closed_hat(),
                DrumPad::pedal_hat(),
                tom(45, "Low tom", -5.0, 0.7, 0.4),
                tom(48, "High tom", 3.0, 0.7, 0.4),
                crash,
                ride,
            ])
        };
        let mut kits = vec![
            FactoryKit {
                label_key: "pulsus.kit.classic",
                tags: &[
                    "pulsus.kit.tag.electronic",
                    "preset.style.pop",
                    "preset.role.drums",
                ],
                kit: mixed(PulsusParams::default(), 0.4, 3.0, 3.0),
            },
            FactoryKit {
                label_key: "pulsus.kit.studio",
                tags: &[
                    "pulsus.kit.tag.acoustic",
                    "preset.style.rock",
                    "preset.style.pop",
                    "preset.role.drums",
                ],
                kit: mixed(PulsusParams::studio(), 0.4, 3.0, 2.0),
            },
            FactoryKit {
                label_key: "pulsus.kit.808",
                tags: &[
                    "pulsus.kit.tag.vintage",
                    "pulsus.kit.tag.electronic",
                    "preset.style.trap",
                    "preset.style.rnb",
                    "preset.role.drums",
                ],
                kit: machine_808,
            },
            FactoryKit {
                label_key: "pulsus.kit.909",
                tags: &[
                    "pulsus.kit.tag.vintage",
                    "pulsus.kit.tag.electronic",
                    "preset.style.house",
                    "preset.role.drums",
                ],
                kit: machine_909,
            },
            FactoryKit {
                label_key: "pulsus.kit.boom_bap",
                tags: &[
                    "pulsus.kit.tag.vintage",
                    "preset.style.trap",
                    "preset.style.lofi",
                    "preset.role.drums",
                ],
                kit: boom_bap,
            },
            FactoryKit {
                label_key: "pulsus.kit.electro",
                tags: &[
                    "pulsus.kit.tag.electronic",
                    "preset.style.house",
                    "preset.role.drums",
                ],
                kit: electro,
            },
            FactoryKit {
                label_key: "pulsus.kit.percussion",
                tags: &[
                    "pulsus.kit.tag.percussion",
                    "preset.style.latin",
                    "preset.role.drums",
                ],
                kit: percussion,
            },
            FactoryKit {
                label_key: "pulsus.kit.trap",
                tags: &[
                    "pulsus.kit.tag.electronic",
                    "preset.style.trap",
                    "preset.role.drums",
                ],
                kit: trap,
            },
            FactoryKit {
                label_key: "pulsus.kit.house", // r37:drums
                tags: &[
                    "pulsus.kit.tag.electronic",
                    "preset.style.house",
                    "preset.role.drums",
                ],
                kit: house,
            },
            FactoryKit {
                label_key: "pulsus.kit.lofi", // r37:drums
                tags: &[
                    "pulsus.kit.tag.vintage",
                    "preset.style.lofi",
                    "preset.role.drums",
                ],
                kit: lofi,
            },
            FactoryKit {
                label_key: "pulsus.kit.rock",
                tags: &[
                    "pulsus.kit.tag.acoustic",
                    "preset.style.rock",
                    "preset.role.drums",
                ],
                kit: mixed(rock, 0.4, 3.0, 2.0),
            },
            FactoryKit {
                label_key: "pulsus.kit.pop",
                tags: &[
                    "pulsus.kit.tag.electronic",
                    "preset.style.pop",
                    "preset.role.drums",
                ],
                kit: pop,
            },
            FactoryKit {
                label_key: "pulsus.kit.rnb",
                tags: &[
                    "pulsus.kit.tag.electronic",
                    "preset.style.rnb",
                    "preset.role.drums",
                ],
                kit: rnb,
            },
            FactoryKit {
                label_key: "pulsus.kit.reggaeton",
                tags: &[
                    "pulsus.kit.tag.electronic",
                    "preset.style.latin",
                    "preset.role.drums",
                ],
                kit: reggaeton,
            },
            FactoryKit {
                label_key: "pulsus.kit.cinematic",
                tags: &[
                    "pulsus.kit.tag.acoustic",
                    "preset.style.film",
                    "preset.style.ambient",
                    "preset.role.drums",
                ],
                kit: cinematic,
            },
            FactoryKit {
                label_key: "pulsus.kit.jazz",
                tags: &[
                    "pulsus.kit.tag.acoustic",
                    "preset.style.jazz",
                    "preset.role.drums",
                ],
                kit: jazz,
            },
        ];
        // r37:drums — each kit's own gain (ADR 0100 §11): the lesser of what brings its
        // groove to the drums window's centre (−19 LUFS on its phrase), what Pulsus's
        // headroom rule leaves (no pad alone over 0.6 peak, less 0.3 dB) and what keeps a
        // dense riff under full scale. Measured by `inst_pulsus.rs` (each pad's peak, the
        // room), `preset_render.rs` (the level) and `instrument_levels.rs` (the riff).
        for f in &mut kits {
            f.kit.kit.gain_db = match f.label_key {
                "pulsus.kit.classic" => 4.3,
                "pulsus.kit.studio" => 4.3,
                "pulsus.kit.808" => 2.0, // the dense riff stays under full scale (−0.9 dBFS)
                "pulsus.kit.909" => 4.5,
                "pulsus.kit.boom_bap" => 4.3,
                "pulsus.kit.electro" => 1.0,
                "pulsus.kit.percussion" => 8.9,
                "pulsus.kit.trap" => -0.7,
                "pulsus.kit.house" => 4.6,
                "pulsus.kit.lofi" => 4.3,
                "pulsus.kit.rock" => 4.3,
                "pulsus.kit.pop" => 4.6,
                "pulsus.kit.rnb" => 4.1,
                "pulsus.kit.reggaeton" => 4.0,
                "pulsus.kit.cinematic" => 4.3,
                "pulsus.kit.jazz" => 7.5,
                _ => 0.0,
            };
        }
        kits
    }
}
