//! **Midnight Static** — trap in F minor at 140 BPM, half-time.
//!
//! Gravis is the 808, long and driven, sliding between notes where they overlap; Unda's
//! digital bells play the dark loop; Modulus plucks the counter-line; Exemplar's pad is
//! chopped into eighths by a Porta gate that listens to the hats' notes. The trap kit:
//! a short kick under the 808, layered clap and snare on three, hats with sixteenth,
//! thirty-second and triplet rolls. Intro 4 · hook 16 · verse 16 · hook 16 · outro 4 =
//! 56 bars.

use super::parts::{bars, drums, hold, lay, repeat, swell};
use super::song::{self, colors, native, section, Feel, Song};
use super::theory::{melody, Ev};
use crate::i18n::t;
use cantus_core::{EffectKind as K, InstrumentKind as I, Project, TrackIcon};

const LOOP: &[(&str, f64)] = &[("Fm", 4.0), ("Db", 4.0), ("Ab", 4.0), ("Eb", 4.0)];

const FORM: [(&str, f64, f64); 5] = [
    ("intro", 0.0, 4.0),
    ("hook", 4.0, 16.0),
    ("verse", 20.0, 16.0),
    ("hook", 36.0, 16.0),
    ("outro", 52.0, 4.0),
];

const KICK: u8 = 36;
const RIM: u8 = 37;
const SNARE: u8 = 38;
const CLAP: u8 = 39;
const HAT: u8 = 42;
const OPEN: u8 = 46;
const CRASH: u8 = 49;

pub fn build() -> Project {
    let mut s = Song::new(&t("demos.song.trap"), 140.0, 0x7_2A9);
    let trap = song::kit("pulsus.kit.trap");
    let kick = s.part(
        &t("demos.track.kick"),
        song::pads_with(&trap, &[KICK], |p| {
            // A short, punchy kick: the 808 track holds the low end.
            p.decay = 0.35;
            p.drive = 0.2;
        }),
        colors::KICK,
        TrackIcon::Drums,
    );
    let clap = s.part(
        &t("demos.track.clap"),
        song::pads(&trap, &[SNARE, CLAP]),
        colors::SNARE,
        TrackIcon::Drums,
    );
    let hats = s.part(
        &t("demos.track.hats"),
        song::pads_with(&trap, &[HAT, OPEN], |p| {
            p.level_db = (p.level_db + 9.0).min(12.0)
        }),
        colors::HATS,
        TrackIcon::Drums,
    );
    let perc = s.part(
        &t("demos.track.perc"),
        song::pads(&trap, &[RIM, CRASH]),
        colors::PERC,
        TrackIcon::Drums,
    );
    let sub = s.part(
        &t("demos.track.sub"),
        native(I::Bass, "bass.preset.trap_long", &[("glide", 70.0)]),
        colors::BASS,
        TrackIcon::Synth,
    );
    let bells = s.part(
        &t("demos.track.bells"),
        native(I::Wavetable, "unda.preset.digital_bells", &[]),
        colors::BELLS,
        TrackIcon::Keys,
    );
    let pluck = s.part(
        &t("demos.track.pluck"),
        native(I::Fm, "fm.preset.pluck", &[]),
        colors::PLUCK,
        TrackIcon::Synth,
    );
    let pad = s.part(
        &t("demos.track.pad"),
        native(I::Sampler, "smp.preset.pad", &[]),
        colors::PAD,
        TrackIcon::Synth,
    );
    for p in [kick, clap, perc] {
        s.feel(
            p,
            Feel {
                jitter: 0.0,
                vel: 4.0,
                ..Feel::HUMAN
            },
        );
    }
    s.feel(
        hats,
        Feel {
            jitter: 0.004,
            vel: 8.0,
            ..Feel::HUMAN
        },
    );
    s.feel(sub, Feel::TIGHT);

    let bus = s.bus(
        &t("demos.strip.drum_bus"),
        colors::BUS,
        &[kick, clap, hats, perc],
    );
    let verb = s.ret(&t("demos.strip.reverb"));
    let delay = s.ret(&t("demos.strip.delay"));
    s.insert(kick.strip, K::Eq, Some("eq.preset.kick"), &[]);
    s.insert(
        clap.strip,
        K::Compressor,
        None,
        &[
            ("thr", -20.0),
            ("ratio", 4.0),
            ("att", 3.0),
            ("rel", 90.0),
            ("auto_gain", 1.0),
        ],
    );
    s.insert(
        hats.strip,
        K::Eq,
        None,
        &[
            ("b0.used", 1.0),
            ("b0.shape", 3.0),
            ("b0.freq", 400.0),
            ("b0.slope", 24.0),
        ],
    );
    s.insert(
        hats.strip,
        K::DeEsser,
        None,
        &[
            ("thr", -30.0),
            ("range", 4.0),
            ("lo", 8_000.0),
            ("hi", 16_000.0),
        ],
    );
    s.insert(bus, K::Limiter, Some("lim.preset.drums"), &[("gain", 2.0)]);
    s.insert(
        sub.strip,
        K::Eq,
        Some("eq.preset.mono_bass"),
        &[("mode", 0.0), ("b0.freq", 28.0)],
    );
    s.insert(
        sub.strip,
        K::Multiband,
        Some("mb.preset.low_end"),
        &[("mode", 0.0)],
    );
    s.insert(
        sub.strip,
        K::Limiter,
        None,
        &[("ceiling", -6.0), ("clip", 0.5), ("rel", 80.0)],
    );
    s.insert(
        bells.strip,
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
        pad.strip,
        K::Eq,
        None,
        &[
            ("b0.used", 1.0),
            ("b0.shape", 3.0),
            ("b0.freq", 200.0),
            ("b0.slope", 18.0),
            ("b1.used", 1.0),
            ("b1.shape", 4.0),
            ("b1.freq", 6_000.0),
            ("b1.slope", 12.0),
        ],
    );
    // The chop: a gate on the pad that opens on every note of the hats.
    let chop = s.insert(
        pad.strip,
        K::Gate,
        None,
        &[
            ("trig", 1.0),
            ("vel", 12.0),
            ("hold", 60.0),
            ("rel", 90.0),
            ("range", 30.0),
        ],
    );
    s.midi(pad.strip, chop, hats);
    s.insert(
        verb,
        K::Space,
        Some("space.preset.concert_hall"),
        &[("mix", 1.0), ("decay", 2.6), ("bright", 0.35)],
    );
    s.insert(
        delay,
        K::Delay,
        None,
        &[
            ("time_ms", 321.0),
            ("feedback", 0.4),
            ("tone_hz", 3_500.0),
            ("mix", 1.0),
        ],
    );
    for (p, level) in [(clap, -14.0), (bells, -8.0), (pluck, -10.0), (pad, -12.0)] {
        s.send(p.strip, verb, level);
    }
    s.send(bells.strip, delay, -12.0);
    s.send(pluck.strip, delay, -10.0);
    s.master(
        K::Eq,
        None,
        &[
            ("b0.used", 1.0),
            ("b0.shape", 3.0),
            ("b0.freq", 20.0),
            ("b0.slope", 18.0),
        ],
    );
    s.master(K::Multiband, Some("mb.preset.glue"), &[]);
    // Mastering (ADR 0026): Latitudo puts the low end in mono, then Culmen's streaming
    // master — three bands, the low one held, a gentle full-band squeeze, into a −1 dBTP
    // ceiling — driven so the song lands near −14 LUFS.
    s.master(K::Stereo, Some("preset.st.master_low"), &[]);
    s.master(K::Maximizer, Some("preset.max.streaming"), &[("gain", 7.5)]);

    s.trim(kick, 6.0);
    s.mix(kick.strip, -4.5, 0.0);
    s.trim(clap, 8.0);
    s.mix(clap.strip, 0.0, 0.0);
    s.trim(hats, 12.0);
    s.mix(hats.strip, 2.0, 0.15);
    s.trim(perc, 12.0);
    s.mix(perc.strip, 0.0, -0.3);
    s.mix(bus, 0.0, 0.0);
    s.trim(sub, 3.0);
    s.mix(sub.strip, -2.0, 0.0);
    s.trim(bells, 4.0);
    s.mix(bells.strip, -5.0, -0.1);
    s.trim(pluck, 0.0);
    s.mix(pluck.strip, -5.0, 0.25);
    s.trim(pad, 0.0);
    s.mix(pad.strip, -2.0, 0.0);
    s.mix(verb, -6.0, 0.0);
    s.mix(delay, -10.0, 0.0);

    let bell_loop = melody(
        "C6:.75 Ab5:.75 F5:.5 C6:.75 Ab5:.75 G5:.5 | Ab5:.75 F5:.75 Db5:.5 Ab5:.75 F5:.75 Eb5:.5 \
         | Eb6:.75 C6:.75 Ab5:.5 Eb6:.75 C6:.75 Bb5:.5 | G5:.75 Eb5:.75 Bb4:.5 G5:.75 Bb5:.75 C6:.5",
        86,
        0.9,
    );
    let counter = melody(
        "F5:2 Eb5:1 C5:1 | Db5:3 C5:1 | Eb5:2 C5:1 Ab4:1 | Bb4:4",
        92,
        0.95,
    );
    for (key, at, len) in FORM {
        let name = section(key);
        let n = len as usize;
        let times = n / 4;
        let chart = lay(LOOP, 3, 53, 72, 62);
        match key {
            "intro" => {
                s.clip(bells, &name, at, len, &swell(&bell_loop, 16.0, 0.6, 0.95));
                s.clip(pad, &name, at, len, &hold(&chart, 70, 0.98));
                s.clip(
                    hats,
                    &name,
                    at,
                    len,
                    &bars(n, 4.0, |i| if i >= 2 { hat_bar(i, false) } else { vec![] }),
                );
            }
            "outro" => {
                s.clip(bells, &name, at, len, &swell(&bell_loop, 16.0, 0.9, 0.5));
                s.clip(sub, &name, at, len, &[Ev::new(0.0, 6.0, 29, 110)]);
                s.clip(
                    kick,
                    &name,
                    at,
                    len,
                    &drums(&[("X...............", KICK)], 0.25),
                );
                s.clip(
                    perc,
                    &name,
                    at,
                    len,
                    &drums(&[("X...............", CRASH)], 0.25),
                );
            }
            _ => {
                let hook = key == "hook";
                s.clip(
                    bells,
                    &name,
                    at,
                    len,
                    &repeat(&bell_loop, 16.0, if hook { times } else { 2 }),
                );
                s.clip(
                    sub,
                    &name,
                    at,
                    len,
                    &repeat(&eight_o_eight(hook), 16.0, times),
                );
                s.clip(kick, &name, at, len, &bars(n, 4.0, |i| kick_bar(i, hook)));
                s.clip(clap, &name, at, len, &bars(n, 4.0, |i| clap_bar(i, n)));
                s.clip(hats, &name, at, len, &bars(n, 4.0, |i| hat_bar(i, hook)));
                s.clip(
                    perc,
                    &name,
                    at,
                    len,
                    &bars(n, 4.0, |i| {
                        let mut v = drums(
                            &[(
                                if i % 2 == 1 {
                                    "......o.......o."
                                } else {
                                    "..........o....."
                                },
                                RIM,
                            )],
                            0.25,
                        );
                        if i % 8 == 0 {
                            v.extend(drums(&[("X...............", CRASH)], 0.25));
                        }
                        v
                    }),
                );
                if hook {
                    s.clip(
                        pad,
                        &name,
                        at,
                        len,
                        &repeat(&hold(&chart, 76, 0.98), 16.0, times),
                    );
                    let c = repeat(&counter, 16.0, times / 2);
                    s.clip(pluck, &name, at + 8.0, len - 8.0, &c);
                } else {
                    // The verse: the pluck takes the loop an octave down, sparser.
                    let low = super::theory::transpose(&counter, -12);
                    s.clip(pluck, &name, at, len, &repeat(&low, 16.0, times));
                }
            }
        }
    }
    s.finish()
}

/// Four bars of the 808: the chord's root with the kick, a pickup, and a slide — the last
/// note of a bar overlaps the next one's, so the bass glides into it.
fn eight_o_eight(hook: bool) -> Vec<Ev> {
    // F1, Db1, Ab1, Eb1.
    let roots = [29u8, 25, 32, 27];
    let mut v = Vec::new();
    for (i, r) in roots.iter().enumerate() {
        let b = i as f64 * 4.0;

        v.push(Ev::new(b, 1.4, *r, 112));
        v.push(Ev::new(b + 1.75, 0.6, *r, 96));
        if hook || i % 2 == 1 {
            v.push(Ev::new(b + 2.5, 0.9, *r, 104));
            // The slide: an octave up, held into the next bar's root.
            v.push(Ev::new(b + 3.25, 0.95, r + 12, 100));
        } else {
            v.push(Ev::new(b + 2.5, 1.4, *r, 104));
        }
    }
    v
}

fn kick_bar(i: usize, hook: bool) -> Vec<Ev> {
    let g = match (i % 4, hook) {
        (3, _) => "X......x..x...x.",
        (1, true) => "X.........x..x..",
        _ => "X.........x.....",
    };
    drums(&[(g, KICK)], 0.25)
}

fn clap_bar(i: usize, n: usize) -> Vec<Ev> {
    if i == n - 1 {
        drums(
            &[("........X.....xX", CLAP), ("........x.....x.", SNARE)],
            0.25,
        )
    } else {
        drums(
            &[("........X.......", CLAP), ("........x.......", SNARE)],
            0.25,
        )
    }
}

/// A bar of hats: eighths, with rolls — sixteenths, thirty-seconds, triplets — that change
/// from bar to bar, and an open hat to end a phrase.
fn hat_bar(i: usize, hook: bool) -> Vec<Ev> {
    let mut v = Vec::new();
    let mut hit = |at: f64, vel: u8| v.push(Ev::new(at, 0.1, HAT, vel));
    let roll_at = match i % 4 {
        1 => Some((3.0, 0.125, 8)),           // a 32nd roll into the bar's end
        2 => Some((1.5, 1.0 / 6.0, 3)),       // a triplet
        3 if hook => Some((2.0, 0.0625, 16)), // a 64th buzz
        3 => Some((3.0, 1.0 / 6.0, 6)),
        _ => None,
    };
    let mut t = 0.0;
    while t < 4.0 - 1e-9 {
        let in_roll = roll_at.is_some_and(|(s, step, count)| t >= s && t < s + step * count as f64);
        if !in_roll {
            hit(t, if (t * 2.0) as i64 % 2 == 0 { 96 } else { 72 });
        }
        t += 0.5;
    }
    if let Some((s, step, count)) = roll_at {
        for k in 0..count {
            let vel = 60 + (k * 40 / count.max(1)) as u8;
            hit(s + k as f64 * step, vel);
        }
    }
    if i % 4 == 3 {
        v.push(Ev::new(3.5, 0.5, OPEN, 84));
        v.retain(|e| !(e.pitch == HAT && e.at >= 3.5));
    }
    v
}
