//! `cargo run -p xtask -- perf`: the performance gate (round 20, `docs/perf/benchmark.md`
//! §6), and paired comparisons of two x86-64 levels' builds (round 22).
//!
//! ```text
//! cargo run -p xtask -- perf                    # quick suite, compared with the baseline
//! cargo run -p xtask -- perf --save             # quick suite, stored as the baseline
//! cargo run -p xtask -- perf --full             # everything, start-up and DAWbench too
//! cargo run -p xtask -- perf --only fx,engine   # some groups
//! cargo run -p xtask -- perf --runs 3           # runs per metric (the suite's own default otherwise)
//! cargo run -p xtask -- perf --pin 2            # Windows: run on logical CPU 2 only
//! cargo run -p xtask -- perf --threshold 30     # the gate's margin, percent (25)
//! cargo run -p xtask -- perf --class laptop-a   # the machine class (else detected)
//! cargo run -p xtask -- perf --cpu v3           # the bench built for x86-64-v3 (not gated)
//! cargo run -p xtask -- perf --ab v2,v3 --pairs 6 --full --only engine,fx --pin 4
//!                                               # paired A/B: the two builds alternate
//! cargo run -p xtask -- perf --ab v2,v3 --full --only engine --sizes 64 --runs 1 --pairs 12
//!                                               # only the 64-track sessions, short runs
//! ```
//!
//! Builds `cantus-bench` in release, runs it into `target/perf/latest.json`, and compares
//! that with `perf/baselines/<machine class>.json`. Exit status 1 when a gated metric
//! regressed beyond the threshold *and* outside the run-to-run noise; 0 when there is no
//! baseline for this class yet (it says how to make one).
//!
//! `--cpu` builds the bench for one level (`levels.rs`) into `target/perf/latest-<level>.json`
//! and gates nothing: the stored baselines are of the plain build. `--ab A,B` builds both
//! levels and runs them alternately, A B, B A, A B…, `--pairs` times (on a shared machine
//! the drift between two runs minutes apart is larger than the difference sought; a pair
//! is two runs back to back), then prints, per metric, each side's median of runs and
//! the median of the per-pair ratios B/A, into `target/perf/ab/summary.md` too.

use crate::levels;
use cantus_launch::Level;
use std::path::{Path, PathBuf};
use std::process::Command;

struct Options {
    full: bool,
    save: bool,
    only: Option<String>,
    runs: Option<String>,
    pin: Option<u32>,
    threshold: String,
    class: Option<String>,
    cpu: Option<Level>,
    ab: Option<(Level, Level)>,
    pairs: usize,
    /// Only the engine sessions of these track counts (`CANTUS_BENCH_SIZES`).
    sizes: Option<String>,
}

fn parse(args: &[String]) -> Options {
    let mut o = Options {
        full: false,
        save: false,
        only: None,
        runs: None,
        pin: None,
        threshold: "25".to_string(),
        class: None,
        cpu: None,
        ab: None,
        pairs: 4,
        sizes: None,
    };
    let level = |s: Option<&String>| {
        s.and_then(|s| Level::parse(s)).unwrap_or_else(|| {
            eprintln!("xtask perf: --cpu takes v2, v3 or v4");
            std::process::exit(2)
        })
    };
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--full" => o.full = true,
            "--save" => o.save = true,
            "--only" => o.only = it.next().cloned(),
            "--runs" => o.runs = it.next().cloned(),
            "--pin" => o.pin = it.next().and_then(|s| s.parse().ok()),
            "--threshold" => o.threshold = it.next().cloned().unwrap_or(o.threshold),
            "--class" => o.class = it.next().cloned(),
            "--cpu" => o.cpu = Some(level(it.next())),
            "--ab" => {
                let list: Vec<Option<Level>> = it
                    .next()
                    .map(|s| s.split(',').map(Level::parse).collect())
                    .unwrap_or_default();
                match list.as_slice() {
                    [Some(a), Some(b)] if a != b => o.ab = Some((*a, *b)),
                    _ => {
                        eprintln!("xtask perf: --ab takes two levels, e.g. v2,v3");
                        std::process::exit(2);
                    }
                }
            }
            "--sizes" => o.sizes = it.next().cloned(),
            "--pairs" => o.pairs = it.next().and_then(|s| s.parse().ok()).unwrap_or(o.pairs),
            other => {
                eprintln!("xtask perf: unknown argument {other:?}");
                std::process::exit(2);
            }
        }
    }
    o
}

/// Build the bench (and the program, for the start-up benchmark) plainly or for a level;
/// returns the bench's path.
fn build_bench(level: Option<Level>, triple: &str, o: &Options) -> PathBuf {
    let full = o.full
        && o.only
            .as_deref()
            .is_none_or(|g| g.split(',').any(|g| g.trim() == "startup"));
    let ws = crate::workspace();
    let mut build = match level {
        Some(l) => levels::cargo_build(l, triple),
        None => {
            let mut c = Command::new(env!("CARGO"));
            c.current_dir(&ws).args(["build", "--release"]);
            c
        }
    };
    build.args(["-p", "cantus-bench"]);
    if full {
        // The start-up benchmark starts the real executable.
        build.args(["-p", "cantus"]);
    }
    crate::run(&mut build);
    match level {
        Some(l) => levels::binary(l, triple, "cantus-bench"),
        None => crate::target_dir()
            .join("release")
            .join(format!("cantus-bench{}", std::env::consts::EXE_SUFFIX)),
    }
}

/// Run the bench into `json`, pinned to one logical CPU when asked.
fn run_bench(exe: &Path, o: &Options, json: &Path) {
    let mut bench_args: Vec<String> = vec![
        if o.full { "--full" } else { "--quick" }.into(),
        "--json".into(),
        json.display().to_string(),
    ];
    if let Some(only) = &o.only {
        bench_args.push("--only".into());
        bench_args.push(only.clone());
    }
    if let Some(runs) = &o.runs {
        bench_args.push("--runs".into());
        bench_args.push(runs.clone());
    }
    let mut cmd = match o.pin {
        // `start /AFFINITY` is the one way Windows offers to start a process on a given
        // core without code of our own; a hybrid CPU's P and E cores differ by 2×, and a
        // run that migrates between them measures the scheduler.
        #[cfg(windows)]
        Some(cpu) => {
            // `start` takes the window title as its first quoted argument, and an empty
            // one must reach cmd as a bare `""`, which `Command::arg` would escape.
            use std::os::windows::process::CommandExt;
            let mut c = Command::new("cmd");
            c.raw_arg(format!(
                "/C start \"\" /B /WAIT {}/AFFINITY {:X}",
                // Paired runs ask for High priority, as round 21's did: on a shared
                // machine a normal-priority bench gets ~80 % of its core.
                if o.ab.is_some() { "/HIGH " } else { "" },
                1u64 << cpu
            ))
            .arg(exe)
            .args(&bench_args);
            c
        }
        _ => {
            let mut c = Command::new(exe);
            c.args(&bench_args);
            c
        }
    };
    if let Some(c) = &o.class {
        cmd.env("CANTUS_PERF_CLASS", c);
    }
    if let Some(sizes) = &o.sizes {
        cmd.env("CANTUS_BENCH_SIZES", sizes);
    }
    crate::run(cmd.current_dir(crate::workspace()));
}

pub fn perf(args: &[String]) {
    let o = parse(args);
    if let Some((a, b)) = o.ab {
        return ab(&o, a, b);
    }
    let ws = crate::workspace();
    let target = crate::target_dir();
    let triple = levels::host_triple();
    let exe = build_bench(o.cpu, &triple, &o);
    let latest = target.join("perf").join(match o.cpu {
        Some(l) => format!("latest-{}.json", l.name()),
        None => "latest.json".into(),
    });
    let _ = std::fs::create_dir_all(latest.parent().expect("a folder"));
    run_bench(&exe, &o, &latest);
    if let Some(l) = o.cpu {
        eprintln!(
            "xtask perf: the {l} build's report is {}; the stored baselines are of the plain \
             build, so it is not gated (compare levels with --ab)",
            latest.display()
        );
        return;
    }

    let text = std::fs::read_to_string(&latest).expect("the bench wrote its report");
    let class = o.class.clone().unwrap_or_else(|| class_in(&text));
    let baseline = ws
        .join("perf")
        .join("baselines")
        .join(format!("{class}{}.json", if o.full { "-full" } else { "" }));
    if o.save {
        let _ = std::fs::create_dir_all(baseline.parent().expect("a folder"));
        std::fs::copy(&latest, &baseline).expect("store the baseline");
        eprintln!("xtask perf: stored {}", baseline.display());
        return;
    }
    if !baseline.exists() {
        eprintln!(
            "xtask perf: no baseline for machine class {class:?} ({}).\n\
             Make one on a quiet machine with `cargo run -p xtask -- perf --save`, \
             review it, and commit it.",
            baseline.display()
        );
        return;
    }
    let status = Command::new(&exe)
        .current_dir(&ws)
        .arg("compare")
        .arg(&latest)
        .arg(&baseline)
        .args(["--threshold", &o.threshold])
        .status()
        .expect("run the comparison");
    std::process::exit(status.code().unwrap_or(1));
}

/// The machine class a report names (`machine.class`).
fn class_in(text: &str) -> String {
    let v: serde_json::Value = serde_json::from_str(text).expect("the report is JSON");
    v["machine"]["class"]
        .as_str()
        .map(str::to_string)
        .unwrap_or_else(|| "unknown".into())
}

/// One metric of a report: id, unit, which way is better, median.
#[derive(Clone, Debug, PartialEq)]
pub struct Point {
    pub id: String,
    pub unit: String,
    pub lower_is_better: bool,
    pub median: f64,
}

/// The metrics of a bench report (`report.rs` in `cantus-bench`).
pub fn points(text: &str) -> Vec<Point> {
    let v: serde_json::Value = serde_json::from_str(text).expect("the report is JSON");
    v["metrics"]
        .as_array()
        .map(|ms| {
            ms.iter()
                .filter_map(|m| {
                    Some(Point {
                        id: m["id"].as_str()?.to_owned(),
                        unit: m["unit"].as_str().unwrap_or("").to_owned(),
                        lower_is_better: m["better"].as_str() != Some("higher"),
                        median: m["summary"]["median"].as_f64()?,
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

fn median(v: &mut [f64]) -> f64 {
    v.sort_by(|a, b| a.total_cmp(b));
    match v.len() {
        0 => f64::NAN,
        n if n % 2 == 1 => v[n / 2],
        n => 0.5 * (v[n / 2 - 1] + v[n / 2]),
    }
}

/// The paired table: per metric, A's and B's median of runs, the median, lowest and
/// highest per-pair ratio B/A, and in how many pairs B was better. `pairs[i]` is pair
/// `i`'s (A report, B report).
pub fn ab_table(a: Level, b: Level, pairs: &[(Vec<Point>, Vec<Point>)]) -> String {
    let mut out = format!(
        "| Metric | Unit | {a} | {b} | {b}/{a} (median of pairs) | Range | {b} better in |\n\
         |---|---|---:|---:|---:|---:|---:|\n"
    );
    let Some((first, _)) = pairs.first() else {
        return out;
    };
    for p in first {
        let mut av = Vec::new();
        let mut bv = Vec::new();
        let mut ratios = Vec::new();
        let mut better = 0;
        for (ra, rb) in pairs {
            let (Some(x), Some(y)) = (
                ra.iter().find(|q| q.id == p.id),
                rb.iter().find(|q| q.id == p.id),
            ) else {
                continue;
            };
            av.push(x.median);
            bv.push(y.median);
            // A kernel under the timer's resolution reads 0 in some runs: no ratio.
            if x.median > 0.0 && y.median > 0.0 && x.median.is_finite() && y.median.is_finite() {
                ratios.push(y.median / x.median);
            }
            if (p.lower_is_better && y.median < x.median)
                || (!p.lower_is_better && y.median > x.median)
            {
                better += 1;
            }
        }
        if ratios.is_empty() {
            continue;
        }
        let n = av.len();
        let (lo, hi) = ratios
            .iter()
            .fold((f64::INFINITY, f64::NEG_INFINITY), |(l, h), r| {
                (l.min(*r), h.max(*r))
            });
        out.push_str(&format!(
            "| `{}` | {} | {:.1} | {:.1} | **{:.3}×** | {:.2}–{:.2} | {better} of {n} |\n",
            p.id,
            p.unit,
            median(&mut av),
            median(&mut bv),
            median(&mut ratios),
            lo,
            hi,
        ));
    }
    out
}

/// `--ab A,B`: build both levels, run them alternately, print the paired table.
fn ab(o: &Options, a: Level, b: Level) {
    for l in [a, b] {
        if !levels::host_runs(l) {
            eprintln!(
                "xtask perf: this machine lacks {} and cannot run the {l} build",
                l.missing(&cantus_launch::detected).join(", ")
            );
            std::process::exit(2);
        }
    }
    let triple = levels::host_triple();
    let exe_a = build_bench(Some(a), &triple, o);
    let exe_b = build_bench(Some(b), &triple, o);
    let dir = crate::target_dir().join("perf").join("ab");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("make target/perf/ab");
    let mut pairs = Vec::new();
    for i in 0..o.pairs.max(1) {
        let ja = dir.join(format!("{}-{i}.json", a.short()));
        let jb = dir.join(format!("{}-{i}.json", b.short()));
        // A B, then B A: the drift within a pair falls on each side equally often.
        if i % 2 == 0 {
            run_bench(&exe_a, o, &ja);
            run_bench(&exe_b, o, &jb);
        } else {
            run_bench(&exe_b, o, &jb);
            run_bench(&exe_a, o, &ja);
        }
        let read = |p: &Path| points(&std::fs::read_to_string(p).expect("the bench wrote it"));
        pairs.push((read(&ja), read(&jb)));
        eprintln!("xtask perf: pair {} of {} done", i + 1, o.pairs.max(1));
    }
    let table = ab_table(a, b, &pairs);
    let header = format!(
        "{} pairs, alternating ({a} first in even pairs); {}{}{}\n\n",
        pairs.len(),
        if o.full { "full suite" } else { "quick suite" },
        o.only
            .as_deref()
            .map(|g| format!(", groups {g}"))
            .unwrap_or_default(),
        o.pin
            .map(|c| format!(", pinned to logical CPU {c}"))
            .unwrap_or_default(),
    );
    let text = format!("{header}{table}");
    print!("{text}");
    std::fs::write(dir.join("summary.md"), &text).expect("write the summary");
    eprintln!("xtask perf: reports and summary in {}", dir.display());
}
