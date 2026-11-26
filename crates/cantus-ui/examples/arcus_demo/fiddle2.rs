//! r42 arcus-expr — the owner after the hoedown: "could you make something much livelier, more
//! cowboy, with more energy?" *Tren de la medianoche*, an original bluegrass / Western-swing
//! breakdown in A at 160 BPM (44 bars, about 66 s), on the same fiddle character and fiddler's
//! rules as `fiddle.rs`:
//!
//! - **form**: a kick-off (chops on 2 and 4, then a lick), the A part twice (the Nashville
//!   shuffle, a flatted seventh, a cross-string arpeggio up to the E string's top), the B part
//!   twice (the Georgia shuffle, slides into the accented notes; the second time **twin
//!   fiddles**, a second Arcus violin a diatonic third under), a quiet **breakdown** (the train
//!   whistle — a double stop sliding up from a minor third under, twice — and chops over a light
//!   band), a **double-time push** of saw-stroke sixteenths, and a shout **tag**;
//! - **the band, driving**: the bass on root and fifth hard on 1 and 3 (Arcus, pizzicato), the
//!   honky-tonk piano stabbing 2 and 4 (Chorda), and a train beat (Tympanum: the snare's eighths
//!   with the accents on 2 and 4, the kick on 1 and 3, a crash at the sections' heads).

use super::fiddle::{self, FNote, Pattern, Section};
use cantus_core::clip_lanes::LaneKind;
use cantus_core::{Color, Command, InstrumentKind, TrackId};
use cantus_ui::CantusApp;

pub const BPM: f64 = 160.0;
const BAR: f64 = 4.0;

const INTRO: &str = "r:1 A4:0.25c r:0.75 r:1 A4:0.25c r:0.75 | \
     E5:0.5 E5:0.5 A5:0.5w E5:0.5w C#5:0.5w E5:0.5w A4:0.5w B4:0.5w";
const A_PART: &str = "C#5:0.5/ A4:0.5 C#5:0.5 E5:0.5 A5:0.5 E5:0.5 C#5:0.5 E5:0.5 | \
     F#5:0.5 E5:0.5 C#5:0.5 A4:0.5 B4:0.5 C#5:0.5 B4:0.5 A4:0.5 | \
     C#5:0.5 A4:0.5 C#5:0.5 E5:0.5 A5:0.5 B5:0.5 C#6:0.5 A5:0.5 | \
     B5:0.5 A5:0.5 F#5:0.5 E5:0.5 F#5:0.5 E5:0.5 C#5:0.5 B4:0.5 | \
     A4:0.5 C#5:0.5 E5:0.5 G5:0.5b F#5:0.5 E5:0.5 D5:0.5 C#5:0.5 | \
     D5:0.5 F#5:0.5 A5:0.5 F#5:0.5 D5:0.5 B4:0.5 A4:0.5 F#4:0.5 | \
     E4:0.5w G#4:0.5w B4:0.5w E5:0.5w G#5:0.5w B5:0.5w E6:0.5w B5:0.5w | \
     A5:1> E5:0.5 C#5:0.5 A4:1 r:1";
const B_PART: &str = "E6:0.5/ C#6:0.5 A5:0.5 C#6:0.5 E6:1/> C#6:1 | \
     D6:0.5 B5:0.5 A5:0.5 F#5:0.5 D6:1/> B5:1 | \
     C#6:0.5 A5:0.5 E5:0.5 A5:0.5 C#6:0.5 B5:0.5 A5:0.5 G#5:0.5 | \
     F#5:0.5 E5:0.5 D5:0.5 B4:0.5 G#4:0.5 B4:0.5 E5:0.5 D5:0.5 | \
     E6:0.5/ C#6:0.5 A5:0.5 C#6:0.5 E6:1/> C#6:1 | \
     D6:0.5 B5:0.5 A5:0.5 F#5:0.5 D6:1/> B5:1 | \
     C#6:0.5 A5:0.5 E5:0.5 C#5:0.5 B4:0.5 E5:0.5 G#5:0.5 B5:0.5 | \
     A5:1> E5:0.5 C#5:0.5 A4:1 r:1";
const BREAKDOWN: &str = "E5:1x% E5:0.5 E5:0.5 E5:1x% r:1 | E5:1x% E5:0.5 E5:0.5 E5:1x% r:1 | \
     r:1 A4:0.25c r:0.75 r:1 A4:0.25c r:0.75 | r:1 A4:0.25c r:0.75 E5:0.5 E5:0.5 E5:0.5 F#5:0.5";
const PUSH: &str = "A5:0.25w G#5:0.25w F#5:0.25w E5:0.25w C#5:0.25w E5:0.25w A5:0.25w E5:0.25w \
     C#5:0.25w A4:0.25w C#5:0.25w E5:0.25w A5:0.5> E5:0.5 | \
     B5:0.25w A5:0.25w F#5:0.25w D5:0.25w B4:0.25w D5:0.25w F#5:0.25w A5:0.25w B5:0.5> F#5:0.5 \
     D5:0.5 B4:0.5 | \
     C#6:0.25w B5:0.25w A5:0.25w E5:0.25w C#5:0.25w E5:0.25w A5:0.25w C#6:0.25w E6:0.5/> \
     C#6:0.5 A5:0.5 E5:0.5 | \
     B5:0.25w G#5:0.25w E5:0.25w B4:0.25w G#4:0.25w B4:0.25w E5:0.25w G#5:0.25w B5:1> r:1";
const TAG: &str = "A5:0.5> r:0.5 E5:0.25 E5:0.25 r:0.5 F#5:0.5 r:0.5 E5:0.5> r:0.5 | \
     r:1 G#5:0.5> r:0.5 A5:1.5> r:0.5";

/// The chords bar by bar (A, D, E).
const CHORDS: &str = "A A  A A A E A D E A  A A A E A D E A  A D A E A D E A  A D A E A D E A  \
     A A A E  A D A E  A A";

pub const FORM: [Section; 8] = [
    Section {
        bar: 1,
        bars: 2,
        notes: INTRO,
        pattern: Pattern::Nashville,
        drone: true,
        cc1: 0.74,
        bpm: BPM,
    },
    Section {
        bar: 3,
        bars: 8,
        notes: A_PART,
        pattern: Pattern::Nashville,
        drone: false,
        cc1: 0.7,
        bpm: BPM,
    },
    Section {
        bar: 11,
        bars: 8,
        notes: A_PART,
        pattern: Pattern::Nashville,
        drone: true,
        cc1: 0.74,
        bpm: BPM,
    },
    Section {
        bar: 19,
        bars: 8,
        notes: B_PART,
        pattern: Pattern::Georgia,
        drone: true,
        cc1: 0.78,
        bpm: BPM,
    },
    Section {
        bar: 27,
        bars: 8,
        notes: B_PART,
        pattern: Pattern::Georgia,
        drone: false,
        cc1: 0.8,
        bpm: BPM,
    },
    Section {
        bar: 35,
        bars: 4,
        notes: BREAKDOWN,
        pattern: Pattern::Nashville,
        drone: true,
        cc1: 0.6,
        bpm: BPM,
    },
    Section {
        bar: 39,
        bars: 4,
        notes: PUSH,
        pattern: Pattern::Nashville,
        drone: false,
        cc1: 0.84,
        bpm: BPM,
    },
    Section {
        bar: 43,
        bars: 2,
        notes: TAG,
        pattern: Pattern::Nashville,
        drone: true,
        cc1: 0.86,
        bpm: BPM,
    },
];

/// A diatonic third under `key` in A major (a note outside the scale: a minor third).
fn third_under(key: u8) -> u8 {
    const SCALE: [u8; 7] = [9, 11, 1, 2, 4, 6, 8];
    let pc = key % 12;
    match SCALE.iter().position(|p| *p == pc) {
        Some(i) => {
            let to = SCALE[(i + 5) % 7];
            let down = (pc + 12 - to) % 12;
            key - if down == 0 { 12 } else { down }
        }
        None => key - 3,
    }
}

/// The chord of a bar: the piano's voicing and the bass's root and fifth.
fn chord(name: &str) -> ([u8; 3], [u8; 2]) {
    match name {
        "D" => ([62, 66, 69], [38, 33]),
        "E" => ([59, 64, 68], [28, 35]),
        _ => ([61, 64, 69], [33, 40]),
    }
}

fn note(at: f64, len: f64, key: u8, vel: u8) -> FNote {
    FNote {
        at,
        len,
        key,
        vel,
        art: None,
        bend: Vec::new(),
    }
}

/// The breakdown, built: the fiddle, its double stops, the twin fiddle, the piano, the bass,
/// the kit — the fiddle forward, the band tight.
pub fn build(app: &mut CantusApp) -> Vec<TrackId> {
    super::fiddle::exec(
        app,
        "the name",
        Command::SetProjectName {
            name: "Tren de la medianoche".to_string(),
        },
    );
    super::fiddle::exec(app, "the tempo", Command::SetTempo { bpm: BPM });
    let room = fiddle::hall(app);
    let add = |app: &mut CantusApp,
               kind: InstrumentKind,
               preset: Option<&str>,
               name: &str,
               color: Color,
               pan: f32,
               gain: f32,
               send: f32| {
        fiddle::add_track(app, kind, preset, name, color, pan, gain, send, room)
    };
    let fid = add(
        app,
        InstrumentKind::BowedString,
        Some("arcus.preset.violin_fiddle"),
        "Violín country",
        Color::rgb(0xD9, 0xA4, 0x41),
        -0.12,
        1.0,
        -18.0,
    );
    let dbl = add(
        app,
        InstrumentKind::BowedString,
        Some("arcus.preset.violin_fiddle"),
        "Violín (dobles cuerdas)",
        Color::rgb(0xC9, 0x94, 0x31),
        -0.12,
        -6.0,
        -18.0,
    );
    let twin = add(
        app,
        InstrumentKind::BowedString,
        Some("arcus.preset.violin_fiddle"),
        "Violín 2 (armonía)",
        Color::rgb(0xB9, 0x84, 0x21),
        0.22,
        -3.0,
        -18.0,
    );
    let piano = add(
        app,
        InstrumentKind::Chorda,
        Some("chorda.preset.honky"),
        "Piano",
        Color::rgb(0x5A, 0x7A, 0x9C),
        0.35,
        -11.0,
        -20.0,
    );
    let bass = add(
        app,
        InstrumentKind::BowedString,
        Some("arcus.preset.bass_pizz"),
        "Contrabajo",
        Color::rgb(0x6E, 0x45, 0x2C),
        0.08,
        -1.0,
        -22.0,
    );
    let kit = add(
        app,
        InstrumentKind::AcousticDrums,
        None,
        "Batería",
        Color::rgb(0x7A, 0x7A, 0x7A),
        0.0,
        -9.0,
        -22.0,
    );
    for (k, sec) in FORM.iter().enumerate() {
        let (notes, drone, [cc1, cc11, cc21]) = fiddle::fiddle(sec, 0x7EE1_0000 + k as u64);
        let lanes = vec![
            (LaneKind::Cc(1), cc1.clone()),
            (LaneKind::Cc(11), cc11.clone()),
            (LaneKind::Cc(21), cc21),
        ];
        fiddle::clip(app, fid, "Violín", sec.bar, sec.bars, &notes, lanes, BPM);
        if !drone.is_empty() {
            let lanes = vec![
                (LaneKind::Cc(1), cc1.clone()),
                (LaneKind::Cc(11), cc11.clone()),
                (LaneKind::Cc(21), vec![(0.0, 0.0)]),
            ];
            fiddle::clip(
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
        // The twin fiddle: the B part's second time, a diatonic third under, a hair behind.
        if k == 4 {
            let harm: Vec<FNote> = notes
                .iter()
                .map(|n| FNote {
                    at: n.at + 0.012,
                    key: third_under(n.key),
                    vel: n.vel.saturating_sub(6),
                    ..n.clone()
                })
                .collect();
            let lanes = vec![(LaneKind::Cc(1), cc1), (LaneKind::Cc(11), cc11)];
            fiddle::clip(app, twin, "Armonía", sec.bar, sec.bars, &harm, lanes, BPM);
        }
    }
    // The band.
    let chords: Vec<&str> = CHORDS.split_whitespace().collect();
    let (mut p, mut b, mut d) = (Vec::new(), Vec::new(), Vec::new());
    for (bar, c) in chords.iter().enumerate() {
        let bar1 = bar as u32 + 1;
        let at = bar as f64 * BAR;
        let (tri, bs) = chord(c);
        let quiet = (35..=38).contains(&bar1);
        let tag = bar1 >= 43;
        // The piano: stabs on 2 and 4 (a fill of eighths up the chord in each section's
        // last bar); out in the breakdown.
        if !quiet && !tag {
            let last_of_section = [10, 18, 26, 34, 42].contains(&bar1);
            for beat in [1.0, 3.0] {
                for (j, key) in tri.iter().enumerate() {
                    p.push(note(at + beat + 0.003 * j as f64, 0.22, *key, 86));
                }
            }
            if last_of_section {
                for (e, key) in [tri[0], tri[1], tri[2], tri[0] + 12].iter().enumerate() {
                    p.push(note(at + 2.0 + 0.5 * e as f64, 0.3, *key + 12, 78));
                }
            }
        }
        // The bass: root and fifth hard on 1 and 3 (in the breakdown the root alone).
        for (j, beat) in [0.0, 2.0].iter().enumerate() {
            if quiet && j == 1 {
                continue;
            }
            b.push(note(at + beat, 0.7, bs[j], if quiet { 84 } else { 112 }));
        }
        // The kit: the train beat.
        if bar1 >= 3 && !tag {
            for e in 0..8 {
                let on_backbeat = e == 2 || e == 6;
                let v = if quiet {
                    if on_backbeat {
                        52
                    } else {
                        26
                    }
                } else if on_backbeat {
                    104
                } else {
                    44 + 6 * (e % 2) as u8
                };
                d.push(note(at + 0.5 * e as f64, 0.2, 38, v));
            }
            if !quiet {
                for beat in [0.0, 2.0] {
                    d.push(note(at + beat, 0.3, 36, 96));
                }
            }
            if [3, 19, 39].contains(&bar1) {
                d.push(note(at, 1.0, 49, 96));
            }
        }
    }
    // The tag: everyone on the shout's hits (the fiddle's accented notes).
    let tag_at = 42.0 * BAR;
    for hit in [0.0, 2.0, 3.0, 5.0, 6.0] {
        let (tri, bs) = chord(if hit == 5.0 { "E" } else { "A" });
        for key in tri {
            p.push(note(tag_at + hit, 0.3, key, 100));
        }
        b.push(note(tag_at + hit, 0.4, bs[0], 118));
        d.push(note(tag_at + hit, 0.3, 38, 110));
        d.push(note(tag_at + hit, 0.3, 36, 104));
    }
    d.push(note(tag_at + 6.0, 1.5, 49, 110));
    let bars = chords.len() as u32;
    fiddle::clip(app, piano, "Piano", 1, bars, &p, vec![], BPM);
    fiddle::clip(app, bass, "Contrabajo", 1, bars, &b, vec![], BPM);
    fiddle::clip(app, kit, "Batería", 1, bars, &d, vec![], BPM);
    vec![fid, dbl, twin]
}
