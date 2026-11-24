//! r42:arcus-upper — Arcus's violin and viola reached as a user reaches them: Arcus added
//! through `CantusApp::assemble` and `apply_pick`, the Instrument card's choice set as its
//! switch sets it (`set_inst_value`), a legato pair played through the real engine wiring (a
//! processor driven block by block) and both notes heard **in tune** — the bowed pitch
//! measured on the output; and each instrument's factory pizzicato preset applied as the
//! preset browser applies it.

use cantus_core::{InstrumentKind, Project, Ticks, PPQ};
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
    let song = Project::new("arcus-upper");
    let app = CantusApp::assemble(&ctx, files, Some(song), engine, None, None, None, false);
    (app, proc_)
}

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
        assert!(since.elapsed().as_secs() < 180, "the phrase never finished");
    }
    app.toggle_play();
    app.sync_engine();
    out
}

/// The partial near `near` Hz over `from…to` s: heterodyned, two boxcars of four periods, the
/// phase's slope (Fides's measure, as `arcus_pick_e2e`).
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

fn hz(key: u8) -> f64 {
    440.0 * cantus_dsp::math::exp2((key as f64 - 69.0) / 12.0)
}

#[test]
fn the_violin_and_the_viola_play_a_legato_pair_in_tune_through_the_app() {
    // (the card's choice, the pair: a fifth up, slurred)
    for (inst, name, low, high) in [(2.0f32, "violin", 74u8, 81u8), (3.0, "viola", 62, 69)] {
        let data = tempfile::tempdir().unwrap();
        let (mut app, mut proc_) = app_with_song(data.path());
        let purpose = PickPurpose::AddInstrument { index: None };
        app.open_picker(purpose.clone());
        app.apply_pick(purpose, PickChoice::Instrument(InstrumentKind::BowedString));
        app.picker = None;
        let track = app.project().tracks.last().unwrap().id;
        // The Instrument card's switch, as it sets the choice.
        app.set_inst_value(track, "inst", inst, false);
        let spec = app
            .project()
            .track(track)
            .and_then(|t| t.instrument.native())
            .cloned()
            .expect("a native instrument");
        assert_eq!(spec.kind, InstrumentKind::BowedString);
        assert_eq!(spec.get("inst"), inst, "{name}");
        let mut p = app.project().clone();
        let mut c = p.make_clip("Pair", Ticks::ZERO, Ticks::beats(4));
        c.notes
            .push(p.make_note(Ticks::ZERO, Ticks(2 * PPQ + PPQ / 16), low, 90));
        c.notes.push(p.make_note(Ticks(2 * PPQ), Ticks(2 * PPQ), high, 90));
        assert!(app.exec(cantus_core::Command::InsertClip {
            track,
            clip: Box::new(c)
        }));
        assert!(app.exec(cantus_core::Command::SetTempo { bpm: 60.0 }));
        let out = play(&mut app, &mut proc_, 5);
        assert!(out.iter().all(|x| x.is_finite()));
        let peak = out.iter().fold(0.0f32, |m, x| m.max(x.abs()));
        assert!(peak > 0.01 && peak < 1.0, "{name}: peak {peak}");
        let mono: Vec<f32> = out.chunks_exact(2).map(|f| 0.5 * (f[0] + f[1])).collect();
        let first = 1200.0 * cantus_dsp::math::log2(measure_hz(&mono, hz(low), 0.6, 1.9) / hz(low));
        let second =
            1200.0 * cantus_dsp::math::log2(measure_hz(&mono, hz(high), 2.6, 3.9) / hz(high));
        println!("{name}'s legato pair: {first:+.2} ct, {second:+.2} ct");
        assert!(first.abs() < 3.0, "{name}: the first note {first:+.2} cents");
        assert!(second.abs() < 3.0, "{name}: the second note {second:+.2} cents");
        let rms = |a: usize, b: usize| {
            (mono[a..b].iter().map(|x| (*x as f64).powi(2)).sum::<f64>() / (b - a) as f64).sqrt()
        };
        let (l1, l2) = (rms(28_800, 91_200), rms(124_800, 187_200));
        assert!(l1 > 1e-3 && l2 > 1e-3, "{name}: {l1} {l2}");
        assert!((l1 / l2).max(l2 / l1) < 3.2, "{name}: {l1} {l2}");

        // Its pizzicato preset, applied as the preset browser applies it, plays the pair.
        let label = format!("arcus.preset.{name}_pizz");
        let pizz = cantus_ui::views::inst::arcus::FACTORY_PRESETS
            .iter()
            .find(|p| p.label_key == label)
            .expect("the pizzicato preset");
        let target = cantus_ui::views::inst::preset_spec(InstrumentKind::BowedString, pizz);
        app.apply_inst_spec(track, &target, "cmd.fx_preset");
        let now = app
            .project()
            .track(track)
            .and_then(|t| t.instrument.native())
            .cloned()
            .unwrap();
        assert_eq!(now.get("tech"), 1.0, "{name}: plucked");
        assert_eq!(now.get("inst"), inst, "{name}: the preset's instrument");
        let out = play(&mut app, &mut proc_, 5);
        let peak = out.iter().fold(0.0f32, |m, x| m.max(x.abs()));
        assert!(peak > 0.01 && peak < 1.0, "{name}'s pizzicato: peak {peak}");
    }
}
