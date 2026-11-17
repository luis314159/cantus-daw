//! Comparatio end to end (fx side round, wave C, ADR 0146 provisional), on the boot path: an
//! app made by `CantusApp::assemble` with the real engine handle; a mix imported as a user
//! imports it (`import_audio`, onto its own track and strip); Comparatio inserted on the
//! master's last slot as the picker inserts it; a reference loaded from a WAV the way the
//! editor's *Load reference…* and a drop load it (`comparatio::add_references`, through the
//! editor's host: the list, the one compared, its size and hash):
//!
//! - **no render ever holds the reference**: rendered with the reference heard, the blind X
//!   hidden, or either blind choice, the master is the dry master, to the bit;
//! - **live, your mix is untouched**: played in odd blocks, the master is the offline render
//!   to the bit (live = offline for the mix);
//! - **the reference is heard loudness-matched**: switched to it, the master's loudness over
//!   the stretch heard is the mix's within 0.1 LU, though the file is 10 dB louder; and
//!   it is the reference that is heard (another sound, brighter);
//! - **back to your mix, to the bit**, once the crossfade has run;
//! - a second reference added and compared, one undo step back to the first.
//!
//! Its own test binary: it points `CANTUS_DATA_DIR` at a temporary folder. // fxc:comparatio

use cantus_core::*;
use cantus_dsp::terminus::loudness::LoudnessMeter;
use cantus_engine::{render, EngineHandle, Processor, RenderOptions};
use cantus_ui::app::Files;
use cantus_ui::views;
use cantus_ui::CantusApp;
use std::path::{Path, PathBuf};

const SR: usize = 48_000;
/// The takes' length: 8.3 s (no whole bar at 120 BPM: never a loop the import warps).
const FRAMES: usize = 398_400;

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

fn noise(seed: &mut u32) -> f32 {
    *seed ^= *seed << 13;
    *seed ^= *seed >> 17;
    *seed ^= *seed << 5;
    (*seed as f32 / u32::MAX as f32) * 2.0 - 1.0
}

/// Steady noise through a one-pole low-pass `a` (0 … 1: smaller is darker), `gain` after.
fn steady(seed: u32, a: f32, gain: f32, frames: usize) -> (Vec<f32>, Vec<f32>) {
    let mut s = seed;
    let (mut l1, mut l2) = (0.0f32, 0.0f32);
    let mut l = Vec::with_capacity(frames);
    let mut r = Vec::with_capacity(frames);
    for _ in 0..frames {
        l1 += a * (noise(&mut s) - l1);
        l2 += a * (noise(&mut s) - l2);
        l.push(gain * l1);
        r.push(gain * (0.8 * l1 + 0.2 * l2));
    }
    (l, r)
}

/// `frames` frames from 0, interleaved, the latency kept.
fn rendered(app: &mut CantusApp) -> Vec<f32> {
    let p = app.project().clone();
    let snap = app.engine.compile_for_render(&p, 1);
    let opts = RenderOptions {
        end: Some(Ticks::from_beats_f64(16.0)),
        tail_seconds: 0.0,
        ..Default::default()
    };
    render(snap, &opts).unwrap().samples
}

fn split(x: &[f32]) -> (Vec<f32>, Vec<f32>) {
    (
        x.iter().step_by(2).copied().collect(),
        x.iter().skip(1).step_by(2).copied().collect(),
    )
}

/// The integrated loudness of interleaved `x` over seconds `a … b`, LUFS (EBU R128).
fn lufs(x: &[f32], a: f64, b: f64) -> f64 {
    let (i, j) = ((a * SR as f64) as usize * 2, (b * SR as f64) as usize * 2);
    let (l, r) = split(&x[i..j]);
    let mut m = LoudnessMeter::new(SR as f32);
    m.process(&l, &r);
    m.integrated().expect("loud enough to measure")
}

/// Play `frames` in blocks that divide nothing, answering the engine as the app's frame
/// does between blocks (the reference handed over by its poll).
fn play(app: &mut CantusApp, proc_: &mut Processor, frames: usize, out: &mut Vec<f32>) {
    let sizes = [256usize, 333, 512, 17, 1024];
    let mut done = 0;
    let mut i = 0;
    while done < frames {
        let n = sizes[i % sizes.len()].min(frames - done);
        i += 1;
        let mut buf = vec![0f32; n * 2];
        proc_.process(&mut buf, 2);
        out.extend_from_slice(&buf);
        done += n;
        app.engine.poll();
    }
}

fn set(app: &mut CantusApp, effect: EffectId, key: &str, value: f32) {
    app.exec(Command::SetEffectParam {
        strip: StripId::MASTER,
        effect,
        key: key.into(),
        value,
    });
    app.sync_engine();
}

#[test]
fn comparatio_compares_a_reference_on_the_boot_path() {
    let (mut app, mut proc_) = assembled();
    let mut p = Project::empty("Comparatio");
    p.tempo_bpm = 120.0;
    app.replace_project(p);
    // The mix: darker, quieter. The reference: brighter, 10 dB louder in the file.
    let (ml, mr) = steady(0x5EED_0001, 0.08, 0.25, FRAMES);
    let (rl, rr) = steady(0x0BAD_5EED, 0.6, 0.30, FRAMES);
    let mix = wav("mix.wav", &ml, &mr);
    let reference = wav("Finished master.wav", &rl, &rr);
    assert_eq!(app.import_audio(&[mix], Ticks::ZERO, None), 1, "imported");
    let track = app.project().tracks[0].id;
    app.add_strips_up_to(1);
    if app.project().track(track).unwrap().strip.is_none() {
        let s = app.project().strips[0].id;
        app.route_track(track, Some(s));
    }
    let dry = rendered(&mut app);
    // Inserted on the master's last slot, as the picker inserts it: it measures, the mix
    // heard (no starting preset moves a sample).
    app.add_effect(StripId::MASTER, EffectKind::Reference);
    let fx = app.project().master.effects.last().unwrap().id;
    assert_eq!(rendered(&mut app), dry, "at its defaults: the dry master");
    // The reference loaded as the editor loads it.
    {
        let mut host = views::fx::host::DawHost::new(&mut app, StripId::MASTER, fx, "comparatio");
        assert!(views::fx::comparatio::add_references(
            &mut host,
            &[reference.clone()]
        ));
    }
    // Read off the audio thread (on a thread of its own when files load in the background):
    // the app's frames attach it.
    let stored = app.project().effect(StripId::MASTER, fx).unwrap().spec.ir.clone().unwrap().path;
    let until = std::time::Instant::now() + std::time::Duration::from_secs(20);
    while !matches!(
        cantus_engine::fx::comparatio_store::status(&stored),
        cantus_engine::fx::comparatio_store::RefStatus::Loaded { .. }
    ) {
        assert!(std::time::Instant::now() < until, "the reference is read");
        app.sync_engine();
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    app.sync_engine();
    let spec = app.project().effect(StripId::MASTER, fx).unwrap().spec.clone();
    let refs = spec.refs.as_deref().expect("its list");
    assert_eq!(refs.list.len(), 1);
    assert_eq!(spec.ir.as_deref().unwrap().path, refs.list[0].path);
    let (bytes, hash) = cantus_engine::fx::comparatio_store::fingerprint(&reference).unwrap();
    assert_eq!((refs.list[0].bytes, refs.list[0].hash.clone()), (bytes, hash));

    // No render ever holds the reference: heard, hidden, either blind choice.
    for listen in [1.0, 2.0, 3.0, 4.0] {
        set(&mut app, fx, "listen", listen);
        assert!(
            rendered(&mut app) == dry,
            "a render with listen {listen} is the dry master, to the bit"
        );
    }
    set(&mut app, fx, "listen", 0.0);

    // Live: the mix untouched, to the bit, against the offline render.
    app.seek(Ticks::ZERO);
    app.sync_engine();
    proc_.settle();
    app.toggle_play();
    let mut live = Vec::new();
    play(&mut app, &mut proc_, 3 * SR, &mut live);
    assert!(
        live.iter().zip(&dry).all(|(a, b)| a.to_bits() == b.to_bits()),
        "live, your mix is the render to the bit"
    );
    // The reference heard: matched to the mix's loudness.
    set(&mut app, fx, "listen", 1.0);
    play(&mut app, &mut proc_, 4 * SR, &mut live);
    let heard = lufs(&live, 4.5, 7.0);
    let mix_lufs = lufs(&dry, 4.5, 7.0);
    let file_lufs = {
        let mut m = LoudnessMeter::new(SR as f32);
        m.process(&rl, &rr);
        m.integrated().unwrap()
    };
    eprintln!(
        "mix {mix_lufs:.2} LUFS, reference file {file_lufs:.2}, heard {heard:.2} ({:+.3} LU)",
        heard - mix_lufs
    );
    assert!(file_lufs - mix_lufs > 8.0, "the file is louder");
    assert!(
        (heard - mix_lufs).abs() < 0.1,
        "heard at the mix's loudness: {heard:.3} against {mix_lufs:.3}"
    );
    let span = |x: &[f32]| x[(5 * SR * 2)..(6 * SR * 2)].to_vec();
    assert!(
        span(&live) != span(&dry),
        "it is the reference that is heard, not the mix"
    );
    // Back to the mix: to the bit once the crossfade (30 ms) has run.
    set(&mut app, fx, "listen", 0.0);
    play(&mut app, &mut proc_, SR, &mut live);
    app.toggle_play();
    let from = (7 * SR + SR / 10) * 2;
    assert!(
        live[from..].iter().zip(&dry[from..]).all(|(a, b)| a.to_bits() == b.to_bits()),
        "back on your mix, to the bit"
    );

    // A second reference added and compared; undone, the first again.
    let (sl, sr) = steady(0x1234_5678, 0.3, 0.2, SR);
    let second = wav("Album master.wav", &sl, &sr);
    {
        let mut host = views::fx::host::DawHost::new(&mut app, StripId::MASTER, fx, "comparatio");
        assert!(views::fx::comparatio::add_references(&mut host, &[second]));
    }
    let s = app.project().effect(StripId::MASTER, fx).unwrap().spec.clone();
    assert_eq!(s.refs.as_deref().unwrap().list.len(), 2);
    assert!(s.ir.as_deref().unwrap().path.ends_with("Album master.wav"));
    app.undo();
    let s = app.project().effect(StripId::MASTER, fx).unwrap().spec.clone();
    assert_eq!(s.refs.as_deref().unwrap().list.len(), 1, "one step");
    assert!(s.ir.as_deref().unwrap().path.ends_with("Finished master.wav"));
}
