//! `cantus-bench` — run the suite, compare a run with a baseline, print a report.
//!
//! ```text
//! cantus-bench [--quick | --full] [--only fx,inst,engine,render,open,ui,dawbench,startup]
//!              [--json <file>]
//! cantus-bench compare <current.json> <baseline.json> [--threshold <percent>]
//! cantus-bench table <report.json>
//! cantus-bench hashes [--whole] [--out <file>]  # the null corpus as SHA-256s (hashes.rs)
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
        Some("hashes") => hashes_cmd(&args[1..]),
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

/// `hashes [--whole] [--out <file>]`: render the null corpus and print (or write) its
/// hashes; `--whole` renders the demo songs whole.
fn hashes_cmd(args: &[String]) -> ExitCode {
    let mut out = None;
    let mut whole = false;
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match (a.as_str(), it.clone().next()) {
            ("--whole", _) => whole = true,
            ("--out", Some(path)) => {
                out = Some(PathBuf::from(path));
                it.next();
            }
            _ => {
                eprintln!("usage: cantus-bench hashes [--whole] [--out <file>]");
                return ExitCode::from(2);
            }
        }
    }
    let t = std::time::Instant::now();
    let items = cantus_bench::hashes::corpus(whole);
    let text = cantus_bench::hashes::report(&items);
    eprintln!(
        "cantus-bench: {} items hashed in {:.1} s ({})",
        items.len(),
        t.elapsed().as_secs_f64(),
        cantus_bench::hashes::build_level()
    );
    match out {
        Some(path) => {
            if let Err(e) = std::fs::write(&path, text) {
                eprintln!("cantus-bench: could not write {}: {e}", path.display());
                return ExitCode::FAILURE;
            }
        }
        None => print!("{text}"),
    }
    ExitCode::SUCCESS
}
