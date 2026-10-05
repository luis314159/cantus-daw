//! **Neon Tide** — house in A minor at 124 BPM.
//!
//! A 909 kit split over four tracks into a drum bus; Gravis plays a saw-and-sub bass on
//! the off-beats; Calor's unison saws hold the chords, pumped by a Comprimo keyed from the
//! kick; Unda plays the plucked hook, the lead and the riser; Exemplar's choir opens the
//! break. Intro 8 · build 8 · drop 16 · break 8 · drop 16 · outro 8 = 64 bars, two
//! minutes.

use super::parts::{self, bars, bass, drums, hold, lay, repeat, B};
use super::song::{self, colors, native, section, Feel, Part, Song};
use super::theory::{melody, shift, Ev};
use crate::i18n::t;
use cantus_core::{EffectKind as K, InstrumentKind as I, Project, StripId, TrackIcon};

const MAIN: &[(&str, f64)] = &[("Am", 4.0), ("F", 4.0), ("C", 4.0), ("G", 4.0)];
const BREAK: &[(&str, f64)] = &[("Fmaj7", 4.0), ("G", 4.0), ("Em7", 4.0), ("Am", 4.0)];

const KICK: u8 = 36;
const RIM: u8 = 37;
const SNARE: u8 = 38;
const CLAP: u8 = 39;
const HAT: u8 = 42;
const OPEN: u8 = 46;
const CRASH: u8 = 49;
const RIDE: u8 = 51;

/// The sections: key, first bar, bars.
const FORM: [(&str, f64, f64); 6] = [
    ("intro", 0.0, 8.0),
    ("build", 8.0, 8.0),
    ("drop", 16.0, 16.0),
    ("break", 32.0, 8.0),
    ("drop", 40.0, 16.0),
    ("outro", 56.0, 8.0),
];

struct Band {
    kick: Part,
    clap: Part,
    hats: Part,
    perc: Part,
    bass: Part,
    chords: Part,
    pluck: Part,
    lead: Part,
    choir: Part,
    rise: Part,
}

pub fn build() -> Project {
    song().finish()
}

/// The second drop — the choir has joined — sixteen bars, looping: what the program opens
/// on.
pub fn startup() -> Project {
    let mut p = song().excerpt(40.0, 16.0);
    p.name = t("demos.song.house");
    p
}

fn song() -> Song {
    let mut s = Song::new(&t("demos.song.house"), 124.0, 0x4E0_71DE);
    let b = band(&mut s);
    desk(&mut s, &b);
    for (key, at, len) in FORM {
        music(&mut s, &b, key, at, len);
    }
    s
}

fn band(s: &mut Song) -> Band {
    let k909 = song::kit("pulsus.kit.909");
    let kick = s.part(
        &t("demos.track.kick"),
        song::pads(&k909, &[KICK]),
        colors::KICK,
        TrackIcon::Drums,
    );
    let clap = s.part(
        &t("demos.track.clap"),
        song::pads(&k909, &[SNARE, CLAP]),
        colors::SNARE,
        TrackIcon::Drums,
    );
    let hats = s.part(
        &t("demos.track.hats"),
        song::pads(&k909, &[HAT, OPEN, RIDE]),
        colors::HATS,
        TrackIcon::Drums,
    );
    let perc = s.part(
        &t("demos.track.perc"),
        song::pads(&k909, &[RIM, CRASH]),
        colors::PERC,
        TrackIcon::Drums,
    );
    let bass = s.part(
        &t("demos.track.bass"),
        native(
            I::Bass,
            "bass.preset.square_sub",
            &[
                ("mode", 1.0),
                ("syn.wave", 0.0),
                ("syn.cutoff", 650.0),
                ("syn.env", 1.6),
                ("syn.f_decay", 170.0),
                ("syn.release", 60.0),
                ("sub.level", 0.55),
            ],
        ),
        colors::BASS,
        TrackIcon::Synth,
    );
    let chords = s.part(
        &t("demos.track.chords"),
        native(I::Analog, "calor.preset.unison_saw", &[]),
        colors::PAD,
        TrackIcon::Synth,
    );
    let pluck = s.part(
        &t("demos.track.pluck"),
        native(I::Wavetable, "unda.preset.pluck_keys", &[]),
        colors::PLUCK,
        TrackIcon::Synth,
    );
    let lead = s.part(
        &t("demos.track.lead"),
        native(I::Wavetable, "unda.preset.hyper_saw", &[]),
        colors::LEAD,
        TrackIcon::Synth,
    );
    let choir = s.part(
        &t("demos.track.choir"),
        native(I::Sampler, "smp.preset.choir", &[]),
        colors::STRINGS,
        TrackIcon::Vocal,
    );
    let rise = s.part(
        &t("demos.section.rise"),
        native(I::Wavetable, "unda.preset.noise_sweep", &[]),
        colors::BELLS,
        TrackIcon::Fx,
    );
    for p in [kick, clap, hats, perc] {
        s.feel(
            p,
            Feel {
                jitter: 0.004,
                vel: 5.0,
                ..Feel::HUMAN
            },
        );
    }
    for p in [bass, chords, pluck, lead] {
        s.feel(
            p,
            Feel {
                jitter: 0.0,
                vel: 3.0,
                ..Feel::HUMAN
            },
        );
    }
    Band {
        kick,
        clap,
        hats,
        perc,
        bass,
        chords,
        pluck,
        lead,
        choir,
        rise,
    }
}

fn desk(s: &mut Song, b: &Band) {
    let bus = s.bus(
        &t("demos.strip.drum_bus"),
        colors::BUS,
        &[b.kick, b.clap, b.hats, b.perc],
    );
    let verb = s.ret(&t("demos.strip.reverb"));
    let delay = s.ret(&t("demos.strip.delay"));
    s.insert(b.kick.strip, K::Eq, Some("eq.preset.kick"), &[]);
    s.insert(
        b.hats.strip,
        K::Eq,
        None,
        &[
            ("b0.used", 1.0),
            ("b0.shape", 3.0),
            ("b0.freq", 350.0),
            ("b0.slope", 24.0),
        ],
    );
    s.insert(
        b.hats.strip,
        K::DeEsser,
        None,
        &[
            ("thr", -32.0),
            ("range", 5.0),
            ("lo", 7_000.0),
            ("hi", 14_000.0),
        ],
    );
    s.insert(
        bus,
        K::Compressor,
        Some("comp.preset.glue"),
        &[("thr", -16.0)],
    );
    s.insert(
        bus,
        K::Multiband,
        Some("mb.preset.low_end"),
        &[("mode", 0.0)],
    );
    s.insert(
        bus,
        K::Limiter,
        Some("lim.preset.drums"),
        &[("gain", 0.0), ("ceiling", -3.0)],
    );
    // The pump: the kick's strip keys a fast compressor on the chords and the bass.
    pump(s, b.chords.strip, b.kick.strip, 12.0);
    pump(s, b.bass.strip, b.kick.strip, 6.0);
    pump(s, b.choir.strip, b.kick.strip, 8.0);
    s.insert(
        b.bass.strip,
        K::Eq,
        Some("eq.preset.mono_bass"),
        &[("mode", 0.0), ("b0.freq", 30.0)],
    );
    s.insert(
        b.chords.strip,
        K::Eq,
        None,
        &[
            ("b0.used", 1.0),
            ("b0.shape", 3.0),
            ("b0.freq", 220.0),
            ("b0.slope", 24.0),
        ],
    );
    s.insert(
        b.lead.strip,
        K::Eq,
        None,
        &[
            ("b0.used", 1.0),
            ("b0.shape", 3.0),
            ("b0.freq", 300.0),
            ("b0.slope", 18.0),
            ("b1.used", 1.0),
            ("b1.freq", 3_000.0),
            ("b1.gain", 1.5),
        ],
    );
    s.insert(
        b.rise.strip,
        K::Gate,
        None,
        &[("thr", -50.0), ("rel", 400.0)],
    );
    s.insert(
        verb,
        K::Space,
        Some("space.preset.concert_hall"),
        &[("mix", 1.0), ("decay", 2.2), ("predelay", 25.0)],
    );
    s.insert(
        verb,
        K::Eq,
        None,
        &[
            ("b0.used", 1.0),
            ("b0.shape", 3.0),
            ("b0.freq", 300.0),
            ("b0.slope", 18.0),
        ],
    );
    s.insert(
        delay,
        K::Delay,
        None,
        &[
            ("time_ms", 363.0),
            ("feedback", 0.38),
            ("tone_hz", 4_000.0),
            ("mix", 1.0),
        ],
    );
    for (p, level) in [
        (b.clap, -14.0),
        (b.pluck, -10.0),
        (b.lead, -10.0),
        (b.chords, -16.0),
        (b.choir, -8.0),
        (b.rise, -10.0),
    ] {
        s.send(p.strip, verb, level);
    }
    s.send(b.pluck.strip, delay, -12.0);
    s.send(b.lead.strip, delay, -14.0);
    s.master(
        K::Eq,
        None,
        &[
            ("b0.used", 1.0),
            ("b0.shape", 3.0),
            ("b0.freq", 22.0),
            ("b0.slope", 18.0),
            ("b1.used", 1.0),
            ("b1.shape", 2.0),
            ("b1.freq", 12_000.0),
            ("b1.gain", 1.0),
        ],
    );
    s.master(K::Multiband, Some("mb.preset.glue"), &[]);
    // Mastering (ADR 0026): Latitudo puts the low end in mono, then Culmen's streaming
    // master — three bands, the low one held, a gentle full-band squeeze, into a −1 dBTP
    // ceiling — driven so the song lands near −14 LUFS.
    s.master(K::Stereo, Some("preset.st.master_air"), &[]);
    s.master(K::Maximizer, Some("preset.max.streaming"), &[("gain", 6.5)]);

    s.trim(b.kick, 6.0);
    s.mix(b.kick.strip, 1.0, 0.0);
    s.trim(b.clap, 12.0);
    s.mix(b.clap.strip, 1.0, 0.0);
    s.trim(b.hats, 12.0);
    s.mix(b.hats.strip, 2.3, 0.18);
    s.trim(b.perc, 12.0);
    s.mix(b.perc.strip, 3.0, -0.25);
    s.mix(bus, 0.0, 0.0);
    s.trim(b.bass, 0.0);
    s.mix(b.bass.strip, -2.0, 0.0);
    s.trim(b.chords, -2.0);
    s.mix(b.chords.strip, -3.5, 0.0);
    s.trim(b.pluck, 0.0);
    s.mix(b.pluck.strip, -1.0, -0.15);
    s.trim(b.lead, 10.0);
    s.mix(b.lead.strip, -2.5, 0.1);
    s.trim(b.choir, -4.0);
    s.mix(b.choir.strip, -3.5, 0.0);
    s.trim(b.rise, 8.0);
    s.mix(b.rise.strip, -7.0, 0.0);
    s.mix(verb, -6.0, 0.0);
    s.mix(delay, -10.0, 0.0);
}

fn pump(s: &mut Song, strip: StripId, key: StripId, depth: f32) {
    let id = s.insert(
        strip,
        K::Compressor,
        Some("comp.preset.pump"),
        &[("range", depth), ("rel", 180.0)],
    );
    s.key(strip, id, key);
}

fn music(s: &mut Song, b: &Band, key: &str, at: f64, len: f64) {
    let n = len as usize;
    let name = section(key);
    let q = 0.25;
    let main = lay(MAIN, 4, 57, 79, 67);
    let chart = if key == "break" {
        lay(BREAK, 4, 57, 79, 67)
    } else {
        main
    };
    let drop = key == "drop";

    // Kick: four on the floor, out for the break, a gap before each drop.
    let kick: Vec<Ev> = bars(n, 4.0, |i| match key {
        "break" => vec![],
        "build" if i == n - 1 => drums(&[("X...x...X...x...", KICK)], q),
        "outro" if i >= n - 2 => {
            if i == n - 1 {
                drums(&[("X...............", KICK)], q)
            } else {
                drums(&[("X...x...X...x...", KICK)], q)
            }
        }
        _ => drums(&[("X...x...X...x...", KICK)], q),
    });
    if !kick.is_empty() {
        s.clip(b.kick, &name, at, len, &kick);
    }

    // Clap on two and four, a ghost before the turn; snare rolls into each drop.
    let clap: Vec<Ev> = bars(n, 4.0, |i| match key {
        "intro" => {
            if i >= 4 {
                drums(&[("....x.......x...", CLAP)], q)
            } else {
                vec![]
            }
        }
        "build" if i >= n - 4 => roll(i - (n - 4)),
        "break" if i >= n - 2 => roll(i - (n - 2) + 2),
        "break" => vec![],
        _ => drums(
            &[(
                if i % 4 == 3 {
                    "....X.......X..g"
                } else {
                    "....X.......X..."
                },
                CLAP,
            )],
            q,
        ),
    });
    if !clap.is_empty() {
        s.clip(b.clap, &name, at, len, &clap);
    }

    // Hats: off-beat closed hats in the intro, open off-beats and sixteenths in the drop.
    let hats: Vec<Ev> = bars(n, 4.0, |i| match key {
        "intro" if i < 2 => vec![],
        "intro" | "outro" | "build" => drums(
            &[
                ("..x...x...x...x.", HAT),
                (
                    if key == "build" {
                        "g...g...g...g..."
                    } else {
                        "................"
                    },
                    HAT,
                ),
            ],
            q,
        ),
        "break" => {
            if i >= 4 {
                drums(&[("..o...o...o...o.", HAT)], q)
            } else {
                vec![]
            }
        }
        _ => drums(
            &[
                ("..x...x...x...x.", OPEN),
                ("x...o...x...o..g", HAT),
                (
                    if i % 2 == 1 {
                        "o...o...o...o..."
                    } else {
                        "................"
                    },
                    RIDE,
                ),
            ],
            q,
        ),
    });
    if !hats.is_empty() {
        s.clip(b.hats, &name, at, len, &hats);
    }

    // Percussion: a syncopated rim, a crash on every eight-bar phrase of the drop.
    let perc: Vec<Ev> = bars(n, 4.0, |i| {
        let mut v = match key {
            "break" => vec![],
            "intro" if i < 4 => vec![],
            _ => drums(
                &[(
                    if i % 2 == 0 {
                        "...o..o....o.o.."
                    } else {
                        "...o..o......o.o"
                    },
                    RIM,
                )],
                q,
            ),
        };
        if (drop && i % 8 == 0) || (key == "outro" && i == 0) || (key == "break" && i == 0) {
            v.extend(drums(&[("X...............", CRASH)], q));
        }
        v
    });
    if !perc.is_empty() {
        s.clip(b.perc, &name, at, len, &perc);
    }

    // Bass: off-beat eighths with an octave kick at the end of the bar.
    if matches!(key, "build" | "drop" | "outro") {
        let rhythm: &[(f64, f64, u8, B)] = &[
            (0.5, 0.4, 100, B::R),
            (1.5, 0.4, 92, B::R),
            (2.5, 0.4, 98, B::R),
            (3.25, 0.2, 84, B::O),
            (3.5, 0.4, 94, B::R),
        ];
        let mut v = bass(&chart, 33, 4.0, rhythm);
        v = repeat(&v, chart.beats, n / 4);
        if key == "build" {
            v.retain(|e| e.at >= 16.0 && e.at < 28.0);
        }
        if key == "outro" {
            v.retain(|e| e.at < 16.0);
        }
        s.clip(b.bass, &name, at, len, &v);
    }

    // Chords: held, pumped by the kick; soft and low in the build, full in the drop.
    let vel = match key {
        "drop" => 92,
        "break" => 84,
        "build" => 64,
        _ => 0,
    };
    if vel > 0 {
        let v = repeat(&hold(&chart, vel, 0.98), chart.beats, n / 4);
        s.clip(b.chords, &name, at, len, &v);
    }

    // The pluck: the hook, from the intro's fifth bar on.
    let hook = melody(
        "A4:.5 C5:.25 E5:.5 A5:.25 G5:.5 E5:.5 C5:.5 D5:.5 E5:.5 \
         | A4:.5 C5:.25 F5:.5 A5:.25 G5:.5 F5:.5 C5:.5 D5:.5 C5:.5 \
         | G4:.5 C5:.25 E5:.5 G5:.25 G5:.5 E5:.5 C5:.5 D5:.5 E5:.5 \
         | G4:.5 B4:.25 D5:.5 G5:.25 A5:.5 G5:.5 D5:.5 B4:.5 D5:.5",
        88,
        0.8,
    );
    let pluck: Vec<Ev> = match key {
        "intro" => parts::swell(&shift(&hook, 16.0), 32.0, 0.55, 0.85),
        "break" => {
            let soft = melody(
                "A4:1 C5:1 E5:1 F5:1 | G4:1 B4:1 D5:1 G5:1 | G4:1 B4:1 E5:1 G5:1 | A4:1 C5:1 E5:1 A5:1",
                70,
                0.9,
            );
            repeat(&soft, 16.0, 2)
        }
        _ => repeat(&hook, 16.0, n / 4),
    };
    s.clip(b.pluck, &name, at, len, &pluck);

    // The lead: the drop's song, the second time an answer above it.
    if drop {
        let a = melody(
            ">E5:.75 E5:.75 D5:.5 E5:1 C5:1 | >C5:.75 C5:.75 A4:.5 C5:1 D5:1 \
             | >E5:.75 E5:.75 G5:.5 E5:1 D5:1 | >D5:.75 B4:.75 G4:.5 B4:1 D5:1",
            98,
            0.92,
        );
        let bb = melody(
            ">E5:.75 E5:.75 D5:.5 E5:1 A5:1 | >G5:.75 F5:.75 E5:.5 C5:1 D5:1 \
             | >E5:.75 G5:.75 E5:.5 D5:1 C5:1 | >D5:1.5 B4:.5 D5:2",
            100,
            0.92,
        );
        let mut v = a.clone();
        v.extend(shift(&bb, 16.0));
        v.extend(shift(&a, 32.0));
        v.extend(shift(&bb, 48.0));
        s.clip(b.lead, &name, at, len, &v);
    }

    // The choir: the break's air, and under the second drop.
    if key == "break" || (drop && at > 30.0) {
        let c = lay(if key == "break" { BREAK } else { MAIN }, 3, 55, 74, 64);
        let v = repeat(
            &hold(&c, if key == "break" { 90 } else { 72 }, 0.99),
            c.beats,
            n / 4,
        );
        s.clip(b.choir, &name, at, len, &v);
    }

    // The riser: a long sweep into each drop.
    if key == "build" || key == "break" {
        let start = if key == "build" { 16.0 } else { 24.0 };
        s.clip(
            b.rise,
            &section("rise"),
            at + start / 4.0,
            len - start / 4.0,
            &[Ev::new(0.0, len * 4.0 - start, 69, 96)],
        );
    }
}

/// One bar of a snare roll that tightens bar by bar: quarters, eighths, sixteenths, then
/// thirty-seconds, louder all the way.
fn roll(stage: usize) -> Vec<Ev> {
    let (grid, step) = match stage {
        0 => ("x...x...x...x...", 0.25),
        1 => ("x.x.x.x.x.x.x.x.", 0.25),
        2 => ("xxxxxxxxxxxxxxxx", 0.25),
        _ => ("xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx", 0.125),
    };
    let from = 0.45 + stage as f64 * 0.15;
    parts::swell(&drums(&[(grid, SNARE)], step), 4.0, from, from + 0.15)
}
