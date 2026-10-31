//! *Convert to MIDI…* through the application (round 34, ADR 0086): melody mode (Cantus's
//! own tracker, no model) from the dialog's path makes a new Unda track under the voice
//! with a MIDI clip aligned to the audio clip — one undo step, undone exactly; drums through
//! the real helper with no model; the placement through the clip's mapping (a loop's
//! periods, a quantise grid, bends as per-note curves); the `audio_to_midi` agent tool —
//! one named undo step, the clip and track named in what it says, a model it lacks refused,
//! never downloaded; and the clip menu's entry.

mod ml_support;

use cantus_core::*;
use cantus_dsp::pitch::midi_to_hz;
use cantus_dsp::pitch::testkit::{self, Vowel};
use cantus_engine::EngineHandle;
use cantus_ml::transcribe::spec::{DrumClass, DrumHit, Mode, NoteEvent, Transcription};
use cantus_ui::transcribe_ui::{self as tr, run, Grid, Settings};
use cantus_ui::CantusApp;
use serde_json::{json, Value};
use std::time::{Duration, Instant};

const SR: u32 = 48_000;
/// The voice's notes: (start s, end s, MIDI).
const SUNG: [(f64, f64, f32); 4] = [
    (0.10, 0.70, 57.0),
    (0.85, 1.45, 60.0),
    (1.60, 2.30, 64.0),
    (2.45, 3.05, 62.0),
];

fn data_dir() {
    static DIR: std::sync::OnceLock<std::path::PathBuf> = std::sync::OnceLock::new();
    let d = DIR.get_or_init(|| {
        let d = std::env::temp_dir().join(format!("cantus-transcribe-ui-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&d);
        d
    });
    std::env::set_var("CANTUS_DATA_DIR", d);
}

fn write_wav(path: &std::path::Path, rate: u32, x: &[f32]) {
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: rate,
        bits_per_sample: 32,
        sample_format: hound::SampleFormat::Float,
    };
    let mut w = hound::WavWriter::create(path, spec).unwrap();
    for s in x {
        w.write_sample(*s).unwrap();
    }
    w.finalize().unwrap();
}

fn voice() -> Vec<f32> {
    testkit::voice(SR as f32, 3.2, Vowel::A, 0.5, move |t| {
        SUNG.iter()
            .find(|(a, b, _)| t >= *a && t < *b)
            .map_or(0.0, |(_, _, m)| midi_to_hz(*m))
    })
}

/// A project with a Keys track and a Voice track holding `path` from bar 1 for 8 beats.
fn project(path: &std::path::Path, rate: u32, frames: u64) -> Project {
    let mut p = Project::new("transcribe");
    p.tempo_bpm = 120.0;
    let keys = p.make_track("Keys");
    p.tracks.push(keys);
    let mut v = p.make_track("Voice");
    let id = AudioSourceId(p.alloc_id());
    p.audio_pool.push(AudioSource {
        id,
        name: "take".into(),
        path: path.to_string_lossy().into_owned(),
        sample_rate: rate,
        channels: 1,
        frames,
        sha256: None,
        recorded: true,
    });
    let c = p.make_audio_clip("Take 1", id, Ticks::ZERO, Ticks::beats(8));
    v.clips.push(c);
    p.tracks.push(v);
    p
}

struct Rig {
    app: CantusApp,
    ctx: egui::Context,
    _dir: tempfile::TempDir,
}

fn rig_with(x: &[f32], rate: u32) -> Rig {
    data_dir();
    cantus_ui::i18n::set_thread_locale(cantus_ui::i18n::Locale::En);
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("take.wav");
    write_wav(&path, rate, x);
    let app = CantusApp::from_parts(
        Some(project(&path, rate, x.len() as u64)),
        EngineHandle::dummy(),
        false,
    );
    let ctx = egui::Context::default();
    cantus_ui::design::install_fonts(&ctx);
    cantus_ui::design::install_style(&ctx);
    Rig {
        app,
        ctx,
        _dir: dir,
    }
}

fn voice_rig() -> Rig {
    rig_with(&voice(), SR)
}

fn ids(app: &CantusApp) -> (TrackId, ClipId) {
    let t = &app.project().tracks[1];
    (t.id, t.clips[0].id)
}

/// The project without what undo never gives back (ids are not reused).
fn comparable(p: &Project) -> Project {
    let mut p = p.clone();
    p.next_id = 0;
    p
}

/// Turn the window's own frames (`CantusApp::frame`: the dialog drawn, the conversion
/// moved on) until the conversion ends.
fn run_until_done(r: &mut Rig) {
    let t0 = Instant::now();
    let input = egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(1400.0, 900.0),
        )),
        ..Default::default()
    };
    while r.app.transcribe.busy() {
        let app = &mut r.app;
        r.ctx
            .run_ui(input.clone(), |ui| app.frame(ui))
            .textures_delta
            .clear();
        assert!(
            t0.elapsed() < Duration::from_secs(120),
            "the conversion never ended"
        );
        std::thread::sleep(Duration::from_millis(5));
    }
}

#[test]
fn melody_from_the_dialog_is_one_undo_step_on_a_new_track() {
    let mut r = voice_rig();
    let before = comparable(r.app.project());
    let steps = r.app.bus.history().len();
    let (t, c) = ids(&r.app);
    run::open(&mut r.app, t, c);
    r.app.transcribe.dialog.as_mut().unwrap().settings.mode = Mode::Melody;
    run::start(&mut r.app);
    run_until_done(&mut r);
    assert!(
        matches!(r.app.transcribe.last, Some(Ok(_))),
        "{:?}",
        r.app.transcribe.last
    );
    assert!(r.app.transcribe.dialog.is_none(), "it closes when done");
    assert_eq!(r.app.bus.history().len(), steps + 1, "one undo step");
    let p = r.app.project();
    assert_eq!(p.tracks.len(), 3);
    let new = &p.tracks[2];
    assert_eq!(new.name, "Take 1 · MIDI");
    assert!(
        matches!(&new.instrument, InstrumentSpec::Native(n) if n.kind == cantus_core::inst::InstrumentKind::Wavetable)
    );
    let clip = &new.clips[0];
    let audio = &p.tracks[1].clips[0];
    assert_eq!(
        (clip.start, clip.length),
        (audio.start, audio.length),
        "aligned to the audio clip"
    );
    let pitches: Vec<u8> = clip.notes.iter().map(|n| n.pitch).collect();
    assert_eq!(pitches, [57, 60, 64, 62], "{:?}", clip.notes);
    // At 120 BPM a beat is half a second: each note starts where it was sung (±30 ms).
    for (n, (a, _, _)) in clip.notes.iter().zip(SUNG) {
        let at = n.start.0 as f64 / Ticks::QUARTER.0 as f64 * 0.5;
        assert!((at - a).abs() < 0.03, "{n:?}");
        assert!(n.expr.is_none(), "no bends unless asked");
    }
    assert_eq!(r.app.bus.history()[steps].label_key, "cmd.transcribe");
    r.app.undo();
    assert_eq!(
        comparable(r.app.project()),
        before,
        "undo gives the project back exactly"
    );
}

#[test]
fn melody_keeps_only_notes_as_long_as_the_shortest_asked() {
    // The sung notes last 0.6, 0.6, 0.7 and 0.6 s: at 650 ms only the third is kept.
    let r = voice_rig();
    let (t, c) = ids(&r.app);
    let src = tr::source(&r.app, t, c).unwrap();
    let at = |ms: f32| {
        let s = Settings {
            mode: Mode::Melody,
            shortest_ms: ms,
            ..Settings::default()
        };
        tr::melody(&src, &s, &|| false)
            .unwrap()
            .notes
            .iter()
            .map(|n| n.pitch)
            .collect::<Vec<u8>>()
    };
    assert_eq!(at(127.7), [57, 60, 64, 62]);
    assert_eq!(at(650.0), [64]);
}

#[test]
fn bends_are_kept_as_per_note_curves_when_asked() {
    let mut r = voice_rig();
    let (t, c) = ids(&r.app);
    run::open(&mut r.app, t, c);
    let s = &mut r.app.transcribe.dialog.as_mut().unwrap().settings;
    s.mode = Mode::Melody;
    s.bends = true;
    s.quantize = Grid::Eighth;
    run::start(&mut r.app);
    run_until_done(&mut r);
    let clip = &r.app.project().tracks[2].clips[0];
    assert!(
        clip.notes
            .iter()
            .all(|n| n.start.0 % (Ticks::QUARTER.0 / 2) == 0),
        "quantised"
    );
    // The sung notes hold their pitch within cents: a bend curve near zero, or none.
    for n in &clip.notes {
        if let Some(e) = &n.expr {
            for p in e.curve(cantus_core::model::note_expr::ExprKind::Bend) {
                let v = cantus_core::model::note_expr::ExprKind::Bend.read(p.1);
                assert!(v.abs() < 0.6, "{v}");
            }
        }
    }
}

#[test]
fn placement_turns_bends_into_curves_and_keeps_the_classes_asked() {
    let mut r = voice_rig();
    let (t, c) = ids(&r.app);
    let src = tr::source(&r.app, t, c).unwrap();
    assert_eq!(src.frames.0, 0);
    let s = Settings {
        mode: Mode::Polyphonic,
        bends: true,
        ..Settings::default()
    };
    let note = |start: f64, end: f64, pitch: u8, bends: Vec<i32>| NoteEvent {
        start,
        end,
        pitch,
        velocity: 90,
        amplitude: 0.7,
        bends,
        curve: Vec::new(),
    };
    let tr_ = Transcription {
        mode: Mode::Polyphonic,
        duration: 3.0,
        notes: vec![
            note(0.5, 1.0, 60, vec![0, 0, 3, 3]),
            note(0.5, 1.0, 64, vec![]),
        ],
        hits: Vec::new(),
    };
    let p = tr::plan(&mut r.app.bus, &src, &s, &tr_).unwrap();
    assert_eq!(p.notes, 2);
    let Command::Batch { commands, .. } = &p.command else {
        panic!()
    };
    let Command::InsertClip { clip, .. } = &commands[1] else {
        panic!()
    };
    let bent = clip.notes.iter().find(|n| n.pitch == 60).unwrap();
    let e = bent.expr.as_ref().expect("a bend curve");
    let last = e
        .curve(cantus_core::model::note_expr::ExprKind::Bend)
        .last()
        .unwrap();
    assert_eq!(
        cantus_core::model::note_expr::ExprKind::Bend.read(last.1),
        1.0,
        "three thirds: a semitone"
    );
    // Loops, reversals, tempo changes: tests/transcribe_place.rs.
    // Drums: only the classes asked for.
    let drums = Transcription {
        mode: Mode::Drums,
        duration: 3.0,
        notes: Vec::new(),
        hits: vec![
            DrumHit {
                time: 0.5,
                class: DrumClass::Kick,
                key: 36,
                velocity: 100,
            },
            DrumHit {
                time: 1.0,
                class: DrumClass::Toms,
                key: 47,
                velocity: 100,
            },
        ],
    };
    let d = Settings {
        mode: Mode::Drums,
        classes: tr::default_classes(false),
        ..Settings::default()
    };
    let placed = tr::place(&src, &d, &drums);
    assert_eq!(
        placed.iter().map(|p| p.2).collect::<Vec<_>>(),
        [36],
        "toms off without the model"
    );
    assert!(matches!(tr::instrument(&d), InstrumentSpec::Pulsus(_)));
}

#[test]
fn drums_through_the_helper_without_a_model() {
    // Kicks every half second at 44.1 kHz.
    let rate = 44_100u32;
    let mut x = vec![0.0f32; 3 * rate as usize];
    for k in 0..5 {
        let at = (0.25 + 0.5 * k as f64) * rate as f64;
        for i in 0..(0.3 * rate as f64) as usize {
            let t = i as f64 / rate as f64;
            let ph = std::f64::consts::TAU * (50.0 * t + 100.0 * 0.03 * (1.0 - (-t / 0.03).exp()));
            if let Some(v) = x.get_mut(at as usize + i) {
                *v += (0.8 * ph.sin() * (-t / 0.15).exp()) as f32;
            }
        }
    }
    let mut r = rig_with(&x, rate);
    let models = cantus_content::models::Models::new(
        r._dir.path().join("Models"),
        cantus_content::models::ModelManifest::bundled(),
    );
    r.app.ml = cantus_ui::ml_service::MlService::new(
        models,
        cantus_ml::client::ClientConfig::new(ml_support::helper_exe()),
        true,
    );
    let (t, c) = ids(&r.app);
    run::open(&mut r.app, t, c);
    r.app.transcribe.dialog.as_mut().unwrap().settings.mode = Mode::Drums;
    run::start(&mut r.app);
    run_until_done(&mut r);
    assert!(
        matches!(r.app.transcribe.last, Some(Ok(_))),
        "{:?}",
        r.app.transcribe.last
    );
    let new = &r.app.project().tracks[2];
    assert!(matches!(new.instrument, InstrumentSpec::Pulsus(_)));
    let keys: Vec<u8> = new.clips[0].notes.iter().map(|n| n.pitch).collect();
    assert_eq!(keys, [36; 5], "{:?}", new.clips[0].notes);
}

#[test]
fn polyphonic_without_its_model_asks_to_download_and_stop_changes_nothing() {
    let mut r = voice_rig();
    let before = comparable(r.app.project());
    let (t, c) = ids(&r.app);
    assert_eq!(run::needed(Mode::Polyphonic), Some(tr::NOTE_MODEL));
    assert_eq!(run::needed(Mode::Melody), None);
    assert_eq!(run::needed(Mode::Drums), None);
    run::open(&mut r.app, t, c);
    run::stop(&mut r.app);
    assert_eq!(comparable(r.app.project()), before);
}

// ---- the agent tool ----

fn agent(r: &mut Rig) -> cantus_mcp::client::TestClient {
    cantus_ui::ai_mcp::wire_with(&mut r.app, &r.ctx, cantus_ui::ai_mcp::catalog::app());
    r.app.settings.ai.allow_edits = true;
    cantus_mcp::client::TestClient::legacy(r.app.ai.server().unwrap().clone(), "2025-11-25")
}

fn call(
    r: &mut Rig,
    c: &mut cantus_mcp::client::TestClient,
    name: &str,
    args: Value,
) -> cantus_mcp::client::ToolCall {
    let app = &mut r.app;
    c.call_tool(name, args, &mut || {
        cantus_ui::ai_mcp::pump(app);
        std::thread::sleep(Duration::from_millis(2));
    })
}

#[test]
fn the_agent_tool_converts_in_one_named_step_and_never_downloads() {
    let _one = AGENTS.lock().unwrap_or_else(|p| p.into_inner());
    let mut r = voice_rig();
    let mut c = agent(&mut r);
    let (t, cl) = ids(&r.app);
    let before = comparable(r.app.project());
    let steps = r.app.bus.history().len();
    let mut res = call(
        &mut r,
        &mut c,
        "audio_to_midi",
        json!({"track": t.0, "clip": cl.0, "mode": "melody"}),
    );
    assert!(!res.is_error, "{}", res.text);
    if let Some(job) = res
        .structured
        .as_ref()
        .and_then(|s| s["job"].as_str())
        .map(str::to_string)
    {
        res = call(
            &mut r,
            &mut c,
            "wait_job",
            json!({"job": job, "timeout_s": 60}),
        );
        assert!(!res.is_error, "{}", res.text);
        res.structured = res.structured.map(|s| s["result"].clone());
    }
    let s = res.structured.expect("structured");
    assert_eq!(s["notes"], 4, "{s}");
    assert_eq!(r.app.bus.history().len(), steps + 1);
    let e = r.app.bus.history()[steps].clone();
    let name = cantus_ui::ai_mcp::history_name(&r.app, &e, String::new());
    assert!(
        name.contains("Take 1") && name.contains("Voice") && name.contains("melody"),
        "{name}"
    );
    r.app.undo();
    assert_eq!(comparable(r.app.project()), before);
    // Basic Pitch is not installed here: refused, nothing downloaded.
    let res = call(
        &mut r,
        &mut c,
        "audio_to_midi",
        json!({"track": t.0, "clip": cl.0, "mode": "polyphonic"}),
    );
    assert!(
        res.is_error && res.text.contains("not installed"),
        "{}",
        res.text
    );
    assert!(!r.app.ml().models().accepted(tr::NOTE_MODEL));
    let res = call(
        &mut r,
        &mut c,
        "audio_to_midi",
        json!({"track": t.0, "clip": 999_999}),
    );
    assert!(
        res.is_error && res.text.contains("not found"),
        "{}",
        res.text
    );
    let res = call(
        &mut r,
        &mut c,
        "audio_to_midi",
        json!({"track": t.0, "clip": cl.0, "quantize": "1/5"}),
    );
    assert!(res.is_error, "{}", res.text);
    let keys = r.app.project().tracks[0].id;
    let res = call(
        &mut r,
        &mut c,
        "audio_to_midi",
        json!({"track": keys.0, "clip": cl.0}),
    );
    assert!(res.is_error, "{}", res.text);
}

/// The agent tests share the cache's agent folders.
static AGENTS: std::sync::Mutex<()> = std::sync::Mutex::new(());

#[test]
fn the_agents_drum_job_leaves_nothing_in_the_job_table() {
    let _one = AGENTS.lock().unwrap_or_else(|p| p.into_inner());
    let mut r = voice_rig();
    let models = cantus_content::models::Models::new(
        r._dir.path().join("Models"),
        cantus_content::models::ModelManifest::bundled(),
    );
    r.app.ml = cantus_ui::ml_service::MlService::new(
        models,
        cantus_ml::client::ClientConfig::new(ml_support::helper_exe()),
        true,
    );
    let mut c = agent(&mut r);
    let (t, cl) = ids(&r.app);
    let mut res = call(
        &mut r,
        &mut c,
        "audio_to_midi",
        json!({"track": t.0, "clip": cl.0, "mode": "drums"}),
    );
    assert!(!res.is_error, "{}", res.text);
    if let Some(job) = res
        .structured
        .as_ref()
        .and_then(|s| s["job"].as_str())
        .map(str::to_string)
    {
        res = call(
            &mut r,
            &mut c,
            "wait_job",
            json!({"job": job, "timeout_s": 120}),
        );
        assert!(!res.is_error, "{}", res.text);
    }
    // The helper's job ended and its starter forgot it (the router makes nothing again).
    std::thread::sleep(Duration::from_millis(200));
    assert_eq!(r.app.ml().kept(), 0);
    // The agents' folders (ids from 2^32; the dialog's tests in this binary have their
    // own) go with their jobs.
    let agents = || {
        std::fs::read_dir(tr::cache_root())
            .map(|d| {
                d.flatten()
                    .filter(|e| {
                        e.file_name()
                            .to_string_lossy()
                            .rsplit('-')
                            .next()
                            .and_then(|n| n.parse::<u64>().ok())
                            .is_some_and(|n| n >= 1 << 32)
                    })
                    .count()
            })
            .unwrap_or(0)
    };
    let t0 = Instant::now();
    while agents() != 0 {
        assert!(
            t0.elapsed() < Duration::from_secs(10),
            "the job's folder went"
        );
        std::thread::sleep(Duration::from_millis(20));
    }
}

#[test]
fn the_tools_said_names_its_target() {
    let r = voice_rig();
    let (t, c) = ids(&r.app);
    let src = tr::source(&r.app, t, c).unwrap();
    let said = cantus_ui::ai_tools::transcribe::said(&src, Mode::Melody);
    // Worded as the card words it (Said's arguments are SaidArg since the mcp contract).
    let words = cantus_mcp::AgentHost::say(&r.app, &said);
    assert_eq!(
        words,
        "Convert “Take 1” on “Voice” to MIDI (melody) on a new track"
    );
}

#[test]
fn every_transcribe_string_is_in_both_languages() {
    let keys = [
        "transcribe.menu",
        "transcribe.title",
        "transcribe.convert",
        "transcribe.classes.without_model",
        "cmd.transcribe",
        "ai.transcribe.said",
    ];
    for k in keys {
        for l in cantus_ui::i18n::Locale::ALL {
            cantus_ui::i18n::set_thread_locale(l);
            assert_ne!(cantus_ui::i18n::t(k), k, "{k} in {l:?}");
        }
    }
}

// ---- round 34's integrated review ----

/// The agent tool's job, prepared as the MCP layer prepares it.
fn agent_job(
    app: &mut CantusApp,
    args: Value,
) -> Result<cantus_ai::tool::Job, cantus_ai::tool::ToolError> {
    let cantus_ai::tool::ToolRun::Job(prepare) = cantus_ui::ai_tools::transcribe::TOOLS[0].run
    else {
        panic!("audio_to_midi is a job")
    };
    prepare(&mut cantus_ai::tool::ToolCx { host: app }, &args)
}

#[test]
fn remove_is_refused_while_an_agents_conversion_reads_its_take() {
    use cantus_ai::tool::JobCx;
    use cantus_ui::stems_ui::models;
    use std::sync::atomic::{AtomicBool, Ordering};
    let _one = AGENTS.lock().unwrap_or_else(|p| p.into_inner());
    let mut r = voice_rig();
    let (t, c) = ids(&r.app);
    let job = agent_job(
        &mut r.app,
        json!({"track": t.0, "clip": c.0, "mode": "drums"}),
    )
    .unwrap();
    let (reading, release) = (AtomicBool::new(false), AtomicBool::new(false));
    std::thread::scope(|sc| {
        let worker = sc.spawn(|| {
            // The take's reader asks between blocks: held there, before any helper job.
            let cancelled = || {
                reading.store(true, Ordering::SeqCst);
                while !release.load(Ordering::SeqCst) {
                    std::thread::sleep(Duration::from_millis(2));
                }
                true
            };
            (job.work)(&JobCx {
                progress: &|_| {},
                cancelled: &cancelled,
            })
            .is_err()
        });
        let t0 = Instant::now();
        while !reading.load(Ordering::SeqCst) {
            assert!(t0.elapsed() < Duration::from_secs(60));
            std::thread::sleep(Duration::from_millis(2));
        }
        assert!(
            !r.app.ml().busy(),
            "no helper job runs while the take is read"
        );
        let e = models::request_remove(&mut r.app, tr::DRUM_MODEL).unwrap_err();
        assert!(e.contains("In use"), "{e}");
        release.store(true, Ordering::SeqCst);
        assert!(worker.join().unwrap(), "cancelled");
    });
    // The job ended: the helper is free again.
    assert!(models::request_remove(&mut r.app, tr::DRUM_MODEL).is_ok());
}

/// The voice rig's clip moved from the arrangement into a session slot.
fn session_rig() -> (Rig, TrackId, ClipId) {
    let mut r = voice_rig();
    let mut p = r.app.project().clone();
    let v = &mut p.tracks[1];
    let clip = v.clips.remove(0);
    let (t, c) = (v.id, clip.id);
    v.slots.push(cantus_core::session::Slot {
        scene: cantus_core::session::SceneId(1),
        clip: Some(cantus_core::session::SessionClip::new(clip)),
        stop: true,
    });
    r.app.replace_project(p);
    (r, t, c)
}

#[test]
fn a_session_clip_is_refused_with_words_that_say_why() {
    let (mut r, t, c) = session_rig();
    assert!(r.app.project().track(t).unwrap().clip(c).is_some());
    let e = tr::source(&r.app, t, c).unwrap_err();
    assert!(e.contains("session clip"), "{e}");
    match agent_job(
        &mut r.app,
        json!({"track": t.0, "clip": c.0, "mode": "melody"}),
    ) {
        Err(cantus_ai::tool::ToolError::Refused(why)) => {
            assert!(why.contains("session clip"), "{why}")
        }
        Err(e) => panic!("{e:?}"),
        Ok(_) => panic!("a session clip was accepted"),
    }
    let desc = cantus_ui::ai_tools::transcribe::TOOLS[0].description;
    assert!(desc.contains("session clip"), "{desc}");
}

#[test]
fn a_drum_conversions_kit_speaks_the_interfaces_language() {
    let r = voice_rig();
    let (t, c) = ids(&r.app);
    let src = tr::source(&r.app, t, c).unwrap();
    let mut r = r;
    cantus_ui::i18n::set_thread_locale(cantus_ui::i18n::Locale::Es);
    let d = Settings {
        mode: Mode::Drums,
        ..Settings::default()
    };
    let hits = Transcription {
        mode: Mode::Drums,
        duration: 3.0,
        notes: Vec::new(),
        hits: vec![DrumHit {
            time: 0.5,
            class: DrumClass::Kick,
            key: 36,
            velocity: 100,
        }],
    };
    let p = tr::plan(&mut r.app.bus, &src, &d, &hits).unwrap();
    cantus_ui::i18n::set_thread_locale(cantus_ui::i18n::Locale::En);
    let Command::Batch { commands, .. } = &p.command else {
        panic!()
    };
    let Command::InsertTrack { track, .. } = &commands[0] else {
        panic!()
    };
    let InstrumentSpec::Pulsus(kit) = &track.instrument else {
        panic!()
    };
    let english = PulsusParams::studio();
    let kick = kit.pads.iter().find(|p| p.pitch == 36).unwrap();
    assert_ne!(kick.name, "Kick", "the kick in Spanish");
    cantus_ui::i18n::set_thread_locale(cantus_ui::i18n::Locale::Es);
    for (got, en) in kit.pads.iter().zip(&english.pads) {
        assert_eq!(got.name, cantus_ui::app::localized_pad_name(en));
    }
    cantus_ui::i18n::set_thread_locale(cantus_ui::i18n::Locale::En);
}
