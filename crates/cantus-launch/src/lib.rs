//! Which build of Cantus this processor runs (ADR 0031 §5.3, round 22).
//!
//! A Windows release carries the program twice — `cantus-x86-64-v3.exe`, compiled for
//! x86-64-v3 (AVX, AVX2, FMA, BMI1/2, F16C, LZCNT, MOVBE: Intel since Haswell, AMD since
//! Excavator), and `cantus-x86-64-v2.exe` for x86-64-v2 (SSE4.2, POPCNT: Intel since
//! Nehalem, AMD since Bulldozer) — and, with `xtask package --v4`, a third for x86-64-v4
//! (AVX-512). `cantus.exe` is this crate's binary: it asks the processor what it has
//! ([`detected`]), takes the highest level it can run whose program is beside it
//! ([`choose`]), and starts it with the same arguments.
//!
//! The levels' feature lists are the ones `rustc --print cfg -C target-cpu=x86-64-vN`
//! enables (a test holds them equal), so the launcher never starts a program that uses an
//! instruction the processor lacks. The builds give the same bits: Rust never fuses
//! `a * b + c` into an FMA on its own and LLVM does not reorder float sums without
//! fast-math, so v2 and v3 differ in speed only (`xtask null-hashes` proves it).

#![deny(unsafe_code)]

mod appid;
pub use appid::{current_app_id, set_app_id, APP_ID};
pub mod taskicon;

use std::path::{Path, PathBuf};

/// A microarchitecture level of x86-64 that Cantus is built for.
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Level {
    V2,
    V3,
    V4,
}

/// x86-64-v2 beyond the plain baseline (Rust's Windows target already assumes
/// `cmpxchg16b` and SSE3).
const V2: &[&str] = &[
    "cmpxchg16b",
    "fxsr",
    "popcnt",
    "sse",
    "sse2",
    "sse3",
    "sse4.1",
    "sse4.2",
    "ssse3",
];
/// What x86-64-v3 adds to v2.
const V3: &[&str] = &[
    "avx", "avx2", "bmi1", "bmi2", "f16c", "fma", "lzcnt", "movbe", "xsave",
];
/// What x86-64-v4 adds to v3.
const V4: &[&str] = &["avx512bw", "avx512cd", "avx512dq", "avx512f", "avx512vl"];

impl Level {
    /// Highest first: the order the launcher tries them in.
    pub const ALL: [Level; 3] = [Level::V4, Level::V3, Level::V2];

    /// `x86-64-v3`: the name `rustc -C target-cpu` takes and the programs are named by.
    pub fn name(self) -> &'static str {
        match self {
            Level::V2 => "x86-64-v2",
            Level::V3 => "x86-64-v3",
            Level::V4 => "x86-64-v4",
        }
    }

    /// `v3`.
    pub fn short(self) -> &'static str {
        &self.name()[7..]
    }

    /// `v3`, `x86-64-v3`, `3` (any case).
    pub fn parse(s: &str) -> Option<Level> {
        let s = s.trim().to_ascii_lowercase();
        let s = s.strip_prefix("x86-64-").unwrap_or(&s);
        match s.strip_prefix('v').unwrap_or(s) {
            "2" => Some(Level::V2),
            "3" => Some(Level::V3),
            "4" => Some(Level::V4),
            _ => None,
        }
    }

    /// The program built for this level: `cantus-x86-64-v3.exe`.
    pub fn program(self) -> String {
        format!("cantus-{}{}", self.name(), std::env::consts::EXE_SUFFIX)
    }

    /// The flags that build it.
    pub fn rustflags(self) -> String {
        format!("-C target-cpu={}", self.name())
    }

    /// Every target feature the level's programs may use, as `is_x86_feature_detected!`
    /// and `rustc --print cfg` name them.
    pub fn features(self) -> Vec<&'static str> {
        let mut v: Vec<&'static str> = V2.to_vec();
        if self >= Level::V3 {
            v.extend_from_slice(V3);
        }
        if self >= Level::V4 {
            v.extend_from_slice(V4);
        }
        v
    }

    /// What this processor lacks for the level (`has` answers per feature).
    pub fn missing(self, has: &dyn Fn(&str) -> bool) -> Vec<&'static str> {
        self.features().into_iter().filter(|f| !has(f)).collect()
    }

    /// The level the calling crate was compiled for, read from its target features: the
    /// app's `--version` says which program is running. `None` below x86-64-v2 (a plain
    /// `cargo build`) and off x86-64.
    pub fn of_this_build() -> Option<Level> {
        let v2 = cfg!(all(
            target_arch = "x86_64",
            target_feature = "popcnt",
            target_feature = "sse4.2",
            target_feature = "ssse3"
        ));
        let v3 = cfg!(all(
            target_feature = "avx2",
            target_feature = "fma",
            target_feature = "bmi2",
            target_feature = "movbe"
        ));
        let v4 = cfg!(all(
            target_feature = "avx512f",
            target_feature = "avx512bw",
            target_feature = "avx512vl"
        ));
        match (v2, v3, v4) {
            (true, true, true) => Some(Level::V4),
            (true, true, false) => Some(Level::V3),
            (true, false, _) => Some(Level::V2),
            _ => None,
        }
    }
}

impl std::fmt::Display for Level {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.name())
    }
}

/// A feature's name and its run-time test.
pub type Probe = (&'static str, fn() -> bool);

/// Every feature a level names, with its run-time test (the operating system's support
/// for the AVX and AVX-512 register state included, as `is_x86_feature_detected!` checks
/// it). A test holds every level's list inside this table.
#[cfg(target_arch = "x86_64")]
pub const DETECT: &[Probe] = &[
    ("cmpxchg16b", || {
        std::arch::is_x86_feature_detected!("cmpxchg16b")
    }),
    ("fxsr", || std::arch::is_x86_feature_detected!("fxsr")),
    ("popcnt", || std::arch::is_x86_feature_detected!("popcnt")),
    ("sse", || std::arch::is_x86_feature_detected!("sse")),
    ("sse2", || std::arch::is_x86_feature_detected!("sse2")),
    ("sse3", || std::arch::is_x86_feature_detected!("sse3")),
    ("sse4.1", || std::arch::is_x86_feature_detected!("sse4.1")),
    ("sse4.2", || std::arch::is_x86_feature_detected!("sse4.2")),
    ("ssse3", || std::arch::is_x86_feature_detected!("ssse3")),
    ("avx", || std::arch::is_x86_feature_detected!("avx")),
    ("avx2", || std::arch::is_x86_feature_detected!("avx2")),
    ("bmi1", || std::arch::is_x86_feature_detected!("bmi1")),
    ("bmi2", || std::arch::is_x86_feature_detected!("bmi2")),
    ("f16c", || std::arch::is_x86_feature_detected!("f16c")),
    ("fma", || std::arch::is_x86_feature_detected!("fma")),
    ("lzcnt", || std::arch::is_x86_feature_detected!("lzcnt")),
    ("movbe", || std::arch::is_x86_feature_detected!("movbe")),
    ("xsave", || std::arch::is_x86_feature_detected!("xsave")),
    ("avx512bw", || {
        std::arch::is_x86_feature_detected!("avx512bw")
    }),
    ("avx512cd", || {
        std::arch::is_x86_feature_detected!("avx512cd")
    }),
    ("avx512dq", || {
        std::arch::is_x86_feature_detected!("avx512dq")
    }),
    ("avx512f", || std::arch::is_x86_feature_detected!("avx512f")),
    ("avx512vl", || {
        std::arch::is_x86_feature_detected!("avx512vl")
    }),
];
#[cfg(not(target_arch = "x86_64"))]
pub const DETECT: &[Probe] = &[];

/// Whether this processor (and its operating system) has `feature`. Unknown names, and
/// every feature off x86-64, answer `false`.
pub fn detected(feature: &str) -> bool {
    DETECT
        .iter()
        .find(|(name, _)| *name == feature)
        .is_some_and(|(_, test)| test())
}

/// The highest level this processor runs, if any.
pub fn cpu_level(has: &dyn Fn(&str) -> bool) -> Option<Level> {
    Level::ALL.into_iter().find(|l| l.missing(has).is_empty())
}

/// What the launcher starts.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Choice {
    /// `None` off x86-64, where there is one program, `cantus`.
    pub level: Option<Level>,
    pub program: PathBuf,
    /// Said on the log: why a level asked for with `CANTUS_CPU` was not taken.
    pub note: Option<String>,
}

/// Why nothing can be started.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Refusal {
    /// The processor is below x86-64-v2; these are what it lacks.
    CpuTooOld { missing: Vec<&'static str> },
    /// No program this processor can run is beside the launcher.
    NoProgram { looked_for: Vec<PathBuf> },
}

/// The environment variable that asks for a level (`v2`, `v3`, `v4`): a lower one than the
/// processor runs is taken (to compare builds, or to step around a fault in one); a
/// higher one is refused with a note, since it would stop on the first instruction the
/// processor lacks.
pub const FORCE_VAR: &str = "CANTUS_CPU";

/// The program to start from `dir` on a processor that answers `has`, with the files
/// `exists` says are there, and the level `forced` asks for (`CANTUS_CPU`).
pub fn choose(
    dir: &Path,
    has: &dyn Fn(&str) -> bool,
    exists: &dyn Fn(&Path) -> bool,
    forced: Option<&str>,
) -> Result<Choice, Refusal> {
    if !cfg!(target_arch = "x86_64") {
        let program = dir.join(format!("cantus{}", std::env::consts::EXE_SUFFIX));
        return if exists(&program) {
            Ok(Choice {
                level: None,
                program,
                note: None,
            })
        } else {
            Err(Refusal::NoProgram {
                looked_for: vec![program],
            })
        };
    }
    let Some(top) = cpu_level(has) else {
        return Err(Refusal::CpuTooOld {
            missing: Level::V2.missing(has),
        });
    };
    let mut note = None;
    if let Some(asked) = forced.filter(|s| !s.trim().is_empty()) {
        match Level::parse(asked) {
            None => note = Some(format!("{FORCE_VAR}={asked}: not a level (v2, v3, v4)")),
            Some(l) if l > top => {
                note = Some(format!(
                    "{FORCE_VAR}={asked}: this processor lacks {} for {l}",
                    l.missing(has).join(", ")
                ))
            }
            Some(l) => {
                let program = dir.join(l.program());
                if exists(&program) {
                    return Ok(Choice {
                        level: Some(l),
                        program,
                        note: None,
                    });
                }
                note = Some(format!("{FORCE_VAR}={asked}: {} is not here", l.program()));
            }
        }
    }
    let runnable: Vec<Level> = Level::ALL.into_iter().filter(|l| *l <= top).collect();
    for l in &runnable {
        let program = dir.join(l.program());
        if exists(&program) {
            return Ok(Choice {
                level: Some(*l),
                program,
                note,
            });
        }
    }
    Err(Refusal::NoProgram {
        looked_for: runnable.iter().map(|l| dir.join(l.program())).collect(),
    })
}

/// What the launcher shows when it cannot start Cantus: English, then Spanish (the
/// launcher has no settings to read a language from, and the message must not need one).
pub fn refusal_text(r: &Refusal) -> String {
    match r {
        Refusal::CpuTooOld { missing } => {
            let m = missing.join(", ");
            format!(
                "Cantus needs a processor with SSE4.2 and POPCNT (x86-64-v2: Intel since 2008, \
AMD since 2011). This one lacks: {m}.\n\n\
Cantus necesita un procesador con SSE4.2 y POPCNT (x86-64-v2: Intel desde 2008, AMD desde \
2011). A este le falta: {m}."
            )
        }
        Refusal::NoProgram { looked_for } => {
            let files: Vec<String> = looked_for.iter().map(|p| p.display().to_string()).collect();
            let files = files.join("\n");
            format!(
                "Cantus's program is missing beside the launcher:\n{files}\n\
Install Cantus again, or unzip the whole portable copy.\n\n\
Falta el programa de Cantus junto al lanzador:\n{files}\n\
Vuelve a instalar Cantus, o descomprime la copia portátil completa."
            )
        }
    }
}

/// When the chosen program does not start (`err` is the system's reason).
pub fn start_failed_text(program: &Path, err: &std::io::Error) -> String {
    let p = program.display();
    format!(
        "Cantus could not start {p}: {err}.\n\n\
Cantus no pudo iniciar {p}: {err}."
    )
}

/// `cantus.exe --launcher-info`: the processor's level, what it lacks for the next one,
/// the programs present and the one that would start — for a hardware report.
pub fn info(dir: &Path, has: &dyn Fn(&str) -> bool, forced: Option<&str>) -> String {
    let mut out = format!("cantus launcher {}\n", env!("CARGO_PKG_VERSION"));
    let top = cpu_level(has);
    match top {
        Some(l) => out.push_str(&format!("processor: {l}\n")),
        None => out.push_str(&format!(
            "processor: below x86-64-v2 (lacks {})\n",
            Level::V2.missing(has).join(", ")
        )),
    }
    if let Some(next) = Level::ALL.into_iter().rev().find(|l| Some(*l) > top) {
        out.push_str(&format!(
            "lacks for {next}: {}\n",
            next.missing(has).join(", ")
        ));
    }
    let present: Vec<String> = Level::ALL
        .into_iter()
        .rev()
        .map(Level::program)
        .filter(|p| dir.join(p).is_file())
        .collect();
    out.push_str(&format!("programs here: {}\n", present.join(", ")));
    match choose(dir, has, &|p| p.is_file(), forced) {
        Ok(c) => {
            out.push_str(&format!("starts: {}\n", c.program.display()));
            if let Some(n) = c.note {
                out.push_str(&format!("note: {n}\n"));
            }
        }
        Err(r) => out.push_str(&format!("starts nothing: {r:?}\n")),
    }
    out
}
