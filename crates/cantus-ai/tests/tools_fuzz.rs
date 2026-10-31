//! Random tool calls, each checked to be one undo step that gives the project back exactly,
//! then every step undone (the start, exactly) and redone (the end, exactly). The tools only
//! plan existing commands, so the commands' own fuzz (`cantus-core/tests/undo_invariants.rs`)
//! needs no new arm; this is the tools' stream (round 34, ADR 0082). Seeds from
//! `CANTUS_TOOLS_FUZZ_SEEDS` (default 64) and `CANTUS_TOOLS_FUZZ_STEPS` (default 80).

mod support;

use serde_json::{json, Value};
use support::*;

struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }
    fn below(&mut self, n: u64) -> u64 {
        self.next() % n.max(1)
    }
    fn pick<'a, T>(&mut self, v: &'a [T]) -> Option<&'a T> {
        (!v.is_empty()).then(|| &v[self.below(v.len() as u64) as usize])
    }
    fn f(&mut self, lo: f64, hi: f64) -> f64 {
        lo + (hi - lo) * (self.below(10_000) as f64 / 10_000.0)
    }
}

const KINDS: &[&str] = &[
    "unda", "calor", "gravis", "chorda", "lamina", "pulsus", "firmus",
];
const EFFECTS: &[&str] = &[
    "aequalis",
    "comprimo",
    "spatium",
    "concentus",
    "fervor",
    "tonsor",
    "delay",
];
const CHARTS: &[&str] = &[
    "C Am | F G7",
    "Fm9 | Dbmaj7 | Abadd9 | Eb Eb/G",
    "Dm7 G7 | Cmaj7 . . A7alt",
    "Bbm7 | Ab/C | Dbmaj7 | Eb7sus4 Eb",
];

/// One random call, from what the project holds now.
fn call(r: &mut Rng, h: &mut cantus_ai::bridge::BareHost) -> (&'static str, Value) {
    let p = h.bus.project();
    let tracks: Vec<u64> = p.tracks.iter().map(|t| t.id.0).collect();
    let pitched: Vec<u64> = p
        .tracks
        .iter()
        .filter(|t| t.instrument.is_chromatic())
        .map(|t| t.id.0)
        .collect();
    let clips: Vec<u64> = p
        .tracks
        .iter()
        .flat_map(|t| {
            t.clips
                .iter()
                .filter(|c| c.audio.is_none() && c.automation.is_none())
                .map(|c| c.id.0)
        })
        .collect();
    let strips: Vec<u64> = p.strips.iter().map(|s| s.id.0).collect();
    let effects: Vec<u64> = p.all_effects().map(|(_, e)| e.id.0).collect();
    let t = r.pick(&tracks).copied().unwrap_or(1);
    let pt = r.pick(&pitched).copied().unwrap_or(t);
    let c = r.pick(&clips).copied().unwrap_or(1);
    let s = r.pick(&strips).copied().unwrap_or(1);
    let e = r.pick(&effects).copied().unwrap_or(1);
    let bar = r.below(8) + 1;
    match r.below(22) {
        0 | 1 => ("add_track", json!({"kind": r.pick(KINDS).unwrap()})),
        2 => (
            "set_track",
            json!({"track_id": t, "gain_db": r.f(-30.0, 6.0), "pan": r.f(-1.0, 1.0)}),
        ),
        3 | 4 => (
            "add_notes",
            json!({"track_id": t, "notes": (0..1 + r.below(6)).map(|i| json!({"start": format!("{bar}:{}", 1 + i % 4), "length": r.f(0.25, 3.0), "pitch": 36 + r.below(60)})).collect::<Vec<_>>()}),
        ),
        5 => (
            "edit_notes",
            json!({"clip_id": c, "transpose_semitones": r.below(13) as i64 - 6, "quantize": {"grid": "1/8", "strength": 0.5}}),
        ),
        6 => (
            "edit_notes",
            json!({"clip_id": c, "humanize": {"seed": r.below(99)}, "velocity_scale": r.f(0.5, 1.5)}),
        ),
        7 => (
            "set_clip",
            json!({"clip_id": c, "start": format!("{bar}:1"), "muted": r.below(2) == 0}),
        ),
        8 => (
            "duplicate_clip",
            json!({"clip_id": c, "times": 1 + r.below(3)}),
        ),
        9 => (
            "write_part",
            json!({"track_id": pt, "chart": r.pick(CHARTS).unwrap(), "chart_start_bar": bar, "pattern": r.pick(&["block", "pulse", "arpeggio", "bass"]).unwrap(), "rate": "1/8", "humanize": {"seed": r.below(9)}}),
        ),
        10 => (
            "voice_chords",
            json!({"track_id": pt, "chart": r.pick(CHARTS).unwrap(), "voices": 2 + r.below(4)}),
        ),
        11 => (
            "write_drum_pattern",
            json!({"track_id": t, "at": format!("{bar}:1"), "rows": [{"pad": "kick", "steps": "x...x..."}, {"pad": "closed_hat", "steps": "x.x.x.x."}]}),
        ),
        12 => (
            "add_effect",
            json!({"strip_id": if r.below(4) == 0 { json!("master") } else { json!(s) }, "kind": r.pick(EFFECTS).unwrap()}),
        ),
        13 => (
            "set_effect",
            json!({"effect_id": e, "bypass": r.below(2) == 0, "mix": r.f(0.0, 1.0)}),
        ),
        14 => (
            "set_strip",
            json!({"strip_id": s, "gain_db": r.f(-20.0, 3.0), "width": r.f(0.0, 2.0)}),
        ),
        15 => (
            "automate",
            json!({"target": {"track_id": t, "param": "pan"}, "points": [{"at": format!("{bar}:1"), "value": r.f(-1.0, 1.0)}, {"at": format!("{}:1", bar + 2), "value": r.f(-1.0, 1.0), "shape": "s_curve"}]}),
        ),
        16 => (
            "add_modulator",
            json!({"host": t, "kind": "lfo", "rate": "1/2", "routes": [{"target": {"track_id": t, "param": "gain_db"}, "depth": r.f(-0.5, 0.5)}]}),
        ),
        17 => (
            "set_song",
            json!({"key": {"root": "D", "scale": "dorian"}, "metronome": r.below(2) == 0}),
        ),
        18 => (
            "set_tempo_point",
            json!({"at": format!("{bar}:1"), "bpm": r.f(60.0, 180.0)}),
        ),
        19 => (
            "set_session",
            json!({"add_scene": {"name": format!("S{bar}")}, "slot": {"track_id": t, "scene_id": 0, "length_beats": 4}}),
        ),
        20 => ("remove_items", json!({"clip_ids": [c]})),
        _ => ("remove_items", json!({"effect_ids": [e]})),
    }
}

#[test]
fn random_tool_calls_undo_and_redo_exactly() {
    let seeds: u64 = std::env::var("CANTUS_TOOLS_FUZZ_SEEDS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(64);
    let steps: usize = std::env::var("CANTUS_TOOLS_FUZZ_STEPS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(80);
    let mut applied = 0;
    for seed in 0..seeds {
        let mut r = Rng(seed.wrapping_mul(0xA24B_AED4_963E_E407) ^ 0x7001);
        let mut h = host();
        let start = bare(h.bus.project());
        for _ in 0..steps {
            let (name, args) = call(&mut r, &mut h);
            let before = bare(h.bus.project());
            // `edit` checks the step's own undo and redo; a refusal must change nothing.
            if support::edit(&mut h, name, args.clone()).is_ok() {
                applied += 1;
            } else {
                assert_eq!(
                    bare(h.bus.project()),
                    before,
                    "seed {seed}: refused {name}({args}) changed the project"
                );
            }
        }
        let end = bare(h.bus.project());
        let mut undone = 0;
        while h.bus.can_undo() {
            h.bus.undo();
            undone += 1;
        }
        assert_eq!(
            bare(h.bus.project()),
            start,
            "seed {seed}: undoing all {undone} steps"
        );
        while h.bus.can_redo() {
            h.bus.redo();
        }
        assert_eq!(bare(h.bus.project()), end, "seed {seed}: redoing all");
    }
    assert!(
        applied > seeds as usize * steps / 3,
        "most calls apply ({applied})"
    );
}
