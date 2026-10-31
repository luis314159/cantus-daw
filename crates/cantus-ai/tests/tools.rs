//! Every project tool, called as the pump calls it, each mutating call one undo step that
//! gives back the project exactly (round 34, ADR 0082).

mod support;

use cantus_ai::tools_project::TOOLS;
use cantus_ai::{ToolClass, ToolError, ToolRun};
use serde_json::json;
use support::*;

#[test]
fn the_table_keeps_its_promises() {
    let mut seen = std::collections::BTreeSet::new();
    for t in TOOLS {
        assert!(seen.insert(t.name), "{} twice", t.name);
        let s = (t.input_schema)();
        assert_eq!(s["type"], "object", "{}", t.name);
        assert_eq!(
            s["additionalProperties"], false,
            "{} accepts unknown arguments",
            t.name
        );
        assert!(t.description.len() > 40, "{} says too little", t.name);
        match t.run {
            ToolRun::Edit(_) => assert!(
                matches!(t.class, ToolClass::Edit | ToolClass::Destructive),
                "{} plans an edit but is {:?}",
                t.name,
                t.class
            ),
            ToolRun::Read(_) => assert_eq!(t.class, ToolClass::Read, "{}", t.name),
            _ => {}
        }
        // Mutating tools never declare what the server adds.
        let props = s["properties"].as_object().unwrap();
        assert!(
            !props.contains_key("expected_revision") && !props.contains_key("change"),
            "{}",
            t.name
        );
    }
    // Removing or replacing the user's material is destructive.
    for name in [
        "remove_items",
        "replace_notes",
        "replace_effect",
        "set_instrument",
    ] {
        assert_eq!(tool(name).class, ToolClass::Destructive, "{name}");
    }
    // The legacy bridge's names that live on.
    for name in [
        "set_tempo",
        "set_tempo_point",
        "add_track",
        "add_notes",
        "add_effect",
    ] {
        tool(name);
    }
    assert!((30..=45).contains(&TOOLS.len()), "{} tools", TOOLS.len());
}

#[test]
fn reading_the_project_tracks_clips_mixer_devices_and_history() {
    let mut h = host();
    let p = read_ok(&mut h, "get_project", json!({}));
    assert_eq!(p["name"], "test");
    assert!(p["limits"]["inserts_per_strip"].as_u64().unwrap() >= 1);
    let t = edit_ok(
        &mut h,
        "add_track",
        json!({"kind": "chorda", "name": "Piano"}),
    );
    let track = id(&t, "track_id");
    let n = edit_ok(
        &mut h,
        "add_notes",
        json!({"track_id": track, "notes": [
            {"start": "2:1", "length": 1, "pitch": "C4"},
            {"start_beats": 5, "length_beats": 0.5, "pitch": 64, "velocity": 80}
        ]}),
    );
    let clip = id(&n, "clip_id");
    let p = read_ok(&mut h, "get_project", json!({}));
    assert_eq!(p["tracks"][0]["name"], "Piano");
    assert_eq!(p["tracks"][0]["instrument"]["kind"], "chorda");
    assert_eq!(p["tracks"][0]["notes"], 2);
    let tr = read_ok(&mut h, "get_track", json!({"track_id": track}));
    assert_eq!(tr["clips"][0]["id"], clip);
    assert_eq!(tr["clips"][0]["start"]["bar_beat"], "2:1");
    let notes = read_ok(&mut h, "get_clip_notes", json!({"clip_id": clip}));
    assert_eq!(notes["notes"][0]["name"], "C4");
    assert_eq!(notes["notes"][0]["start"]["beats"], 4.0);
    assert_eq!(notes["notes"][1]["start"]["bar_beat"], "2:2");
    let only = read_ok(
        &mut h,
        "get_clip_notes",
        json!({"clip_id": clip, "from": 4.5}),
    );
    assert_eq!(only["notes"].as_array().unwrap().len(), 1);
    let m = read_ok(&mut h, "get_mixer", json!({}));
    let strip = tr["strip_id"].as_u64().unwrap();
    assert!(m["strips"]
        .as_array()
        .unwrap()
        .iter()
        .any(|s| s["id"] == strip && s["tracks"][0] == track));
    let d = read_ok(
        &mut h,
        "get_device",
        json!({"track_id": track, "all": true}),
    );
    assert!(d["params"]
        .as_array()
        .unwrap()
        .iter()
        .any(|p| p["key"] == "pedal"));
    let hist = read_ok(&mut h, "get_history", json!({}));
    assert_eq!(hist["undo"][0]["by"], "agent");
    // Mistakes come back as errors the model can act on.
    assert!(matches!(
        read(&mut h, "get_track", json!({"track_id": 999})),
        Err(ToolError::NotFound(_))
    ));
    assert!(matches!(
        read(&mut h, "get_clip_notes", json!({})),
        Err(ToolError::BadArgument { .. })
    ));
    assert!(matches!(
        read(&mut h, "get_device", json!({})),
        Err(ToolError::BadArgument { .. })
    ));
}

#[test]
fn the_song_frame_tempo_metre_key_loop_and_markers() {
    let mut h = host();
    edit_ok(&mut h, "set_tempo", json!({"bpm": 116}));
    assert_eq!(h.bus.project().tempo_bpm, 116.0);
    assert!(matches!(
        edit_err(&mut h, "set_tempo", json!({"bpm": 2000})),
        ToolError::BadArgument { .. }
    ));
    edit_ok(
        &mut h,
        "set_tempo_point",
        json!({"at": "9:1", "bpm": 140, "ramp": false}),
    );
    assert_eq!(h.bus.project().tempo_points.len(), 2);
    edit_ok(
        &mut h,
        "set_song",
        json!({"name": "Heart", "key": {"root": "F", "scale": "minor"},
        "loop": {"start": "1:1", "end": "5:1"}, "metronome": true}),
    );
    let p = h.bus.project();
    assert_eq!(p.name, "Heart");
    assert_eq!(p.key.map(|k| k.root), Some(5));
    assert!(p.loop_enabled && p.metronome);
    edit_ok(&mut h, "set_song", json!({"time_signature": "3/4"}));
    assert_eq!(h.bus.project().time_signature.to_string(), "3/4");
    edit_ok(
        &mut h,
        "set_song",
        json!({"time_signature": "6/8", "time_signature_at_bar": 5}),
    );
    let map = h.bus.project().tempo_map();
    assert_eq!(map.signature_at(map.bar_tick(5)).to_string(), "6/8");
    assert_eq!(map.signature_at(map.bar_tick(4)).to_string(), "3/4");
    edit_ok(&mut h, "set_song", json!({"key": null, "loop": null}));
    assert!(h.bus.project().key.is_none() && h.bus.project().loop_region.is_none());
    let m = edit_ok(&mut h, "set_marker", json!({"at": "3:1", "name": "Chorus"}));
    let mid = id(&m, "marker_id");
    edit_ok(
        &mut h,
        "set_marker",
        json!({"marker_id": mid, "name": "Verse"}),
    );
    assert_eq!(h.bus.project().markers[0].name, "Verse");
    // A position in 3/4: bar 3 starts at beat 6.
    let p = read_ok(&mut h, "get_project", json!({}));
    assert_eq!(p["markers"][0]["at"]["beats"], 6.0);
    let at = p["tempo_points"][1]["at"]["beats"].clone();
    edit_ok(
        &mut h,
        "remove_items",
        json!({"marker_ids": [mid], "tempo_points_at": [at]}),
    );
    assert_eq!(h.bus.project().tempo_points.len(), 1);
    assert!(h.bus.project().markers.is_empty());
    assert!(matches!(
        edit_err(&mut h, "set_marker", json!({"at": "0:1", "name": "x"})),
        ToolError::BadArgument { .. }
    ));
}

#[test]
fn tracks_instruments_and_their_parameters() {
    let mut h = host();
    let a = edit_ok(&mut h, "add_track", json!({"kind": "gravis"}));
    let b = edit_ok(&mut h, "add_track", json!({"kind": "gravis"}));
    let (ta, tb) = (id(&a, "track_id"), id(&b, "track_id"));
    assert_eq!(a["name"], "Gravis");
    assert_eq!(b["name"], "Gravis 2", "names are made unique");
    assert_ne!(a["strip_id"], b["strip_id"], "each track has its own strip");
    // Every strip taken: the track still gets one.
    for _ in 0..h.bus.project().strips.len() {
        edit_ok(&mut h, "add_track", json!({"kind": "unda"}));
    }
    let last = h.bus.project().tracks.last().unwrap();
    assert!(last.strip.is_some(), "a new strip was made");
    edit_ok(
        &mut h,
        "set_track",
        json!({"track_id": ta, "name": "Bass", "gain_db": -3, "pan": -0.2,
        "muted": true, "color": "#336699", "index": 1}),
    );
    let t = h.bus.project().track(cantus_core::TrackId(ta)).unwrap();
    assert_eq!((t.name.as_str(), t.gain_db, t.muted), ("Bass", -3.0, true));
    assert_eq!(
        h.bus.project().track_index(cantus_core::TrackId(ta)),
        Some(1)
    );
    assert!(matches!(
        edit_err(&mut h, "set_track", json!({"track_id": ta, "gain_db": 20})),
        ToolError::BadArgument { .. }
    ));
    assert!(matches!(
        edit_err(&mut h, "set_track", json!({"track_id": ta})),
        ToolError::BadArgument { .. }
    ));
    // Parameters by key in their units; out of range refused with the range.
    let key = cantus_core::InstrumentKind::Bass.params()[0];
    edit_ok(
        &mut h,
        "set_instrument_param",
        json!({"track_id": tb, "params": {key.key: key.max}}),
    );
    assert_eq!(
        h.bus
            .project()
            .track(cantus_core::TrackId(tb))
            .unwrap()
            .instrument
            .native()
            .unwrap()
            .get(key.key),
        key.max
    );
    let e = edit_err(
        &mut h,
        "set_instrument_param",
        json!({"track_id": tb, "params": {key.key: key.max + 1000.0}}),
    );
    assert!(e.to_string().contains("must be"), "{e}");
    assert!(matches!(
        edit_err(
            &mut h,
            "set_instrument_param",
            json!({"track_id": tb, "params": {"nope": 1}})
        ),
        ToolError::BadArgument { .. }
    ));
    // Firmus by its keys — the legacy bridge's `set_synth_param` call shape too.
    let f = id(
        &edit_ok(&mut h, "add_track", json!({"name": "Lead"})),
        "track_id",
    );
    edit_ok(
        &mut h,
        "set_instrument_param",
        json!({"track_id": f, "param": "cutoff", "value": 1200}),
    );
    edit_ok(
        &mut h,
        "set_instrument_param",
        json!({"track_id": f, "params": {"amp_release": 0.5, "osc_mix": 0.3}}),
    );
    // Pulsus pads by note.
    let d = id(
        &edit_ok(
            &mut h,
            "add_track",
            json!({"kind": "pulsus", "name": "Drums"}),
        ),
        "track_id",
    );
    edit_ok(
        &mut h,
        "set_instrument_param",
        json!({"track_id": d, "params": {"pad36.decay": 0.5, "pad38.tune": 2, "pad36.name": "Boom", "kit.seed": 7}}),
    );
    let kit = h
        .bus
        .project()
        .track(cantus_core::TrackId(d))
        .unwrap()
        .instrument
        .pulsus()
        .unwrap()
        .clone();
    let kick = kit.pads.iter().find(|p| p.pitch == 36).unwrap();
    assert_eq!((kick.decay, kick.name.as_str()), (0.5, "Boom"));
    assert!(matches!(
        edit_err(
            &mut h,
            "set_instrument_param",
            json!({"track_id": d, "params": {"pad99.decay": 1}})
        ),
        ToolError::NotFound(_)
    ));
    // A sample's path outside the project's folder is refused.
    let x = id(
        &edit_ok(&mut h, "add_track", json!({"kind": "exemplar"})),
        "track_id",
    );
    let asset = ["sample", "file", "z0.sample"]
        .into_iter()
        .find(|k| cantus_core::InstrumentKind::Sampler.is_asset_key(k))
        .expect("Exemplar has an asset key");
    assert!(matches!(
        edit_err(
            &mut h,
            "set_instrument_param",
            json!({"track_id": x, "params": {asset: "C:/Windows/win.ini"}})
        ),
        ToolError::Refused(_)
    ));
    // Swapping the instrument renames a track named after it.
    edit_ok(
        &mut h,
        "set_instrument",
        json!({"track_id": tb, "kind": "calor"}),
    );
    let t = h.bus.project().track(cantus_core::TrackId(tb)).unwrap();
    assert_eq!(
        cantus_ai::tools_project::args::instrument_id(&t.instrument),
        "calor"
    );
    assert_eq!(t.name, "Calor");
}

#[test]
fn clips_and_notes_add_replace_edit_move_duplicate() {
    let mut h = host();
    let t = id(
        &edit_ok(&mut h, "add_track", json!({"kind": "lamina"})),
        "track_id",
    );
    let c = edit_ok(
        &mut h,
        "add_clip",
        json!({"track_id": t, "start": "1:1", "length": "2 bars", "name": "Keys"}),
    );
    let clip = id(&c, "clip_id");
    assert_eq!(c["end"]["bar_beat"], "3:1");
    let n = edit_ok(
        &mut h,
        "add_notes",
        json!({"track_id": t, "clip_id": clip, "notes": [
        {"start": 0, "length": 1, "pitch": 60}, {"start": 1.1, "length": 1, "pitch": 62},
        {"start": 2, "length": 1, "pitch": 64}, {"start": 7, "length": 2, "pitch": 65}]}),
    );
    assert_eq!(n["clip_created"], false);
    // A note past the clip grows it to the bar.
    let clipr = h.bus.project().tracks[0].clips[0].clone();
    assert_eq!(clipr.length.as_beats_f64(), 12.0);
    let ids: Vec<u64> = n["note_ids"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_u64().unwrap())
        .collect();
    edit_ok(
        &mut h,
        "edit_notes",
        json!({"clip_id": clip, "transpose_semitones": 12, "quantize": {"grid": "1/4"}, "velocity": 70}),
    );
    let notes = &h.bus.project().tracks[0].clips[0].notes;
    assert!(notes.iter().all(|n| n.pitch >= 72 && n.velocity == 70));
    assert!(
        notes.iter().all(|n| n.start.0 % cantus_core::PPQ == 0),
        "quantised"
    );
    edit_ok(
        &mut h,
        "edit_notes",
        json!({"clip_id": clip, "note_ids": [ids[0]], "humanize": {"seed": 3}}),
    );
    edit_ok(
        &mut h,
        "edit_notes",
        json!({"clip_id": clip, "legato": true, "length_scale": 0.5, "move_beats": 1}),
    );
    assert!(matches!(
        edit_err(
            &mut h,
            "edit_notes",
            json!({"clip_id": clip, "transpose_semitones": 60, "note_ids": [ids[3]]})
        ),
        ToolError::BadArgument { .. }
    ));
    assert!(matches!(
        edit_err(&mut h, "edit_notes", json!({"clip_id": clip})),
        ToolError::BadArgument { .. }
    ));
    edit_ok(
        &mut h,
        "replace_notes",
        json!({"clip_id": clip, "from": 0, "to": 4, "notes": [{"start": 0, "length": 4, "pitch": "A3"}]}),
    );
    edit_ok(
        &mut h,
        "set_clip",
        json!({"clip_id": clip, "start": "5:1", "name": "Keys 2", "muted": false, "loop_beats": 8, "color": "#aa0000"}),
    );
    let c = &h.bus.project().tracks[0].clips[0];
    assert_eq!((c.start.as_beats_f64(), c.name.as_str()), (16.0, "Keys 2"));
    let d = edit_ok(
        &mut h,
        "duplicate_clip",
        json!({"clip_id": clip, "times": 3}),
    );
    assert_eq!(d["copies"].as_array().unwrap().len(), 3);
    assert_eq!(h.bus.project().tracks[0].clips.len(), 4);
    // Every id is new: nothing shared between the copies.
    let mut all: Vec<u64> = h.bus.project().tracks[0]
        .clips
        .iter()
        .flat_map(|c| std::iter::once(c.id.0).chain(c.notes.iter().map(|n| n.id.0)))
        .collect();
    let len = all.len();
    all.sort_unstable();
    all.dedup();
    assert_eq!(all.len(), len);
    let other = id(
        &edit_ok(&mut h, "add_track", json!({"kind": "unda"})),
        "track_id",
    );
    edit_ok(
        &mut h,
        "set_clip",
        json!({"clip_id": clip, "track_id": other}),
    );
    assert_eq!(h.bus.project().tracks[1].clips.len(), 1);
    edit_ok(&mut h, "remove_items", json!({"clip_ids": [clip]}));
    assert!(h.bus.project().tracks[1].clips.is_empty());
    assert!(matches!(
        edit_err(
            &mut h,
            "add_notes",
            json!({"track_id": t, "notes": [{"start": 0, "length": 0, "pitch": 60}]})
        ),
        ToolError::BadArgument { .. }
    ));
    assert!(matches!(
        edit_err(
            &mut h,
            "add_notes",
            json!({"track_id": t, "notes": [{"start": 0, "length": 1, "pitch": "H2"}]})
        ),
        ToolError::BadArgument { .. }
    ));
}
