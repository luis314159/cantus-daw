//! ARA 2 in the application (round 36, ADR 0097), with the repository's ARA test plug-in:
//! the agent tools list, bind and let go; the window's engine sync binds the plug-in and
//! hands the engine the render; saving takes the plug-in's archive; undo binds again. And the
//! clip menu's words are there in both languages.

use cantus_core::*;
use cantus_mcp::client::{TestClient, ToolCall};
use cantus_mcp::Answer;
use cantus_ui::CantusApp;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::process::Command as Process;

fn workspace() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// The ARA test plug-in built (by cargo, into a target folder of its own) and bundled.
fn bundle() -> PathBuf {
    let (built, arch, ext) = if cfg!(target_os = "windows") {
        ("cantus_test_ara.dll", "x86_64-win", ".vst3")
    } else if cfg!(target_os = "macos") {
        ("libcantus_test_ara.dylib", "MacOS", "")
    } else {
        ("libcantus_test_ara.so", "x86_64-linux", ".so")
    };
    let target = workspace().join("target").join("cantus-test-plugins");
    let cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
    let ok = Process::new(cargo)
        .current_dir(workspace())
        .args(["build", "--quiet", "-p", "cantus-test-ara", "--target-dir"])
        .arg(&target)
        .status()
        .expect("cargo builds the ARA test plug-in");
    assert!(ok.success());
    let bundle = target.join("bundles-ara-ui").join("CantusTestARA.vst3");
    let dir = bundle.join("Contents").join(arch);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::copy(
        target.join("debug").join(built),
        dir.join(format!("CantusTestARA{ext}")),
    )
    .unwrap();
    bundle
}

fn app_with(p: Project) -> (CantusApp, TestClient) {
    cantus_ui::i18n::set_thread_locale(cantus_ui::i18n::Locale::En);
    let mut app = CantusApp::from_parts(Some(p), cantus_engine::EngineHandle::dummy(), false);
    let ctx = egui::Context::default();
    cantus_ui::ai_mcp::wire_with(&mut app, &ctx, cantus_ui::ai_mcp::catalog::app());
    let client = TestClient::legacy(app.ai.server().unwrap().clone(), "2025-11-25");
    (app, client)
}

fn call(app: &mut CantusApp, c: &mut TestClient, name: &str, args: Value) -> ToolCall {
    c.call_tool(name, args, &mut || {
        cantus_ui::ai_mcp::pump(app);
        if let Some(agents) = app.ai.agents_mut() {
            let ids: Vec<u64> = agents.asks().map(|x| x.id).collect();
            for id in ids {
                agents.answer(id, Answer::Allow);
            }
        }
    })
}

fn ok(app: &mut CantusApp, c: &mut TestClient, name: &str, args: Value) -> Value {
    let r = call(app, c, name, args.clone());
    assert!(!r.is_error, "{name}({args}): {}", r.text);
    r.structured.unwrap_or(Value::Null)
}

/// One track with one audio clip over a short stereo file.
fn project(dir: &Path) -> (Project, ClipId) {
    let path = dir.join("take.wav");
    let spec = hound::WavSpec {
        channels: 2,
        sample_rate: 48_000,
        bits_per_sample: 32,
        sample_format: hound::SampleFormat::Float,
    };
    let mut w = hound::WavWriter::create(&path, spec).unwrap();
    for i in 0..48_000 {
        let x = (i as f32 * 0.01).sin() * 0.5;
        w.write_sample(x).unwrap();
        w.write_sample(-x).unwrap();
    }
    w.finalize().unwrap();
    let mut p = Project::new("ara");
    let mut t = p.make_track("Vocal");
    let id = AudioSourceId(p.alloc_id());
    p.audio_pool.push(AudioSource {
        id,
        name: "take".into(),
        path: path.to_string_lossy().into_owned(),
        sample_rate: 48_000,
        channels: 2,
        frames: 48_000,
        sha256: None,
        recorded: false,
    });
    let c = p.make_audio_clip("take", id, Ticks::ZERO, Ticks::beats(1));
    let cid = c.id;
    t.clips.push(c);
    p.tracks.push(t);
    (p, cid)
}

#[test]
fn agents_bind_and_let_go_and_the_window_renders_saves_and_undoes() {
    let dir = tempfile::tempdir().unwrap();
    let (p, clip) = project(dir.path());
    let (mut app, mut c) = app_with(p);
    let none = ok(&mut app, &mut c, "list_ara_plugins", json!({}));
    assert_eq!(none["plugins"], json!([]));
    assert_eq!(none["in_process"], json!(true));
    let r = call(
        &mut app,
        &mut c,
        "bind_ara_plugin",
        json!({"clip_id": clip.0, "plugin": "Cantus Test ARA"}),
    );
    assert!(r.is_error, "no such plug-in yet");
    // The scan found the test plug-in.
    use cantus_host::Backend;
    let info = cantus_host::vst3::Vst3Backend::new()
        .scan(&bundle())
        .unwrap()
        .remove(0);
    assert!(info
        .features
        .iter()
        .any(|f| f == cantus_host::db::ARA_FEATURE));
    app.plugins.db.put(cantus_host::DbEntry {
        path: info.path.clone(),
        format: cantus_host::Format::Vst3,
        modified: 1,
        size: 1,
        status: cantus_host::ScanStatus::Ok,
        plugins: vec![info.clone()],
    });
    let listed = ok(&mut app, &mut c, "list_ara_plugins", json!({}));
    assert_eq!(listed["plugins"][0]["name"], json!("Cantus Test ARA"));
    let bound = ok(
        &mut app,
        &mut c,
        "bind_ara_plugin",
        json!({"clip_id": clip.0, "plugin": "Cantus Test ARA"}),
    );
    assert_eq!(bound["in_process"], json!(true));
    // The window's engine sync binds the plug-in and renders the clip.
    // An offline render waits while the clip's render is on its way (r36 fix round).
    app.sync_engine();
    let now = app.project().clone();
    assert!(!app.plugins.ara_settled());
    assert!(!app
        .plugins
        .render_ready(cantus_host::RenderFor::Export, &now, 48_000.0, false));
    frame(&mut app);
    let mut ready = false;
    for _ in 0..200 {
        if app
            .plugins
            .render_ready(cantus_host::RenderFor::Export, &now, 48_000.0, false)
        {
            ready = true;
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    assert!(
        ready && app.plugins.ara_settled(),
        "ready once the render is played"
    );
    assert_eq!(
        app.plugins.ara.status(clip),
        Some(&cantus_host::ara::AraStatus::Live)
    );
    assert!(app.plugins.ara.renders().contains_key(&clip));
    let listed = ok(&mut app, &mut c, "list_ara_plugins", json!({}));
    assert_eq!(listed["bound_clips"][0]["status"], json!("live"));
    assert!(app
        .ara_status_text((app.project().tracks[0].id, clip))
        .unwrap()
        .contains("renders this clip"));
    // Saving takes the plug-in's archive of the clip.
    let now = app.project().clone();
    let saved = app.plugins.project_for_save(&now);
    let ara = saved.tracks[0].clips[0]
        .audio
        .as_ref()
        .unwrap()
        .ara
        .as_ref()
        .unwrap();
    assert!(ara.archive.is_some(), "an archive");
    // Let go, then the user's undo binds it again.
    ok(
        &mut app,
        &mut c,
        "unbind_ara_plugin",
        json!({"clip_id": clip.0}),
    );
    frame(&mut app);
    assert!(app.plugins.ara.renders().is_empty());
    app.bus.undo().unwrap();
    frame(&mut app);
    assert!(app.plugins.ara.renders().contains_key(&clip));
}

#[test]
fn the_clip_menus_words_are_in_both_languages() {
    use cantus_ui::i18n::{set_thread_locale, t, Locale};
    for key in [
        "ara.menu.band",
        "ara.menu.in_process",
        "ara.menu.none",
        "cmd.bind_ara",
        "cmd.unbind_ara",
        "ara.why.no_source",
    ] {
        set_thread_locale(Locale::En);
        let en = t(key);
        set_thread_locale(Locale::Es);
        let es = t(key);
        assert!(en != key && es != key && en != es, "{key}: {en} / {es}");
    }
    set_thread_locale(Locale::En);
    assert!(t("ara.menu.in_process").contains("not isolated"));
}

/// The window's engine sync, the render thread waited for, and the sync again (what the
/// next frames do).
fn frame(app: &mut CantusApp) {
    app.sync_engine();
    app.plugins.ara.wait(std::time::Duration::from_secs(20));
    app.sync_engine();
}
