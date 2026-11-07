//! Sectio, Exemplar's slicer (round 37, ADR 0099), through the app: a loop sliced at its
//! markers, dumped, replayed to the bit at its tempo and kept on the grid at another; the
//! three gestures one undo step each; a loop dropped through the app (made by
//! `CantusApp::assemble`, with the engine it drives) plays its dumped clip.
//! Its own test binary: it points `CANTUS_DATA_DIR` at a temporary folder.

use cantus_core::sectio::{SliceMode, Slicing, FIRST_KEY};
use cantus_core::*;
use cantus_engine::{render, EngineHandle, RenderOptions};
use cantus_ui::app::Files;
use cantus_ui::sectio_ui::{self, Gesture};
use cantus_ui::CantusApp;
use std::path::{Path, PathBuf};
use std::sync::Arc;

const RATE: u32 = 48_000;

fn data() -> &'static Path {
    static DIR: std::sync::OnceLock<tempfile::TempDir> = std::sync::OnceLock::new();
    DIR.get_or_init(|| {
        let d = tempfile::tempdir().unwrap();
        std::env::set_var("CANTUS_DATA_DIR", d.path());
        d
    })
    .path()
}

fn chunk(id: &[u8; 4], body: &[u8]) -> Vec<u8> {
    let mut v = id.to_vec();
    v.extend((body.len() as u32).to_le_bytes());
    v.extend(body);
    if body.len() % 2 == 1 {
        v.push(0);
    }
    v
}

/// A two-second loop at 120 BPM (four beats): a struck tone on every eighth, louder on the
/// beat, 16-bit mono at 48 kHz, with `cue ` points on the eighths and an ACID chunk (4 beats)
/// when `marked`. Written byte by byte after the RIFF layouts (Microsoft 1991; ACID as
/// libsndfile reads it).
fn loop_wav(path: &Path, marked: bool) -> Vec<u32> {
    let frames = 2 * RATE as usize;
    let mut x = vec![0i16; frames];
    let eighth = RATE as usize / 4;
    let hits: Vec<u32> = (0..8).map(|i| (i * eighth) as u32).collect();
    for (i, at) in hits.iter().enumerate() {
        let g = if i % 2 == 0 { 0.8 } else { 0.45 };
        let f = 110.0 * (1 + i % 3) as f32;
        for k in 0..eighth {
            let t = k as f32 / RATE as f32;
            let env = (-t * 18.0).exp();
            let v = g * env * (std::f32::consts::TAU * f * t).sin();
            let n = (*at as usize + k).min(frames - 1);
            x[n] = x[n].saturating_add((v * 30_000.0) as i16);
        }
    }
    let mut fmt = Vec::new();
    fmt.extend(1u16.to_le_bytes());
    fmt.extend(1u16.to_le_bytes());
    fmt.extend(RATE.to_le_bytes());
    fmt.extend((RATE * 2).to_le_bytes());
    fmt.extend(2u16.to_le_bytes());
    fmt.extend(16u16.to_le_bytes());
    let pcm: Vec<u8> = x.iter().flat_map(|s| s.to_le_bytes()).collect();
    let mut chunks = vec![chunk(b"fmt ", &fmt), chunk(b"data", &pcm)];
    if marked {
        let mut c = (hits.len() as u32).to_le_bytes().to_vec();
        for (i, p) in hits.iter().enumerate() {
            c.extend((i as u32 + 1).to_le_bytes());
            c.extend(p.to_le_bytes());
            c.extend(b"data");
            c.extend(0u32.to_le_bytes());
            c.extend(0u32.to_le_bytes());
            c.extend(p.to_le_bytes());
        }
        chunks.push(chunk(b"cue ", &c));
        let mut a = Vec::new();
        a.extend(0u32.to_le_bytes());
        a.extend(60u16.to_le_bytes());
        a.extend(0x8000u16.to_le_bytes());
        a.extend(0f32.to_le_bytes());
        a.extend(4u32.to_le_bytes());
        a.extend(4u16.to_le_bytes());
        a.extend(4u16.to_le_bytes());
        a.extend(120f32.to_le_bytes());
        chunks.push(chunk(b"acid", &a));
    }
    let body: Vec<u8> = b"WAVE".iter().copied().chain(chunks.concat()).collect();
    let mut w = b"RIFF".to_vec();
    w.extend((body.len() as u32).to_le_bytes());
    w.extend(body);
    std::fs::write(path, w).unwrap();
    hits
}

fn file(name: &str, marked: bool) -> (PathBuf, Vec<u32>) {
    let p = data().join(name);
    let hits = loop_wav(&p, marked);
    (p, hits)
}

fn project_of(spec: NativeSpec, notes: &[(i64, i64, u8, u8)], bpm: f64) -> Project {
    let mut p = Project::new("slices");
    p.tempo_bpm = bpm;
    let mut t = p.make_track("Loop");
    t.instrument = InstrumentSpec::Native(spec);
    let end = notes.iter().map(|n| n.0 + n.1).max().unwrap_or(1);
    let mut c = p.make_clip("Play", Ticks::ZERO, Ticks(end));
    for (s, l, k, v) in notes {
        let n = p.make_note(Ticks(*s), Ticks(*l), *k, *v);
        c.notes.push(n);
    }
    t.clips.push(c);
    p.tracks.push(t);
    p
}

fn render_left(p: &Project) -> Vec<f32> {
    let mut snap = Snapshot::compile(p, 1);
    cantus_engine::SampleCache::new().attach(&mut snap);
    let r = render(
        Arc::new(snap),
        &RenderOptions {
            sample_rate: RATE,
            tail_seconds: 0.2,
            ..Default::default()
        },
    )
    .unwrap();
    r.samples.chunks(2).map(|f| f[0]).collect()
}

#[test]
fn a_marked_loop_is_sliced_at_its_cue_points_with_its_acid_tempo() {
    let (path, hits) = file("marked.wav", true);
    let s = sectio_ui::analyse(&path, SliceMode::Markers, 0.5, 4).unwrap();
    assert_eq!(s.mode, SliceMode::Markers);
    assert!((s.bpm - 120.0).abs() < 1e-3, "{}", s.bpm);
    // At 120 BPM and 48 kHz a tick is 25 samples: an eighth (12 000) is already on one.
    assert_eq!(s.points, hits);
    assert_eq!((s.len, s.rate), (2 * RATE, RATE));
}

#[test]
fn an_unmarked_loop_is_sliced_at_its_transients() {
    let (path, hits) = file("plain.wav", false);
    let s = sectio_ui::analyse(&path, SliceMode::Markers, 0.8, 4).unwrap();
    assert_eq!(s.mode, SliceMode::Transients, "no markers: transients");
    for h in &hits {
        let near = s
            .points
            .iter()
            .any(|p| (*p as i64 - *h as i64).abs() <= 480);
        assert!(near, "{h} not within 10 ms of {:?}", s.points);
    }
}

/// The dumped clip, played at the loop's tempo, equals the whole file played by the same
/// instrument, to the bit — from the song's start and from bar 2.
#[test]
fn the_dump_replays_the_file_to_the_bit_at_its_tempo() {
    let (path, _) = file("dump.wav", true);
    let s = sectio_ui::analyse(&path, SliceMode::Markers, 0.5, 4).unwrap();
    let sample = path.to_string_lossy().into_owned();
    let sliced = sectio_ui::spec_of(&sample, &s, None);
    let notes = sectio_ui::dump_notes(&s);
    assert_eq!(notes.len(), s.points.len());
    assert_eq!(notes[0].2, FIRST_KEY);
    // The whole file as one slice, one note as long as the loop.
    let whole = Slicing {
        points: vec![0],
        ..s.clone()
    };
    let one = sectio_ui::spec_of(&sample, &whole, None);
    let len = notes.iter().map(|n| n.0 + n.1).max().unwrap();
    for shift in [0i64, Ticks::beats(4).0] {
        let moved: Vec<_> = notes.iter().map(|n| (n.0 + shift, n.1, n.2, n.3)).collect();
        let a = render_left(&project_of(sliced.clone(), &moved, 120.0));
        let b = render_left(&project_of(
            one.clone(),
            &[(shift, len, FIRST_KEY, 127)],
            120.0,
        ));
        assert!(a.iter().any(|x| *x != 0.0));
        assert_eq!(a.len(), b.len());
        let diff = a
            .iter()
            .zip(&b)
            .position(|(x, y)| x.to_bits() != y.to_bits());
        assert_eq!(
            diff, None,
            "shift {shift}: first difference at frame {diff:?}"
        );
    }
}

/// At another tempo the slices move with the grid: each plays whole, at its pitch, from the
/// sample its note's tick falls on at the new tempo.
#[test]
fn a_tempo_change_keeps_the_slices_on_the_grid() {
    let (path, _) = file("tempo.wav", true);
    let s = sectio_ui::analyse(&path, SliceMode::Markers, 0.5, 4).unwrap();
    let sample = path.to_string_lossy().into_owned();
    let sliced = sectio_ui::spec_of(&sample, &s, None);
    let notes = sectio_ui::dump_notes(&s);
    let at_100 = render_left(&project_of(sliced.clone(), &notes, 100.0));
    let at_120 = render_left(&project_of(sliced, &notes, 120.0));
    for (i, n) in notes.iter().enumerate() {
        let (a, b) = s.span(i);
        let len = (b - a) as usize;
        let start =
            cantus_engine::transport::sample_at(0.0, n.0 as f64, 100.0, RATE as f64) as usize;
        let start_120 = a as usize;
        assert_eq!(
            &at_100[start..start + len],
            &at_120[start_120..start_120 + len],
            "slice {i}"
        );
    }
}

fn assembled() -> (CantusApp, cantus_engine::Processor) {
    let files = Files::read(data(), Some(cantus_ui::i18n::Locale::En));
    let ctx = egui::Context::default();
    let (engine, proc_) = EngineHandle::manual(RATE as f64);
    let app = CantusApp::assemble(&ctx, files, None, engine, None, None, None, false);
    (app, proc_)
}

#[test]
fn the_three_gestures_are_one_undo_step_each() {
    let (mut app, _p) = assembled();
    let (path, _) = file("gestures.wav", true);
    let track = app.add_sliced_track(&path).expect("a sliced track");
    let clip = app.dump_slices(track, Ticks::ZERO).expect("the dump");
    let notes = |a: &CantusApp| {
        let mut v: Vec<(i64, i64, u8)> = a
            .project()
            .track(track)
            .unwrap()
            .clips
            .iter()
            .find(|c| c.id == clip)
            .unwrap()
            .notes
            .iter()
            .map(|n| (n.start.0, n.length.0, n.pitch))
            .collect();
        v.sort();
        v
    };
    let before = notes(&app);
    for g in [
        Gesture::Random(7),
        Gesture::Stutter {
            amount: 0.5,
            repeats: 4,
            seed: 3,
        },
        Gesture::Quantize {
            grid: Ticks::QUARTER.0 / 2,
            swing: 0.4,
        },
    ] {
        let steps = app.bus.history().len();
        assert!(app.slice_gesture(track, clip, g), "{g:?}");
        assert_eq!(app.bus.history().len(), steps + 1, "{g:?}: one undo step");
        assert_ne!(notes(&app), before, "{g:?} changed the clip");
        app.undo();
        assert_eq!(notes(&app), before, "{g:?}: undone");
    }
}

/// The e2e: a loop dropped through the app becomes a sliced Exemplar (one undo step), its
/// slices are dumped, and the engine the app drives plays the clip live — what the offline
/// render of the project plays, to the bit, and the slices where the file has them.
#[test]
fn a_loop_dropped_through_the_app_plays_its_dumped_clip_live() {
    let (mut app, mut proc_) = assembled();
    let (path, _) = file("dropped.wav", true);
    // The sample browser's menu on the file: *Slice into a new Exemplar track*.
    let steps = app.bus.history().len();
    let before = app.project().tracks.len();
    assert!(cantus_ui::views::samples_browser::slice_from_menu(
        &mut app, &path, false
    ));
    assert_eq!(app.project().tracks.len(), before + 1);
    let track = app.project().tracks.last().unwrap().id;
    assert_eq!(
        app.bus.history().len(),
        steps + 1,
        "the slice is one undo step"
    );
    let spec = app.native_of(track).unwrap();
    assert_eq!(spec.kind, InstrumentKind::Sampler);
    let s = Slicing::of(&spec).expect("sliced");
    assert_eq!(s.points.len(), 8);
    app.dump_slices(track, Ticks::ZERO).expect("the dump");
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
    if app.project().tempo_bpm != 120.0 {
        assert!(app.exec(Command::SetTempo { bpm: 120.0 }));
    }
    app.sync_engine();
    // The sample arrives through the loader: the frames pick it up.
    let t = std::time::Instant::now();
    while app.engine.samples.loading() && t.elapsed() < std::time::Duration::from_secs(20) {
        std::thread::sleep(std::time::Duration::from_millis(20));
        app.sync_engine();
    }
    app.sync_engine();
    // The new zone tables wait for their starts to be read (ADR 0050, third review #6): the
    // window's frames send them once the stream's reader has them, the device running.
    let t = std::time::Instant::now();
    while app.engine.flush_pending() && t.elapsed() < std::time::Duration::from_secs(10) {
        let mut buf = vec![0f32; 512 * 2];
        proc_.process(&mut buf, 2);
        std::thread::sleep(std::time::Duration::from_micros(10_667));
        app.sync_engine();
    }
    proc_.settle();
    app.toggle_play();
    let frames = 2 * RATE as usize;
    let mut live = Vec::with_capacity(frames * 2);
    // In real time, as a device calls it: the app streams Exemplar's samples from disk
    // (ADR 0050), and the stream's reader serves a live engine at the pace it plays.
    let mut blocks = 0;
    while live.len() / 2 < frames {
        let mut buf = vec![0f32; 512 * 2];
        proc_.process(&mut buf, 2);
        live.extend_from_slice(&buf);
        std::thread::sleep(std::time::Duration::from_micros(10_667));
        // The window's frames go on beside the device: the snapshot held back for its
        // zones' starts goes out once the stream's reader has them (ADR 0050).
        blocks += 1;
        if blocks % 4 == 0 {
            app.sync_engine();
        }
    }
    let peak = live.iter().fold(0f32, |m, x| m.max(x.abs()));
    assert!(
        peak > 0.05,
        "the dumped clip sounds live: {peak} (loading {}, streaming {})",
        app.engine.samples.loading(),
        app.engine.samples.streaming()
    );
    let p = app.project().clone();
    let mut snap = Snapshot::compile(&p, 1);
    cantus_engine::SampleCache::new().attach(&mut snap);
    let offline = render(
        Arc::new(snap),
        &RenderOptions {
            sample_rate: RATE,
            tail_seconds: 0.2,
            ..Default::default()
        },
    )
    .unwrap()
    .samples;
    assert!(
        live[..frames * 2] == offline[..frames * 2],
        "live equals the offline render"
    );
}

/// The slice editor through the app (review 2 #2): a point moved (on a zero crossing), one
/// added, one removed, the loop re-cut on a grid and a style applied from the library — each
/// one undo step, the zones rebuilt from the points every time; a second loop dropped on
/// the card (`drop::land`, `Target::Slicer`) re-slices this Exemplar.
#[test]
fn the_slice_editor_moves_adds_removes_and_recuts_each_in_one_step() {
    let (mut app, _p) = assembled();
    let (path, _) = file("editor.wav", true);
    assert!(cantus_ui::views::samples_browser::slice_from_menu(
        &mut app, &path, false
    ));
    let track = app.project().tracks.last().unwrap().id;
    let points = |a: &CantusApp| Slicing::of(&a.native_of(track).unwrap()).unwrap().points;
    let zones_follow = |a: &CantusApp| {
        let spec = a.native_of(track).unwrap();
        let s = Slicing::of(&spec).unwrap();
        for (i, p) in s.points.iter().enumerate() {
            let z = cantus_core::inst::sampler::Zone::of(&spec, i).expect("a zone per slice");
            assert_eq!(z.get(cantus_core::inst::sampler::zf::START) as u32, *p);
            assert_eq!(
                z.get(cantus_core::inst::sampler::zf::END) as u32,
                s.span(i).1
            );
        }
        assert!(cantus_core::inst::sampler::Zone::of(&spec, s.points.len()).is_none());
    };
    let p0 = points(&app);
    let step = |a: &mut CantusApp, f: &mut dyn FnMut(&mut CantusApp) -> bool| {
        let n = a.bus.history().len();
        assert!(f(a));
        assert_eq!(a.bus.history().len(), n + 1, "one undo step");
    };
    step(&mut app, &mut |a| a.move_slice(track, 2, p0[2] + 300));
    assert_eq!(points(&app)[2], p0[2] + 300);
    zones_follow(&app);
    step(&mut app, &mut |a| a.add_slice(track, p0[1] + 2_000));
    assert_eq!(points(&app).len(), p0.len() + 1);
    zones_follow(&app);
    step(&mut app, &mut |a| a.remove_slice(track, 1));
    assert_eq!(points(&app).len(), p0.len());
    zones_follow(&app);
    step(&mut app, &mut |a| a.reslice(track, SliceMode::Grid, 0.5, 2));
    zones_follow(&app);
    app.undo();
    app.undo();
    app.undo();
    app.undo();
    assert_eq!(points(&app), p0, "every edit undone");
    // A style from the library: the user's loop re-cut its way, choked, in one step.
    let style = cantus_ui::sectio_ui::styles::sampler_presets()
        .iter()
        .find(|p| p.label_key == "slicer.style.break")
        .expect("the break style");
    let mut target = cantus_core::NativeSpec::new(InstrumentKind::Sampler);
    for (k, v) in style.values {
        target.set(k, *v);
    }
    let n = app.bus.history().len();
    app.apply_inst_spec(track, &target, "cmd.slice_again");
    assert_eq!(app.bus.history().len(), n + 1);
    let spec = app.native_of(track).unwrap();
    let z = cantus_core::inst::sampler::Zone::of(&spec, 0).unwrap();
    assert_eq!(
        z.sample.as_deref(),
        Some(&*path.to_string_lossy()),
        "the user's loop kept"
    );
    assert_eq!(z.get(cantus_core::inst::sampler::zf::EXCL), 1.0, "choked");
    zones_follow(&app);
    // A second loop dropped on the card.
    let (other, _) = file("second.wav", false);
    cantus_ui::samples::drop::land(
        &mut app,
        &other,
        cantus_ui::samples::drop::Target::Slicer { track },
        egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(400.0, 200.0)),
        egui::pos2(100.0, 100.0),
    );
    let spec = app.native_of(track).unwrap();
    let z = cantus_core::inst::sampler::Zone::of(&spec, 0).unwrap();
    assert_eq!(z.sample.as_deref(), Some(&*other.to_string_lossy()));
}
