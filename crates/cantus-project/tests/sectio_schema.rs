//! Schema 55 (round 37, ADR 0099): a sliced loop's points and a zone's own low-pass come
//! back as they were saved; an older file has neither.

use cantus_core::inst::sampler::{zf, Zone};
use cantus_core::sectio::{SliceMode, Slicing};
use cantus_core::*;

fn sliced(v: u32) -> Option<NativeSpec> {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures");
    let text = std::fs::read_to_string(dir.join(format!("v{v}.cantus"))).ok()?;
    let p = cantus_project::parse(&text, true).unwrap().file.project;
    p.tracks
        .iter()
        .filter_map(|t| t.instrument.native())
        .find(|n| Slicing::of(n).is_some())
        .cloned()
}

#[test]
fn the_newest_fixtures_sliced_loop_comes_back_whole() {
    let spec = sliced(cantus_project::SCHEMA_VERSION).expect("a sliced Exemplar");
    let s = Slicing::of(&spec).unwrap();
    assert_eq!(s.points, vec![0, 24_000, 48_000, 72_000]);
    assert_eq!(
        (s.mode, s.len, s.rate, s.grid),
        (SliceMode::Transients, 96_000, 48_000, 4)
    );
    assert!((s.bpm - 120.0).abs() < 1e-6 && (s.sens - 0.6).abs() < 1e-6);
    let z2 = Zone::of(&spec, 2).unwrap();
    assert_eq!((z2.get(zf::REV), z2.get(zf::LPF)), (1.0, 2_400.0));
    assert_eq!((z2.get(zf::START), z2.get(zf::END)), (48_000.0, 72_000.0));
    assert_eq!(Zone::of(&spec, 3).unwrap().get(zf::EXCL), 1.0);
}

#[test]
fn files_before_schema_55_hold_no_slices() {
    for v in 16..55 {
        assert!(sliced(v).is_none(), "v{v}");
    }
}
