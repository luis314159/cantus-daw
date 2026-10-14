//! Third review #9: a note pitched too high to stream plays its sample's start only — the
//! status line says so once a session, and the status bar keeps the count.

mod support;

use cantus_core::inst::sampler::Zone;
use cantus_core::EffectKind;
use cantus_core::*;
use cantus_engine::EngineHandle;
use cantus_ui::CantusApp;

fn wav(path: &std::path::Path, frames: usize) {
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: 48_000,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut w = hound::WavWriter::create(path, spec).unwrap();
    for i in 0..frames {
        w.write_sample(((i % 61) as i16 - 30) * 300).unwrap();
    }
    w.finalize().unwrap();
}

#[test]
fn a_note_too_high_to_stream_is_told_once() {
    cantus_ui::i18n::default_thread_locale(cantus_ui::i18n::Locale::En);
    let dir = tempfile::tempdir().unwrap();
    cantus_engine::inst::stream::hub().set_cache_dir(dir.path().join("cache"));
    let src = dir.path().join("long.wav");
    wav(&src, 600_000);
    let mut spec = NativeSpec::new(InstrumentKind::Sampler);
    let z = Zone::new(Some(src.to_string_lossy().into()), 0, 127, 0);
    for (k, v) in z.values_for(0) {
        spec.set(&k, v).unwrap();
    }
    for (k, t) in z.texts_for(0) {
        spec.set_text(&k, Some(t)).unwrap();
    }
    spec.set("quality", 0.0).unwrap();
    let mut p = Project::new("cut");
    let mut t = p.make_track("Exemplar");
    t.instrument = InstrumentSpec::Native(spec);
    let mut clip = p.make_clip("Notes", Ticks::ZERO, Ticks::beats(8));
    for i in 0..4 {
        let n = p.make_note(Ticks::beats(i), Ticks::QUARTER, 127, 100);
        clip.notes.push(n);
    }
    t.clips.push(clip);
    p.tracks.push(t);
    let (mut engine, mut processor) = EngineHandle::manual(48_000.0);
    engine.samples.set_streaming(true);
    let mut app = CantusApp::from_parts(Some(p), engine, false);
    app.sync_engine();
    app.toggle_play();
    app.sync_engine();
    let mut buf = vec![0f32; 512 * 2];
    let mut told = 0;
    for _ in 0..400 {
        processor.process(&mut buf, 2);
        let before = app.status.text.clone();
        // A frame: the engine's figures in, then the sync.
        app.engine.poll();
        app.sync_engine();
        if app.status.text != before && app.status.text.contains("only its start") {
            told += 1;
        }
    }
    assert!(
        processor.stats().stream_cut >= 2,
        "{}",
        processor.stats().stream_cut
    );
    assert_eq!(told, 1, "said once: {}", app.status.text);
}

/// Third review #9: an export's result says how many notes played their start only.
#[test]
fn an_export_says_how_many_notes_played_their_start_only() {
    cantus_ui::i18n::default_thread_locale(cantus_ui::i18n::Locale::En);
    let done = cantus_export::Done {
        cut_notes: 3,
        ..Default::default()
    };
    let text = cantus_ui::views::export::summary(
        &done,
        std::path::Path::new("mix.wav"),
        &cantus_export::ExportSettings::default(),
    );
    assert!(
        text.contains("3 notes played only their sample's start"),
        "{text}"
    );
}

/// Fourth review #2, #4: while an Exemplar's zone table waits for its new start (a slow
/// disk), the window compiles nothing more — no whole-project recompile and plug-in sync
/// every frame — and an effect added with it reaches the engine at once.
#[test]
fn a_waiting_zone_table_costs_the_window_nothing() {
    cantus_ui::i18n::default_thread_locale(cantus_ui::i18n::Locale::En);
    let dir = tempfile::tempdir().unwrap();
    cantus_engine::inst::stream::hub().set_cache_dir(dir.path().join("cache"));
    let src = dir.path().join("long.wav");
    wav(&src, 2_000_000);
    let spec = |start: f32| {
        let mut spec = NativeSpec::new(InstrumentKind::Sampler);
        let mut z = Zone::new(Some(src.to_string_lossy().into()), 0, 127, 60);
        z.set(cantus_core::inst::sampler::zf::START, start);
        for (k, v) in z.values_for(0) {
            spec.set(&k, v).unwrap();
        }
        for (k, t) in z.texts_for(0) {
            spec.set_text(&k, Some(t)).unwrap();
        }
        spec
    };
    let mut p = Project::new("wait");
    let mut t = p.make_track("Exemplar");
    t.instrument = InstrumentSpec::Native(spec(0.0));
    p.tracks.push(t);
    let (mut engine, _processor) = EngineHandle::manual(48_000.0);
    engine.samples.set_streaming(true);
    let mut app = CantusApp::from_parts(Some(p), engine, false);
    app.sync_engine();
    let t0 = std::time::Instant::now();
    while app.engine.snapshot_pending() && t0.elapsed() < std::time::Duration::from_secs(3) {
        app.sync_engine();
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    cantus_engine::inst::stream::set_read_delay(std::time::Duration::from_millis(20));
    let track = app.project().tracks[0].id;
    app.set_instrument(track, InstrumentSpec::Native(spec(900_000.0)));
    let strip = app.project().tracks[0].strip.expect("a strip");
    app.add_effect(strip, EffectKind::Compressor);
    app.sync_engine();
    assert!(app.engine.snapshot_pending(), "the zone table waits");
    let compiled = app.engine.compiles();
    let effect = app.project().strip(strip).unwrap().effects[0].id;
    assert!(app.engine.knows_effect(effect), "the effect went at once");
    for _ in 0..30 {
        app.sync_engine();
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    cantus_engine::inst::stream::set_read_delay(std::time::Duration::ZERO);
    assert_eq!(
        app.engine.compiles(),
        compiled,
        "nothing recompiled while it waits"
    );
}
