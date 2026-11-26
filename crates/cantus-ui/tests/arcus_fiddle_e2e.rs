//! r42 fiddle (ADR 0164) — the country fiddle reached as a user reaches it: Arcus added
//! through `CantusApp::assemble` and the picker, the *Fiddle* preset applied from the preset
//! browser, a clip inserted with a double stop (the open A and F#5 struck together), then a
//! crossing (the open E, then B4 on the A string), and the whole played through the real engine
//! wiring (a processor driven block by block):
//!
//! - the double stop sounds **both** notes under one bow — with `bow.pair` turned off by its
//!   command, the second note slurs out of the first as before and the open A is gone;
//! - F#5 is tuned most of the way to the just sixth over the open A (`dstop.just` 0.6);
//! - the open strings ring in sympathy after the bow lifts — `sym` off by its command, the
//!   tail is quieter.

use cantus_core::note_art::{Articulation, Bow, NoteArt};
use cantus_core::{Command, InstrumentKind, Project, Ticks, PPQ};
use cantus_engine::EngineHandle;
use cantus_ui::app::Files;
use cantus_ui::i18n::Locale;
use cantus_ui::picker::{PickChoice, PickPurpose};
use cantus_ui::preset_audition::Target;
use cantus_ui::CantusApp;

const SR: f64 = 48_000.0;

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
    let song = Project::new("arcus-fiddle");
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

/// The power of the partial at `hz` over `from…to` s (a Hann-windowed single bin).
fn bin(x: &[f32], hz: f64, from: f64, to: f64) -> f64 {
    let (a, b) = ((from * SR) as usize, ((to * SR) as usize).min(x.len()));
    let n = (b - a) as f64;
    let (mut re, mut im) = (0.0, 0.0);
    for (k, v) in x[a..b].iter().enumerate() {
        let w = 0.5 - 0.5 * cantus_dsp::math::cos(std::f64::consts::TAU * k as f64 / n);
        let (s, c) = cantus_dsp::math::sin_cos(std::f64::consts::TAU * hz * k as f64 / SR);
        re += w * *v as f64 * c;
        im -= w * *v as f64 * s;
    }
    (re * re + im * im) / (n * n)
}

/// The frequency of the strongest partial within ±25 cents of `near` over `from…to` s, by a
/// fine search of [`bin`] (0.1-cent steps).
fn peak_hz(x: &[f32], near: f64, from: f64, to: f64) -> f64 {
    let mut best = (0.0, near);
    let mut c = -25.0;
    while c <= 25.0 {
        let f = near * cantus_dsp::math::exp2(c / 1200.0);
        let p = bin(x, f, from, to);
        if p > best.0 {
            best = (p, f);
        }
        c += 0.1;
    }
    best.1
}

fn energy(x: &[f32], from: f64, to: f64) -> f64 {
    let (a, b) = ((from * SR) as usize, (to * SR) as usize);
    x[a..b].iter().map(|v| (*v as f64) * (*v as f64)).sum()
}

/// One session: Arcus picked, the *Fiddle* preset applied, the clip inserted (60 BPM: the
/// double stop 0 … 1.5 s, the open E 2 … 2.5 s, B4 2.5 … 3 s), `off` the fiddle's keys set
/// to 0 by their command; six seconds played.
fn session(off: &[&str]) -> Vec<f32> {
    let data = tempfile::tempdir().unwrap();
    let (mut app, mut proc_) = app_with_song(data.path());
    let purpose = PickPurpose::AddInstrument { index: None };
    app.open_picker(purpose.clone());
    app.apply_pick(purpose, PickChoice::Instrument(InstrumentKind::BowedString));
    app.picker = None;
    let track = app.project().tracks.last().unwrap().id;
    app.open_preset_browser(Target::Track(track));
    assert!(app.apply_preset_id("arcus.preset.violin_fiddle"));
    app.close_preset_browser();
    for key in off {
        assert!(app.exec(Command::SetInstrumentValue {
            track,
            key: (*key).into(),
            value: 0.0,
        }));
    }
    let mut p = app.project().clone();
    let mut c = p.make_clip("Fiddle", Ticks::ZERO, Ticks::beats(4));
    let half = Ticks(PPQ / 2);
    let notes = [
        p.make_note(Ticks::ZERO, Ticks(PPQ * 3 / 2), 69, 92),
        p.make_note(Ticks::ZERO, Ticks(PPQ * 3 / 2), 78, 92),
        p.make_note(Ticks::beats(2), half, 76, 92),
        p.make_note(Ticks::beats(2) + half, half, 71, 92),
    ];
    let ids: Vec<_> = notes.iter().map(|n| n.id).collect();
    c.notes.extend(notes);
    let clip = c.id;
    assert!(app.exec(Command::InsertClip {
        track,
        clip: Box::new(c)
    }));
    assert!(app.exec(Command::SetTempo { bpm: 60.0 }));
    let art = |b| {
        Some(NoteArt {
            articulation: Articulation::Sustain,
            bow: Some(b),
            start: None,
            part: None,
            string: None,
        })
    };
    assert!(app.exec(Command::SetNoteArt {
        track,
        clip,
        arts: vec![
            (ids[0], art(Bow::Down)),
            (ids[1], art(Bow::Down)),
            (ids[2], art(Bow::Up)),
            (ids[3], art(Bow::Down)),
        ],
    }));
    let out = play(&mut app, &mut proc_, 6);
    assert!(out.iter().all(|x| x.is_finite()));
    out
}

#[test]
fn the_fiddle_bows_two_strings_with_one_bow_and_rings_through_the_app() {
    let on = session(&[]);
    let a4 = 440.0;
    let fs5 = 440.0 * cantus_dsp::math::exp2(9.0 / 12.0);
    // Both notes of the double stop sound.
    let (pa, pf) = (bin(&on, a4, 0.6, 1.4), bin(&on, fs5, 0.6, 1.4));
    let solo = session(&["bow.pair"]);
    let (sa, sf) = (bin(&solo, a4, 0.6, 1.4), bin(&solo, fs5, 0.6, 1.4));
    let db = |x: f64, y: f64| 10.0 * (x / y.max(1e-30)).log10();
    // (The weaker of the two against the stronger: with one bow both sound; without, the
    // second note slurs out of the first, whichever the engine hands in first.)
    let both = db(pa.min(pf), pa.max(pf));
    let one = db(sa.min(sf), sa.max(sf));
    println!("the double stop's weaker note: {both:.1} dB under the stronger with one bow on two strings, {one:.1} dB without");
    assert!(both > -12.0, "both notes sound: {both:.1} dB");
    assert!(one < both - 20.0, "without the pair one note is gone: {one:.1} dB");
    // F#5 tuned towards the just sixth (5/3, −15.6 cents) by 0.6 over the open A.
    let ct = 1200.0 * cantus_dsp::math::log2(peak_hz(&on, fs5, 0.7, 1.4) / fs5);
    println!("F#5 over the open A: {ct:+.2} cents from tempered (just ×0.6: −9.4)");
    assert!((ct + 0.6 * 15.64).abs() < 2.5, "{ct:+.2} cents");
    // The ring after the last note (3.2 … 4.5 s): the open strings in sympathy.
    let dry = session(&["sym"]);
    let ring = db(energy(&on, 3.2, 4.5), energy(&dry, 3.2, 4.5));
    println!("the tail with the open strings in sympathy: {ring:+.1} dB over none");
    assert!(ring > 2.0, "{ring:+.1} dB");
}
