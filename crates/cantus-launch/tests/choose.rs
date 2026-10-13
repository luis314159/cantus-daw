//! The launcher's choice, on made-up processors and folders; the levels' feature lists
//! against what rustc itself enables for each `target-cpu`.

// Most of these tests choose between x86-64 levels and run only on x86-64; elsewhere
// (Apple silicon in CI) their helpers and imports are unused.
#![cfg_attr(not(target_arch = "x86_64"), allow(unused_imports, dead_code))]

use cantus_launch::{choose, cpu_level, info, refusal_text, Choice, Level, Refusal, DETECT};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

/// A processor that has exactly `level`'s features (or the bare SSE2 baseline).
fn cpu(level: Option<Level>) -> impl Fn(&str) -> bool {
    let have: BTreeSet<&'static str> = match level {
        Some(l) => l.features().into_iter().collect(),
        None => ["sse", "sse2", "fxsr", "cmpxchg16b", "sse3"].into(),
    };
    move |f: &str| have.contains(f)
}

/// A folder holding `programs` (by level).
fn folder(programs: &[Level]) -> impl Fn(&Path) -> bool {
    let names: Vec<String> = programs.iter().map(|l| l.program()).collect();
    move |p: &Path| {
        p.parent() == Some(Path::new("app"))
            && names
                .iter()
                .any(|n| p.file_name().is_some_and(|f| f == n.as_str()))
    }
}

fn pick(
    cpu_has: Option<Level>,
    programs: &[Level],
    forced: Option<&str>,
) -> Result<Choice, Refusal> {
    choose(Path::new("app"), &cpu(cpu_has), &folder(programs), forced)
}

fn level(r: Result<Choice, Refusal>) -> Level {
    r.expect("a program").level.expect("a level")
}

#[cfg(target_arch = "x86_64")]
#[test]
fn the_highest_level_the_processor_runs_and_the_folder_holds_is_started() {
    use Level::*;
    let release = [V2, V3];
    assert_eq!(level(pick(Some(V3), &release, None)), V3);
    assert_eq!(level(pick(Some(V2), &release, None)), V2);
    // An AVX-512 machine without the --v4 program runs v3.
    assert_eq!(level(pick(Some(V4), &release, None)), V3);
    assert_eq!(level(pick(Some(V4), &[V2, V3, V4], None)), V4);
    // A Haswell-class machine never gets the v4 program.
    assert_eq!(level(pick(Some(V3), &[V2, V3, V4], None)), V3);
    // A program gone missing: the next one down.
    assert_eq!(level(pick(Some(V3), &[V2], None)), V2);
    let c = pick(Some(V3), &release, None).unwrap();
    assert_eq!(c.program, PathBuf::from("app").join(V3.program()));
    assert_eq!(c.note, None);
}

#[cfg(target_arch = "x86_64")]
#[test]
fn cantus_cpu_steps_down_but_never_up() {
    use Level::*;
    let release = [V2, V3];
    let c = pick(Some(V3), &release, Some("v2")).unwrap();
    assert_eq!((c.level, c.note), (Some(V2), None));
    assert_eq!(level(pick(Some(V3), &release, Some("x86-64-v2"))), V2);
    assert_eq!(level(pick(Some(V3), &release, Some(" 2 "))), V2);
    assert_eq!(level(pick(Some(V3), &release, Some(""))), V3);
    // Up would crash on the first missing instruction: refused, with the reason.
    let c = pick(Some(V2), &release, Some("v3")).unwrap();
    assert_eq!(c.level, Some(V2));
    let note = c.note.unwrap();
    assert!(
        note.contains("avx2") && note.contains("x86-64-v3"),
        "{note}"
    );
    // Nonsense and a missing program are noted, and the automatic choice stands.
    let c = pick(Some(V3), &release, Some("fast")).unwrap();
    assert_eq!(c.level, Some(V3));
    assert!(c.note.unwrap().contains("not a level"));
    let c = pick(Some(V4), &release, Some("v4")).unwrap();
    assert_eq!(c.level, Some(V3));
    assert!(c.note.unwrap().contains("is not here"));
}

#[cfg(target_arch = "x86_64")]
#[test]
fn a_processor_below_v2_or_an_empty_folder_is_refused_in_words() {
    use Level::*;
    let r = pick(None, &[V2, V3], None).unwrap_err();
    let Refusal::CpuTooOld { missing } = &r else {
        panic!("{r:?}")
    };
    assert!(missing.contains(&"sse4.2") && missing.contains(&"popcnt"));
    assert!(!missing.contains(&"sse2"));
    let text = refusal_text(&r);
    assert!(text.contains("needs a processor") && text.contains("necesita un procesador"));
    assert!(text.contains("sse4.2"));

    let r = pick(Some(V3), &[], None).unwrap_err();
    let Refusal::NoProgram { looked_for } = &r else {
        panic!("{r:?}")
    };
    // Both programs this processor could run, highest first.
    assert_eq!(
        looked_for,
        &[
            PathBuf::from("app").join(V3.program()),
            PathBuf::from("app").join(V2.program()),
        ]
    );
    let text = refusal_text(&r);
    assert!(text.contains("Install Cantus again") && text.contains("Vuelve a instalar"));
}

#[test]
fn levels_have_one_name_each() {
    for l in Level::ALL {
        assert_eq!(Level::parse(l.name()), Some(l));
        assert_eq!(Level::parse(l.short()), Some(l));
        assert_eq!(Level::parse(&l.short().to_uppercase()), Some(l));
        assert!(l.program().starts_with(&format!("cantus-{}", l.name())));
        assert_eq!(l.rustflags(), format!("-C target-cpu={}", l.name()));
    }
    assert_eq!(
        Level::V3.program(),
        format!("cantus-x86-64-v3{}", std::env::consts::EXE_SUFFIX)
    );
    assert_eq!(Level::parse("v5"), None);
    assert_eq!(Level::parse("x86-64"), None);
    assert!(Level::V2 < Level::V3 && Level::V3 < Level::V4);
    // Each level holds the one below it.
    let set = |l: Level| l.features().into_iter().collect::<BTreeSet<_>>();
    assert!(set(Level::V2).is_subset(&set(Level::V3)));
    assert!(set(Level::V3).is_subset(&set(Level::V4)));
    assert_eq!(cpu_level(&cpu(Some(Level::V3))), Some(Level::V3));
    assert_eq!(cpu_level(&cpu(None)), None);
}

/// The launcher must never start a program built with a feature it did not check: each
/// level's list is exactly what `rustc --print cfg -C target-cpu=<level>` turns on, and
/// every name has a run-time test.
#[cfg(target_arch = "x86_64")]
#[test]
fn the_feature_lists_are_rustcs_own() {
    let rustc = std::env::var("RUSTC").unwrap_or_else(|_| "rustc".into());
    for l in Level::ALL {
        let out = std::process::Command::new(&rustc)
            .args(["--print", "cfg", "-C"])
            .arg(format!("target-cpu={}", l.name()))
            .output()
            .expect("run rustc");
        assert!(out.status.success(), "rustc --print cfg failed");
        let text = String::from_utf8(out.stdout).unwrap();
        let from_rustc: BTreeSet<String> = text
            .lines()
            .filter_map(|line| line.strip_prefix("target_feature=\""))
            .filter_map(|rest| rest.strip_suffix('"'))
            // Features that are not instructions the program could execute.
            .filter(|f| !["crt-static"].contains(f))
            .map(str::to_owned)
            .collect();
        let ours: BTreeSet<String> = l.features().into_iter().map(str::to_owned).collect();
        assert_eq!(ours, from_rustc, "{l}");
        for f in l.features() {
            assert!(
                DETECT.iter().any(|(name, _)| *name == f),
                "{f} has no run-time test"
            );
        }
    }
    // Every Windows 11 machine is at least v2 (24H2 requires SSE4.2 and POPCNT).
    assert!(cpu_level(&cantus_launch::detected).is_some());
}

#[test]
fn the_info_names_what_would_start() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path().to_path_buf();
    std::fs::write(dir.join(Level::V2.program()), b"MZ").unwrap();
    let text = info(&dir, &cpu(Some(Level::V3)), None);
    assert!(text.contains("processor: x86-64-v3"), "{text}");
    assert!(text.contains("lacks for x86-64-v4: avx512"), "{text}");
    assert!(text.contains(&format!("programs here: {}", Level::V2.program())));
    assert!(text.contains(&format!(
        "starts: {}",
        dir.join(Level::V2.program()).display()
    )));
}
