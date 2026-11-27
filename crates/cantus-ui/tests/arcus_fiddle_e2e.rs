//! r42 fiddle (ADR 0164) — the country fiddle reached as a user reaches it: Arcus added
//! through `CantusApp::assemble` and the picker, the *Fiddle* preset applied from the preset
//! browser, a clip inserted with a double stop (the open A and F#5 struck together), then a
//! crossing (the open E, then D5 on the A string — the open D's octave), and the whole played through the real engine
//! wiring (a processor driven block by block):
//!
//! - the double stop sounds **both** notes under one bow — with `bow.pair` turned off by its
//!   command, the second note slurs out of the first as before and the open A is gone;
//! - F#5 is tuned most of the way to the just sixth over the open A (`dstop.just` 0.6);
//! - the open strings ring in sympathy after the bow lifts — `sym` off by its command, the
//!   tail is quieter;
//! - one close microphone and a fiddle's bridge (`body.mic`, `body.bridge`) lift the bridge
//!   hill's band of the two sides' sum against the classical pair and bridge.

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
/// double stop 0.5 … 2 s, the open E 2.5 … 3 s, D5 3 … 3.5 s), `off` the fiddle's keys set
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
    // Half a beat in: not on the transport's very first sample.
    let mut c = p.make_clip("Fiddle", Ticks(PPQ / 2), Ticks::beats(4));
    let half = Ticks(PPQ / 2);
    let notes = [
        p.make_note(Ticks::ZERO, Ticks(PPQ * 3 / 2), 69, 92),
        p.make_note(Ticks::ZERO, Ticks(PPQ * 3 / 2), 78, 92),
        p.make_note(Ticks::beats(2), half, 76, 92),
        p.make_note(Ticks::beats(2) + half, half, 74, 92),
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
    // Each note's partial where it is (a double stop's just F#5 sits ~9 cents under the
    // tempered one: a 0.8 s window's bin is narrower than that).
    let at = |x: &[f32], f: f64| bin(x, peak_hz(x, f, 1.1, 1.9), 1.1, 1.9);
    let (pa, pf) = (at(&on, a4), at(&on, fs5));
    let solo = session(&["bow.pair"]);
    let (sa, sf) = (at(&solo, a4), at(&solo, fs5));
    let db = |x: f64, y: f64| 10.0 * (x / y.max(1e-30)).log10();
    // (The weaker of the two against the stronger: with one bow both sound; without, the
    // second note slurs out of the first, whichever the engine hands in first.)
    let both = db(pa.min(pf), pa.max(pf));
    let one = db(sa.min(sf), sa.max(sf));
    println!("the double stop's weaker note: {both:.1} dB under the stronger with one bow on two strings, {one:.1} dB without");
    assert!(both > -15.0, "both notes sound: {both:.1} dB");
    assert!(
        one < -20.0 && one < both - 10.0,
        "without the pair one note is gone: {one:.1} dB"
    );
    // F#5 tuned towards the just sixth (5/3, −15.6 cents) by 0.6 over the open A.
    let ct = 1200.0 * cantus_dsp::math::log2(peak_hz(&on, fs5, 1.2, 1.9) / fs5);
    println!("F#5 over the open A: {ct:+.2} cents from tempered (just ×0.6: −9.4)");
    assert!((ct + 0.6 * 15.64).abs() < 2.5, "{ct:+.2} cents");
    // The ring after the last note (3.7 … 5 s): the open strings in sympathy.
    let dry = session(&["sym"]);
    let ring = db(energy(&on, 3.7, 5.0), energy(&dry, 3.7, 5.0));
    println!("the tail with the open strings in sympathy: {ring:+.1} dB over none");
    assert!(ring > 2.0, "{ring:+.1} dB");
    // The close microphone and the fiddle's bridge: the bridge hill's band (1.6–3.2 kHz) over
    // the whole, both sides summed, against the pair through the measured violin's bridge.
    let paired = session(&["body.mic", "body.bridge"]);
    let hill = |x: &[f32]| {
        let band: f64 = (0..33)
            .map(|k| {
                bin(
                    x,
                    1600.0 * cantus_dsp::math::exp2(k as f64 / 32.0),
                    0.6,
                    1.4,
                )
            })
            .sum();
        band / energy(x, 1.1, 1.9)
    };
    let lift = db(hill(&on), hill(&paired));
    println!("the 1.6–3.2 kHz band with one close mic and a fiddle's bridge: {lift:+.1} dB");
    assert!(lift > 3.0, "{lift:+.1} dB");
}

/// The aperiodic share of `x` over `from…to` s: the residual of the signal against itself one
/// period later (the period searched in 0.01-sample steps around `SR / f0`, the delayed copy
/// read by a four-point Lagrange interpolation), over both copies' energy — YIN's d', the
/// noise between the harmonics (a strict period → 0, noise → 1).
fn aperiodic(x: &[f32], f0: f64, from: f64, to: f64) -> f64 {
    let (a, b) = ((from * SR) as usize, (to * SR) as usize);
    let at = |t: f64| {
        let i = t.floor() as usize;
        let u = t - i as f64;
        let p = |k: usize| x[k] as f64;
        let (y0, y1, y2, y3) = (p(i - 1), p(i), p(i + 1), p(i + 2));
        y1 + u
            * ((y2 - y0) / 2.0
                + u * ((2.0 * y0 - 5.0 * y1 + 4.0 * y2 - y3) / 2.0
                    + u * (3.0 * (y1 - y2) + y3 - y0) / 2.0))
    };
    let t0 = SR / f0;
    let mut best = f64::MAX;
    let mut k = -50;
    while k <= 50 {
        let t = t0 * (1.0 + k as f64 * 1e-4);
        let (mut d, mut e) = (0.0, 0.0);
        for (n, v) in x.iter().enumerate().take(b).skip(a) {
            let (p, q) = (*v as f64, at(n as f64 + t));
            d += (p - q) * (p - q);
            e += p * p + q * q;
        }
        best = best.min(d / e.max(1e-30));
        k += 1;
    }
    best
}

/// r42 fiddle — the bite (`bow.bite`), moved through the app by its command as the knob, an
/// automation lane or a learned controller moves it: one held B4 at an ordinary dynamic
/// (velocity 76) on the *Fiddle* preset, its vibrato off so the period holds still. The noise
/// between the harmonics rises with every step of the bite (0 → 1/3 → 2/3 → 1), and the bite
/// is a key a lane, a learned controller and a modulator may drive.
///
/// (r42 integration: each step is heard at four seeds — the `seed` knob, set by its command —
/// and their noise averaged. At bite 1/3 an mf note gets a quarter of the grit, +0.6 dB of the
/// hair's noise, which moves the noise between the harmonics by about +0.4 dB on average; one
/// seed's take of the same note scatters ±0.3 dB about that, since the noise drives the
/// friction and the pitch loop's path with it. With one seed the step was a draw: −27.5 →
/// −27.6 dB after the merge, and the DSP alone fell at that step before it, on that seed.)
#[test]
fn the_bite_raises_the_noise_between_the_harmonics_through_the_app() {
    assert!(InstrumentKind::BowedString.automatable("bow.bite"));
    assert!(InstrumentKind::BowedString.modulatable("bow.bite"));
    let take = |bite: f32, seed: f32| {
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
        for (key, value) in [("vib.depth", 0.0), ("bow.bite", bite), ("seed", seed)] {
            assert!(app.exec(Command::SetInstrumentValue {
                track,
                key: key.into(),
                value,
            }));
        }
        let mut p = app.project().clone();
        let mut c = p.make_clip("Fiddle", Ticks(PPQ / 2), Ticks::beats(2));
        c.notes
            .push(p.make_note(Ticks::ZERO, Ticks(PPQ * 3 / 2), 71, 76));
        assert!(app.exec(Command::InsertClip {
            track,
            clip: Box::new(c)
        }));
        assert!(app.exec(Command::SetTempo { bpm: 60.0 }));
        let out = play(&mut app, &mut proc_, 3);
        assert!(out.iter().all(|x| x.is_finite()));
        let b4 = 440.0 * cantus_dsp::math::exp2(2.0 / 12.0);
        let f0 = peak_hz(&out, b4, 1.0, 1.8);
        aperiodic(&out, f0, 1.0, 1.8)
    };
    const SEEDS: [f32; 4] = [0.0, 1.0, 2.0, 3.0];
    let r: Vec<f64> = [0.0, 1.0 / 3.0, 2.0 / 3.0, 1.0]
        .iter()
        .map(|b| SEEDS.iter().map(|s| take(*b, *s)).sum::<f64>() / SEEDS.len() as f64)
        .collect();
    let db: Vec<f64> = r.iter().map(|v| 10.0 * v.max(1e-12).log10()).collect();
    println!("the noise between the harmonics at bite 0, 1/3, 2/3, 1: {db:.1?} dB");
    for w in db.windows(2) {
        assert!(w[1] > w[0], "the noise rises with the bite: {db:.1?}");
    }
    assert!(db[3] - db[0] > 1.0, "the bite is heard: {db:.1?}");
}
