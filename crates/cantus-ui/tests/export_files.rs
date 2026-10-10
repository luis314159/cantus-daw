//! Each choice of the export dialog, through the application, against the file it wrote
//! (round 23, BACKLOG §1): the dialog's settings go through `export::start` on the
//! export thread, and the file read back has the rate, channels, sample format, range,
//! tail and loudness the dialog said — the same render the engine makes, to the bit, when
//! nothing converts it. The crate's own tests (`cantus-export/tests/options.rs`) check
//! every format's encoder; these check that the dialog's choices reach it.

use cantus_core::Ticks;
use cantus_engine::{EngineHandle, RenderOptions};
use cantus_export::{Channels, Depth, Format, Rate, Scope};
use cantus_ui::i18n::{self, Locale};
use cantus_ui::views::export;
use cantus_ui::{starter_project, CantusApp};
use std::path::{Path, PathBuf};

static LANGUAGE: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn language() -> std::sync::MutexGuard<'static, ()> {
    let guard = LANGUAGE.lock().unwrap_or_else(|e| e.into_inner());
    i18n::set_locale(Locale::En);
    guard
}

fn app(dir: &Path) -> CantusApp {
    app_with(dir, starter_project())
}

fn app_with(dir: &Path, project: cantus_core::Project) -> CantusApp {
    let mut a = CantusApp::from_parts(Some(project), EngineHandle::dummy(), false);
    a.settings_path = dir.join("settings.json");
    a
}

/// The starter project with notes on every beat of its 16-beat clip, so any range of it
/// sounds — and a range's tail would pick up the notes after it.
fn busy_project() -> cantus_core::Project {
    let mut p = starter_project();
    let mut notes = Vec::new();
    for beat in 4..16 {
        notes.push(p.make_note(
            Ticks::beats(beat),
            Ticks::QUARTER,
            60 + (beat % 7) as u8,
            110,
        ));
    }
    p.tracks[0].clips[0].notes.extend(notes);
    p
}

/// `p` without the notes that start at or after `at` on the timeline.
fn without_notes_from(p: &cantus_core::Project, at: Ticks) -> cantus_core::Project {
    let mut q = p.clone();
    for t in &mut q.tracks {
        for c in &mut t.clips {
            let start = c.start;
            c.notes.retain(|n| start + n.start < at);
        }
    }
    q
}

/// Open the dialog on a 32-bit float WAV at the session's rate, stereo, as mixed: the
/// file is then the engine's render itself.
fn plain(a: &mut CantusApp) {
    export::open(a, None);
    let d = a.export.dialog.as_mut().unwrap();
    d.scope = Scope::Song;
    d.format = Format::Wav;
    d.depth = Depth::Float32;
    d.rate = Rate::Session;
    d.channels = Channels::Stereo;
    d.loudness.target_lufs = None;
    d.tail_seconds = 0.5;
}

fn run(a: &mut CantusApp, target: &Path) {
    assert!(export::start(a, target), "the export started");
    export::wait(a);
    assert!(!a.status.error, "{}", a.status.text);
}

fn wav(path: &Path) -> (hound::WavSpec, Vec<f32>) {
    let r = hound::WavReader::open(path).unwrap();
    let spec = r.spec();
    let samples = match spec.sample_format {
        hound::SampleFormat::Float => r.into_samples::<f32>().map(|s| s.unwrap()).collect(),
        hound::SampleFormat::Int => {
            let full = (1i64 << (spec.bits_per_sample - 1)) as f32;
            r.into_samples::<i32>()
                .map(|s| s.unwrap() as f32 / full)
                .collect()
        }
    };
    (spec, samples)
}

/// The engine's render of the app's project with `opts` (stereo, at 48 kHz, aligned).
fn engine(a: &mut CantusApp, opts: RenderOptions) -> Vec<f32> {
    let snap = a.engine.compile(a.bus.project(), a.bus.revision());
    cantus_engine::render(
        snap,
        &RenderOptions {
            sample_rate: 48_000,
            compensate_latency: true,
            ..opts
        },
    )
    .unwrap()
    .samples
}

#[test]
fn the_song_is_the_engines_render_with_the_tail_chosen() {
    let _l = language();
    let dir = tempfile::tempdir().unwrap();
    let mut a = app(dir.path());
    for tail in [0.0f32, 0.5, 2.0] {
        plain(&mut a);
        a.export.dialog.as_mut().unwrap().tail_seconds = tail;
        let path = dir.path().join(format!("tail{tail}.wav"));
        run(&mut a, &path);
        let (spec, got) = wav(&path);
        assert_eq!(
            (spec.sample_rate, spec.channels, spec.bits_per_sample),
            (48_000, 2, 32)
        );
        let want = engine(
            &mut a,
            RenderOptions {
                tail_seconds: tail as f64,
                ..Default::default()
            },
        );
        assert_eq!(got.len(), want.len(), "tail {tail}: the length");
        assert!(got == want, "tail {tail}: the engine's render, to the bit");
    }
}

#[test]
fn the_selection_is_the_range_and_its_tail_rings_out_alone() {
    let _l = language();
    let dir = tempfile::tempdir().unwrap();
    let mut a = app_with(dir.path(), busy_project());
    let (from, to) = (Ticks::beats(4), Ticks::beats(8));
    a.set_time_selection(from, to);
    plain(&mut a);
    a.export.dialog.as_mut().unwrap().scope = Scope::Selection;
    let path = dir.path().join("sel.wav");
    run(&mut a, &path);
    let (_, got) = wav(&path);
    let want = engine(
        &mut a,
        RenderOptions {
            start: from,
            end: Some(to),
            tail_seconds: 0.5,
            ..Default::default()
        },
    );
    assert!(got.iter().any(|x| *x != 0.0), "the range sounds");
    assert!(got == want, "the selection's render");
    let secs = (to - from).to_seconds(a.project().tempo_bpm) + 0.5;
    assert_eq!(got.len() / 2, (secs * 48_000.0).ceil() as usize);
    // Alone: the same as the project without the notes from the selection's end on.
    let mut b = app_with(dir.path(), without_notes_from(&busy_project(), to));
    let alone = engine(
        &mut b,
        RenderOptions {
            start: from,
            end: Some(to),
            tail_seconds: 0.5,
            ..Default::default()
        },
    );
    assert!(
        got == alone,
        "the tail rings out what was playing and nothing after it"
    );
    // And it is that range, not another of the same length.
    let earlier = engine(
        &mut a,
        RenderOptions {
            start: Ticks::ZERO,
            end: Some(from),
            tail_seconds: 0.5,
            ..Default::default()
        },
    );
    assert!(got != earlier, "a range starting elsewhere differs");
}

#[test]
fn the_loop_range_exports_the_loop() {
    let _l = language();
    let dir = tempfile::tempdir().unwrap();
    let mut p = busy_project();
    p.loop_region = Some(cantus_core::LoopRegion {
        start: Ticks::beats(5),
        end: Ticks::beats(9),
    });
    let mut a = app_with(dir.path(), p);
    let lr = a.project().loop_region.clone().expect("the loop brace");
    plain(&mut a);
    a.export.dialog.as_mut().unwrap().scope = Scope::Loop;
    let path = dir.path().join("loop.wav");
    run(&mut a, &path);
    let (_, got) = wav(&path);
    let want = engine(
        &mut a,
        RenderOptions {
            start: lr.start,
            end: Some(lr.end),
            tail_seconds: 0.5,
            ..Default::default()
        },
    );
    assert!(got.iter().any(|x| *x != 0.0), "the loop sounds");
    assert!(got == want, "the loop's render");
    let secs = (lr.end - lr.start).to_seconds(a.project().tempo_bpm) + 0.5;
    assert_eq!(
        got.len() / 2,
        (secs * 48_000.0).ceil() as usize,
        "the loop's length"
    );
    let song = engine(
        &mut a,
        RenderOptions {
            tail_seconds: 0.5,
            ..Default::default()
        },
    );
    assert_ne!(got.len(), song.len(), "not the song");
}

#[test]
fn the_rate_channels_and_depth_chosen_are_the_files() {
    let _l = language();
    let dir = tempfile::tempdir().unwrap();
    let mut a = app(dir.path());
    for (rate, channels, depth, want) in [
        (
            Rate::Hz(44_100),
            Channels::Stereo,
            Depth::Int16,
            (44_100, 2, 16),
        ),
        (
            Rate::Hz(96_000),
            Channels::Mono,
            Depth::Int24,
            (96_000, 1, 24),
        ),
        (
            Rate::Session,
            Channels::Mono,
            Depth::Float32,
            (48_000, 1, 32),
        ),
    ] {
        plain(&mut a);
        {
            let d = a.export.dialog.as_mut().unwrap();
            d.rate = rate;
            d.channels = channels;
            d.depth = depth;
        }
        let path = dir.path().join(format!("{}-{}.wav", want.0, want.1));
        run(&mut a, &path);
        let (spec, _) = wav(&path);
        assert_eq!(
            (spec.sample_rate, spec.channels, spec.bits_per_sample),
            want,
            "{rate:?} {channels:?} {depth:?}"
        );
    }
}

#[test]
fn the_loudness_target_is_the_files_loudness_and_the_summary_says_it() {
    let _l = language();
    let dir = tempfile::tempdir().unwrap();
    let mut a = app(dir.path());
    plain(&mut a);
    {
        let d = a.export.dialog.as_mut().unwrap();
        d.loudness.target_lufs = Some(-16.0);
        d.loudness.ceiling_dbtp = -1.0;
        d.loudness.limit = true;
    }
    let path = dir.path().join("loud.wav");
    run(&mut a, &path);
    let (spec, got) = wav(&path);
    let audio = cantus_export::Audio {
        rate: spec.sample_rate,
        channels: spec.channels,
        samples: got,
    };
    let (lufs, tp) = cantus_export::process::measure(&audio);
    let lufs = lufs.expect("measured");
    assert!((lufs + 16.0).abs() < 0.5, "{lufs:.2} LUFS");
    assert!(tp <= -1.0 + 0.1, "{tp:.2} dBTP");
    let last = a.export.last.clone().unwrap().unwrap();
    assert!(last.contains("LUFS"), "{last}");
}

#[test]
fn a_name_with_another_extension_gets_the_formats() {
    let _l = language();
    let dir = tempfile::tempdir().unwrap();
    let mut a = app(dir.path());
    plain(&mut a);
    a.export.dialog.as_mut().unwrap().format = Format::Flac;
    // Typed in the save dialog as `mix.wav` while FLAC was chosen; and `mix` alone.
    run(&mut a, &dir.path().join("mix.wav"));
    run(&mut a, &dir.path().join("take two"));
    let mut names: Vec<String> = std::fs::read_dir(dir.path())
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .filter(|n| n != "settings.json")
        .collect();
    names.sort();
    assert_eq!(
        names,
        vec!["mix.flac".to_string(), "take two.flac".to_string()]
    );
    let head = std::fs::read(dir.path().join("mix.flac")).unwrap();
    assert_eq!(&head[..4], b"fLaC");
}

#[test]
fn stems_say_they_are_stems_even_when_one_comes_out() {
    let _l = language();
    let dir = tempfile::tempdir().unwrap();
    let mut a = app(dir.path());
    export::open(&mut a, Some(Scope::Stems));
    a.export.dialog.as_mut().unwrap().tail_seconds = 0.5;
    let out: PathBuf = dir.path().join("stems");
    run(&mut a, &out);
    let last = a.export.last.clone().unwrap().unwrap();
    assert!(last.contains(&out.display().to_string()), "{last}");
    assert!(last.contains("stem"), "the stems summary: {last}");
}

#[test]
fn export_audio_after_export_stems_opens_on_the_mix() {
    let _l = language();
    let dir = tempfile::tempdir().unwrap();
    let mut a = app(dir.path());
    export::open(&mut a, Some(Scope::Stems));
    export::close(&mut a);
    assert_eq!(a.settings.export.scope, Scope::Stems);
    a.export_dialog();
    assert_eq!(a.export.dialog.as_ref().unwrap().scope, Scope::Song);
    export::close(&mut a);
    export::open(&mut a, Some(Scope::Stems));
    assert_eq!(a.export.dialog.as_ref().unwrap().scope, Scope::Stems);
}

#[test]
fn the_bar_shows_the_renders_progress_and_the_status_line_follows() {
    let _l = language();
    let dir = tempfile::tempdir().unwrap();
    let mut a = app(dir.path());
    plain(&mut a);
    a.export.dialog.as_mut().unwrap().tail_seconds = 30.0;
    assert!(export::start(&mut a, &dir.path().join("long.wav")));
    let mut during_render = None;
    let started = std::time::Instant::now();
    while a.export.busy() && started.elapsed().as_secs() < 60 {
        if let Some((phase, f)) = a.export.progress() {
            if phase == "export_audio.phase.render" && f.is_some_and(|f| f > 0.0) {
                during_render = f;
                export::poll(&mut a);
                break;
            }
        }
        std::thread::yield_now();
    }
    let f = during_render.expect("the render reports a fraction, not a pulse");
    assert!(f > 0.0 && f <= 0.5, "{f}");
    assert!(
        a.status.text.contains('%'),
        "the status line: {}",
        a.status.text
    );
    export::stop(&mut a);
    export::wait(&mut a);
}

#[test]
fn the_errors_are_in_the_interfaces_language() {
    let _l = language();
    i18n::set_locale(Locale::Es);
    let dir = tempfile::tempdir().unwrap();
    let mut a = app(dir.path());
    plain(&mut a);
    // A folder where the file should go: the write fails.
    let blocked = dir.path().join("blocked.wav");
    std::fs::create_dir_all(blocked.join("x")).unwrap();
    assert!(export::start(&mut a, &blocked));
    export::wait(&mut a);
    i18n::set_locale(Locale::En);
    let last = a.export.last.clone().unwrap().unwrap_err();
    assert!(
        last.starts_with("Falló la exportación: no se pudo escribir el archivo"),
        "in Spanish, with the system's reason after it: {last}"
    );
}

#[test]
fn the_loop_range_is_written_to_the_settings_file_as_the_song() {
    let _l = language();
    let dir = tempfile::tempdir().unwrap();
    let mut a = app(dir.path());
    plain(&mut a);
    a.export.dialog.as_mut().unwrap().scope = Scope::Loop;
    export::close(&mut a);
    let text = std::fs::read_to_string(&a.settings_path).unwrap();
    assert!(
        !text.contains("\"Loop\""),
        "builds out before round 23 cannot read it"
    );
    assert_eq!(a.settings.export.scope, Scope::Song);
}

#[test]
fn a_settings_field_this_build_cannot_read_keeps_the_others() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("settings.json");
    let mut s = cantus_ui::settings::Settings::default();
    s.export.tags.artist = "Coro".into();
    s.save(&path).unwrap();
    // A newer build wrote a choice this one does not know.
    let text = std::fs::read_to_string(&path).unwrap();
    let mut v: serde_json::Value = serde_json::from_str(&text).unwrap();
    v["export"]["scope"] = serde_json::json!("SomethingNew");
    v["autosave_seconds"] = serde_json::json!(420);
    std::fs::write(&path, v.to_string()).unwrap();
    let back = cantus_ui::settings::Settings::load(&path);
    assert_eq!(
        back.export,
        cantus_export::ExportSettings::default(),
        "the unknown field's default"
    );
    assert_eq!(
        back.autosave_seconds, 420,
        "and the other fields are the user's"
    );
}
