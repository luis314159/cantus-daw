//! **Aurora** — a cinematic piece in D minor at 66 BPM.
//!
//! Unda's glass choir is the air it starts in; Modulus rings the bells of the theme;
//! Calor's string machine and its glass pluck ostinato build the rise; Exemplar's choir
//! and Pulsus II's toms and low drums carry the climax; Gravis holds a sine sub under
//! everything. A cathedral on the reverb return. Swell 8 · rise 8 · climax 8 · outro 8 =
//! 32 bars, about two minutes.

use super::parts::{arp, bars, bass, drums, hold, lay, swell, Chart, B};
use super::song::{self, colors, native, section, Feel, Song};
use super::theory::{melody, Ev};
use crate::i18n::t;
use cantus_core::{EffectKind as K, InstrumentKind as I, Project, TrackIcon};

const SWELL: &[(&str, f64)] = &[("Dmadd9", 8.0), ("Bbmaj7", 8.0), ("Fadd9", 8.0), ("C", 8.0)];
const RISE: &[(&str, f64)] = &[
    ("Dm", 4.0),
    ("Bb", 4.0),
    ("Gm7", 4.0),
    ("A7sus4", 2.0),
    ("A", 2.0),
    ("Dm", 4.0),
    ("Bb", 4.0),
    ("Gm", 4.0),
    ("Asus4", 4.0),
];
const CLIMAX: &[(&str, f64)] = &[
    ("Bb", 4.0),
    ("F/A", 4.0),
    ("Gm7", 4.0),
    ("Dm", 4.0),
    ("Bb", 4.0),
    ("F/A", 4.0),
    ("Gm7", 4.0),
    ("Asus4", 2.0),
    ("A", 2.0),
];
const OUTRO: &[(&str, f64)] = &[("Bbmaj7", 8.0), ("Fadd9", 8.0), ("Dmadd9", 16.0)];

const FORM: [(&str, f64, Chart<'static>); 4] = [
    ("swell", 0.0, SWELL),
    ("rise", 8.0, RISE),
    ("chorus", 16.0, CLIMAX),
    ("outro", 24.0, OUTRO),
];

const KICK: u8 = 36;
const LOW_TOM: u8 = 45;
const MID_TOM: u8 = 47;
const CRASH: u8 = 49;

pub fn build() -> Project {
    let mut s = Song::new(&t("demos.song.ambient"), 66.0, 0xA0_0A);
    let studio = song::kit("pulsus.kit.studio");
    let pad = s.part(
        &t("demos.track.pad"),
        native(I::Wavetable, "unda.preset.glass_choir", &[]),
        colors::PAD,
        TrackIcon::Synth,
    );
    let strings = s.part(
        &t("demos.track.strings"),
        native(I::Analog, "calor.preset.string_machine", &[]),
        colors::STRINGS,
        TrackIcon::Strings,
    );
    let arp_p = s.part(
        &t("demos.track.arp"),
        native(I::Analog, "calor.preset.glass_pluck", &[]),
        colors::PLUCK,
        TrackIcon::Synth,
    );
    let bells = s.part(
        &t("demos.track.bells"),
        native(I::Fm, "fm.preset.bell", &[]),
        colors::BELLS,
        TrackIcon::Keys,
    );
    let choir = s.part(
        &t("demos.track.choir"),
        native(I::Sampler, "smp.preset.choir", &[]),
        colors::LEAD,
        TrackIcon::Vocal,
    );
    let sub = s.part(
        &t("demos.track.bass"),
        native(I::Bass, "bass.preset.sub_sine", &[]),
        colors::BASS,
        TrackIcon::Synth,
    );
    let drums_p = s.part(
        &t("demos.track.drums"),
        song::pads_with(&studio, &[KICK, LOW_TOM, MID_TOM, CRASH], |p| {
            // Bigger, lower drums than a kit's: the toms tuned down and left to ring.
            if p.pitch == LOW_TOM || p.pitch == MID_TOM {
                p.tune -= 5.0;
                p.decay *= 1.6;
            }
            if p.pitch == CRASH {
                p.decay *= 1.5;
            }
        }),
        colors::PERC,
        TrackIcon::Drums,
    );
    s.feel(
        drums_p,
        Feel {
            jitter: 0.01,
            vel: 8.0,
            ..Feel::HUMAN
        },
    );

    let verb = s.ret(&t("demos.strip.reverb"));
    let delay = s.ret(&t("demos.strip.delay"));
    s.insert(
        pad.strip,
        K::Eq,
        None,
        &[
            ("b0.used", 1.0),
            ("b0.shape", 3.0),
            ("b0.freq", 160.0),
            ("b0.slope", 24.0),
        ],
    );
    s.insert(
        strings.strip,
        K::Eq,
        None,
        &[
            ("b0.used", 1.0),
            ("b0.shape", 3.0),
            ("b0.freq", 140.0),
            ("b0.slope", 12.0),
            ("b1.used", 1.0),
            ("b1.freq", 2_800.0),
            ("b1.gain", -2.0),
            ("b1.q", 0.8),
        ],
    );
    s.insert(strings.strip, K::Multiband, Some("mb.preset.glue"), &[]);
    s.insert(
        choir.strip,
        K::DeEsser,
        None,
        &[("thr", -36.0), ("range", 6.0)],
    );
    s.insert(
        choir.strip,
        K::Eq,
        None,
        &[
            ("b0.used", 1.0),
            ("b0.shape", 3.0),
            ("b0.freq", 200.0),
            ("b0.slope", 18.0),
        ],
    );
    s.insert(
        sub.strip,
        K::Eq,
        None,
        &[
            ("b0.used", 1.0),
            ("b0.shape", 4.0),
            ("b0.freq", 200.0),
            ("b0.slope", 24.0),
        ],
    );
    s.insert(
        drums_p.strip,
        K::Compressor,
        Some("comp.preset.drum_bus"),
        &[("thr", -18.0)],
    );
    s.insert(
        drums_p.strip,
        K::Gate,
        Some("preset.gate.toms"),
        &[("thr", -46.0), ("rel", 600.0), ("hold", 120.0)],
    );
    s.insert(
        drums_p.strip,
        K::Limiter,
        Some("lim.preset.drums"),
        &[("gain", 0.0), ("ceiling", -4.0)],
    );
    s.insert(
        verb,
        K::Space,
        Some("space.preset.cathedral"),
        &[("mix", 1.0), ("decay", 6.0)],
    );
    s.insert(
        delay,
        K::Delay,
        None,
        &[
            ("time_ms", 682.0),
            ("feedback", 0.5),
            ("tone_hz", 3_000.0),
            ("mix", 1.0),
        ],
    );
    for (p, level) in [
        (pad, -6.0),
        (strings, -8.0),
        (arp_p, -10.0),
        (bells, -4.0),
        (choir, -6.0),
        (drums_p, -10.0),
    ] {
        s.send(p.strip, verb, level);
    }
    s.send(bells.strip, delay, -8.0);
    s.send(arp_p.strip, delay, -14.0);
    s.master(
        K::Eq,
        None,
        &[
            ("b0.used", 1.0),
            ("b0.shape", 3.0),
            ("b0.freq", 22.0),
            ("b0.slope", 18.0),
        ],
    );
    s.master(K::Multiband, Some("mb.preset.glue"), &[]);
    // Mastering (ADR 0026): Latitudo puts the low end in mono, then Culmen's streaming
    // master — three bands, the low one held, a gentle full-band squeeze, into a −1 dBTP
    // ceiling — driven so the song lands near −14 LUFS.
    s.master(K::Stereo, Some("preset.st.master_air"), &[]);
    s.master(K::Maximizer, Some("preset.max.streaming"), &[("gain", 7.0)]);

    s.trim(pad, 3.0);
    s.mix(pad.strip, -2.0, 0.0);
    s.trim(strings, 0.0);
    s.mix(strings.strip, -3.4, -0.2);
    s.trim(arp_p, 10.0);
    s.mix(arp_p.strip, -5.0, 0.25);
    s.trim(bells, 6.0);
    s.mix(bells.strip, -3.0, 0.1);
    s.trim(choir, -3.0);
    s.mix(choir.strip, -5.0, 0.0);
    s.trim(sub, -4.0);
    s.mix(sub.strip, -5.0, 0.0);
    s.trim(drums_p, 6.0);
    s.mix(drums_p.strip, 2.0, 0.0);
    s.mix(verb, -4.0, 0.0);
    s.mix(delay, -8.0, 0.0);

    for (key, at, chart) in FORM {
        let name = section(key);
        let len = 8.0;
        let wide = lay(chart, 4, 50, 76, 62);
        let high = lay(chart, 3, 60, 81, 70);
        let climax = key == "chorus";

        let mut pv = hold(&wide, if climax { 84 } else { 72 }, 1.0);
        if key == "swell" {
            pv = swell(&pv, 32.0, 0.55, 1.0);
        }
        s.clip(pad, &name, at, len, &pv);

        if key != "swell" {
            let sv = hold(&high, if climax { 96 } else { 74 }, 0.98);
            let sv = if key == "rise" {
                swell(&sv, 32.0, 0.75, 1.15)
            } else {
                sv
            };
            s.clip(strings, &name, at, len, &sv);
        }
        if key == "rise" || climax {
            let step = if climax { 0.25 } else { 0.5 };
            let a = arp(
                &lay(chart, 3, 62, 81, 70),
                50,
                &[0, 1, 2, 3, 1, 2, 3, 2],
                step,
                step * 0.9,
                if climax { 84 } else { 72 },
            );
            s.clip(arp_p, &name, at, len, &a);
        }
        if key == "swell" {
            s.clip(
                sub,
                &name,
                at + 4.0,
                4.0,
                &bass(
                    &lay(&SWELL[2..], 3, 50, 76, 62),
                    26,
                    8.0,
                    &[(0.0, 7.8, 90, B::R)],
                ),
            );
        } else {
            s.clip(
                sub,
                &name,
                at,
                len,
                &bass(
                    &wide,
                    26,
                    4.0,
                    &[(0.0, 3.9, if climax { 100 } else { 88 }, B::R)],
                ),
            );
        }
        if climax || key == "outro" {
            let c = hold(
                &lay(chart, 3, 57, 76, 66),
                if climax { 100 } else { 80 },
                0.99,
            );
            s.clip(choir, &name, at, len, &c);
        }
        let d = drums_of(key);
        if !d.is_empty() {
            s.clip(drums_p, &name, at, len, &d);
        }
    }

    let intro_bells = melody(
        "r:4 A5:4 | r:4 D6:2 C6:2 | r:4 C6:4 | r:4 G5:2 E5:2",
        74,
        1.0,
    );
    s.clip(bells, &section("swell"), 0.0, 8.0, &intro_bells);
    let theme = melody(
        ">D6:1.5 C6:.5 Bb5:1 A5:1 | A5:3 F5:1 | G5:1.5 A5:.5 Bb5:1 D6:1 | A5:4 \
         | >D6:1.5 C6:.5 Bb5:1 F6:1 | E6:1.5 C6:.5 A5:2 | D6:1 Bb5:1 G5:1 Bb5:1 | A5:2 C#6:2",
        96,
        1.0,
    );
    s.clip(bells, &section("chorus"), 16.0, 8.0, &theme);
    let last = melody("r:4 F6:4 | r:4 G6:2 C6:2 | r:4 A5:4 | D6:8", 70, 1.0);
    s.clip(bells, &section("outro"), 24.0, 8.0, &last);
    s.finish()
}

/// The drums: nothing in the swell, a heartbeat on the low tom through the rise and a
/// roll into the climax, a pounding ensemble in the climax, one last hit.
fn drums_of(key: &str) -> Vec<Ev> {
    let q = 0.25;
    match key {
        "rise" => swell(
            &bars(8, 4.0, |i| match i {
                7 => drums(
                    &[("xxxxxxxxxxxxxxxx", LOW_TOM), ("........xxxxxxxx", MID_TOM)],
                    q,
                ),
                6 => drums(&[("x.......x...x.x.", LOW_TOM)], q),
                _ => drums(&[("x.......x.......", LOW_TOM)], q),
            }),
            32.0,
            0.45,
            1.05,
        ),
        "chorus" => bars(8, 4.0, |i| {
            let mut v = drums(
                &[
                    ("X.......X.......", KICK),
                    ("X..x..x.X...x.x.", LOW_TOM),
                    ("...g...g.g.....g", MID_TOM),
                ],
                q,
            );
            if i % 4 == 0 {
                v.extend(drums(&[("X...............", CRASH)], q));
            }
            if i == 7 {
                v = drums(&[("X.......", KICK), ("x.x.xxxxxxxxxxxx", LOW_TOM)], q);
            }
            v
        }),
        "outro" => drums(
            &[
                ("X...............", KICK),
                ("X...............", LOW_TOM),
                ("X...............", CRASH),
            ],
            q,
        ),
        _ => vec![],
    }
}
