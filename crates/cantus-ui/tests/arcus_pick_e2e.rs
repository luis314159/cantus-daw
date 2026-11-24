//! Arcus in the instrument picker (round 42, ADR 0157): the device picker lists it in both
//! languages; choosing it through `CantusApp::assemble` and `apply_pick` adds a track that
//! hosts a new Arcus (a cello, legato, its model stored); a legato pair on that track plays
//! through the real engine wiring (a processor driven block by block) and both notes are
//! heard **in tune** — the bowed pitch measured on the output; and a factory preset applied as
//! the browser applies it plays through the same path.

use cantus_core::{InstrumentKind, Project, Ticks, PPQ};
use cantus_engine::EngineHandle;
use cantus_ui::app::Files;
use cantus_ui::i18n::{set_thread_locale, Locale};
use cantus_ui::picker::{PickChoice, PickPurpose};
use cantus_ui::picker_catalog::{self, Category};
use cantus_ui::CantusApp;

#[test]
#[ignore = "runs once Arcus is in ALL — the owner's listening verdict (docs/round42/arcus.md)"]
fn the_picker_lists_arcus_in_both_languages() {
    for (locale, what) in [
        (Locale::En, "Bowed strings: cello and double bass"),
        (Locale::Es, "Cuerdas frotadas: chelo y contrabajo"),
    ] {
        set_thread_locale(locale);
        let devices = picker_catalog::build(std::iter::empty());
        let d = devices
            .iter()
            .find(|d| d.choice == PickChoice::Instrument(InstrumentKind::BowedString))
            .unwrap_or_else(|| panic!("{locale:?}: Arcus is not in the picker"));
        assert_eq!(d.name, "Arcus");
        assert_eq!(d.what, what);
        assert_eq!(d.slug, Some("arcus"));
        assert!(d.in_category(Category::OtherInstruments));
    }
    assert!(InstrumentKind::BowedString.ready());
}

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
    let song = Project::new("arcus");
    let app = CantusApp::assemble(&ctx, files, Some(song), engine, None, None, None, false);
    (app, proc_)
}

/// Play the song from the start for `seconds` through the processor: the output (stereo).
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
        assert!(since.elapsed().as_secs() < 120, "the phrase never finished");
    }
    app.toggle_play();
    app.sync_engine();
    out
}

/// The frequency of the partial near `near` Hz in `x` (mono at 48 kHz) over `from…to`
/// seconds: heterodyned, two boxcars of four periods, the phase's slope (Fides's measure).
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

fn cents(f: f64, want: f64) -> f64 {
    1200.0 * cantus_dsp::math::log2(f / want)
}

#[test]
fn choosing_arcus_adds_a_track_whose_legato_pair_plays_in_tune() {
    let data = tempfile::tempdir().unwrap();
    let (mut app, mut proc_) = app_with_song(data.path());
    let before = app.project().tracks.len();
    let purpose = PickPurpose::AddInstrument { index: None };
    app.open_picker(purpose.clone());
    app.apply_pick(purpose, PickChoice::Instrument(InstrumentKind::BowedString));
    app.picker = None;
    assert_eq!(app.project().tracks.len(), before + 1);
    let track = app.project().tracks.last().unwrap().id;
    let spec = app
        .project()
        .track(track)
        .and_then(|t| t.instrument.native())
        .cloned()
        .expect("a native instrument");
    assert_eq!(spec.kind, InstrumentKind::BowedString);
    assert_eq!(
        spec.params.get("model"),
        Some(&cantus_core::inst::arcus::MODEL)
    );
    assert_eq!(spec.get("inst"), 0.0, "a cello");
    assert_eq!(spec.get("mode"), 2.0, "legato");

    // A legato pair at 60 BPM: D3 for two beats, slurred (overlapping) to A3 for two.
    let mut p = app.project().clone();
    let mut c = p.make_clip("Pair", Ticks::ZERO, Ticks::beats(4));
    c.notes
        .push(p.make_note(Ticks::ZERO, Ticks(2 * PPQ + PPQ / 16), 50, 90));
    c.notes
        .push(p.make_note(Ticks(2 * PPQ), Ticks(2 * PPQ), 57, 90));
    assert!(app.exec(cantus_core::Command::InsertClip {
        track,
        clip: Box::new(c)
    }));
    assert!(app.exec(cantus_core::Command::SetTempo { bpm: 60.0 }));
    let out = play(&mut app, &mut proc_, 5);
    assert!(out.iter().all(|x| x.is_finite()));
    let peak = out.iter().fold(0.0f32, |m, x| m.max(x.abs()));
    assert!(peak > 0.01 && peak < 1.0, "peak {peak}");
    let mono: Vec<f32> = out.chunks_exact(2).map(|f| 0.5 * (f[0] + f[1])).collect();
    // Each note over a window late in its stroke, past its attack (the vibrato off by the
    // windows' whole cycles: the measure fits the slope over many of them).
    let d3 = 440.0 * cantus_dsp::math::exp2((50.0 - 69.0) / 12.0);
    let a3 = 440.0 * cantus_dsp::math::exp2((57.0 - 69.0) / 12.0);
    let first = cents(measure_hz(&mono, d3, 0.6, 1.9), d3);
    let second = cents(measure_hz(&mono, a3, 2.6, 3.9), a3);
    println!("the legato pair: D3 {first:+.2} ct, A3 {second:+.2} ct");
    assert!(first.abs() < 3.0, "D3 {first:+.2} cents");
    assert!(second.abs() < 3.0, "A3 {second:+.2} cents");
    // Both heard: the second note's level within 10 dB of the first's.
    let rms = |a: usize, b: usize| {
        (mono[a..b].iter().map(|x| (*x as f64).powi(2)).sum::<f64>() / (b - a) as f64).sqrt()
    };
    let (l1, l2) = (rms(28_800, 91_200), rms(124_800, 187_200));
    assert!(l1 > 1e-3 && l2 > 1e-3, "{l1} {l2}");
    assert!((l1 / l2).max(l2 / l1) < 3.2, "{l1} {l2}");

    // The pizzicato preset applied as the preset browser applies it plays the same pair.
    let pizz = cantus_ui::views::inst::arcus::FACTORY_PRESETS
        .iter()
        .find(|p| p.label_key == "arcus.preset.cello_pizz")
        .expect("the pizzicato preset");
    let target = cantus_ui::views::inst::preset_spec(InstrumentKind::BowedString, pizz);
    app.apply_inst_spec(track, &target, "cmd.fx_preset");
    let now = app
        .project()
        .track(track)
        .and_then(|t| t.instrument.native())
        .cloned()
        .unwrap();
    assert_eq!(now.get("tech"), 1.0, "plucked");
    let out = play(&mut app, &mut proc_, 5);
    let peak = out.iter().fold(0.0f32, |m, x| m.max(x.abs()));
    assert!(peak > 0.01 && peak < 1.0, "the pizzicato: peak {peak}");
}
