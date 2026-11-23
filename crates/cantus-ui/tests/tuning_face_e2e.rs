//! The song's tuning end to end (round 42, ADR 0155's surface), on the boot path: an app
//! made by `CantusApp::assemble` with the real engine wiring, a Chorda track added as the
//! picker adds one, and
//!
//! - **a Scala file from outside bytes** — `marvel12.scl` of Surge's MIT `tuning-library`
//!   test set (`research/tuning-library-tests/`) — read through the Tuning window's import
//!   action into one `SetProjectTuning`, the song keeping the file's 12 degrees as written;
//! - **Chorda plays it**: each key's measured pitch moves from its equal-temperament pitch by
//!   the table every unit reads (`Tuning::offsets()`), within 0.1 cent (Chorda's own stretch
//!   is in both renders, so it cancels);
//! - **exported and read back**: the `.scl` the window writes holds the same cents (1e-9);
//! - **undone and redone**: the import is one step with its exact inverse;
//! - the window's summary and the top bar's line say it, and say nothing at the default.
//!
//! Honest note: until the `tuning` builder's `Tuning::offsets()` fills Scala scales, the
//! table is the diapason's (flat) and the measured move is 0; the comparison is the same
//! one, so at the merge it measures the scale. Its own test binary: it points
//! `CANTUS_DATA_DIR` at a temporary folder. r42:tuning-face

use cantus_core::tuning::{ScaleDegree, TuningSystem};
use cantus_core::*;
use cantus_engine::{render, EngineHandle, RenderOptions};
use cantus_ui::app::Files;
use cantus_ui::picker::{PickChoice, PickPurpose};
use cantus_ui::views::tuning::{self, Target};
use cantus_ui::CantusApp;
use std::f64::consts::TAU;
use std::path::{Path, PathBuf};
use std::sync::Arc;

const FS: f64 = 48_000.0;
/// The keys measured: A4 (the diapason's), C#4 and E4 (marvel's 115.7 and 383.8 cents).
const KEYS: [u8; 3] = [69, 61, 64];

fn data() -> &'static Path {
    static DIR: std::sync::OnceLock<tempfile::TempDir> = std::sync::OnceLock::new();
    DIR.get_or_init(|| {
        let d = tempfile::tempdir().unwrap();
        std::env::set_var("CANTUS_DATA_DIR", d.path());
        d
    })
    .path()
}

fn outside(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../research/tuning-library-tests")
        .join(name)
}

/// The phase-advance frequency of the partial near `hz` over `x[a..b)` (frames `len` long,
/// a quarter apart): the heterodyned signal's mean rotation, as `chorda_tune_e2e` measures.
fn partial(x: &[f64], hz: f64, a: usize, b: usize, len: usize) -> f64 {
    let hop = len / 4;
    let w = TAU * hz / FS;
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
    let (mut ar, mut ai) = (0.0, 0.0);
    let mut s = a;
    let mut prev = frame(s);
    while s + hop + len <= b {
        let next = frame(s + hop);
        ar += next.0 * prev.0 + next.1 * prev.1;
        ai += next.1 * prev.0 - next.0 * prev.1;
        prev = next;
        s += hop;
    }
    hz + ai.atan2(ar) * FS / (TAU * hop as f64)
}

/// Each key of [`KEYS`]'s fundamental as the song's Chorda renders now, Hz.
fn measured(app: &CantusApp) -> Vec<f64> {
    let mut p = app.project().clone();
    p.tracks.retain(|t| {
        t.instrument
            .native()
            .is_some_and(|n| n.kind == InstrumentKind::Chorda)
    });
    let r = render(
        Arc::new(Snapshot::compile(&p, 1)),
        &RenderOptions {
            tail_seconds: 0.2,
            ..Default::default()
        },
    )
    .unwrap();
    let mono: Vec<f64> = r
        .samples
        .chunks(2)
        .map(|c| f64::from(c[0]) + f64::from(c[1]))
        .collect();
    let beat = 60.0 / p.tempo_bpm;
    KEYS.iter()
        .enumerate()
        .map(|(i, k)| {
            let want = 440.0 * 2f64.powf((f64::from(*k) - 69.0) / 12.0);
            let at = (4.0 * i as f64 * beat * FS) as usize;
            let (a, b) = (at + (0.15 * FS) as usize, at + (1.15 * FS) as usize);
            let len = ((6.0 * FS / want) as usize).max(64);
            partial(&mono, want, a, b, len)
        })
        .collect()
}

#[test]
fn a_scala_file_imported_through_the_window_is_what_chorda_plays_and_exports() {
    let files = Files::read(data(), Some(cantus_ui::i18n::Locale::En));
    let ctx = egui::Context::default();
    let (engine, _proc) = EngineHandle::manual(48_000.0);
    let mut app = CantusApp::assemble(&ctx, files, None, engine, None, None, None, false);
    // The boot loaded MTS-ESP (or said why not): the window has its link.
    assert!(
        app.tuning.mts.is_some(),
        "the boot path loads MTS-ESP's link"
    );
    let reference = cantus_engine::inst::chorda::reference();
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
    let mut clip = app.bus.new_clip("Keys", Ticks::ZERO, Ticks::beats(16));
    for (i, k) in KEYS.iter().enumerate() {
        let n = app
            .bus
            .new_note(Ticks::beats(4 * i as i64), Ticks::beats(3), *k, 80);
        clip.notes.push(n);
    }
    assert!(app.exec(Command::InsertClip {
        track,
        clip: Box::new(clip),
    }));
    assert!(tuning::words::project_summary(app.project()).is_none());
    let equal = measured(&app);
    // A4 is A4: the piano is in tune before anything is imported.
    assert!(
        (1200.0 * (equal[0] / 440.0).log2()).abs() < 3.0,
        "A4 near 440: {}",
        equal[0]
    );

    // The window's import action, on outside bytes.
    tuning::open(&mut app, Target::Project);
    let said = tuning::io::import(&mut app, Target::Project, &outside("marvel12.scl"), None)
        .expect("marvel12.scl reads");
    assert!(said.contains("marvel12.scl"), "{said}");
    let tu = app.project().tuning.clone();
    let TuningSystem::Scale(scale) = &tu.system else {
        panic!("a scale: {tu:?}");
    };
    assert_eq!(scale.description, "Marvel[12] hobbit in 197-tET");
    assert_eq!(scale.degrees.len(), 12);
    assert_eq!(scale.degrees[0], ScaleDegree::Cents(115.73604));
    assert_eq!(scale.degrees[11], ScaleDegree::Ratio(2, 1));
    assert_eq!(tu.a4_hz, 440.0, "no keyboard map: the diapason stays");
    let summary = tuning::words::project_summary(app.project()).expect("said");
    assert_eq!(summary, "Marvel[12] hobbit in 197-tET \u{b7} A440");

    // Chorda plays the table every unit reads.
    let table = tu.offsets();
    let tuned = measured(&app);
    for (i, k) in KEYS.iter().enumerate() {
        let moved = 1200.0 * (tuned[i] / equal[i]).log2();
        let want = 100.0 * f64::from(table.at(*k));
        eprintln!(
            "key {k}: equal {:.3} Hz, tuned {:.3} Hz, moved {moved:+.3} c, table {want:+.3} c",
            equal[i], tuned[i]
        );
        assert!(
            (moved - want).abs() < 0.1,
            "key {k}: moved {moved} c, table {want} c"
        );
    }

    // Exported, and read back to the same cents.
    let out = tempfile::tempdir().unwrap();
    let files = tuning::io::export(&app, Target::Project, &out.path().join("song")).unwrap();
    assert_eq!(files.len(), 1, "no keyboard map: a .scl alone");
    let back = tuning::io::read_scl(&std::fs::read(&files[0]).unwrap()).unwrap();
    assert_eq!(back.degrees.len(), 12);
    for (a, b) in back.degrees.iter().zip(&scale.degrees) {
        let (a, b) = (
            tuning::words::degree_cents(a),
            tuning::words::degree_cents(b),
        );
        assert!((a - b).abs() < 1e-9, "{a} vs {b}");
    }

    // One step, its exact inverse.
    app.undo();
    assert!(app.project().tuning.is_default());
    app.redo();
    assert_eq!(app.project().tuning, tu);
}

#[test]
fn a_file_the_scala_grammar_refuses_is_refused_with_its_reason_and_changes_nothing() {
    let files = Files::read(data(), Some(cantus_ui::i18n::Locale::En));
    let ctx = egui::Context::default();
    let (engine, _proc) = EngineHandle::manual(48_000.0);
    let mut app = CantusApp::assemble(&ctx, files, None, engine, None, None, None, false);
    let before = app.project().tuning.clone();
    for bad in [
        "bad/missingnote.scl",
        "bad/badnote.scl",
        "bad/blanknote.scl",
    ] {
        let e = tuning::io::import(&mut app, Target::Project, &outside(bad), None).expect_err(bad);
        assert!(e.starts_with("Not read: "), "{bad}: {e}");
        assert_eq!(app.project().tuning, before, "{bad}");
    }
}

/// The owner's ask (2026-11-22): only Chorda in another tuning, the song left as it is.
/// Through Chorda's Temperament card's action, *Werckmeister III* in E: one undo step that
/// gives the Chorda track a tuning of its own; Chorda plays its table (measured, against its
/// own equal-temperament render), while a synthesiser on another track keeps 12-TET at 440
/// (its table equal to the bit, and its render the same to the bit as before Chorda's
/// tuning), and the project's tuning stays the default. Undone, all of it goes.
#[test]
fn chordas_own_tuning_changes_only_chorda() {
    let files = Files::read(data(), Some(cantus_ui::i18n::Locale::En));
    let ctx = egui::Context::default();
    let (engine, _proc) = EngineHandle::manual(48_000.0);
    let mut app = CantusApp::assemble(&ctx, files, None, engine, None, None, None, false);
    cantus_engine::inst::chorda::ensure(cantus_engine::inst::chorda::reference(), 48_000.0);
    app.apply_pick(
        PickPurpose::AddInstrument { index: None },
        PickChoice::Instrument(InstrumentKind::Chorda),
    );
    let chorda = app
        .project()
        .tracks
        .iter()
        .find(|t| {
            t.instrument
                .native()
                .is_some_and(|n| n.kind == InstrumentKind::Chorda)
        })
        .map(|t| t.id)
        .expect("Chorda");
    app.apply_pick(
        PickPurpose::AddInstrument { index: None },
        PickChoice::Instrument(InstrumentKind::Analog),
    );
    let synth = app
        .project()
        .tracks
        .iter()
        .find(|t| {
            t.instrument
                .native()
                .is_some_and(|n| n.kind == InstrumentKind::Analog)
        })
        .map(|t| t.id)
        .expect("Calor");
    let others: Vec<TrackId> = app
        .project()
        .tracks
        .iter()
        .map(|t| t.id)
        .filter(|t| *t != chorda && *t != synth)
        .collect();
    for id in others {
        assert!(app.exec(Command::RemoveTrack { id }));
    }
    for t in [chorda, synth] {
        let clips: Vec<ClipId> = app
            .project()
            .track(t)
            .unwrap()
            .clips
            .iter()
            .map(|c| c.id)
            .collect();
        for clip in clips {
            assert!(app.exec(Command::RemoveClip { track: t, clip }));
        }
    }
    let mut clip = app.bus.new_clip("Keys", Ticks::ZERO, Ticks::beats(16));
    for (i, k) in KEYS.iter().enumerate() {
        let n = app
            .bus
            .new_note(Ticks::beats(4 * i as i64), Ticks::beats(3), *k, 80);
        clip.notes.push(n);
    }
    assert!(app.exec(Command::InsertClip {
        track: chorda,
        clip: Box::new(clip)
    }));
    let mut a4 = app.bus.new_clip("A", Ticks::ZERO, Ticks::beats(4));
    a4.notes
        .push(app.bus.new_note(Ticks::ZERO, Ticks::beats(3), 69, 100));
    assert!(app.exec(Command::InsertClip {
        track: synth,
        clip: Box::new(a4)
    }));
    let synth_alone = |app: &CantusApp| {
        let mut solo = app.project().clone();
        solo.tracks.retain(|t| t.id == synth);
        render(
            Arc::new(Snapshot::compile(&solo, 1)),
            &RenderOptions {
                tail_seconds: 0.0,
                ..Default::default()
            },
        )
        .unwrap()
        .samples
    };
    let synth_before = synth_alone(&app);
    let equal = measured(&app);

    // The card's action: the preset in E.
    let spec = app
        .project()
        .track(chorda)
        .unwrap()
        .instrument
        .native()
        .unwrap()
        .clone();
    let preset = tuning::chorda::PRESETS
        .iter()
        .find(|p| p.name == "tuning.temper.werckmeister_iii")
        .unwrap();
    let before = app.project().clone();
    assert!(app.exec(tuning::chorda::preset_command(&spec, chorda, preset, 4)));
    assert!(
        app.project().tuning.is_default(),
        "the song keeps its tuning"
    );
    let mine = tuning::tuning_of(&app, Target::Track(chorda));
    assert!(matches!(
        mine.system,
        TuningSystem::Temperament {
            key: 4,
            follow_song_key: false,
            ..
        }
    ));
    assert_eq!(
        tuning::table_of(&app, Target::Track(synth)),
        cantus_core::tuning::TuningTable::EQUAL
    );
    assert_eq!(tuning::chorda::a4_of(&app, synth), tuning::chorda::A4::Knob);

    // Chorda plays its own table.
    let table = mine.offsets();
    let tuned = measured(&app);
    for (i, k) in KEYS.iter().enumerate() {
        let moved = 1200.0 * (tuned[i] / equal[i]).log2();
        let want = 100.0 * f64::from(table.at(*k));
        assert!(
            (moved - want).abs() < 0.1,
            "Chorda key {k}: moved {moved} c, table {want} c"
        );
    }

    // The synthesiser beside it is as it was, to the bit.
    assert!(
        synth_alone(&app) == synth_before,
        "the other track left in 12-TET at 440"
    );

    // Undone: the track follows the project again, its stretch and unisons as they were.
    app.undo();
    assert_eq!(
        app.project().track(chorda).unwrap().tuning,
        before.track(chorda).unwrap().tuning
    );
    assert_eq!(
        app.project().track(chorda).unwrap().instrument,
        before.track(chorda).unwrap().instrument
    );
}
