//! Nitor end to end (round 41, ADR 0151), on the boot path: an app made by
//! `CantusApp::assemble` with the real engine handle, Nitor inserted on a strip as the
//! picker inserts it (its starting preset, *Gentle shine*: a third of the Clear character,
//! level-matched), and:
//!
//! - **it changes the sound** and not the loudness: the strip differs from the dry one, and
//!   its loudness is within half a LU of it;
//! - **the sub follows the project's key**: in *tonic* mode it sounds on G1 while the song is
//!   in G, and on D1 once the project's key is D (a render each, and live: the key changed
//!   while it plays, the sub moves in the next blocks);
//! - **live in odd blocks equals the offline render**, to the bit;
//! - **at amount 0 every part off it is the dry song**, to the bit.
//!
//! Its own test binary: it points `CANTUS_DATA_DIR` at a temporary folder. // r41:nitor

use cantus_core::scale::{Key, ScaleKind};
use cantus_core::*;
use cantus_engine::{render, EngineHandle, Processor, RenderOptions};
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

fn rendered(app: &mut CantusApp, beats: i64) -> Vec<f32> {
    let p = app.project().clone();
    let snap = app.engine.compile_for_render(&p, 1);
    let opts = RenderOptions {
        start: Ticks::ZERO,
        end: Some(Ticks::beats(beats)),
        tail_seconds: 0.0,
        ..Default::default()
    };
    render(snap, &opts).unwrap().samples
}

fn left(x: &[f32]) -> Vec<f32> {
    x.iter().step_by(2).copied().collect()
}

/// The left channel's amplitude at `hz` from `a` to `b` seconds (one DFT bin over whole
/// cycles), dB.
fn tone_db(x: &[f32], hz: f64, a: f64, b: f64) -> f64 {
    let l = left(x);
    let per = SR as f64 / hz;
    let (i, mut j) = ((a * SR as f64) as usize, (b * SR as f64) as usize);
    j = i + (((j - i) as f64 / per).floor() * per) as usize;
    let (mut re, mut im) = (0.0f64, 0.0f64);
    for (n, v) in l[i..j].iter().enumerate() {
        let w = std::f64::consts::TAU * hz * n as f64 / SR as f64;
        re += f64::from(*v) * w.cos();
        im += f64::from(*v) * w.sin();
    }
    20.0 * (2.0 * (re * re + im * im).sqrt() / (j - i) as f64)
        .max(1e-12)
        .log10()
}

/// K-weighted loudness of interleaved stereo from `a` seconds, LUFS (no gates).
fn loudness(x: &[f32], a: f64) -> f64 {
    let mut k = cantus_dsp::comprimo::loudness::KWeighting::new(SR as f32);
    let from = (a * SR as f64) as usize;
    let (mut sum, mut n) = (0.0, 0usize);
    for (i, f) in x.chunks_exact(2).enumerate() {
        let e = k.tick(f[0], f[1]);
        if i >= from {
            sum += e;
            n += 1;
        }
    }
    -0.691 + 10.0 * (sum / n.max(1) as f64).max(1e-30).log10()
}

fn line(p: &mut Project, name: &str, notes: &[(u8, f64, f64, u8)]) -> Track {
    let mut track = p.make_track(name);
    let mut clip = p.make_clip(name, Ticks::ZERO, Ticks::beats(32));
    for &(pitch, at, len, vel) in notes {
        let n = p.make_note(
            Ticks::from_beats_f64(at),
            Ticks::from_beats_f64(len),
            pitch,
            vel,
        );
        clip.notes.push(n);
    }
    track.clips.push(clip);
    track
}

/// A song in G: a bass hitting G2 on every beat, a chord over it.
fn song() -> Project {
    let mut p = Project::new("Nitor");
    p.tempo_bpm = 120.0;
    p.key = Some(Key::new(7, ScaleKind::Major));
    let hits: Vec<(u8, f64, f64, u8)> = (0..16).map(|b| (43, b as f64, 0.8, 110)).collect();
    let bass = line(&mut p, "Bass", &hits);
    p.tracks.push(bass);
    let chord = line(
        &mut p,
        "Chord",
        &[
            (67, 0.0, 16.0, 80),
            (71, 0.0, 16.0, 80),
            (74, 0.0, 16.0, 80),
        ],
    );
    p.tracks.push(chord);
    p
}

fn set(app: &mut CantusApp, strip: StripId, effect: EffectId, key: &str, value: f32) {
    app.exec(Command::SetEffectParam {
        strip,
        effect,
        key: key.into(),
        value,
    });
}

#[test]
fn nitor_colours_the_strip_follows_the_song_s_key_and_is_a_wire_at_zero_on_the_boot_path() {
    let (mut app, mut proc_) = assembled();
    app.replace_project(song());
    let strip = app.project().tracks[0].strip.expect("a strip");
    let dry = rendered(&mut app, 8);
    assert!(loudness(&dry, 0.5) > -60.0, "the song sounds");

    // Inserted as the picker inserts it: the starting preset, which sounds at once.
    app.add_effect(strip, EffectKind::Enhancer);
    let fx = app.project().strip(strip).unwrap().effects[0].clone();
    assert_eq!(
        fx.spec.get("amount"),
        0.35,
        "Gentle shine: a third of Clear"
    );
    assert_eq!(fx.spec.get("match"), 1.0, "level-matched");
    let fx = fx.id;
    let wet = rendered(&mut app, 8);
    let moved = dry.iter().zip(&wet).filter(|(a, b)| a != b).count();
    let (ld, lw) = (loudness(&dry, 1.5), loudness(&wet, 1.5));
    eprintln!("the starting preset: {moved} samples moved, {ld:.2} → {lw:.2} LUFS");
    assert!(moved > dry.len() / 2, "it changes the sound");
    assert!(
        (lw - ld).abs() < 0.5,
        "and not the loudness: {ld:.2} → {lw:.2}"
    );

    // The sub on the song's tonic.
    set(&mut app, strip, fx, "sub", 1.0);
    set(&mut app, strip, fx, "submode", 1.0);
    set(&mut app, strip, fx, "match", 0.0);
    let in_g = rendered(&mut app, 8);
    let (g1, d1) = (
        tone_db(&in_g, 49.0, 1.0, 4.0),
        tone_db(&in_g, 36.71, 1.0, 4.0),
    );
    eprintln!("in G: G1 {g1:.1} dB, D1 {d1:.1} dB");
    assert!(g1 - d1 > 20.0, "the sub on G1: {g1:.1} against {d1:.1}");
    app.exec(Command::SetProjectKey {
        key: Some(Key::new(2, ScaleKind::Major)),
    });
    let in_d = rendered(&mut app, 8);
    let (g1d, d1d) = (
        tone_db(&in_d, 49.0, 1.0, 4.0),
        tone_db(&in_d, 36.71, 1.0, 4.0),
    );
    eprintln!("in D: G1 {g1d:.1} dB, D1 {d1d:.1} dB");
    assert!(
        d1d - g1d > 20.0,
        "the sub moved to D1: {d1d:.1} against {g1d:.1}"
    );

    // Live: the key changed while it plays — the sub moves within the next blocks.
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
    app.exec(Command::SetProjectKey {
        key: Some(Key::new(7, ScaleKind::Major)),
    });
    app.seek(Ticks::ZERO);
    app.sync_engine();
    proc_.settle();
    app.toggle_play();
    let mut live = Vec::new();
    play(&mut proc_, 2 * SR, &mut live);
    app.exec(Command::SetProjectKey {
        key: Some(Key::new(2, ScaleKind::Major)),
    });
    app.sync_engine();
    play(&mut proc_, 2 * SR, &mut live);
    app.toggle_play();
    let (lg, ld1) = (
        tone_db(&live, 49.0, 0.5, 2.0),
        tone_db(&live, 36.71, 0.5, 2.0),
    );
    let (lg2, ld2) = (
        tone_db(&live, 49.0, 2.5, 4.0),
        tone_db(&live, 36.71, 2.5, 4.0),
    );
    eprintln!("live: G1 {lg:.1} / D1 {ld1:.1} dB, then G1 {lg2:.1} / D1 {ld2:.1} dB");
    assert!(
        lg - ld1 > 20.0 && ld2 - lg2 > 20.0,
        "the sub followed the key live"
    );
    // Live in odd blocks equals the offline render (in G for its first two seconds).
    let offline_g = in_g[..2 * SR * 2].to_vec();
    let first = &live[..2 * SR * 2];
    let differ = first
        .iter()
        .zip(&offline_g)
        .position(|(a, b)| a.to_bits() != b.to_bits());
    assert_eq!(differ, None, "live = offline, to the bit");

    // At amount 0, every part off: the dry song to the bit.
    for (k, v) in [("amount", 0.0), ("sub", 0.0), ("match", 1.0)] {
        set(&mut app, strip, fx, k, v);
    }
    let null = rendered(&mut app, 8);
    let first = null
        .iter()
        .zip(&dry)
        .position(|(a, b)| a.to_bits() != b.to_bits());
    assert_eq!(first, None, "a wire at its defaults");
}
