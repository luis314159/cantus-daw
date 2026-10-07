//! `cantus-bench` — run the suite, compare a run with a baseline, print a report.
//!
//! ```text
//! cantus-bench [--quick | --full] [--only fx,inst,engine,render,open,ui,dawbench,startup]
//!              [--json <file>]
//! cantus-bench compare <current.json> <baseline.json> [--threshold <percent>]
//! cantus-bench table <report.json>
//! cantus-bench paced [--frames 256] [--seconds 120] [--songs startup,house,…]
//!                    [--workers auto|N] [--promote 1|0] [--spin 0|1] [--share 0.1] [--callback mmcss|normal]
//! ```
//!
//! Always run it in release (`cargo run --release -p cantus-bench`, or through
//! `cargo run -p xtask -- perf`, which does).

use cantus_bench::report::Report;
use cantus_bench::{compare, Plan};
use std::path::PathBuf;
use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("compare") => compare_cmd(&args[1..]),
        Some("table") => table_cmd(&args[1..]),
        Some("paced") => paced_cmd(&args[1..]),
        _ => run_cmd(&args),
    }
}

fn run_cmd(args: &[String]) -> ExitCode {
    let mut plan = Plan::QUICK;
    let mut only = Vec::new();
    let mut json: Option<PathBuf> = None;
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--quick" => plan = Plan::QUICK,
            "--full" => plan = Plan::FULL,
            "--only" => {
                only = it
                    .next()
                    .map(|s| s.split(',').map(|x| x.trim().to_string()).collect())
                    .unwrap_or_default()
            }
            "--json" => json = it.next().map(PathBuf::from),
            "--runs" => {
                if let Some(n) = it.next().and_then(|s| s.parse().ok()) {
                    plan.runs = n;
                }
            }
            other => {
                eprintln!("cantus-bench: unknown argument {other:?}");
                return ExitCode::from(2);
            }
        }
    }
    if cfg!(debug_assertions) {
        eprintln!("cantus-bench: this is a debug build; its numbers mean nothing. Use --release.");
    }
    for g in &only {
        if !cantus_bench::GROUPS.contains(&g.as_str()) {
            eprintln!(
                "cantus-bench: no group {g:?}; the groups are {:?}",
                cantus_bench::GROUPS
            );
            return ExitCode::from(2);
        }
    }
    let report = cantus_bench::run(&plan, &only);
    print!("{}", report.table(""));
    if !report.hot_spots.is_empty() {
        println!("\n| Session | Node | Share | µs/block |\n|---|---|---:|---:|");
        for h in &report.hot_spots {
            println!(
                "| {} | {} | {:.1} % | {:.1} |",
                h.session, h.node, h.share_pct, h.us_per_block
            );
        }
    }
    if let Some(path) = json {
        if let Err(e) = report.save(&path) {
            eprintln!("cantus-bench: could not write {}: {e}", path.display());
            return ExitCode::FAILURE;
        }
        eprintln!("cantus-bench: wrote {}", path.display());
    }
    ExitCode::SUCCESS
}

fn compare_cmd(args: &[String]) -> ExitCode {
    let mut files = Vec::new();
    let mut threshold = 25.0;
    let mut it = args.iter();
    while let Some(a) = it.next() {
        if a == "--threshold" {
            threshold = it.next().and_then(|s| s.parse().ok()).unwrap_or(threshold);
        } else {
            files.push(PathBuf::from(a));
        }
    }
    let [current, baseline] = files.as_slice() else {
        eprintln!(
            "usage: cantus-bench compare <current.json> <baseline.json> [--threshold <percent>]"
        );
        return ExitCode::from(2);
    };
    let (c, b) = match (Report::load(current), Report::load(baseline)) {
        (Ok(c), Ok(b)) => (c, b),
        (Err(e), _) | (_, Err(e)) => {
            eprintln!("cantus-bench: {e}");
            return ExitCode::from(2);
        }
    };
    if c.machine.class != b.machine.class {
        eprintln!(
            "cantus-bench: the baseline is for {:?}, this run is {:?}: not comparable",
            b.machine.class, c.machine.class
        );
        return ExitCode::from(2);
    }
    let v = compare::compare(&c, &b, threshold);
    print!("{}", compare::describe(&v));
    if compare::failed(&v) {
        eprintln!("cantus-bench: performance regressed beyond {threshold} % (see REGRESS above)");
        ExitCode::FAILURE
    } else {
        eprintln!("cantus-bench: no regression beyond {threshold} %");
        ExitCode::SUCCESS
    }
}

fn table_cmd(args: &[String]) -> ExitCode {
    let Some(path) = args.first() else {
        eprintln!("usage: cantus-bench table <report.json>");
        return ExitCode::from(2);
    };
    match Report::load(&PathBuf::from(path)) {
        Ok(r) => {
            print!("{}", r.table(""));
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("cantus-bench: {e}");
            ExitCode::from(2)
        }
    }
}

/// The real processor at the device period: every demo and the startup song, a miss
/// counted whenever a block ends after its period (`cantus_bench::paced`).
fn paced_cmd(args: &[String]) -> ExitCode {
    let (mut frames, mut seconds) = (256usize, 120.0f64);
    let mut only: Vec<String> = Vec::new();
    let mut workers = cantus_engine::Workers::auto();
    let (mut promote, mut spin, mut callback) = (true, false, true);
    let mut share = 0.1f32;
    let mut it = args.iter();
    while let Some(a) = it.next() {
        let v = it.next();
        match (a.as_str(), v) {
            ("--frames", Some(v)) => frames = v.parse().unwrap_or(frames),
            ("--seconds", Some(v)) => seconds = v.parse().unwrap_or(seconds),
            ("--songs", Some(v)) => only = v.split(',').map(|s| s.trim().to_string()).collect(),
            ("--promote", Some(v)) => promote = v == "1",
            ("--spin", Some(v)) => spin = v == "1",
            ("--share", Some(v)) => share = v.parse().unwrap_or(share),
            ("--callback", Some(v)) => callback = v == "mmcss",
            ("--workers", Some(v)) => {
                workers = match v.as_str() {
                    "auto" => cantus_engine::Workers::auto(),
                    n => cantus_engine::Workers::live(n.parse().unwrap_or(0)),
                }
            }
            _ => {
                eprintln!("cantus-bench paced: unknown argument {a:?}");
                return ExitCode::from(2);
            }
        }
    }
    workers.promote = promote;
    workers.spin = spin;
    workers.spin_share = share;
    println!(
        "paced: {frames} frames ({:.2} ms), {seconds} s per song, {} workers (promoted: {promote}, spinning: {spin}, spin share {share}), callback {}
",
        frames as f64 / 48.0,
        workers.count(),
        if callback { "in MMCSS" } else { "at normal priority" }
    );
    println!("{}", cantus_bench::paced::HEADER);
    let mut misses = 0;
    for (name, p) in cantus_bench::paced::songs() {
        if !only.is_empty() && !only.contains(&name) {
            continue;
        }
        let r = cantus_bench::paced::run(&name, &p, frames, seconds, workers.clone(), callback);
        misses += r.misses;
        println!("{}", cantus_bench::paced::row(&r));
    }
    if misses == 0 {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}
