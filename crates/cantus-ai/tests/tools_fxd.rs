//! The X-Y pad and the formula controller through an agent's tools (fx side round wave D,
//! ADRs 0147, 0148): made on a host with their settings, routes linked through an axis and
//! a range, inputs moved as the hand moves them, a controller's input as a target that
//! reads as itself and round-trips, a formula that does not compile refused at its
//! character — every call one undo step (`support::edit` checks the undo and the redo).

mod support;

use cantus_ai::tools_project::{json as tj, modulation, words};
use cantus_ai::{Said, ToolCx, ToolError, ToolRun};
use cantus_core::modulator::{ModSource, RouteLink, RouteRange};
use cantus_core::{AutoTarget, Command, ModulatorId, Project, TrackId};
use serde_json::{json, Value};
use support::*;

/// The interface's words for a language, read from the locale files the application loads.
fn locale(es: bool) -> Vec<(String, String)> {
    let files = if es {
        [
            include_str!("../../cantus-ui/locales/areas/ai.es.txt"),
            include_str!("../../cantus-ui/locales/fx/controllers.es.txt"),
        ]
    } else {
        [
            include_str!("../../cantus-ui/locales/areas/ai.en.txt"),
            include_str!("../../cantus-ui/locales/fx/controllers.en.txt"),
        ]
    };
    files
        .iter()
        .flat_map(|f| f.lines())
        .filter(|l| !l.trim_start().starts_with('#'))
        .filter_map(|l| l.split_once(" = "))
        .map(|(k, v)| (k.trim().to_string(), v.to_string()))
        .collect()
}

/// A sentence in words, as the host words it; a key the locale lacks fails the test.
fn words_of(said: &Said, es: bool) -> String {
    let loc = locale(es);
    let word = |key: &str, args: &[(&str, String)]| -> String {
        let frame = loc
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.clone())
            .unwrap_or_else(|| panic!("{key} has no words (es: {es})"));
        cantus_ai::fill(&frame, args)
    };
    said.render(&word)
}

/// The plan a call makes, without executing it: its command and its sentence.
fn plan_of(h: &mut cantus_ai::bridge::BareHost, name: &str, args: Value) -> (Command, Said) {
    let ToolRun::Edit(f) = tool(name).run else {
        panic!("{name} is not an edit")
    };
    let p = f(&mut ToolCx { host: h }, &args).unwrap_or_else(|e| panic!("{name}: {e}"));
    (p.command, p.said)
}

fn modulator(p: &Project, id: u64) -> cantus_core::Modulator {
    p.modulator(ModulatorId(id)).expect("there").2.clone()
}

fn bad_reason(e: ToolError) -> (String, String) {
    match e {
        ToolError::BadArgument { name, reason } => (name, reason),
        other => panic!("not a bad argument: {other:?}"),
    }
}

/// A track (its strip) and a pad on the master driving its level (Y, through a range)
/// and its pan (X as it is).
fn with_pad(h: &mut cantus_ai::bridge::BareHost) -> (u64, u64, u64) {
    let t = edit_ok(h, "add_track", json!({"kind": "firmus", "name": "Lead"}));
    let (track, strip) = (id(&t, "track_id"), id(&t, "strip_id"));
    let m = edit_ok(
        h,
        "add_modulator",
        json!({"host": "master", "kind": "xy_pad", "x": 0.2, "y": 0.7, "unipolar": true,
        "inertia_ms": 250, "play_path": true,
        "path": {"points": [[0, 0], [1, 0], [1, 1], [0, 1]], "beats": 8, "smooth": true},
        "routes": [
            {"target": {"track_id": track, "param": "gain_db"}, "depth": 0.5, "axis": "y",
             "range": {"from": 0, "to": 0.6, "curve": 0.4}},
            {"target": {"track_id": track, "param": "pan"}, "depth": 0.3}
        ]}),
    );
    (track, strip, id(&m, "modulator_id"))
}

#[test]
fn an_agent_makes_an_x_y_pad_with_its_settings_axes_and_ranges() {
    let mut h = host();
    let (track, _, pad) = with_pad(&mut h);
    let m = modulator(h.bus.project(), pad);
    let ModSource::XyPad(x) = &m.source else {
        panic!("a pad: {:?}", m.source)
    };
    assert_eq!(
        (x.x, x.y, x.unipolar, x.inertia_ms),
        (0.2, 0.7, true, 250.0)
    );
    let path = x.path.as_ref().expect("a path");
    assert_eq!(path.points.len(), 4);
    assert_eq!((path.beats, path.smooth, x.play_path), (8.0, true, true));
    // One link, for the route that reads Y through a range; the pan reads X as it is.
    assert_eq!(
        x.links,
        vec![RouteLink {
            target: AutoTarget::TrackGain(TrackId(track)),
            output: 1,
            range: Some(RouteRange {
                from: 0.0,
                to: 0.6,
                curve: 0.4
            }),
        }]
    );
    assert_eq!(m.routes.len(), 2);
    // Read back as set_modulator takes it, from the mixer (the master's modulators).
    let mixer = read_ok(&mut h, "get_mixer", json!({}));
    let r = &mixer["master"]["modulators"][0];
    assert_eq!(r["kind"], "xy_pad");
    assert_eq!(r["settings"]["x"], json!(0.2));
    assert_eq!(r["settings"]["y"], json!(0.7));
    assert_eq!(r["settings"]["inertia_ms"], json!(250.0));
    assert_eq!(r["settings"]["play_path"], json!(true));
    assert_eq!(r["settings"]["path"]["beats"], json!(8.0));
    assert_eq!(r["settings"]["path"]["points"][2], json!([1.0, 1.0]));
    assert_eq!(r["routes"][0]["axis"], "y");
    assert_eq!(
        r["routes"][0]["range"],
        json!({"from": 0.0, "to": 0.6, "curve": 0.4})
    );
    assert_eq!(r["routes"][1]["axis"], "x");
    assert!(r["routes"][1].get("range").is_none());
    assert!(
        r["source"]["XyPad"].get("links").is_none(),
        "the links are the routes' axes and ranges, not the file's form"
    );
    // A pad's setting on another kind, and a formula's on a pad, are refused by name.
    let lfo = id(
        &edit_ok(
            &mut h,
            "add_modulator",
            json!({"host": track, "kind": "lfo"}),
        ),
        "modulator_id",
    );
    let (k, why) = bad_reason(edit_err(
        &mut h,
        "set_modulator",
        json!({"modulator_id": lfo, "inertia_ms": 10}),
    ));
    assert_eq!(k, "inertia_ms");
    assert!(why.contains("X-Y pad") && why.contains("lfo"), "{why}");
    let (k, _) = bad_reason(edit_err(
        &mut h,
        "set_modulator",
        json!({"modulator_id": pad, "formula": "a"}),
    ));
    assert_eq!(k, "formula");
    // A route of an LFO has no axis or range.
    let (_, why) = bad_reason(edit_err(
        &mut h,
        "set_modulator",
        json!({"modulator_id": lfo, "add_routes": [{"target": {"track_id": track, "param": "pan"}, "depth": 0.2, "axis": "y"}]}),
    ));
    assert!(why.contains("only an X-Y pad"), "{why}");
    // Playing a path it does not have is refused; removing the path stops playing it.
    edit_ok(
        &mut h,
        "set_modulator",
        json!({"modulator_id": pad, "path": null}),
    );
    let ModSource::XyPad(x) = modulator(h.bus.project(), pad).source else {
        unreachable!()
    };
    assert!(x.path.is_none() && !x.play_path);
    let (k, _) = bad_reason(edit_err(
        &mut h,
        "set_modulator",
        json!({"modulator_id": pad, "play_path": true}),
    ));
    assert_eq!(k, "play_path");
}

#[test]
fn an_agent_makes_a_formula_and_one_that_does_not_compile_is_refused_at_its_character() {
    let mut h = host();
    let t = edit_ok(&mut h, "add_track", json!({"kind": "firmus"}));
    let strip = id(&t, "strip_id");
    let m = edit_ok(
        &mut h,
        "add_modulator",
        json!({"host": format!("strip:{strip}"), "kind": "formula",
            "formula": "a*Sin(SongTime*Pi*b)+c", "a": 0.5, "b": 0.25, "c": 0.1, "bipolar": true,
            "routes": [{"target": {"strip_id": strip, "param": "width"}, "depth": 0.4,
                "range": {"from": -0.2, "to": 0.8}}]}),
    );
    let f = id(&m, "modulator_id");
    let got = modulator(h.bus.project(), f);
    let ModSource::Formula(src) = &got.source else {
        panic!("a formula: {:?}", got.source)
    };
    assert_eq!(src.text, "a*Sin(SongTime*Pi*b)+c");
    assert_eq!((src.inputs, src.bipolar), ([0.5, 0.25, 0.1], true));
    assert_eq!(src.links.len(), 1);
    assert_eq!(src.links[0].output, 0);
    // Its default is FL's 0…1 and FL's first example.
    let plain = id(
        &edit_ok(
            &mut h,
            "add_modulator",
            json!({"host": "master", "kind": "formula"}),
        ),
        "modulator_id",
    );
    let ModSource::Formula(d) = modulator(h.bus.project(), plain).source else {
        unreachable!()
    };
    assert!(!d.bipolar);
    assert_eq!(d.text, cantus_core::modulator::FormulaSource::DEFAULT_TEXT);
    // Read back: the text, the knobs, what moves it, the route's range (no axis: one output).
    let mixer = read_ok(&mut h, "get_mixer", json!({}));
    let r = &mixer["strips"][0]["modulators"][0];
    assert_eq!(r["kind"], "formula");
    assert_eq!(r["settings"]["formula"], "a*Sin(SongTime*Pi*b)+c");
    assert_eq!(r["settings"]["b"], json!(0.25));
    assert_eq!(r["settings"]["bipolar"], json!(true));
    assert_eq!(
        r["settings"]["reads"],
        json!({"knobs": ["a", "b", "c"], "time": true})
    );
    assert!(r["routes"][0].get("axis").is_none());
    assert_eq!(r["routes"][0]["range"]["from"], json!(-0.2));
    // Text that does not compile is never applied, here or on a new one: the error names
    // the character (counted from 1) and why.
    for (text, at, why) in [
        ("a*Sin(SongTime", "character 6", "never closed"),
        ("MouseX*2", "character 1", "depends on the moment"),
        (
            "a + Bogus(1)",
            "character 5",
            "no variable or function is called \"bogus\"",
        ),
        ("a b", "character 3", "follows a complete formula"),
        ("Sin(1, 2)", "character 1", "takes 1 value in parentheses"),
        ("a + ", "character 5", "where a value is needed"),
    ] {
        let (k, reason) = bad_reason(edit_err(
            &mut h,
            "set_modulator",
            json!({"modulator_id": f, "formula": text}),
        ));
        assert_eq!(k, "formula");
        assert!(
            reason.contains("does not compile") && reason.contains(at),
            "{text}: {reason}"
        );
        assert!(
            reason.to_lowercase().contains(&why.to_lowercase()),
            "{text}: {reason}"
        );
        bad_reason(edit_err(
            &mut h,
            "add_modulator",
            json!({"host": "master", "kind": "formula", "formula": text}),
        ));
    }
    // A formula's text edited through set_modulator.
    edit_ok(
        &mut h,
        "set_modulator",
        json!({"modulator_id": f, "formula": "saw(SongTime/4)"}),
    );
    let ModSource::Formula(src) = modulator(h.bus.project(), f).source else {
        unreachable!()
    };
    assert_eq!(src.text, "saw(SongTime/4)");
    assert_eq!(src.inputs, [0.5, 0.25, 0.1], "the knobs stay");
    // A formula has one output: no Y axis; bipolar and unipolar that disagree are refused.
    let (k, _) = bad_reason(edit_err(
        &mut h,
        "set_modulator",
        json!({"modulator_id": f, "change_routes": [{"target": {"strip_id": strip, "param": "width"}, "axis": "y"}]}),
    ));
    assert_eq!(k, "change_routes[0].axis");
    bad_reason(edit_err(
        &mut h,
        "set_modulator",
        json!({"modulator_id": f, "bipolar": true, "unipolar": true}),
    ));
}

#[test]
fn inputs_move_as_the_hand_moves_them_each_call_one_undo_step() {
    let mut h = host();
    let (_, _, pad) = with_pad(&mut h);
    let f = id(
        &edit_ok(
            &mut h,
            "add_modulator",
            json!({"host": "master", "kind": "formula"}),
        ),
        "modulator_id",
    );
    // The plan is the input's own command (a batch of them for two), worded as a move.
    let (cmd, said) = plan_of(
        &mut h,
        "set_modulator",
        json!({"modulator_id": pad, "x": 0.25, "y": 0.75}),
    );
    match &cmd {
        Command::Batch { commands, .. } => {
            assert_eq!(commands.len(), 2);
            assert!(commands
                .iter()
                .all(|c| matches!(c, Command::SetModulatorInput { id, .. } if id.0 == pad)));
        }
        other => panic!("two inputs are a batch of SetModulatorInput: {other:?}"),
    }
    assert_eq!(
        words_of(&said, false),
        "move X-Y pad 1 on the master: X to 0.25, Y to 0.75"
    );
    assert_eq!(
        words_of(&said, true),
        "mover Pad X-Y 1 del máster: X a 0,25, Y a 0,75"
    );
    edit_ok(
        &mut h,
        "set_modulator",
        json!({"modulator_id": pad, "x": 0.25, "y": 0.75}),
    );
    let m = modulator(h.bus.project(), pad);
    assert_eq!(
        (m.source.input(0), m.source.input(1)),
        (Some(0.25), Some(0.75))
    );
    let (cmd, said) = plan_of(
        &mut h,
        "set_modulator",
        json!({"modulator_id": f, "b": 0.3}),
    );
    assert!(
        matches!(cmd, Command::SetModulatorInput { input: 1, .. }),
        "{cmd:?}"
    );
    assert_eq!(
        words_of(&said, false),
        "move Formula 1 on the master: knob b to 0.3"
    );
    assert_eq!(
        words_of(&said, true),
        "mover Fórmula 1 del máster: perilla b a 0,3"
    );
    edit_ok(
        &mut h,
        "set_modulator",
        json!({"modulator_id": f, "b": 0.3}),
    );
    assert_eq!(modulator(h.bus.project(), f).source.input(1), Some(0.3));
    // A setting with an input: the modulator set, then the input moved — one step.
    edit_ok(
        &mut h,
        "set_modulator",
        json!({"modulator_id": f, "c": 1, "formula": "c"}),
    );
    assert_eq!(modulator(h.bus.project(), f).source.input(2), Some(1.0));
    // The wrong kind's input, and an input out of range, are refused.
    for (m, args, key) in [
        (f, json!({"x": 0.5}), "x"),
        (pad, json!({"a": 0.5}), "a"),
        (pad, json!({"x": 1.5}), "x"),
    ] {
        let mut args = args;
        args["modulator_id"] = json!(m);
        assert_eq!(bad_reason(edit_err(&mut h, "set_modulator", args)).0, key);
    }
}

#[test]
fn a_controller_input_is_a_target_that_reads_as_itself_and_round_trips() {
    let mut h = host();
    let (track, _, pad) = with_pad(&mut h);
    let f = id(
        &edit_ok(
            &mut h,
            "add_modulator",
            json!({"host": track, "kind": "formula", "b": 0.4}),
        ),
        "modulator_id",
    );
    let p = h.bus.project().clone();
    let y = AutoTarget::ModInput {
        modulator: ModulatorId(pad),
        input: 1,
    };
    let b = AutoTarget::ModInput {
        modulator: ModulatorId(f),
        input: 1,
    };
    // As an agent reads it: the input by name, where it is, its range.
    let jy = tj::target_json(&p, &y);
    assert_eq!(
        jy,
        json!({"modulator_id": pad, "param": "y", "value": 0.7, "min": 0.0, "max": 1.0})
    );
    let jb = tj::target_json(&p, &b);
    assert_eq!(jb["param"], "b");
    assert_eq!(jb["value"], json!(0.4));
    // And back: the same target, never the modulator's amount (the bug this fixes).
    for (t, j) in [(&y, &jy), (&b, &jb)] {
        assert_eq!(&modulation::target_of(&p, j, "target").unwrap(), t);
    }
    let amount = AutoTarget::ModAmount(ModulatorId(f));
    let ja = tj::target_json(&p, &amount);
    assert_eq!(modulation::target_of(&p, &ja, "target").unwrap(), amount);
    assert_eq!(
        modulation::target_of(&p, &json!({"modulator_id": f}), "t").unwrap(),
        amount
    );
    // In words, as itself.
    assert_eq!(words_of(&words::target(&p, &y), false), "X-Y pad 1 · Y");
    assert_eq!(words_of(&words::target(&p, &y), true), "Pad X-Y 1 · Y");
    assert_eq!(
        words_of(&words::target(&p, &b), false),
        "Formula 1 · knob b"
    );
    assert_eq!(
        words_of(&words::target(&p, &b), true),
        "Fórmula 1 · perilla b"
    );
    assert_eq!(
        words_of(&words::target(&p, &amount), false),
        "the amount of Formula 1"
    );
    // A name the modulator has no input of is refused, saying which it has.
    let wrong = json!({"modulator_id": pad, "param": "c"});
    let (_, why) = bad_reason(modulation::target_of(&p, &wrong, "t").unwrap_err());
    assert!(why.contains("x, y"), "{why}");
    let lfo = id(
        &edit_ok(
            &mut h,
            "add_modulator",
            json!({"host": track, "kind": "lfo"}),
        ),
        "modulator_id",
    );
    let p = h.bus.project().clone();
    let wrong = json!({"modulator_id": lfo, "param": "x"});
    let (_, why) = bad_reason(modulation::target_of(&p, &wrong, "t").unwrap_err());
    assert!(why.contains("no inputs"), "{why}");
    // The input is a parameter through the tools: an LFO drives knob b (one level), and a
    // gesture is drawn on the formula's knob a (on the track's own lanes) and the pad's X.
    edit_ok(
        &mut h,
        "set_modulator",
        json!({"modulator_id": lfo, "add_routes": [{"target": {"modulator_id": f, "param": "b"}, "depth": 0.2}]}),
    );
    assert_eq!(modulator(h.bus.project(), lfo).routes[0].target, b);
    let a = edit_ok(
        &mut h,
        "automate",
        json!({"target": {"modulator_id": f, "param": "a"}, "points": [{"at": 0, "value": 0}, {"at": 4, "value": 1}]}),
    );
    assert_eq!(
        id(&a, "lane_track_id"),
        track,
        "a track's formula: its own lanes"
    );
    let clip = read_ok(
        &mut h,
        "get_clip_notes",
        json!({"clip_id": id(&a, "clip_id")}),
    );
    assert_eq!(clip["clip"]["target"]["param"], "a");
    assert_eq!(clip["clip"]["target"]["modulator_id"], json!(f));
    edit_ok(
        &mut h,
        "automate",
        json!({"target": {"modulator_id": pad, "param": "x"}, "lane_track_id": track,
            "points": [{"at": 0, "value": 0.1}, {"at": 4, "value": 0.9}]}),
    );
    // The track's modulators read back with the input target by name.
    let tr = read_ok(&mut h, "get_track", json!({"track_id": track}));
    let lfo_json = tr["modulators"]
        .as_array()
        .unwrap()
        .iter()
        .find(|m| m["id"] == json!(lfo))
        .unwrap()
        .clone();
    assert_eq!(lfo_json["routes"][0]["target"]["param"], "b");
}

#[test]
fn links_follow_their_routes() {
    let mut h = host();
    let (track, _, pad) = with_pad(&mut h);
    let gain = json!({"track_id": track, "param": "gain_db"});
    let pan = json!({"track_id": track, "param": "pan"});
    let (gain_t, pan_t) = (
        AutoTarget::TrackGain(TrackId(track)),
        AutoTarget::TrackPan(TrackId(track)),
    );
    let links = |h: &cantus_ai::bridge::BareHost| match modulator(h.bus.project(), pad).source {
        ModSource::XyPad(x) => x.links,
        _ => unreachable!(),
    };
    // The pan's route moves to Y with a range and a new depth; the level's loses its range.
    edit_ok(
        &mut h,
        "set_modulator",
        json!({"modulator_id": pad, "change_routes": [
            {"target": pan, "axis": "y", "depth": -0.5, "range": {"from": 0.1, "to": 0.2}},
            {"target": gain, "range": null}
        ]}),
    );
    let m = modulator(h.bus.project(), pad);
    assert_eq!(m.route(&pan_t).unwrap().depth, -0.5);
    assert_eq!(links(&h).len(), 2);
    assert_eq!(
        m.source.link(&gain_t),
        (1, None),
        "the level still reads Y, as it is"
    );
    let r = RouteRange {
        from: 0.1,
        to: 0.2,
        curve: 0.0,
    };
    assert_eq!(m.source.link(&pan_t), (1, Some(r)));
    // Back to X as it is: no link is kept for it.
    edit_ok(
        &mut h,
        "set_modulator",
        json!({"modulator_id": pad, "change_routes": [{"target": pan, "axis": "x", "range": null}]}),
    );
    assert_eq!(links(&h).len(), 1);
    // A route it does not have cannot be changed; one named twice is refused.
    bad_reason(edit_err(
        &mut h,
        "set_modulator",
        json!({"modulator_id": pad, "change_routes": [{"target": {"strip_id": "master", "param": "pan"}, "depth": 0.1}]}),
    ));
    bad_reason(edit_err(
        &mut h,
        "set_modulator",
        json!({"modulator_id": pad, "change_routes": [{"target": pan, "depth": 0.1}, {"target": pan, "depth": 0.2}]}),
    ));
    // A removed route takes its link with it; a route added again starts as it is told.
    edit_ok(
        &mut h,
        "remove_items",
        json!({"modulator_routes": [{"modulator_id": pad, "target": gain}]}),
    );
    assert!(links(&h).is_empty(), "the level's link went with its route");
    edit_ok(
        &mut h,
        "set_modulator",
        json!({"modulator_id": pad, "add_routes": [{"target": gain, "depth": 0.2, "range": {"from": 0, "to": 1, "curve": -0.5}}]}),
    );
    let r = RouteRange {
        from: 0.0,
        to: 1.0,
        curve: -0.5,
    };
    assert_eq!(
        modulator(h.bus.project(), pad).source.link(&gain_t),
        (0, Some(r))
    );
    // A range out of −1…1 is refused before anything is applied.
    bad_reason(edit_err(
        &mut h,
        "set_modulator",
        json!({"modulator_id": pad, "change_routes": [{"target": gain, "range": {"from": 0, "to": 2}}]}),
    ));
}

/// The pad's inertia takes the model's whole range from an agent — 0 to 10 000 ms, the
/// window's knob's too (fxd follow-up) — and refuses past it, naming the argument.
#[test]
fn an_agent_sets_a_pads_inertia_over_the_models_whole_range() {
    use cantus_core::modulator::xy::MAX_INERTIA_MS;
    let mut h = host();
    let (_, _, pad) = with_pad(&mut h);
    edit_ok(
        &mut h,
        "set_modulator",
        json!({"modulator_id": pad, "inertia_ms": MAX_INERTIA_MS}),
    );
    let ModSource::XyPad(x) = modulator(h.bus.project(), pad).source else {
        panic!("a pad")
    };
    assert_eq!(x.inertia_ms, MAX_INERTIA_MS);
    let (k, _) = bad_reason(edit_err(
        &mut h,
        "set_modulator",
        json!({"modulator_id": pad, "inertia_ms": MAX_INERTIA_MS + 1.0}),
    ));
    assert_eq!(k, "inertia_ms");
}
