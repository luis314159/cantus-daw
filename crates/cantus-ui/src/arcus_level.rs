//! r42:arcus-p5 — Arcus's level heard as music (`docs/round42/arcus-match.md`, *Pass 5*): one
//! common phrase — a cantabile line in D major at 74 BPM, each note its own bow but a slurred
//! pair, a CC1 swell from p to f and back — played by each bowed preset an octave apart in its
//! instrument's own register (the violin from D5, the viola D4, the cello D3, the bass D2).
//! The presets' gains are fitted to [`TARGET`] on it (`tools/arcus/presets.sh <dir> fit`):
//! the audition phrases a preset browser plays are velocity-only and sit in no instrument's
//! register, and the demo heard the violin and the viola 6–7 LU under the cello and the bass
//! with CC1 dynamics. A pizzicato preset keeps its role's fit (`preset_measure`).

use crate::preset_audition as aud;
use crate::preset_tags::Role;
use cantus_core::clip_lanes::{ClipLane, LaneKind};
use cantus_core::note_art::{Articulation, NoteArt};
use cantus_core::{AutoPoint, InstrumentSpec, NativeSpec, Project, Ticks, PPQ};

/// The phrase's tempo.
pub const BPM: f64 = 74.0;
/// The phrase's length, beats (the render runs its tail past it).
pub const BEATS: f64 = 24.0;
/// The level every bowed preset is fitted to on the phrase, integrated LUFS (the cello's
/// cantabile where round 42 left it).
pub const TARGET: f64 = -19.0;
/// The most the fitted presets may differ on the phrase, LU (the gate's).
pub const PARITY_LU: f64 = 1.5;

/// The phrase as scale degrees from the tonic (semitones), beats, and whether the note is
/// slurred from the one before.
const NOTES: [(i32, f64, bool); 16] = [
    (0, 1.0, false),
    (2, 1.0, false),
    (4, 1.0, false),
    (5, 1.0, false),
    (7, 2.0, false),
    (5, 1.0, false),
    (4, 1.0, true),
    (2, 1.0, false),
    (4, 1.0, false),
    (7, 1.0, false),
    (9, 2.0, false),
    (7, 1.0, false),
    (5, 1.0, false),
    (4, 1.0, false),
    (2, 1.0, false),
    (0, 4.0, false),
];

/// CC1 along the phrase (beat, value): p, a crescendo to f, back to mp.
const CC1: [(f64, f32); 4] = [(0.0, 0.35), (8.0, 0.8), (14.0, 0.75), (21.0, 0.45)];

/// The tonic an instrument plays the phrase from (`inst`: the spec's `inst`).
pub fn tonic(inst: f32) -> u8 {
    match inst.round() as i32 {
        1 => 38, // the bass: D2
        2 => 74, // the violin: D5
        3 => 62, // the viola: D4
        _ => 50, // the cello: D3
    }
}

fn tick(beats: f64) -> Ticks {
    Ticks((beats * PPQ as f64).round() as i64)
}

/// The phrase's project for a preset's `spec`, at [`BPM`].
pub fn project(spec: &NativeSpec) -> Project {
    let mut p = aud::phrase_project_at(InstrumentSpec::Native(spec.clone()), Role::Lead, BPM);
    let root = tonic(spec.get("inst")) as i32;
    let mut clip = p.make_clip("music", Ticks::ZERO, tick(BEATS));
    let mut at = 0.0;
    for (deg, len, slurred) in NOTES {
        let mut n = p.make_note(tick(at), tick(len), (root + deg) as u8, 84);
        if slurred {
            n.art = Some(NoteArt {
                articulation: Articulation::Legato,
                bow: None,
                start: None,
                part: None,
                string: None,
            });
        }
        clip.notes.push(n);
        at += len;
    }
    clip.lanes = vec![ClipLane::new(
        LaneKind::Cc(1),
        CC1.iter()
            .map(|(b, v)| AutoPoint::new(tick(*b), *v))
            .collect(),
    )];
    p.tracks[0].clips = vec![clip];
    p
}

/// Whether a preset is fitted on the phrase: a bowed one (arco or tremolo), not a pizzicato.
pub fn is_bowed(spec: &NativeSpec) -> bool {
    spec.get("tech").round() as i32 != 1
}
