//! r39:conv — `set_impulse_response` (ADR 0112): an agent gives Aula a response from a file
//! inside the project's folder, one undo step; a file outside it, a path that is not audio,
//! a missing file and an effect that is not Aula are refused and change nothing; `null` goes
//! back to Cantus's own.

mod support;
use cantus_ai::ToolError;
use cantus_core::{EffectId, EffectKind};
use serde_json::json;
use support::*;

#[test]
fn an_agent_loads_a_response_from_inside_the_project_and_only_there() {
    let dir = std::env::temp_dir().join(format!("cantus-ai-aula-{}", std::process::id()));
    let outside = std::env::temp_dir().join(format!("cantus-ai-aula-out-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::create_dir_all(&outside).unwrap();
    let inside = dir.join("hall.wav");
    let away = outside.join("hall.wav");
    for p in [&inside, &away] {
        std::fs::write(p, b"RIFF").unwrap();
    }
    let mut h = host();
    h.dir = Some(dir.clone());
    let t = edit_ok(&mut h, "add_track", json!({"kind": "unda"}));
    let strip = id(&t, "strip_id");
    let a = edit_ok(
        &mut h,
        "add_effect",
        json!({"strip_id": strip, "kind": "aula"}),
    );
    let aula = id(&a, "effect_id");
    let c = edit_ok(
        &mut h,
        "add_effect",
        json!({"strip_id": strip, "kind": "convolution"}),
    );
    assert_ne!(id(&c, "effect_id"), aula, "the plain name names Aula too");
    let comp = edit_ok(
        &mut h,
        "add_effect",
        json!({"strip_id": strip, "kind": "compressor"}),
    );
    let comp = id(&comp, "effect_id");

    let path = inside.to_string_lossy().to_string();
    edit_ok(
        &mut h,
        "set_impulse_response",
        json!({"effect_id": aula, "path": path}),
    );
    let p = h.bus.project();
    let host_strip = p.effect_host(EffectId(aula)).unwrap();
    let spec = &p.effect(host_strip, EffectId(aula)).unwrap().spec;
    assert_eq!(spec.kind, EffectKind::Aula);
    // The stored path is the resolved one (`resolve_touch`): the same file as the one given.
    let stored = spec.ir.as_deref().map(|f| f.path.clone()).unwrap();
    assert_eq!(
        std::fs::canonicalize(&stored).unwrap(),
        std::fs::canonicalize(&inside).unwrap()
    );

    for (args, why) in [
        (
            json!({"effect_id": aula, "path": away.to_string_lossy()}),
            "outside the folders",
        ),
        (
            json!({"effect_id": aula, "path": dir.join("notes.txt").to_string_lossy()}),
            "not audio",
        ),
        (
            json!({"effect_id": aula, "path": dir.join("gone.wav").to_string_lossy()}),
            "missing",
        ),
        (
            json!({"effect_id": aula, "path": dir.join("..").join(outside.file_name().unwrap()).join("hall.wav").to_string_lossy()}),
            "outside through ..",
        ),
        (json!({"effect_id": comp, "path": path}), "not Aula"),
    ] {
        assert!(
            matches!(
                edit_err(&mut h, "set_impulse_response", args),
                ToolError::BadArgument { .. }
            ),
            "{why}"
        );
    }
    edit_ok(
        &mut h,
        "set_impulse_response",
        json!({"effect_id": aula, "path": null}),
    );
    let p = h.bus.project();
    assert!(p
        .effect(host_strip, EffectId(aula))
        .unwrap()
        .spec
        .ir
        .is_none());
    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::remove_dir_all(&outside);
}
