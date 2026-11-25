//! r42 fix2-arcus (review finding 9) — Arcus reached as a user reaches it, with a tuning and
//! the notes' own articulations: added through `CantusApp::assemble` and `apply_pick`, the
//! track given a tuning of its own (Werckmeister III in E flat at A415) by its command, a
//! phrase inserted, two of its notes marked staccato and pizzicato by `SetNoteArt`, and the
//! whole played through the real engine wiring (a processor driven block by block): the
//! bowed note sounds its table's pitch within ADR 0155's cent, and each marked note sounds
//! otherwise than unmarked.

use cantus_core::note_art::{Articulation, NoteArt};
use cantus_core::tuning::{Temperament, TrackTuning, Tuning, TuningSystem};
use cantus_core::{Command, InstrumentKind, Project, Ticks, PPQ};
use cantus_engine::EngineHandle;
use cantus_ui::app::Files;
use cantus_ui::i18n::Locale;
use cantus_ui::picker::{PickChoice, PickPurpose};
use cantus_ui::CantusApp;

fn app_with_song(data: &std::path::Path) -> (CantusApp, cantus_engine::Processor) {
    std::env::set_var("CANTUS_DATA_DIR", data);
    std::fs::write(
        data.join("settings.json"),
        r#"{"audio_setup":{"setup_done":true}}"#,
    )
    .unwrap();
    let files = Files::read(data, Some(Locale::En));
    let ctx = egui::Context::default();
    let (engine, proc_) = EngineHandle::manual(48_000.0);
    let song = Project::new("arcus-tuning-art");
    let app = CantusApp::assemble(&ctx, files, Some(song), engine, None, None, None, false);
    (app, proc_)
}

/// `seconds` of the song from its start, mono, through the processor in blocks of 512.
fn play(app: &mut CantusApp, proc_: &mut cantus_engine::Processor, seconds: usize) -> Vec<f32> {
    app.sync_engine();
    app.seek(Ticks::ZERO);
    app.toggle_play();
    app.sync_engine();
    let frames = 48_000 * seconds;
    let mut out = Vec::with_capacity(frames * 2);
    let since = std::time::Instant::now();
    while out.len() < frames * 2 {
        let mut buf = vec![0f32; 512 * 2];
        proc_.process(&mut buf, 2);
        out.extend_from_slice(&buf);
        assert!(since.elapsed().as_secs() < 240, "the phrase never finished");
    }
    app.toggle_play();
    app.sync_engine();
    out.chunks_exact(2).map(|f| 0.5 * (f[0] + f[1])).collect()
}

/// The partial near `near` Hz over `from…to` s: heterodyned, two boxcars of four periods, the
/// phase's slope (as `arcus_upper_e2e`).
fn measure_hz(x: &[f32], near: f64, from: f64, to: f64) -> f64 {
    let sr = 48_000.0f64;
    let len = ((4.0 * sr / near).round() as usize).max(4);
    let w = std::f64::consts::TAU * near / sr;
    let (mut re, mut im) = (Vec::with_capacity(x.len()), Vec::with_capacity(x.len()));
    for (n, v) in x.iter().enumerate() {
        let (s, c) = cantus_dsp::math::sin_cos(w * n as f64);
        re.push(*v as f64 * c);
        im.push(-(*v as f64) * s);
    }
    let boxcar = |v: &[f64]| -> Vec<f64> {
        let mut out = vec![0.0; v.len()];
        let mut acc = 0.0;
        for i in 0..v.len() {
            acc += v[i];
            if i >= len {
                acc -= v[i - len];
            }
            out[i] = acc / len as f64;
        }
        out
    };
    let (re, im) = (boxcar(&boxcar(&re)), boxcar(&boxcar(&im)));
    let (a, b) = ((from * sr) as usize, ((to * sr) as usize).min(x.len()));
    let (mut ts, mut ph) = (Vec::new(), Vec::new());
    let (mut last, mut unwrap) = (0.0f64, 0.0f64);
    for i in (a..b).step_by(16) {
        let p = cantus_dsp::math::atan2(im[i], re[i]);
        if !ts.is_empty() {
            let d = p - last;
            if d > std::f64::consts::PI {
                unwrap -= std::f64::consts::TAU;
            } else if d < -std::f64::consts::PI {
                unwrap += std::f64::consts::TAU;
            }
        }
        last = p;
        ts.push(i as f64 / sr);
        ph.push(p + unwrap);
    }
    let n = ts.len() as f64;
    let (mt, mp) = (ts.iter().sum::<f64>() / n, ph.iter().sum::<f64>() / n);
    let (mut num, mut den) = (0.0, 0.0);
    for (t, p) in ts.iter().zip(&ph) {
        num += (t - mt) * (p - mp);
        den += (t - mt) * (t - mt);
    }
    near + num / den / std::f64::consts::TAU
}

/// The energy of `x` over `from…to` s.
fn energy(x: &[f32], from: f64, to: f64) -> f64 {
    let (a, b) = ((from * 48_000.0) as usize, (to * 48_000.0) as usize);
    x[a..b].iter().map(|v| (*v as f64) * (*v as f64)).sum()
}

/// One session from a fresh app: Arcus picked, its track's own tuning (Werckmeister III in E
/// flat at A415) by its command, the phrase inserted, and — `marked` — its second note
/// marked staccato and its third pizzicato by `SetNoteArt`; six seconds played. The output
/// and the track's table.
fn session(marked: bool) -> (Vec<f32>, cantus_core::tuning::TuningTable) {
    let data = tempfile::tempdir().unwrap();
    let (mut app, mut proc_) = app_with_song(data.path());
    let purpose = PickPurpose::AddInstrument { index: None };
    app.open_picker(purpose.clone());
    app.apply_pick(purpose, PickChoice::Instrument(InstrumentKind::BowedString));
    app.picker = None;
    let track = app.project().tracks.last().unwrap().id;
    let werck = Tuning {
        a4_hz: 415.0,
        system: TuningSystem::Temperament {
            id: Temperament::WerckmeisterIii,
            key: 3,
            follow_song_key: false,
        },
        map: None,
    };
    assert!(app.exec(Command::SetTrackTuning {
        track,
        tuning: TrackTuning::Own {
            tuning: werck.clone()
        },
    }));
    // A held bowed note (0 … 1.6 s), then the notes to be marked staccato (2 s) and
    // pizzicato (3.5 s), at 60 BPM.
    let mut p = app.project().clone();
    let mut c = p.make_clip("Phrase", Ticks::ZERO, Ticks::beats(6));
    let held = p.make_note(Ticks::ZERO, Ticks(PPQ + PPQ * 3 / 5), 50, 90);
    let short = p.make_note(Ticks::beats(2), Ticks(PPQ), 55, 90);
    let plucked = p.make_note(Ticks(PPQ * 7 / 2), Ticks(PPQ), 57, 90);
    let ids = (short.id, plucked.id);
    c.notes.extend([held, short, plucked]);
    let clip = c.id;
    assert!(app.exec(Command::InsertClip {
        track,
        clip: Box::new(c)
    }));
    assert!(app.exec(Command::SetTempo { bpm: 60.0 }));
    if marked {
        // The two notes marked, by the command the piano roll sends.
        let art = |a| {
            Some(NoteArt {
                articulation: a,
                bow: None,
                start: None,
                part: None,
                string: None,
            })
        };
        assert!(app.exec(Command::SetNoteArt {
            track,
            clip,
            arts: vec![
                (ids.0, art(Articulation::Staccato)),
                (ids.1, art(Articulation::Pizzicato)),
            ],
        }));
    }
    let out = play(&mut app, &mut proc_, 6);
    assert!(out.iter().all(|x| x.is_finite()));
    (out, werck.offsets())
}

#[test]
fn arcus_plays_its_track_s_tuning_and_its_notes_articulations_through_the_app() {
    let (plain, table) = session(false);
    // The bowed note sounds its table's pitch.
    let want = 440.0 * cantus_dsp::math::exp2((50.0 + f64::from(table.at(50)) - 69.0) / 12.0);
    let err = 1200.0 * cantus_dsp::math::log2(measure_hz(&plain, want, 0.6, 1.5) / want);
    println!("the held D3 in Werckmeister III at A415: {err:+.3} ct from its table");
    assert!(err.abs() <= 1.0, "{err:+.3} ct");
    let (marked, _) = session(true);
    // The unmarked note is the same; the marked ones are not.
    let first: f64 = plain[..72_000]
        .iter()
        .zip(&marked[..72_000])
        .map(|(a, b)| ((a - b) as f64).powi(2))
        .sum();
    assert!(
        first <= 1e-3 * energy(&plain, 0.0, 1.5),
        "the unmarked note moved: {first}"
    );
    for (name, from, to) in [("staccato", 2.0, 3.0), ("pizzicato", 3.5, 4.5)] {
        let diff: f64 = {
            let (a, b) = ((from * 48_000.0) as usize, (to * 48_000.0) as usize);
            plain[a..b]
                .iter()
                .zip(&marked[a..b])
                .map(|(x, y)| ((x - y) as f64).powi(2))
                .sum()
        };
        let base = energy(&plain, from, to);
        println!(
            "{name}: the difference {:.1} % of the unmarked note's energy",
            100.0 * diff / base
        );
        assert!(
            diff > 0.1 * base,
            "{name} changed too little: {diff} of {base}"
        );
    }
}
