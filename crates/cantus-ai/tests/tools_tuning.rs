//! r42 fix2-model (review 9): the MCP tools and round 42 — a note's articulation read and
//! written back (get_clip_notes → replace_notes keeps it), Arcus in the instrument table, and
//! the tuning read and set (`get_tuning`, `set_tuning`) through the commands, each mutating
//! call one undo step that gives back the project exactly.

mod support;

use cantus_ai::ToolError;
use cantus_core::note_art::{Articulation, Bow, NoteArt};
use cantus_core::tuning::{Temperament, TrackTuning, TuningSystem};
use cantus_core::InstrumentKind;
use serde_json::{json, Value};
use support::*;

/// A track with three notes, the second pizzicato with an up bow; its ids.
fn strings(h: &mut cantus_ai::bridge::BareHost) -> (u64, u64) {
    let t = edit_ok(h, "add_track", json!({"kind": "arcus", "name": "Cello"}));
    let track = id(&t, "track_id");
    let n = edit_ok(
        h,
        "add_notes",
        json!({"track_id": track, "notes": [
            {"start": 0, "length": 1, "pitch": "C3"},
            {"start": 1, "length": 1, "pitch": "D3", "art": {"articulation": "pizzicato", "bow": "up"}},
            {"start": 2, "length": 1, "pitch": "E3", "art": {"articulation": "legato"}}
        ]}),
    );
    (track, id(&n, "clip_id"))
}

#[test]
fn arcus_is_in_the_instrument_table() {
    let mut h = host();
    let (track, _) = strings(&mut h);
    let p = h.bus.project();
    let t = p.tracks.iter().find(|t| t.id.0 == track).unwrap();
    assert_eq!(
        t.instrument.native().map(|n| n.kind),
        Some(InstrumentKind::BowedString)
    );
    let read = read_ok(&mut h, "get_project", json!({}));
    assert_eq!(read["tracks"][0]["instrument"]["kind"], "arcus");
    assert_eq!(read["tracks"][0]["instrument"]["name"], "Arcus");
    let schema = (tool("add_track").input_schema)();
    let kinds = schema["properties"]["kind"].to_string();
    assert!(kinds.contains("arcus"), "{kinds}");
}

#[test]
fn a_notes_articulation_is_read_and_survives_a_replace() {
    let mut h = host();
    let (_, clip) = strings(&mut h);
    let notes = read_ok(&mut h, "get_clip_notes", json!({"clip_id": clip}));
    let list = notes["notes"].as_array().unwrap();
    assert!(list[0].get("art").is_none(), "a note without one says none");
    assert_eq!(
        list[1]["art"],
        json!({"articulation": "pizzicato", "bow": "up"})
    );
    assert_eq!(list[2]["art"], json!({"articulation": "legato"}));
    // Read and written back as an agent edits a clip: the articulations are kept.
    let back: Vec<Value> = list
        .iter()
        .map(|n| {
            let mut w = json!({
                "start": n["start"]["beats"],
                "length": n["length_beats"],
                "pitch": n["pitch"],
                "velocity": n["velocity"],
            });
            if let Some(a) = n.get("art") {
                w["art"] = a.clone();
            }
            w
        })
        .collect();
    edit_ok(
        &mut h,
        "replace_notes",
        json!({"clip_id": clip, "notes": back}),
    );
    let arts: Vec<Option<NoteArt>> = h
        .bus
        .project()
        .tracks
        .iter()
        .flat_map(|t| t.clips.iter())
        .flat_map(|c| c.notes.iter())
        .map(|n| n.art)
        .collect();
    assert_eq!(
        arts,
        vec![
            None,
            Some(NoteArt {
                articulation: Articulation::Pizzicato,
                bow: Some(Bow::Up)
            }),
            Some(NoteArt {
                articulation: Articulation::Legato,
                bow: None
            }),
        ]
    );
    // A bad articulation is refused with the field named.
    let track = h.bus.project().tracks[0].id.0;
    match edit_err(
        &mut h,
        "add_notes",
        json!({"track_id": track, "notes": [{"start": 4, "length": 1, "pitch": 60, "art": {"articulation": "col legno"}}]}),
    ) {
        ToolError::BadArgument { .. } => {}
        e => panic!("{e:?}"),
    }
}

#[test]
fn the_project_s_tuning_is_read_and_set_through_its_command() {
    let mut h = host();
    let t = read_ok(&mut h, "get_tuning", json!({}));
    assert_eq!(t["tuning"]["system"], "equal");
    assert_eq!(t["tuning"]["is_default"], true);
    edit_ok(
        &mut h,
        "set_tuning",
        json!({"temperament": "werckmeister_iii", "key": "Eb", "a4_hz": 415}),
    );
    let p = h.bus.project();
    assert_eq!(p.tuning.a4_hz, 415.0);
    assert!(matches!(
        p.tuning.system,
        TuningSystem::Temperament {
            id: Temperament::WerckmeisterIii,
            key: 3,
            follow_song_key: false
        }
    ));
    let t = read_ok(&mut h, "get_tuning", json!({}));
    assert_eq!(t["tuning"]["temperament"], "werckmeister_iii");
    assert_eq!(t["tuning"]["key"], "Eb");
    let a4 = t["plays"]["a4_hz"].as_f64().unwrap();
    assert!((a4 - 415.0).abs() < 1e-6, "{a4}");
    // Only the key moves; the rest stays.
    edit_ok(
        &mut h,
        "set_tuning",
        json!({"key": 7, "follow_song_key": true}),
    );
    assert!(matches!(
        h.bus.project().tuning.system,
        TuningSystem::Temperament {
            key: 7,
            follow_song_key: true,
            ..
        }
    ));
    assert_eq!(h.bus.project().tuning.a4_hz, 415.0);
    // A Scala file's text, and a .kbm's.
    let scl = "! 5edo.scl\n!\nFive equal steps\n 5\n!\n 240.0\n 480.0\n 720.0\n 960.0\n 2/1\n";
    edit_ok(&mut h, "set_tuning", json!({"scala": scl, "a4_hz": 440}));
    let TuningSystem::Scale(s) = &h.bus.project().tuning.system else {
        panic!("a scale")
    };
    assert_eq!(
        (s.description.as_str(), s.degrees.len()),
        ("Five equal steps", 5)
    );
    let kbm = "12\n0\n127\n60\n60\n261.625565\n5\n0\nx\n1\nx\n2\n3\nx\n4\nx\nx\nx\nx\n";
    edit_ok(&mut h, "set_tuning", json!({"kbm": kbm}));
    let map = h.bus.project().tuning.map.clone().expect("a map");
    assert_eq!(map.mapping.iter().filter(|e| e.is_none()).count(), 7);
    let t = read_ok(&mut h, "get_tuning", json!({}));
    assert_eq!(t["tuning"]["keyboard_map"]["unmapped"], 7);
    assert!(t["plays"]["cents_c4_to_b4"][1].is_null(), "C#4 is an x key");
    // The MTS-ESP master's, then equal temperament drops the map.
    edit_ok(&mut h, "set_tuning", json!({"system": "external"}));
    assert_eq!(h.bus.project().tuning.system, TuningSystem::External);
    assert!(h.bus.project().tuning.map.is_none());
    edit_ok(
        &mut h,
        "set_tuning",
        json!({"system": "equal", "a4_hz": 440}),
    );
    assert!(h.bus.project().tuning.is_default());
}

#[test]
fn what_the_commands_refuse_the_tools_refuse() {
    let mut h = host();
    for bad in [
        json!({"a4_hz": 600}),
        json!({"temperament": "pythagorean", "key": 12}),
        json!({"temperament": "pythagorean", "key": "H"}),
        json!({"temperament": "no such"}),
        json!({"scala": "two\n2\n100.0\n"}),
        json!({"key": "C"}),
        json!({"system": "scale"}),
        json!({"kbm": "0\n0\n127\n60\n69\n440\n0\n"}),
        json!({"mode": "own"}),
        json!({}),
    ] {
        match edit_err(&mut h, "set_tuning", bad.clone()) {
            ToolError::BadArgument { .. } => {}
            e => panic!("{bad}: {e:?}"),
        }
    }
}

#[test]
fn a_track_follows_plays_equal_or_has_its_own() {
    let mut h = host();
    let (track, _) = strings(&mut h);
    edit_ok(
        &mut h,
        "set_tuning",
        json!({"temperament": "vallotti", "key": "D"}),
    );
    let t = read_ok(&mut h, "get_tuning", json!({"track_id": track}));
    assert_eq!(t["mode"], "follow");
    assert_eq!(t["plays_tuning"]["temperament"], "vallotti");
    edit_ok(
        &mut h,
        "set_tuning",
        json!({"track_id": track, "mode": "equal"}),
    );
    assert_eq!(h.bus.project().tracks[0].tuning, TrackTuning::Equal);
    let t = read_ok(&mut h, "get_tuning", json!({"track_id": track}));
    assert_eq!(t["plays"]["cents_c4_to_b4"][0], 0.0);
    edit_ok(
        &mut h,
        "set_tuning",
        json!({"track_id": track, "a4_hz": 432}),
    );
    let TrackTuning::Own { tuning } = &h.bus.project().tracks[0].tuning else {
        panic!("its own")
    };
    assert_eq!(tuning.a4_hz, 432.0);
    assert_eq!(
        tuning.system,
        TuningSystem::Equal,
        "from what it played: equal"
    );
    edit_ok(
        &mut h,
        "set_tuning",
        json!({"track_id": track, "mode": "follow"}),
    );
    assert_eq!(h.bus.project().tracks[0].tuning, TrackTuning::Follow);
    match edit_err(
        &mut h,
        "set_tuning",
        json!({"track_id": track, "mode": "equal", "a4_hz": 415}),
    ) {
        ToolError::BadArgument { .. } => {}
        e => panic!("{e:?}"),
    }
    match edit_err(
        &mut h,
        "set_tuning",
        json!({"track_id": 999_999, "mode": "equal"}),
    ) {
        ToolError::NotFound(_) | ToolError::BadArgument { .. } => {}
        e => panic!("{e:?}"),
    }
}
