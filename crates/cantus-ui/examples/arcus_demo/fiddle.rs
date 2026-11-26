//! r42 arcus-expr — the fiddle (the owner, 2026-11-25: "we should also be able to play country
//! on the violin, it sounds very different from the classical style"): *Hoedown en re*, an
//! original tune for the fiddle with a honky-tonk piano (Chorda), a pizzicato bass (Arcus) and a
//! light kit (Tympanum), built in a running app as the quartet is; and one passage played twice
//! on the violin alone, classical then fiddle.
//!
//! The fiddle is Arcus's violin with the *Fiddle* preset (`arcus.preset.violin_fiddle`: nearer
//! the bridge, heavier, grittier, short strokes, little and late vibrato, the open strings
//! ringing), played by the fiddler's rules below — all of it entered as a user enters it:
//!
//! - **Bowing patterns**: the Nashville shuffle on the A part (each four eighths: two slurred,
//!   then two separate — long, short, short), the Georgia shuffle on the B part (one, two
//!   slurred from the off-beat, one), saw strokes (every note its own bow) where a note says `w`;
//!   the bows alternate, the slurs start on the off-beats so the accents land there.
//! - **The pulse**: the off-beat eighths a little louder (velocity and a CC11 kick) and a little
//!   late (a light lilt, 3 % of a beat), the on-beats a little held back; no long swells.
//! - **Ornaments** (per-note MPE bends): `/` a slide up from a semitone under (70 ms), `%` from a
//!   minor third under (100 ms), `g` a hammer-on grace from a whole step under (35 ms), `b` a
//!   bluesy bent note (a flatted third or seventh pushed a fifth of a tone up through the note).
//! - **Double stops and drones**: a second Arcus violin (*dobles cuerdas*) bows the open string
//!   next to the melody's — the A under an E-string note, the D under an A-string one — on the
//!   parts that ask for it, re-bowed where the melody's bow changes and in its direction (one
//!   bow on two strings, as a fiddler's flat bridge lets it; one Arcus cannot bow two strings
//!   with one bow yet, `arcus-expr.md` *Next*).
//! - **The chop** (`c`): a short stroke from the air at the frog, the fiddle and its drone
//!   together, on beats 2 and 4 of the kick-off.
//! - **Vibrato**: none, but a little and late on a long note at a phrase's end.

use super::build::{bar_tick, tick};
use super::measure;
use super::score::{parse_t, Part, Who, Written};
use cantus_core::clip_lanes::{ClipLane, LaneKind};
use cantus_core::model::note_expr::{ExprKind, ExprPoint, NoteExpr};
use cantus_core::note_art::{Articulation, Bow, BowPart, BowStart, NoteArt};
use cantus_core::{
    AutoPoint, Color, Command, Dock, EffectKind, InstrumentKind, Send, StripId, Ticks, TrackId,
};
use cantus_ui::picker::{PickChoice, PickPurpose};
use cantus_ui::preset_audition::Target;
use cantus_ui::CantusApp;
use std::path::Path;

pub const BPM: f64 = 116.0;
const BAR: f64 = 4.0;

/// The kick-off (bar 1), the A part (2–9), the B part (10–17), the A part again with double
/// stops (18–25).
const KICKOFF: &str = "r:1 D4:0.25c r:0.75 r:1 D4:0.25c r:0.25 A4:0.25 B4:0.25";
const A_PART: &str = "F#5:0.5/ E5:0.5 D5:0.5 E5:0.5 F#5:0.5 A5:0.5 B5:0.5 A5:0.5 | \
     F#5:0.5 D5:0.5 E5:0.5 C#5:0.5 D5:1 A4:1 | \
     B4:0.5 D5:0.5 F5:0.5b F#5:0.5 A5:0.5 F#5:0.5 E5:0.5 D5:0.5 | \
     E5:0.5 F#5:0.5 E5:0.5 D5:0.5 B4:0.5 A4:0.5 B4:0.5 C#5:0.5 | \
     D5:0.5% E5:0.5 F#5:0.5 D5:0.5 E5:0.5 F#5:0.5 A5:0.5g B5:0.5 | \
     A5:0.5 F#5:0.5 E5:0.5 D5:0.5 B4:0.5 D5:0.5 B4:0.5 A4:0.5 | \
     F#4:0.5w A4:0.5w B4:0.5w C#5:0.5w D5:0.5w E5:0.5w F#5:0.5w E5:0.5w | \
     D5:1 A4:0.5 F#4:0.5 D4:1 r:1";
const B_PART: &str = "A5:0.5 B5:0.5 C#6:0.5 D6:0.5 A5:1 F#5:1 | \
     G5:0.5 A5:0.5 B5:0.5 G5:0.5 E5:1 C#5:1 | \
     D5:0.5 F#5:0.5 A5:0.5 F#5:0.5 G5:0.5 F#5:0.5 E5:0.5 D5:0.5 | \
     C#5:0.5 E5:0.5 A5:0.5/ E5:0.5 C#5:0.5 E5:0.5 A4:1 | \
     A5:0.5 B5:0.5 C#6:0.5 D6:0.5 A5:1 F#5:1 | \
     G5:0.5 A5:0.5 C6:0.5b B5:0.5 G5:0.5 E5:0.5 C#5:0.5 A4:0.5 | \
     D5:0.5w F#5:0.5w A5:0.5w F#5:0.5w E5:0.5w C#5:0.5w A4:0.5w C#5:0.5w | \
     D5:1 A4:0.5 F#4:0.5 D4:2";

/// The chords bar by bar from bar 1 (D, G, A: their piano voicing and the bass's root and
/// fifth).
const CHORDS: &str = "D D D G A D G A D A G D A A G A D D D G A D G A D";

/// The bowing pattern a section's eighths take.
#[derive(Copy, Clone, PartialEq, Eq)]
pub enum Pattern {
    Nashville,
    Georgia,
}

pub struct Section {
    pub bar: u32,
    pub bars: u32,
    pub notes: &'static str,
    pub pattern: Pattern,
    pub drone: bool,
    pub cc1: f32,
    /// The tempo the section is played at (its milliseconds in beats).
    pub bpm: f64,
}

const FORM: [Section; 4] = [
    Section {
        bar: 1,
        bars: 1,
        notes: KICKOFF,
        pattern: Pattern::Nashville,
        drone: true,
        cc1: 0.7,
        bpm: BPM,
    },
    Section {
        bar: 2,
        bars: 8,
        notes: A_PART,
        pattern: Pattern::Nashville,
        drone: false,
        cc1: 0.62,
        bpm: BPM,
    },
    Section {
        bar: 10,
        bars: 8,
        notes: B_PART,
        pattern: Pattern::Georgia,
        drone: true,
        cc1: 0.7,
        bpm: BPM,
    },
    Section {
        bar: 18,
        bars: 8,
        notes: A_PART,
        pattern: Pattern::Nashville,
        drone: true,
        cc1: 0.66,
        bpm: BPM,
    },
];

/// A note as the fiddler plays it: beats from the clip's start.
#[derive(Clone)]
pub struct FNote {
    pub at: f64,
    pub len: f64,
    pub key: u8,
    pub vel: u8,
    pub art: Option<NoteArt>,
    /// The per-note bend: `(seconds from the note's start, semitones)`.
    pub bend: Vec<(f64, f64)>,
}

fn art(a: Articulation, bow: Option<Bow>) -> NoteArt {
    NoteArt {
        articulation: a,
        bow,
        start: None,
        part: None,
        string: None,
    }
}

fn unit(seed: u64, i: u64) -> f64 {
    let mut z = seed.wrapping_add(i.wrapping_mul(0x9E37_79B9_7F4A_7C15));
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^= z >> 31;
    (z >> 11) as f64 / (1u64 << 52) as f64 - 1.0
}

/// The bows of a section's notes, by its pattern: `(first, last)` each.
fn bows(w: &[Written], pattern: Pattern) -> Vec<(usize, usize)> {
    let mut out = Vec::new();
    let mut i = 0;
    let eighth = |k: usize| (w[k].len - 0.5).abs() < 1e-9 && !w[k].flags.contains('w');
    let joined = |a: usize, b: usize| (w[b].at - (w[a].at + w[a].len)).abs() < 1e-9;
    while i < w.len() {
        let beat = w[i].at.rem_euclid(2.0);
        let four = i + 3 < w.len()
            && (0..4).all(|k| eighth(i + k))
            && (1..4).all(|k| joined(i + k - 1, i + k));
        match pattern {
            // Long (two slurred), short, short — from a half-bar's start.
            Pattern::Nashville if four && beat.abs() < 1e-9 => {
                out.push((i, i + 1));
                out.push((i + 2, i + 2));
                out.push((i + 3, i + 3));
                i += 4;
            }
            // One, then two slurred from the off-beat (the accent there), then one.
            Pattern::Georgia if four && beat.abs() < 1e-9 => {
                out.push((i, i));
                out.push((i + 1, i + 2));
                out.push((i + 3, i + 3));
                i += 4;
            }
            _ => {
                out.push((i, i));
                i += 1;
            }
        }
    }
    out
}

/// The fiddle's notes and its CC1, CC11 and CC21 lanes for one section, and the drone's notes.
pub fn fiddle(sec: &Section, seed: u64) -> (Vec<FNote>, Vec<FNote>, [Vec<(f64, f32)>; 3]) {
    let part = Part {
        who: Who::Violin,
        section: 0,
        vel: 80,
        notes: sec.notes,
        cc1: &[],
    };
    let (w, _) = parse_t(&part, 0);
    let gs = bows(&w, sec.pattern);
    let beat = |s: f64| s * sec.bpm / 60.0;
    let mut notes: Vec<FNote> = Vec::new();
    let mut dir = Bow::Down;
    let mut last_end = f64::NEG_INFINITY;
    for (k, &(a, b)) in gs.iter().enumerate() {
        // A bow after a rest starts down on a beat, up on an off-beat.
        if w[a].at - last_end > 0.25 {
            dir = if w[a].at.fract().abs() < 1e-9 {
                Bow::Down
            } else {
                Bow::Up
            };
        }
        for i in a..=b {
            let x = &w[i];
            let offbeat = (x.at.fract() - 0.5).abs() < 1e-9;
            // The lilt and a few ms of a player.
            let mut at = x.at + if offbeat { 0.03 } else { 0.0 };
            at += beat(0.003 * unit(seed, i as u64)).clamp(-0.01, 0.01);
            let chop = x.flags.contains('c');
            let mut vel = if chop {
                127.0f64
            } else if offbeat {
                92.0
            } else if x.at.rem_euclid(BAR).abs() < 1e-9 {
                84.0
            } else {
                78.0
            };
            if i != a {
                vel -= 10.0; // inside a slur
            }
            let mut na = if chop {
                NoteArt {
                    start: Some(BowStart::Air),
                    part: Some(BowPart::Frog),
                    ..art(Articulation::Staccato, Some(Bow::Down))
                }
            } else if i == a {
                art(Articulation::Sustain, Some(dir))
            } else {
                art(Articulation::Legato, Some(dir))
            };
            // Short strokes in the bow's upper half; a long one from wherever it is.
            if !chop && i == a && x.len <= 0.5 {
                na.part = Some(BowPart::Middle);
            }
            let mut bend = Vec::new();
            if x.flags.contains('/') {
                bend = vec![(0.0, -1.0), (0.07, 0.0)];
            } else if x.flags.contains('%') {
                bend = vec![(0.0, -3.0), (0.1, 0.0)];
            } else if x.flags.contains('g') {
                bend = vec![(0.0, -2.0), (0.034, -2.0), (0.036, 0.0)];
            } else if x.flags.contains('b') {
                bend = vec![(0.0, 0.0), (0.12, 0.2), (60.0 / sec.bpm * x.len, 0.25)];
            }
            vel = vel.clamp(20.0, 127.0);
            notes.push(FNote {
                at,
                len: x.len,
                key: x.key,
                vel: vel as u8,
                art: Some(na),
                bend,
            });
        }
        let _ = k;
        last_end = w[b].at + w[b].len;
        dir = match dir {
            Bow::Down => Bow::Up,
            Bow::Up => Bow::Down,
        };
    }
    // Lengths: a slur's notes meet, a stroke ends where the next begins (a short gap for the
    // pulse), a chop is a chop.
    for i in 0..notes.len() {
        let end = w[i].at + w[i].len;
        let mut e = end;
        if i + 1 < notes.len() && (w[i + 1].at - end).abs() < 1e-9 {
            let same_bow = gs.iter().any(|g| g.0 <= i && i < g.1);
            e = if same_bow {
                notes[i + 1].at
            } else {
                notes[i + 1].at - 0.04
            };
        }
        if w[i].flags.contains('c') {
            e = notes[i].at + 0.2;
        }
        notes[i].len = (e - notes[i].at).max(0.06);
    }
    // The drone: the open string under the melody's, one bow with it.
    let mut drone: Vec<FNote> = Vec::new();
    if sec.drone {
        for (n, x) in notes.iter().zip(&w) {
            let d = if x.flags.contains('x') {
                Some(x.key - 3) // the train whistle: a third under, sliding with it
            } else if x.flags.contains('c') {
                Some(69) // the chop: D and A together
            } else if x.key >= 76 {
                Some(69)
            } else if x.key >= 69 {
                Some(62)
            } else {
                None
            };
            let Some(d) = d else { continue };
            // Joined to the last drone note when the melody's bow goes on (a slur) and the
            // drone's pitch is the same.
            let slurred = n
                .art
                .is_some_and(|a| a.articulation == Articulation::Legato);
            if let Some(last) = drone.last_mut() {
                if slurred && last.key == d && (last.at + last.len - n.at).abs() < 0.05 {
                    last.len = n.at + n.len - last.at;
                    continue;
                }
            }
            let mut a = n.art.unwrap_or(art(Articulation::Sustain, None));
            if a.articulation == Articulation::Legato {
                a.articulation = Articulation::Sustain;
            }
            drone.push(FNote {
                at: n.at,
                len: n.len,
                key: d,
                vel: n.vel.saturating_sub(18),
                art: Some(a),
                bend: if x.flags.contains('x') {
                    n.bend.clone()
                } else {
                    Vec::new()
                },
            });
        }
    }
    // CC1 the section's dynamic; CC11 the pulse (an off-beat kick, the on-beats held back);
    // CC21 the vibrato (none, a little and late on a long note before a rest).
    let len = f64::from(sec.bars) * BAR;
    let cc1 = vec![(0.0, sec.cc1), (len, sec.cc1)];
    let mut cc11 = Vec::new();
    let mut t = 0.0;
    while t < len - 1e-9 {
        cc11.push((t, 0.8));
        cc11.push((t + 0.5, 1.0));
        cc11.push((t + 0.75, 0.84));
        t += 1.0;
    }
    let mut cc21 = vec![(0.0, 0.0)];
    for (i, n) in notes.iter().enumerate() {
        let rest_after = i + 1 == notes.len() || notes[i + 1].at - (n.at + n.len) > 0.3;
        if n.len >= 1.0 && rest_after {
            let on = n.at + beat(0.3);
            cc21.extend([
                (n.at, 0.0),
                (on, 0.0),
                (on + beat(0.25), 0.28),
                (n.at + n.len, 0.2),
            ]);
            cc21.push((n.at + n.len + 0.05, 0.0));
        }
    }
    cc21.sort_by(|a, b| a.0.total_cmp(&b.0));
    cc21.dedup_by(|a, b| (a.0 - b.0).abs() < 1.0 / 96.0);
    (notes, drone, [cc1, cc11, cc21])
}

pub fn exec(app: &mut CantusApp, what: &str, cmd: Command) {
    assert!(app.exec(cmd), "the app refused {what}");
}

/// A track from the picker with a factory preset, named, panned, at a fader, sent to `hall`.
#[allow(clippy::too_many_arguments)]
pub fn add_track(
    app: &mut CantusApp,
    kind: InstrumentKind,
    preset: Option<&str>,
    name: &str,
    color: Color,
    pan: f32,
    gain_db: f32,
    send_db: f32,
    hall: StripId,
) -> TrackId {
    let purpose = PickPurpose::AddInstrument { index: None };
    app.open_picker(purpose.clone());
    app.apply_pick(purpose, PickChoice::Instrument(kind));
    app.picker = None;
    let t = app.project().tracks.last().expect("the new track");
    let (track, strip) = (t.id, t.strip.expect("the track's strip"));
    if let Some(p) = preset {
        app.open_preset_browser(Target::Track(track));
        assert!(app.apply_preset_id(p), "the preset {p}");
        app.close_preset_browser();
    }
    exec(
        app,
        "a name",
        Command::SetTrackName {
            id: track,
            name: name.to_string(),
        },
    );
    exec(
        app,
        "a strip name",
        Command::SetStripName {
            id: strip,
            name: name.to_string(),
        },
    );
    exec(app, "a colour", Command::SetTrackColor { id: track, color });
    exec(app, "a pan", Command::SetStripPan { id: strip, pan });
    exec(app, "a fader", Command::SetStripGain { id: strip, gain_db });
    let mut s = Send::new(hall);
    s.level = cantus_core::db_to_linear(send_db);
    exec(
        app,
        "a send",
        Command::InsertStripSend {
            id: strip,
            index: 0,
            send: s,
        },
    );
    track
}

/// A clip of `notes` at `bar` for `bars`, with their articulations and the lanes.
#[allow(clippy::too_many_arguments)]
pub fn clip(
    app: &mut CantusApp,
    track: TrackId,
    name: &str,
    bar: u32,
    bars: u32,
    notes: &[FNote],
    lanes: Vec<(LaneKind, Vec<(f64, f32)>)>,
    bpm: f64,
) {
    let mut scratch = app.project().clone();
    let len = f64::from(bars) * BAR;
    let mut c = scratch.make_clip(name, bar_tick(bar), tick(len));
    let mut arts = Vec::new();
    for n in notes {
        let at = n.at.clamp(0.0, len - 0.05);
        let l = n.len.min(len - at);
        let mut note = scratch.make_note(tick(at), tick(l), n.key, n.vel);
        if !n.bend.is_empty() {
            let ppq = cantus_core::PPQ as f64;
            note.expr = NoteExpr {
                bend: n
                    .bend
                    .iter()
                    .map(|(s, st)| {
                        ExprPoint(
                            (s * bpm / 60.0 * ppq).round() as i64,
                            ExprKind::Bend.store(*st),
                        )
                    })
                    .collect(),
                ..NoteExpr::default()
            }
            .normalised()
            .boxed();
        }
        if n.art.is_some() {
            arts.push((note.id, n.art));
        }
        c.notes.push(note);
    }
    app.bus.reserve_ids(scratch.next_id);
    let id = c.id;
    exec(
        app,
        "a clip",
        Command::InsertClip {
            track,
            clip: Box::new(c),
        },
    );
    if !arts.is_empty() {
        exec(
            app,
            "the articulations",
            Command::SetNoteArt {
                track,
                clip: id,
                arts,
            },
        );
    }
    if !lanes.is_empty() {
        let lanes = lanes
            .into_iter()
            .map(|(k, pts)| {
                ClipLane::new(
                    k,
                    pts.iter()
                        .filter(|(b, _)| *b >= 0.0 && *b <= len)
                        .map(|(b, v)| AutoPoint::new(tick(*b), *v))
                        .collect(),
                )
            })
            .collect();
        exec(
            app,
            "the lanes",
            Command::SetClipLanes {
                track,
                clip: id,
                lanes,
            },
        );
    }
}

/// The hall: a free return with Aula's smaller room on it.
pub fn hall(app: &mut CantusApp) -> StripId {
    let hall = app
        .project()
        .strips
        .iter()
        .find(|s| s.dock == Dock::Right && s.name.is_empty())
        .map(|s| s.id)
        .expect("a free return");
    exec(
        app,
        "the room's name",
        Command::SetStripName {
            id: hall,
            name: "Sala".to_string(),
        },
    );
    let purpose = PickPurpose::AddEffect { strip: hall };
    app.open_picker(purpose.clone());
    app.apply_pick(purpose, PickChoice::Effect(EffectKind::Aula));
    app.picker = None;
    let verb = app
        .project()
        .strip(hall)
        .and_then(|s| s.effects.last())
        .map(|e| e.id)
        .expect("Aula");
    for (key, value) in [
        ("mix", 1.0),
        ("predelay", 10.0),
        ("low", 160.0),
        ("high", 8_000.0),
    ] {
        exec(
            app,
            "a room setting",
            Command::SetEffectParam {
                strip: hall,
                effect: verb,
                key: key.to_string(),
                value,
            },
        );
    }
    hall
}

/// The chord of a bar: its piano voicing and its bass's root and fifth.
fn chord(name: &str) -> ([u8; 3], [u8; 2]) {
    match name {
        "G" => ([62, 67, 71], [31, 38]),
        "A" => ([61, 64, 69], [33, 40]),
        _ => ([62, 66, 69], [38, 33]),
    }
}

/// The hoedown, built: the fiddle, its double stops, the piano, the bass, the kit.
fn build_tune(app: &mut CantusApp) -> Vec<TrackId> {
    exec(
        app,
        "the name",
        Command::SetProjectName {
            name: "Hoedown en re".to_string(),
        },
    );
    exec(app, "the tempo", Command::SetTempo { bpm: BPM });
    let room = hall(app);
    let fid = add_track(
        app,
        InstrumentKind::BowedString,
        Some("arcus.preset.violin_fiddle"),
        "Violín country",
        Color::rgb(0xD9, 0xA4, 0x41),
        -0.15,
        0.0,
        -16.0,
        room,
    );
    let dbl = add_track(
        app,
        InstrumentKind::BowedString,
        Some("arcus.preset.violin_fiddle"),
        "Violín (dobles cuerdas)",
        Color::rgb(0xC9, 0x94, 0x31),
        -0.15,
        -7.0,
        -16.0,
        room,
    );
    let piano = add_track(
        app,
        InstrumentKind::Chorda,
        Some("chorda.preset.honky"),
        "Piano",
        Color::rgb(0x5A, 0x7A, 0x9C),
        0.35,
        -9.0,
        -18.0,
        room,
    );
    let bass = add_track(
        app,
        InstrumentKind::BowedString,
        Some("arcus.preset.bass_pizz"),
        "Contrabajo",
        Color::rgb(0x6E, 0x45, 0x2C),
        0.1,
        -2.0,
        -18.0,
        room,
    );
    let kit = add_track(
        app,
        InstrumentKind::AcousticDrums,
        None,
        "Batería",
        Color::rgb(0x7A, 0x7A, 0x7A),
        0.0,
        -12.0,
        -20.0,
        room,
    );
    for (k, sec) in FORM.iter().enumerate() {
        let (notes, drone, [cc1, cc11, cc21]) = fiddle(sec, 0xF1DD_1E00 + k as u64);
        let lanes = vec![
            (LaneKind::Cc(1), cc1.clone()),
            (LaneKind::Cc(11), cc11.clone()),
            (LaneKind::Cc(21), cc21.clone()),
        ];
        clip(app, fid, "Violín", sec.bar, sec.bars, &notes, lanes, BPM);
        if !drone.is_empty() {
            let lanes = vec![
                (LaneKind::Cc(1), cc1),
                (LaneKind::Cc(11), cc11),
                (LaneKind::Cc(21), vec![(0.0, 0.0)]),
            ];
            clip(
                app,
                dbl,
                "Dobles cuerdas",
                sec.bar,
                sec.bars,
                &drone,
                lanes,
                BPM,
            );
        }
    }
    // The band: the piano on 2 and 4, the bass root and fifth on 1 and 3, the kit light.
    let chords: Vec<&str> = CHORDS.split_whitespace().collect();
    let (mut p, mut b, mut d) = (Vec::new(), Vec::new(), Vec::new());
    for (bar, c) in chords.iter().enumerate() {
        let at = bar as f64 * BAR;
        let (tri, bs) = chord(c);
        for beat in [1.0, 3.0] {
            for (j, key) in tri.iter().enumerate() {
                p.push(FNote {
                    at: at + beat + 0.004 * j as f64,
                    len: 0.4,
                    key: *key,
                    vel: 72,
                    art: None,
                    bend: Vec::new(),
                });
            }
        }
        for (j, beat) in [0.0, 2.0].iter().enumerate() {
            b.push(FNote {
                at: at + beat,
                len: 0.9,
                key: bs[j],
                vel: 96,
                art: None,
                bend: Vec::new(),
            });
        }
        if bar > 0 {
            for e in 0..8 {
                let t = at + 0.5 * e as f64 + if e % 2 == 1 { 0.03 } else { 0.0 };
                d.push(FNote {
                    at: t,
                    len: 0.2,
                    key: 42,
                    vel: if e % 2 == 1 { 46 } else { 34 },
                    art: None,
                    bend: Vec::new(),
                });
            }
            for beat in [0.0, 2.0] {
                d.push(FNote {
                    at: at + beat,
                    len: 0.3,
                    key: 36,
                    vel: 58,
                    art: None,
                    bend: Vec::new(),
                });
            }
            for beat in [1.0, 3.0] {
                d.push(FNote {
                    at: at + beat,
                    len: 0.3,
                    key: 37,
                    vel: 62,
                    art: None,
                    bend: Vec::new(),
                });
            }
        }
    }
    let bars = chords.len() as u32;
    clip(app, piano, "Piano", 1, bars, &p, vec![], BPM);
    clip(app, bass, "Contrabajo", 1, bars, &b, vec![], BPM);
    clip(app, kit, "Batería", 1, bars, &d, vec![], BPM);
    vec![fid, dbl]
}

/// One passage (the B part's first four bars) on the violin alone, classical or fiddle.
fn build_passage(app: &mut CantusApp, country: bool) -> TrackId {
    exec(
        app,
        "the name",
        Command::SetProjectName {
            name: if country {
                "Pasaje, violín country"
            } else {
                "Pasaje, violín clásico"
            }
            .to_string(),
        },
    );
    exec(app, "the tempo", Command::SetTempo { bpm: BPM });
    let room = hall(app);
    let preset = if country {
        "arcus.preset.violin_fiddle"
    } else {
        "arcus.preset.violin_cantabile"
    };
    let v = add_track(
        app,
        InstrumentKind::BowedString,
        Some(preset),
        "Violín",
        Color::rgb(0xD9, 0xA4, 0x41),
        0.0,
        0.0,
        -14.0,
        room,
    );
    let four: String = B_PART.split('|').take(4).collect::<Vec<_>>().join("|");
    let four: &'static str = Box::leak(four.into_boxed_str());
    let sec = Section {
        bar: 1,
        bars: 4,
        notes: four,
        pattern: Pattern::Georgia,
        drone: country,
        cc1: 0.66,
        bpm: BPM,
    };
    if country {
        let (notes, drone, [cc1, cc11, cc21]) = fiddle(&sec, 0xF1DD_1E77);
        let lanes = vec![
            (LaneKind::Cc(1), cc1.clone()),
            (LaneKind::Cc(11), cc11.clone()),
            (LaneKind::Cc(21), cc21),
        ];
        clip(app, v, "Violín", 1, 4, &notes, lanes, BPM);
        let d = add_track(
            app,
            InstrumentKind::BowedString,
            Some(preset),
            "Violín (dobles cuerdas)",
            Color::rgb(0xC9, 0x94, 0x31),
            0.0,
            -7.0,
            -14.0,
            room,
        );
        let lanes = vec![(LaneKind::Cc(1), cc1), (LaneKind::Cc(11), cc11)];
        clip(app, d, "Dobles cuerdas", 1, 4, &drone, lanes, BPM);
    } else {
        // Classical: the same notes slurred by the bar's halves, legato, the vibrato on, a
        // phrase's swell (the quartet's player, `perform.rs`, as the melody).
        let part = Part {
            who: Who::Violin,
            section: 0,
            vel: 70,
            notes: four,
            cc1: &[(0.0, 0.5), (6.0, 0.64), (12.0, 0.58), (16.0, 0.42)],
        };
        let (mut w, beats) = parse_t(&part, 0);
        let mut slur = 0;
        for (i, x) in w.iter_mut().enumerate() {
            if x.at.rem_euclid(2.0).abs() < 1e-9 || i == 0 {
                slur += 1;
            }
            x.slur = slur;
        }
        let ctx = super::perform::Ctx {
            bpm: BPM,
            climax: false,
            intimate: false,
            seed: 0xC1A5,
        };
        let perf = super::perform::perform(&part, &w, beats, super::perform::Role::Melody, &ctx);
        let notes: Vec<FNote> = perf
            .notes
            .iter()
            .map(|n| FNote {
                at: n.at,
                len: n.len,
                key: n.key,
                vel: n.vel,
                art: n.art,
                bend: if n.bend != 0.0 {
                    vec![(0.0, n.bend)]
                } else {
                    Vec::new()
                },
            })
            .collect();
        let lanes = vec![
            (LaneKind::Cc(1), perf.cc1),
            (LaneKind::Cc(11), perf.cc11),
            (LaneKind::Cc(21), perf.cc21),
        ];
        clip(app, v, "Violín", 1, 4, &notes, lanes, BPM);
    }
    v
}

/// Render the tune and the two passages into `out`; checks each (a second export the same
/// bytes, under −1 dBTP with no full-scale sample, no click at the fiddle's boundaries).
pub fn render(out: &Path) -> bool {
    std::fs::create_dir_all(out).expect("the fiddle's folder");
    let mut ok = true;
    for (name, which) in [
        ("hoedown", 0u8),
        ("pasaje-clasico", 1),
        ("pasaje-country", 2),
        ("tren", 3),
    ] {
        let data = tempfile::tempdir().expect("a data folder");
        let (mut app, _proc) = super::app_on(data.path(), cantus_core::Project::new(name));
        let tracks = match which {
            0 => build_tune(&mut app),
            1 => vec![build_passage(&mut app, false)],
            3 => super::fiddle2::build(&mut app),
            _ => vec![build_passage(&mut app, true)],
        };
        app.save_to_path(&out.join(format!("{name}.cantus")));
        let wav = out.join(format!("{name}.wav"));
        let said = super::export(&mut app, &wav, false);
        let again = out.join(format!(".{name}-again.wav"));
        super::export(&mut app, &again, false);
        let same = measure::same_bytes(&wav, &again);
        let _ = std::fs::remove_file(&again);
        let a = measure::read_wav(&wav);
        let lv = measure::level(&a);
        // The boundaries are heard in the fiddle's own stems (in the band's mix the
        // cross-stick on 2 and 4 lands on the fiddle's starts).
        let stems = out.join(format!("{name}-stems"));
        super::export(&mut app, &stems, true);
        let fiddle_mix = std::fs::read_dir(&stems)
            .expect("the stems")
            .flatten()
            .map(|e| e.path())
            .filter(|p| {
                p.file_name()
                    .and_then(|n| n.to_str())
                    .is_some_and(|n| n.contains("Violín"))
            })
            .map(|p| measure::mono(&measure::read_wav(&p)))
            .reduce(|a, b| a.iter().zip(&b).map(|(x, y)| x + y).collect())
            .expect("a violin stem");
        let x = fiddle_mix;
        let p = app.project();
        let tm = p.tempo_map();
        let mut at: Vec<(f64, &'static str)> = Vec::new();
        for t in &tracks {
            for c in &p.track(*t).expect("the track").clips {
                for n in &c.notes {
                    let s: Ticks = c.start + n.start;
                    at.push((tm.seconds_at(s.0 as f64), "start"));
                    at.push((tm.seconds_at((s + n.length).0 as f64), "end"));
                }
            }
        }
        let edges = measure::edges(&x, a.rate, &at);
        let worst = edges
            .iter()
            .map(|e| e.over_db)
            .fold(f64::NEG_INFINITY, f64::max);
        let pass = same && lv.dbtp <= -0.9 && lv.full_scale == 0 && worst < 15.0;
        ok &= pass;
        println!(
            "{name}: {said}; {:.1} LUFS, {:.2} dBTP, the same bytes again {same}, the worst \
             boundary {worst:.1} dB over its music ({} measured) — {}",
            lv.lufs.unwrap_or(-200.0),
            lv.dbtp,
            edges.len(),
            if pass { "pass" } else { "FAIL" }
        );
    }
    ok
}
