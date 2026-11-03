//! The recording mode in the interface (round 36, ADR 0092): *Prepare to record* renders
//! every strip that feeds the master through the freezes and puts the lock on, the mix is
//! the live one to the bit, an armed track stays live, neither entering nor leaving is an undo step
//! and keeps (or discards) the renders, a file is never written out of the mode's state,
//! and an agent reads and drives it.

mod support;

use cantus_ai::{ToolCx, ToolRun};
use cantus_core::freeze::FreezeTarget;
use cantus_core::*;
use cantus_engine::{render, RenderOptions};
use cantus_ui::record_mode::{Options, Phase};
use cantus_ui::CantusApp;
use serde_json::json;
use std::time::{Duration, Instant};

fn song() -> Project {
    let mut p = cantus_ui::starter_project();
    p.tracks.clear();
    for (name, notes) in [("Synth", [60u8, 64, 67]), ("Bass", [36, 43, 36])] {
        let mut t = p.make_track(name);
        let mut c = p.make_clip(name, Ticks::ZERO, Ticks::beats(8));
        for (i, k) in notes.iter().enumerate() {
            c.notes
                .push(p.make_note(Ticks::beats(2 * i as i64), Ticks::beats(2), *k, 100));
        }
        t.clips.push(c);
        p.tracks.push(t);
    }
    let synth_strip = p.tracks[0].strip.unwrap();
    let delay = p.make_effect(EffectKind::Delay);
    p.strip_mut(synth_strip).unwrap().effects.push(delay);
    p
}

fn app(name: &str) -> (CantusApp, std::path::PathBuf) {
    let dir = std::env::temp_dir()
        .join("cantus-record-mode-ui")
        .join(format!("{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let mut app = support::app(song());
    app.engine.frozen.set_app_dir(dir.join("renders"));
    app.path = Some(dir.join("song.cantus"));
    (app, dir)
}

/// Frames until the preparation is done (or a while has passed).
fn settle(app: &mut CantusApp, ctx: &egui::Context) {
    let t0 = Instant::now();
    loop {
        app.sync_engine();
        cantus_ui::freeze::tick(app, ctx);
        if !app.freeze.record.is_preparing() || t0.elapsed() > Duration::from_secs(120) {
            break;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
}

fn mix(app: &mut CantusApp) -> Vec<f32> {
    app.sync_engine();
    let project = app.project().clone();
    let snap = app.engine.compile_for_render(&project, 1);
    render(
        snap,
        &RenderOptions {
            tail_seconds: 1.0,
            compensate_latency: true,
            ..Default::default()
        },
    )
    .unwrap()
    .samples
}

fn same_bits(a: &[f32], b: &[f32], what: &str) {
    assert_eq!(a.len(), b.len(), "{what}: lengths");
    if let Some(i) = a
        .iter()
        .zip(b)
        .position(|(x, y)| x.to_bits() != y.to_bits())
    {
        panic!("{what}: sample {i}: {} vs {}", a[i], b[i]);
    }
    assert!(a.iter().any(|x| *x != 0.0), "{what}: silent");
}

fn enter(app: &mut CantusApp, ctx: &egui::Context, options: Options) {
    app.record_mode_ask();
    assert!(app.freeze.record.ask.is_some(), "the question comes first");
    app.record_mode_begin(options).expect("begins");
    assert!(app.freeze.record.is_preparing());
    settle(app, ctx);
    assert!(
        app.freeze.record.is_active(),
        "{:?}",
        app.freeze.record.phase
    );
}

#[test]
fn the_question_shows_the_plan_and_its_size_before_anything_is_rendered() {
    let (mut app, dir) = app("ask");
    app.record_mode_ask();
    let (plan, est) = app.freeze.record.ask.clone().expect("asked");
    assert_eq!(plan.bake.len(), 2, "the synth's and the bass's strips");
    assert_eq!(est.targets, 2);
    assert!(est.bytes_new > 0 && est.bytes_all == est.bytes_new);
    assert!(!app.freeze.busy(), "nothing rendered yet");
    assert!(!app.project().any_frozen());
    assert!(
        !dir.join("renders").exists()
            || std::fs::read_dir(dir.join("renders"))
                .unwrap()
                .next()
                .is_none()
    );
}

#[test]
fn prepare_bakes_every_strip_the_lock_comes_on_and_the_mix_is_the_live_one() {
    let ctx = support::ctx();
    let (mut app, _dir) = app("enter");
    let live = mix(&mut app);
    enter(&mut app, &ctx, Options::default());
    let p = app.project();
    for t in &p.tracks {
        assert!(
            p.frozen(FreezeTarget::Strip(t.strip.unwrap())).is_some(),
            "{} is baked",
            t.name
        );
    }
    assert!(app.bus.record_lock().is_some());
    same_bits(&mix(&mut app), &live, "in the mode");
    // What changes the sound is refused, said as the mode says it.
    assert!(!app.exec(Command::SetTempo { bpm: 99.0 }));
    assert!(app.status.error && app.status.text.contains("Recording mode"));
    assert!(app.freeze.refusal.is_none(), "no Unfreeze card in the mode");
    let synth = app.project().tracks[0].id;
    let clip = app.project().tracks[0].clips[0].id;
    assert!(!app.exec(Command::SetClipMute {
        track: synth,
        clip,
        muted: true
    }));
    let strip = app.project().tracks[0].strip.unwrap();
    assert!(!app.exec(freeze_off(strip)));
    assert!(!app.unfreeze_target_ok(strip));
    // Undo cannot unbake it either.
    app.undo();
    assert!(app.project().frozen(FreezeTarget::Strip(strip)).is_some());
    // The mix controls are live.
    assert!(app.exec(Command::SetStripGain {
        id: strip,
        gain_db: -3.0
    }));
    // The bar says what it is.
    let words = cantus_ui::views::record_bar::bar_text(&app);
    assert!(words.starts_with('2') && words.contains("baked"), "{words}");
}

fn freeze_off(strip: StripId) -> Command {
    cantus_core::freeze::unfreeze_command(FreezeTarget::Strip(strip))
}

trait UnfreezeOk {
    fn unfreeze_target_ok(&mut self, strip: StripId) -> bool;
}

impl UnfreezeOk for CantusApp {
    /// The menu's *Unfreeze* in the mode: refused, still frozen.
    fn unfreeze_target_ok(&mut self, strip: StripId) -> bool {
        self.unfreeze_target(FreezeTarget::Strip(strip));
        self.project().frozen(FreezeTarget::Strip(strip)).is_none()
    }
}

#[test]
fn locked_mix_controls_are_refused_too() {
    let ctx = support::ctx();
    let (mut app, _dir) = app("locked");
    enter(
        &mut app,
        &ctx,
        Options {
            mix_live: false,
            ..Options::default()
        },
    );
    let strip = app.project().tracks[0].strip.unwrap();
    assert!(!app.exec(Command::SetStripGain {
        id: strip,
        gain_db: -3.0
    }));
    // The master is live in both.
    assert!(app.exec(Command::SetMasterGain { gain_db: -2.0 }));
}

#[test]
fn an_armed_track_stays_live_and_takes_edits() {
    let ctx = support::ctx();
    let (mut app, _dir) = app("armed");
    let bass = app.project().tracks[1].id;
    assert!(app.exec(Command::SetTrackArmed {
        id: bass,
        armed: true
    }));
    let live = mix(&mut app);
    enter(&mut app, &ctx, Options::default());
    let bass_strip = app.project().tracks[1].strip.unwrap();
    let synth_strip = app.project().tracks[0].strip.unwrap();
    assert!(app
        .project()
        .frozen(FreezeTarget::Strip(bass_strip))
        .is_none());
    assert!(app
        .project()
        .frozen(FreezeTarget::Strip(synth_strip))
        .is_some());
    let a = app.freeze.record.active().unwrap();
    assert_eq!(a.live.len(), 1);
    same_bits(&mix(&mut app), &live, "armed live, rest baked");
    // The take lands on the live track as an edit does.
    let clip = app.project().tracks[1].clips[0].id;
    assert!(app.exec(Command::SetClipMute {
        track: bass,
        clip,
        muted: false
    }));
}

#[test]
fn leaving_is_no_undo_step_keeps_the_renders_and_entering_again_renders_nothing() {
    let ctx = support::ctx();
    let (mut app, dir) = app("leave");
    let before = app.project().clone();
    enter(&mut app, &ctx, Options::default());
    let steps_in = app.bus.history().len();
    assert_eq!(steps_in, 0, "entering is no undo step");
    assert!(app.record_mode_leave(false).is_ok());
    assert!(app.bus.record_lock().is_none());
    assert!(!app.project().any_frozen());
    assert_eq!(app.bus.history().len(), steps_in, "leaving is no undo step");
    assert!(
        !app.bus.is_dirty(),
        "nothing to save: the mode is the session's"
    );
    // The renders stay.
    let cfrz = |d: &std::path::Path| {
        fn walk(d: &std::path::Path, n: &mut usize) {
            for e in std::fs::read_dir(d).into_iter().flatten().flatten() {
                let p = e.path();
                if p.is_dir() {
                    walk(&p, n);
                } else if p.extension().is_some_and(|x| x == "cfrz") {
                    *n += 1;
                }
            }
        }
        let mut n = 0;
        walk(d, &mut n);
        n
    };
    assert_eq!(cfrz(&dir), 2);
    // Entering again: no render runs, nothing new on disk.
    app.record_mode_ask();
    let (_, est) = app.freeze.record.ask.clone().unwrap();
    assert_eq!(est.reused, 2, "found by their keys");
    assert_eq!(est.bytes_new, 0);
    app.record_mode_begin(Options::default()).unwrap();
    settle(&mut app, &ctx);
    assert!(app.freeze.record.is_active());
    assert_eq!(cfrz(&dir), 2);
    // Leaving and discarding frees the disk.
    assert!(app.record_mode_leave(true).is_ok());
    assert_eq!(cfrz(&dir), 0);
    let _ = before;
}

#[test]
fn a_file_is_never_written_out_of_the_mode_so_a_crash_reopens_out_of_it() {
    let ctx = support::ctx();
    let (mut app, _dir) = app("crash");
    enter(&mut app, &ctx, Options::default());
    assert!(app.project().any_frozen());
    let copy = app.autosave_copy();
    assert!(!copy.any_frozen(), "the autosave holds the live project");
    assert_eq!(copy.tracks.len(), app.project().tracks.len());
    app.save();
    let path = app.path.clone().unwrap();
    let t0 = Instant::now();
    while app.save_in_flight() && t0.elapsed() < Duration::from_secs(30) {
        app.poll_save(true);
        std::thread::sleep(Duration::from_millis(5));
    }
    let loaded = cantus_project::load(&path, false).expect("loads").project;
    assert!(!loaded.any_frozen(), "the file holds no mode freezes");
}

#[test]
fn another_project_ends_the_mode() {
    let ctx = support::ctx();
    let (mut app, _dir) = app("epoch");
    enter(&mut app, &ctx, Options::default());
    app.replace_project(song());
    assert!(
        app.bus.record_lock().is_none(),
        "the new project is never locked"
    );
    cantus_ui::freeze::tick(&mut app, &ctx);
    assert!(matches!(app.freeze.record.phase, Phase::Idle));
    assert!(app.bus.record_lock().is_none());
}

#[test]
fn it_will_not_start_while_recording_or_when_there_is_nothing_to_do() {
    let (mut app, _dir) = app("refuse");
    // Every strip frozen already: nothing to prepare.
    let ctx = support::ctx();
    enter(&mut app, &ctx, Options::default());
    assert!(
        app.record_mode_begin(Options::default()).is_err(),
        "on already"
    );
    assert!(app.record_mode_leave(false).is_ok());
    assert!(
        app.record_mode_leave(false).is_err(),
        "leaving what is not on says so"
    );
}

fn run_read(app: &mut CantusApp, name: &str) -> serde_json::Value {
    let tool = cantus_ui::ai_tools::find(name).expect("registered");
    let ToolRun::Read(f) = tool.run else {
        panic!("{name} is a read")
    };
    f(&ToolCx { host: app }, &json!({})).expect("answers")
}

fn run_host(
    app: &mut CantusApp,
    name: &str,
    args: serde_json::Value,
) -> Result<serde_json::Value, String> {
    let tool = cantus_ui::ai_tools::find(name).expect("registered");
    let ToolRun::Host(f) = tool.run else {
        panic!("{name} is a host tool")
    };
    f(&mut ToolCx { host: app }, &args).map_err(|e| e.to_string())
}

#[test]
fn an_agent_prepares_reads_and_leaves() {
    let ctx = support::ctx();
    let (mut app, _dir) = app("agent");
    assert_eq!(run_read(&mut app, "get_recording_mode")["state"], "off");
    let dry = run_host(&mut app, "prepare_to_record", json!({"dry_run": true})).unwrap();
    assert_eq!(dry["bake"].as_array().unwrap().len(), 2);
    assert!(dry["disk_new_bytes_at_most"].as_u64().unwrap() > 0);
    assert!(!app.freeze.record.on(), "a dry run changes nothing");
    let started = run_host(
        &mut app,
        "prepare_to_record",
        json!({"mix_controls": "locked"}),
    )
    .unwrap();
    assert_eq!(started["started"], true);
    assert_eq!(
        run_read(&mut app, "get_recording_mode")["state"],
        "preparing"
    );
    settle(&mut app, &ctx);
    let on = run_read(&mut app, "get_recording_mode");
    assert_eq!(on["state"], "on");
    assert_eq!(on["baked"].as_array().unwrap().len(), 2);
    assert_eq!(on["mix_controls"], "locked");
    assert!(run_host(&mut app, "prepare_to_record", json!({})).is_err());
    let left = run_host(&mut app, "leave_recording_mode", json!({})).unwrap();
    assert_eq!(left["left"], true);
    assert_eq!(run_read(&mut app, "get_recording_mode")["state"], "off");
    assert!(run_host(&mut app, "leave_recording_mode", json!({})).is_err());
}

#[test]
fn every_string_is_in_both_languages() {
    let en = include_str!("../locales/areas/record.en.txt");
    let es = include_str!("../locales/areas/record.es.txt");
    let keys = |s: &str| -> Vec<String> {
        s.lines()
            .filter(|l| !l.trim().is_empty() && !l.starts_with('#'))
            .filter_map(|l| l.split_once(" = ").map(|(k, _)| k.to_string()))
            .collect()
    };
    let (en, es) = (keys(en), keys(es));
    assert!(en.len() > 40);
    for k in &en {
        assert!(es.contains(k), "{k} has no Spanish");
    }
    assert_eq!(en.len(), es.len());
}

#[test]
fn an_undo_after_leaving_never_freezes_anything_again() {
    let ctx = support::ctx();
    let (mut app, _dir) = app("undo_after");
    let bass = app.project().tracks[1].id;
    assert!(app.exec(Command::SetTrackGain {
        id: bass,
        gain_db: -2.0
    }));
    enter(&mut app, &ctx, Options::default());
    assert!(app.record_mode_leave(false).is_ok());
    app.undo();
    assert!(
        !app.project().any_frozen(),
        "the undo was the gain's, nothing frozen"
    );
    assert_eq!(app.project().track(bass).unwrap().gain_db, 0.0);
}

fn with_master_chain(app: &mut CantusApp) {
    let mut p = app.project().clone();
    let mut lim = p.make_effect(EffectKind::Limiter);
    lim.spec.set("look", 5.0).unwrap();
    assert!(app.exec(Command::InsertEffect {
        strip: StripId::MASTER,
        index: 0,
        slot: Box::new(lim),
    }));
    assert!(app.exec(Command::SetMasterGain { gain_db: -1.5 }));
}

#[test]
fn a_baked_master_plays_the_live_mix_to_the_bit_and_locks_the_master() {
    let ctx = support::ctx();
    let (mut app, _dir) = app("master");
    with_master_chain(&mut app);
    let live = mix(&mut app);
    app.record_mode_ask();
    let opts = Options {
        master_live: false,
        ..Options::default()
    };
    let (plan, _) = app.freeze.record.ask.clone().unwrap();
    let est = cantus_ui::record_mode::estimate(&app, &plan, opts);
    enter(&mut app, &ctx, opts);
    assert!(
        app.freeze.record.master_baked(),
        "{:?}",
        app.freeze.record.phase
    );
    assert!(app.engine.frozen.master().is_some());
    same_bits(&mix(&mut app), &live, "the baked master");
    // Nothing of the master, nor the mix, moves.
    assert!(!app.exec(Command::SetMasterGain { gain_db: -3.0 }));
    let strip = app.project().tracks[0].strip.unwrap();
    assert!(!app.exec(Command::SetStripGain {
        id: strip,
        gain_db: -3.0
    }));
    // The disk said up front is never less than what was written.
    let a = app.freeze.record.active().unwrap();
    assert!(
        a.bytes > 0 && a.bytes <= est.bytes_all,
        "{} <= {}",
        a.bytes,
        est.bytes_all
    );
    assert!(app.record_mode_leave(true).is_ok());
    assert!(app.engine.frozen.master().is_none());
    same_bits(&mix(&mut app), &live, "left");
}

#[test]
fn arming_a_baked_track_offers_its_strip_live_and_then_arms_it() {
    let ctx = support::ctx();
    let (mut app, _dir) = app("arm");
    let live = mix(&mut app);
    enter(&mut app, &ctx, Options::default());
    let bass = app.project().tracks[1].id;
    let bass_strip = app.project().tracks[1].strip.unwrap();
    // The arm is refused, and offered.
    assert!(!app.exec(Command::SetTrackArmed {
        id: bass,
        armed: true
    }));
    assert_eq!(app.freeze.record.offer, Some(bass));
    let baked_before = app.freeze.record.active().unwrap().baked.len();
    app.record_mode_bring_live(bass).expect("brought live");
    assert!(app.project().track(bass).unwrap().armed);
    assert!(app
        .project()
        .frozen(FreezeTarget::Strip(bass_strip))
        .is_none());
    let a = app.freeze.record.active().unwrap();
    assert_eq!(a.baked.len(), baked_before - 1, "the bar's counts follow");
    assert!(a
        .live
        .iter()
        .any(|(t, _)| *t == FreezeTarget::Strip(bass_strip)));
    assert_eq!(app.bus.history().len(), 1, "the arm is the one step");
    // No render made again: the live chain plays, aligned — the live mix to the bit.
    same_bits(&mix(&mut app), &live, "brought live");
    // Its own edits now go through; the synth's are still locked.
    assert!(app.exec(Command::SetStripGain {
        id: bass_strip,
        gain_db: -1.0
    }));
    let synth = app.project().tracks[0].id;
    let clip = app.project().tracks[0].clips[0].id;
    assert!(!app.exec(Command::SetClipMute {
        track: synth,
        clip,
        muted: true
    }));
}

#[test]
fn it_will_not_start_while_the_tempo_follows_a_sync_source() {
    let (mut app, _dir) = app("sync");
    app.settings.sync.source = cantus_ui::views::transport::sync_service::SyncSource::Link;
    assert!(app.record_mode_begin(Options::default()).is_err());
    app.record_mode_ask();
    assert!(app.freeze.record.ask.is_none());
    assert!(app.status.error && app.status.text.contains("Link"));
}

#[test]
fn following_a_sync_source_during_the_mode_leaves_it() {
    let ctx = support::ctx();
    let (mut app, _dir) = app("sync_during");
    enter(&mut app, &ctx, Options::default());
    app.settings.sync.source = cantus_ui::views::transport::sync_service::SyncSource::Clock;
    cantus_ui::freeze::tick(&mut app, &ctx);
    assert!(!app.freeze.record.on());
    assert!(!app.project().any_frozen());
    assert!(app.bus.record_lock().is_none());
}

#[test]
fn low_latency_is_forced_on_and_said_while_the_mode_is_on() {
    let ctx = support::ctx();
    let (mut app, _dir) = app("ll");
    app.settings.low_latency = false;
    assert!(cantus_ui::record_mode::forced_limit(&app).is_none());
    enter(&mut app, &ctx, Options::default());
    assert!(cantus_ui::record_mode::forced_limit(&app).is_some());
}

#[test]
fn an_agent_arming_a_baked_track_brings_it_live() {
    let ctx = support::ctx();
    let (mut app, _dir) = app("agent_arm");
    enter(&mut app, &ctx, Options::default());
    let bass = app.project().tracks[1].id;
    let r = cantus_mcp::AgentHost::execute(
        &mut app,
        Command::SetTrackArmed {
            id: bass,
            armed: true,
        },
        None,
    );
    assert!(r.is_ok(), "{r:?}");
    assert!(app.project().track(bass).unwrap().armed);
}

#[test]
fn a_desk_recall_that_reaches_baked_strips_is_refused_and_said() {
    use cantus_core::mix_cmd::MixCommand;
    let ctx = support::ctx();
    let (mut app, _dir) = app("recall");
    enter(&mut app, &ctx, Options::default());
    let synth = app.project().tracks[0].strip.unwrap();
    let mut state = cantus_core::mix_snap::capture(app.project(), &[], false);
    if let Some(st) = state.strips.iter_mut().find(|x| x.id == synth) {
        st.phase_invert = true;
    }
    assert!(!app.exec(Command::Mix(MixCommand::Apply {
        state: Box::new(state)
    })));
    assert!(
        app.status.error && app.status.text.contains("recall"),
        "{}",
        app.status.text
    );
}

/// The host's test gain (a CLAP), built where the host's tests build it (once a process).
fn test_gain() -> std::path::PathBuf {
    static BUILT: std::sync::OnceLock<std::path::PathBuf> = std::sync::OnceLock::new();
    BUILT
        .get_or_init(|| {
            let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("..")
                .join("..");
            let target = root.join("target").join("test-plugins");
            let file = target.join("debug").join(if cfg!(windows) {
                "cantus_test_clap.dll"
            } else if cfg!(target_os = "macos") {
                "libcantus_test_clap.dylib"
            } else {
                "libcantus_test_clap.so"
            });
            if std::env::var_os("CANTUS_TEST_PLUGINS_PREBUILT").is_some() && file.is_file() {
                return file;
            }
            let status = std::process::Command::new(
                std::env::var("CARGO").unwrap_or_else(|_| "cargo".into()),
            )
            .args(["build", "-p", "cantus-test-clap", "--target-dir"])
            .arg(&target)
            .current_dir(&root)
            .status()
            .expect("cargo");
            assert!(status.success());
            file
        })
        .clone()
}

/// The app's plug-in database knows the test gain, as after a scan; its info.
fn knows_gain(app: &mut CantusApp, dir: &std::path::Path) -> cantus_host::PluginInfo {
    use cantus_host::Backend;
    app.plugins.db_path = None;
    let file = dir.join("Cantus Test Gain.clap");
    std::fs::copy(test_gain(), &file).unwrap();
    let plugins = cantus_host::clap::ClapBackend.scan(&file).unwrap();
    let (modified, size) = cantus_host::db::stamp(&file).unwrap();
    let info = plugins
        .iter()
        .find(|p| p.id == "org.cantus.test.gain")
        .expect("the gain")
        .clone();
    app.plugins.db.put(cantus_host::DbEntry {
        path: file,
        format: cantus_host::Format::Clap,
        modified,
        size,
        status: cantus_host::ScanStatus::Ok,
        plugins,
    });
    info
}

#[test]
fn a_hosted_plug_in_on_the_master_is_in_the_baked_master() {
    let ctx = support::ctx();
    let (mut app, dir) = app("master_hosted");
    let before = mix(&mut app);
    let info = knows_gain(&mut app, &dir);
    let slot = app
        .plugins
        .make_slot_isolated(
            &mut app.bus,
            StripId::MASTER,
            &info,
            Some(PluginIsolation::Within),
        )
        .expect("it loads");
    let effect = slot.id;
    assert!(app.exec(Command::InsertEffect {
        strip: StripId::MASTER,
        index: 0,
        slot: Box::new(slot),
    }));
    assert!(app.exec(Command::SetPluginParam {
        strip: StripId::MASTER,
        effect,
        id: 0,
        value: Some(-6.0),
    }));
    app.sync_engine();
    enter(
        &mut app,
        &ctx,
        Options {
            master_live: false,
            ..Options::default()
        },
    );
    assert!(
        app.freeze.record.master_baked(),
        "{:?}",
        app.freeze.record.phase
    );
    // The baked master holds the plug-in: the mix 6 dB down.
    let baked = mix(&mut app);
    let g = 10f32.powf(-6.0 / 20.0);
    assert_eq!(baked.len(), before.len());
    let worst = baked
        .iter()
        .zip(&before)
        .map(|(b, a)| (b - a * g).abs())
        .fold(0f32, f32::max);
    assert!(worst < 1e-5, "the gain is in the render: {worst}");
}

#[test]
fn a_master_plug_in_that_cannot_render_keeps_the_master_live_and_is_named() {
    let ctx = support::ctx();
    let (mut app, _dir) = app("master_missing");
    // A hosted insert on the master that is not installed here.
    let mut p = app.project().clone();
    let mut slot = p.make_effect(EffectKind::External);
    let mut plugin = cantus_core::ExternalPlugin::new(
        cantus_core::PluginFormat::Clap,
        "org.cantus.test.nowhere".to_string(),
        "Nowhere Limiter",
    );
    plugin.isolation = Some(PluginIsolation::Within);
    slot.spec.external = Some(Box::new(plugin));
    assert!(app.exec(Command::InsertEffect {
        strip: StripId::MASTER,
        index: 0,
        slot: Box::new(slot),
    }));
    app.record_mode_ask();
    app.record_mode_begin(Options {
        master_live: false,
        ..Options::default()
    })
    .unwrap();
    settle(&mut app, &ctx);
    assert!(app.freeze.record.is_active());
    assert!(!app.freeze.record.master_baked(), "the master stays live");
    assert!(app.engine.frozen.master().is_none());
    let a = app.freeze.record.active().unwrap();
    assert!(a.options.master_live);
    assert!(
        app.status.text.contains("Nowhere Limiter"),
        "{}",
        app.status.text
    );
}
