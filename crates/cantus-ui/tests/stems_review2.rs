//! *Separate stems…*, the second review's points (round 34, ADR 0085 §5), each held here
//! with the helper and the tests' stand-in network (each part a quarter of the track, so
//! the parts sum to the original): the stems sound at the original's level under every
//! pan law, muted clips stay muted, crossfades and comp seams are kept, a selection over
//! several tracks is refused, a separation never lands in another project. Its own test
//! binary: it points `CANTUS_DATA_DIR` at a temporary folder.

mod ml_support;
mod stems_support;

use cantus_core::{Command, Origin, PanLaw, Ticks};
use cantus_ui::stems_ui::{run, Target};
use cantus_ui::CantusApp;
use stems_support::*;

/// The engine's render of the app's project at 48 kHz, as an export renders it.
fn render(app: &mut CantusApp) -> Vec<f32> {
    let snap = app
        .engine
        .compile_for_render(app.bus.project(), app.bus.revision());
    cantus_engine::render(
        snap,
        &cantus_engine::RenderOptions {
            sample_rate: 48_000,
            tail_seconds: 0.0,
            compensate_latency: true,
            ..Default::default()
        },
    )
    .unwrap()
    .samples
}

/// The level of `b` against `a` in dB, and what is left of `a` in `b` (SNR, dB).
fn compare(a: &[f32], b: &[f32]) -> (f64, f64) {
    let n = a.len().min(b.len());
    let (mut ea, mut eb, mut err) = (0f64, 0f64, 0f64);
    for i in 0..n {
        let (x, y) = (a[i] as f64, b[i] as f64);
        ea += x * x;
        eb += y * y;
        err += (x - y) * (x - y);
    }
    (
        10.0 * (eb / ea).log10(),
        10.0 * (ea / err.max(1e-30)).log10(),
    )
}

/// The original against the sum of its stems, rendered: the same level, the same sound,
/// under the default law and the strongest; with the original on a strip and without.
#[test]
fn the_stems_play_at_the_originals_level_under_every_pan_law() {
    let (_one, data) = data_folder();
    let x = tone(48_000, 4.0);
    let f = data.path().join("song.wav");
    write(&f, 48_000, &x);
    for (law, strip) in [
        (PanLaw::Minus3dB, false),
        (PanLaw::Minus6dB, true),
        (PanLaw::ZeroDb, false),
    ] {
        let (p, tid, _) = project(&[(f.clone(), 48_000, x.0.len() as u64)]);
        let mut app = app_with(p, data.path());
        assert!(app.exec(Command::SetPanLaw { law }));
        if strip {
            let s = app.bus.new_strip("Mix");
            let sid = s.id;
            let index = app.project().strips.len();
            app.bus
                .execute(
                    Command::Batch {
                        label_key: "strip".into(),
                        commands: vec![
                            Command::InsertStrip {
                                index,
                                strip: Box::new(s),
                            },
                            Command::SetTrackStrip {
                                id: tid,
                                strip: Some(sid),
                            },
                        ],
                    },
                    Origin::User,
                )
                .unwrap();
        }
        let before = render(&mut app);
        run::start(&mut app, Target::Track(tid), fixture_model()).unwrap();
        until_done(&mut app);
        assert_eq!(app.stems.runs[0].phase, run::Phase::Done);
        let after = render(&mut app);
        let (level, snr) = compare(&before, &after);
        assert!(
            level.abs() < 0.05,
            "{law:?} (strip {strip}): the stems are {level:.2} dB against the original"
        );
        assert!(snr > 40.0, "{law:?} (strip {strip}): {snr:.1} dB");
    }
}

/// A clip the user muted keeps its parts silent: only what was playing plays again
/// (review 2 #2).
#[test]
fn a_muted_clips_parts_stay_muted() {
    let (_one, data) = data_folder();
    let x = tone(44_100, 3.0);
    let f = data.path().join("m.wav");
    write(&f, 44_100, &x);
    let n = x.0.len() as u64;
    let (p, tid, clips) = project(&[(f.clone(), 44_100, n), (f, 44_100, n)]);
    let mut app = app_with(p, data.path());
    assert!(app.exec(Command::SetClipMute {
        track: tid,
        clip: clips[1],
        muted: true,
    }));
    run::start(&mut app, Target::Track(tid), fixture_model()).unwrap();
    until_done(&mut app);
    let p = app.project();
    for t in &p.tracks[1..] {
        let muted: Vec<bool> = t.clips.iter().map(|c| c.muted).collect();
        assert_eq!(muted, [false, true], "{}", t.name);
    }
    let orig = p.track(tid).unwrap();
    assert!(orig.clips.iter().all(|c| c.muted));
}

/// A comped track's seams and an overlap's crossfade stay on every part, and the parts
/// still sum to what the track played (review 2 #3).
#[test]
fn crossfades_and_comp_seams_are_kept() {
    let (_one, data) = data_folder();
    let x = tone(48_000, 8.0);
    let f = data.path().join("take.wav");
    write(&f, 48_000, &x);
    let (p, tid, clips) = project(&[(f, 48_000, x.0.len() as u64)]);
    let mut app = app_with(p, data.path());
    let source = app.project().audio_pool[0].id;
    // Two comp windows meeting at beat 4 with a seam, and a clip laid over the second's
    // end with a crossfade over the overlap.
    let first = app
        .project()
        .track(tid)
        .unwrap()
        .clip(clips[0])
        .unwrap()
        .clone();
    let mut a = first.clone();
    a.length = Ticks::beats(4);
    a.comp = true;
    let mut b = app.bus.new_clip("B", Ticks::beats(4), Ticks::beats(4));
    let mut region = cantus_core::AudioRegion::of(source);
    region.xfade = Ticks::beats(1) / 2;
    b.audio = Some(region);
    b.offset = Ticks::beats(3);
    b.comp = true;
    let mut c = app.bus.new_clip("C", Ticks::beats(7), Ticks::beats(4));
    let mut region = cantus_core::AudioRegion::of(source);
    region.cross = Some(cantus_core::crossfade::XfadeCurve::SCurve);
    c.audio = Some(region);
    c.offset = Ticks::beats(2);
    let cmds = vec![
        Command::RemoveClip {
            track: tid,
            clip: first.id,
        },
        Command::InsertClip {
            track: tid,
            clip: Box::new(a),
        },
        Command::InsertClip {
            track: tid,
            clip: Box::new(b),
        },
        Command::InsertClip {
            track: tid,
            clip: Box::new(c),
        },
    ];
    app.bus
        .execute(
            Command::Batch {
                label_key: "comp".into(),
                commands: cmds,
            },
            Origin::User,
        )
        .unwrap();
    let before_project = app.project().clone();
    let before = render(&mut app);
    run::start(&mut app, Target::Track(tid), fixture_model()).unwrap();
    until_done(&mut app);
    assert_eq!(app.stems.runs[0].phase, run::Phase::Done);
    let orig = before_project.track(tid).unwrap();
    for t in &app.project().tracks[1..] {
        assert_eq!(t.clips.len(), orig.clips.len());
        for (c, o) in t.clips.iter().zip(&orig.clips) {
            let (ca, oa) = (c.audio.as_ref().unwrap(), o.audio.as_ref().unwrap());
            assert_eq!(
                (c.comp, c.lane, ca.xfade, ca.cross),
                (o.comp, o.lane, oa.xfade, oa.cross),
                "{} on {}",
                o.name,
                t.name
            );
            assert_ne!(c.id, o.id);
            assert_eq!(c.link, None);
        }
    }
    let after = render(&mut app);
    let (level, snr) = compare(&before, &after);
    assert!(level.abs() < 0.05, "{level:.2} dB");
    assert!(snr > 40.0, "{snr:.1} dB");
}

/// The tool's job with the stand-in model in place of an installed one.
fn tool_job(
    app: &mut CantusApp,
    args: &serde_json::Value,
) -> Result<cantus_ai::tool::Job, cantus_ai::tool::ToolError> {
    cantus_ui::ai_tools::stems::prepare_with_model(app, args, fixture_model())
}

/// Clips on two tracks are refused everywhere — the menu, a run, the agent tool, the plan —
/// in words that say why: each track's parts go under a bus where that track fed, at its
/// level (review 2 #13).
#[test]
fn a_selection_over_several_tracks_is_refused() {
    let (_one, data) = data_folder();
    cantus_ui::i18n::set_thread_locale(cantus_ui::i18n::Locale::En);
    let x = tone(44_100, 2.0);
    let f = data.path().join("two.wav");
    write(&f, 44_100, &x);
    let (p, tid, clips) = project(&[(f, 44_100, x.0.len() as u64)]);
    let mut app = app_with(p, data.path());
    // A second track with a copy of the clip, on a strip of its own.
    let mut other = app.bus.new_track("Other");
    let oid = other.id;
    let mut copy = app.project().track(tid).unwrap().clips[0].clone();
    copy.id = cantus_core::ClipId(app.bus.alloc_id());
    other.clips.push(copy.clone());
    assert!(app.exec(Command::InsertTrack {
        index: 1,
        track: Box::new(other),
    }));
    let target = Target::Clips(vec![(tid, clips[0]), (oid, copy.id)]);
    let before = app.project().clone();
    let e = run::start(&mut app, target.clone(), fixture_model()).unwrap_err();
    assert!(e.contains("one track"), "{e}");
    app.open_separate_stems(target);
    assert!(app.stems.dialog.is_none());
    let args = serde_json::json!({"clip_ids": [clips[0].0, copy.id.0]});
    assert!(matches!(
        tool_job(&mut app, &args),
        Err(cantus_ai::tool::ToolError::BadArgument { .. })
    ));
    assert_eq!(app.project(), &before);
}

/// The agent tool takes the arrangement's clips only: a session clip's id is not one, so
/// it never gets an arrangement clip's parts (review 2 #14).
#[test]
fn a_session_clip_is_not_an_arrangement_clip() {
    let (_one, data) = data_folder();
    let x = tone(44_100, 2.0);
    let f = data.path().join("session.wav");
    write(&f, 44_100, &x);
    let (mut p, _tid, _) = project(&[(f, 44_100, x.0.len() as u64)]);
    let mut clip = p.tracks[0].clips[0].clone();
    clip.id = cantus_core::ClipId(p.next_id + 1);
    p.next_id += 2;
    let sid = clip.id;
    p.tracks[0].slots.push(cantus_core::session::Slot {
        scene: cantus_core::session::SceneId(p.next_id + 7),
        clip: Some(cantus_core::session::SessionClip::new(clip)),
        stop: false,
    });
    let tid = p.tracks[0].id;
    let mut app = app_with(p, data.path());
    assert!(
        app.project().track(tid).unwrap().clip(sid).is_some(),
        "the session clip is found by Track::clip"
    );
    let args = serde_json::json!({"clip_ids": [sid.0]});
    assert!(matches!(
        tool_job(&mut app, &args),
        Err(cantus_ai::tool::ToolError::NotFound(_))
    ));
    assert!(Target::Clips(vec![(tid, sid)])
        .clips(app.project())
        .is_empty());
}

/// A file converted a block at a time gives back, to the bit, what converting it whole
/// gives — down to the model's rate and back up, from memory and from a WAV read in
/// pieces, past several blocks (review 2 #8).
#[test]
fn a_block_at_a_time_is_the_whole_file_to_the_bit() {
    use cantus_ui::stems_ui::convert::{self, Frames};
    let dir = tempfile::tempdir().unwrap();
    for (from, to) in [
        (48_000u32, 44_100u32),
        (96_000, 44_100),
        (44_100, 48_000),
        (44_100, 192_000),
        (22_050, 44_100),
        (44_100, 44_100),
    ] {
        let x = tone(from, 5.3);
        let inter: Vec<f32> = x.0.iter().zip(&x.1).flat_map(|(l, r)| [*l, *r]).collect();
        let whole = cantus_engine::samples::resample::resample(&inter, 2, from, to);
        let wav = dir.path().join(format!("in-{from}.wav"));
        write(&wav, from, &x);
        for from_file in [false, true] {
            let out = dir.path().join(format!("out-{from}-{to}-{from_file}.wav"));
            let mut o = convert::Out::create(&out, 2, to).unwrap();
            let n = if from_file {
                let (mut w, rate) = convert::Wav::open(&wav).unwrap();
                assert_eq!((rate, w.channels()), (from, 2));
                convert::convert(&mut w, from, to, None, &mut o, &|| false).unwrap()
            } else {
                let mut m = convert::Memory {
                    data: &inter,
                    channels: 2,
                };
                convert::convert(&mut m, from, to, None, &mut o, &|| false).unwrap()
            };
            o.finish().unwrap();
            let (rate, l, r) = read(&out);
            assert_eq!(rate, to);
            assert_eq!(n as usize * 2, whole.len());
            let got: Vec<f32> = l.iter().zip(&r).flat_map(|(a, b)| [*a, *b]).collect();
            assert!(
                got == whole,
                "{from} -> {to} (file {from_file}): not the whole file's samples"
            );
        }
    }
}

/// A 24-bit WAV is read and scaled as the decoder does; a file whose stems would pass a
/// WAV's 4 GB is refused before anything is done, in words that say so.
#[test]
fn integer_wavs_convert_and_files_past_4_gb_are_refused() {
    let (_one, data) = data_folder();
    cantus_ui::i18n::set_thread_locale(cantus_ui::i18n::Locale::En);
    let x = tone(48_000, 2.0);
    let f = data.path().join("int24.wav");
    {
        let spec = hound::WavSpec {
            channels: 2,
            sample_rate: 48_000,
            bits_per_sample: 24,
            sample_format: hound::SampleFormat::Int,
        };
        let mut w = hound::WavWriter::create(&f, spec).unwrap();
        for i in 0..x.0.len() {
            w.write_sample((x.0[i] * 8_388_607.0) as i32).unwrap();
            w.write_sample((x.1[i] * 8_388_607.0) as i32).unwrap();
        }
        w.finalize().unwrap();
    }
    let decoded = cantus_engine::samples::assets::decode_any(&f).unwrap();
    let want = cantus_engine::samples::resample::resample(&decoded.frames, 2, 48_000, 44_100);
    let (input, rate, frames) = run::prepare(&f, &data.path().join("scratch"), &|| false).unwrap();
    assert_eq!((rate, frames), (48_000, x.0.len() as u64));
    let (r44, l, r) = read(&input);
    assert_eq!(r44, 44_100);
    let got: Vec<f32> = l.iter().zip(&r).flat_map(|(a, b)| [*a, *b]).collect();
    assert_eq!(got.len(), want.len());
    let worst = got
        .iter()
        .zip(&want)
        .map(|(a, b)| (a - b).abs())
        .fold(0f32, f32::max);
    assert!(worst < 1e-6, "{worst}");
    // An hour at 192 kHz: 5.5 GB of stems. An hour at 48 kHz fits.
    assert!(run::too_long(3_600 * 192_000, 192_000));
    assert!(!run::too_long(3_600 * 48_000, 48_000));
    let (mut p, tid, _) = project(&[(f, 48_000, x.0.len() as u64)]);
    p.audio_pool[0].sample_rate = 192_000;
    p.audio_pool[0].frames = 3_600 * 192_000;
    let mut app = app_with(p, data.path());
    let e = run::start(&mut app, Target::Track(tid), fixture_model()).unwrap_err();
    assert!(e.contains("4 GB") && e.contains("192000 Hz"), "{e}");
}

/// The files in the stems' scratch folder (converted inputs).
fn scratch_files() -> Vec<std::path::PathBuf> {
    let dir = cantus_ui::app::data_dir().join("cache").join("stems");
    std::fs::read_dir(dir)
        .map(|d| d.flatten().map(|e| e.path()).collect())
        .unwrap_or_default()
}

/// An agent's separation: its cancel reaches the conversion (no helper job is started
/// once it is asked), and the converted input goes whichever way the job ends — cancelled
/// during the separation too (review 2 #6).
#[test]
fn an_agents_cancel_leaves_nothing_and_stops_the_conversion() {
    let (_one, data) = data_folder();
    let x = tone(48_000, 40.0);
    let f = data.path().join("long48.wav");
    write(&f, 48_000, &x);
    let (p, _tid, _) = project(&[(f, 48_000, x.0.len() as u64)]);
    let mut app = app_with(p, data.path());
    let clips = Target::Track(app.project().tracks[0].id).clips(app.project());
    let model = fixture_model();
    let sources = run::sources_for(&mut app, &clips, &model).unwrap();
    let scratch = cantus_ui::app::data_dir().join("cache").join("stems");
    let ml = app.ml.starter();
    // Cancelled while it separates (after its first progress).
    let started = std::sync::atomic::AtomicBool::new(false);
    let progress = |_: f32| started.store(true, std::sync::atomic::Ordering::Relaxed);
    let cancelled = || started.load(std::sync::atomic::Ordering::Relaxed);
    let r = run::separate_blocking(
        &ml,
        &sources,
        &model,
        0,
        cantus_ml::spec::Device::Cpu,
        &scratch,
        &progress,
        &cancelled,
    );
    assert!(r.is_err());
    assert!(scratch_files().is_empty(), "{:?}", scratch_files());
    // Cancelled during the conversion: no helper job at all.
    let jobs = app.ml.job_count();
    let asked = std::sync::atomic::AtomicUsize::new(0);
    let cancelled = || asked.fetch_add(1, std::sync::atomic::Ordering::Relaxed) >= 2;
    let r = run::separate_blocking(
        &ml,
        &sources,
        &model,
        0,
        cantus_ml::spec::Device::Cpu,
        &scratch,
        &|_| {},
        &cancelled,
    );
    assert_eq!(r.unwrap_err(), "cancelled");
    assert_eq!(app.ml.job_count(), jobs, "a job started after the cancel");
    assert!(scratch_files().is_empty(), "{:?}", scratch_files());
}

/// *Cancel* in the panel removes what a worker wrote only once the worker lets it go:
/// with the worker held at a known point, a cancel leaves the run running and its folder
/// in place; released, the worker answers and everything goes — while the stems come
/// back, and while the input converts (review 2 #6, review 3 #5).
#[test]
fn a_panel_cancel_waits_for_its_worker() {
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;
    let (_one, data) = data_folder();
    let x = tone(96_000, 20.0);
    let f = data.path().join("long96.wav");
    write(&f, 96_000, &x);
    let (p, tid, _) = project(&[(f, 96_000, x.0.len() as u64)]);
    let mut app = app_with(p, data.path());
    for phase in ["returning", "preparing"] {
        run::start(&mut app, Target::Track(tid), fixture_model()).unwrap();
        let i = app.stems.runs.len() - 1;
        let (armed, held, release) = (
            Arc::new(AtomicBool::new(phase == "preparing")),
            Arc::new(AtomicBool::new(false)),
            Arc::new(AtomicBool::new(false)),
        );
        let (a, h, r) = (armed.clone(), held.clone(), release.clone());
        app.stems.runs[i].hold_workers(Arc::new(move || {
            if a.load(Ordering::SeqCst) {
                h.store(true, Ordering::SeqCst);
                while !r.load(Ordering::SeqCst) {
                    std::thread::sleep(std::time::Duration::from_millis(2));
                }
            }
        }));
        let t = std::time::Instant::now();
        let wait = |what: &str, t: &std::time::Instant| {
            assert!(t.elapsed() < std::time::Duration::from_secs(60), "{what}");
            std::thread::sleep(std::time::Duration::from_millis(2));
        };
        if phase == "returning" {
            // Armed once the model has its input: the return's worker is held.
            while !matches!(app.stems.runs[i].phase, run::Phase::Separating(_)) {
                run::step(&mut app);
                wait("separating", &t);
            }
            armed.store(true, Ordering::SeqCst);
        }
        while !held.load(Ordering::SeqCst) {
            run::step(&mut app);
            wait("held", &t);
        }
        run::cancel(&mut app, i);
        for _ in 0..20 {
            run::step(&mut app);
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        assert!(
            app.stems.runs[i].running(),
            "{phase}: ended under its worker"
        );
        for s in &app.stems.runs[i].sources {
            assert!(s.out.exists(), "{phase}: removed under its worker");
        }
        release.store(true, Ordering::SeqCst);
        until_done(&mut app);
        assert_eq!(app.stems.runs[i].phase, run::Phase::Cancelled, "{phase}");
        for s in &app.stems.runs[i].sources {
            assert!(!s.out.exists(), "{phase}: {} left", s.out.display());
        }
        assert!(scratch_files().is_empty(), "{phase}: {:?}", scratch_files());
    }
}

/// A worker that answers wakes the window: a separation moves on while nothing else asks
/// for a frame (review 2 #5).
#[test]
fn a_worker_wakes_the_window_when_it_answers() {
    let (_one, data) = data_folder();
    let x = tone(48_000, 20.0);
    let f = data.path().join("idle.wav");
    write(&f, 48_000, &x);
    let (p, tid, _) = project(&[(f, 48_000, x.0.len() as u64)]);
    let mut app = app_with(p, data.path());
    let ctx = egui::Context::default();
    let woke = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let flag = woke.clone();
    ctx.set_request_repaint_callback(move |_| {
        flag.store(true, std::sync::atomic::Ordering::Relaxed)
    });
    app.stems.repaint = Some(ctx.clone());
    run::start(&mut app, Target::Track(tid), fixture_model()).unwrap();
    // One step hands the conversion to its worker; then no frame at all.
    run::step(&mut app);
    let t = std::time::Instant::now();
    while !woke.load(std::sync::atomic::Ordering::Relaxed) {
        assert!(
            t.elapsed() < std::time::Duration::from_secs(30),
            "the worker answered and the window slept on"
        );
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    run::cancel(&mut app, 0);
    until_done(&mut app);
}

/// The licence row claims no more than the licence gives: the model limits nothing, and
/// the music stays its owner's — in both languages (review 2 #12).
#[test]
fn the_licence_row_claims_no_more_than_the_licence() {
    for (locale, want) in [
        (
            cantus_ui::i18n::Locale::En,
            "MIT: the model puts no limit on what you make; the music's rights are still its owner's",
        ),
        (
            cantus_ui::i18n::Locale::Es,
            "MIT: el modelo no pone límites a lo que hagas; los derechos de la música siguen siendo de su dueño",
        ),
    ] {
        cantus_ui::i18n::set_thread_locale(locale);
        let got = cantus_ui::i18n::tf("stems.detail.licence_value", &[("licence", &"MIT")]);
        assert_eq!(got, want);
    }
    cantus_ui::i18n::set_thread_locale(cantus_ui::i18n::Locale::En);
}

/// *Use the graphics processor* does something only where the runtime has one: the
/// manifest names CoreML for macOS and nothing for the processor-only builds of Windows
/// and Linux, and there a run asks for the processor whatever the setting (review 2 #11).
#[test]
fn the_graphics_processor_is_asked_for_only_where_the_runtime_has_one() {
    let (_one, data) = data_folder();
    let manifest = cantus_content::models::ModelManifest::bundled();
    for (platform, want) in [
        ("windows-x86_64", None),
        ("linux-x86_64", None),
        ("macos-aarch64", Some("coreml")),
        ("macos-x86_64", Some("coreml")),
    ] {
        let r = manifest.runtime("onnxruntime", platform).unwrap();
        assert_eq!(r.gpu.as_deref(), want, "{platform}");
    }
    let x = tone(44_100, 2.0);
    let f = data.path().join("gpu.wav");
    write(&f, 44_100, &x);
    let (p, tid, _) = project(&[(f, 44_100, x.0.len() as u64)]);
    let mut app = app_with(p, data.path());
    app.settings.stems.gpu = true;
    let offered = cantus_ui::stems_ui::gpu_provider(&app, "htdemucs");
    assert_eq!(offered.is_some(), cfg!(target_os = "macos"));
    run::start(&mut app, Target::Track(tid), fixture_model()).unwrap();
    let want = if cfg!(target_os = "macos") {
        cantus_ml::spec::Device::Auto
    } else {
        cantus_ml::spec::Device::Cpu
    };
    assert_eq!(app.stems.runs[0].device, want);
    until_done(&mut app);
}

/// *Remove* is refused while a separation runs — the helper is not ended under it, so the
/// separation finishes — and, asked when nothing runs, it never makes the window wait
/// (review 2 #10).
#[test]
fn remove_waits_for_no_one_and_never_cuts_a_separation() {
    use cantus_ui::stems_ui::models;
    let (_one, data) = data_folder();
    cantus_ui::i18n::set_thread_locale(cantus_ui::i18n::Locale::En);
    let x = tone(44_100, 30.0);
    let f = data.path().join("busy.wav");
    write(&f, 44_100, &x);
    let (p, tid, _) = project(&[(f, 44_100, x.0.len() as u64)]);
    let mut app = app_with(p, data.path());
    run::start(&mut app, Target::Track(tid), fixture_model()).unwrap();
    let t = std::time::Instant::now();
    while !matches!(app.stems.runs[0].phase, run::Phase::Separating(_)) {
        run::step(&mut app);
        assert!(t.elapsed() < std::time::Duration::from_secs(60));
        std::thread::sleep(std::time::Duration::from_millis(2));
    }
    let e = models::request_remove(&mut app, "htdemucs").unwrap_err();
    assert!(e.contains("In use"), "{e}");
    // The words fit a conversion's hold too (transcribe's tool holds the same counter).
    assert!(e.contains("separation or a conversion"), "{e}");
    until_done(&mut app);
    assert_eq!(
        app.stems.runs[0].phase,
        run::Phase::Done,
        "the separation was cut"
    );
    // Nothing runs: asked at once, tried a frame at a time.
    let t = std::time::Instant::now();
    models::request_remove(&mut app, "htdemucs").unwrap();
    assert!(t.elapsed() < std::time::Duration::from_millis(500));
    let t = std::time::Instant::now();
    while app.stems.removing.is_some() {
        run::step(&mut app);
        assert!(t.elapsed() < std::time::Duration::from_secs(10));
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
}

/// The room a separation needs is counted before it starts — the converted input, every
/// pass of *Best* beside the stems, a stem brought back — and a disk without it is said,
/// not found out half way (review 2 #7).
#[test]
fn a_separation_is_refused_when_the_disk_lacks_room() {
    let (_one, data) = data_folder();
    cantus_ui::i18n::set_thread_locale(cantus_ui::i18n::Locale::En);
    let x = tone(48_000, 10.0);
    let f = data.path().join("room.wav");
    write(&f, 48_000, &x);
    let (p, tid, _) = project(&[(f, 48_000, x.0.len() as u64)]);
    let mut app = app_with(p, data.path());
    let clips = Target::Track(tid).clips(app.project());
    let sources = run::sources_for(&mut app, &clips, &fixture_model()).unwrap();
    let scratch = data.path().join("scratch");
    let need: u64 = run::space_needed(&sources, &scratch, 4, 4)
        .iter()
        .map(|n| n.1)
        .sum();
    let n44 = cantus_engine::samples::resample::converted_len(x.0.len(), 48_000, 44_100) as u64;
    assert!(need >= 4 * 5 * n44 * 8 + n44 * 8, "{need}");
    let fast: u64 = run::space_needed(&sources, &scratch, 4, 0)
        .iter()
        .map(|n| n.1)
        .sum();
    assert!(fast < need);
    let spare = 64 << 20;
    let one = |free: u64| move |_: &std::path::Path| Some(("c:\\".to_string(), free));
    assert!(run::check_space_with(&sources, &scratch, 4, 4, &one(need + spare)).is_ok());
    let e = run::check_space_with(&sources, &scratch, 4, 4, &one(need)).unwrap_err();
    assert!(e.contains("Not enough free space"), "{e}");
}

/// Folders are counted together only when they are on one volume, known by the volume
/// itself: two disks that happen to have the same free bytes are each checked for their
/// own part (review 3 #2).
#[test]
fn disks_are_told_apart_by_their_volume() {
    let (_one, data) = data_folder();
    cantus_ui::i18n::set_thread_locale(cantus_ui::i18n::Locale::En);
    let x = tone(48_000, 10.0);
    let f = data.path().join("vol.wav");
    write(&f, 48_000, &x);
    let (p, tid, _) = project(&[(f, 48_000, x.0.len() as u64)]);
    let mut app = app_with(p, data.path());
    let clips = Target::Track(tid).clips(app.project());
    let sources = run::sources_for(&mut app, &clips, &fixture_model()).unwrap();
    let scratch = data.path().join("scratch");
    let needs = run::space_needed(&sources, &scratch, 4, 0);
    let (input, stems) = (needs[0].1, needs[1].1);
    let spare = 64 << 20;
    // Enough for the larger part alone, not for both together.
    let free = input.max(stems) + spare;
    let scratch_too = scratch.clone();
    let two = move |p: &std::path::Path| {
        let v = if p == scratch_too { "d:\\" } else { "e:\\" };
        Some((v.to_string(), free))
    };
    assert!(run::check_space_with(&sources, &scratch, 4, 0, &two).is_ok());
    let same = move |_: &std::path::Path| Some(("e:\\".to_string(), free));
    assert!(run::check_space_with(&sources, &scratch, 4, 0, &same).is_err());
    // This machine's own folders: one volume, named.
    let here = cantus_rt::disk::volume(data.path()).unwrap();
    assert_eq!(cantus_rt::disk::volume(&scratch), Some(here));
}

/// The folders under `Stems/` of the app's audio folder.
fn stems_folders(app: &mut CantusApp) -> Vec<std::path::PathBuf> {
    let root = app.audio_record_dir().join("Stems");
    std::fs::read_dir(root)
        .map(|d| d.flatten().map(|e| e.path()).collect())
        .unwrap_or_default()
}

/// A refusal — a file missing, a file too long — leaves no folder behind, even for the
/// files checked before it (review 3 #4).
#[test]
fn a_refused_separation_leaves_no_folder() {
    let (_one, data) = data_folder();
    cantus_ui::i18n::set_thread_locale(cantus_ui::i18n::Locale::En);
    let x = tone(44_100, 1.0);
    let (a, b) = (data.path().join("a.wav"), data.path().join("b.wav"));
    write(&a, 44_100, &x);
    write(&b, 44_100, &x);
    let n = x.0.len() as u64;
    let (p, tid, _) = project(&[(a, 44_100, n), (b.clone(), 44_100, n)]);
    let mut app = app_with(p, data.path());
    std::fs::remove_file(&b).unwrap();
    let e = run::start(&mut app, Target::Track(tid), fixture_model()).unwrap_err();
    assert!(e.contains("b.wav"), "{e}");
    assert!(
        stems_folders(&mut app).is_empty(),
        "{:?}",
        stems_folders(&mut app)
    );
    write(&b, 44_100, &x);
    let mut p = app.project().clone();
    p.audio_pool[1].sample_rate = 192_000;
    p.audio_pool[1].frames = 3_600 * 192_000;
    let mut app = app_with(p, data.path());
    assert!(run::start(&mut app, Target::Track(tid), fixture_model()).is_err());
    assert!(
        stems_folders(&mut app).is_empty(),
        "{:?}",
        stems_folders(&mut app)
    );
}

/// An agent's separation keeps the helper in use for its whole run: between one source's
/// helper job and the next (while the second converts), *Remove* is still refused
/// (review 3 #1).
#[test]
fn remove_is_refused_between_an_agents_helper_jobs() {
    use cantus_ai::tool::JobCx;
    use cantus_ui::stems_ui::models;
    use std::sync::atomic::{AtomicBool, Ordering};
    let (_one, data) = data_folder();
    cantus_ui::i18n::set_thread_locale(cantus_ui::i18n::Locale::En);
    let x = tone(48_000, 3.0);
    let (a, b) = (data.path().join("one.wav"), data.path().join("two.wav"));
    write(&a, 48_000, &x);
    write(&b, 48_000, &x);
    let n = x.0.len() as u64;
    let (p, tid, _) = project(&[(a, 48_000, n), (b, 48_000, n)]);
    let mut app = app_with(p, data.path());
    let job = tool_job(&mut app, &serde_json::json!({"track_id": tid.0})).unwrap();
    let (first_done, in_gap, release) = (
        AtomicBool::new(false),
        AtomicBool::new(false),
        AtomicBool::new(false),
    );
    std::thread::scope(|sc| {
        let worker = sc.spawn(|| {
            let progress = |d: f32| {
                if d >= 0.5 - 1e-6 {
                    first_done.store(true, Ordering::SeqCst);
                }
            };
            // The second source's conversion asks first: held there, in the gap.
            let cancelled = || {
                if first_done.load(Ordering::SeqCst) && !in_gap.swap(true, Ordering::SeqCst) {
                    while !release.load(Ordering::SeqCst) {
                        std::thread::sleep(std::time::Duration::from_millis(2));
                    }
                }
                false
            };
            (job.work)(&JobCx {
                progress: &progress,
                cancelled: &cancelled,
            })
            .is_ok()
        });
        let t = std::time::Instant::now();
        while !in_gap.load(Ordering::SeqCst) {
            assert!(t.elapsed() < std::time::Duration::from_secs(60));
            std::thread::sleep(std::time::Duration::from_millis(2));
        }
        assert!(!app.ml.busy(), "no helper job runs in the gap");
        let e = models::request_remove(&mut app, "htdemucs").unwrap_err();
        assert!(e.contains("In use"), "{e}");
        release.store(true, Ordering::SeqCst);
        assert!(worker.join().unwrap());
    });
    assert!(models::request_remove(&mut app, "htdemucs").is_ok());
}

/// The tool tells an agent what it refuses and why the new strips are not at 0 dB
/// (review 3 #6).
#[test]
fn the_tool_says_one_track_arrangement_clips_and_the_trim() {
    let registry = cantus_ui::ai_tools::registry();
    let tool = registry
        .iter()
        .find(|t| t.name == "separate_stems")
        .unwrap();
    let d = tool.description;
    for want in [
        "one track",
        "arrangement",
        "session clips",
        "above 0 dB",
        "pan law",
    ] {
        assert!(d.contains(want), "the description lacks {want:?}");
    }
    let schema = (tool.input_schema)().to_string();
    assert!(schema.contains("one track") && schema.contains("not session clips"));
    let result = (tool.output_schema.unwrap())().to_string();
    assert!(result.contains("above 0 dB"));
}

/// The pan law changed after a separation trims the stems' strips with it, in the same
/// undo step: the parts still sum to the original under the new law, and undo puts both
/// back exactly (review 3 #3).
#[test]
fn changing_the_pan_law_keeps_the_stems_at_the_originals_level() {
    let (_one, data) = data_folder();
    let x = tone(48_000, 4.0);
    let f = data.path().join("law.wav");
    write(&f, 48_000, &x);
    let (p, tid, _) = project(&[(f, 48_000, x.0.len() as u64)]);
    let mut app = app_with(p, data.path());
    assert!(app.exec(Command::SetPanLaw {
        law: PanLaw::Minus3dB
    }));
    run::start(&mut app, Target::Track(tid), fixture_model()).unwrap();
    until_done(&mut app);
    let separated = app.project().clone();
    assert_eq!(cantus_ui::stems_ui::stems_strips(&separated).len(), 5);
    for law in [PanLaw::Minus6dB, PanLaw::ZeroDb, PanLaw::Minus4_5dB] {
        let cmd = cantus_ui::stems_ui::pan_law_command(app.project(), law);
        assert!(app.exec(cmd));
        assert_eq!(app.project().pan_law, law);
        let after = render(&mut app);
        // The original alone, under the same law: its clips unmuted, the parts muted.
        let mut solo = app.project().clone();
        for t in solo.tracks.iter_mut() {
            let orig = t.id == tid;
            for c in t.clips.iter_mut() {
                c.muted = !orig;
            }
        }
        let mut alone = app_with(solo, data.path());
        let before = render(&mut alone);
        let (level, snr) = compare(&before, &after);
        assert!(level.abs() < 0.05, "{law:?}: {level:.2} dB");
        assert!(snr > 40.0, "{law:?}: {snr:.1} dB");
    }
    // Undo, step by step, back to the separation as it was.
    for _ in 0..3 {
        app.undo();
    }
    assert_eq!(app.project(), &separated);
}

/// A model whose removal is waiting for the helper is not ready to start — not from the
/// panel, not for an agent — and a removal is not carried out under a job that took the
/// helper meanwhile (integration review #3).
#[test]
fn a_model_being_removed_cannot_be_started() {
    use cantus_ui::stems_ui::models;
    let (_one, data) = data_folder();
    cantus_ui::i18n::set_thread_locale(cantus_ui::i18n::Locale::En);
    let x = tone(44_100, 30.0);
    let f = data.path().join("going.wav");
    write(&f, 44_100, &x);
    let (p, tid, _) = project(&[(f, 44_100, x.0.len() as u64)]);
    let mut app = app_with(p, data.path());
    app.stems.removing = Some(("htdemucs".into(), std::time::Instant::now()));
    assert!(models::removing(&app, "htdemucs"));
    assert!(models::ready(&app, "htdemucs").is_none());
    let registry = cantus_ui::ai_tools::registry();
    let tool = registry
        .iter()
        .find(|t| t.name == "separate_stems")
        .unwrap();
    let cantus_ai::tool::ToolRun::Job(prepare) = tool.run else {
        panic!("a job");
    };
    let args = serde_json::json!({"track_id": tid.0});
    let refused = prepare(&mut cantus_ai::tool::ToolCx { host: &mut app }, &args);
    assert!(
        matches!(refused, Err(cantus_ai::tool::ToolError::Refused(ref m)) if m.contains("being removed")),
        "{:?}",
        refused.as_ref().err()
    );
    // A separation took the helper: the removal is dropped, not done under it.
    app.stems.removing = None;
    run::start(&mut app, Target::Track(tid), fixture_model()).unwrap();
    app.stems.removing = Some(("htdemucs".into(), std::time::Instant::now()));
    run::step(&mut app);
    assert!(
        app.stems.removing.is_none(),
        "the removal waited on under a job"
    );
    until_done(&mut app);
    assert_eq!(app.stems.runs[0].phase, run::Phase::Done);
}

/// The stems' folders under the app's audio folder, and the WAVs in them.
fn stems_files(app: &mut CantusApp) -> (usize, usize) {
    let dirs = stems_folders(app);
    let wavs = dirs
        .iter()
        .flat_map(|d| std::fs::read_dir(d).into_iter().flatten().flatten())
        .count();
    (dirs.len(), wavs)
}

/// An agent's separation leaves nothing when its edit is not made: a job denied before
/// it runs makes no folder; a job whose planned edit is refused (another project, a
/// denied or timed-out card, a plan the host turns down) has its folders and WAVs
/// removed; an edit made keeps them (integration review #2).
#[test]
fn an_agents_stems_go_when_its_edit_is_not_made() {
    use cantus_ai::tool::{JobCx, JobDone, ToolCx};
    let (_one, data) = data_folder();
    let x = tone(44_100, 2.0);
    let f = data.path().join("agent.wav");
    write(&f, 44_100, &x);
    let (p, tid, _) = project(&[(f, 44_100, x.0.len() as u64)]);
    let mut app = app_with(p, data.path());
    let args = serde_json::json!({"track_id": tid.0});
    // Denied: prepared, never run.
    let job = tool_job(&mut app, &args).unwrap();
    drop(job);
    assert_eq!(stems_files(&mut app), (0, 0), "a denied job made folders");
    assert_eq!(app.ml.ml_agents(), 0);
    for made in [false, true] {
        let job = tool_job(&mut app, &args).unwrap();
        assert_eq!(app.ml.ml_agents(), 1);
        let done = (job.work)(&JobCx {
            progress: &|_: f32| {},
            cancelled: &|| false,
        })
        .unwrap();
        let JobDone::Then(_, then) = done else {
            panic!("an edit");
        };
        assert_eq!(stems_files(&mut app).0, 1);
        let plan = then(&mut ToolCx { host: &mut app }).unwrap();
        assert_eq!(app.ml.ml_agents(), 0, "held until the job's edit");
        if made {
            assert!(app.exec(plan.command));
        } else {
            drop(plan);
        }
        run::step(&mut app);
        let (dirs, wavs) = stems_files(&mut app);
        if made {
            assert_eq!((dirs, wavs), (1, 4), "the made edit's files");
        } else {
            assert_eq!((dirs, wavs), (0, 0), "a refused edit's files stayed");
        }
        assert!(app.stems.pending.is_empty());
    }
}

/// The agent's edit lays its files out from the project's audio folder as it is when the
/// edit is planned: a Save As during the job does not break the stems' paths
/// (integration review #4).
#[test]
fn an_agents_stems_follow_a_save_as_during_the_job() {
    use cantus_ai::tool::{JobCx, JobDone, ToolCx};
    let (_one, data) = data_folder();
    let x = tone(44_100, 2.0);
    let f = data.path().join("saved.wav");
    write(&f, 44_100, &x);
    let (p, tid, _) = project(&[(f, 44_100, x.0.len() as u64)]);
    let mut app = app_with(p, data.path());
    let first = data.path().join("first");
    std::fs::create_dir_all(&first).unwrap();
    app.save_to_path(&first.join("song.cantus"));
    let job = tool_job(&mut app, &serde_json::json!({"track_id": tid.0})).unwrap();
    let done = (job.work)(&JobCx {
        progress: &|_: f32| {},
        cancelled: &|| false,
    })
    .unwrap();
    let JobDone::Then(_, then) = done else {
        panic!("an edit");
    };
    let second = data.path().join("second");
    std::fs::create_dir_all(&second).unwrap();
    app.save_to_path(&second.join("song.cantus"));
    let plan = then(&mut ToolCx { host: &mut app }).unwrap();
    assert!(app.exec(plan.command));
    let pool = app.project().audio_pool.clone();
    let stems: Vec<_> = pool
        .iter()
        .filter(|s| app.project().provenance.contains_key(&s.id))
        .collect();
    assert_eq!(stems.len(), 4);
    for s in stems {
        let file = app.audio_file(s);
        assert!(file.is_file(), "{} → {}", s.path, file.display());
    }
}

/// Runs planned on the same source at once never share a folder: each makes its own, the
/// next free name when another took it first — so one run's clean-up can never take the
/// other's stems (final review #1).
#[test]
fn runs_at_once_never_share_a_stems_folder() {
    let (_one, data) = data_folder();
    let x = tone(44_100, 1.0);
    let f = data.path().join("same.wav");
    write(&f, 44_100, &x);
    let (p, tid, _) = project(&[(f, 44_100, x.0.len() as u64)]);
    let mut app = app_with(p, data.path());
    let clips = Target::Track(tid).clips(app.project());
    // Eight runs planned before any makes its folder: the same name chosen by all.
    let planned: Vec<Vec<run::Source>> = (0..8)
        .map(|_| run::plan_sources(&mut app, &clips, &fixture_model()).unwrap())
        .collect();
    assert!(planned.iter().all(|s| s[0].out == planned[0][0].out));
    let start = std::sync::Barrier::new(planned.len());
    let made: Vec<std::path::PathBuf> = std::thread::scope(|sc| {
        let hs: Vec<_> = planned
            .into_iter()
            .map(|mut s| {
                let start = &start;
                sc.spawn(move || {
                    start.wait();
                    run::make_dirs(&mut s).unwrap();
                    s[0].out.clone()
                })
            })
            .collect();
        hs.into_iter().map(|h| h.join().unwrap()).collect()
    });
    let mut distinct = made.clone();
    distinct.sort();
    distinct.dedup();
    assert_eq!(distinct.len(), made.len(), "{made:?}");
    assert!(made.iter().all(|d| d.is_dir()));
}

/// Cantus closing settles every pending edit at once: a refused edit's folders go, a made
/// one's stay (final review #2).
#[test]
fn closing_settles_the_pending_edits() {
    use cantus_ai::tool::{JobCx, JobDone, ToolCx};
    let (_one, data) = data_folder();
    let x = tone(44_100, 1.0);
    let f = data.path().join("closing.wav");
    write(&f, 44_100, &x);
    let (p, tid, _) = project(&[(f, 44_100, x.0.len() as u64)]);
    let mut app = app_with(p, data.path());
    let args = serde_json::json!({"track_id": tid.0});
    let mut folders = Vec::new();
    for made in [true, false] {
        let job = tool_job(&mut app, &args).unwrap();
        let done = (job.work)(&JobCx {
            progress: &|_: f32| {},
            cancelled: &|| false,
        })
        .unwrap();
        let JobDone::Then(_, then) = done else {
            panic!("an edit");
        };
        let plan = then(&mut ToolCx { host: &mut app }).unwrap();
        folders.push(app.stems.pending.last().unwrap().dirs[0].clone());
        if made {
            assert!(app.exec(plan.command));
        }
    }
    // No frame settled them: Cantus closes.
    assert_eq!(app.stems.pending.len(), 2);
    app.close_session();
    assert!(app.stems.pending.is_empty());
    assert!(folders[0].is_dir(), "the made edit's stems went");
    assert!(!folders[1].exists(), "the refused edit's stems stayed");
}

/// A project opened sweeps the `Audio/Stems/` folders a run left behind — named as Cantus
/// names them, played by nothing, all older than the last save — and nothing else: not a
/// folder the project plays, not one newer than the save, not one named otherwise
/// (final review #2).
#[test]
fn opening_a_project_sweeps_the_stems_a_closed_run_left() {
    let (_one, data) = data_folder();
    let x = tone(44_100, 1.0);
    let f = data.path().join("open.wav");
    write(&f, 44_100, &x);
    let (p, _tid, _) = project(&[(f, 44_100, x.0.len() as u64)]);
    let mut app = app_with(p, data.path());
    let dir = data.path().join("proj");
    std::fs::create_dir_all(&dir).unwrap();
    let stems = dir.join("Audio").join("Stems");
    let folder = |name: &str| {
        let d = stems.join(name);
        std::fs::create_dir_all(&d).unwrap();
        write(&d.join("drums.wav"), 44_100, &tone(44_100, 0.1));
        d
    };
    let orphan = folder("Old.wav _ htdemucs");
    let orphan2 = folder("Old.wav _ htdemucs 2");
    let mine = folder("Mine");
    let kept = folder("Kept.wav _ htdemucs");
    // The project plays the kept folder's file.
    let id = cantus_core::AudioSourceId(app.bus.alloc_id());
    assert!(app.exec(Command::InsertAudioSource {
        source: cantus_core::AudioSource {
            id,
            name: "Kept drums".into(),
            path: kept.join("drums.wav").display().to_string(),
            sample_rate: 44_100,
            channels: 2,
            frames: 4_410,
            sha256: None,
            recorded: false,
        },
    }));
    std::thread::sleep(std::time::Duration::from_millis(1_100));
    let file = dir.join("song.cantus");
    app.save_to_path(&file);
    // Made after the save: a run of this session's, not a leftover.
    let fresh = folder("Fresh.wav _ htdemucs");
    let later = std::time::SystemTime::now() + std::time::Duration::from_secs(3_600);
    std::fs::File::options()
        .write(true)
        .open(fresh.join("drums.wav"))
        .unwrap()
        .set_modified(later)
        .unwrap();
    let mut other = app_with(cantus_core::Project::new("empty"), data.path());
    other.open(&file);
    assert_eq!(other.path.as_deref(), Some(file.as_path()));
    assert!(
        !orphan.exists() && !orphan2.exists(),
        "the leftovers stayed"
    );
    assert!(mine.is_dir(), "a folder named otherwise went");
    assert!(kept.is_dir(), "a folder the project plays went");
    assert!(fresh.is_dir(), "a folder newer than the save went");
    assert!(run::is_stems_folder(
        "Song.wav _ htdemucs-6s 3",
        &["htdemucs-6s".into()]
    ));
    assert!(!run::is_stems_folder("htdemucs", &["htdemucs".into()]));
}
