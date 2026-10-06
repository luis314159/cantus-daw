//! What a run of the suite produced, as JSON a later run can be compared with
//! (`compare.rs`), and as the Markdown tables `docs/perf/baseline.md` is made of.

use crate::stats::Summary;
use serde::{Deserialize, Serialize};
use std::fmt::Write as _;
use std::path::Path;

/// The report's format; bumped when a field changes meaning.
pub const SCHEMA: u32 = 1;

/// Which way is good.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Better {
    Lower,
    Higher,
}

/// One measured quantity.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Metric {
    /// Stable dotted name: `fx.space.heavy`, `engine.s64.block128`, `render.house.xrt`.
    pub id: String,
    pub unit: String,
    pub better: Better,
    /// Part of the regression gate (`compare.rs`). Metrics whose run-to-run spread is too
    /// wide to gate on a shared machine are reported but not gated.
    pub gate: bool,
    pub summary: Summary,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub note: String,
}

impl Metric {
    pub fn new(id: impl Into<String>, unit: &str, better: Better, summary: Summary) -> Metric {
        Metric {
            id: id.into(),
            unit: unit.into(),
            better,
            gate: true,
            summary,
            note: String::new(),
        }
    }

    pub fn ungated(mut self) -> Metric {
        self.gate = false;
        self
    }

    pub fn note(mut self, note: impl Into<String>) -> Metric {
        self.note = note.into();
        self
    }
}

/// Where a session's block time went (the engine's profiler, `cantus_engine::profile`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct HotSpot {
    pub session: String,
    /// The node in the project's words: "Spatium on Pad bus", "Calor on Lead".
    pub node: String,
    pub share_pct: f64,
    pub us_per_block: f64,
}

/// The machine a report was measured on.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Machine {
    /// What baselines are kept per: `CANTUS_PERF_CLASS`, or the OS and CPU as a slug.
    pub class: String,
    pub cpu: String,
    pub logical_cores: usize,
    pub os: String,
    pub arch: String,
}

impl Machine {
    pub fn detect() -> Machine {
        let cpu = cpu_name();
        let os = std::env::consts::OS.to_string();
        let class = std::env::var("CANTUS_PERF_CLASS")
            .ok()
            .filter(|s| !s.trim().is_empty())
            .unwrap_or_else(|| class_of(&os, &cpu));
        Machine {
            class,
            cpu,
            logical_cores: std::thread::available_parallelism().map_or(1, |n| n.get()),
            os,
            arch: std::env::consts::ARCH.to_string(),
        }
    }
}

/// The default machine class: the OS and the CPU's model, as a file-name-safe slug
/// (`windows-12th-gen-intel-core-i7-12700h`).
pub fn class_of(os: &str, cpu: &str) -> String {
    let cleaned = cpu
        .to_lowercase()
        .replace("(r)", " ")
        .replace("(tm)", " ")
        .replace(" cpu ", " ");
    let cleaned = cleaned.split('@').next().unwrap_or("").to_string();
    let mut slug = String::from(os);
    let mut dash = true;
    slug.push('-');
    for c in cleaned.chars() {
        if c.is_ascii_alphanumeric() {
            slug.push(c);
            dash = false;
        } else if !dash {
            slug.push('-');
            dash = true;
        }
    }
    slug.trim_end_matches('-').to_string()
}

fn cpu_name() -> String {
    let run = |cmd: &str, args: &[&str]| {
        std::process::Command::new(cmd)
            .args(args)
            .output()
            .ok()
            .map(|o| String::from_utf8_lossy(&o.stdout).into_owned())
    };
    let name = if cfg!(windows) {
        run(
            "reg",
            &[
                "query",
                r"HKLM\HARDWARE\DESCRIPTION\System\CentralProcessor\0",
                "/v",
                "ProcessorNameString",
            ],
        )
        .and_then(|s| {
            s.lines()
                .find(|l| l.contains("ProcessorNameString"))
                .and_then(|l| l.split("REG_SZ").nth(1))
                .map(|x| x.trim().to_string())
        })
    } else if cfg!(target_os = "macos") {
        run("sysctl", &["-n", "machdep.cpu.brand_string"]).map(|s| s.trim().to_string())
    } else {
        std::fs::read_to_string("/proc/cpuinfo").ok().and_then(|s| {
            s.lines()
                .find(|l| l.starts_with("model name"))
                .and_then(|l| l.split(':').nth(1))
                .map(|x| x.trim().to_string())
        })
    };
    name.filter(|s| !s.is_empty())
        .unwrap_or_else(|| "unknown-cpu".into())
}

/// A whole run.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Report {
    pub schema: u32,
    /// `quick` (the gate) or `full`.
    pub suite: String,
    pub machine: Machine,
    pub git: String,
    /// The day it ran, `YYYY-MM-DD` (UTC).
    pub date: String,
    pub metrics: Vec<Metric>,
    #[serde(default)]
    pub hot_spots: Vec<HotSpot>,
}

impl Report {
    pub fn new(suite: &str) -> Report {
        Report {
            schema: SCHEMA,
            suite: suite.into(),
            machine: Machine::detect(),
            git: git_head(),
            date: today(),
            metrics: Vec::new(),
            hot_spots: Vec::new(),
        }
    }

    pub fn metric(&self, id: &str) -> Option<&Metric> {
        self.metrics.iter().find(|m| m.id == id)
    }

    pub fn save(&self, path: &Path) -> std::io::Result<()> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let text = serde_json::to_string_pretty(self).map_err(std::io::Error::other)?;
        std::fs::write(path, text + "\n")
    }

    pub fn load(path: &Path) -> std::io::Result<Report> {
        let text = std::fs::read_to_string(path)?;
        serde_json::from_str(&text).map_err(std::io::Error::other)
    }

    /// The metrics as a Markdown table, those whose id starts with `prefix`.
    pub fn table(&self, prefix: &str) -> String {
        let mut s = String::from(
            "| Metric | Median | p10 – p90 | Min – max | Unit | Runs | Gate | Note |\n\
             |---|---:|---:|---:|---|---:|:---:|---|\n",
        );
        for m in self.metrics.iter().filter(|m| m.id.starts_with(prefix)) {
            let x = &m.summary;
            let _ = writeln!(
                s,
                "| `{}` | {} | {} – {} | {} – {} | {} | {} | {} | {} |",
                m.id,
                num(x.median),
                num(x.p10),
                num(x.p90),
                num(x.min),
                num(x.max),
                m.unit,
                x.runs,
                if m.gate { "✓" } else { "" },
                m.note
            );
        }
        s
    }
}

/// A number with three significant figures, the way a person reads a measurement.
pub fn num(x: f64) -> String {
    let a = x.abs();
    if a == 0.0 {
        "0".into()
    } else if a >= 1000.0 {
        format!("{x:.0}")
    } else if a >= 100.0 {
        format!("{x:.1}")
    } else if a >= 10.0 {
        format!("{x:.2}")
    } else if a >= 1.0 {
        format!("{x:.3}")
    } else {
        format!("{x:.4}")
    }
}

fn git_head() -> String {
    std::process::Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_else(|| "unknown".into())
}

/// Today in UTC, `YYYY-MM-DD`, from the system clock (days to civil date, Howard Hinnant's
/// algorithm) — no date crate for one string.
pub fn today() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs()) as i64;
    let z = secs.div_euclid(86_400) + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + if m <= 2 { 1 } else { 0 };
    format!("{y:04}-{m:02}-{d:02}")
}
