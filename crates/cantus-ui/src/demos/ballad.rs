//! **Morning Light** — a worship / pop ballad in D major at 74 BPM.
//!
//! Chorda's warm grand carries it from the first bar; Unda's warm pad and Exemplar's
//! nylon guitar fill the chorus; Modulus plays the melody on its FM flute and Exemplar's
//! bells answer it; Gravis is a fingered electric bass; Pulsus II's studio kit is split
//! over four tracks into a drum bus. Intro 4 · verse 8 · chorus 8 · bridge 4 · chorus 8 ·
//! outro 4 = 36 bars, about two minutes.

use super::parts::{self, arp, bars, bass, comp, drums, hold, lay, Chart, B};
use super::song::{self, colors, native, section, Feel, Song};
use super::theory::{melody, Ev};
use crate::i18n::t;
use cantus_core::{EffectKind as K, InstrumentKind as I, Project, TrackIcon};

const VERSE: &[(&str, f64)] = &[("D", 4.0), ("A/C#", 4.0), ("Bm7", 4.0), ("Gadd9", 4.0)];
const CHORUS: &[(&str, f64)] = &[
    ("G", 4.0),
    ("D/F#", 4.0),
    ("A", 4.0),
    ("Bm7", 4.0),
    ("G", 4.0),
    ("D/F#", 4.0),
    ("Em7", 4.0),
    ("Asus4", 2.0),
    ("A", 2.0),
];
const BRIDGE: &[(&str, f64)] = &[("Em7", 4.0), ("G", 4.0), ("Bm7", 4.0), ("A", 4.0)];
const OUTRO: &[(&str, f64)] = &[("G", 4.0), ("A", 4.0), ("D", 8.0)];

/// The sections: name key, first bar, length in bars, chart.
const FORM: [(&str, f64, f64, Chart<'static>); 6] = [
    ("intro", 0.0, 4.0, VERSE),
    ("verse", 4.0, 8.0, VERSE),
    ("chorus", 12.0, 8.0, CHORUS),
    ("bridge", 20.0, 4.0, BRIDGE),
    ("chorus", 24.0, 8.0, CHORUS),
    ("outro", 32.0, 4.0, OUTRO),
];

const KICK: u8 = 36;
const RIM: u8 = 37;
const SNARE: u8 = 38;
const HAT: u8 = 42;
const PEDAL: u8 = 44;
const LOW_TOM: u8 = 45;
const OPEN: u8 = 46;
const MID_TOM: u8 = 47;
const CRASH: u8 = 49;
const HIGH_TOM: u8 = 50;
const RIDE: u8 = 51;

pub fn build() -> Project {
    let mut s = Song::new(&t("demos.song.ballad"), 74.0, 0xBA11AD);
    let studio = song::kit("pulsus.kit.studio");

    // ---- the band ----
    let kick = s.part(
        &t("demos.track.kick"),
        song::pads(&studio, &[KICK]),
        colors::KICK,
        TrackIcon::Drums,
    );
    let snare = s.part(
        &t("demos.track.snare"),
        song::pads(&studio, &[RIM, SNARE]),
        colors::SNARE,
        TrackIcon::Drums,
    );
    let cym = s.part(
        &t("demos.track.cymbals"),
        song::pads(&studio, &[HAT, PEDAL, OPEN, CRASH, RIDE]),
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
        native(I::Bass, "bass.preset.p_fingers", &[]),
        colors::BASS,
        TrackIcon::Strings,
    );
    let keys = s.part(
        &t("demos.track.piano"),
        native(I::Chorda, "chorda.preset.ballad", &[]),
        colors::KEYS,
        TrackIcon::Keys,
    );
    let pad = s.part(
        &t("demos.track.pad"),
        native(I::Wavetable, "unda.preset.warp_pad", &[]),
        colors::PAD,
        TrackIcon::Synth,
    );
    let guitar = s.part(
        &t("demos.track.guitar"),
        native(I::Sampler, "smp.preset.nylon", &[]),
        colors::PLUCK,
        TrackIcon::Strings,
    );
    let flute = s.part(
        &t("demos.track.melody"),
        native(I::Fm, "fm.preset.flute", &[]),
        colors::LEAD,
        TrackIcon::Vocal,
    );
    let bells = s.part(
        &t("demos.track.bells"),
        native(I::Sampler, "smp.preset.bells", &[]),
        colors::BELLS,
        TrackIcon::Keys,
    );
    s.feel(
        kick,
        Feel {
            jitter: 0.006,
            ..Feel::HUMAN
        },
    );

    // ---- the desk ----
    let drum_bus = s.bus(
        &t("demos.strip.drum_bus"),
        colors::BUS,
        &[kick, snare, cym, toms],
    );
    let verb = s.ret(&t("demos.strip.reverb"));
    let delay = s.ret(&t("demos.strip.delay"));
    s.insert(kick.strip, K::Eq, Some("eq.preset.kick"), &[]);
    s.insert(
        toms.strip,
        K::Gate,
        Some("preset.gate.toms"),
        &[("thr", -40.0)],
    );
    s.insert(
        drum_bus,
        K::Compressor,
        Some("comp.preset.drum_bus"),
        &[("thr", -22.0)],
    );
    s.insert(
        drum_bus,
        K::Limiter,
        Some("lim.preset.drums"),
        &[("gain", 0.0), ("ceiling", -3.0)],
    );
    s.insert(
        bass_p.strip,
        K::Eq,
        None,
        &[
            ("b0.used", 1.0),
            ("b0.shape", 3.0),
            ("b0.freq", 35.0),
            ("b0.slope", 24.0),
            ("b1.used", 1.0),
            ("b1.freq", 750.0),
            ("b1.gain", 2.0),
            ("b1.q", 1.2),
        ],
    );
    s.insert(bass_p.strip, K::Compressor, Some("comp.preset.bass"), &[]);
    s.insert(
        keys.strip,
        K::Eq,
        None,
        &[
            ("b0.used", 1.0),
            ("b0.shape", 3.0),
            ("b0.freq", 70.0),
            ("b0.slope", 12.0),
            ("b1.used", 1.0),
            ("b1.freq", 320.0),
            ("b1.gain", -2.0),
            ("b1.q", 1.0),
            ("b2.used", 1.0),
            ("b2.shape", 2.0),
            ("b2.freq", 6_000.0),
            ("b2.gain", 2.0),
        ],
    );
    s.insert(
        pad.strip,
        K::Eq,
        None,
        &[
            ("b0.used", 1.0),
            ("b0.shape", 3.0),
            ("b0.freq", 180.0),
            ("b0.slope", 24.0),
        ],
    );
    s.insert(flute.strip, K::DeEsser, Some("ds.preset.female"), &[]);
    s.insert(flute.strip, K::Compressor, Some("comp.preset.vocal"), &[]);
    s.insert(
        verb,
        K::Space,
        Some("space.preset.concert_hall"),
        &[("mix", 1.0), ("decay", 2.8)],
    );
    s.insert(
        delay,
        K::Delay,
        None,
        &[
            ("time_ms", 608.0),
            ("feedback", 0.35),
            ("tone_hz", 3_500.0),
            ("mix", 1.0),
        ],
    );
    for (part, level) in [
        (snare, -14.0),
        (toms, -12.0),
        (keys, -12.0),
        (pad, -10.0),
        (guitar, -12.0),
        (flute, -9.0),
        (bells, -8.0),
        (cym, -20.0),
    ] {
        s.send(part.strip, verb, level);
    }
    s.send(flute.strip, delay, -16.0);
    s.send(bells.strip, delay, -12.0);
    s.master(
        K::Eq,
        None,
        &[
            ("b0.used", 1.0),
            ("b0.shape", 3.0),
            ("b0.freq", 25.0),
            ("b0.slope", 18.0),
            ("b1.used", 1.0),
            ("b1.shape", 2.0),
            ("b1.freq", 11_000.0),
            ("b1.gain", 1.5),
        ],
    );
    s.master(K::Multiband, Some("mb.preset.glue"), &[]);
    // Mastering (ADR 0026): Latitudo puts the low end in mono, then Culmen's streaming
    // master — three bands, the low one held, a gentle full-band squeeze, into a −1 dBTP
    // ceiling — driven so the song lands near −14 LUFS.
    s.master(K::Stereo, Some("preset.st.master_low"), &[]);
    s.master(K::Maximizer, Some("preset.max.streaming"), &[("gain", 7.5)]);

    // Levels, set by ear-in-numbers: every part rendered alone, measured, then placed.
    s.trim(kick, 6.0);
    s.mix(kick.strip, -2.0, 0.0);
    s.trim(snare, 11.0);
    s.mix(snare.strip, 1.5, 0.0);
    s.trim(cym, 10.0);
    s.mix(cym.strip, -2.0, 0.15);
    s.trim(toms, 6.0);
    s.mix(toms.strip, 0.0, -0.1);
    s.mix(drum_bus, 0.0, 0.0);
    s.trim(bass_p, 3.0);
    s.mix(bass_p.strip, -0.5, 0.0);
    s.trim(keys, 3.0); // +8 before Lamina's calibration (ADR 0039, +5 dB)
    s.mix(keys.strip, 0.0, -0.12);
    s.trim(pad, 12.0);
    s.mix(pad.strip, 2.5, 0.0);
    s.trim(guitar, 8.0);
    s.mix(guitar.strip, 0.5, 0.35);
    s.trim(flute, 0.0);
    s.mix(flute.strip, 0.5, 0.05);
    s.trim(bells, 3.0);
    s.mix(bells.strip, -4.0, -0.3);
    s.mix(verb, -6.0, 0.0);
    s.mix(delay, -10.0, 0.0);

    // ---- the music ----
    for (key, at, len, chart) in FORM {
        let name = section(key);
        let piano = lay(chart, 3, 57, 76, 66);
        let wide = lay(chart, 4, 50, 74, 62);
        let full = key == "chorus";

        // Piano: in the verse a gentle pulse with the left hand under it; in the chorus
        // fuller, octaves in the left hand.
        let mut k = if full {
            let mut v = comp(
                &piano,
                4.0,
                &[
                    (0.0, 1.4, 84),
                    (1.5, 0.9, 64),
                    (2.0, 1.4, 76),
                    (3.0, 0.5, 62),
                    (3.5, 0.5, 66),
                ],
            );
            v.extend(bass(
                &piano,
                38,
                4.0,
                &[(0.0, 3.8, 78, B::R), (0.0, 3.8, 64, B::O)],
            ));
            v
        } else {
            let mut v = comp(
                &piano,
                4.0,
                &[
                    (0.0, 1.9, 74),
                    (1.5, 1.0, 56),
                    (2.0, 1.9, 66),
                    (3.5, 0.5, 54),
                ],
            );
            v.extend(bass(
                &piano,
                38,
                4.0,
                &[(0.0, 3.8, 70, B::R), (2.0, 1.9, 58, B::F)],
            ));
            v
        };
        if key == "outro" {
            k = hold(&piano, 70, 0.98);
            k.extend(bass(&piano, 38, 4.0, &[(0.0, 3.8, 68, B::R)]));
        }
        s.clip(keys, &name, at, len, &k);

        let mut p = hold(&wide, if full { 76 } else { 60 }, 1.0);
        if key == "intro" {
            p = parts::swell(&p, 16.0, 0.6, 1.0);
        }
        s.clip(pad, &name, at, len, &p);
        if key == "verse" || key == "chorus" {
            let rhythm: &[(f64, f64, u8, B)] = if full {
                &[
                    (0.0, 1.4, 98, B::R),
                    (1.5, 0.45, 76, B::R),
                    (2.0, 0.95, 90, B::F),
                    (3.0, 0.45, 80, B::O),
                    (3.5, 0.45, 82, B::A),
                ]
            } else {
                &[
                    (0.0, 1.45, 92, B::R),
                    (1.5, 0.45, 70, B::R),
                    (2.0, 1.4, 86, B::R),
                    (3.5, 0.45, 74, B::A),
                ]
            };
            s.clip(bass_p, &name, at, len, &bass(&wide, 33, 4.0, rhythm));
        }
        if key == "bridge" {
            s.clip(
                bass_p,
                &name,
                at,
                len,
                &bass(&wide, 33, 4.0, &[(0.0, 3.9, 86, B::R)]),
            );
        }
        if key == "outro" {
            // G, A, then the home chord held to the end.
            let mut v = bass(&wide, 33, 4.0, &[(0.0, 3.9, 84, B::R)]);
            v.truncate(3);
            if let Some(last) = v.last_mut() {
                last.len = 7.5;
            }
            s.clip(bass_p, &name, at, len, &v);
        }
        if full {
            let g = lay(chart, 3, 52, 71, 62);
            s.clip(
                guitar,
                &name,
                at,
                len,
                &arp(&g, 40, &[0, 1, 2, 3, 2, 1, 2, 3], 0.5, 0.9, 72),
            );
        }
        // Drums.
        let d = drum_section(key, len as usize);
        s.clip(kick, &name, at, len, &d.kick);
        if !d.snare.is_empty() {
            s.clip(snare, &name, at, len, &d.snare);
        }
        if !d.cym.is_empty() {
            s.clip(cym, &name, at, len, &d.cym);
        }
        if !d.toms.is_empty() {
            s.clip(toms, &name, at, len, &d.toms);
        }
    }

    // ---- melody and answers ----
    let verse_mel = melody(
        "r:1 F#4:.5 A4:.5 A4:1 B4:.5 A4:.5 | A4:1.5 E4:.5 E4:2 | r:.5 D4:.5 F#4:.5 A4:.5 B4:1 A4:.5 F#4:.5 | G4:1.5 F#4:.5 E4:2 \
         | r:1 F#4:.5 A4:.5 >D5:1 C#5:.5 B4:.5 | A4:1.5 E4:.5 A4:1 B4:.5 C#5:.5 | D5:1.5 C#5:.5 B4:1 A4:1 | B4:2 A4:1 r:1",
        84,
        0.94,
    );
    let chorus_mel = melody(
        ">D5:1.5 B4:.5 D5:1 E5:1 | >F#5:2 E5:1 D5:1 | E5:1.5 C#5:.5 E5:1 F#5:1 | D5:3 r:1 \
         | >D5:1.5 B4:.5 D5:1 G5:1 | >F#5:2 E5:1 D5:1 | E5:1.5 D5:.5 B4:1 D5:1 | D5:2 C#5:2",
        92,
        0.96,
    );
    s.clip(flute, &section("verse"), 4.0, 8.0, &verse_mel);
    s.clip(flute, &section("chorus"), 12.0, 8.0, &chorus_mel);
    s.clip(flute, &section("chorus"), 24.0, 8.0, &chorus_mel);
    let bells_intro = melody(
        "F#5:1.5 E5:.5 D5:2 | C#5:1.5 D5:.5 E5:2 | D5:1.5 C#5:.5 B4:2 | G4:2 A4:2",
        80,
        1.0,
    );
    let bells_bridge = melody(
        "B5:2 A5:1 G5:1 | B5:2 D6:2 | F#6:2 E6:1 D6:1 | E6:3 r:1",
        78,
        1.0,
    );
    let bells_outro = melody("D6:1 A5:1 F#5:1 E5:1 | E5:4 | F#5:8", 74, 1.0);
    s.clip(bells, &section("intro"), 0.0, 4.0, &bells_intro);
    s.clip(bells, &section("bridge"), 20.0, 4.0, &bells_bridge);
    s.clip(bells, &section("outro"), 32.0, 4.0, &bells_outro);
    s.finish()
}

struct Kit {
    kick: Vec<Ev>,
    snare: Vec<Ev>,
    cym: Vec<Ev>,
    toms: Vec<Ev>,
}

/// The drums of one section: a groove per bar, a fill on the last bar into the next.
fn drum_section(key: &str, n: usize) -> Kit {
    let mut k = Kit {
        kick: vec![],
        snare: vec![],
        cym: vec![],
        toms: vec![],
    };
    let q = 0.25;
    match key {
        "intro" => {
            // A pedal hat counts in under the last two bars, a swell on the ride.
            k.cym = bars(n, 4.0, |b| match b {
                2 => drums(&[("x...o...x...o...", PEDAL)], q),
                3 => drums(
                    &[("x...o...x...o.o.", PEDAL), ("....g...o...x...", RIDE)],
                    q,
                ),
                _ => vec![],
            });
            k.kick = bars(n, 4.0, |b| {
                if b == 3 {
                    drums(&[("........o.....x.", KICK)], q)
                } else {
                    vec![]
                }
            });
        }
        "verse" => {
            k.kick = bars(n, 4.0, |b| {
                drums(
                    &[(
                        if b % 2 == 0 {
                            "X.....o.x......."
                        } else {
                            "X.....o.x.....o."
                        },
                        KICK,
                    )],
                    q,
                )
            });
            k.snare = bars(n, 4.0, |b| {
                if b == n - 1 {
                    drums(&[("....x.......x...", RIM), ("..........g.o.xx", SNARE)], q)
                } else {
                    drums(
                        &[
                            ("....x.......x...", RIM),
                            (
                                if b % 4 == 3 {
                                    "..............g."
                                } else {
                                    "................"
                                },
                                SNARE,
                            ),
                        ],
                        q,
                    )
                }
            });
            k.cym = bars(n, 4.0, |b| {
                let hats = if b < 4 {
                    "o.g.o.g.o.g.o.g."
                } else {
                    "x.o.x.o.x.o.x.og"
                };
                drums(&[(hats, HAT)], q)
            });
        }
        "chorus" => {
            k.kick = bars(n, 4.0, |b| {
                drums(
                    &[(
                        if b % 2 == 0 {
                            "X.....x.x......."
                        } else {
                            "X.....x.x.....x."
                        },
                        KICK,
                    )],
                    q,
                )
            });
            k.snare = bars(n, 4.0, |b| {
                if b == n - 1 {
                    drums(&[("....X..g..g.X...", SNARE)], q)
                } else {
                    drums(
                        &[(
                            if b % 2 == 0 {
                                "....X..g......X."
                            } else {
                                "....X......g.X.."
                            },
                            SNARE,
                        )],
                        q,
                    )
                }
            });
            k.cym = bars(n, 4.0, |b| {
                let mut v = drums(
                    &[("x.o.x.o.x.o.x.o.", RIDE), ("..o...o...o...o.", PEDAL)],
                    q,
                );
                if b == 0 || b == 4 {
                    v.extend(drums(&[("X...............", CRASH)], q));
                }
                v
            });
            k.toms = bars(n, 4.0, |b| {
                if b == n - 1 {
                    drums(&[("..............xo", LOW_TOM)], q)
                } else {
                    vec![]
                }
            });
        }
        "bridge" => {
            // Half time on the toms, building to a snare roll.
            k.kick = bars(n, 4.0, |b| {
                drums(
                    &[(
                        if b < 3 {
                            "X..............."
                        } else {
                            "X.......x...x..."
                        },
                        KICK,
                    )],
                    q,
                )
            });
            k.toms = bars(n, 4.0, |b| {
                if b < 3 {
                    drums(
                        &[("....o.......o.g.", LOW_TOM), ("........g.......", MID_TOM)],
                        q,
                    )
                } else {
                    drums(&[("x.x.....", HIGH_TOM), ("....x.x.", MID_TOM)], 0.5)
                }
            });
            k.snare = bars(n, 4.0, |b| {
                if b == 3 {
                    parts::swell(
                        &drums(&[("........gogoxoxXxXXX", SNARE)], 0.2),
                        4.0,
                        0.5,
                        1.05,
                    )
                } else {
                    vec![]
                }
            });
            k.cym = bars(n, 4.0, |b| {
                drums(
                    &[(
                        if b == 0 {
                            "X..............."
                        } else {
                            "o.......o......."
                        },
                        if b == 0 { CRASH } else { RIDE },
                    )],
                    q,
                )
            });
        }
        _ => {
            // Outro: a crash and the last hit.
            k.kick = bars(n, 4.0, |b| {
                if b == 0 || b == 2 {
                    drums(&[("X...............", KICK)], q)
                } else {
                    vec![]
                }
            });
            k.cym = bars(n, 4.0, |b| {
                if b == 0 || b == 2 {
                    drums(&[("X...............", CRASH)], q)
                } else {
                    vec![]
                }
            });
            k.toms = bars(n, 4.0, |b| {
                if b == 1 {
                    drums(&[("........o.o.x.x.", LOW_TOM)], q)
                } else {
                    vec![]
                }
            });
        }
    }
    k
}
