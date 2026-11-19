//! Chorda's tuning end to end (round 40, the technician's pass, `DSP_VERSION` 10, ADR 0098's
//! fourth amendment): a Chorda track added the way the instrument picker adds one, on the
//! boot path, plays the keys the owner heard out of tune (Bb2, B2, C3) and the worst of
//! candidate 16 (B0, 18 cents sharp) on the curve its keyboard is designed to — measured on
//! the real engine's offline render, each partial against the smooth stretch (the keyboard
//! without its technician's table). Its own test binary: it points `CANTUS_DATA_DIR` at a
//! temporary folder.

use cantus_core::*;
use cantus_dsp::chorda::tables;
use cantus_dsp::chorda::voicing::Voicing;
use cantus_engine::{render, EngineHandle, RenderOptions};
use cantus_ui::app::Files;
use cantus_ui::picker::{PickChoice, PickPurpose};
use cantus_ui::CantusApp;
use std::f64::consts::TAU;
use std::sync::Arc;

fn data() -> &'static std::path::Path {
    static DIR: std::sync::OnceLock<tempfile::TempDir> = std::sync::OnceLock::new();
    DIR.get_or_init(|| {
        let d = tempfile::tempdir().unwrap();
        std::env::set_var("CANTUS_DATA_DIR", d.path());
        d
    })
    .path()
}

/// Partial `hz`'s power-weighted frequency (Hz) over `x`'s samples `[a, b)`: the signal
/// heterodyned to `hz`, averaged over frames `len` long every `hop`, the phase advance of
/// the lag-one product (a beating pair's mean, as the ear's), and its mean power.
fn partial(x: &[f64], fs: f64, hz: f64, a: usize, b: usize, len: usize) -> (f64, f64) {
    let hop = len / 4;
    let w = TAU * hz / fs;
    let frame = |s: usize| {
        let (mut re, mut im) = (0.0, 0.0);
        for i in 0..len {
            let hann = 0.5 - 0.5 * (TAU * (i as f64 + 0.5) / len as f64).cos();
            let ph = w * (s + i) as f64;
            re += x[s + i] * hann * ph.cos();
            im -= x[s + i] * hann * ph.sin();
        }
        (re, im)
    };
    let (mut ar, mut ai, mut p) = (0.0, 0.0, 0.0);
    let mut s = a;
    let mut prev = frame(s);
    while s + hop + len <= b {
        let next = frame(s + hop);
        ar += next.0 * prev.0 + next.1 * prev.1;
        ai += next.1 * prev.0 - next.0 * prev.1;
        p += prev.0 * prev.0 + prev.1 * prev.1;
        prev = next;
        s += hop;
    }
    (hz + ai.atan2(ar) * fs / (TAU * hop as f64), p)
}

#[test]
fn a_new_chorda_plays_the_keys_the_owner_heard_on_their_curve() {
    let files = Files::read(data(), Some(cantus_ui::i18n::Locale::En));
    let ctx = egui::Context::default();
    let (engine, _proc) = EngineHandle::manual(48_000.0);
    let mut app = CantusApp::assemble(&ctx, files, None, engine, None, None, None, false);
    // The keyboard a new Chorda plays carries the technician's table.
    let reference = cantus_engine::inst::chorda::reference();
    assert_eq!(reference.tune, Voicing::default().tune);
    assert_eq!(reference.tune, cantus_dsp::chorda::voicing::tune::TUNE);
    cantus_engine::inst::chorda::ensure(reference, 48_000.0);

    app.apply_pick(
        PickPurpose::AddInstrument { index: None },
        PickChoice::Instrument(InstrumentKind::Chorda),
    );
    let track = app
        .project()
        .tracks
        .iter()
        .find(|t| {
            t.instrument
                .native()
                .is_some_and(|n| n.kind == InstrumentKind::Chorda)
        })
        .map(|t| t.id)
        .expect("the new Chorda track");
    let keys = [46u8, 47, 48, 23];
    let mut clip = app.bus.new_clip("Keys", Ticks::ZERO, Ticks::beats(32));
    for (i, k) in keys.iter().enumerate() {
        let n = app
            .bus
            .new_note(Ticks::beats(8 * i as i64), Ticks::beats(6), *k, 75);
        clip.notes.push(n);
    }
    assert!(app.exec(Command::InsertClip {
        track,
        clip: Box::new(clip),
    }));
    let others: Vec<TrackId> = app
        .project()
        .tracks
        .iter()
        .map(|t| t.id)
        .filter(|t| *t != track)
        .collect();
    for id in others {
        assert!(app.exec(Command::RemoveTrack { id }));
    }
    let p = app.project().clone();
    assert!(p.tempo_points.is_empty(), "one tempo");
    let beat = 60.0 / p.tempo_bpm;
    let r = render(
        Arc::new(Snapshot::compile(&p, 1)),
        &RenderOptions {
            tail_seconds: 0.5,
            ..Default::default()
        },
    )
    .unwrap();
    let fs = 48_000.0;
    let mono: Vec<f64> = r
        .samples
        .chunks(2)
        .map(|c| c[0] as f64 + c[1] as f64)
        .collect();

    // The curve: the keyboard's keys without the table.
    let mut curve = reference;
    curve.tune = [0.0; tables::KEYS];
    let curve = tables::keyboard_for(&curve);
    for (i, k) in keys.iter().enumerate() {
        let key = curve[(*k - tables::LOWEST) as usize];
        let at = (8.0 * i as f64 * beat * fs) as usize;
        let (a, b) = (at + (0.1 * fs) as usize, at + (1.1 * fs) as usize);
        let len = ((4.0 * fs / key.f0()) as usize).max(64);
        // The ear's pitch: partials 1–8 below 5 kHz by their level and the dominance region.
        let (mut sw, mut sc) = (0.0, 0.0);
        let mut line = format!("key {k}:");
        for n in 1..=8u32 {
            let want = key.partial(n);
            if want > 5_000.0 {
                break;
            }
            let (got, pw) = partial(&mono, fs, want, a, b, len);
            let ct = 1200.0 * (got / want).log2();
            let o = (want / 700.0).log2() / 1.5;
            let w = pw.sqrt() * (0.1 + 0.9 * (-0.5 * o * o).exp());
            sw += w;
            sc += w * ct;
            line += &format!(" p{n} {ct:+.1}");
        }
        let pitch = sc / sw;
        eprintln!("{line}; pitch {pitch:+.2} cents");
        assert!(
            pitch.abs() <= 1.5,
            "key {k} sounds {pitch:+.2} cents off its curve through the app"
        );
    }
}
