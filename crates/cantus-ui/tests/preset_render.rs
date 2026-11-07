//! The preset render check (round 37, ADR 0101 §5): every factory preset of every unit
//! that sounds, rendered on its role's phrase (an instrument, a kit) or on its role's fixed
//! source (an effect, on the master of a small project made by Cantus's own instruments) —
//! the material the browser auditions on (`preset_audition`).
//!
//! Each render: **no sample that is not a number, no silence, no clipping** (sample peak at
//! or under 0 dBFS), its **loudness inside its role's window** (`preset_measure`: an
//! instrument within ±2 LU of its role's target, an effect within ±2 LU of its dry source; a
//! limiter, a maximizer and a clipper make level by design) and its **CPU under its unit's
//! budget**, in units of the run's own median (the machine's speed and its other load divide
//! out): in a release build alone and best of two; in a debug build (what CI runs) from the
//! parallel pass, with twice the slack. A preset under its window because louder would pass
//! its velocity-127 headroom (the headroom test's dense riff) is limited by its dynamics, not
//! out of level, and the check says so.
//!
//! What fails today is listed check by check in `tests/data/preset_render_known.txt` with
//! what was measured. A failure not listed fails; **a listed one that got worse** (level by
//! more than 1 LU, a clip by more than 0.5 dB, CPU by more than a quarter) fails; a listed one
//! of a unit the library owns that is gone fails until it is struck off.
//! `CANTUS_PRESET_KNOWN_DUMP=1` rewrites the list from a run (in release, so CPU is measured);
//! `cargo test --release -p cantus-ui --test preset_render -- --nocapture` prints the table.

use cantus_core::{Project, Snapshot};
use cantus_engine::{render, RenderOptions, Rendered};
use cantus_ui::preset_audition::{self as aud, Target};
use cantus_ui::preset_catalog::{entries, units, Entry, Unit};
use cantus_ui::preset_measure::{
    budget, by_design, levels_by_design, lufs, target, HEADROOM_DB, WINDOW,
};
use cantus_ui::preset_tags::Role;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;

const KNOWN: &str = include_str!("data/preset_render_known.txt");
const KNOWN_PATH: &str = "tests/data/preset_render_known.txt";

/// The median share of a core a phrase took in a release build when the budgets were set
/// (round 37: 0.024). A debug build holds each preset to `DEBUG_SLACK` × its budget over this
/// median, in units of its own run's median.
const RELEASE_MEDIAN: f64 = 0.024;
const DEBUG_SLACK: f64 = 2.0;

/// How much worse a listed failure may get before the test fails.
const WORSE_LEVEL: f64 = 1.0;
const WORSE_CLIP: f64 = 0.5;
const WORSE_CPU: f64 = 1.25;

struct Measured {
    unit: Unit,
    id: String,
    role: Role,
    finite: bool,
    peak_db: f64,
    lufs: f64,
    /// LU off the window's centre.
    off: f64,
    cpu: f64,
    /// An instrument's peak on the headroom test's dense riff, dBFS.
    riff_db: Option<f64>,
}

fn db(x: f32) -> f64 {
    20.0 * f64::from(x.max(1e-9)).log10()
}

fn render_project(
    p: &Project,
    start: cantus_core::Ticks,
    end: cantus_core::Ticks,
) -> (Rendered, f64) {
    let snap = Arc::new(Snapshot::compile(p, 0));
    let t0 = Instant::now();
    let r = render(
        snap,
        &RenderOptions {
            start,
            end: Some(end),
            tail_seconds: aud::TAIL,
            compensate_latency: true,
            ..Default::default()
        },
    )
    .expect("it renders");
    let took = t0.elapsed().as_secs_f64();
    let secs = r.frames() as f64 / f64::from(r.sample_rate);
    (r, took / secs.max(1e-9))
}

fn par_map<T: Sync, R: Send>(items: &[T], f: impl Fn(&T) -> R + Sync) -> Vec<R> {
    let threads = std::thread::available_parallelism()
        .map_or(4, |n| n.get())
        .min(8);
    let chunk = items.len().div_ceil(threads).max(1);
    std::thread::scope(|scope| {
        let handles: Vec<_> = items
            .chunks(chunk)
            .map(|c| {
                let f = &f;
                scope.spawn(move || c.iter().map(f).collect::<Vec<R>>())
            })
            .collect();
        handles
            .into_iter()
            .flat_map(|h| h.join().expect("a render panicked"))
            .collect()
    })
}

fn every_audible_preset() -> Vec<Entry> {
    cantus_ui::i18n::set_thread_locale(cantus_ui::i18n::Locale::En);
    units()
        .into_iter()
        .filter(|u| u.audible())
        .flat_map(|u| entries(u, false))
        .collect()
}

fn loudness(r: &Rendered) -> f64 {
    lufs(&r.samples, r.channels as usize, r.sample_rate as f32)
}

fn measure_all(all: &[Entry]) -> Vec<Measured> {
    let empty = Project::new("none");
    let mut roles: Vec<Role> = all
        .iter()
        .filter(|e| !e.unit.is_instrument())
        .map(Entry::role)
        .collect();
    roles.sort();
    roles.dedup();
    let dry: HashMap<Role, f64> = par_map(&roles, |role| {
        let p = aud::source_project(*role);
        let end = cantus_core::Ticks::from_beats_f64(aud::BARS * 4.0);
        let (r, _) = render_project(&p, cantus_core::Ticks::ZERO, end);
        (*role, loudness(&r))
    })
    .into_iter()
    .collect();
    par_map(all, |e| {
        cantus_ui::i18n::set_thread_locale(cantus_ui::i18n::Locale::En);
        let prepared = aud::prepare(&empty, Target::None, e, None, aud::TEMPO);
        let (r, cpu) = render_project(&prepared.project, prepared.start, prepared.end);
        let riff_db = match &e.payload {
            cantus_ui::preset_catalog::Payload::Native(spec) => {
                let p = aud::riff_project(spec);
                let end = cantus_core::Ticks::from_beats_f64(aud::RIFF_BARS * 4.0);
                let (riff, _) = render_project(&p, cantus_core::Ticks::ZERO, end);
                Some(db(riff.peak))
            }
            _ => None,
        };
        let l = loudness(&r);
        let level = if e.unit.is_instrument() {
            l
        } else {
            l - dry[&e.role()]
        };
        Measured {
            unit: e.unit,
            id: e.id.clone(),
            role: e.role(),
            finite: r.samples.iter().all(|x| x.is_finite()),
            peak_db: db(r.peak),
            lufs: l,
            off: level - target(e),
            cpu,
            riff_db,
        }
    })
}

/// Each preset's share of a core, alone and after a first render (a unit's tables are
/// designed on first use), best of two — a release build's measure — and best of five for a
/// preset near its budget: another program's burst on a shared machine is not the preset's
/// cost (the CPU tests' rule since round 23).
fn cpu_alone(all: &[Entry]) -> HashMap<String, f64> {
    let empty = Project::new("none");
    all.iter()
        .map(|e| {
            let p = aud::prepare(&empty, Target::None, e, None, aud::TEMPO);
            let _ = render_project(&p.project, p.start, p.end);
            let mut best = f64::INFINITY;
            for i in 0..5 {
                if i >= 2 && best <= 0.8 * budget(e.unit) {
                    break;
                }
                best = best.min(render_project(&p.project, p.start, p.end).1);
            }
            (e.id.clone(), best)
        })
        .collect()
}

/// `id`'s cost alone over the cost alone of the preset at the run's median (best of three
/// each): a debug build's second look at a preset the parallel pass put over its budget.
fn alone_against_median(all: &[Entry], id: &str, rows: &[Measured], median: f64) -> f64 {
    let reference = rows
        .iter()
        .min_by(|a, b| (a.cpu - median).abs().total_cmp(&(b.cpu - median).abs()))
        .map(|m| m.id.clone())
        .unwrap_or_default();
    let empty = Project::new("none");
    let cost = |id: &str| {
        let Some(e) = all.iter().find(|e| e.id == id) else {
            return 1.0;
        };
        let p = aud::prepare(&empty, Target::None, e, None, aud::TEMPO);
        let _ = render_project(&p.project, p.start, p.end);
        (0..3)
            .map(|_| render_project(&p.project, p.start, p.end).1)
            .fold(f64::INFINITY, f64::min)
    };
    cost(id) / cost(&reference).max(1e-9)
}

fn known() -> HashMap<(String, String), f64> {
    KNOWN
        .lines()
        .filter(|l| !l.trim().is_empty() && !l.starts_with('#'))
        .map(|l| {
            let mut w = l.split_whitespace();
            let id = w.next().unwrap_or_default().to_string();
            let check = w.next().unwrap_or_default().to_string();
            let v = w.next().and_then(|v| v.parse().ok()).unwrap_or(0.0);
            ((id, check), v)
        })
        .collect()
}

/// Whether `now` is worse than the listed `then` by more than the check's tolerance.
fn worse(check: &str, then: f64, now: f64) -> bool {
    match check {
        "level" => now.abs() > then.abs() + WORSE_LEVEL,
        "clip" => now > then + WORSE_CLIP,
        "cpu" => now > then * WORSE_CPU,
        _ => false,
    }
}

#[test]
fn every_factory_preset_renders_clean_level_and_within_its_budget() {
    let all = every_audible_preset();
    let mut rows = measure_all(&all);
    let release = !cfg!(debug_assertions);
    if release {
        let cpu = cpu_alone(&all);
        for m in &mut rows {
            m.cpu = cpu[&m.id];
        }
    }
    let mut sorted: Vec<f64> = rows.iter().map(|m| m.cpu).collect();
    sorted.sort_by(f64::total_cmp);
    let median = sorted
        .get(sorted.len() / 2)
        .copied()
        .unwrap_or(1.0)
        .max(1e-9);
    // A release build too is held in units of its run's median (scaled to the median when
    // the budgets were set): the machine's speed and its other load divide out — the
    // builders share it, and a busy machine made every preset look heavier.
    if release {
        for m in &mut rows {
            m.cpu = m.cpu / median * RELEASE_MEDIAN;
        }
    }
    let known = known();
    let mut failing: Vec<(String, &'static str, f64, Unit)> = Vec::new();
    println!("| Unit | Preset | Role | LUFS | Off its target | Peak dBFS | CPU |");
    println!("|---|---|---|---|---|---|---|");
    for m in &rows {
        println!(
            "| {:?} | {} | {} | {:.1} | {:+.1} | {:.1} | {:.3} |",
            m.unit,
            m.id,
            m.role.id(),
            m.lufs,
            m.off,
            m.peak_db,
            m.cpu
        );
        assert!(m.finite, "{}: a sample that is not a number", m.id);
        if m.lufs < -60.0 || m.peak_db < -60.0 {
            failing.push((m.id.clone(), "silent", m.lufs, m.unit));
        }
        if m.peak_db > 0.0 {
            failing.push((m.id.clone(), "clip", m.peak_db, m.unit));
        }
        // Under its window because louder would pass the velocity-127 headroom: limited by
        // its own dynamics, not out of level.
        let limited = m.off < -WINDOW && m.riff_db.is_some_and(|p| p > HEADROOM_DB - 1.0);
        if limited {
            println!(
                "{}: limited by its headroom ({:+.1} LU, {:.1} dBFS on the dense riff)",
                m.id,
                m.off,
                m.riff_db.unwrap_or(0.0)
            );
        }
        let exempt = levels_by_design(m.unit) || by_design(&m.id).is_some() || limited;
        if !exempt && m.off.abs() > WINDOW && m.lufs >= -60.0 {
            failing.push((m.id.clone(), "level", m.off, m.unit));
        }
        if release {
            if m.cpu > budget(m.unit) {
                // A second look, alone beside the median preset alone: the machine's other
                // load comes in bursts that one sequential pass cannot divide out.
                let again =
                    alone_against_median(&all, &m.id, &rows, RELEASE_MEDIAN) * RELEASE_MEDIAN;
                if again > budget(m.unit) {
                    failing.push((m.id.clone(), "cpu", again, m.unit));
                }
            }
        } else {
            // In units of this run's median: a debug build's speed and the parallel pass's
            // load divide out. A preset listed for its CPU is held to its release figure.
            let allowed = DEBUG_SLACK * budget(m.unit) / RELEASE_MEDIAN;
            let listed = known.contains_key(&(m.id.clone(), "cpu".to_string()));
            if !listed && m.cpu / median > allowed {
                // The parallel pass shares the machine with every other render (and with
                // the other builds): a preset over is measured again alone, beside the
                // run's median preset alone, before it fails.
                let again = alone_against_median(&all, &m.id, &rows, median);
                if again > allowed {
                    failing.push((m.id.clone(), "cpu", again * RELEASE_MEDIAN, m.unit));
                }
            }
        }
    }
    if std::env::var_os("CANTUS_PRESET_KNOWN_DUMP").is_some() {
        let mut out = String::from(
            "# The presets that fail a check of tests/preset_render.rs: <id> <check> <measured>\n\
             # (LU off the window's centre, dBFS, share of a core). The library's round\n\
             # retunes them and strikes them off (ADR 0101 §5).\n",
        );
        for (id, check, v, _) in &failing {
            out.push_str(&format!("{id} {check} {v:.2}\n"));
        }
        std::fs::write(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(KNOWN_PATH),
            out,
        )
        .expect("the list is written");
    }
    let mut new = Vec::new();
    for (id, check, v, _) in &failing {
        match known.get(&(id.clone(), check.to_string())) {
            None => new.push(format!("{id}: {check} {v:+.2}")),
            Some(then) if worse(check, *then, *v) => new.push(format!(
                "{id}: {check} {v:+.2}, worse than the listed {then:+.2}"
            )),
            _ => {}
        }
    }
    let failed: std::collections::HashSet<(String, String)> = failing
        .iter()
        .map(|(id, c, _, _)| (id.clone(), c.to_string()))
        .collect();
    let mut stale = Vec::new();
    for (id, check) in known.keys() {
        if failed.contains(&(id.clone(), check.clone())) || (check == "cpu" && !release) {
            continue;
        }
        stale.push(format!("{id} {check}"));
    }
    stale.sort();
    assert!(
        new.is_empty() && stale.is_empty(),
        "{} new or worse failures: {new:#?}\nlisted but passing now (strike them off): {stale:#?}",
        new.len()
    );
}

#[test]
fn a_listed_failure_that_gets_worse_fails() {
    assert!(worse("level", -5.0, -6.2), "1.2 LU further");
    assert!(!worse("level", -5.0, -5.9));
    assert!(worse("clip", 1.0, 1.6));
    assert!(!worse("clip", 1.0, 1.4));
    assert!(worse("cpu", 0.2, 0.26));
    assert!(!worse("cpu", 0.2, 0.24));
}
