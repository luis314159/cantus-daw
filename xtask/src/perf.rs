//! `cargo run -p xtask -- perf`: the performance gate (round 20, `docs/perf/benchmark.md`
//! §6).
//!
//! ```text
//! cargo run -p xtask -- perf                    # quick suite, compared with the baseline
//! cargo run -p xtask -- perf --save             # quick suite, stored as the baseline
//! cargo run -p xtask -- perf --full             # everything, start-up and DAWbench too
//! cargo run -p xtask -- perf --only fx,engine   # some groups
//! cargo run -p xtask -- perf --pin 2            # Windows: run on logical CPU 2 only
//! cargo run -p xtask -- perf --threshold 30     # the gate's margin, percent (25)
//! cargo run -p xtask -- perf --class laptop-a   # the machine class (else detected)
//! ```
//!
//! Builds `cantus-bench` in release, runs it into `target/perf/latest.json`, and compares
//! that with `perf/baselines/<machine class>.json`. Exit status 1 when a gated metric
//! regressed beyond the threshold *and* outside the run-to-run noise; 0 when there is no
//! baseline for this class yet (it says how to make one).

use std::process::Command;

pub fn perf(args: &[String]) {
    let mut full = false;
    let mut save = false;
    let mut only: Option<String> = None;
    let mut pin: Option<u32> = None;
    let mut threshold = "25".to_string();
    let mut class: Option<String> = None;
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--full" => full = true,
            "--save" => save = true,
            "--only" => only = it.next().cloned(),
            "--pin" => pin = it.next().and_then(|s| s.parse().ok()),
            "--threshold" => threshold = it.next().cloned().unwrap_or(threshold),
            "--class" => class = it.next().cloned(),
            other => {
                eprintln!("xtask perf: unknown argument {other:?}");
                std::process::exit(2);
            }
        }
    }
    let ws = crate::workspace();
    let target = crate::target_dir();
    let mut build = Command::new(env!("CARGO"));
    build
        .current_dir(&ws)
        .args(["build", "--release", "-p", "cantus-bench"]);
    if full {
        // The start-up benchmark starts the real executable.
        build.args(["-p", "cantus"]);
    }
    crate::run(&mut build);

    let exe = target
        .join("release")
        .join(format!("cantus-bench{}", std::env::consts::EXE_SUFFIX));
    let latest = target.join("perf").join("latest.json");
    let _ = std::fs::create_dir_all(latest.parent().expect("a folder"));
    let mut bench_args: Vec<String> = vec![
        if full { "--full" } else { "--quick" }.into(),
        "--json".into(),
        latest.display().to_string(),
    ];
    if let Some(o) = only {
        bench_args.push("--only".into());
        bench_args.push(o);
    }
    let mut cmd = match pin {
        // `start /AFFINITY` is the one way Windows offers to start a process on a given
        // core without code of our own; a hybrid CPU's P and E cores differ by 2×, and a
        // run that migrates between them measures the scheduler.
        Some(cpu) if cfg!(windows) => {
            let mut c = Command::new("cmd");
            c.args(["/C", "start", "\"\"", "/B", "/WAIT", "/AFFINITY"])
                .arg(format!("{:X}", 1u64 << cpu))
                .arg(&exe)
                .args(&bench_args);
            c
        }
        _ => {
            let mut c = Command::new(&exe);
            c.args(&bench_args);
            c
        }
    };
    if let Some(c) = &class {
        cmd.env("CANTUS_PERF_CLASS", c);
    }
    crate::run(cmd.current_dir(&ws));

    let text = std::fs::read_to_string(&latest).expect("the bench wrote its report");
    let class = class.unwrap_or_else(|| class_in(&text));
    let baseline = ws
        .join("perf")
        .join("baselines")
        .join(format!("{class}{}.json", if full { "-full" } else { "" }));
    if save {
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
        .args(["--threshold", &threshold])
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
