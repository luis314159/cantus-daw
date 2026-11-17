//! Lenis end to end (fx side round, wave B, ADR 0143 provisional), on the boot path: an
//! app made by `CantusApp::assemble` with the real engine handle; a bright saw chord on two
//! tracks, each through an Aequalis bell of +15 dB at 3 kHz (the ring), the second the
//! first's twin. Lenis is inserted on the first as the picker inserts it — on its starting
//! preset, Gentle, 10.6 ms late:
//!
//! - **the latency compensated on the master**: at its defaults Lenis is its input delayed
//!   by its latency, and the twin is held back by as much, so the master is the dry master
//!   delayed by the latency, to the bit — at Standard, and after a change to Precise and to
//!   Fast (each re-compensated);
//! - **the ring cut, its neighbours untouched**: the 3 kHz band measurably down, the bands
//!   at 500 Hz and 10 kHz within a fraction of a decibel;
//! - **What is removed heard**: the delta is the ring, and the delayed input less the
//!   output;
//! - an edit heard and undone to the bit; live in odd blocks equal to the offline render.
//!
//! Its own test binary: it points `CANTUS_DATA_DIR` at a temporary folder. // fxb:lenis

use cantus_core::*;
use cantus_engine::{render, EffectUnit, EngineHandle, Processor, RenderOptions};
use cantus_ui::app::Files;
use cantus_ui::CantusApp;
use std::path::Path;
use std::sync::Arc;

const SR: usize = 48_000;

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

/// Eight beats at 120 BPM (four seconds), interleaved stereo.
fn rendered(app: &mut CantusApp) -> Vec<f32> {
    let p = app.project().clone();
    let snap = app.engine.compile_for_render(&p, 1);
    let opts = RenderOptions {
        end: Some(Ticks::beats(8)),
        tail_seconds: 0.0,
        ..Default::default()
    };
    render(snap, &opts).unwrap().samples
}

/// The left channel's power in a band (three two-pole band-passes at `hz` in series, Q 6
/// each, so a loud ring an octave away does not leak in) from `a` to `b` seconds, in dB.
fn band_db(x: &[f32], hz: f64, a: f64, b: f64) -> f64 {
    let w = std::f64::consts::TAU * hz / SR as f64;
    let alpha = w.sin() / (2.0 * 6.0);
    let a0 = 1.0 + alpha;
    let (b0, b2) = (alpha / a0, -alpha / a0);
    let (a1, a2) = (-2.0 * w.cos() / a0, (1.0 - alpha) / a0);
    let mut z = [[0.0f64; 4]; 3];
    let (i0, i1) = ((a * SR as f64) as usize, (b * SR as f64) as usize);
    let mut sum = 0.0;
    for (i, frame) in x.chunks_exact(2).enumerate().take(i1) {
        let mut v = f64::from(frame[0]);
        for [x1, x2, y1, y2] in z.iter_mut() {
            let y = b0 * v + b2 * *x2 - a1 * *y1 - a2 * *y2;
            (*x2, *x1, *y2, *y1) = (*x1, v, *y1, y);
            v = y;
        }
        if i >= i0 {
            sum += v * v;
        }
    }
    10.0 * (sum / (i1 - i0) as f64).max(1e-30).log10()
}

fn power_db(x: &[f32]) -> f64 {
    let s: f64 = x.iter().map(|v| f64::from(*v) * f64::from(*v)).sum();
    10.0 * (s / x.len() as f64).max(1e-30).log10()
}

/// `out` is `reference` exactly `latency` frames later, to the bit.
fn assert_delayed(reference: &[f32], out: &[f32], latency: usize, what: &str) {
    let shift = 2 * latency;
    assert!(
        out[..shift].iter().all(|x| *x == 0.0),
        "{what}: sound before the latency"
    );
    let bad = reference[..reference.len() - shift]
        .iter()
        .zip(&out[shift..])
        .filter(|(a, b)| a.to_bits() != b.to_bits())
        .count();
    assert_eq!(
        bad, 0,
        "{what}: {bad} samples differ from the delayed dry master"
    );
}

/// A bright saw (no filter movement, one oscillator) holding A2, E3 and A3 for eight beats.
fn ringing_track(p: &mut Project, name: &str) -> Track {
    let mut track = p.make_track(name);
    track.instrument = InstrumentSpec::Firmus(FirmusParams {
        osc_mix: 0.0,
        osc2_detune_cents: 0.0,
        cutoff_hz: 18_000.0,
        resonance: 0.0,
        filter_env_octaves: 0.0,
        key_track: 0.0,
        amp_sustain: 1.0,
        ..FirmusParams::default()
    });
    let mut clip = p.make_clip(name, Ticks::ZERO, Ticks::beats(8));
    for pitch in [45u8, 52, 57] {
        let n = p.make_note(Ticks::ZERO, Ticks::beats(8), pitch, 100);
        clip.notes.push(n);
    }
    track.clips.push(clip);
    track
}

#[test]
fn lenis_cuts_a_ring_is_compensated_on_the_master_and_undoes_on_the_boot_path() {
    let (mut app, mut proc_) = assembled();
    let mut p = Project::new("Lenis");
    p.tempo_bpm = 120.0;
    for name in ["Ring", "Twin"] {
        let t = ringing_track(&mut p, name);
        p.tracks.push(t);
    }
    app.replace_project(p);
    let strips = [0, 1].map(|t| app.project().tracks[t].strip.expect("a strip"));
    // The ring: a narrow bell on each strip, inserted neutral and set by hand.
    app.settings.insert_neutral = true;
    let bell = cantus_dsp::EqShape::Bell.index() as f32;
    for s in strips {
        app.add_effect(s, EffectKind::Eq);
        let eq = app.project().strip(s).unwrap().effects[0].id;
        for (key, value) in [
            ("b0.used", 1.0),
            ("b0.on", 1.0),
            ("b0.shape", bell),
            ("b0.freq", 3_000.0),
            ("b0.gain", 15.0),
            ("b0.q", 8.0),
            ("b0.gq", 0.0),
        ] {
            app.exec(Command::SetEffectParam {
                strip: s,
                effect: eq,
                key: key.into(),
                value,
            });
        }
    }
    app.settings.insert_neutral = false;
    let twin = app.project().tracks[1].id;
    let mute = |app: &mut CantusApp, muted: bool| {
        app.exec(Command::SetTrackMute { id: twin, muted });
    };
    let dry_both = rendered(&mut app);
    mute(&mut app, true);
    let dry = rendered(&mut app);
    mute(&mut app, false);
    let ring_dry = band_db(&dry, 3_000.0, 0.5, 3.5);
    eprintln!(
        "dry: the 3 kHz band {ring_dry:.1} dB, 500 Hz {:.1} dB, 10 kHz {:.1} dB",
        band_db(&dry, 500.0, 0.5, 3.5),
        band_db(&dry, 10_000.0, 0.5, 3.5)
    );
    assert!(power_db(&dry) > -40.0, "the chord sounds");

    // Inserted as the picker inserts it, after the bell: Gentle, 511 samples late.
    let s0 = strips[0];
    app.add_effect(s0, EffectKind::ResonanceSuppressor);
    let effect = app.project().strip(s0).unwrap().effects[1].id;
    let spec = app.project().effect(s0, effect).unwrap().spec.clone();
    assert_eq!(spec.kind, EffectKind::ResonanceSuppressor);
    assert_eq!(
        (spec.get("depth"), spec.get("lo"), spec.get("curve")),
        (0.5, 150.0, 0.0),
        "Gentle: half, from 150 Hz, the soft curve"
    );
    let slot = app.project().strip(s0).unwrap().effects[1].clone();
    assert_eq!(EffectUnit::new(&slot, 48_000.0).latency(), 511);
    let set = |app: &mut CantusApp, key: &str, value: f32| {
        app.exec(Command::SetEffectParam {
            strip: s0,
            effect,
            key: key.into(),
            value,
        })
    };
    let gentle = rendered(&mut app);
    assert!(
        gentle[2 * 511..]
            .iter()
            .zip(&dry_both)
            .any(|(a, b)| (a - b).abs() > 1e-4),
        "Gentle sounds on the ring"
    );

    // Its neutral defaults: the dry master delayed by the latency, to the bit — the twin
    // held back by the master's compensation exactly as Lenis is late.
    let defaults: Vec<Command> = EffectKind::ResonanceSuppressor
        .params()
        .iter()
        .map(|i| Command::SetEffectParam {
            strip: s0,
            effect,
            key: i.key.into(),
            value: i.default,
        })
        .collect();
    app.exec(Command::Batch {
        label_key: "cmd.fx_preset".into(),
        commands: defaults,
    });
    assert_delayed(
        &dry_both,
        &rendered(&mut app),
        511,
        "the defaults at Standard",
    );
    // A quality change: re-compensated at the new frame's latency.
    for (qual, latency) in [(2.0, 1_023), (0.0, 255), (1.0, 511)] {
        set(&mut app, "qual", qual);
        assert_delayed(
            &dry_both,
            &rendered(&mut app),
            latency,
            &format!("the defaults at quality {qual}"),
        );
    }

    // The ring cut, its neighbours untouched (the twin muted): Hard, most of each bump,
    // the focus over the upper mids.
    mute(&mut app, true);
    let still = rendered(&mut app);
    for (key, value) in [
        ("depth", 0.9),
        ("curve", 1.0),
        ("select", 0.3),
        ("lo", 1_000.0),
        ("hi", 6_000.0),
    ] {
        set(&mut app, key, value);
    }
    let wet = rendered(&mut app);
    let change = |hz: f64| band_db(&wet, hz, 0.5, 3.5) - band_db(&still, hz, 0.5, 3.5);
    let (ring, low, high) = (change(3_000.0), change(500.0), change(10_000.0));
    eprintln!(
        "Lenis Hard 90 %: the 3 kHz band {ring:+.2} dB, 500 Hz {low:+.3} dB, 10 kHz {high:+.3} dB"
    );
    assert!(ring < -3.0, "the ring is cut: {ring:+.2} dB");
    assert!(low.abs() < 0.2, "500 Hz untouched: {low:+.3} dB");
    assert!(high.abs() < 0.2, "10 kHz untouched: {high:+.3} dB");

    // What is removed: the ring, and exactly the delayed input less the output.
    set(&mut app, "delta", 1.0);
    let removed = rendered(&mut app);
    let latency = 511;
    let mut worst = 0f32;
    for (i, d) in removed.iter().enumerate() {
        let x = if i >= 2 * latency {
            dry[i - 2 * latency]
        } else {
            0.0
        };
        worst = worst.max((x - wet[i] - d).abs());
    }
    let (r_ring, r_low) = (
        band_db(&removed, 3_000.0, 0.5, 3.5),
        band_db(&removed, 500.0, 0.5, 3.5),
    );
    eprintln!(
        "What is removed: the 3 kHz band {r_ring:.1} dB, 500 Hz {r_low:.1} dB (dry {:.1}); \
         against the delayed input less the output {worst:.1e}",
        band_db(&dry, 500.0, 0.5, 3.5)
    );
    assert!(worst < 1e-5, "the delta is what is taken away: {worst:.1e}");
    assert!(
        r_ring > r_low + 20.0,
        "the delta is the ring: {r_ring:.1} at 3 kHz, {r_low:.1} at 500 Hz"
    );
    set(&mut app, "delta", 0.0);
    mute(&mut app, false);

    // An edit heard, undone to the bit.
    let base = rendered(&mut app);
    set(&mut app, "depth", 0.3);
    let moved = rendered(&mut app);
    assert!(
        moved.iter().zip(&base).any(|(a, b)| (a - b).abs() > 1e-4),
        "90 % to 30 % is heard"
    );
    app.undo();
    assert_eq!(rendered(&mut app), base, "undone, to the bit");

    // What plays is the offline render: the engine the app drives, from the top, in blocks
    // that divide nothing.
    let play = |proc_: &mut Processor, frames: usize, out: &mut Vec<f32>| {
        let sizes = [256usize, 333, 512, 17, 1024];
        let (mut i, mut done) = (0, 0);
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
    let offline = {
        let p = app.project().clone();
        let r = render(
            Arc::new(Snapshot::compile(&p, 1)),
            &RenderOptions {
                end: Some(Ticks::beats(8)),
                tail_seconds: 0.0,
                ..Default::default()
            },
        )
        .unwrap();
        r.samples[..live.len()].to_vec()
    };
    assert!(power_db(&live[2 * 1024..]) > -40.0, "it plays live");
    assert!(
        live.iter()
            .zip(&offline)
            .all(|(a, b)| a.to_bits() == b.to_bits()),
        "live equals the render"
    );
}

/// The starting preset is the file's Gentle, named in the editor; every factory preset
/// names real keys, is in both languages, and every quality and both curves are among
/// them.
#[test]
fn the_presets_name_real_keys_in_both_languages_every_quality_and_curve() {
    use cantus_ui::views::fx::frame::{read_presets, recognise};
    let kind = EffectKind::ResonanceSuppressor;
    let file: Vec<_> =
        cantus_ui::preset_data::presets_of(cantus_ui::preset_data::FileUnit::Effect(kind))
            .collect();
    assert!(file.len() >= 12, "twelve presets or more: {}", file.len());
    let mut seen = [[false; 2]; 3];
    let mut chans = [false; 3];
    for p in &file {
        let mut spec = EffectSpec::new(kind);
        for (k, v) in &p.values {
            assert!(spec.set(k, *v).is_some(), "{}: no parameter {k}", p.key);
        }
        assert!(spec.get("depth") > 0.0, "{}: it does something", p.key);
        assert_eq!(spec.get("delta"), 0.0, "{}: never the delta", p.key);
        seen[spec.get("qual") as usize][spec.get("curve") as usize] = true;
        chans[spec.get("chan") as usize] = true;
        for lang in [cantus_ui::i18n::Locale::En, cantus_ui::i18n::Locale::Es] {
            cantus_ui::i18n::set_thread_locale(lang);
            assert_ne!(cantus_ui::i18n::t(&p.key), p.key, "{} in {lang:?}", p.key);
        }
    }
    cantus_ui::i18n::set_thread_locale(cantus_ui::i18n::Locale::En);
    for (q, curves) in seen.iter().enumerate() {
        assert!(curves.iter().any(|c| *c), "quality {q} among the presets");
    }
    assert!(
        seen.iter().any(|c| c[0]) && seen.iter().any(|c| c[1]),
        "both curves among the presets"
    );
    assert_eq!(chans, [true; 3], "every channel mode among the presets");
    let start = cantus_core::starting::starting_spec(kind);
    let list = read_presets(kind, false);
    let named = recognise(&start, &list).expect("named");
    assert_eq!(named.name, cantus_ui::i18n::t("preset.lenis.gentle"));
}
