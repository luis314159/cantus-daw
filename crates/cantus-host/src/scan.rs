//! Finding plug-ins. Each candidate file is read by a separate process, the `cantus-scan`
//! binary, with a time limit: a plug-in that crashes or hangs while it is loaded takes the
//! scanner down, never Cantus, and is marked blocked in the database.
//!
//! The scanner prints one line, [`RESULT_MARK`] followed by JSON ([`ScanOutput`]), after
//! whatever the plug-in itself printed.

use crate::backend::{Backend, Format, PluginInfo};
use crate::db::{stamp, DbEntry, PluginDb, ScanStatus};
use serde::{Deserialize, Serialize};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

/// Marks the scanner's answer among whatever else the plug-in printed.
pub const RESULT_MARK: &str = "CANTUS-SCAN-RESULT ";

/// What the scanner process answers.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum ScanOutput {
    Plugins(Vec<PluginInfo>),
    Error(String),
}

impl ScanOutput {
    pub fn to_line(&self) -> String {
        format!(
            "{RESULT_MARK}{}",
            serde_json::to_string(self).unwrap_or_else(|_| "null".into())
        )
    }

    /// The answer in a scanner's output; `None` when there is none (it died first).
    pub fn from_output(text: &str) -> Option<ScanOutput> {
        text.lines()
            .rev()
            .find_map(|l| l.strip_prefix(RESULT_MARK))
            .and_then(|json| serde_json::from_str(json.trim()).ok())
    }
}

/// Format names on the scanner's command line.
pub fn format_arg(f: Format) -> &'static str {
    match f {
        Format::Clap => "clap",
        Format::Vst3 => "vst3",
    }
}

pub fn parse_format(s: &str) -> Option<Format> {
    match s {
        "clap" => Some(Format::Clap),
        "vst3" => Some(Format::Vst3),
        _ => None,
    }
}

/// Runs the scanner process.
#[derive(Clone, Debug)]
pub struct Scanner {
    /// The `cantus-scan` executable.
    pub exe: PathBuf,
    /// How long one file may take.
    pub timeout: Duration,
    /// Look in each format's standard folders as well as the user's (off for tests, which
    /// must not load whatever is installed on the machine).
    pub standard_folders: bool,
}

impl Default for Scanner {
    fn default() -> Self {
        Scanner {
            exe: Self::beside_this_program(),
            timeout: Duration::from_secs(30),
            standard_folders: true,
        }
    }
}

/// How a scan went, for the status line.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ScanReport {
    /// Files read now.
    pub scanned: usize,
    /// Files whose cached entry was still good.
    pub cached: usize,
    /// Files newly blocked (crashed or hung).
    pub blocked: usize,
    pub failed: usize,
    /// Usable plug-ins in the database afterwards.
    pub plugins: usize,
    /// The names of the plug-ins this scan found that the database did not have before
    /// (round 30: "3 new plug-ins"), filled in by the session ([`crate::db::newly_found`]).
    pub new: Vec<String>,
}

impl Scanner {
    /// `cantus-scan` next to the running program — where Cargo and the installer put it.
    pub fn beside_this_program() -> PathBuf {
        let name = if cfg!(windows) {
            "cantus-scan.exe"
        } else {
            "cantus-scan"
        };
        std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(|d| d.join(name)))
            .unwrap_or_else(|| PathBuf::from(name))
    }

    /// Read one file in a scanner process.
    pub fn scan_file(&self, format: Format, path: &Path) -> (ScanStatus, Vec<PluginInfo>) {
        let mut cmd = Command::new(&self.exe);
        cmd.arg(format_arg(format))
            .arg(path)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null());
        // A console program started by a windowed one (round 29): no console window
        // flashing up for each file scanned.
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            const CREATE_NO_WINDOW: u32 = 0x0800_0000;
            cmd.creation_flags(CREATE_NO_WINDOW);
        }
        let child = cmd.spawn();
        let mut child = match child {
            Ok(c) => c,
            Err(e) => {
                return (
                    ScanStatus::Failed(format!(
                        "cannot run the scanner ({}): {e}",
                        self.exe.display()
                    )),
                    Vec::new(),
                )
            }
        };
        // Read the output on a thread, so a plug-in that prints a lot cannot fill the pipe
        // and hang the scanner on it.
        let mut stdout = child.stdout.take();
        let reader = std::thread::spawn(move || {
            let mut text = String::new();
            if let Some(out) = stdout.as_mut() {
                let _ = out.read_to_string(&mut text);
            }
            text
        });
        let started = Instant::now();
        let status = loop {
            match child.try_wait() {
                Ok(Some(status)) => break Some(status),
                Ok(None) if started.elapsed() > self.timeout => {
                    let _ = child.kill();
                    let _ = child.wait();
                    break None;
                }
                Ok(None) => std::thread::sleep(Duration::from_millis(5)),
                Err(_) => break None,
            }
        };
        let text = reader.join().unwrap_or_default();
        match status {
            None => (
                ScanStatus::Blocked(format!(
                    "did not answer in {} s",
                    self.timeout.as_secs_f32().round()
                )),
                Vec::new(),
            ),
            Some(s) if !s.success() => (
                ScanStatus::Blocked(match s.code() {
                    Some(code) => format!("crashed (exit code {code:#x})"),
                    None => "crashed".to_string(),
                }),
                Vec::new(),
            ),
            Some(_) => match ScanOutput::from_output(&text) {
                Some(ScanOutput::Plugins(list)) => (ScanStatus::Ok, list),
                Some(ScanOutput::Error(e)) => (ScanStatus::Failed(e), Vec::new()),
                None => (
                    ScanStatus::Blocked("ended without an answer".to_string()),
                    Vec::new(),
                ),
            },
        }
    }
}

/// Every candidate file of `backend` under `folders`, depth-first, sorted.
pub fn candidates(backend: &dyn Backend, folders: &[PathBuf]) -> Vec<PathBuf> {
    fn walk(backend: &dyn Backend, dir: &Path, depth: usize, out: &mut Vec<PathBuf>) {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return;
        };
        for e in entries.flatten() {
            let p = e.path();
            if backend.is_candidate(&p) {
                out.push(p);
            } else if depth < 8 && p.is_dir() {
                walk(backend, &p, depth + 1, out);
            }
        }
    }
    let mut out = Vec::new();
    for f in folders {
        if backend.is_candidate(f) {
            out.push(f.clone());
        } else {
            walk(backend, f, 0, &mut out);
        }
    }
    out.sort();
    out.dedup();
    out
}

/// Scan every candidate of every backend, in its standard folders and the user's, reading
/// again only what changed. `force` reads everything again, blocked files included.
pub fn scan_all(
    db: &mut PluginDb,
    backends: &[Box<dyn Backend>],
    scanner: &Scanner,
    force: bool,
) -> ScanReport {
    let mut report = ScanReport::default();
    for backend in backends {
        let mut folders = if scanner.standard_folders {
            backend.search_paths()
        } else {
            Vec::new()
        };
        folders.extend(db.folders.iter().cloned());
        for path in candidates(backend.as_ref(), &folders) {
            let Some(stamp) = stamp(&path) else { continue };
            if !force && db.is_fresh(&path, stamp) {
                report.cached += 1;
                continue;
            }
            let (status, plugins) = scanner.scan_file(backend.format(), &path);
            report.scanned += 1;
            match &status {
                ScanStatus::Blocked(why) => {
                    log::warn!("plug-in scan: {} blocked: {why}", path.display());
                    report.blocked += 1;
                }
                ScanStatus::Failed(why) => {
                    log::info!("plug-in scan: {}: {why}", path.display());
                    report.failed += 1;
                }
                ScanStatus::Ok => {}
            }
            db.put(DbEntry {
                path,
                format: backend.format(),
                modified: stamp.0,
                size: stamp.1,
                status,
                plugins,
            });
        }
    }
    report.plugins = db.plugins().len();
    report
}
