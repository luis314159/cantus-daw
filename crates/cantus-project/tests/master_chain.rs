//! The master's insert chain survives save and open (ADR 0026), and the round-19 fixture
//! opens with its limiter on the master.

use cantus_core::*;
use cantus_project::{load, parse, save};

#[test]
fn the_master_chain_survives_save_and_open() {
    let mut p = Project::new("Mastered");
    let key = p.strips[3].id;
    let mut eq = p.make_effect(EffectKind::Eq);
    eq.spec.set("b0.used", 1.0).unwrap();
    eq.spec.set("b0.gain", -2.5).unwrap();
    eq.mix = 0.8;
    let mut comp = p.make_effect(EffectKind::Compressor);
    comp.sidechain = Some(key);
    comp.bypass = true;
    let mut lim = p.make_effect(EffectKind::Limiter);
    lim.spec.set("ceiling", -1.0).unwrap();
    p.master.effects = vec![eq, comp, lim];
    p.master.fx_enabled = false;
    p.master.gain_db = -1.5;

    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("mastered.cantus");
    save(&p, &path, "test").unwrap();
    let back = load(&path, true).unwrap();
    assert_eq!(back.project.master, p.master);
    assert_eq!(back.project, p);
}

#[test]
fn a_bare_master_writes_no_chain() {
    // `effects` is skipped while empty, so a project without a master chain writes what
    // it wrote before the chain existed.
    let p = Project::new("Bare");
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("bare.cantus");
    save(&p, &path, "test").unwrap();
    let text = std::fs::read_to_string(&path).unwrap();
    let v: serde_json::Value = serde_json::from_str(&text).unwrap();
    assert!(v["project"]["master"].get("effects").is_none());
    assert_eq!(load(&path, true).unwrap().project.master, p.master);
}

#[test]
fn the_v18_fixture_opens_with_a_limiter_on_the_master() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/v18.cantus");
    let text = std::fs::read_to_string(path).unwrap();
    let r = parse(&text, false).unwrap();
    let m = &r.file.project.master;
    assert_eq!(m.effects.len(), 1);
    assert_eq!(m.effects[0].spec.kind, EffectKind::Limiter);
    assert!(m.fx_enabled);
    let host = r.file.project.effect_host(m.effects[0].id);
    assert_eq!(host, Some(StripId::MASTER));
}
