//! **Paper Lanterns** — lo-fi hip-hop in F major at 80 BPM.
//!
//! Lamina's worn tines play ninth chords with a lazy roll; Gravis is a fretless bass;
//! Exemplar's nylon guitar sings the tune; Modulus answers on marimba; Calor's velvet pad
//! sits under the last section. A boom-bap kit, swung hard and behind the beat, into a
//! squashed drum bus. The colour is the mixing: Aequalis takes the top off the keys and
//! the master, Comprimo pumps the drums, Spatium puts it all in a small dark room.
//! Intro 4 · A 8 · B 8 · A 8 · outro 4 = 32 bars.

use super::parts::{bars, bass, drums, hold, lay, repeat, Laid, B};
use super::song::{self, colors, native, section, Feel, Song};
use super::theory::{melody, Ev};
use crate::i18n::t;
use cantus_core::{EffectKind as K, InstrumentKind as I, Project, TrackIcon};

const LOOP: &[(&str, f64)] = &[("Gm9", 4.0), ("C13", 4.0), ("Fmaj9", 4.0), ("Dm9", 4.0)];
const B_PART: &[(&str, f64)] = &[("Bbmaj7", 4.0), ("Am7", 4.0), ("Gm9", 4.0), ("C7sus4", 4.0)];
const END: &[(&str, f64)] = &[("Gm9", 4.0), ("C13", 4.0), ("Fmaj9", 8.0)];

const FORM: [(&str, f64, f64); 5] = [
    ("intro", 0.0, 4.0),
    ("verse", 4.0, 8.0),
    ("hook", 12.0, 8.0),
    ("verse", 20.0, 8.0),
    ("outro", 28.0, 4.0),
];

const KICK: u8 = 36;
const RIM: u8 = 37;
const SNARE: u8 = 38;
const HAT: u8 = 42;
const OPEN: u8 = 46;
const SHAKER: u8 = 70;

pub fn build() -> Project {
    let mut s = Song::new(&t("demos.song.lofi"), 80.0, 0x10F1);
    let boom = song::kit("pulsus.kit.boom_bap");
    let kick = s.part(
        &t("demos.track.kick"),
        song::pads(&boom, &[KICK]),
        colors::KICK,
        TrackIcon::Drums,
    );
    let snare = s.part(
        &t("demos.track.snare"),
        song::pads(&boom, &[RIM, SNARE]),
        colors::SNARE,
        TrackIcon::Drums,
    );
    // The hats: the studio kit's, which keep their top through the bus's low-pass, and the
    // boom-bap shaker brought up.
    let studio = song::kit("pulsus.kit.studio");
    let mut hat_pads: Vec<cantus_core::DrumPad> = studio
        .pads
        .iter()
        .filter(|p| p.pitch == HAT || p.pitch == OPEN)
        .cloned()
        .map(|mut p| {
            p.level_db += 6.0;
            p
        })
        .collect();
    hat_pads.extend(
        boom.pads
            .iter()
            .filter(|p| p.pitch == SHAKER)
            .cloned()
            .map(|mut p| {
                p.level_db += 8.0;
                p
            }),
    );
    let hat_kit = cantus_core::PulsusParams::of(hat_pads);
    let hats = s.part(
        &t("demos.track.hats"),
        song::pads(&hat_kit, &[HAT, OPEN, SHAKER]),
        colors::HATS,
        TrackIcon::Drums,
    );
    let keys = s.part(
        &t("demos.track.keys"),
        native(I::ElectricPiano, "epiano.preset.worn", &[]),
        colors::KEYS,
        TrackIcon::Keys,
    );
    let bass_p = s.part(
        &t("demos.track.bass"),
        native(I::Bass, "bass.preset.fretless", &[]),
        colors::BASS,
        TrackIcon::Strings,
    );
    let guitar = s.part(
        &t("demos.track.guitar"),
        native(I::Sampler, "smp.preset.nylon", &[]),
        colors::PLUCK,
        TrackIcon::Strings,
    );
    let vibes = s.part(
        &t("demos.track.bells"),
        native(I::Fm, "fm.preset.marimba", &[]),
        colors::BELLS,
        TrackIcon::Keys,
    );
    let pad = s.part(
        &t("demos.track.pad"),
        native(I::Analog, "calor.preset.velvet_pad", &[]),
        colors::PAD,
        TrackIcon::Synth,
    );
    let lazy = Feel {
        jitter: 0.015,
        vel: 8.0,
        ..Feel::HUMAN
    }
    .swung(0.3, 0.25);
    for p in [kick, hats] {
        s.feel(p, lazy);
    }
    s.feel(
        snare,
        Feel {
            jitter: 0.02,
            ..lazy
        },
    );
    for p in [keys, bass_p, guitar, vibes] {
        s.feel(
            p,
            Feel {
                jitter: 0.015,
                vel: 6.0,
                ..Feel::HUMAN
            }
            .swung(0.3, 0.25),
        );
    }

    let bus = s.bus(
        &t("demos.strip.drum_bus"),
        colors::BUS,
        &[kick, snare, hats],
    );
    let verb = s.ret(&t("demos.strip.reverb"));
    let delay = s.ret(&t("demos.strip.delay"));
    s.insert(
        kick.strip,
        K::Eq,
        Some("eq.preset.kick"),
        &[("b3.gain", 0.0)],
    );
    s.insert(
        bus,
        K::Compressor,
        Some("comp.preset.drum_bus"),
        &[("thr", -26.0), ("ratio", 6.0), ("mix", 0.7)],
    );
    s.insert(
        bus,
        K::Limiter,
        Some("lim.preset.drums"),
        &[("gain", 0.0), ("ceiling", -3.0)],
    );
    s.insert(
        bus,
        K::Eq,
        None,
        &[
            ("b0.used", 1.0),
            ("b0.shape", 4.0),
            ("b0.freq", 11_000.0),
            ("b0.slope", 12.0),
        ],
    );
    s.insert(
        keys.strip,
        K::Eq,
        None,
        &[
            ("b0.used", 1.0),
            ("b0.shape", 3.0),
            ("b0.freq", 110.0),
            ("b0.slope", 12.0),
            ("b1.used", 1.0),
            ("b1.shape", 4.0),
            ("b1.freq", 4_500.0),
            ("b1.slope", 12.0),
            ("b2.used", 1.0),
            ("b2.freq", 900.0),
            ("b2.gain", 2.0),
            ("b2.q", 0.7),
        ],
    );
    // Worn tape on the keys (Fervor): a little drive, the top rolled off, level-matched.
    s.insert(
        keys.strip,
        K::Saturator,
        Some("preset.sat.tape_bus"),
        &[("drive", 9.0), ("char", 0.7), ("hi_cut", 9_000.0)],
    );
    s.insert(
        keys.strip,
        K::Compressor,
        None,
        &[
            ("thr", -24.0),
            ("ratio", 3.0),
            ("att", 15.0),
            ("rel", 200.0),
            ("auto_gain", 1.0),
        ],
    );
    s.insert(bass_p.strip, K::Compressor, Some("comp.preset.bass"), &[]);
    // The nylon's attack held by a limiter, so the tune can sit up front without its
    // plucks peaking over the mix.
    s.insert(
        guitar.strip,
        K::Limiter,
        None,
        &[("gain", 6.0), ("ceiling", -9.0), ("rel", 120.0)],
    );
    s.insert(
        guitar.strip,
        K::Eq,
        None,
        &[
            ("b0.used", 1.0),
            ("b0.shape", 3.0),
            ("b0.freq", 180.0),
            ("b0.slope", 12.0),
            ("b1.used", 1.0),
            ("b1.shape", 4.0),
            ("b1.freq", 6_000.0),
            ("b1.slope", 12.0),
        ],
    );
    s.insert(
        vibes.strip,
        K::Gate,
        None,
        &[("thr", -46.0), ("rel", 300.0)],
    );
    s.insert(
        verb,
        K::Space,
        Some("space.preset.drum_room"),
        &[("mix", 1.0), ("decay", 1.6), ("bright", 0.25)],
    );
    s.insert(
        delay,
        K::Delay,
        None,
        &[
            ("time_ms", 562.0),
            ("feedback", 0.42),
            ("tone_hz", 2_200.0),
            ("mix", 1.0),
        ],
    );
    for (p, level) in [
        (snare, -12.0),
        (keys, -12.0),
        (guitar, -10.0),
        (vibes, -8.0),
        (pad, -10.0),
    ] {
        s.send(p.strip, verb, level);
    }
    s.send(guitar.strip, delay, -12.0);
    s.send(vibes.strip, delay, -10.0);
    s.master(
        K::Eq,
        None,
        &[
            ("b0.used", 1.0),
            ("b0.shape", 3.0),
            ("b0.freq", 30.0),
            ("b0.slope", 18.0),
            ("b1.used", 1.0),
            ("b1.shape", 4.0),
            ("b1.freq", 12_000.0),
            ("b1.slope", 12.0),
        ],
    );
    s.master(K::Compressor, Some("comp.preset.glue"), &[]);
    // Mastering (ADR 0026): Latitudo puts the low end in mono, then Culmen's streaming
    // master — three bands, the low one held, a gentle full-band squeeze, into a −1 dBTP
    // ceiling — driven so the song lands near −14 LUFS.
    s.master(K::Stereo, Some("preset.st.master_low"), &[]);
    s.master(K::Maximizer, Some("preset.max.streaming"), &[("gain", 6.0)]);

    s.trim(kick, 6.0);
    s.mix(kick.strip, -1.5, 0.0);
    s.trim(snare, 8.0);
    s.mix(snare.strip, 0.0, 0.05);
    s.trim(hats, 12.0);
    s.mix(hats.strip, 1.0, 0.25);
    s.mix(bus, 0.0, 0.0);
    s.trim(keys, 9.0);
    s.mix(keys.strip, -0.5, -0.1);
    s.trim(bass_p, 3.0);
    s.mix(bass_p.strip, -1.0, 0.0);
    s.trim(guitar, 8.0);
    s.mix(guitar.strip, 2.0, 0.2);
    s.trim(vibes, 8.0);
    s.mix(vibes.strip, -4.0, -0.3);
    s.trim(pad, 0.0);
    s.mix(pad.strip, -7.0, 0.0);
    s.mix(verb, -6.0, 0.0);
    s.mix(delay, -10.0, 0.0);

    for (key, at, len) in FORM {
        let name = section(key);
        let n = len as usize;
        let chart = match key {
            "hook" => B_PART,
            "outro" => END,
            _ => LOOP,
        };
        let k = lay(chart, 4, 55, 77, 65);
        let times = ((len * 4.0) / k.beats).max(1.0) as usize;

        // Keys: a lazy roll up each chord, struck again on the and of three.
        let mut kv = rolled(&k, &[(0.0, 2.3, 76), (2.5, 1.4, 64)]);
        if key == "outro" {
            kv = rolled(&k, &[(0.0, 3.8, 70)]);
        }
        s.clip(keys, &name, at, len, &repeat(&kv, k.beats, times));

        if key != "intro" {
            let bl = bass(
                &k,
                29,
                4.0,
                &[
                    (0.0, 1.4, 96, B::R),
                    (1.75, 0.5, 70, B::F),
                    (2.5, 0.9, 86, B::R),
                    (3.5, 0.45, 76, B::A),
                ],
            );
            let bl = if key == "outro" {
                bass(&k, 29, 4.0, &[(0.0, 3.8, 90, B::R)])
            } else {
                bl
            };
            s.clip(bass_p, &name, at, len, &repeat(&bl, k.beats, times));
            let d = beat(key, n);
            s.clip(kick, &name, at, len, &d.0);
            s.clip(snare, &name, at, len, &d.1);
            s.clip(hats, &name, at, len, &d.2);
        }
        if key == "verse" && at > 10.0 {
            let pv = hold(&lay(chart, 3, 53, 72, 62), 66, 0.99);
            s.clip(pad, &name, at, len, &repeat(&pv, 16.0, 2));
            let answer = melody(
                "r:2 D6:.5 C6:.5 A5:1 | r:2 E5:.5 G5:.5 Bb5:1 | r:2 A5:.5 G5:.5 E5:1 | r:2 F5:.5 E5:.5 D5:1",
                78,
                0.9,
            );
            s.clip(vibes, &name, at, len, &repeat(&answer, 16.0, 2));
        }
    }
    let tune = melody(
        "r:.5 D5:.5 F5:.5 A5:.5 >G5:1 F5:.5 D5:.5 | E5:1.5 D5:.25 C5:.25 A4:1 r:1 \
         | r:.5 C5:.5 E5:.5 G5:.5 >A5:1.5 G5:.5 | F5:1 E5:.5 D5:.5 C5:1 A4:1 \
         | r:.5 D5:.5 F5:.5 A5:.5 >Bb5:1 A5:.5 F5:.5 | G5:1 E5:.5 C5:.5 D5:1 Bb4:1 \
         | A4:.5 C5:.5 E5:.5 G5:.5 >A5:2 | r:1 F5:.5 E5:.5 D5:2",
        84,
        0.9,
    );
    // The tune over the A section's chords, then the hook's line over the B chords.
    s.clip(guitar, &section("verse"), 4.0, 8.0, &tune);
    let hook = melody(
        "D5:1 F5:.5 A5:1 G5:.5 F5:1 | E5:1.5 C5:.5 E5:1 G5:1 | F5:1 D5:.5 Bb4:1 A4:.5 G4:1 | C5:2 Bb4:1 G4:1 \
         | D5:1 F5:.5 A5:1 C6:.5 Bb5:1 | A5:1.5 G5:.5 E5:1 C5:1 | D5:1 F5:.5 A5:1 G5:.5 F5:1 | G5:4",
        86,
        0.92,
    );
    s.clip(guitar, &section("hook"), 12.0, 8.0, &hook);
    s.finish()
}

/// Chords struck with a roll: each voice a little after the one under it.
fn rolled(laid: &Laid, rhythm: &[(f64, f64, u8)]) -> Vec<Ev> {
    let mut out = Vec::new();
    let mut base = 0.0;
    while base < laid.beats - 1e-9 {
        for (at, len, vel) in rhythm {
            let t0 = base + at;
            for (i, p) in laid.at(t0).3.iter().enumerate() {
                let d = i as f64 * 0.035;
                out.push(Ev::new(
                    t0 + d,
                    len - d,
                    *p,
                    vel.saturating_sub(i as u8 * 3),
                ));
            }
        }
        base += 4.0;
    }
    out
}

/// Kick, snare and hats of a section.
fn beat(key: &str, n: usize) -> (Vec<Ev>, Vec<Ev>, Vec<Ev>) {
    let q = 0.25;
    let last = |i: usize| i == n - 1;
    let kick = bars(n, 4.0, |i| match key {
        "outro" if i >= 2 => {
            if i == 2 {
                drums(&[("X...............", KICK)], q)
            } else {
                vec![]
            }
        }
        _ if last(i) => drums(&[("X......x..x.....", KICK)], q),
        _ => drums(
            &[(
                if i % 2 == 0 {
                    "X.......x.x....."
                } else {
                    "X......xx......."
                },
                KICK,
            )],
            q,
        ),
    });
    let snare = bars(n, 4.0, |i| match key {
        "outro" if i >= 2 => vec![],
        _ if last(i) => drums(&[("....X..g....X.gx", SNARE)], q),
        "hook" => drums(&[("....X..g....X...", SNARE), ("..........o.....", RIM)], q),
        _ => drums(
            &[(
                if i % 2 == 0 {
                    "....X.......X..."
                } else {
                    "....X..g....X.g."
                },
                SNARE,
            )],
            q,
        ),
    });
    let hats = bars(n, 4.0, |i| match key {
        "outro" if i >= 2 => {
            if i == 2 {
                drums(&[("x.......", OPEN)], 0.5)
            } else {
                vec![]
            }
        }
        "hook" => drums(
            &[("x.o.x.o.x.o.x.o.", HAT), ("gogogogogogogogo", SHAKER)],
            q,
        ),
        _ => drums(
            &[
                (
                    if i % 4 == 3 {
                        "x.o.x.o.x.o.x..."
                    } else {
                        "x.o.x.o.x.o.x.oo"
                    },
                    HAT,
                ),
                (
                    if i % 4 == 3 {
                        "..............x."
                    } else {
                        "................"
                    },
                    OPEN,
                ),
            ],
            q,
        ),
    });
    (kick, snare, hats)
}
