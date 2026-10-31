//! The mixer, effects, movement, the session, drums, the linter and the chunky writers —
//! and a short song composed only through tools, then undone to nothing.

mod support;

use cantus_ai::ToolError;
use cantus_core::{EffectKind, StripId, TrackId};
use serde_json::json;
use support::*;

#[test]
fn the_mixer_strips_sends_and_effects_on_any_strip() {
    let mut h = host();
    let t = edit_ok(&mut h, "add_track", json!({"kind": "unda"}));
    let (track, strip) = (id(&t, "track_id"), id(&t, "strip_id"));
    let bus = edit_ok(
        &mut h,
        "add_strip",
        json!({"name": "Music bus", "tracks": []}),
    );
    let bus_id = id(&bus, "strip_id");
    edit_ok(
        &mut h,
        "set_strip",
        json!({"strip_id": strip, "output": bus_id, "gain_db": -6, "width": 1.5, "eq": [{"band": 1, "freq_hz": 800, "gain_db": -3, "q": 1.2}]}),
    );
    let s = h.bus.project().strip(StripId(strip)).unwrap();
    assert_eq!((s.output, s.gain_db), (Some(StripId(bus_id)), -6.0));
    assert_eq!(s.eq.bands[1].gain_db, -3.0);
    assert!(
        matches!(
            edit_err(
                &mut h,
                "set_strip",
                json!({"strip_id": bus_id, "output": strip})
            ),
            ToolError::BadArgument { .. }
        ),
        "a loop"
    );
    edit_ok(
        &mut h,
        "set_strip",
        json!({"strip_id": "master", "gain_db": -1, "width": 0.9}),
    );
    assert_eq!(h.bus.project().master.gain_db, -1.0);
    assert!(matches!(
        edit_err(
            &mut h,
            "set_strip",
            json!({"strip_id": "master", "muted": true})
        ),
        ToolError::BadArgument { .. }
    ));
    let rev = edit_ok(&mut h, "add_strip", json!({"name": "Reverb"}));
    let rev_id = id(&rev, "strip_id");
    edit_ok(
        &mut h,
        "set_send",
        json!({"strip_id": strip, "to": rev_id, "level_db": -12}),
    );
    let lvl = h
        .bus
        .project()
        .strip(StripId(strip))
        .unwrap()
        .send_to(StripId(rev_id))
        .unwrap()
        .level;
    assert!((lvl - 0.2512).abs() < 1e-3, "{lvl}");
    edit_ok(
        &mut h,
        "set_send",
        json!({"strip_id": strip, "to": rev_id, "pre_fader": true}),
    );
    // Effects: on a track's strip, on a strip, on the master; set, move, replace, remove.
    let e1 = id(
        &edit_ok(
            &mut h,
            "add_effect",
            json!({"track_id": track, "kind": "aequalis", "params": {"b0.gain": 3}}),
        ),
        "effect_id",
    );
    let e2 = id(
        &edit_ok(
            &mut h,
            "add_effect",
            json!({"strip_id": rev_id, "kind": "spatium", "mix": 1}),
        ),
        "effect_id",
    );
    let e3 = id(
        &edit_ok(
            &mut h,
            "add_effect",
            json!({"strip_id": "master", "kind": "limiter"}),
        ),
        "effect_id",
    );
    // The legacy bridge's call: a track and a delay.
    edit_ok(
        &mut h,
        "add_effect",
        json!({"track_id": track, "kind": "delay"}),
    );
    assert_eq!(
        h.bus.project().master.effects[0].spec.kind,
        EffectKind::Limiter
    );
    edit_ok(
        &mut h,
        "set_effect",
        json!({"effect_id": e1, "params": {"b0.freq": 120}, "bypass": true, "mix": 0.5, "index": 1}),
    );
    let s = h.bus.project().strip(StripId(strip)).unwrap();
    assert_eq!(s.effects[1].id.0, e1);
    assert!(s.effects[1].bypass);
    let e = edit_err(
        &mut h,
        "set_effect",
        json!({"effect_id": e1, "params": {"b0.gain": 99}}),
    );
    assert!(e.to_string().contains("must be"), "{e}");
    edit_ok(
        &mut h,
        "set_effect",
        json!({"effect_id": e1, "reset": ["b0.gain"]}),
    );
    let comp = id(
        &edit_ok(
            &mut h,
            "add_effect",
            json!({"strip_id": strip, "kind": "compressor", "neutral": true}),
        ),
        "effect_id",
    );
    // Keyed from the strip it sends to would be a loop: refused; from another strip, set.
    assert!(matches!(
        edit_err(
            &mut h,
            "set_effect",
            json!({"effect_id": comp, "sidechain_strip_id": rev_id})
        ),
        ToolError::Failed(_)
    ));
    let key = id(
        &edit_ok(&mut h, "add_strip", json!({"name": "Key"})),
        "strip_id",
    );
    edit_ok(
        &mut h,
        "set_effect",
        json!({"effect_id": comp, "sidechain_strip_id": key}),
    );
    let r = edit_ok(
        &mut h,
        "replace_effect",
        json!({"effect_id": e2, "kind": "concentus"}),
    );
    assert_ne!(id(&r, "effect_id"), e2);
    edit_ok(
        &mut h,
        "remove_items",
        json!({"effect_ids": [e3], "sends": [{"strip_id": strip, "to": rev_id}]}),
    );
    assert!(h.bus.project().master.effects.is_empty());
    // A strip still fed is not removed; with its tracks it is.
    assert!(matches!(
        edit_err(&mut h, "remove_items", json!({"strip_ids": [strip]})),
        ToolError::BadArgument { .. }
    ));
    edit_ok(
        &mut h,
        "remove_items",
        json!({"track_ids": [track], "strip_ids": [strip]}),
    );
    assert!(h.bus.project().strip(StripId(strip)).is_none());
}

#[test]
fn automation_and_modulators_in_the_parameters_units() {
    let mut h = host();
    let t = edit_ok(
        &mut h,
        "add_track",
        json!({"kind": "chorda", "name": "Piano"}),
    );
    let (track, strip) = (id(&t, "track_id"), id(&t, "strip_id"));
    let fx = id(
        &edit_ok(
            &mut h,
            "add_effect",
            json!({"strip_id": strip, "kind": "aequalis"}),
        ),
        "effect_id",
    );
    let a = edit_ok(
        &mut h,
        "automate",
        json!({"target": {"effect_id": fx, "param": "b0.freq"},
        "points": [{"at": "1:1", "value": 200, "shape": "curve", "tension": 0.5}, {"at": "3:1", "value": 2000}]}),
    );
    let clip = id(&a, "clip_id");
    let notes = read_ok(&mut h, "get_clip_notes", json!({"clip_id": clip}));
    let v0 = notes["points"][0]["value"].as_f64().unwrap();
    assert!((v0 - 200.0).abs() < 0.5, "the value in Hz comes back: {v0}");
    // Drawn again over part of the lane: into the same clip.
    let b = edit_ok(
        &mut h,
        "automate",
        json!({"target": {"effect_id": fx, "param": "b0.freq"}, "points": [{"at": "2:1", "value": 500}]}),
    );
    assert_eq!(id(&b, "clip_id"), clip);
    assert_eq!(b["clip_created"], false);
    edit_ok(
        &mut h,
        "automate",
        json!({"target": {"track_id": track, "param": "gain_db"}, "points": [{"at": 0, "value": -12}, {"at": 8, "value": 0}]}),
    );
    edit_ok(
        &mut h,
        "automate",
        json!({"target": {"strip_id": "master", "param": "gain_db"}, "lane_track_id": track, "points": [{"at": 0, "value": 0, "shape": "hold"}]}),
    );
    assert!(matches!(
        edit_err(
            &mut h,
            "automate",
            json!({"target": {"track_id": track, "param": "gain_db"}, "points": [{"at": 0, "value": 40}]})
        ),
        ToolError::BadArgument { .. }
    ));
    assert!(
        matches!(
            edit_err(
                &mut h,
                "automate",
                json!({"target": {"strip_id": "master", "param": "gain_db"}, "points": [{"at": 0, "value": 0}]})
            ),
            ToolError::BadArgument { .. }
        ),
        "the master needs a lane track"
    );
    let m = edit_ok(
        &mut h,
        "add_modulator",
        json!({"host": track, "kind": "lfo", "rate": "1/1", "shape": "triangle",
        "routes": [{"target": {"effect_id": fx, "param": "b0.gain"}, "depth": 0.25}]}),
    );
    let mid = id(&m, "modulator_id");
    edit_ok(
        &mut h,
        "set_modulator",
        json!({"modulator_id": mid, "rate": 2.5, "amount": 0.5}),
    );
    edit_ok(
        &mut h,
        "add_modulator",
        json!({"host": format!("strip:{strip}"), "kind": "follower", "listen_strip_id": strip, "attack_ms": 2, "release_ms": 140,
        "routes": [{"target": {"strip_id": strip, "param": "gain_db"}, "depth": -0.12}]}),
    );
    edit_ok(
        &mut h,
        "add_modulator",
        json!({"host": "master", "kind": "step", "steps": [1, 0, -1, 0], "step_rate": "1/16"}),
    );
    edit_ok(
        &mut h,
        "add_modulator",
        json!({"host": track, "kind": "random", "rate": "1/8", "smooth": true, "seed": 116}),
    );
    edit_ok(
        &mut h,
        "add_modulator",
        json!({"host": track, "kind": "envelope", "attack_ms": 5, "sustain": 0.3}),
    );
    assert!(matches!(
        edit_err(
            &mut h,
            "set_modulator",
            json!({"modulator_id": mid, "rate": "1/5"})
        ),
        ToolError::BadArgument { .. }
    ));
    edit_ok(&mut h, "remove_items", json!({"modulator_ids": [mid]}));
}

#[test]
fn the_session_scenes_slots_launch_and_follow() {
    let mut h = host();
    let t = id(
        &edit_ok(&mut h, "add_track", json!({"kind": "unda"})),
        "track_id",
    );
    let s = edit_ok(
        &mut h,
        "set_session",
        json!({"add_scene": {"name": "Verse", "tempo_bpm": 120},
        "slot": {"track_id": t, "scene_id": 0, "length_beats": 4, "notes": [{"start_beats": 0, "length_beats": 1, "pitch": "C4"}],
            "launch": {"mode": "gate", "quantize": "1/4", "legato": true}, "follow": {"a": "next", "after_bars": 2}}}),
    );
    let scene = id(&s, "scene_id");
    let sc = h
        .bus
        .project()
        .track(TrackId(t))
        .unwrap()
        .slot_clip(cantus_core::session::SceneId(scene))
        .unwrap()
        .clone();
    assert_eq!(sc.clip.notes.len(), 1);
    assert_eq!(sc.launch.mode, cantus_core::session::LaunchMode::Gate);
    assert!(sc.follow.on);
    edit_ok(
        &mut h,
        "set_session",
        json!({"scene": {"scene_id": scene, "name": "Chorus", "tempo_bpm": null, "time_signature": "3/4"}}),
    );
    edit_ok(
        &mut h,
        "set_session",
        json!({"slot": {"track_id": t, "scene_id": scene, "launch": {"mode": "toggle"}}}),
    );
    assert!(
        matches!(
            edit_err(
                &mut h,
                "set_session",
                json!({"slot": {"track_id": t, "scene_id": scene, "length_beats": 4}})
            ),
            ToolError::BadArgument { .. }
        ),
        "an occupied slot"
    );
    let arr = id(
        &edit_ok(
            &mut h,
            "add_notes",
            json!({"track_id": t, "notes": [{"start": 0, "length": 1, "pitch": 60}]}),
        ),
        "clip_id",
    );
    let s2 = id(
        &edit_ok(&mut h, "set_session", json!({"add_scene": {"name": "B"}})),
        "scene_id",
    );
    edit_ok(
        &mut h,
        "set_session",
        json!({"slot": {"track_id": t, "scene_id": s2, "from_clip_id": arr}}),
    );
    edit_ok(
        &mut h,
        "remove_items",
        json!({"session_slots": [{"track_id": t, "scene_id": s2}], "scene_ids": [scene]}),
    );
    assert_eq!(h.bus.project().session.scenes.len(), 1);
}

#[test]
fn drum_patterns_by_pad_name_and_general_midi() {
    let mut h = host();
    let d = id(
        &edit_ok(
            &mut h,
            "add_track",
            json!({"kind": "pulsus", "name": "Drums"}),
        ),
        "track_id",
    );
    let r = edit_ok(
        &mut h,
        "write_drum_pattern",
        json!({"track_id": d, "at": "1:1", "bars": 2, "step": "1/16", "swing": 0.5, "rows": [
        {"pad": "kick", "steps": "x...x...x...x..."},
        {"pad": 38, "steps": "....X.......X..."},
        {"pad": "closed_hat", "steps": "x.o.x.o.x.o.x.o.", "velocity": 90}]}),
    );
    assert_eq!(r["hits"], 8 + 4 + 16);
    let notes = &h.bus.project().track(TrackId(d)).unwrap().clips[0].notes;
    assert!(notes.iter().any(|n| n.pitch == 36) && notes.iter().any(|n| n.pitch == 42));
    // Every hit lands on a pad the kit has.
    let lint = read_ok(&mut h, "lint_music", json!({"track_ids": [d]}));
    assert_eq!(lint["errors"], 0, "{lint}");
    assert!(matches!(
        edit_err(
            &mut h,
            "write_drum_pattern",
            json!({"track_id": d, "at": 0, "rows": [{"pad": "gong", "steps": "x"}]})
        ),
        ToolError::BadArgument { .. }
    ));
    assert!(matches!(
        edit_err(
            &mut h,
            "write_drum_pattern",
            json!({"track_id": d, "at": 0, "rows": [{"pad": "kick", "steps": "x?x"}]})
        ),
        ToolError::BadArgument { .. }
    ));
}

/// Round 34 (`font_coverage`): an agent may write a chart in music signs; what the tools say
/// of its chords — the lint's messages, an error's quote — is spelled in the fonts' glyphs.
#[test]
fn a_chart_in_music_signs_is_read_and_spoken_in_plain_letters() {
    let mut h = host();
    let t = id(
        &edit_ok(
            &mut h,
            "add_track",
            json!({"kind": "chorda", "name": "Piano"}),
        ),
        "track_id",
    );
    edit_ok(
        &mut h,
        "add_notes",
        json!({"track_id": t, "notes": [{"start": 1, "length": 3, "pitch": "Ab3"}]}),
    );
    let r = read_ok(
        &mut h,
        "lint_music",
        json!({"chart": "Fm7 E\u{266d}/G | C\u{394}"}),
    );
    let text = r.to_string();
    assert!(text.contains("Eb/G"), "{text}");
    assert!(
        !text.contains(['\u{266d}', '\u{266f}', '\u{394}']),
        "{text}"
    );
    let e = format!(
        "{:?}",
        read(
            &mut h,
            "lint_music",
            json!({"chart": "E\u{266d}\u{394}x\u{266f}"})
        )
    );
    assert!(e.contains("Ebmaj7x#"), "{e}");
    assert!(!e.contains(['\u{266d}', '\u{266f}', '\u{394}']), "{e}");
}

/// The round-28 demo's mistake, made through the tools and found by `lint_music`.
#[test]
fn lint_music_finds_the_demo_fm7_held_under_eb_over_g() {
    let mut h = host();
    let t = id(
        &edit_ok(
            &mut h,
            "add_track",
            json!({"kind": "chorda", "name": "Piano"}),
        ),
        "track_id",
    );
    edit_ok(
        &mut h,
        "add_notes",
        json!({"track_id": t, "notes": [
        {"start": 1, "length": 3, "pitch": "F3"}, {"start": 1, "length": 3, "pitch": "Ab3"},
        {"start": 1, "length": 3, "pitch": "C4"}, {"start": 1, "length": 3, "pitch": "Eb4"},
        {"start": 1, "length": 1, "pitch": "Eb4"}]}),
    );
    let r = read_ok(&mut h, "lint_music", json!({"chart": "Fm7 Eb/G"}));
    let findings = r["findings"].as_array().unwrap();
    let ab = findings
        .iter()
        .find(|f| f["issue"] == "rings_into_change" && f["severity"] == "error")
        .expect("the A♭ rings into E♭/G");
    let msg = ab["message"].as_str().unwrap();
    assert!(
        msg.contains("Ab3") && msg.contains("Eb/G") && msg.contains("against its G"),
        "{msg}"
    );
    assert_eq!(ab["at"]["bar_beat"], "1:3");
    assert!(
        findings.iter().any(|f| f["issue"] == "doubled"),
        "the E♭ written twice"
    );
    // A note Chorda cannot play.
    edit_ok(
        &mut h,
        "add_notes",
        json!({"track_id": t, "notes": [{"start": 8, "length": 1, "pitch": 12}]}),
    );
    let r = read_ok(
        &mut h,
        "lint_music",
        json!({"track_ids": [t], "from": "3:1"}),
    );
    assert_eq!(r["findings"][0]["issue"], "out_of_range");
    assert!(matches!(
        read(&mut h, "lint_music", json!({"chart": "Xm7"})),
        Err(ToolError::BadArgument { .. })
    ));
}

/// A short piece made only through tools: tempo and key, a pedalled piano voicing a chart,
/// a bass line, an arpeggio, drums, an automated sweep, a reverb send, a modulator — each
/// one undo step, the linter clean, and undoing it all gives back the empty project.
#[test]
fn compose_a_song_only_through_tools_then_undo_it_all() {
    let mut h = host();
    let empty = bare(h.bus.project());
    edit_ok(&mut h, "set_tempo", json!({"bpm": 96}));
    edit_ok(
        &mut h,
        "set_song",
        json!({"name": "Through tools", "key": {"root": "F", "scale": "minor"}}),
    );
    let chart = "Fm9 | Dbmaj7 | Abadd9 | Eb Eb/G | Bbm7 | Ab/C | Dbmaj7 | Eb7sus4 Eb";
    let piano = id(
        &edit_ok(
            &mut h,
            "add_track",
            json!({"kind": "chorda", "name": "Piano"}),
        ),
        "track_id",
    );
    let bass = id(
        &edit_ok(
            &mut h,
            "add_track",
            json!({"kind": "gravis", "name": "Bass"}),
        ),
        "track_id",
    );
    let pluck = id(
        &edit_ok(
            &mut h,
            "add_track",
            json!({"kind": "unda", "name": "Pluck"}),
        ),
        "track_id",
    );
    let drums = id(
        &edit_ok(
            &mut h,
            "add_track",
            json!({"kind": "pulsus", "name": "Drums"}),
        ),
        "track_id",
    );
    let v = edit_ok(
        &mut h,
        "voice_chords",
        json!({"track_id": piano, "chart": chart, "voices": 4, "range": {"low": "F3", "high": "F5"}}),
    );
    assert!(v["notes"].as_u64().unwrap() >= 8 * 3);
    edit_ok(
        &mut h,
        "pedal_from_chart",
        json!({"track_id": piano, "chart": chart}),
    );
    edit_ok(
        &mut h,
        "write_part",
        json!({"track_id": bass, "chart": chart, "pattern": "bass", "bass_style": "root_fifth", "rhythm": "x..x..x.", "step": "1/8"}),
    );
    edit_ok(
        &mut h,
        "write_part",
        json!({"track_id": pluck, "chart": chart, "pattern": "arpeggio", "direction": "up_down", "octaves": 2,
        "rate": "1/16", "range": {"low": "C4", "high": "C6"}, "gate": 0.6, "swing": 0.3, "swing_grid": "1/16", "humanize": {"seed": 9}}),
    );
    edit_ok(
        &mut h,
        "write_drum_pattern",
        json!({"track_id": drums, "at": "1:1", "bars": 8, "rows": [
        {"pad": "kick", "steps": "x.......x......."}, {"pad": "snare", "steps": "....x.......x..."}, {"pad": "closed_hat", "steps": "x.x.x.x.x.x.x.x."}]}),
    );
    let pstrip = h
        .bus
        .project()
        .track(TrackId(pluck))
        .unwrap()
        .strip
        .unwrap()
        .0;
    let eq = id(
        &edit_ok(
            &mut h,
            "add_effect",
            json!({"track_id": pluck, "kind": "aequalis", "neutral": true}),
        ),
        "effect_id",
    );
    edit_ok(
        &mut h,
        "automate",
        json!({"target": {"effect_id": eq, "param": "b23.freq"}, "points": [{"at": "1:1", "value": 400}, {"at": "8:1", "value": 12000, "tension": 0.6}]}),
    );
    let rev = id(
        &edit_ok(&mut h, "add_strip", json!({"name": "Hall"})),
        "strip_id",
    );
    edit_ok(
        &mut h,
        "add_effect",
        json!({"strip_id": rev, "kind": "spatium"}),
    );
    edit_ok(
        &mut h,
        "set_send",
        json!({"strip_id": pstrip, "to": rev, "level_db": -10}),
    );
    edit_ok(
        &mut h,
        "add_modulator",
        json!({"host": pluck, "kind": "lfo", "rate": "1/1", "routes": [{"target": {"track_id": pluck, "param": "pan"}, "depth": 0.3}]}),
    );
    edit_ok(
        &mut h,
        "set_strip",
        json!({"strip_id": "master", "gain_db": -2}),
    );
    edit_ok(
        &mut h,
        "add_effect",
        json!({"strip_id": "master", "kind": "culmen"}),
    );
    // The music is clean: nothing rings into a chord change, nothing doubled or unplayable.
    let lint = read_ok(&mut h, "lint_music", json!({"chart": chart, "info": false}));
    let bad: Vec<_> = lint["findings"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|f| f["severity"] == "error")
        .collect();
    assert!(bad.is_empty(), "{bad:?}");
    let p = read_ok(&mut h, "get_project", json!({}));
    assert!(p["end"]["beats"].as_f64().unwrap() <= 32.25, "{}", p["end"]);
    // Undo everything: the empty project, exactly.
    while h.bus.can_undo() {
        h.bus.undo();
    }
    assert_eq!(bare(h.bus.project()), empty);
}

#[test]
fn resources_and_prompts_read_the_project() {
    use cantus_ai::tools_project::{PROMPTS, RESOURCES, RESOURCE_TEMPLATES};
    use cantus_ai::{ResourceBody, ToolCx};
    let mut h = host();
    let t = id(
        &edit_ok(&mut h, "add_track", json!({"kind": "unda", "name": "Lead"})),
        "track_id",
    );
    let c = id(
        &edit_ok(
            &mut h,
            "add_notes",
            json!({"track_id": t, "notes": [{"start": 0, "length": 1, "pitch": 60}]}),
        ),
        "clip_id",
    );
    let cx = ToolCx { host: &mut h };
    for r in RESOURCES {
        let ResourceBody::Text(s) = (r.read)(&cx).unwrap() else {
            panic!("{} is text", r.uri)
        };
        let v: serde_json::Value = serde_json::from_str(&s).unwrap();
        assert!(v.is_object(), "{}", r.uri);
    }
    let vars = |id: u64| {
        [("id".to_string(), id.to_string())]
            .into_iter()
            .collect::<cantus_ai::TemplateVars>()
    };
    let track = RESOURCE_TEMPLATES
        .iter()
        .find(|r| r.uri_template == "cantus://tracks/{id}")
        .unwrap();
    let ResourceBody::Text(s) = (track.read)(&cx, &vars(t)).unwrap() else {
        panic!()
    };
    assert!(s.contains("\"Lead\""));
    assert!(matches!(
        (track.read)(&cx, &vars(999)),
        Err(ToolError::NotFound(_))
    ));
    let notes = RESOURCE_TEMPLATES
        .iter()
        .find(|r| r.uri_template == "cantus://clips/{id}/notes")
        .unwrap();
    let ResourceBody::Text(s) = (notes.read)(&cx, &vars(c)).unwrap() else {
        panic!()
    };
    assert!(s.contains("\"C4\""));
    for p in PROMPTS {
        let args = json!({"track": "Lead", "bars": "8", "style": "lo-fi", "unit": "Fluctus"});
        let m = (p.get)(&cx, &args).unwrap();
        assert!(
            !m.is_empty() && m[0].said.key.starts_with("ai.prompt."),
            "{}",
            p.name
        );
    }
}
