//! How a modelled kit's pieces sound: each articulation's pad for Pulsus's engine — model 4,
//! **acoustic** (`cantus_dsp::pulsus::acoustic`), for the drums and cymbals, measured
//! against recorded kits (`tympanum_measure`, round 40 phase 3) — sized and tuned by the
//! piece (its diameter sets the pitch, its depth the heads' coupling) before the piece's
//! own keys (`p{n}.tune/damp/wires`) move it.
//!
//! What a field means to model 4 (`DrumPad`'s names kept): `tune` sets the lowest mode
//! (220 Hz × 2^(tune/12) for a drum, 330 Hz for a cymbal); `decay` its T60; `damping` how
//! much faster the high modes die; `position` centre → edge (a cymbal: bell → bow → edge);
//! `hardness` the stick (with the velocity: velocity → timbre); `glide` the semitones a
//! full-force hit starts sharp; `shell` the second head's coupling; `click` the stick's or
//! beater's own sound and `tone` its band; `wires` and `noise_decay` a snare's wires;
//! `band_mix` a rimshot's rim or a hat's rattle half open; `swell_ms` how long a cymbal's
//! wash takes to build.

use super::{Articulation, PieceDef, PieceKind};
use crate::pads::{DrumPad, PadVoice};

/// Semitones from `base` Hz to `hz`.
fn semis(hz: f32, base: f32) -> f32 {
    12.0 * cantus_dsp::math::log2(hz / base)
}

/// Whether the articulation plays model 4 (the acoustic drum or cymbal); the rest keep
/// Pulsus's own voices (a cowbell, claves).
pub fn piece_acoustic(p: &PieceDef, _art: &Articulation) -> bool {
    !matches!(
        p.kind,
        PieceKind::Cowbell | PieceKind::Tambourine | PieceKind::Percussion
    )
}

/// How an articulation plays model 5, **measured** (`cantus_dsp::pulsus::measured`:
/// synthesised from tables measured off DRSKit's recordings).
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Measured {
    /// 1 the crash, 2 the hi-hat, 3 the ride, 4 the kick, 5 the snare, 6–8 the toms.
    pub table: u8,
    /// The hat's foot (the pedal's articulation).
    pub foot: bool,
    /// A ride's zone (0 the bow, 1 the bell) or a snare's (0 the head, 0.5 the rimshot,
    /// 1 the sidestick).
    pub zone: f32,
    /// Semitones from the table's own cymbal, and its decay's factor.
    pub tune: f32,
    pub decay: f32,
}

/// The measured tables a modelled kit plays: the studio kit DRSKit's (ids 1–8), the jazz
/// kit Virtuosity Drums' (the same pieces, 16 on: 17–24).
pub fn measured_set(kit: &str) -> u8 {
    if kit == "jazz" {
        16
    } else {
        0
    }
}

/// The measured crash's own size, inches (DRSKit's 18″).
const CRASH_IN: f32 = 18.0;

/// Whether the articulation plays model 5, and how. The crash's kin with no recording of
/// their own — the splash, the china, the stack — play the crash's table sized to them (a
/// smaller plate higher and shorter); a china's own trash is not modelled yet.
pub fn piece_measured(p: &PieceDef, art: &Articulation) -> Option<Measured> {
    let m = |table: u8| Measured {
        table,
        foot: false,
        zone: 0.0,
        tune: 0.0,
        decay: 1.0,
    };
    match p.kind {
        PieceKind::Crash => Some(m(1)),
        PieceKind::HiHat => Some(Measured {
            foot: art.id == "pedal",
            ..m(2)
        }),
        PieceKind::Ride => Some(Measured {
            zone: if art.id == "bell" { 1.0 } else { 0.0 },
            ..m(3)
        }),
        PieceKind::Kick => Some(Measured {
            // the kick's open stroke (the beater off the head) rings longer
            decay: if art.id == "open" { 1.4 } else { 1.0 },
            ..m(4)
        }),
        PieceKind::Snare => Some(Measured {
            zone: match art.id.as_str() {
                "rimshot" => 0.5,
                "sidestick" => 1.0,
                _ => 0.0,
            },
            ..m(5)
        }),
        PieceKind::RackTom | PieceKind::FloorTom => {
            // DRSKit's toms, each the size its measured one is (12″, 14″, 16″), a head's
            // pitch falling as its diameter grows; the second centre a semitone up.
            let inches = (p.diameter / 2.54).max(6.0);
            let (table, own) = if inches <= 13.0 {
                (6, 12.0)
            } else if inches <= 15.0 {
                (7, 14.0)
            } else {
                (8, 16.0)
            };
            let second = if art.id == "center2" { 1.0 } else { 0.0 };
            Some(Measured {
                tune: 12.0 * cantus_dsp::math::log2(own / inches) + second,
                ..m(table)
            })
        }
        PieceKind::Splash | PieceKind::China | PieceKind::Stack => {
            let inches = (p.diameter / 2.54).max(6.0);
            let ratio = CRASH_IN / inches;
            Some(Measured {
                tune: 12.0 * cantus_dsp::math::log2(ratio),
                decay: (1.0 / ratio).clamp(0.3, 1.5),
                ..m(1)
            })
        }
        _ => None,
    }
}

fn drum(art: &Articulation, name: &str, voice: PadVoice, hz: f32) -> DrumPad {
    let mut d = DrumPad::new(art.key, name, voice);
    d.tune = semis(hz, 220.0);
    d.modes = 40;
    // The shell's ring (`mix`, which model 4's drums read so).
    d.mix = 0.0;
    d
}

fn metal(art: &Articulation, name: &str, hz: f32) -> DrumPad {
    let mut c = DrumPad::new(art.key, name, PadVoice::HiHat);
    c.tune = semis(hz, 330.0);
    c
}

/// The pad a modelled kit's articulation plays.
pub fn piece_pad(p: &PieceDef, art: &Articulation) -> DrumPad {
    let name = p.name.as_str();
    let dia = p.diameter.max(6.0);
    let mut d = match p.kind {
        PieceKind::Kick => {
            // A 22″ kick: a felt beater, both heads, its fundamental near 55 Hz; a hard hit
            // pushes the head sharp by a few semitones.
            // (Diameters are in cm: a 22″ kick is 55.9.)
            let mut k = drum(art, name, PadVoice::Kick, 3_480.0 / dia);
            k.decay = 0.42;
            k.damping = 0.42;
            k.position = 0.42;
            k.hardness = 0.55;
            k.glide = 3.0;
            k.shell = 0.85;
            k.mix = 1.0;
            k.modes = 32;
            k.click = 0.55;
            k.tone = 2_600.0;
            k.level_db = 0.0;
            k
        }
        PieceKind::Snare => {
            // A 14″ snare near 208 Hz.
            let mut s = drum(art, name, PadVoice::Snare, 7_400.0 / dia);
            s.decay = 0.34;
            s.damping = 0.5;
            s.position = 0.3;
            s.hardness = 0.72;
            s.glide = 0.7;
            s.shell = 0.6;
            s.mix = 0.4;
            s.modes = 48;
            s.click = 0.45;
            s.tone = 3_600.0;
            s.wires = 0.9;
            s.noise_decay = 0.3;
            // Levels set against DRSKit's through the same engine (`tympanum_measure`).
            s.level_db = 5.0;
            s
        }
        PieceKind::RackTom | PieceKind::FloorTom => {
            // A tom's pitch falls with its size: 10″ near 200 Hz, 16″ near 115.
            let hz = 200.0 * cantus_dsp::math::powf(25.4 / dia.max(10.0), 1.2);
            let mut t = drum(art, name, PadVoice::Tom, hz);
            t.decay = 0.55 + dia / 45.0;
            t.damping = 0.4;
            t.mix = 0.35;
            t.position = 0.25;
            t.hardness = 0.66;
            t.glide = 1.1;
            t.shell = (p.depth / 30.0).clamp(0.5, 0.95);
            t.click = 0.3;
            t.tone = 3_000.0;
            t.level_db = 3.5;
            t
        }
        PieceKind::HiHat => {
            // A 14″ hat's lowest mode near 365 Hz.
            let mut h = metal(art, name, 13_000.0 / dia);
            h.decay = 0.25;
            h.open_decay = 3.0;
            h.hardness = 0.75;
            h.position = 0.65;
            h.tone = 10_500.0;
            h.swell_ms = 40.0;
            h.click = 0.55;
            h.band_mix = 0.6;
            h.choke = p.choke.unwrap_or(1);
            h.pedal = true;
            h.openness = art.openness.map_or(0.0, |o| o.at);
            h.level_db = -2.0;
            h
        }
        PieceKind::Crash | PieceKind::Splash | PieceKind::China | PieceKind::Stack => {
            // Bigger cymbals ring longer and lower.
            // An 18″ crash near 444 Hz (DRSKit's crashes: 430–530).
            let mut c = metal(art, name, 20_300.0 / dia);
            c.decay = 0.8 + dia / 15.0;
            c.hardness = 0.7;
            c.position = 0.9;
            c.tone = 5_000.0;
            c.swell_ms = 110.0;
            // The stick's ping on the edge first (DRSKit's and MuldjordKit's crashes peak
            // within 70 ms), the wash after.
            c.click = 1.0;
            c.level_db = -24.0;
            if p.kind == PieceKind::China {
                c.drive = 0.35;
                c.swell_ms = 60.0;
            }
            c
        }
        PieceKind::Ride => {
            // A 20″ ride near 400 Hz.
            let mut r = metal(art, name, 20_300.0 / dia);
            r.decay = 5.5;
            r.hardness = 0.85;
            r.position = 0.5;
            r.tone = 6_500.0;
            r.swell_ms = 900.0;
            // A ride is its stick's ping first (DRSKit's peaks at once), its wash after.
            r.click = 2.0;
            r.level_db = -23.0;
            r
        }
        PieceKind::Cowbell => DrumPad::cowbell(),
        PieceKind::Tambourine | PieceKind::Percussion => DrumPad::claves(),
    };
    // The articulations' differences.
    match art.id.as_str() {
        "rimshot" => {
            d.level_db -= 6.0;
            d.hardness = 0.95;
            d.position = 0.85;
            d.click = 0.9;
            d.band_mix = 1.0;
        }
        "sidestick" => {
            // The stick laid across, its butt on the rim: a bright wooden click and the
            // rim's ring, the head barely moving, the wires a whisper.
            d.hardness = 1.0;
            d.position = 0.97;
            d.click = 1.6;
            d.tone = 3_800.0;
            d.band_mix = 0.55;
            d.wires = 0.15;
            d.modes = 20;
            d.decay = 0.12;
            d.glide = 0.0;
            d.level_db -= 9.0;
        }
        "open" if p.kind == PieceKind::Kick => {
            d.damping = 0.4;
            d.decay *= 1.6;
        }
        "center2" => d.tune += 1.0,
        "splash" => {
            // The foot's splash: the plates clash (no stick) and ring open a moment, on
            // their own (the pedal does not hold them), until the next hat stroke chokes them.
            d.click = 0.0;
            d.hardness = 0.35;
            d.pedal = false;
            d.decay = 0.9;
            d.level_db -= 3.0;
        }
        "pedal" => {
            // The foot: the plates clap, no stick.
            d.click = 0.0;
            d.hardness = 0.3;
            d.decay = 0.12;
        }
        "bell" => {
            d.position = 0.05;
            d.click = 0.6;
            d.decay *= 0.8;
            d.level_db += 11.0;
        }
        "edge" if p.kind == PieceKind::Ride => {
            d.position = 0.9;
            d.swell_ms = 150.0;
            d.click = 0.4;
        }
        _ => {}
    }
    d.pitch = art.key;
    d
}
