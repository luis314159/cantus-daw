//! The project's tools through the MCP server with the application as host (round 34,
//! ADR 0082): the unit catalogue complete and generated from the tables, every sentence in
//! both languages, a song composed only through tools and rendered, comping, export, and
//! the user's Undo taking every agent step back exactly.

use cantus_core::*;
use cantus_mcp::client::{TestClient, ToolCall};
use cantus_mcp::Answer;
use cantus_ui::ai_tools::project_app::units;
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

fn bare(p: &Project) -> Project {
    let mut p = p.clone();
    p.next_id = 0;
    p
}

#[test]
fn the_catalogue_is_complete_and_generated_from_the_tables() {
    cantus_ui::i18n::set_thread_locale(cantus_ui::i18n::Locale::En);
    let list = units::list();
    let ids: Vec<&str> = list["units"]
        .as_array()
        .unwrap()
        .iter()
        .map(|u| u["id"].as_str().unwrap())
        .collect();
    for kind in EffectKind::ALL
        .into_iter()
        .filter(|k| *k != EffectKind::External)
    {
        let (id, _) = cantus_ai::tools_project::args::effect_names(kind);
        assert!(ids.contains(&id), "{kind:?} is not in the list");
        let e = units::entry(id).unwrap();
        let params = e["params"].as_array().unwrap();
        assert_eq!(
            params.len(),
            kind.params().len(),
            "{kind:?}: every parameter"
        );
        for (row, info) in params.iter().zip(kind.params()) {
            assert_eq!(row["key"], info.key);
            let name = row["name"].as_str().unwrap();
            assert!(
                !name.is_empty() && name != info.label_key,
                "{kind:?} {}: no English name",
                info.key
            );
            if cantus_ui::controllers_words::effect_choice_words(
                kind,
                info.key,
                cantus_ui::i18n::Locale::En,
            )
            .is_some()
            {
                let n = (info.max - info.min).round() as usize + 1;
                assert_eq!(
                    row["choices"].as_array().map(Vec::len),
                    Some(n),
                    "{kind:?} {}: choices by name",
                    info.key
                );
            }
        }
        assert!(
            e["latency_samples_at_48k"].as_u64().unwrap() < 48_000,
            "{kind:?}"
        );
        assert!(!e["what"].as_str().unwrap().is_empty());
        for p in e["presets"].as_array().unwrap() {
            for (k, _) in p["sets"].as_object().unwrap() {
                assert!(
                    kind.param(k).is_some(),
                    "{kind:?} preset sets {k}, which it does not have"
                );
            }
        }
    }
    for kind in InstrumentKind::ALL {
        let id = cantus_ai::tools_project::args::native_id(kind);
        assert!(ids.contains(&id), "{kind:?} is not in the list");
        let e = units::entry(id).unwrap();
        assert_eq!(
            e["params"].as_array().unwrap().len(),
            kind.params().len(),
            "{kind:?}"
        );
        for row in e["params"].as_array().unwrap() {
            assert!(
                !row["name"].as_str().unwrap().contains('.')
                    || row["name"].as_str().unwrap().contains(' '),
                "{kind:?} {}: an English name",
                row["key"]
            );
        }
    }
    assert_eq!(
        units::entry("firmus").unwrap()["params"]
            .as_array()
            .unwrap()
            .len(),
        FirmusParam::ALL.len()
    );
    assert!(
        units::entry("pulsus").unwrap()["params"]
            .as_array()
            .unwrap()
            .len()
            > 20
    );
    // A plain role finds its unit; an unknown one finds nothing.
    assert_eq!(units::entry("eq").unwrap()["id"], "aequalis");
    assert!(units::entry("theremin").is_none());
}

/// Every sentence and step name the tools use is in English and in Spanish.
#[test]
fn every_sentence_is_in_both_languages() {
    let sources = [
        include_str!("../../cantus-ai/src/tools_args.rs"),
        include_str!("../../cantus-ai/src/tools_song.rs"),
        include_str!("../../cantus-ai/src/tools_tracks.rs"),
        include_str!("../../cantus-ai/src/tools_notes.rs"),
        include_str!("../../cantus-ai/src/tools_notes2.rs"),
        include_str!("../../cantus-ai/src/tools_mixer.rs"),
        include_str!("../../cantus-ai/src/tools_mod.rs"),
        include_str!("../../cantus-ai/src/tools_music.rs"),
        include_str!("../../cantus-ai/src/tools_session.rs"),
        include_str!("../../cantus-ai/src/tools_remove.rs"),
        include_str!("../../cantus-ai/src/tools_res.rs"),
        include_str!("../../cantus-ai/src/tools_words.rs"),
        include_str!("../src/agent_tools.rs"),
        include_str!("../src/agent_export.rs"),
    ];
    let mut keys = std::collections::BTreeSet::new();
    for s in sources {
        for part in s.split('"').skip(1).step_by(2) {
            if ["ai.tools.", "ai.prompt.", "ai.phrase.", "ai.field.", "cmd."]
                .iter()
                .any(|p| part.starts_with(p))
                && part
                    .chars()
                    .all(|c| c.is_ascii_lowercase() || c == '.' || c == '_')
            {
                keys.insert(part.to_string());
            }
        }
    }
    assert!(keys.len() > 60, "{} keys found", keys.len());
    for k in &keys {
        for l in [cantus_ui::i18n::Locale::En, cantus_ui::i18n::Locale::Es] {
            assert!(cantus_ui::i18n::has(l, k), "{k} has no words in {l:?}");
        }
    }
}

/// A short piece made only through the server: tracks, a voiced chart, a bass line, drums,
/// a preset, automation, a modulator, the session — each call one undo step — rendered
/// offline to sound, and every step taken back by the user's Undo.
#[test]
fn compose_a_song_through_the_server_render_it_and_undo_it_all() {
    let (mut app, mut c) = app_with(Project::new("Through the server"));
    app.settings.ai.allow_edits = true;
    let start = bare(app.project());
    let tools: Vec<String> = c
        .request("tools/list", json!({}), &mut || {
            cantus_ui::ai_mcp::pump(&mut app)
        })
        .unwrap()["tools"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["name"].as_str().unwrap().to_string())
        .collect();
    for name in [
        "get_project",
        "write_part",
        "lint_music",
        "get_unit",
        "export_audio",
        "transport",
    ] {
        assert!(tools.iter().any(|t| t == name), "{name} is listed");
    }
    ok(&mut app, &mut c, "set_tempo", json!({"bpm": 100}));
    ok(
        &mut app,
        &mut c,
        "set_song",
        json!({"key": {"root": "C", "scale": "major"}}),
    );
    let chart = "C | Am | F | G7";
    let keys = ok(
        &mut app,
        &mut c,
        "add_track",
        json!({"kind": "lamina", "name": "Keys"}),
    )["track_id"]
        .as_u64()
        .unwrap();
    let bass = ok(
        &mut app,
        &mut c,
        "add_track",
        json!({"kind": "gravis", "name": "Bass"}),
    )["track_id"]
        .as_u64()
        .unwrap();
    let drums = ok(
        &mut app,
        &mut c,
        "add_track",
        json!({"kind": "pulsus", "name": "Drums"}),
    )["track_id"]
        .as_u64()
        .unwrap();
    ok(
        &mut app,
        &mut c,
        "voice_chords",
        json!({"track_id": keys, "chart": chart}),
    );
    ok(
        &mut app,
        &mut c,
        "write_part",
        json!({"track_id": bass, "chart": chart, "pattern": "bass", "rate": "1/4"}),
    );
    ok(
        &mut app,
        &mut c,
        "write_drum_pattern",
        json!({"track_id": drums, "at": 0, "bars": 4, "rows": [{"pad": "kick", "steps": "x...x...x...x..."}, {"pad": "snare", "steps": "....x.......x..."}]}),
    );
    let fx = ok(
        &mut app,
        &mut c,
        "add_effect",
        json!({"track_id": keys, "kind": "concentus"}),
    )["effect_id"]
        .as_u64()
        .unwrap();
    let presets = ok(&mut app, &mut c, "list_presets", json!({"effect_id": fx}));
    let first = presets["presets"][0]["name"].as_str().unwrap().to_string();
    ok(
        &mut app,
        &mut c,
        "apply_preset",
        json!({"effect_id": fx, "preset": first}),
    );
    let lamina_presets = ok(&mut app, &mut c, "list_presets", json!({"track_id": keys}));
    if let Some(p) = lamina_presets["presets"][0]["name"].as_str() {
        ok(
            &mut app,
            &mut c,
            "apply_preset",
            json!({"track_id": keys, "preset": p}),
        );
    }
    ok(
        &mut app,
        &mut c,
        "automate",
        json!({"target": {"track_id": keys, "param": "gain_db"}, "points": [{"at": 0, "value": -12}, {"at": "3:1", "value": 0}]}),
    );
    ok(
        &mut app,
        &mut c,
        "add_modulator",
        json!({"host": keys, "kind": "lfo", "rate": "1/1", "routes": [{"target": {"track_id": keys, "param": "pan"}, "depth": 0.2}]}),
    );
    ok(
        &mut app,
        &mut c,
        "set_session",
        json!({"add_scene": {"name": "Loop"}, "slot": {"track_id": bass, "scene_id": 0, "length_beats": 4, "notes": [{"start_beats": 0, "length_beats": 1, "pitch": "C2"}]}}),
    );
    let lint = ok(
        &mut app,
        &mut c,
        "lint_music",
        json!({"chart": chart, "info": false}),
    );
    assert_eq!(lint["errors"], 0, "{lint}");
    // The application's own: the selection and the transport (not undo steps).
    let sel = ok(&mut app, &mut c, "get_selection", json!({}));
    assert_eq!(sel["transport"]["playing"], false);
    ok(
        &mut app,
        &mut c,
        "transport",
        json!({"action": "locate", "at": "2:1"}),
    );
    // Resources and prompts.
    let r = c
        .request(
            "resources/read",
            json!({"uri": "cantus://units/chorda"}),
            &mut || cantus_ui::ai_mcp::pump(&mut app),
        )
        .unwrap();
    assert!(r["contents"][0]["text"]
        .as_str()
        .unwrap()
        .contains("\"pedal\""));
    let r = c
        .request(
            "resources/read",
            json!({"uri": "cantus://project"}),
            &mut || cantus_ui::ai_mcp::pump(&mut app),
        )
        .unwrap();
    assert!(r["contents"][0]["text"]
        .as_str()
        .unwrap()
        .contains("\"Keys\""));
    let r = c
        .request(
            "prompts/get",
            json!({"name": "write_a_part", "arguments": {"track": "Keys"}}),
            &mut || cantus_ui::ai_mcp::pump(&mut app),
        )
        .unwrap();
    assert!(r["messages"][0]["content"]["text"]
        .as_str()
        .unwrap()
        .contains("Keys"));
    // It sounds: rendered offline, every track heard, nothing over full scale or not a number.
    let snap = Snapshot::compile(app.project(), 0);
    let out = cantus_engine::render(
        std::sync::Arc::new(snap),
        &cantus_engine::RenderOptions {
            tail_seconds: 0.5,
            ..Default::default()
        },
    )
    .expect("renders");
    assert!(out.samples.iter().all(|s| s.is_finite()));
    assert!(out.peak > 0.01 && out.peak <= 1.0, "peak {}", out.peak);
    // Every step is the agent's, and the user's Undo takes them all back.
    assert!(app.bus.history().iter().all(|e| e.origin == Origin::Mcp));
    while app.bus.can_undo() {
        app.undo();
    }
    assert_eq!(bare(app.project()), start);
}

#[test]
fn comping_takes_through_the_server() {
    let mut p = Project::new("comp");
    let t = p.make_track("Vox");
    let tid = t.id;
    p.tracks.push(t);
    for (name, lane) in [("A", 1u8), ("B", 2), ("B", 0)] {
        let id = AudioSourceId(p.alloc_id());
        if !p.audio_pool.iter().any(|s| s.name == name) {
            p.audio_pool.push(cantus_core::audio::AudioSource {
                id,
                name: name.into(),
                path: format!("C:/rec/{name}.wav"),
                sample_rate: 48_000,
                channels: 1,
                frames: 480_000,
                sha256: None,
                recorded: true,
            });
        }
        let src = p.audio_pool.iter().find(|s| s.name == name).unwrap().id;
        let mut c = p.make_audio_clip("take", src, Ticks::ZERO, Ticks::beats(8));
        c.lane = lane;
        c.comp = lane == 0;
        p.track_mut(tid).unwrap().clips.push(c);
    }
    let (mut app, mut c) = app_with(p);
    app.settings.ai.allow_edits = true;
    let before = bare(app.project());
    ok(
        &mut app,
        &mut c,
        "comp_takes",
        json!({"track_id": tid.0, "op": "swipe", "lane": 1, "from": 2, "to": 4}),
    );
    let lane0 = app
        .project()
        .track(tid)
        .unwrap()
        .clips
        .iter()
        .filter(|c| c.lane == 0)
        .count();
    assert!(lane0 >= 2, "the swipe split the comp: {lane0}");
    ok(
        &mut app,
        &mut c,
        "comp_takes",
        json!({"track_id": tid.0, "op": "duplicate"}),
    );
    ok(
        &mut app,
        &mut c,
        "comp_takes",
        json!({"track_id": tid.0, "op": "rename", "comp": "B", "name": "Safe"}),
    );
    ok(
        &mut app,
        &mut c,
        "comp_takes",
        json!({"track_id": tid.0, "op": "switch", "comp": 0}),
    );
    ok(
        &mut app,
        &mut c,
        "tidy_takes",
        json!({"track_id": tid.0, "op": "delete_comp", "comp": "B"}),
    );
    assert!(
        call(
            &mut app,
            &mut c,
            "comp_takes",
            json!({"track_id": tid.0, "op": "switch", "comp": "Z"})
        )
        .is_error
    );
    while app.bus.can_undo() {
        app.undo();
    }
    assert_eq!(bare(app.project()), before);
}

#[test]
fn export_writes_only_where_allowed_and_only_when_files_are_allowed() {
    let dir = std::env::temp_dir().join(format!("cantus-agent-export-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let mut p = Project::new("export");
    let t = p.make_track("Lead");
    let tid = t.id;
    p.tracks.push(t);
    let (mut app, mut c) = app_with(p);
    app.settings.ai.allow_edits = true;
    ok(
        &mut app,
        &mut c,
        "add_notes",
        json!({"track_id": tid.0, "notes": [{"start": 0, "length": 1, "pitch": "C4"}]}),
    );
    app.path = Some(dir.join("song.cantus"));
    // Files are refused until the user allows them.
    let r = call(
        &mut app,
        &mut c,
        "export_project",
        json!({"format": "midi", "file": "song"}),
    );
    assert!(r.is_error, "{}", r.text);
    assert!(!dir.join("song.mid").exists());
    app.settings.ai.allow_files = true;
    let r = ok(
        &mut app,
        &mut c,
        "export_project",
        json!({"format": "midi", "file": "song"}),
    );
    let written = dir.join("song.mid");
    assert!(written.exists(), "{r}");
    assert_eq!(&std::fs::read(&written).unwrap()[..4], b"MThd");
    // Not over an existing file unless asked; never outside the project's folder.
    assert!(
        call(
            &mut app,
            &mut c,
            "export_project",
            json!({"format": "midi", "file": "song"})
        )
        .is_error
    );
    ok(
        &mut app,
        &mut c,
        "export_project",
        json!({"format": "midi", "file": "song", "overwrite": true, "track_id": tid.0}),
    );
    let outside = std::env::temp_dir().join("cantus-agent-outside.mid");
    let r = call(
        &mut app,
        &mut c,
        "export_project",
        json!({"format": "midi", "file": outside.display().to_string()}),
    );
    assert!(r.is_error && !outside.exists(), "{}", r.text);
    let r = call(
        &mut app,
        &mut c,
        "export_project",
        json!({"format": "midi", "file": "../escape"}),
    );
    assert!(r.is_error, "{}", r.text);
    // Audio starts through the export the dialog runs.
    let r = ok(
        &mut app,
        &mut c,
        "export_audio",
        json!({"format": "wav", "file": "mix"}),
    );
    assert_eq!(r["started"], true);
    cantus_ui::views::export::wait(&mut app);
    assert!(dir.join("mix.wav").exists());
    let sel = ok(&mut app, &mut c, "get_selection", json!({}));
    assert!(sel["export"]["last"]["done"].is_string(), "{sel}");
    let _ = std::fs::remove_dir_all(&dir);
}

/// Every tool of the project's two lists, called once through the server with the
/// application as host: each answers, and each that edits is exactly one undo step.
#[test]
fn every_project_tool_answers_through_the_server() {
    let (mut app, mut c) = app_with(Project::new("every tool"));
    app.settings.ai.allow_edits = true;
    let mut called = std::collections::BTreeSet::new();
    let mut step = |app: &mut CantusApp, c: &mut TestClient, name: &str, args: Value| -> Value {
        let def = cantus_ui::ai_tools::find(name).unwrap_or_else(|| panic!("{name} is registered"));
        let depth = app.bus.history().len();
        let v = ok(app, c, name, args);
        let edits = matches!(
            def.class,
            cantus_ai::ToolClass::Edit | cantus_ai::ToolClass::Destructive
        );
        assert_eq!(
            app.bus.history().len(),
            depth + usize::from(edits),
            "{name}: one undo step"
        );
        called.insert(name.to_string());
        v
    };
    let t = step(
        &mut app,
        &mut c,
        "add_track",
        json!({"kind": "chorda", "name": "Piano"}),
    )["track_id"]
        .as_u64()
        .unwrap();
    let d = step(&mut app, &mut c, "add_track", json!({"kind": "pulsus"}))["track_id"]
        .as_u64()
        .unwrap();
    let u = step(&mut app, &mut c, "add_track", json!({"kind": "unda"}))["track_id"]
        .as_u64()
        .unwrap();
    step(&mut app, &mut c, "get_project", json!({}));
    step(&mut app, &mut c, "get_track", json!({"track_id": t}));
    step(&mut app, &mut c, "get_mixer", json!({}));
    step(&mut app, &mut c, "get_history", json!({}));
    step(&mut app, &mut c, "get_selection", json!({}));
    step(&mut app, &mut c, "set_tempo", json!({"bpm": 110}));
    step(
        &mut app,
        &mut c,
        "set_tempo_point",
        json!({"at": "5:1", "bpm": 120}),
    );
    step(
        &mut app,
        &mut c,
        "set_song",
        json!({"key": {"root": "G", "scale": "major"}}),
    );
    let m = step(
        &mut app,
        &mut c,
        "set_marker",
        json!({"at": "3:1", "name": "B"}),
    )["marker_id"]
        .as_u64()
        .unwrap();
    step(&mut app, &mut c, "transport", json!({"action": "stop"}));
    step(
        &mut app,
        &mut c,
        "set_track",
        json!({"track_id": u, "pan": 0.3}),
    );
    step(
        &mut app,
        &mut c,
        "set_instrument_param",
        json!({"track_id": t, "params": {"pedal": 0.5}}),
    );
    let clip = step(
        &mut app,
        &mut c,
        "add_clip",
        json!({"track_id": u, "start": "1:1", "length": "1 bar"}),
    )["clip_id"]
        .as_u64()
        .unwrap();
    step(
        &mut app,
        &mut c,
        "add_notes",
        json!({"track_id": u, "clip_id": clip, "notes": [{"start": 0, "length": 1, "pitch": "G4"}]}),
    );
    step(&mut app, &mut c, "get_clip_notes", json!({"clip_id": clip}));
    step(
        &mut app,
        &mut c,
        "edit_notes",
        json!({"clip_id": clip, "velocity": 90}),
    );
    step(
        &mut app,
        &mut c,
        "replace_notes",
        json!({"clip_id": clip, "notes": [{"start": 1, "length": 1, "pitch": "B4"}]}),
    );
    step(
        &mut app,
        &mut c,
        "set_clip",
        json!({"clip_id": clip, "name": "Hook"}),
    );
    let copies = step(&mut app, &mut c, "duplicate_clip", json!({"clip_id": clip}));
    let copy = copies["copies"][0]["clip_id"].as_u64().unwrap();
    step(
        &mut app,
        &mut c,
        "write_drum_pattern",
        json!({"track_id": d, "at": 0, "rows": [{"pad": "kick", "steps": "x..."}]}),
    );
    step(
        &mut app,
        &mut c,
        "set_session",
        json!({"add_scene": {"name": "A"}}),
    );
    let s = step(&mut app, &mut c, "add_strip", json!({"name": "Bus"}))["strip_id"]
        .as_u64()
        .unwrap();
    let ustrip = app.project().track(TrackId(u)).unwrap().strip.unwrap().0;
    step(
        &mut app,
        &mut c,
        "set_strip",
        json!({"strip_id": s, "gain_db": -3}),
    );
    step(
        &mut app,
        &mut c,
        "set_send",
        json!({"strip_id": ustrip, "to": s, "level_db": -6}),
    );
    let fx = step(
        &mut app,
        &mut c,
        "add_effect",
        json!({"strip_id": ustrip, "kind": "fervor"}),
    )["effect_id"]
        .as_u64()
        .unwrap();
    step(&mut app, &mut c, "get_device", json!({"effect_id": fx}));
    step(
        &mut app,
        &mut c,
        "set_effect",
        json!({"effect_id": fx, "mix": 0.5}),
    );
    let fx = step(
        &mut app,
        &mut c,
        "replace_effect",
        json!({"effect_id": fx, "kind": "tonsor"}),
    )["effect_id"]
        .as_u64()
        .unwrap();
    step(&mut app, &mut c, "list_presets", json!({"effect_id": fx}));
    let preset = ok(&mut app, &mut c, "list_presets", json!({"effect_id": fx}))["presets"][0]
        ["name"]
        .as_str()
        .unwrap()
        .to_string();
    step(
        &mut app,
        &mut c,
        "apply_preset",
        json!({"effect_id": fx, "preset": preset}),
    );
    step(&mut app, &mut c, "list_units", json!({}));
    let unit = step(&mut app, &mut c, "get_unit", json!({"id": "tonsor"}));
    let key = unit["params"]
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["taper"] != "stepped")
        .unwrap()
        .clone();
    step(
        &mut app,
        &mut c,
        "automate",
        json!({"target": {"effect_id": fx, "param": key["key"]}, "points": [{"at": 0, "value": key["default"]}]}),
    );
    step(
        &mut app,
        &mut c,
        "replace_automation",
        json!({"target": {"effect_id": fx, "param": key["key"]}, "points": [{"at": 0, "value": key["min"]}]}),
    );
    let mid = step(
        &mut app,
        &mut c,
        "add_modulator",
        json!({"host": u, "kind": "random"}),
    )["modulator_id"]
        .as_u64()
        .unwrap();
    step(
        &mut app,
        &mut c,
        "set_modulator",
        json!({"modulator_id": mid, "smooth": true}),
    );
    step(
        &mut app,
        &mut c,
        "write_part",
        json!({"track_id": u, "chart": "G | C", "chart_start_bar": 3, "pattern": "pulse"}),
    );
    step(
        &mut app,
        &mut c,
        "voice_chords",
        json!({"track_id": t, "chart": "G | C"}),
    );
    step(
        &mut app,
        &mut c,
        "pedal_from_chart",
        json!({"track_id": t, "chart": "G | C"}),
    );
    step(&mut app, &mut c, "lint_music", json!({"chart": "G | C"}));
    step(
        &mut app,
        &mut c,
        "set_instrument",
        json!({"track_id": u, "kind": "calor"}),
    );
    step(
        &mut app,
        &mut c,
        "remove_items",
        json!({"clip_ids": [copy], "marker_ids": [m]}),
    );
    // Comping and export have tests of their own above; here only what they refuse.
    for (name, args) in [
        (
            "comp_takes",
            json!({"track_id": t, "op": "promote_lane", "lane": 1}),
        ),
        (
            "tidy_takes",
            json!({"track_id": t, "op": "delete_unused_takes"}),
        ),
        ("export_audio", json!({"format": "wav", "file": "x"})),
        ("export_project", json!({"format": "midi", "file": "x"})),
    ] {
        let r = call(&mut app, &mut c, name, args);
        assert!(
            r.is_error,
            "{name}: refused on a track with no takes / with files not allowed"
        );
        called.insert(name.to_string());
    }
    let mine: std::collections::BTreeSet<String> = cantus_ai::tools_project::TOOLS
        .iter()
        .chain(cantus_ui::ai_tools::project_app::TOOLS)
        .map(|t| t.name.to_string())
        .collect();
    let missing: Vec<_> = mine.difference(&called).collect();
    assert!(missing.is_empty(), "never called: {missing:?}");
}

/// The sentences are worded whole in the interface's language — the phrases inside them
/// too (bars, the level of a track, the master, a modulator, a number's decimal comma), the
/// user's names in its quotes, one line (fix review 2 #9: whole sentences asserted).
#[test]
fn sentences_are_worded_whole_in_spanish() {
    use cantus_mcp::AgentHost;
    let mut p = Project::new("es");
    let t = p.make_track(
        "Piano
nuevo",
    );
    let tid = t.id;
    p.tracks.push(t);
    let (mut app, mut c) = app_with(p);
    app.settings.ai.allow_edits = true;
    let m = ok(
        &mut app,
        &mut c,
        "add_modulator",
        json!({"host": tid.0, "kind": "lfo"}),
    )["modulator_id"]
        .as_u64()
        .unwrap();
    cantus_ui::i18n::set_thread_locale(cantus_ui::i18n::Locale::Es);
    let plan = |app: &mut CantusApp, name: &str, args: Value| {
        let def = cantus_ui::ai_tools::find(name).unwrap();
        let cantus_ai::ToolRun::Edit(f) = def.run else {
            panic!("{name}")
        };
        let plan = f(&mut cantus_ai::ToolCx { host: app }, &args).unwrap();
        app.say(&plan.said)
    };
    let cases = [
        ("automate", json!({"target": {"track_id": tid.0, "param": "gain_db"}, "points": [{"at": "1:1", "value": -6}, {"at": "4:1", "value": 0}]}),
            "automatizar el nivel de «Piano nuevo» en los carriles de «Piano nuevo» (compases 1–4, 2 puntos)"),
        ("set_strip", json!({"strip_id": "master", "width": 1.1}), "ajustar anchura del máster"),
        ("set_tempo", json!({"bpm": 116.5}), "poner el tempo a 116,5 BPM"),
        ("set_tempo_point", json!({"at": "9:2.5", "bpm": 90}), "poner un cambio de tempo a 90 BPM en 9:2,5"),
        ("set_modulator", json!({"modulator_id": m, "amount": 0.5}), "ajustar el modulador LFO 1 de «Piano nuevo»"),
        ("remove_items", json!({"modulator_ids": [m]}), "quitar el modulador LFO 1 de «Piano nuevo»"),
        ("automate", json!({"target": {"modulator_id": m}, "lane_track_id": tid.0, "points": [{"at": "2:1", "value": 0.5}]}),
            "automatizar la cantidad del modulador LFO 1 en los carriles de «Piano nuevo» (compás 2, 1 punto)"),
    ];
    for (name, args, want) in cases {
        assert_eq!(plan(&mut app, name, args), want, "{name}");
    }
    cantus_ui::i18n::set_thread_locale(cantus_ui::i18n::Locale::En);
}

/// Fix review 2 #10: a Files card shows the whole path the user approves.
#[test]
fn a_files_card_shows_the_whole_path() {
    use cantus_mcp::AgentHost;
    let deep = std::env::temp_dir()
        .join(format!(
            "cantus-a-folder-with-a-rather-long-name-{}",
            std::process::id()
        ))
        .join("and-another-long-one-under-it");
    std::fs::create_dir_all(&deep).unwrap();
    let (mut app, _c) = app_with(Project::new("p"));
    app.path = Some(deep.join("song.cantus"));
    let def = cantus_ui::ai_tools::find("export_audio").unwrap();
    let cantus_ai::ToolRun::HostSaid { said, .. } = def.run else {
        panic!()
    };
    let s = said(
        &cantus_ai::ToolCx { host: &mut app },
        &json!({"format": "wav", "file": "the-final-mix-for-the-label"}),
    );
    let words = app.say(&s);
    assert!(words.contains("the-final-mix-for-the-label.wav"), "{words}");
    let _ = std::fs::remove_dir_all(deep.parent().unwrap());
}

/// Review #5 for the application's tools: every schema closed to the bottom.
#[test]
fn the_application_tools_schemas_are_closed() {
    fn closed(name: &str, v: &Value, at: &str) {
        match v {
            Value::Object(m) => {
                if m.contains_key("properties") {
                    assert_eq!(
                        m.get("additionalProperties"),
                        Some(&json!(false)),
                        "{name}: {at} is open"
                    );
                }
                for (k, x) in m {
                    closed(name, x, &format!("{at}.{k}"));
                }
            }
            Value::Array(a) => a.iter().for_each(|x| closed(name, x, at)),
            _ => {}
        }
    }
    for t in cantus_ui::ai_tools::project_app::TOOLS {
        closed(t.name, &(t.input_schema)(), "");
    }
}

/// Review #1: a preset replaces a device's settings, so it asks.
#[test]
fn a_preset_is_destructive() {
    assert_eq!(
        cantus_ui::ai_tools::find("apply_preset").unwrap().class,
        cantus_ai::ToolClass::Destructive
    );
}

/// Review #11: an agent neither ends the user's take nor moves the playhead under it.
#[test]
fn the_transport_is_the_users_while_recording() {
    let (mut app, mut c) = app_with(Project::new("rec"));
    app.recorder.start();
    assert!(app.is_recording());
    for action in [
        json!({"action": "stop"}),
        json!({"action": "locate", "at": 8}),
        json!({"action": "play"}),
    ] {
        let r = call(&mut app, &mut c, "transport", action);
        assert!(r.is_error, "{}", r.text);
    }
    assert!(app.is_recording(), "the take goes on");
}

/// Review #3, #4 and mcp's fourth review: an export says the file it writes and whether it
/// replaces one; stems never overwrite without leave; a file that appears after the plan is
/// refused when the worker opens it.
#[test]
fn exports_name_their_file_and_never_overwrite_unasked() {
    let dir = std::env::temp_dir().join(format!("cantus-agent-review-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let mut p = Project::new("review");
    let t = p.make_track("Lead");
    let tid = t.id;
    p.tracks.push(t);
    let (mut app, mut c) = app_with(p);
    app.settings.ai.allow_edits = true;
    app.settings.ai.allow_files = true;
    ok(
        &mut app,
        &mut c,
        "add_notes",
        json!({"track_id": tid.0, "notes": [{"start": 0, "length": 1, "pitch": "C4"}]}),
    );
    app.path = Some(dir.join("song.cantus"));
    let said = |app: &mut CantusApp, args: Value| {
        use cantus_mcp::AgentHost;
        let def = cantus_ui::ai_tools::find("export_audio").unwrap();
        let cantus_ai::ToolRun::HostSaid { said, .. } = def.run else {
            panic!("export_audio says its file")
        };
        let s = said(&cantus_ai::ToolCx { host: app }, &args);
        app.say(&s)
    };
    let s = said(&mut app, json!({"format": "wav", "file": "mix"}));
    assert!(s.contains("mix.wav") && !s.contains("replacing"), "{s}");
    std::fs::write(dir.join("mix.wav"), b"old").unwrap();
    let s = said(
        &mut app,
        json!({"format": "wav", "file": "mix", "overwrite": true}),
    );
    assert!(s.contains("replacing it"), "{s}");
    assert!(
        call(
            &mut app,
            &mut c,
            "export_audio",
            json!({"format": "wav", "file": "mix"})
        )
        .is_error
    );
    // Stems: one file already there is not overwritten without leave.
    let stems = dir.join("stems");
    std::fs::create_dir_all(&stems).unwrap();
    let mut d = app.settings.export.clone();
    d.scope = cantus_export::Scope::Stems;
    d.format = cantus_export::Format::Wav;
    let specs =
        cantus_ui::views::interop::stem_specs(app.project(), d.stems.tracks, d.stems.master);
    let names = cantus_export::stems::output_names(&specs, &d);
    std::fs::write(stems.join(&names[0]), b"old").unwrap();
    let r = call(
        &mut app,
        &mut c,
        "export_audio",
        json!({"format": "wav", "file": "stems", "scope": "stems"}),
    );
    assert!(r.is_error && r.text.contains("overwrite"), "{}", r.text);
    assert_eq!(std::fs::read(stems.join(&names[0])).unwrap(), b"old");
    // A file that appears between the plan and the write: the worker refuses it.
    let def = cantus_ui::ai_tools::find("export_project").unwrap();
    let cantus_ai::ToolRun::Job(prepare) = def.run else {
        panic!()
    };
    let job = prepare(
        &mut cantus_ai::ToolCx { host: &mut app },
        &json!({"format": "midi", "file": "late"}),
    )
    .unwrap();
    std::fs::write(dir.join("late.mid"), b"theirs").unwrap();
    let progress = |_: f32| {};
    let cancelled = || false;
    let r = (job.work)(&cantus_ai::JobCx {
        progress: &progress,
        cancelled: &cancelled,
    });
    assert!(matches!(r, Err(cantus_ai::ToolError::Refused(_))));
    assert_eq!(std::fs::read(dir.join("late.mid")).unwrap(), b"theirs");
    let _ = std::fs::remove_dir_all(&dir);
}

/// Fix review #2: an agent's audio export creates and truncates nothing before the
/// exporter writes. Refused at the start, stopped, or failed — with or without overwrite —
/// the user's file is as it was and no empty file is left; a file that appears during the
/// render is never replaced without leave.
#[test]
fn an_audio_export_loses_nothing_when_it_does_not_finish() {
    let dir = std::env::temp_dir().join(format!("cantus-agent-fail-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let mut p = Project::new("long");
    let t = p.make_track("Lead");
    let tid = t.id;
    p.tracks.push(t);
    let (mut app, mut c) = app_with(p);
    app.settings.ai.allow_edits = true;
    app.settings.ai.allow_files = true;
    // A long song, so a stop or a file put in place arrives while it renders.
    ok(
        &mut app,
        &mut c,
        "add_notes",
        json!({"track_id": tid.0, "notes": [{"start": 0, "length": 1, "pitch": "C4"}, {"start": "300:1", "length": 1, "pitch": "C4"}]}),
    );
    app.path = Some(dir.join("song.cantus"));
    let mix = dir.join("mix.wav");
    // Refused at the start (no time selection): nothing made, nothing touched.
    let r = call(
        &mut app,
        &mut c,
        "export_audio",
        json!({"format": "wav", "file": "mix", "scope": "selection"}),
    );
    assert!(r.is_error, "{}", r.text);
    assert!(!mix.exists(), "no empty file left by a refused start");
    std::fs::write(&mix, b"the user's").unwrap();
    let r = call(
        &mut app,
        &mut c,
        "export_audio",
        json!({"format": "wav", "file": "mix", "scope": "selection", "overwrite": true}),
    );
    assert!(r.is_error);
    assert_eq!(
        std::fs::read(&mix).unwrap(),
        b"the user's",
        "not truncated by a refused start"
    );
    // Stopped, with overwrite: the old file stays whole.
    ok(
        &mut app,
        &mut c,
        "export_audio",
        json!({"format": "wav", "file": "mix", "overwrite": true}),
    );
    cantus_ui::views::export::stop(&mut app);
    cantus_ui::views::export::wait(&mut app);
    assert_eq!(
        std::fs::read(&mix).unwrap(),
        b"the user's",
        "a stop leaves the old file whole"
    );
    // Stopped, without overwrite: no empty file to refuse the next call.
    let fresh = dir.join("fresh.wav");
    ok(
        &mut app,
        &mut c,
        "export_audio",
        json!({"format": "wav", "file": "fresh"}),
    );
    cantus_ui::views::export::stop(&mut app);
    cantus_ui::views::export::wait(&mut app);
    assert!(!fresh.exists(), "no empty file after a stop");
    // A file already there, without leave to overwrite: refused, and that file left as it is.
    // (One that appears while it renders is refused at the commit and left whole: held with
    // no race by cantus-export's `never_replacing_refuses_a_file_that_appeared_and_leaves_it_whole`.)
    std::fs::write(&fresh, b"theirs").unwrap();
    let r = call(
        &mut app,
        &mut c,
        "export_audio",
        json!({"format": "wav", "file": "fresh"}),
    );
    assert!(r.is_error, "{}", r.text);
    cantus_ui::views::export::wait(&mut app);
    assert_eq!(
        std::fs::read(&fresh).unwrap(),
        b"theirs",
        "never replaced without leave"
    );
    assert!(std::fs::read_dir(&dir).unwrap().all(|e| !e
        .unwrap()
        .file_name()
        .to_string_lossy()
        .ends_with(".part")));
    let _ = std::fs::remove_dir_all(&dir);
}

/// Fix review 3 #1: an agent's "never replace" is its own export's: the settings the
/// dialog and the user's exports start from keep none of it.
#[test]
fn an_agents_no_replace_stays_out_of_the_users_settings() {
    let dir = std::env::temp_dir().join(format!("cantus-agent-keep-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let mut p = Project::new("keep");
    let t = p.make_track("Lead");
    let tid = t.id;
    p.tracks.push(t);
    let (mut app, mut c) = app_with(p);
    app.settings.ai.allow_edits = true;
    app.settings.ai.allow_files = true;
    ok(
        &mut app,
        &mut c,
        "add_notes",
        json!({"track_id": tid.0, "notes": [{"start": 0, "length": 1, "pitch": "C4"}]}),
    );
    app.path = Some(dir.join("song.cantus"));
    ok(
        &mut app,
        &mut c,
        "export_audio",
        json!({"format": "wav", "file": "mix"}),
    );
    cantus_ui::views::export::wait(&mut app);
    assert!(
        !app.settings.export.no_replace,
        "the user's next export may replace what they confirm"
    );
    let _ = std::fs::remove_dir_all(&dir);
}
