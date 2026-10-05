//! **Sunday Strut** — funk and soul in E minor at 102 BPM.
//!
//! Organum's soul registration through its rotary speaker holds the chords; Lamina's
//! growling reed piano chops sixteenths against it; Gravis slaps the bass line; Modulus is
//! the horn section; Calor takes the solo; Pulsus II's studio kit plays a sixteenth-note
//! groove full of ghost notes, split over four tracks into a drum bus. Intro 4 · verse 8 ·
//! chorus 8 · solo 8 · chorus 8 · outro 4 = 40 bars.

use super::parts::{bars, bass, comp, drums, hold, lay, repeat, B};
use super::song::{self, colors, native, section, Feel, Song};
use super::theory::{melody, Ev};
use crate::i18n::t;
use cantus_core::{EffectKind as K, InstrumentKind as I, Project, TrackIcon};

const VAMP: &[(&str, f64)] = &[("Em9", 4.0), ("A13", 4.0)];
const CHORUS: &[(&str, f64)] = &[
    ("Cmaj7", 4.0),
    ("Bm7", 4.0),
    ("Am7", 4.0),
    ("D9", 4.0),
    ("Cmaj7", 4.0),
    ("Bm7", 4.0),
    ("Am7", 4.0),
    ("B7#9", 4.0),
];
const OUTRO: &[(&str, f64)] = &[("Em9", 4.0), ("A13", 4.0), ("Em9", 8.0)];

const FORM: [(&str, f64, f64); 6] = [
    ("intro", 0.0, 4.0),
    ("verse", 4.0, 8.0),
    ("chorus", 12.0, 8.0),
    ("solo", 20.0, 8.0),
    ("chorus", 28.0, 8.0),
    ("outro", 36.0, 4.0),
];

const KICK: u8 = 36;
const RIM: u8 = 37;
const SNARE: u8 = 38;
const HAT: u8 = 42;
const LOW_TOM: u8 = 45;
const OPEN: u8 = 46;
const MID_TOM: u8 = 47;
const CRASH: u8 = 49;
const HIGH_TOM: u8 = 50;

pub fn build() -> Project {
    let mut s = Song::new(&t("demos.song.funk"), 102.0, 0xF0_4C);
    let studio = song::kit("pulsus.kit.studio");
    let kick = s.part(
        &t("demos.track.kick"),
        song::pads(&studio, &[KICK]),
        colors::KICK,
        TrackIcon::Drums,
    );
    let snare = s.part(
        &t("demos.track.snare"),
        song::pads_with(&studio, &[RIM, SNARE], |p| p.level_db += 6.0),
        colors::SNARE,
        TrackIcon::Drums,
    );
    let hats = s.part(
        &t("demos.track.hats"),
        song::pads(&studio, &[HAT, OPEN, CRASH]),
        colors::HATS,
        TrackIcon::Drums,
    );
    let toms = s.part(
        &t("demos.track.toms"),
        song::pads(&studio, &[LOW_TOM, MID_TOM, HIGH_TOM]),
        colors::PERC,
        TrackIcon::Drums,
    );
    let bass_p = s.part(
        &t("demos.track.bass"),
        native(I::Bass, "bass.preset.slap", &[]),
        colors::BASS,
        TrackIcon::Strings,
    );
    let organ = s.part(
        &t("demos.track.organ"),
        native(I::Organ, "organ.preset.soul", &[]),
        colors::ORGAN,
        TrackIcon::Keys,
    );
    let keys = s.part(
        &t("demos.track.keys"),
        native(I::ElectricPiano, "epiano.preset.reed_growl", &[]),
        colors::KEYS,
        TrackIcon::Keys,
    );
    let brass = s.part(
        &t("demos.track.brass"),
        native(I::Fm, "fm.preset.brass", &[]),
        colors::BRASS,
        TrackIcon::Synth,
    );
    let lead = s.part(
        &t("demos.track.lead"),
        native(I::Analog, "calor.preset.sync_lead", &[]),
        colors::LEAD,
        TrackIcon::Synth,
    );
    // A drummer who sits a little behind the beat, and a sixteenth swing.
    let pocket = Feel {
        jitter: 0.008,
        vel: 7.0,
        ..Feel::HUMAN
    }
    .swung(0.12, 0.25);
    for p in [kick, snare, hats, toms] {
        s.feel(p, pocket);
    }
    s.feel(
        bass_p,
        Feel {
            jitter: 0.006,
            vel: 6.0,
            ..Feel::HUMAN
        }
        .swung(0.12, 0.25),
    );
    s.feel(keys, Feel::HUMAN.swung(0.12, 0.25));

    let bus = s.bus(
        &t("demos.strip.drum_bus"),
        colors::BUS,
        &[kick, snare, hats, toms],
    );
    let verb = s.ret(&t("demos.strip.reverb"));
    let delay = s.ret(&t("demos.strip.delay"));
    s.insert(kick.strip, K::Eq, Some("eq.preset.kick"), &[]);
    s.insert(
        snare.strip,
        K::Gate,
        Some("preset.gate.snare"),
        &[("thr", -36.0), ("range", 18.0)],
    );
    s.insert(
        toms.strip,
        K::Gate,
        Some("preset.gate.toms"),
        &[("thr", -40.0)],
    );
    s.insert(
        bus,
        K::Compressor,
        Some("comp.preset.parallel"),
        &[("dry", -3.0), ("makeup", 6.0)],
    );
    // Tape on the drum bus (Fervor): the squashed kit rounded off, level-matched.
    s.insert(
        bus,
        K::Saturator,
        Some("preset.sat.tape_bus"),
        &[("drive", 6.0)],
    );
    s.insert(
        bus,
        K::Limiter,
        Some("lim.preset.drums"),
        &[("gain", 0.0), ("ceiling", -3.0)],
    );
    s.insert(bass_p.strip, K::Compressor, Some("comp.preset.bass"), &[]);
    s.insert(
        bass_p.strip,
        K::Eq,
        None,
        &[
            ("b0.used", 1.0),
            ("b0.shape", 3.0),
            ("b0.freq", 32.0),
            ("b0.slope", 24.0),
            ("b1.used", 1.0),
            ("b1.freq", 2_200.0),
            ("b1.gain", 2.0),
            ("b1.q", 1.0),
        ],
    );
    s.insert(
        organ.strip,
        K::Eq,
        None,
        &[
            ("b0.used", 1.0),
            ("b0.shape", 3.0),
            ("b0.freq", 120.0),
            ("b0.slope", 12.0),
            ("b1.used", 1.0),
            ("b1.freq", 400.0),
            ("b1.gain", -2.5),
            ("b1.q", 0.9),
        ],
    );
    s.insert(keys.strip, K::Multiband, Some("mb.preset.deharsh"), &[]);
    s.insert(
        brass.strip,
        K::Compressor,
        None,
        &[
            ("thr", -20.0),
            ("ratio", 3.0),
            ("att", 8.0),
            ("auto_gain", 1.0),
        ],
    );
    s.insert(
        lead.strip,
        K::Eq,
        None,
        &[
            ("b0.used", 1.0),
            ("b0.shape", 3.0),
            ("b0.freq", 250.0),
            ("b0.slope", 18.0),
        ],
    );
    s.insert(
        verb,
        K::Space,
        Some("space.preset.drum_room"),
        &[("mix", 1.0), ("decay", 1.1)],
    );
    s.insert(
        delay,
        K::Delay,
        None,
        &[
            ("time_ms", 441.0),
            ("feedback", 0.3),
            ("tone_hz", 3_000.0),
            ("mix", 1.0),
        ],
    );
    for (p, level) in [
        (snare, -12.0),
        (toms, -10.0),
        (organ, -14.0),
        (keys, -14.0),
        (brass, -10.0),
        (lead, -12.0),
    ] {
        s.send(p.strip, verb, level);
    }
    s.send(lead.strip, delay, -12.0);
    s.send(brass.strip, delay, -20.0);
    s.master(
        K::Eq,
        None,
        &[
            ("b0.used", 1.0),
            ("b0.shape", 3.0),
            ("b0.freq", 25.0),
            ("b0.slope", 18.0),
            ("b1.used", 1.0),
            ("b1.shape", 1.0),
            ("b1.freq", 90.0),
            ("b1.gain", 1.0),
        ],
    );
    s.master(K::Compressor, Some("comp.preset.master"), &[]);
    // Mastering (ADR 0026): Latitudo puts the low end in mono, then Culmen's streaming
    // master — three bands, the low one held, a gentle full-band squeeze, into a −1 dBTP
    // ceiling — driven so the song lands near −14 LUFS.
    s.master(K::Stereo, Some("preset.st.master_low"), &[]);
    s.master(K::Maximizer, Some("preset.max.streaming"), &[("gain", 4.5)]);

    s.trim(kick, 6.0);
    s.mix(kick.strip, 1.0, 0.0);
    s.trim(snare, 12.0);
    s.mix(snare.strip, -2.5, 0.0);
    s.trim(hats, 12.0);
    s.mix(hats.strip, 0.0, 0.22);
    s.trim(toms, 6.0);
    s.mix(toms.strip, -1.0, -0.15);
    s.mix(bus, -2.0, 0.0);
    s.trim(bass_p, -1.0);
    s.mix(bass_p.strip, -1.5, 0.0);
    s.trim(organ, -3.0);
    s.mix(organ.strip, -3.5, -0.25);
    s.trim(keys, 8.0);
    s.mix(keys.strip, -2.0, 0.3);
    s.trim(brass, 0.0);
    s.mix(brass.strip, -5.5, 0.1);
    s.trim(lead, 2.0);
    s.mix(lead.strip, -2.0, 0.0);
    s.mix(verb, -6.0, 0.0);
    s.mix(delay, -10.0, 0.0);

    for (key, at, len) in FORM {
        let name = section(key);
        let n = len as usize;
        let chart_src = match key {
            "chorus" => CHORUS,
            "outro" => OUTRO,
            _ => VAMP,
        };
        let organ_l = lay(chart_src, 4, 52, 76, 64);
        let keys_l = lay(chart_src, 3, 60, 79, 69);
        let times = (len * 4.0 / organ_l.beats).max(1.0) as usize;

        // The bass: a written riff in the vamp, a groove that follows the chords in the
        // chorus.
        let b: Vec<Ev> = match key {
            "chorus" => bass(
                &organ_l,
                28,
                4.0,
                &[
                    (0.0, 0.4, 108, B::R),
                    (0.75, 0.2, 70, B::O),
                    (1.0, 0.2, 86, B::R),
                    (1.5, 0.4, 94, B::F),
                    (2.5, 0.2, 82, B::R),
                    (2.75, 0.2, 74, B::O),
                    (3.0, 0.4, 98, B::R),
                    (3.5, 0.45, 90, B::A),
                ],
            ),
            "outro" => {
                let mut v = repeat(&riff(), 8.0, 1);
                v.push(Ev::new(8.0, 3.5, 28, 112));
                v
            }
            _ => repeat(&riff(), 8.0, n / 2),
        };
        s.clip(bass_p, &name, at, len, &b);

        // The organ: held in the chorus and solo, stabs on the off-beats in the verse.
        let o: Vec<Ev> = match key {
            "intro" => vec![],
            "verse" => comp(
                &organ_l,
                4.0,
                &[
                    (0.0, 0.9, 84),
                    (1.5, 0.35, 70),
                    (2.5, 1.3, 78),
                    (3.75, 0.2, 64),
                ],
            ),
            "outro" => hold(&organ_l, 84, 0.98),
            _ => repeat(&hold(&organ_l, 80, 0.97), organ_l.beats, times),
        };
        if !o.is_empty() {
            let o = if key == "verse" {
                repeat(&o, organ_l.beats, times)
            } else {
                o
            };
            s.clip(organ, &name, at, len, &o);
        }

        // The reed piano: sixteenth chops, answering the organ.
        if key != "intro" {
            let rhythm: &[(f64, f64, u8)] = &[
                (0.5, 0.15, 86),
                (0.75, 0.12, 58),
                (1.25, 0.15, 80),
                (2.0, 0.15, 64),
                (2.5, 0.15, 88),
                (3.0, 0.12, 56),
                (3.25, 0.15, 82),
            ];
            let v = comp(&keys_l, 4.0, rhythm);
            let v = if key == "outro" {
                v.into_iter().filter(|e| e.at < 8.0).collect()
            } else {
                repeat(&v, keys_l.beats, times)
            };
            s.clip(keys, &name, at, len, &v);
        }

        // The horns: stabs in the chorus, a fall into the last chord.
        if key == "chorus" || key == "outro" {
            let h = lay(chart_src, 3, 64, 84, 74);
            let v = if key == "chorus" {
                comp(
                    &h,
                    8.0,
                    &[
                        (0.5, 0.2, 104),
                        (0.75, 0.35, 96),
                        (2.5, 0.25, 92),
                        (4.75, 0.2, 98),
                        (5.0, 0.6, 106),
                        (7.5, 0.3, 90),
                    ],
                )
            } else {
                comp(&h, 16.0, &[(8.0, 3.0, 110)])
            };
            s.clip(brass, &name, at, len, &v);
        }

        let d = groove(key, n);
        s.clip(kick, &name, at, len, &d.0);
        s.clip(snare, &name, at, len, &d.1);
        if !d.2.is_empty() {
            s.clip(hats, &name, at, len, &d.2);
        }
        if !d.3.is_empty() {
            s.clip(toms, &name, at, len, &d.3);
        }
    }

    let solo = melody(
        "r:.5 B4:.25 D5:.25 >E5:1 D5:.5 B4:.5 A4:.5 G4:.5 | A4:.75 B4:.25 C#5:.5 E5:.5 >F#5:1 E5:1 \
         | G5:.5 E5:.25 D5:.25 E5:.5 G5:.5 >A5:1 G5:.5 E5:.5 | F#5:1.5 E5:.5 C#5:.5 B4:.5 A4:1 \
         | E5:.25 G5:.25 A5:.25 B5:.25 >D6:.5 B5:.5 A5:.5 G5:.5 E5:1 | F#5:.5 A5:.5 >C#6:1 B5:.5 A5:.5 F#5:1 \
         | G5:.25 F#5:.25 E5:.25 D5:.25 E5:.5 D5:.5 B4:.5 A4:.5 B4:1 | C#5:.5 E5:.5 F#5:.5 A5:.5 >B5:2",
        96,
        0.9,
    );
    s.clip(lead, &section("solo"), 20.0, 8.0, &solo);
    s.finish()
}

/// The vamp's bass line, two bars: Em9 then A13, sixteenths with dead space in them.
fn riff() -> Vec<Ev> {
    let mut v = melody(
        ">E1:.5 r:.25 E2:.25 r:.25 E1:.25 G1:.25 A1:.25 B1:.5 r:.25 D2:.25 E2:.25 r:.25 B1:.25 D2:.5 \
         | >A1:.5 r:.25 A2:.25 r:.25 A1:.25 C#2:.25 E2:.25 G2:.5 r:.25 E2:.25 C#2:.25 r:.25 B1:.25 G1:.5",
        96,
        0.85,
    );
    // The octave pops are the thumb's partner: a little softer and shorter.
    for e in &mut v {
        if e.pitch >= 40 {
            e.vel = e.vel.saturating_sub(10);
            e.len *= 0.8;
        }
    }
    v
}

/// The drums of a section: kick, snare (and rim), hats and cymbals, toms.
fn groove(key: &str, n: usize) -> (Vec<Ev>, Vec<Ev>, Vec<Ev>, Vec<Ev>) {
    let q = 0.25;
    let last = |i: usize| i == n - 1;
    let kick = bars(n, 4.0, |i| match key {
        "intro" if i < 2 => drums(&[("X.....x.x.......", KICK)], q),
        "outro" if last(i) => drums(&[("X...............", KICK)], q),
        _ => drums(
            &[(
                if i % 2 == 0 {
                    "X.....x.x..x...."
                } else {
                    "X..x..x...x..x.."
                },
                KICK,
            )],
            q,
        ),
    });
    let snare = bars(n, 4.0, |i| match key {
        "intro" if i < 2 => drums(&[("....x.......x...", RIM)], q),
        "outro" if last(i) => drums(&[("X...............", SNARE)], q),
        _ if last(i) && key != "outro" => drums(&[("....X..g.g..XxXx", SNARE)], q),
        "solo" => drums(&[("....X..g.g..X..g", SNARE), ("..o.......o.....", RIM)], q),
        _ => drums(
            &[(
                if i % 2 == 0 {
                    "....X..g.g..X..g"
                } else {
                    ".g..X..g..g.X.g."
                },
                SNARE,
            )],
            q,
        ),
    });
    let hats = bars(n, 4.0, |i| {
        let mut v = match key {
            "intro" if i < 2 => drums(&[("x.o.x.o.x.o.x.o.", HAT)], q),
            "outro" if last(i) => vec![],
            "chorus" => drums(&[("xoxoxoxoxoxoxo..", HAT), ("..............x.", OPEN)], q),
            _ => drums(&[("xgogxgogxgogxg..", HAT), ("..............o.", OPEN)], q),
        };
        if (i == 0 && key != "intro") || (key == "outro" && last(i)) {
            v.extend(drums(&[("X...............", CRASH)], q));
        }
        v
    });
    let toms = bars(n, 4.0, |i| {
        if key == "intro" && i == 1 {
            drums(&[("........xxooxoxo", HIGH_TOM)], q)
        } else if key == "intro" && last(i) {
            drums(
                &[("........x.x.", MID_TOM), ("............xxxx", LOW_TOM)],
                q,
            )
        } else if key == "chorus" && i == 3 {
            drums(
                &[("............x.o.", MID_TOM), ("..............xo", LOW_TOM)],
                q,
            )
        } else {
            vec![]
        }
    });
    (kick, snare, hats, toms)
}
