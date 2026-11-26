//! Chorda's round-42 voicing pass end to end (`chorda-c17`, `DSP_VERSION` 15, ADR 0098's
//! sixth amendment), on the boot path: a Chorda track added the way the instrument picker adds
//! one plays the new default through the real engine's offline render — the action's knock
//! heard under a top key's fundamental, as the recorded grands carry it — and its *Colour*
//! knob, turned the way the editor turns it (one `SetInstrumentValue`), moves the middle's
//! long-term spectrum by the measured shape; undone, the render returns to the bit. Its own
//! test binary: it points `CANTUS_DATA_DIR` at a temporary folder.

use cantus_core::*;
use cantus_engine::{render, EngineHandle, RenderOptions};
use cantus_ui::app::Files;
use cantus_ui::picker::{PickChoice, PickPurpose};
use cantus_ui::CantusApp;
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

fn mono(app: &CantusApp) -> Vec<f64> {
    let p = app.project().clone();
    let r = render(
        Arc::new(Snapshot::compile(&p, 1)),
        &RenderOptions {
            tail_seconds: 0.2,
            ..Default::default()
        },
    )
    .unwrap();
    r.samples
        .chunks(2)
        .map(|c| c[0] as f64 + c[1] as f64)
        .collect()
}

/// Power in `[lo, hi)` Hz of `x[a..b]` (a Hann-windowed DFT on a 2 Hz grid).
fn band(x: &[f64], a: usize, b: usize, lo: f64, hi: f64) -> f64 {
    let fs = 48_000.0;
    let n = b - a;
    let mut p = 0.0;
    let mut f = lo;
    while f < hi {
        let w = std::f64::consts::TAU * f / fs;
        let (mut re, mut im) = (0.0, 0.0);
        for i in 0..n {
            let h = 0.5 - 0.5 * (std::f64::consts::TAU * i as f64 / n as f64).cos();
            re += x[a + i] * h * (w * i as f64).cos();
            im -= x[a + i] * h * (w * i as f64).sin();
        }
        p += re * re + im * im;
        f += 2.0;
    }
    p
}

#[test]
fn a_new_chorda_knocks_like_a_grand_and_its_colour_knob_moves_the_middle() {
    let files = Files::read(data(), Some(cantus_ui::i18n::Locale::En));
    let ctx = egui::Context::default();
    let (engine, _proc) = EngineHandle::manual(48_000.0);
    let mut app = CantusApp::assemble(&ctx, files, None, engine, None, None, None, false);
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
    let spec = app
        .project()
        .tracks
        .iter()
        .find(|t| t.id == track)
        .and_then(|t| t.instrument.native())
        .expect("its spec")
        .clone();
    assert_eq!(spec.get("colour"), 0.0, "a new Chorda plays its own colour");
    // C8 alone at mf, then middle C held.
    let mut clip = app.bus.new_clip("Keys", Ticks::ZERO, Ticks::beats(16));
    clip.notes
        .push(app.bus.new_note(Ticks::ZERO, Ticks::beats(2), 108, 75));
    clip.notes
        .push(app.bus.new_note(Ticks::beats(4), Ticks::beats(6), 60, 75));
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
    let beat = 60.0 / app.project().tempo_bpm;
    let fs = 48_000.0;
    let own = mono(&app);
    let on = (0..(0.2 * fs) as usize)
        .find(|i| own[*i].abs() > 1e-4)
        .expect("C8 sounds");
    // The knock under C8's fundamental (4.19 kHz): 0.5–2.4 kHz over 5–60 ms against the note's
    // whole first 60 ms. The recorded grands: −13 … −7 dB (`docs/round42/chorda-c17.md`);
    // candidate 16 sounded none of it (−30 dB and under).
    let ms = |t: f64| on + (t * fs / 1000.0) as usize;
    let knock = band(&own, ms(5.0), ms(60.0), 500.0, 2_400.0);
    let note = band(&own, ms(0.0), ms(60.0), 30.0, 12_000.0);
    let rel = 10.0 * (knock / note).log10();
    assert!(rel > -22.0 && rel < -4.0, "C8's knock {rel:.1} dB under its note");

    // Colour 1, as the editor's knob sets it.
    assert!(app.exec(Command::SetInstrumentValue {
        track,
        key: "colour".to_string(),
        value: 1.0,
    }));
    let coloured = mono(&app);
    let c4 = (4.0 * beat * fs) as usize;
    let (a, b) = (c4 + (0.1 * fs) as usize, c4 + (1.3 * fs) as usize);
    for (i, &hz) in cantus_dsp::chorda::colour::CENTRES.iter().enumerate() {
        let want = cantus_dsp::chorda::colour::SHAPE[i];
        let (lo, hi) = (hz / 2f64.sqrt(), hz * 2f64.sqrt());
        let got = 10.0 * (band(&coloured, a, b, lo, hi) / band(&own, a, b, lo, hi)).log10();
        assert!(
            (got - want).abs() < 2.5,
            "{hz} Hz octave moved {got:.1} dB, the shape asks {want:.1}"
        );
    }
    app.undo();
    assert!(mono(&app) == own, "undone, the colour left a trace");
}
