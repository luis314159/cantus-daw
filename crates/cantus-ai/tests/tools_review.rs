//! The first review of the project's tools (round 34): each finding held by a test that
//! fails without its fix.

mod support;

use cantus_ai::bridge::BareHost;
use cantus_ai::tools_project::TOOLS;
use cantus_ai::{Said, SaidArg, ToolClass, ToolError};
use serde_json::{json, Value};
use support::*;

/// Every object a schema declares properties for is closed: an unknown key is refused
/// wherever one could be written (review #5 — the server checks against it).
pub fn assert_closed(name: &str, v: &Value, at: &str) {
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
                assert_closed(name, x, &format!("{at}.{k}"));
            }
        }
        Value::Array(a) => a
            .iter()
            .enumerate()
            .for_each(|(i, x)| assert_closed(name, x, &format!("{at}[{i}]"))),
        _ => {}
    }
}

#[test]
fn every_schema_is_closed_to_the_bottom() {
    for t in TOOLS {
        assert_closed(t.name, &(t.input_schema)(), "");
    }
}

fn piano(h: &mut BareHost) -> u64 {
    id(
        &edit_ok(h, "add_track", json!({"kind": "chorda", "name": "Piano"})),
        "track_id",
    )
}

/// #2: automation never silently draws over the user's points.
#[test]
fn automation_over_points_is_refused_and_redrawn_only_when_asked() {
    let mut h = host();
    let t = piano(&mut h);
    let target = json!({"track_id": t, "param": "gain_db"});
    edit_ok(
        &mut h,
        "automate",
        json!({"target": target, "points": [{"at": "1:1", "value": -12}, {"at": "3:1", "value": 0}]}),
    );
    let e = edit_err(
        &mut h,
        "automate",
        json!({"target": target, "points": [{"at": "2:1", "value": -3}, {"at": "3:1", "value": -6}]}),
    );
    assert!(
        matches!(e, ToolError::Refused(_)) && e.to_string().contains("replace_automation"),
        "{e}"
    );
    assert_eq!(tool("replace_automation").class, ToolClass::Destructive);
    edit_ok(
        &mut h,
        "replace_automation",
        json!({"target": target, "points": [{"at": "2:1", "value": -3}, {"at": "3:1", "value": -6}]}),
    );
    let pts = h.bus.project().tracks[0].clips[0]
        .automation
        .as_ref()
        .unwrap()
        .points
        .len();
    assert_eq!(pts, 3, "the first point kept, the span redrawn");
    // A span with no points is drawn without asking.
    edit_ok(
        &mut h,
        "automate",
        json!({"target": target, "points": [{"at": "4:3", "value": -1}]}),
    );
    // The pedal from a chart, twice over the same bars: refused the second time.
    edit_ok(
        &mut h,
        "pedal_from_chart",
        json!({"track_id": t, "chart": "C | F"}),
    );
    assert!(matches!(
        edit_err(
            &mut h,
            "pedal_from_chart",
            json!({"track_id": t, "chart": "C | F"})
        ),
        ToolError::Refused(_)
    ));
}

/// #5: the limits a schema states hold in the code, refused rather than clamped.
#[test]
fn drum_and_session_limits_are_refused_not_clamped() {
    let mut h = host();
    let d = id(
        &edit_ok(&mut h, "add_track", json!({"kind": "pulsus"})),
        "track_id",
    );
    let row = json!({"pad": "kick", "steps": "x..."});
    for args in [
        json!({"track_id": d, "at": 0, "bars": 300, "rows": [row]}),
        json!({"track_id": d, "at": 0, "rows": [{"pad": "kick", "steps": "x...", "velocity": 200}]}),
        json!({"track_id": d, "at": 0, "rows": vec![row.clone(); 33]}),
        json!({"track_id": d, "at": 0, "rows": [{"pad": "kick", "steps": "x".repeat(300)}]}),
    ] {
        assert!(matches!(
            edit_err(&mut h, "write_drum_pattern", args),
            ToolError::BadArgument { .. }
        ));
    }
    let t = id(
        &edit_ok(&mut h, "add_track", json!({"kind": "unda"})),
        "track_id",
    );
    let notes: Vec<Value> = (0..4097)
        .map(|_| json!({"start_beats": 0, "length_beats": 1, "pitch": 60}))
        .collect();
    let e = edit_err(
        &mut h,
        "set_session",
        json!({"add_scene": {"name": "A"}, "slot": {"track_id": t, "scene_id": 0, "length_beats": 4, "notes": notes}}),
    );
    assert!(e.to_string().contains("4096"), "{e}");
}

/// #6: a writer makes its own clip and never mixes its part into the user's notes.
#[test]
fn writers_make_their_own_clip() {
    let mut h = host();
    let t = piano(&mut h);
    let mine = id(
        &edit_ok(
            &mut h,
            "add_notes",
            json!({"track_id": t, "notes": [{"start": "1:1", "length": 1, "pitch": 60}]}),
        ),
        "clip_id",
    );
    let e = edit_err(
        &mut h,
        "write_part",
        json!({"track_id": t, "chart": "C | F", "pattern": "block"}),
    );
    assert!(matches!(e, ToolError::Refused(_)), "{e}");
    assert!(matches!(
        edit_err(
            &mut h,
            "voice_chords",
            json!({"track_id": t, "chart": "C | F"})
        ),
        ToolError::Refused(_)
    ));
    // Asked by clip_id, into that clip; elsewhere, a clip of its own.
    let r = edit_ok(
        &mut h,
        "voice_chords",
        json!({"track_id": t, "chart": "C | F", "clip_id": mine}),
    );
    assert_eq!(id(&r, "clip_id"), mine);
    let r = edit_ok(
        &mut h,
        "write_part",
        json!({"track_id": t, "chart": "C | F", "chart_start_bar": 5, "pattern": "pulse"}),
    );
    assert_ne!(id(&r, "clip_id"), mine);
    assert_eq!(r["clip_created"], true);
}

/// #10: a chart's first bar is bounded, its end inside the horizon; no overflow.
#[test]
fn a_chart_past_the_horizon_is_refused() {
    let mut h = host();
    let t = piano(&mut h);
    for bar in [json!(2_000_000), json!(u64::MAX), json!(999_999)] {
        let e = edit_err(
            &mut h,
            "write_part",
            json!({"track_id": t, "chart": "C | F | G", "chart_start_bar": bar, "pattern": "block"}),
        );
        assert!(matches!(e, ToolError::BadArgument { .. }), "{bar}: {e}");
    }
}

/// A host whose user allowed a folder beside the project's, for file tools.
struct Wide {
    bus: cantus_core::CommandBus,
    dir: std::path::PathBuf,
    extra: std::path::PathBuf,
}

impl cantus_ai::ToolHost for Wide {
    fn bus(&self) -> &cantus_core::CommandBus {
        &self.bus
    }
    fn bus_mut(&mut self) -> &mut cantus_core::CommandBus {
        &mut self.bus
    }
    fn project_dir(&self) -> Option<std::path::PathBuf> {
        Some(self.dir.clone())
    }
    fn file_roots(&self) -> Vec<std::path::PathBuf> {
        vec![self.dir.clone(), self.extra.clone()]
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

/// #12: a sample comes only from the project's folder, never from the folders allowed to
/// file tools (those ask for leave of their own, and an edit does not).
#[test]
fn a_sample_comes_only_from_the_project_folder() {
    let base = std::env::temp_dir().join(format!("cantus-review12-{}", std::process::id()));
    let (proj, other) = (base.join("song"), base.join("library"));
    std::fs::create_dir_all(&proj).unwrap();
    std::fs::create_dir_all(&other).unwrap();
    std::fs::write(proj.join("in.wav"), b"RIFF").unwrap();
    std::fs::write(other.join("out.wav"), b"RIFF").unwrap();
    let mut p = cantus_core::Project::new("s");
    let mut t = p.make_track("X");
    t.instrument = cantus_core::InstrumentSpec::Native(cantus_core::NativeSpec::new(
        cantus_core::InstrumentKind::Sampler,
    ));
    let x = t.id.0;
    p.tracks.push(t);
    let mut h = Wide {
        bus: cantus_core::CommandBus::new(p),
        dir: proj.clone(),
        extra: other.clone(),
    };
    let key = ["sample", "file", "z0.sample"]
        .into_iter()
        .find(|k| cantus_core::InstrumentKind::Sampler.is_asset_key(k))
        .unwrap();
    let cantus_ai::ToolRun::Edit(f) = tool("set_instrument_param").run else {
        panic!()
    };
    let mut call = |v: Value| {
        f(
            &mut cantus_ai::ToolCx { host: &mut h },
            &json!({"track_id": x, "params": {key: v}}),
        )
    };
    assert!(call(json!("in.wav")).is_ok());
    let outside = other.join("out.wav").display().to_string();
    assert!(
        matches!(call(json!(outside)), Err(ToolError::Refused(_))),
        "an allowed folder is the file tools', not an edit's"
    );
    assert!(matches!(
        call(json!("../library/out.wav")),
        Err(ToolError::Refused(_))
    ));
    let _ = std::fs::remove_dir_all(&base);
}

/// #13: notes never land where nothing sounds.
#[test]
fn notes_stay_where_they_are_heard() {
    let mut h = host();
    let t = piano(&mut h);
    let c = id(
        &edit_ok(
            &mut h,
            "add_clip",
            json!({"track_id": t, "start": 0, "length": "2 bars"}),
        ),
        "clip_id",
    );
    edit_ok(&mut h, "set_clip", json!({"clip_id": c, "loop_beats": 4}));
    let e = edit_err(
        &mut h,
        "replace_notes",
        json!({"clip_id": c, "notes": [{"start": 5, "length": 1, "pitch": 60}]}),
    );
    assert!(e.to_string().contains("first pass"), "{e}");
    // A note moved past the loop's first pass is refused.
    edit_ok(
        &mut h,
        "replace_notes",
        json!({"clip_id": c, "notes": [{"start": 3, "length": 1, "pitch": 60}]}),
    );
    assert!(matches!(
        edit_err(&mut h, "edit_notes", json!({"clip_id": c, "move_beats": 2})),
        ToolError::BadArgument { .. }
    ));
    // Past the end of a clip that does not loop, the clip grows to the bar.
    let d = id(
        &edit_ok(
            &mut h,
            "add_clip",
            json!({"track_id": t, "start": "5:1", "length": "1 bar"}),
        ),
        "clip_id",
    );
    edit_ok(
        &mut h,
        "add_notes",
        json!({"track_id": t, "clip_id": d, "notes": [{"start": "5:4", "length": 1, "pitch": 62}]}),
    );
    let r = edit_ok(&mut h, "edit_notes", json!({"clip_id": d, "move_beats": 3}));
    assert_eq!(r["clip_grown"], true);
    let clip = h.bus.project().tracks[0]
        .clip(cantus_core::ClipId(d))
        .unwrap()
        .clone();
    assert!(clip.end().as_beats_f64() >= 16.0 + 7.0, "{:?}", clip.end());
    // Before a trimmed clip's window: refused.
    h.bus
        .execute(
            cantus_core::Command::SetClipOffset {
                track: cantus_core::TrackId(t),
                clip: cantus_core::ClipId(d),
                offset: cantus_core::Ticks::beats(2),
            },
            cantus_core::Origin::User,
        )
        .unwrap();
    assert!(matches!(
        edit_err(
            &mut h,
            "edit_notes",
            json!({"clip_id": d, "move_beats": -6})
        ),
        ToolError::BadArgument { .. }
    ));
}

/// #14: the lint's reading of looped clips is capped and says so.
#[test]
fn the_lint_caps_a_loops_repetitions() {
    let mut h = host();
    let t = piano(&mut h);
    let c = id(
        &edit_ok(
            &mut h,
            "add_clip",
            json!({"track_id": t, "start": 0, "length": "30000 bars"}),
        ),
        "clip_id",
    );
    edit_ok(
        &mut h,
        "add_notes",
        json!({"track_id": t, "clip_id": c, "notes": [{"start": 0, "length": 0.25, "pitch": 60}]}),
    );
    edit_ok(&mut h, "set_clip", json!({"clip_id": c, "loop_beats": 0.5}));
    let started = std::time::Instant::now();
    let r = read_ok(&mut h, "lint_music", json!({}));
    assert_eq!(r["notes_capped"], true, "{}", r["notes_checked"]);
    assert!(started.elapsed().as_secs_f64() < 5.0);
}

/// #15: a name the user gave survives a swap of instrument.
#[test]
fn a_user_named_track_keeps_its_name_on_a_swap() {
    let mut h = host();
    let a = id(
        &edit_ok(
            &mut h,
            "add_track",
            json!({"kind": "gravis", "name": "Gravis Bass Line"}),
        ),
        "track_id",
    );
    let b = id(
        &edit_ok(&mut h, "add_track", json!({"kind": "gravis"})),
        "track_id",
    );
    let c = id(
        &edit_ok(&mut h, "add_track", json!({"kind": "gravis"})),
        "track_id",
    );
    edit_ok(
        &mut h,
        "set_instrument",
        json!({"track_id": a, "kind": "calor"}),
    );
    edit_ok(
        &mut h,
        "set_instrument",
        json!({"track_id": c, "kind": "calor"}),
    );
    let name = |id: u64| {
        h.bus
            .project()
            .track(cantus_core::TrackId(id))
            .unwrap()
            .name
            .clone()
    };
    assert_eq!(name(a), "Gravis Bass Line");
    assert_eq!(name(b), "Gravis");
    assert_eq!(name(c), "Calor", "\"Gravis 2\" was the instrument's name");
}

/// #16, #17: words — a limit named for what it is, a modulator named by which it is, a
/// name kept to one line.
#[test]
fn words_say_what_they_mean() {
    let mut h = host();
    let p = read_ok(&mut h, "get_project", json!({}));
    assert!(
        p["limits"]["automation_points_per_call"].is_number()
            && p["limits"].get("points_per_lane").is_none()
    );
    let t = id(
        &edit_ok(
            &mut h,
            "add_track",
            json!({"kind": "unda", "name": "Lead\nline\u{7}"}),
        ),
        "track_id",
    );
    let m = id(
        &edit_ok(&mut h, "add_modulator", json!({"host": t, "kind": "lfo"})),
        "modulator_id",
    );
    let def = tool("set_modulator");
    let cantus_ai::ToolRun::Edit(f) = def.run else {
        panic!()
    };
    let plan = f(
        &mut cantus_ai::ToolCx { host: &mut h },
        &json!({"modulator_id": m, "amount": 0.5}),
    )
    .unwrap();
    let flat = flatten(&plan.said);
    assert!(
        flat.contains("ai.phrase.modname.lfo") && flat.contains("n=1"),
        "{flat}"
    );
    assert!(
        flat.contains("Lead line") && !flat.contains('\n') && !flat.contains('\u{7}'),
        "{flat}"
    );
}

/// A sentence with its phrases as text, for looking into.
fn flatten(s: &Said) -> String {
    let mut out = s.key.to_string();
    for (k, v) in &s.args {
        out.push_str(&match v {
            SaidArg::Text(t) => format!(" {k}={t}"),
            SaidArg::Name(t) => format!(" {k}=“{t}”"),
            SaidArg::Bars { from, to } => format!(" {k}={from}-{to}"),
            SaidArg::Said(x) => format!(" {k}=[{}]", flatten(x)),
        });
    }
    out
}

// --- review 2 -----------------------------------------------------------------------------

/// A clip split in two: both halves hold the whole list of notes (a split shares them), each
/// showing its own part through its window.
fn split_clip(h: &mut BareHost) -> (u64, u64, u64) {
    use cantus_core::{Command, Origin, Ticks, TrackId};
    let t = piano(h);
    let c = id(
        &edit_ok(
            h,
            "add_clip",
            json!({"track_id": t, "start": 0, "length": "2 bars"}),
        ),
        "clip_id",
    );
    edit_ok(
        h,
        "add_notes",
        json!({"track_id": t, "clip_id": c, "notes": [
        {"start": 0, "length": 1, "pitch": 60}, {"start": 2, "length": 1, "pitch": 62},
        {"start": 5, "length": 1, "pitch": 64}, {"start": 7, "length": 1, "pitch": 65}]}),
    );
    let left = h.bus.project().tracks[0]
        .clip(cantus_core::ClipId(c))
        .unwrap()
        .clone();
    let mut right = left.clone();
    right.id = cantus_core::ClipId(h.bus.alloc_id());
    right.start = Ticks::beats(4);
    right.offset = Ticks::beats(4);
    right.length = Ticks::beats(4);
    let tid = TrackId(t);
    h.bus
        .execute(
            Command::SetClipBounds {
                track: tid,
                clip: left.id,
                start: Ticks::ZERO,
                length: Ticks::beats(4),
            },
            Origin::User,
        )
        .unwrap();
    h.bus
        .execute(
            Command::InsertClip {
                track: tid,
                clip: Box::new(right.clone()),
            },
            Origin::User,
        )
        .unwrap();
    (t, c, right.id.0)
}

/// Fix review #1: an edit of one half of a split touches only the notes it shows, grows
/// nothing and refuses nothing for the other half's notes; a trimmed tail grows nothing.
#[test]
fn an_edit_of_a_split_half_touches_only_what_it_shows() {
    let mut h = host();
    let (_, left, right) = split_clip(&mut h);
    let r = edit_ok(
        &mut h,
        "edit_notes",
        json!({"clip_id": left, "velocity": 50}),
    );
    assert_eq!(r["note_ids"].as_array().unwrap().len(), 2, "{r}");
    assert_eq!(r["clip_grown"], false);
    let l = h.bus.project().tracks[0]
        .clip(cantus_core::ClipId(left))
        .unwrap()
        .clone();
    assert_eq!(
        l.length,
        cantus_core::Ticks::beats(4),
        "the left half did not grow over the right"
    );
    let r = edit_ok(
        &mut h,
        "edit_notes",
        json!({"clip_id": right, "transpose_semitones": 2}),
    );
    assert_eq!(
        r["note_ids"].as_array().unwrap().len(),
        2,
        "the right half's own notes: {r}"
    );
    // A note whose tail the window cuts: a velocity edit grows nothing.
    let t = id(
        &edit_ok(&mut h, "add_track", json!({"kind": "lamina"})),
        "track_id",
    );
    let c = id(
        &edit_ok(
            &mut h,
            "add_clip",
            json!({"track_id": t, "start": 0, "length": "1 bar"}),
        ),
        "clip_id",
    );
    edit_ok(
        &mut h,
        "add_notes",
        json!({"track_id": t, "clip_id": c, "notes": [{"start": 3, "length": 1, "pitch": 60}]}),
    );
    edit_ok(&mut h, "set_clip", json!({"clip_id": c, "length": 3.5}));
    let r = edit_ok(&mut h, "edit_notes", json!({"clip_id": c, "velocity": 70}));
    assert_eq!(r["clip_grown"], false);
}

/// Fix review 2 #5: the lint's work is bounded however short a loop is, and a loop under
/// 1/64 beat is refused.
#[test]
fn a_tiny_loop_neither_floods_the_lint_nor_is_accepted() {
    use cantus_core::{Command, Origin, Ticks, TrackId};
    let mut h = host();
    let t = piano(&mut h);
    let c = id(
        &edit_ok(
            &mut h,
            "add_clip",
            json!({"track_id": t, "start": 0, "length": "100000 bars"}),
        ),
        "clip_id",
    );
    edit_ok(
        &mut h,
        "add_notes",
        json!({"track_id": t, "clip_id": c, "notes": [{"start": 0, "length": 0.01, "pitch": 60}]}),
    );
    assert!(matches!(
        edit_err(
            &mut h,
            "set_clip",
            json!({"clip_id": c, "loop_beats": 0.001})
        ),
        ToolError::BadArgument { .. }
    ));
    // A one-tick loop from a file: the lint stops at its cap, at once.
    h.bus
        .execute(
            Command::SetClipLoop {
                track: TrackId(t),
                clip: cantus_core::ClipId(c),
                content_length: Some(Ticks(1)),
            },
            Origin::User,
        )
        .unwrap();
    let started = std::time::Instant::now();
    let r = read_ok(&mut h, "lint_music", json!({}));
    assert_eq!(r["notes_capped"], true);
    assert!(
        started.elapsed().as_secs_f64() < 3.0,
        "{:?}",
        started.elapsed()
    );
}

/// Fix review 2 #6: a chart given as a list is held to the horizon too.
#[test]
fn a_list_chart_past_the_horizon_is_refused() {
    let mut h = host();
    let t = piano(&mut h);
    let e = edit_err(
        &mut h,
        "write_part",
        json!({"track_id": t, "pattern": "block",
        "chart": [{"at": 999_990, "chord": "C"}, {"at": 999_996, "chord": "F", "length": "4 bars"}]}),
    );
    assert!(e.to_string().contains("horizon"), "{e}");
}

/// Fix review 2 #7: a later clip of the same parameter is the user's curve too.
#[test]
fn automation_never_overrides_another_clips_points() {
    let mut h = host();
    let t = piano(&mut h);
    let target = json!({"track_id": t, "param": "gain_db"});
    edit_ok(
        &mut h,
        "automate",
        json!({"target": target, "points": [{"at": "3:1", "value": -12}, {"at": "4:1", "value": 0}]}),
    );
    // A new clip from bar 1 to bar 4 would take over bar 3's points.
    let e = edit_err(
        &mut h,
        "automate",
        json!({"target": target, "points": [{"at": "1:1", "value": -6}, {"at": "3:3", "value": -3}]}),
    );
    assert!(matches!(e, ToolError::Refused(_)), "{e}");
    let e = edit_err(
        &mut h,
        "automate",
        json!({"target": target, "start": "1:1", "points": [{"at": "2:1", "value": -6}, {"at": "3:3", "value": -3}]}),
    );
    assert!(matches!(e, ToolError::Refused(_)), "{e}");
    // Asked, the other clip's points in the span go and the new curve is drawn.
    edit_ok(
        &mut h,
        "replace_automation",
        json!({"target": target, "points": [{"at": "1:1", "value": -6}, {"at": "3:3", "value": -3}]}),
    );
    let clips = &h.bus.project().tracks[0].clips;
    let later = clips
        .iter()
        .find(|c| c.start.as_beats_f64() == 8.0)
        .unwrap();
    assert_eq!(
        later.automation.as_ref().unwrap().points.len(),
        1,
        "bar 4's point stays"
    );
}

/// Fix review 2 #8: no Edit tool overwrites the user's routes or mixes into their clips.
#[test]
fn routes_are_added_and_drums_write_their_own_clip() {
    let mut h = host();
    let t = id(
        &edit_ok(&mut h, "add_track", json!({"kind": "unda"})),
        "track_id",
    );
    let m = id(
        &edit_ok(
            &mut h,
            "add_modulator",
            json!({"host": t, "kind": "lfo", "routes": [{"target": {"track_id": t, "param": "pan"}, "depth": 0.2}]}),
        ),
        "modulator_id",
    );
    let e = edit_err(
        &mut h,
        "set_modulator",
        json!({"modulator_id": m, "routes": [{"target": {"track_id": t, "param": "gain_db"}, "depth": 0.1}]}),
    );
    assert!(matches!(e, ToolError::Refused(_)), "{e}");
    edit_ok(
        &mut h,
        "set_modulator",
        json!({"modulator_id": m, "add_routes": [{"target": {"track_id": t, "param": "gain_db"}, "depth": 0.1}]}),
    );
    let routes = |h: &BareHost| h.bus.project().tracks[0].modulators[0].routes.len();
    assert_eq!(routes(&h), 2);
    assert_eq!(tool("remove_items").class, ToolClass::Destructive);
    edit_ok(
        &mut h,
        "remove_items",
        json!({"modulator_routes": [{"modulator_id": m, "target": {"track_id": t, "param": "pan"}}]}),
    );
    assert_eq!(routes(&h), 1);
    let d = id(
        &edit_ok(&mut h, "add_track", json!({"kind": "pulsus"})),
        "track_id",
    );
    let row = json!([{"pad": "kick", "steps": "x..."}]);
    let c = id(
        &edit_ok(
            &mut h,
            "write_drum_pattern",
            json!({"track_id": d, "at": 0, "rows": row}),
        ),
        "clip_id",
    );
    assert!(
        matches!(
            edit_err(
                &mut h,
                "write_drum_pattern",
                json!({"track_id": d, "at": 0, "rows": row})
            ),
            ToolError::Refused(_)
        ),
        "no doubled hits"
    );
    edit_ok(
        &mut h,
        "write_drum_pattern",
        json!({"track_id": d, "at": 0, "rows": [{"pad": "snare", "steps": "..x."}], "clip_id": c}),
    );
    // An empty clip of the user's is theirs too.
    let k = id(
        &edit_ok(&mut h, "add_track", json!({"kind": "lamina"})),
        "track_id",
    );
    edit_ok(
        &mut h,
        "add_clip",
        json!({"track_id": k, "start": 0, "length": "2 bars"}),
    );
    assert!(matches!(
        edit_err(
            &mut h,
            "write_part",
            json!({"track_id": k, "chart": "C | F", "pattern": "block"})
        ),
        ToolError::Refused(_)
    ));
}

/// Fix review 2 #11: add_notes holds notes to a loop's first pass from its offset.
#[test]
fn add_notes_follows_the_loop_from_its_offset() {
    use cantus_core::{Command, Origin, Ticks, TrackId};
    let mut h = host();
    let t = piano(&mut h);
    let c = id(
        &edit_ok(
            &mut h,
            "add_clip",
            json!({"track_id": t, "start": 0, "length": "4 bars"}),
        ),
        "clip_id",
    );
    edit_ok(&mut h, "set_clip", json!({"clip_id": c, "loop_beats": 4}));
    h.bus
        .execute(
            Command::SetClipOffset {
                track: TrackId(t),
                clip: cantus_core::ClipId(c),
                offset: Ticks::beats(2),
            },
            Origin::User,
        )
        .unwrap();
    // The first pass is content 2…6 beats, heard at 0…4: a note at song beat 3 is content 5.
    edit_ok(
        &mut h,
        "add_notes",
        json!({"track_id": t, "clip_id": c, "notes": [{"start": 3, "length": 0.5, "pitch": 60}]}),
    );
    assert!(matches!(
        edit_err(
            &mut h,
            "add_notes",
            json!({"track_id": t, "clip_id": c, "notes": [{"start": 4.5, "length": 0.5, "pitch": 60}]})
        ),
        ToolError::BadArgument { .. }
    ));
}

// --- review 3 -----------------------------------------------------------------------------

/// Fix review 3 #3: two routes of one modulator removed in one call both go; one named
/// twice is refused.
#[test]
fn two_routes_of_one_modulator_go_together() {
    let mut h = host();
    let t = id(
        &edit_ok(&mut h, "add_track", json!({"kind": "unda"})),
        "track_id",
    );
    let pan = json!({"track_id": t, "param": "pan"});
    let gain = json!({"track_id": t, "param": "gain_db"});
    let m = id(
        &edit_ok(
            &mut h,
            "add_modulator",
            json!({"host": t, "kind": "lfo", "routes": [{"target": pan, "depth": 0.2}, {"target": gain, "depth": 0.1}]}),
        ),
        "modulator_id",
    );
    assert!(matches!(
        edit_err(
            &mut h,
            "remove_items",
            json!({"modulator_routes": [{"modulator_id": m, "target": pan}, {"modulator_id": m, "target": pan}]})
        ),
        ToolError::BadArgument { .. }
    ));
    edit_ok(
        &mut h,
        "remove_items",
        json!({"modulator_routes": [{"modulator_id": m, "target": pan}, {"modulator_id": m, "target": gain}]}),
    );
    assert!(h.bus.project().tracks[0].modulators[0].routes.is_empty());
}

/// Fix review 3 #4: a clip of the target on another track's lanes is the user's curve too,
/// and replace clears it on its own track.
#[test]
fn automation_checks_every_tracks_lanes() {
    let mut h = host();
    let a = piano(&mut h);
    let b = id(
        &edit_ok(&mut h, "add_track", json!({"kind": "unda"})),
        "track_id",
    );
    let target = json!({"strip_id": "master", "param": "gain_db"});
    edit_ok(
        &mut h,
        "automate",
        json!({"target": target, "lane_track_id": b, "points": [{"at": "2:1", "value": -6}]}),
    );
    let e = edit_err(
        &mut h,
        "automate",
        json!({"target": target, "lane_track_id": a, "points": [{"at": "1:1", "value": 0}, {"at": "3:1", "value": -3}]}),
    );
    assert!(matches!(e, ToolError::Refused(_)), "{e}");
    edit_ok(
        &mut h,
        "replace_automation",
        json!({"target": target, "lane_track_id": a, "points": [{"at": "1:1", "value": 0}, {"at": "3:1", "value": -3}]}),
    );
    let on_b = h
        .bus
        .project()
        .track(cantus_core::TrackId(b))
        .unwrap()
        .clips[0]
        .automation
        .as_ref()
        .unwrap()
        .points
        .len();
    assert_eq!(on_b, 0, "cleared on its own track");
}

/// Fix review 3 #9: add_routes and routes together are refused, not half done.
#[test]
fn add_routes_and_routes_together_are_refused() {
    let mut h = host();
    let t = id(
        &edit_ok(&mut h, "add_track", json!({"kind": "unda"})),
        "track_id",
    );
    let m = id(
        &edit_ok(&mut h, "add_modulator", json!({"host": t, "kind": "lfo"})),
        "modulator_id",
    );
    let r = json!([{"target": {"track_id": t, "param": "pan"}, "depth": 0.2}]);
    assert!(matches!(
        edit_err(
            &mut h,
            "set_modulator",
            json!({"modulator_id": m, "routes": r, "add_routes": r})
        ),
        ToolError::BadArgument { .. }
    ));
}

/// Fix review 3 #5: a range late in a long loop checks the repeats there; reaching the cap
/// says so.
#[test]
fn a_late_range_on_a_long_loop_checks_its_own_repeats() {
    let mut h = host();
    let t = piano(&mut h);
    let c = id(
        &edit_ok(
            &mut h,
            "add_clip",
            json!({"track_id": t, "start": 0, "length": "60000 bars"}),
        ),
        "clip_id",
    );
    edit_ok(
        &mut h,
        "add_notes",
        json!({"track_id": t, "clip_id": c, "notes": [{"start": 0, "length": 0.25, "pitch": 60}]}),
    );
    edit_ok(&mut h, "set_clip", json!({"clip_id": c, "loop_beats": 0.5}));
    // Bars 50000–50001: 16 repeats, well under the cap.
    let r = read_ok(
        &mut h,
        "lint_music",
        json!({"from": "50000:1", "to": "50002:1"}),
    );
    assert_eq!(r["notes_checked"], 16, "{r}");
    assert_eq!(r["notes_capped"], false);
    let r = read_ok(&mut h, "lint_music", json!({}));
    assert_eq!(r["notes_capped"], true);
}
