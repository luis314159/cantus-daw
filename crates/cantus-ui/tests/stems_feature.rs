//! *Separate stems…* end to end in the application (round 34, ADR 0085), with the helper and
//! the tests' stand-in network (each part a quarter of the track, so the right answer is
//! known): a clip at 48 kHz and one at 96 kHz converted to the model's rate and back, the
//! parts laid on new tracks under a group bus, each clip where the original is with the
//! same window, the files at the original's rate and length, the originals muted, the
//! files' provenance — one undo step that undo takes back whole. And the agent tool, its
//! words naming what it separates. Its own test binary: it points `CANTUS_DATA_DIR` at a
//! temporary folder.

mod ml_support;
mod stems_support;

use cantus_ui::stems_ui::{self, run, Target};
use cantus_ui::CantusApp;
use std::time::{Duration, Instant};
use stems_support::*;

#[test]
fn a_track_at_48_and_96_khz_separates_into_aligned_tracks_in_one_undo_step() {
    let (_one, data) = data_folder();
    cantus_ui::i18n::set_thread_locale(cantus_ui::i18n::Locale::En);
    let x48 = tone(48_000, 4.0);
    let x96 = tone(96_000, 3.0);
    let (f48, f96) = (data.path().join("a48.wav"), data.path().join("b96.wav"));
    write(&f48, 48_000, &x48);
    write(&f96, 96_000, &x96);
    let (p, tid, clips) = project(&[
        (f48.clone(), 48_000, x48.0.len() as u64),
        (f96.clone(), 96_000, x96.0.len() as u64),
    ]);
    let mut app = app_with(p, data.path());
    let before = app.project().clone();
    run::start(&mut app, Target::Track(tid), fixture_model()).unwrap();
    until_done(&mut app);
    assert_eq!(app.stems.runs[0].phase, run::Phase::Done);
    let p = app.project().clone();
    // Four tracks right under the original, each on its own strip feeding one bus in a
    // group; the bus where the original fed (the master here).
    assert_eq!(p.tracks.len(), 5);
    assert_eq!(p.tracks[0].id, tid);
    let group = p.strip_groups.last().unwrap().id;
    let bus = p
        .strips
        .iter()
        .find(|s| s.name == "Mix stems")
        .expect("the bus named after the track, short enough for a strip");
    assert_eq!(bus.group, Some(group));
    for (k, t) in p.tracks[1..].iter().enumerate() {
        assert_eq!(t.name, ["Drums", "Bass", "Other", "Vocals"][k]);
        let strip = p.strip(t.strip.unwrap()).unwrap();
        assert_eq!(strip.output, Some(bus.id));
        assert_eq!(strip.group, Some(group));
        assert_eq!(t.clips.len(), 2);
        for (c, orig_id) in t.clips.iter().zip(&clips) {
            let orig = before.track(tid).unwrap().clip(*orig_id).unwrap();
            // The same window: start, length, offset; another file under it.
            assert_eq!(
                (c.start, c.length, c.offset),
                (orig.start, orig.length, orig.offset)
            );
            let a = c.audio.as_ref().unwrap();
            let oa = orig.audio.as_ref().unwrap();
            let file = p.audio_source(a.source).unwrap();
            let of = before.audio_source(oa.source).unwrap();
            assert_eq!(
                (file.sample_rate, file.frames),
                (of.sample_rate, of.frames),
                "the original's rate and length"
            );
            let prov = p.provenance.get(&a.source).expect("its provenance");
            assert_eq!(
                (prov.model.as_str(), prov.part.as_str()),
                ("htdemucs", PARTS[k])
            );
            assert_eq!(prov.from, of.name);
            // Its file: a quarter of the original, at the original's rate, sample for
            // sample (the stand-in network passes the time branch at 1/4; the conversion
            // to 44.1 kHz and back is the only change).
            let (rate, l, _) = read(&cantus_ui_resolve(&app, &file.path));
            assert_eq!(rate, of.sample_rate);
            assert_eq!(l.len() as u64, of.frames);
            let src = if of.sample_rate == 48_000 { &x48 } else { &x96 };
            let (sig, err) = (2_000..l.len() - 2_000).fold((0f64, 0f64), |(s, e), i| {
                let want = 0.25 * src.0[i] as f64;
                (s + want * want, e + (l[i] as f64 - want).powi(2))
            });
            let snr = 10.0 * (sig / err).log10();
            assert!(snr > 50.0, "{}: {snr:.1} dB", file.name);
        }
    }
    // The originals muted, not removed.
    for c in &clips {
        assert!(p.track(tid).unwrap().clip(*c).unwrap().muted);
    }
    // One undo step takes it all back (ids are never handed out twice: the counter stays);
    // the files stay for a redo.
    app.undo();
    let mut now = app.project().clone();
    now.next_id = before.next_id;
    assert_eq!(now, before);
}

#[test]
fn a_cancelled_separation_leaves_no_files_and_no_change() {
    let (_one, data) = data_folder();
    let x = tone(44_100, 30.0);
    let f = data.path().join("long.wav");
    write(&f, 44_100, &x);
    let (p, tid, _) = project(&[(f, 44_100, x.0.len() as u64)]);
    let mut app = app_with(p, data.path());
    let before = app.project().clone();
    run::start(&mut app, Target::Track(tid), fixture_model()).unwrap();
    // Cancel once it separates.
    let t = Instant::now();
    while !matches!(app.stems.runs[0].phase, run::Phase::Separating(_)) {
        run::step(&mut app);
        assert!(t.elapsed() < Duration::from_secs(60));
        std::thread::sleep(Duration::from_millis(5));
    }
    run::cancel(&mut app, 0);
    until_done(&mut app);
    assert_eq!(app.stems.runs[0].phase, run::Phase::Cancelled);
    let mut now = app.project().clone();
    now.next_id = before.next_id;
    assert_eq!(now, before);
    for s in &app.stems.runs[0].sources {
        assert!(!s.out.exists(), "{} left", s.out.display());
    }
}

#[test]
fn the_agent_tool_names_its_target_and_ends_in_one_edit() {
    use cantus_ai::tool::{JobCx, JobDone, ToolCx, ToolRun};
    let (_one, data) = data_folder();
    cantus_ui::i18n::set_thread_locale(cantus_ui::i18n::Locale::En);
    let x = tone(44_100, 3.0);
    let f = data.path().join("s.wav");
    write(&f, 44_100, &x);
    let (p, tid, clips) = project(&[(f, 44_100, x.0.len() as u64)]);
    let mut app = app_with(p, data.path());
    let registry = cantus_ui::ai_tools::registry();
    let tool = registry
        .iter()
        .find(|t| t.name == "separate_stems")
        .unwrap();
    let ToolRun::Job(prepare) = tool.run else {
        panic!("a job");
    };
    // No model installed: refused, in words that say where to install it.
    let args = serde_json::json!({"track_id": tid.0});
    let refused = prepare(&mut ToolCx { host: &mut app }, &args);
    assert!(
        matches!(refused, Err(cantus_ai::tool::ToolError::Refused(ref m)) if m.contains("Settings"))
    );
    // With a model (the stand-in): its words name the file, the track and the parts.
    let _ = stems_ui::view::MODELS;
    let job = prepare_with(&mut app, &args);
    let said = job.said.render(&|k: &str, a: &[(&str, String)]| {
        let args: Vec<(&str, &dyn std::fmt::Display)> = a
            .iter()
            .map(|(n, v)| (*n, v as &dyn std::fmt::Display))
            .collect();
        cantus_ui::i18n::tf(k, &args)
    });
    assert_eq!(
        said,
        "Separate “Song 0.wav” on “Mix” into 4 stems under a new bus"
    );
    let progress = |_: f32| {};
    let cancelled = || false;
    let done = (job.work)(&JobCx {
        progress: &progress,
        cancelled: &cancelled,
    })
    .unwrap();
    let JobDone::Then(_, then) = done else {
        panic!("an edit");
    };
    let plan = then(&mut ToolCx { host: &mut app }).unwrap();
    assert!(app.exec(plan.command));
    assert_eq!(plan.result["track_ids"].as_array().unwrap().len(), 4);
    assert_eq!(plan.result["muted_clip_ids"][0], clips[0].0);
    assert!(
        app.project()
            .track(tid)
            .unwrap()
            .clip(clips[0])
            .unwrap()
            .muted
    );
}

/// The tool's job with the stand-in model in place of an installed one.
fn prepare_with(app: &mut CantusApp, args: &serde_json::Value) -> cantus_ai::tool::Job {
    cantus_ui::ai_tools::stems::prepare_with_model(app, args, fixture_model()).unwrap()
}
