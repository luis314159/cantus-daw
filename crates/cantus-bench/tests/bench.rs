//! The suite's own tests: the sessions are what they claim to be, the gate fails exactly
//! when it should, and a short run of the rig produces sound and names its hot spots.
//! Nothing here times anything — timing belongs to `cantus-bench` itself, in release.

use cantus_bench::compare::{self, Status};
use cantus_bench::report::{class_of, Better, Metric, Report};
use cantus_bench::rig::Rig;
use cantus_bench::sessions;
use cantus_bench::stats::Summary;
use cantus_core::{EffectKind, InstrumentKind, Ticks};
use std::collections::HashSet;

#[test]
fn scaled_sessions_have_the_tracks_asked_for_with_fresh_ids() {
    for n in [10usize, 32, 64, 100] {
        let p = sessions::scaled(n);
        assert_eq!(p.tracks.len(), n, "tracks of s{n}");
        assert!(p.strips.len() <= sessions::ENGINE_TRACKS, "strips of s{n}");
        assert!(p.loop_enabled && p.loop_region.is_some());
        let mut ids = HashSet::new();
        for t in &p.tracks {
            assert!(ids.insert(t.id.0), "track id {} twice", t.id.0);
            for c in &t.clips {
                assert!(ids.insert(c.id.0), "clip id {} twice", c.id.0);
                for note in &c.notes {
                    assert!(ids.insert(note.id.0), "note id {} twice", note.id.0);
                }
            }
            if let Some(s) = t.strip {
                assert!(p.strip(s).is_some(), "a track feeds a strip that exists");
            }
        }
        for s in &p.strips {
            assert!(ids.insert(s.id.0), "strip id twice");
            for e in &s.effects {
                assert!(ids.insert(e.id.0), "effect id twice");
            }
        }
        // Every track the engine plays has something to play inside the loop.
        let l = p.loop_region.clone().unwrap();
        let playing = p
            .tracks
            .iter()
            .filter(|t| t.clips.iter().any(|c| c.start < l.end && c.end() > l.start))
            .count();
        assert!(
            playing * 10 >= n * 7,
            "s{n}: only {playing} tracks play in the loop"
        );
    }
}

#[test]
fn the_dawbench_sessions_are_built_as_the_method_says() {
    let p = sessions::dsp(4, 8);
    assert_eq!(p.tracks.len(), 4);
    for t in &p.tracks {
        let s = p.strip(t.strip.expect("a strip")).unwrap();
        assert_eq!(s.effects.len(), 8);
        assert!(s
            .effects
            .iter()
            .all(|e| e.spec.kind == EffectKind::Compressor));
        assert_eq!(s.effects[0].spec.get("thr"), -30.0);
    }
    let p = sessions::vi(3, InstrumentKind::Analog);
    assert_eq!(p.tracks.len(), 3);
    assert!(p.tracks.iter().all(|t| t.clips[0].notes.len() == 64));
}

fn report(values: &[(&str, Better, [f64; 3])]) -> Report {
    let mut r = Report::new("quick");
    for (id, better, v) in values {
        r.metrics
            .push(Metric::new(*id, "x", *better, Summary::of(v)));
    }
    r
}

#[test]
fn the_gate_fails_on_a_clear_regression_and_only_then() {
    let base = report(&[
        ("a", Better::Lower, [100.0, 102.0, 104.0]),
        ("b", Better::Lower, [100.0, 102.0, 104.0]),
        ("c", Better::Higher, [50.0, 51.0, 52.0]),
        ("d", Better::Lower, [10.0, 10.0, 10.0]),
        ("gone", Better::Lower, [1.0, 1.0, 1.0]),
    ]);
    let now = report(&[
        // 40 % slower, the runs apart: a regression.
        ("a", Better::Lower, [140.0, 142.0, 145.0]),
        // 40 % slower in the median, but one run as fast as before: noise.
        ("b", Better::Lower, [101.0, 142.0, 150.0]),
        // Twice the throughput: better.
        ("c", Better::Higher, [100.0, 101.0, 102.0]),
        // 10 % slower and very repeatable: under the threshold.
        ("d", Better::Lower, [11.0, 11.0, 11.0]),
        ("new", Better::Lower, [1.0, 1.0, 1.0]),
    ]);
    let v = compare::compare(&now, &base, 25.0);
    let status = |id: &str| v.iter().find(|x| x.id == id).unwrap().status;
    assert_eq!(status("a"), Status::Regressed);
    assert_eq!(status("b"), Status::Noisy);
    assert_eq!(status("c"), Status::Improved);
    assert_eq!(status("d"), Status::Same);
    assert_eq!(status("new"), Status::New);
    assert_eq!(status("gone"), Status::Missing);
    assert!(compare::failed(&v));
    assert!(compare::describe(&v).starts_with("REGRESS"));
    // The same run against itself passes; an ungated metric never fails.
    assert!(!compare::failed(&compare::compare(&base, &base, 25.0)));
    let mut ungated = now.clone();
    ungated.metrics[0].gate = false;
    let v = compare::compare(&ungated, &base, 25.0);
    assert_eq!(
        v.iter().find(|x| x.id == "a").unwrap().status,
        Status::Ungated
    );
}

#[test]
fn a_report_round_trips_and_names_its_machine_class() {
    let r = report(&[("fx.x.default", Better::Lower, [1.0, 2.0, 3.0])]);
    let dir = std::env::temp_dir().join(format!("cantus-bench-test-{}", std::process::id()));
    let path = dir.join("r.json");
    r.save(&path).unwrap();
    assert_eq!(Report::load(&path).unwrap(), r);
    let _ = std::fs::remove_dir_all(&dir);
    assert!(r.table("fx.").contains("`fx.x.default`"));
    assert_eq!(
        class_of("windows", "12th Gen Intel(R) Core(TM) i7-12700H"),
        "windows-12th-gen-intel-core-i7-12700h"
    );
    assert_eq!(
        class_of("linux", "AMD Ryzen 7 5800X 8-Core Processor @ 3.8GHz"),
        "linux-amd-ryzen-7-5800x-8-core-processor"
    );
    assert_eq!(r.date.len(), 10);
}

#[test]
fn the_rig_plays_a_session_and_names_where_the_time_went() {
    let p = sessions::scaled(10);
    let start = p.loop_region.as_ref().unwrap().start;
    let mut rig = Rig::new(&p, start, 256);
    let run = rig.run(20, 100);
    assert_eq!(run.block_us.len(), 100);
    assert!(run.voices > 0, "something is sounding");
    let heard = (0..20).any(|_| {
        rig.block();
        rig.output().iter().any(|x| x.abs() > 1e-4)
    });
    assert!(heard, "the session is heard");
    let spots = rig.hot_spots("s10", 5);
    assert_eq!(spots.len(), 5);
    assert!(spots.windows(2).all(|w| w[0].share_pct >= w[1].share_pct));
    assert!(spots.iter().all(|h| !h.node.is_empty()));
    assert!(
        spots.iter().any(|h| h.node.contains(" on ")),
        "an instrument or insert is named: {spots:?}"
    );
    let _ = Ticks::ZERO;
}

/// The null corpus (`hashes.rs`, compared across builds by `xtask null-hashes`): the same
/// bits twice in one process, one line per item with a unique name, and every effect
/// audible — a silent render would make two builds agree for nothing.
#[test]
fn the_null_corpus_is_repeatable_named_once_and_audible() {
    use cantus_bench::hashes::{effect_items, math_items, report};
    let math = math_items();
    assert_eq!(math.len(), 39);
    assert_eq!(math, math_items());
    let fx = effect_items();
    assert_eq!(fx.len(), 50);
    assert!(fx.iter().all(|i| i.peak > 1e-4 && i.samples == 2 * 96_000));
    let mut items = math;
    items.extend(fx);
    let text = report(&items);
    let lines: Vec<&str> = text.lines().filter(|l| !l.starts_with('#')).collect();
    assert_eq!(lines.len(), items.len());
    let mut names: Vec<&str> = lines
        .iter()
        .map(|l| l.rsplit("  ").next().unwrap())
        .collect();
    names.sort();
    names.dedup();
    assert_eq!(names.len(), items.len());
    assert!(text.starts_with("# cantus null hashes; build "));
    assert!(lines[0].split("  ").next().unwrap().len() == 64);
}

/// Sound review 2: the rig measures a Chorda that plays — its keyboard designed before the
/// rig is built (a unit no longer waits for it, and the held notes at tick 0 were dropped,
/// so `inst.chorda.v8` timed a silent piano).
#[test]
fn the_rig_measures_a_chorda_that_sounds_its_voices() {
    let spec =
        cantus_core::InstrumentSpec::Native(cantus_core::NativeSpec::new(InstrumentKind::Chorda));
    let p = sessions::instrument(spec, 8);
    let mut rig = Rig::new(&p, Ticks::ZERO, cantus_bench::kernels::BLOCK);
    let run = rig.run(
        cantus_bench::rig::SAMPLE_RATE as usize / cantus_bench::kernels::BLOCK / 4,
        8,
    );
    assert_eq!(run.voices, 8, "eight voices sounding");
}

/// Round 29's third review (finding 31): a row whose baseline is zero (a hit's place: 0
/// frames) regresses when it moves a unit, the runs apart — never "0 % worse" whatever.
#[test]
fn a_zero_baseline_is_an_absolute_bound() {
    let base = report(&[("warp.place", Better::Lower, [0.0, 0.0, 0.0])]);
    let now = report(&[("warp.place", Better::Lower, [2.0, 3.0, 3.0])]);
    let v = compare::compare(&now, &base, 25.0);
    assert_eq!(v[0].status, Status::Regressed, "{v:?}");
    assert!(!compare::failed(&compare::compare(&base, &base, 25.0)));
}
