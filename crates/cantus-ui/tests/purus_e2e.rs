//! Purus end to end (fx side round, wave C, ADR 0142), on the boot path: an app
//! made by `CantusApp::assemble` with the real engine handle; five recordings imported as a
//! user imports them (`import_audio`, each onto its own track): music under a 50 Hz mains
//! hum, the same music with clicks, the music clean, a voice in a reverberant room, and the
//! hummed take again on a parallel strip with nothing on it. Purus is inserted on three
//! strips as the picker inserts it — on its starting preset, Mains hum 50 Hz, followed:
//!
//! - **the hum comes out**: each of the eight harmonics on the master down by tens of
//!   decibels (the DSP report's notches: 37–43 dB);
//! - **the clicks are repaired**: the error against the clean music, through the de-click,
//!   down against the same input through it at mix 0 (aligned to the sample);
//! - **a reverberant voice's tail is reduced**: the room's tail in the pauses down against
//!   the same voice through the room module at mix 0;
//! - **Hear what is removed is heard**: the hum module's delta is the hum, the music gone;
//! - **the latency is compensated against a parallel dry strip, to the sample**: every
//!   module on (87 ms) at mix 0, the hummed strip's master is the dry strip's, to the bit;
//! - an edit heard and undone to the bit; live in odd blocks equal to the offline render;
//!   and at its defaults (every module off), the dry master to the bit.
//!
//! Its own test binary: it points `CANTUS_DATA_DIR` at a temporary folder. // fxc:purus

use cantus_core::*;
use cantus_engine::{render, EffectUnit, EngineHandle, Processor, RenderOptions};
use cantus_ui::app::Files;
use cantus_ui::CantusApp;
use std::f64::consts::TAU;
use std::path::{Path, PathBuf};

const SR: usize = 48_000;
/// The takes' length: 4.3 s (no whole bar at 120 BPM: never a loop the import warps).
const FRAMES: usize = 206_400;

fn data() -> &'static Path {
    static DIR: std::sync::OnceLock<tempfile::TempDir> = std::sync::OnceLock::new();
    DIR.get_or_init(|| {
        let d = tempfile::tempdir().unwrap();
        std::env::set_var("CANTUS_DATA_DIR", d.path());
        d
    })
    .path()
}

fn assembled() -> (CantusApp, Processor) {
    let files = Files::read(data(), Some(cantus_ui::i18n::Locale::En));
    let ctx = egui::Context::default();
    let (engine, proc_) = EngineHandle::manual(48_000.0);
    let app = CantusApp::assemble(&ctx, files, None, engine, None, None, None, false);
    (app, proc_)
}

/// A stereo float WAV of `left`/`right` in the data folder.
fn wav(name: &str, l: &[f32], r: &[f32]) -> PathBuf {
    let path = data().join(name);
    let spec = hound::WavSpec {
        channels: 2,
        sample_rate: SR as u32,
        bits_per_sample: 32,
        sample_format: hound::SampleFormat::Float,
    };
    let mut w = hound::WavWriter::create(&path, spec).unwrap();
    for (a, b) in l.iter().zip(r) {
        w.write_sample(*a).unwrap();
        w.write_sample(*b).unwrap();
    }
    w.finalize().unwrap();
    path
}

/// A number in −1 … 1 from a running seed: the same every run.
fn noise(seed: &mut u32) -> f32 {
    *seed ^= *seed << 13;
    *seed ^= *seed >> 17;
    *seed ^= *seed << 5;
    (*seed as f32 / u32::MAX as f32) * 2.0 - 1.0
}

/// Music clear of the hum's harmonics: three plucked tones (440, 659 and 1 318 Hz) every
/// half second, each its own decay, each pluck's attack soft (no click of its own).
fn music() -> Vec<f32> {
    (0..FRAMES)
        .map(|i| {
            let t = i as f64 / SR as f64;
            let since = t % 0.5;
            // A soft attack (4 ms): a pluck, not a click.
            let env = (1.0 - (-since / 0.004).exp()) * (-since / 0.25).exp();
            [(440.0, 0.10), (659.26, 0.07), (1_318.5, 0.04)]
                .iter()
                .map(|(f, a)| a * env * (TAU * f * t).sin())
                .sum::<f64>() as f32
        })
        .collect()
}

/// The mains' hum: 50 Hz and its first eight harmonics, the fundamental at −34 dBFS.
fn hum() -> Vec<f32> {
    (0..FRAMES)
        .map(|i| {
            let t = i as f64 / SR as f64;
            (1..=8)
                .map(|k| {
                    let a = 0.02 / (k as f64).sqrt();
                    a * (TAU * 50.0 * k as f64 * t + 0.7 * k as f64).sin()
                })
                .sum::<f64>() as f32
        })
        .collect()
}

/// Twenty-four clicks of three samples, 0.3 … 0.5 at their peak.
fn clicks() -> Vec<f32> {
    let mut x = vec![0.0f32; FRAMES];
    let mut seed = 0xC1_1C_4Bu32;
    for c in 0..24 {
        let at = 6_000 + c * 8_111;
        let a = 0.3 + 0.2 * noise(&mut seed).abs();
        let sgn = if noise(&mut seed) > 0.0 { 1.0 } else { -1.0 };
        for (j, w) in [1.0f32, -0.6, 0.3].iter().enumerate() {
            x[at + j] += sgn * a * w;
        }
    }
    x
}

/// A voice in a reverberant room (DRR about 0 dB, T60 about 0.8 s): syllables of shaped
/// noise 0.18 s long every 0.75 s, through four combs and two allpasses.
fn room_voice() -> Vec<f32> {
    let mut seed = 0x5EED_0F11u32;
    let mut dry = vec![0.0f32; FRAMES];
    let (mut lp, mut bp) = (0.0f32, 0.0f32);
    for (i, v) in dry.iter_mut().enumerate() {
        let t = i as f32 / SR as f32;
        let since = t % 0.75;
        let env = if since < 0.18 {
            (std::f32::consts::PI * since / 0.18).sin()
        } else {
            0.0
        };
        // Noise through a resonance near 700 Hz and a gentle low-pass: a vowel-like hiss.
        let n = noise(&mut seed);
        lp += 0.25 * (n - lp);
        bp = 0.9 * bp + 0.1 * lp;
        *v = 0.6 * env * (lp - bp);
    }
    let mut wet = vec![0.0f32; FRAMES];
    for d in [1_557usize, 1_617, 1_491, 1_422] {
        let g = 10f32.powf(-3.0 * d as f32 / (0.8 * SR as f32));
        let mut line = vec![0.0f32; d];
        let mut p = 0;
        for i in 0..FRAMES {
            let y = line[p];
            line[p] = dry[i] + g * y;
            p = (p + 1) % d;
            wet[i] += y * 0.25;
        }
    }
    for d in [225usize, 556] {
        let mut line = vec![0.0f32; d];
        let mut p = 0;
        for v in wet.iter_mut() {
            let b = line[p];
            let y = -0.5 * *v + b;
            line[p] = *v + 0.5 * y;
            p = (p + 1) % d;
            *v = y;
        }
    }
    dry.iter().zip(&wet).map(|(a, b)| a + b).collect()
}

fn sum(a: &[f32], b: &[f32]) -> Vec<f32> {
    a.iter().zip(b).map(|(x, y)| x + y).collect()
}

/// Eight beats and a half at 120 BPM (4.25 s), interleaved stereo, latency kept.
fn rendered(app: &mut CantusApp) -> Vec<f32> {
    let p = app.project().clone();
    let snap = app.engine.compile_for_render(&p, 1);
    let opts = RenderOptions {
        end: Some(Ticks::from_beats_f64(8.5)),
        tail_seconds: 0.0,
        ..Default::default()
    };
    render(snap, &opts).unwrap().samples
}

fn left(x: &[f32]) -> Vec<f32> {
    x.iter().step_by(2).copied().collect()
}

fn db(p: f64) -> f64 {
    10.0 * p.max(1e-30).log10()
}

/// The level of `x` (one channel) at `hz` over `a … b` seconds, dBFS of a sine's peak — a
/// Hann-windowed Goertzel, so a tone a few tens of hertz away does not leak in.
fn tone_db(x: &[f32], hz: f64, a: f64, b: f64) -> f64 {
    let (i, j) = ((a * SR as f64) as usize, (b * SR as f64) as usize);
    let n = (j - i) as f64;
    let c = 2.0 * (TAU * hz / SR as f64).cos();
    let (mut s1, mut s2) = (0.0f64, 0.0f64);
    for (k, v) in x[i..j].iter().enumerate() {
        let w = 0.5 - 0.5 * (TAU * k as f64 / n).cos();
        let s0 = f64::from(*v) * w + c * s1 - s2;
        s2 = s1;
        s1 = s0;
    }
    let p = s1 * s1 + s2 * s2 - c * s1 * s2;
    // A sine of amplitude A reads (A·n/4)² through the Hann window.
    db(p.max(0.0) / (n * n / 16.0))
}

/// Mean square of `x` over the sample ranges given.
fn power_in(x: &[f32], spans: &[(usize, usize)]) -> f64 {
    let (mut s, mut n) = (0.0f64, 0usize);
    for &(a, b) in spans {
        for v in &x[a..b.min(x.len())] {
            s += f64::from(*v) * f64::from(*v);
            n += 1;
        }
    }
    s / n.max(1) as f64
}

fn diff(a: &[f32], b: &[f32]) -> Vec<f32> {
    a.iter().zip(b).map(|(x, y)| x - y).collect()
}

#[test]
fn purus_takes_out_hum_clicks_and_a_room_on_the_boot_path() {
    let (mut app, mut proc_) = assembled();
    let mut p = Project::empty("Purus");
    p.tempo_bpm = 120.0;
    app.replace_project(p);
    let (m, h, k) = (music(), hum(), clicks());
    let hummed = sum(&m, &h);
    let clicked = sum(&m, &k);
    let voice = room_voice();
    let files = [
        wav("hummed.wav", &hummed, &hummed),
        wav("clicked.wav", &clicked, &clicked),
        wav("clean.wav", &m, &m),
        wav("room.wav", &voice, &voice),
        wav("hummed-dry.wav", &hummed, &hummed),
    ];
    assert_eq!(app.import_audio(&files, Ticks::ZERO, None), 5, "imported");
    let tracks: Vec<TrackId> = app.project().tracks.iter().map(|t| t.id).collect();
    assert_eq!(tracks.len(), 5, "a track each");
    // Each track routed to a strip of its own, as a user routes them on the desk.
    app.add_strips_up_to(5);
    let desk: Vec<StripId> = app.project().strips.iter().map(|s| s.id).collect();
    for (t, s) in tracks.iter().zip(&desk) {
        if app.project().track(*t).unwrap().strip.is_none() {
            app.route_track(*t, Some(*s));
        }
    }
    let strips: Vec<StripId> = app
        .project()
        .tracks
        .iter()
        .map(|t| t.strip.expect("a strip"))
        .collect();
    let (hum_t, click_t, clean_t, room_t, dry_t) =
        (tracks[0], tracks[1], tracks[2], tracks[3], tracks[4]);
    let (hum_s, click_s, room_s) = (strips[0], strips[1], strips[3]);
    // Only `on` sounds.
    let solo = |app: &mut CantusApp, on: TrackId| {
        let commands = tracks
            .iter()
            .map(|t| Command::SetTrackMute {
                id: *t,
                muted: *t != on,
            })
            .collect();
        app.exec(Command::Batch {
            label_key: "cmd.fx_preset".into(),
            commands,
        });
    };
    // Every track as it plays with no Purus: the dry master of each.
    solo(&mut app, hum_t);
    let dry_hum = rendered(&mut app);
    assert!(
        tone_db(&left(&dry_hum), 50.0, 1.5, 4.0) > -40.0,
        "the hum is in the take"
    );
    solo(&mut app, clean_t);
    let clean = left(&rendered(&mut app));

    // Inserted as the picker inserts it: Mains hum 50 Hz, followed, no latency.
    let insert = |app: &mut CantusApp, strip: StripId| -> EffectId {
        app.add_effect(strip, EffectKind::Repair);
        app.project().strip(strip).unwrap().effects[0].id
    };
    let hum_fx = insert(&mut app, hum_s);
    let spec = app.project().effect(hum_s, hum_fx).unwrap().spec.clone();
    assert_eq!(
        (spec.get("hon"), spec.get("hmode"), spec.get("hhz")),
        (1.0, 1.0, 50.0),
        "Mains hum 50 Hz, followed"
    );
    let slot = app.project().strip(hum_s).unwrap().effects[0].clone();
    assert_eq!(EffectUnit::new(&slot, 48_000.0).latency(), 0, "notches");
    let set = |app: &mut CantusApp, strip: StripId, effect: EffectId, kv: &[(&str, f32)]| {
        let commands = kv
            .iter()
            .map(|(k, v)| Command::SetEffectParam {
                strip,
                effect,
                key: (*k).into(),
                value: *v,
            })
            .collect();
        app.exec(Command::Batch {
            label_key: "cmd.fx_preset".into(),
            commands,
        });
    };

    // The hum comes out: each harmonic on the master, from 1.5 s on (the follower settled).
    solo(&mut app, hum_t);
    let cleaned = left(&rendered(&mut app));
    let dry_l = left(&dry_hum);
    let mut worst = f64::INFINITY;
    let mut downs = Vec::new();
    for n in 1..=8 {
        let hz = 50.0 * n as f64;
        let down = tone_db(&dry_l, hz, 1.5, 4.0) - tone_db(&cleaned, hz, 1.5, 4.0);
        downs.push(format!("{down:.1}"));
        worst = worst.min(down);
    }
    let music_kept = tone_db(&dry_l, 440.0, 1.5, 4.0) - tone_db(&cleaned, 440.0, 1.5, 4.0);
    eprintln!(
        "hum down by harmonic: {} dB; the music at 440 Hz {music_kept:+.2} dB",
        downs.join(" ")
    );
    assert!(
        worst > 30.0,
        "every harmonic down 30 dB and more: {worst:.1}"
    );
    assert!(
        music_kept.abs() < 0.5,
        "the music stays: {music_kept:+.2} dB"
    );

    // Hear what is removed: the hum module's delta is the hum (its level kept within 2 dB),
    // the music gone from it.
    set(&mut app, hum_s, hum_fx, &[("hdelta", 1.0)]);
    let removed = left(&rendered(&mut app));
    let hum_heard = tone_db(&removed, 50.0, 1.5, 4.0) - tone_db(&dry_l, 50.0, 1.5, 4.0);
    let music_gone = tone_db(&dry_l, 440.0, 1.5, 4.0) - tone_db(&removed, 440.0, 1.5, 4.0);
    eprintln!("what is removed: the hum {hum_heard:+.2} dB, the music down {music_gone:.1} dB");
    assert!(
        hum_heard.abs() < 2.0,
        "the hum is heard: {hum_heard:+.2} dB"
    );
    assert!(music_gone > 20.0, "the music is not: {music_gone:.1} dB");
    app.undo();

    // Clicks repaired: the de-click on the clicked strip; the error against the clean music
    // through it, against the same input through it at mix 0 (aligned to the sample).
    let click_fx = insert(&mut app, click_s);
    set(&mut app, click_s, click_fx, &[("hon", 0.0), ("con", 1.0)]);
    solo(&mut app, click_t);
    let repaired = left(&rendered(&mut app));
    set(&mut app, click_s, click_fx, &[("cmix", 0.0)]);
    let unrepaired = left(&rendered(&mut app));
    app.undo();
    let lat = 767;
    let shifted: Vec<f32> = std::iter::repeat_n(0.0, lat)
        .chain(clean.iter().copied())
        .take(clean.len())
        .collect();
    let all = [(lat, FRAMES)];
    let before = power_in(&diff(&unrepaired, &shifted), &all);
    let after = power_in(&diff(&repaired, &shifted), &all);
    let clicks_down = db(before) - db(after);
    eprintln!("clicks down {clicks_down:.1} dB");
    assert!(
        before > 1e-6,
        "the clicks are there at mix 0 (aligned to the clean music)"
    );
    assert!(
        clicks_down > 10.0,
        "the clicks repaired: {clicks_down:.1} dB"
    );

    // A reverberant voice's tail reduced: the room module, its decay estimated, Direct sound
    // 0 dB (a far microphone); the tail in the pauses against the same voice at mix 0.
    let room_fx = insert(&mut app, room_s);
    set(
        &mut app,
        room_s,
        room_fx,
        &[
            ("hon", 0.0),
            ("von", 1.0),
            ("vdecay", 1.0),
            ("vdirect", 0.0),
        ],
    );
    solo(&mut app, room_t);
    let dereverbed = left(&rendered(&mut app));
    set(&mut app, room_s, room_fx, &[("vmix", 0.0)]);
    let reverberant = left(&rendered(&mut app));
    app.undo();
    // The pauses: from 80 ms after each syllable to the next, after the first two (the
    // decay learned), in the output's time (both renders late by the same 1 023 samples).
    let l = 1_023;
    let pauses: Vec<(usize, usize)> = (2..5)
        .map(|s| {
            let at = (s as f64 * 0.75 * SR as f64) as usize + l;
            (
                at + (0.26 * SR as f64) as usize,
                at + (0.74 * SR as f64) as usize,
            )
        })
        .collect();
    let syllables: Vec<(usize, usize)> = (2..5)
        .map(|s| {
            let at = (s as f64 * 0.75 * SR as f64) as usize + l;
            (at, at + (0.18 * SR as f64) as usize)
        })
        .collect();
    let tail_down = db(power_in(&reverberant, &pauses)) - db(power_in(&dereverbed, &pauses));
    let voice_kept = db(power_in(&dereverbed, &syllables)) - db(power_in(&reverberant, &syllables));
    eprintln!(
        "the room's tail down {tail_down:.1} dB in the pauses; the syllables {voice_kept:+.1} dB"
    );
    assert!(tail_down > 6.0, "the tail reduced: {tail_down:.1} dB");
    assert!(
        voice_kept > -6.0,
        "the voice is not taken with it: {voice_kept:+.1} dB"
    );

    // The latency compensated against a parallel dry strip, to the sample: every module on
    // the hummed strip (the gentle look-ahead, the de-click, the room: 87 ms) at mix 0, so
    // what leaves it is its input exactly that late; the master with it alone is the
    // master with the dry strip alone, to the bit.
    set(
        &mut app,
        hum_s,
        hum_fx,
        &[
            ("hgentle", 1.0),
            ("con", 1.0),
            ("von", 1.0),
            ("hmix", 0.0),
            ("cmix", 0.0),
            ("vmix", 0.0),
        ],
    );
    let slot = app.project().strip(hum_s).unwrap().effects[0].clone();
    assert_eq!(
        EffectUnit::new(&slot, 48_000.0).latency(),
        2_400 + 767 + 1_023,
        "every module's latency"
    );
    solo(&mut app, hum_t);
    let through = rendered(&mut app);
    solo(&mut app, dry_t);
    let parallel = rendered(&mut app);
    assert!(
        through
            .iter()
            .zip(&parallel)
            .all(|(a, b)| a.to_bits() == b.to_bits()),
        "the latent strip is the dry strip, to the sample"
    );
    set(
        &mut app,
        hum_s,
        hum_fx,
        &[
            ("hgentle", 0.0),
            ("con", 0.0),
            ("von", 0.0),
            ("hmix", 1.0),
            ("cmix", 1.0),
            ("vmix", 1.0),
        ],
    );

    // An edit heard, undone to the bit: two harmonics instead of eight.
    solo(&mut app, hum_t);
    let base = rendered(&mut app);
    set(&mut app, hum_s, hum_fx, &[("hharm", 2.0)]);
    let moved = rendered(&mut app);
    assert!(
        tone_db(&left(&moved), 300.0, 1.5, 4.0) > tone_db(&left(&base), 300.0, 1.5, 4.0) + 20.0,
        "the sixth harmonic is heard again"
    );
    app.undo();
    assert_eq!(rendered(&mut app), base, "undone, to the bit");

    // What plays is the offline render: every strip sounding, the room's latency on the
    // master, played from the top in blocks that do not divide anything.
    let commands = tracks
        .iter()
        .map(|t| Command::SetTrackMute {
            id: *t,
            muted: false,
        })
        .collect();
    app.exec(Command::Batch {
        label_key: "cmd.fx_preset".into(),
        commands,
    });
    set(&mut app, room_s, room_fx, &[("con", 1.0)]);
    let offline = rendered(&mut app);
    let play = |proc_: &mut Processor, frames: usize, out: &mut Vec<f32>| {
        let sizes = [256usize, 333, 512, 17, 1024];
        let mut i = 0;
        let mut done = 0;
        while done < frames {
            let n = sizes[i % sizes.len()].min(frames - done);
            i += 1;
            let mut buf = vec![0f32; n * 2];
            proc_.process(&mut buf, 2);
            out.extend_from_slice(&buf);
            done += n;
        }
    };
    app.seek(Ticks::ZERO);
    app.sync_engine();
    proc_.settle();
    app.toggle_play();
    let mut live = Vec::new();
    play(&mut proc_, 2 * SR, &mut live);
    app.toggle_play();
    assert!(power_in(&live, &[(0, live.len())]) > 1e-6, "it plays live");
    assert!(
        live.iter()
            .zip(&offline[..live.len()])
            .all(|(a, b)| a.to_bits() == b.to_bits()),
        "live equals the render"
    );

    // Its defaults (every module off): the dry master, to the bit, on every strip at once.
    let dry_all = {
        // The three inserts at their defaults…
        for (strip, effect) in [(hum_s, hum_fx), (click_s, click_fx), (room_s, room_fx)] {
            let defaults: Vec<Command> = EffectKind::Repair
                .params()
                .iter()
                .map(|i| Command::SetEffectParam {
                    strip,
                    effect,
                    key: i.key.into(),
                    value: i.default,
                })
                .collect();
            app.exec(Command::Batch {
                label_key: "cmd.fx_preset".into(),
                commands: defaults,
            });
        }
        rendered(&mut app)
    };
    // …against the same project with no Purus at all.
    for (strip, effect) in [(hum_s, hum_fx), (click_s, click_fx), (room_s, room_fx)] {
        app.exec(Command::RemoveEffect { strip, effect });
    }
    assert_eq!(dry_all, rendered(&mut app), "the null, to the bit");
}
