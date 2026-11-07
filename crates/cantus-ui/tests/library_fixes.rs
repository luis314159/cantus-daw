//! Round 37's fixes to the preset library's agent tools (ADR 0101), through the app on the
//! boot path (`CantusApp::assemble`, whose MCP server the frame serves) and the manual
//! engine's processor:
//!
//! 8. `find_presets` and `audition_preset` answer in English and leave the window in its
//!    own language (they switched a Spanish window to English for good);
//! 9. every preset `find_presets` names, for every unit, is one `apply_preset` loads (it
//!    named sixteen kits and loaded two);
//! 10. the retired Reverb is offered to no agent: `reverb` makes a Spatium, on a strip and
//!     on a pad's chain;
//! 11. `audition_preset` is a job — the window never waits for its render — and what it
//!     plays is the browser's audition, so Stop stops it.
//!
//! Its own binary: it points `CANTUS_DATA_DIR` at a temporary folder and boots in Spanish.

use cantus_core::*;
use cantus_engine::{EngineHandle, Processor};
use cantus_mcp::client::{TestClient, ToolCall};
use cantus_mcp::Answer;
use cantus_ui::i18n::{self, Locale};
use cantus_ui::CantusApp;
use serde_json::{json, Value};
use std::time::{Duration, Instant};

fn data() -> &'static std::path::Path {
    static DIR: std::sync::OnceLock<tempfile::TempDir> = std::sync::OnceLock::new();
    DIR.get_or_init(|| {
        let d = tempfile::tempdir().unwrap();
        std::env::set_var("CANTUS_DATA_DIR", d.path());
        d
    })
    .path()
}

/// One kit track, one track per native instrument, and a strip holding one insert of every
/// effect offered.
fn devices() -> Project {
    let mut p = Project::new("Devices");
    let mut kit = p.make_track("Drums");
    kit.instrument = InstrumentSpec::Pulsus(PulsusParams::studio());
    p.tracks.push(kit);
    for k in InstrumentKind::ALL {
        let mut t = p.make_track(format!("{k:?}"));
        t.instrument = InstrumentSpec::Native(NativeSpec::new(k));
        p.tracks.push(t);
    }
    let mut s = p.make_strip("Effects");
    for k in EffectKind::ALL {
        if k != EffectKind::External && k.offered() {
            let e = p.make_effect(k);
            s.effects.push(e);
        }
    }
    p.strips.push(s);
    p
}

fn boot(p: Project, locale: Locale) -> (CantusApp, Processor, TestClient) {
    let files = cantus_ui::app::Files::read(data(), Some(locale));
    let ctx = egui::Context::default();
    let (engine, proc) = EngineHandle::manual(48_000.0);
    let app = CantusApp::assemble(&ctx, files, Some(p), engine, None, None, None, false);
    let client = TestClient::legacy(app.ai.server().unwrap().clone(), "2025-11-25");
    (app, proc, client)
}

/// A call whose every question is answered Allow, as a user at the card would; each pump
/// is one frame's work (the server's pump and the audition's poll), timed.
fn call_timed(
    app: &mut CantusApp,
    c: &mut TestClient,
    name: &str,
    args: Value,
    longest: &mut Duration,
) -> ToolCall {
    loop {
        let r = c.call_tool(name, args.clone(), &mut || {
            let t0 = Instant::now();
            cantus_ui::ai_mcp::pump(app);
            app.poll_preset_audition();
            if let Some(agents) = app.ai.agents_mut() {
                let ids: Vec<u64> = agents.asks().map(|x| x.id).collect();
                for id in ids {
                    agents.answer(id, Answer::Allow);
                }
            }
            *longest = (*longest).max(t0.elapsed());
        });
        // The server limits an agent's mutating calls, and a fast machine meets the limit in
        // a loop of applies: a polite client waits the time it is told and asks again.
        const BUSY: &str = "too many calls: wait ";
        let wait = r
            .text
            .find(BUSY)
            .and_then(|at| r.text[at + BUSY.len()..].split(' ').next())
            .and_then(|ms| ms.parse::<u64>().ok());
        match wait {
            Some(ms) if r.is_error => std::thread::sleep(Duration::from_millis(ms + 1)),
            _ => return r,
        }
    }
}

fn call(app: &mut CantusApp, c: &mut TestClient, name: &str, args: Value) -> ToolCall {
    let mut d = Duration::ZERO;
    call_timed(app, c, name, args, &mut d)
}

fn ok(app: &mut CantusApp, c: &mut TestClient, name: &str, args: Value) -> Value {
    let r = call(app, c, name, args.clone());
    assert!(!r.is_error, "{name}({args}): {}", r.text);
    r.structured.unwrap_or(Value::Null)
}

/// A job's answer: waited on through `wait_job` when the tool answered with a handle.
fn finished(app: &mut CantusApp, c: &mut TestClient, mut v: Value, what: &str) -> Value {
    for _ in 0..60 {
        let Some(job) = v.get("job").and_then(Value::as_str).map(str::to_string) else {
            return v;
        };
        if v["state"] == "done" {
            return v["result"].clone();
        }
        v = ok(app, c, "wait_job", json!({ "job": job, "timeout_s": 30 }));
    }
    panic!("{what} never finished");
}

fn names(v: &Value) -> Vec<(String, String)> {
    v["presets"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| {
            (
                p["unit"].as_str().unwrap().to_string(),
                p["name"].as_str().unwrap().to_string(),
            )
        })
        .collect()
}

/// A unit's id as the agents' tools name it.
fn unit_id(u: cantus_ui::preset_catalog::Unit) -> String {
    use cantus_ai::tools_project::args;
    use cantus_ui::preset_catalog::Unit;
    match u {
        Unit::Instrument(k) => args::native_id(k).to_string(),
        Unit::Effect(k) => args::effect_names(k).0.to_string(),
        Unit::Kit => "pulsus".to_string(),
    }
}

// ---- 8 and 11: the window's language, the audition as a job, Stop ---------------------------

#[test]
fn the_audition_is_a_job_stop_stops_it_and_the_window_keeps_its_language() {
    let _ = data();
    let (mut app, mut proc, mut c) = boot(devices(), Locale::Es);
    assert_eq!(i18n::locale(), Locale::Es, "booted in Spanish");
    let spanish = i18n::t("padout.chain");
    // find_presets answers in English…
    let found = ok(
        &mut app,
        &mut c,
        "find_presets",
        json!({"unit": "unda", "limit": 3}),
    );
    let first = names(&found)[0].1.clone();
    let key = cantus_ui::preset_catalog::entries(
        cantus_ui::preset_catalog::Unit::Instrument(InstrumentKind::Wavetable),
        false,
    )[0]
    .id
    .clone();
    assert_eq!(first, i18n::t_in(Locale::En, &key), "the answer is English");
    // …and the window is still Spanish.
    assert_eq!(
        i18n::locale(),
        Locale::Es,
        "find_presets left the window Spanish"
    );
    assert_eq!(i18n::t("padout.chain"), spanish);
    // Stop pressed while the render runs: the window was free to take it, and the job
    // answers that it was stopped.
    let r = c.call_tool(
        "audition_preset",
        json!({"unit": "unda", "preset": first}),
        &mut || {
            cantus_ui::ai_mcp::pump(&mut app);
            if app.presets.audition.rendering() {
                app.stop_preset_audition();
            }
            app.poll_preset_audition();
        },
    );
    assert!(!r.is_error, "{}", r.text);
    let v = finished(
        &mut app,
        &mut c,
        r.structured.unwrap(),
        "the stopped audition",
    );
    assert_eq!(v["stopped"], true, "{v}");
    assert!(app.presets.audition.heard.is_none() && app.presets.audition.until.is_none());
    assert_eq!(
        i18n::locale(),
        Locale::Es,
        "audition_preset left the window Spanish"
    );
    // Played through: the answer measures it, and the browser's audition holds it — no
    // frame waited for its render.
    let mut longest = Duration::ZERO;
    let r = call_timed(
        &mut app,
        &mut c,
        "audition_preset",
        json!({"unit": "unda", "preset": first}),
        &mut longest,
    );
    assert!(!r.is_error, "{}", r.text);
    let v = finished(&mut app, &mut c, r.structured.unwrap(), "the audition");
    assert!(v["loudness_lufs"].as_f64().unwrap() < -5.0, "{v}");
    assert_eq!(v["played"], true);
    assert_eq!(v["changed_the_project"], false);
    let start = Instant::now();
    while app.presets.audition.until.is_none() && start.elapsed() < Duration::from_secs(10) {
        app.poll_preset_audition();
        std::thread::sleep(Duration::from_millis(5));
    }
    assert_eq!(app.presets.audition.heard.as_deref(), Some(key.as_str()));
    assert!(app.presets.audition.until.is_some(), "heard until its end");
    assert!(
        longest < Duration::from_millis(500),
        "a frame waited {longest:?}"
    );
    // Heard from the engine, then Stop: a fade, then silence.
    let block = |proc: &mut Processor| {
        let mut buf = vec![0f32; 1024];
        proc.process(&mut buf, 2);
        buf.iter().fold(0f32, |m, x| m.max(x.abs()))
    };
    let heard = (0..40).map(|_| block(&mut proc)).fold(0f32, f32::max);
    assert!(heard > 1e-3, "the audition plays: {heard}");
    app.stop_preset_audition();
    for _ in 0..8 {
        block(&mut proc);
    }
    let after = (0..20).map(|_| block(&mut proc)).fold(0f32, f32::max);
    assert!(after < 1e-6, "Stop stopped it: {after}");
    assert_eq!(i18n::locale(), Locale::Es);
}

// ---- 9 and 10: every preset found loads; no retired Reverb -----------------------------------

#[test]
fn every_preset_found_loads_and_reverb_is_spatium() {
    let _ = data();
    i18n::set_thread_locale(Locale::En);
    let (mut app, _proc, mut c) = boot(devices(), Locale::En);
    let p = app.project().clone();
    let kit = p.tracks[0].id;
    let fx = p
        .strips
        .iter()
        .find(|s| s.name == "Effects")
        .unwrap()
        .clone();
    for u in cantus_ui::preset_catalog::units() {
        use cantus_ui::preset_catalog::Unit;
        if matches!(u, Unit::Effect(k) if !k.offered()) {
            continue;
        }
        let id = unit_id(u);
        let found = names(&ok(
            &mut app,
            &mut c,
            "find_presets",
            json!({"unit": id, "limit": 500}),
        ));
        let listed: Vec<String> = ok(&mut app, &mut c, "list_presets", json!({"unit": id}))
            ["presets"]
            .as_array()
            .unwrap()
            .iter()
            .map(|x| x["name"].as_str().unwrap().to_lowercase())
            .collect();
        for (_, name) in &found {
            assert!(
                listed.contains(&name.to_lowercase()),
                "{id}: find_presets names {name:?}, apply_preset does not know it"
            );
        }
        // Loaded for real: every kit, and each unit's last preset.
        let device = match u {
            Unit::Kit => json!({"track_id": kit.0}),
            Unit::Instrument(k) => {
                let t = p
                    .tracks
                    .iter()
                    .find(|t| t.instrument.native().is_some_and(|n| n.kind == k))
                    .unwrap();
                json!({"track_id": t.id.0})
            }
            Unit::Effect(k) => {
                let e = fx.effects.iter().find(|e| e.spec.kind == k).unwrap();
                json!({"effect_id": e.id.0})
            }
        };
        let load: Vec<&String> = match u {
            Unit::Kit => found.iter().map(|(_, n)| n).collect(),
            _ => found.last().map(|(_, n)| n).into_iter().collect(),
        };
        for name in load {
            let mut a = device.clone();
            a["preset"] = json!(name);
            ok(&mut app, &mut c, "apply_preset", a);
        }
    }
    let kits = PulsusParams::factory().len();
    assert!(kits >= 16, "{kits} factory kits");
    // The two old names still load.
    for name in ["Default kit", "Studio kit"] {
        ok(
            &mut app,
            &mut c,
            "apply_preset",
            json!({"track_id": kit.0, "preset": name}),
        );
    }
    // The name reverb makes a Spatium now: on a strip…
    let on = app.project().track(kit).unwrap().strip.unwrap();
    let v = ok(
        &mut app,
        &mut c,
        "add_effect",
        json!({"strip_id": on.0, "kind": "reverb"}),
    );
    let e = EffectId(v["effect_id"].as_u64().unwrap());
    assert_eq!(
        app.project().effect(on, e).unwrap().spec.kind,
        EffectKind::Space
    );
    // …and on a pad's chain.
    ok(
        &mut app,
        &mut c,
        "set_pad_chain",
        json!({"track_id": kit.0, "pad": 38, "add": "reverb"}),
    );
    let k = app
        .project()
        .track(kit)
        .unwrap()
        .instrument
        .pulsus()
        .unwrap()
        .clone();
    let snare = k.pads.iter().find(|x| x.pitch == 38).unwrap();
    let chain = padout::chain_effects(app.project(), snare);
    assert_eq!(chain.last().unwrap().spec.kind, EffectKind::Space);
    // No schema offers the retired one.
    let list = c
        .request("tools/list", json!({}), &mut || {
            cantus_ui::ai_mcp::pump(&mut app)
        })
        .unwrap();
    assert!(
        !list.to_string().contains("reverb (simple)"),
        "the old Reverb described"
    );
    let units = ok(&mut app, &mut c, "list_units", json!({}));
    assert!(
        !units["units"]
            .as_array()
            .unwrap()
            .iter()
            .any(|u| u["id"] == "reverb"),
        "list_units offers the retired Reverb"
    );
}

// ---- The second fixes review: an old project's Reverb keeps a name of its own ---------------

/// An old project's Reverb is reported as `reverb_legacy`, and that name describes it — its
/// own keys, which `set_effect` takes on it — while `reverb` describes Spatium, which a new
/// insert by that name is.
#[test]
fn an_old_projects_reverb_answers_to_its_own_name() {
    let _ = data();
    i18n::set_thread_locale(Locale::En);
    let mut p = Project::new("Old");
    let t = p.make_track("Pad");
    let strip = t.strip.unwrap();
    p.tracks.push(t);
    let old = p.make_effect(EffectKind::Reverb);
    let old_id = old.id;
    p.strip_mut(strip).unwrap().effects.push(old);
    let (mut app, _proc, mut c) = boot(p, Locale::En);
    let m = ok(&mut app, &mut c, "get_mixer", json!({}));
    let kinds: Vec<String> = m["strips"]
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|s| s["effects"].as_array().cloned().unwrap_or_default())
        .filter(|e| e["effect_id"] == old_id.0 || e["id"] == old_id.0)
        .map(|e| e["kind"].as_str().unwrap().to_string())
        .collect();
    assert_eq!(kinds, vec!["reverb_legacy".to_string()], "{m}");
    // Its own description, its own keys…
    let unit = ok(&mut app, &mut c, "get_unit", json!({"id": "reverb_legacy"}));
    let keys: Vec<String> = unit["params"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r["key"].as_str().unwrap().to_string())
        .collect();
    let want: Vec<String> = EffectKind::Reverb
        .params()
        .iter()
        .map(|i| i.key.to_string())
        .collect();
    assert_eq!(keys, want, "get_unit(reverb_legacy) is the old Reverb");
    ok(
        &mut app,
        &mut c,
        "list_presets",
        json!({"unit": "reverb_legacy"}),
    );
    // …which set_effect takes on it.
    let info = &EffectKind::Reverb.params()[0];
    let mut params = serde_json::Map::new();
    params.insert(info.key.to_string(), json!(info.default));
    ok(
        &mut app,
        &mut c,
        "set_effect",
        json!({"effect_id": old_id.0, "params": params}),
    );
    // And `reverb` is Spatium.
    let sp = ok(&mut app, &mut c, "get_unit", json!({"id": "reverb"}));
    assert_eq!(sp["id"], "spatium", "{sp}");
}
