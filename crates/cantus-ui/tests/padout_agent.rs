//! Pulsus III for agents through the MCP dispatcher, the application as host (round 37,
//! ADR 0100, review 2 #2 #4 #6 #8): `split_kit_outputs`, `set_pad_chain` and
//! `make_kit_from_folder` answer as a client sees them; the mixer's JSON says each strip's
//! kind; removing or replacing a kit through the agent's tools leaves no pad strip behind;
//! and a kit from a folder reads only the project's folder and the folders the user
//! allowed, refuses a track that is not a kit, and names no path in what it answers.

use cantus_core::*;
use cantus_mcp::client::{TestClient, ToolCall};
use cantus_mcp::Answer;
use cantus_ui::CantusApp;
use serde_json::{json, Value};

fn app_with(p: Project) -> (CantusApp, TestClient) {
    cantus_ui::i18n::set_thread_locale(cantus_ui::i18n::Locale::En);
    let mut app = CantusApp::from_parts(Some(p), cantus_engine::EngineHandle::dummy(), false);
    let ctx = egui::Context::default();
    cantus_ui::ai_mcp::wire_with(&mut app, &ctx, cantus_ui::ai_mcp::catalog::app());
    let client = TestClient::legacy(app.ai.server().unwrap().clone(), "2025-11-25");
    (app, client)
}

/// A call whose every question is answered Allow, as a user at the card would.
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

/// A kit track (the studio kit) and a synth track.
fn kit_project() -> Project {
    let mut p = Project::new("Kit");
    let mut t = p.make_track("Drums");
    t.instrument = InstrumentSpec::Pulsus(PulsusParams::studio());
    p.tracks.push(t);
    let keys = p.make_track("Keys");
    p.tracks.push(keys);
    p
}

fn kinds(m: &Value, kind: &str) -> Vec<u64> {
    m["strips"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|s| s["kind"] == kind)
        .map(|s| s["id"].as_u64().unwrap())
        .collect()
}

#[test]
fn split_merge_and_a_pads_chain_answer_through_the_dispatcher() {
    let (mut app, mut c) = app_with(kit_project());
    let t = app.project().tracks[0].id;
    let keys = app.project().tracks[1].id;
    // Split: every pad on a strip of its own, said "pad" in the mixer's JSON.
    ok(
        &mut app,
        &mut c,
        "split_kit_outputs",
        json!({"track_id": t.0}),
    );
    assert!(padout::is_split(app.project(), t));
    let outs = app.project().track(t).unwrap().instrument.output_strips();
    let m = ok(&mut app, &mut c, "get_mixer", json!({}));
    let mut pads = kinds(&m, "pad");
    pads.sort_unstable();
    let mut want: Vec<u64> = outs.iter().map(|s| s.0).collect();
    want.sort_unstable();
    want.dedup();
    assert_eq!(pads, want, "the pad strips, by kind");
    // Merge back: none left.
    ok(
        &mut app,
        &mut c,
        "split_kit_outputs",
        json!({"track_id": t.0, "merge": true}),
    );
    assert!(!padout::is_split(app.project(), t));
    let m = ok(&mut app, &mut c, "get_mixer", json!({}));
    assert!(kinds(&m, "pad").is_empty());
    // A pad's chain: made with its first insert, said "chain", gone with its last.
    ok(
        &mut app,
        &mut c,
        "set_pad_chain",
        json!({"track_id": t.0, "pad": 38, "add": "compressor"}),
    );
    let snare = |app: &CantusApp| {
        app.project()
            .track(t)
            .unwrap()
            .instrument
            .pulsus()
            .unwrap()
            .pads
            .iter()
            .find(|p| p.pitch == 38)
            .unwrap()
            .clone()
    };
    let pad = snare(&app);
    let chain = padout::chain_of(app.project(), &pad).expect("a chain");
    let m = ok(&mut app, &mut c, "get_mixer", json!({}));
    assert_eq!(kinds(&m, "chain"), vec![chain.0]);
    let fx = padout::chain_effects(app.project(), &pad)[0].id;
    assert_eq!(
        padout::chain_effects(app.project(), &pad)[0].spec.kind,
        EffectKind::Compressor
    );
    ok(
        &mut app,
        &mut c,
        "set_pad_chain",
        json!({"track_id": t.0, "pad": 38, "remove": fx.0}),
    );
    assert!(padout::chain_of(app.project(), &snare(&app)).is_none());
    assert!(
        app.project().strip(chain).is_none(),
        "the chain went with it"
    );
    // A track that is not a kit is refused, and nothing changes.
    let before = app.project().clone();
    for (tool, args) in [
        ("split_kit_outputs", json!({"track_id": keys.0})),
        (
            "set_pad_chain",
            json!({"track_id": keys.0, "pad": 38, "add": "eq"}),
        ),
    ] {
        let r = call(&mut app, &mut c, tool, args);
        assert!(
            r.is_error && r.text.contains("not a Pulsus kit"),
            "{tool}: {}",
            r.text
        );
    }
    assert_eq!(app.project(), &before);
}

/// Removing the kit's track (`remove_items`) or loading another kit on it (`apply_preset`)
/// through the agent's tools: no pad strip, no chain left behind (review 2 #4) — and a
/// preset on a split kit keeps it split where its pads match.
#[test]
fn the_agents_remove_and_preset_leave_no_pad_strip_behind() {
    let (mut app, mut c) = app_with(kit_project());
    let t = app.project().tracks[0].id;
    let made = |app: &CantusApp| {
        app.project()
            .strips
            .iter()
            .filter(|s| padout::pad_made(s))
            .count()
    };
    ok(
        &mut app,
        &mut c,
        "set_pad_chain",
        json!({"track_id": t.0, "pad": 36, "add": "eq"}),
    );
    ok(
        &mut app,
        &mut c,
        "split_kit_outputs",
        json!({"track_id": t.0}),
    );
    assert!(made(&app) > 2);
    // Another kit by preset: pads on the same notes keep their strips; nothing unused.
    ok(
        &mut app,
        &mut c,
        "apply_preset",
        json!({"track_id": t.0, "preset": "Default kit"}),
    );
    assert!(padout::orphans(app.project()).is_empty());
    assert!(padout::is_split(app.project(), t), "still split");
    // The track removed: every strip its pads played on goes with it.
    ok(
        &mut app,
        &mut c,
        "remove_items",
        json!({"track_ids": [t.0]}),
    );
    assert_eq!(made(&app), 0, "a pad strip or a chain was left");
}

/// A short tone on disk.
fn tone(path: &std::path::Path, hz: f32) {
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: 48_000,
        bits_per_sample: 32,
        sample_format: hound::SampleFormat::Float,
    };
    let mut w = hound::WavWriter::create(path, spec).unwrap();
    for n in 0..2_400 {
        let env = 1.0 - n as f32 / 2_400.0;
        let x = (std::f32::consts::TAU * hz * n as f32 / 48_000.0).sin() * 0.5 * env;
        w.write_sample(x).unwrap();
    }
    w.finalize().unwrap();
}

fn drums_in(dir: &std::path::Path) {
    std::fs::create_dir_all(dir).unwrap();
    for (n, hz) in [
        ("Kick.wav", 60.0),
        ("Snare.wav", 200.0),
        ("HH Closed.wav", 3000.0),
    ] {
        tone(&dir.join(n), hz);
    }
}

#[test]
fn a_kit_from_a_folder_reads_only_the_allowed_folders_and_names_no_path() {
    let (mut app, mut c) = app_with(kit_project());
    let kit = app.project().tracks[0].id;
    let keys = app.project().tracks[1].id;
    let dir = tempfile::tempdir().unwrap();
    let song = dir.path().join("song");
    let samples = dir.path().join("samples");
    let outside = dir.path().join("outside");
    std::fs::create_dir_all(&song).unwrap();
    drums_in(&samples);
    drums_in(&outside);
    app.path = Some(song.join("song.cantus"));
    app.settings.ai.folders = vec![samples.clone()];
    let n = app.project().tracks.len();
    let path = |p: &std::path::Path| json!({"path": p.display().to_string()});
    // Files off: refused.
    let r = call(&mut app, &mut c, "make_kit_from_folder", path(&samples));
    assert!(r.is_error, "{}", r.text);
    app.settings.ai.allow_files = true;
    // Outside the roots, directly or through `..`: refused, nothing read.
    for p in [outside.clone(), samples.join("..").join("outside")] {
        let r = call(&mut app, &mut c, "make_kit_from_folder", path(&p));
        assert!(
            r.is_error && r.text.contains("outside"),
            "{p:?}: {}",
            r.text
        );
    }
    assert_eq!(app.project().tracks.len(), n);
    // A track that is not a kit: refused, its synth kept.
    let r = call(
        &mut app,
        &mut c,
        "make_kit_from_folder",
        json!({"path": samples.display().to_string(), "track_id": keys.0}),
    );
    assert!(
        r.is_error && r.text.contains("not a Pulsus kit"),
        "{}",
        r.text
    );
    assert!(app
        .project()
        .track(keys)
        .unwrap()
        .instrument
        .pulsus()
        .is_none());
    // Inside: a new kit track; what it answers names no folder.
    let r = call(&mut app, &mut c, "make_kit_from_folder", path(&samples));
    assert!(!r.is_error, "{}", r.text);
    assert_eq!(app.project().tracks.len(), n + 1);
    let said = format!("{} {:?}", r.text, r.structured);
    let root = dir.path().display().to_string();
    assert!(!said.contains(&root), "a path in the answer: {said}");
    let v = r.structured.unwrap();
    let notes: Vec<u64> = v["pads"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| p["note"].as_u64().unwrap())
        .collect();
    assert!(
        notes.contains(&36) && notes.contains(&38) && notes.contains(&42),
        "{v}"
    );
    // Onto the kit: the same track.
    let v = ok(
        &mut app,
        &mut c,
        "make_kit_from_folder",
        json!({"path": samples.display().to_string(), "track_id": kit.0}),
    );
    assert_eq!(v["track_id"], kit.0);
    // An SFZ inside whose region names a sample outside: that region is left out, said by
    // count, and the rest is a kit.
    std::fs::write(
        samples.join("kit.sfz"),
        "<region> sample=Kick.wav key=36\n<region> sample=../outside/Snare.wav key=38\n",
    )
    .unwrap();
    let r = call(
        &mut app,
        &mut c,
        "make_kit_from_folder",
        path(&samples.join("kit.sfz")),
    );
    assert!(!r.is_error, "{}", r.text);
    let v = r.structured.unwrap();
    let notes: Vec<u64> = v["pads"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| p["note"].as_u64().unwrap())
        .collect();
    assert_eq!(notes, vec![36], "{v}");
    assert!(v["not_kept"].to_string().contains("outside"), "{v}");
    assert!(!v.to_string().contains(&root), "{v}");
}
