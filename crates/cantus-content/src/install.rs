//! Installing a pack: download into `.downloads`, verify, unpack into `.staging/<id>`,
//! check the licence is there, write the receipt, and only then move the folder into
//! place — so a pack folder either is whole or does not exist (ADR 0030).

use crate::fetch::{self, FetchError, Transport};
use crate::manifest::{Credit, Pack, Source, Text, ZipSource};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::time::Duration;

/// The receipt written into every installed pack's folder.
pub const RECEIPT: &str = "cantus-pack.json";

/// Where content lives: `<data dir>/Content`.
#[derive(Clone, Debug)]
pub struct Layout {
    pub root: PathBuf,
}

impl Layout {
    pub fn new(root: impl Into<PathBuf>) -> Layout {
        Layout { root: root.into() }
    }
    pub fn pack_dir(&self, id: &str) -> PathBuf {
        self.root.join(id)
    }
    fn staging(&self, id: &str) -> PathBuf {
        self.root.join(".staging").join(id)
    }
    fn downloads(&self) -> PathBuf {
        self.root.join(".downloads")
    }
    fn trash(&self) -> PathBuf {
        self.root.join(".trash")
    }
}

/// What an installed pack is, read back from its folder.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Receipt {
    pub id: String,
    pub version: u32,
    pub name: Text,
    pub licence: String,
    pub licence_url: String,
    pub licence_file: String,
    pub credits: Vec<Credit>,
    /// Seconds since the Unix epoch.
    pub installed_at: u64,
    pub files: u64,
    pub bytes: u64,
}

/// Where an install is.
#[derive(Clone, Debug, PartialEq)]
pub enum Phase {
    Downloading {
        done: u64,
        total: u64,
    },
    /// Waiting to try again after `error`.
    Retrying {
        error: String,
        wait: Duration,
    },
    Verifying,
    Unpacking {
        done: u64,
        total: u64,
    },
}

#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum InstallError {
    #[error(transparent)]
    Fetch(#[from] FetchError),
    #[error("the archive cannot be read: {0}")]
    Archive(String),
    #[error("the pack has no licence file at {0}; it was not installed")]
    NoLicence(String),
    #[error("disk: {0}")]
    Io(String),
}

impl From<std::io::Error> for InstallError {
    fn from(e: std::io::Error) -> Self {
        InstallError::Io(e.to_string())
    }
}

/// How hard to try. Retries stop when either limit is reached.
#[derive(Clone, Copy, Debug)]
pub struct Patience {
    pub attempts: u32,
    /// Total time spent waiting between attempts (a proxy quarantine lasts minutes).
    pub wait: Duration,
}

impl Default for Patience {
    fn default() -> Self {
        Patience {
            attempts: 12,
            wait: Duration::from_secs(12 * 60),
        }
    }
}

impl Patience {
    /// No waiting: what tests use.
    pub fn none() -> Patience {
        Patience {
            attempts: 1,
            wait: Duration::ZERO,
        }
    }
}

/// Everything an install needs besides the pack.
pub struct Job<'a> {
    pub layout: &'a Layout,
    pub transport: &'a dyn Transport,
    pub cancel: &'a AtomicBool,
    pub patience: Patience,
    /// For a zip pack, an archive the user already has (`file://…`): the offline install.
    pub archive: Option<String>,
}

/// Run `f` until it succeeds, cannot succeed, or patience runs out, sleeping in small
/// steps so a cancel is honoured within a fraction of a second.
fn with_retries<T>(
    job: &Job,
    report: &mut dyn FnMut(Phase),
    mut f: impl FnMut(&mut dyn FnMut(Phase)) -> Result<T, FetchError>,
) -> Result<T, FetchError> {
    let mut waited = Duration::ZERO;
    let mut attempt = 0;
    loop {
        match f(report) {
            Ok(v) => return Ok(v),
            Err(e) => {
                attempt += 1;
                let Some(wait) = e.retry_after(attempt - 1) else {
                    return Err(e);
                };
                if attempt >= job.patience.attempts || waited + wait > job.patience.wait {
                    return Err(e);
                }
                report(Phase::Retrying {
                    error: e.to_string(),
                    wait,
                });
                let step = Duration::from_millis(100);
                let mut slept = Duration::ZERO;
                while slept < wait {
                    if job.cancel.load(std::sync::atomic::Ordering::Relaxed) {
                        return Err(FetchError::Cancelled);
                    }
                    std::thread::sleep(step);
                    slept += step;
                }
                waited += wait;
            }
        }
    }
}

/// Install (or update) `pack`. Blocking: the manager runs it on a worker thread.
pub fn install(
    job: &Job,
    pack: &Pack,
    report: &mut dyn FnMut(Phase),
) -> Result<Receipt, InstallError> {
    let staging = job.layout.staging(&pack.id);
    std::fs::create_dir_all(&staging)?;
    let (files, bytes) = match &pack.source {
        Source::Files(list) => install_files(job, list, &staging, report)?,
        Source::Zip(z) => install_zip(job, z, &staging, report)?,
    };
    if !staging.join(&pack.licence_file).is_file() {
        return Err(InstallError::NoLicence(pack.licence_file.clone()));
    }
    let receipt = Receipt {
        id: pack.id.clone(),
        version: pack.version,
        name: pack.name.clone(),
        licence: pack.licence.clone(),
        licence_url: pack.licence_url.clone(),
        licence_file: pack.licence_file.clone(),
        credits: pack.credits.clone(),
        installed_at: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0),
        files,
        bytes,
    };
    let json =
        serde_json::to_string_pretty(&receipt).map_err(|e| InstallError::Io(e.to_string()))?;
    std::fs::write(staging.join(RECEIPT), json)?;
    // Swap into place: the old version (an update) goes to the trash first.
    let dest = job.layout.pack_dir(&pack.id);
    if dest.exists() {
        let old = job
            .layout
            .trash()
            .join(format!("{}-{}", pack.id, receipt.installed_at));
        std::fs::create_dir_all(job.layout.trash())?;
        std::fs::rename(&dest, &old)?;
        let _ = std::fs::remove_dir_all(&old);
    }
    std::fs::rename(&staging, &dest)?;
    Ok(receipt)
}

fn install_files(
    job: &Job,
    list: &[crate::manifest::FileEntry],
    staging: &Path,
    report: &mut dyn FnMut(Phase),
) -> Result<(u64, u64), InstallError> {
    let total: u64 = list.iter().map(|f| f.size).sum();
    let mut done = 0u64;
    for f in list {
        let dest = staging.join(&f.to);
        let part = job.layout.downloads().join(format!("{}.part", f.sha256));
        let base = done;
        with_retries(job, report, |report| {
            fetch::download(
                job.transport,
                &f.url,
                f.size,
                &f.sha256,
                &part,
                &dest,
                job.cancel,
                &mut |n| {
                    report(Phase::Downloading {
                        done: base + n,
                        total,
                    })
                },
            )
        })?;
        done += f.size;
    }
    Ok((list.len() as u64, total))
}

fn install_zip(
    job: &Job,
    z: &ZipSource,
    staging: &Path,
    report: &mut dyn FnMut(Phase),
) -> Result<(u64, u64), InstallError> {
    let archive = job.layout.downloads().join(format!("{}.zip", z.sha256));
    let part = job.layout.downloads().join(format!("{}.part", z.sha256));
    let url = job.archive.clone().unwrap_or_else(|| z.url.clone());
    with_retries(job, report, |report| {
        fetch::download(
            job.transport,
            &url,
            z.size,
            &z.sha256,
            &part,
            &archive,
            job.cancel,
            &mut |n| {
                report(Phase::Downloading {
                    done: n,
                    total: z.size,
                })
            },
        )
    })?;
    report(Phase::Verifying);
    let r = unpack(&archive, z, staging, job.cancel, report);
    if r.is_ok() {
        let _ = std::fs::remove_file(&archive);
    }
    r
}

/// Unpack the entries `z` keeps into `staging`. Entry names that would climb out of the
/// folder are refused (zip-slip).
pub fn unpack(
    archive: &Path,
    z: &ZipSource,
    staging: &Path,
    cancel: &AtomicBool,
    report: &mut dyn FnMut(Phase),
) -> Result<(u64, u64), InstallError> {
    let file = std::fs::File::open(archive)?;
    let mut zip = zip::ZipArchive::new(file).map_err(|e| InstallError::Archive(e.to_string()))?;
    let total = zip.len() as u64;
    let (mut files, mut bytes) = (0u64, 0u64);
    for i in 0..zip.len() {
        if cancel.load(std::sync::atomic::Ordering::Relaxed) {
            return Err(FetchError::Cancelled.into());
        }
        let mut entry = zip
            .by_index(i)
            .map_err(|e| InstallError::Archive(e.to_string()))?;
        if entry.is_dir() {
            continue;
        }
        let Some(name) = entry.enclosed_name() else {
            return Err(InstallError::Archive(format!(
                "unsafe entry name {}",
                entry.name()
            )));
        };
        let name = name.to_string_lossy().replace('\\', "/");
        let Some(rel) = name.strip_prefix(z.strip.as_str()) else {
            continue;
        };
        if rel.is_empty() || !keeps(z, rel) {
            continue;
        }
        crate::manifest::check_path(rel).map_err(InstallError::Archive)?;
        let out = staging.join(rel);
        if let Some(dir) = out.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let mut w = std::fs::File::create(&out)?;
        bytes += std::io::copy(&mut entry, &mut w)?;
        files += 1;
        if i % 16 == 0 {
            report(Phase::Unpacking {
                done: i as u64,
                total,
            });
        }
    }
    report(Phase::Unpacking { done: total, total });
    Ok((files, bytes))
}

/// Whether an entry (after `strip`) is part of the pack.
pub fn keeps(z: &ZipSource, rel: &str) -> bool {
    (z.include.is_empty() || z.include.iter().any(|p| glob(p, rel)))
        && !z.exclude.iter().any(|p| glob(p, rel))
}

/// `*` matches within a path component, `**` across components; case-insensitive.
pub fn glob(pattern: &str, path: &str) -> bool {
    fn m(p: &[u8], s: &[u8]) -> bool {
        match p.first() {
            None => s.is_empty(),
            Some(b'*') if p.get(1) == Some(&b'*') => {
                let rest = if p.get(2) == Some(&b'/') {
                    &p[3..]
                } else {
                    &p[2..]
                };
                (0..=s.len())
                    .any(|i| (i == 0 || s[i - 1] == b'/' || rest.is_empty()) && m(rest, &s[i..]))
            }
            Some(b'*') => (0..=s.len())
                .take_while(|&i| i == 0 || s[i - 1] != b'/')
                .any(|i| m(&p[1..], &s[i..])),
            Some(c) => s.first().is_some_and(|d| d.eq_ignore_ascii_case(c)) && m(&p[1..], &s[1..]),
        }
    }
    m(pattern.as_bytes(), path.as_bytes())
}

/// Remove an installed pack: moved aside first, so a failure half-way never leaves a
/// half pack that still looks installed.
pub fn remove(layout: &Layout, id: &str) -> std::io::Result<()> {
    let dir = layout.pack_dir(id);
    if !dir.exists() {
        return Ok(());
    }
    std::fs::create_dir_all(layout.trash())?;
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let old = layout.trash().join(format!("{id}-{stamp}"));
    std::fs::rename(&dir, &old)?;
    std::fs::remove_dir_all(&old)
}

/// The receipts of every installed pack.
pub fn installed(layout: &Layout) -> Vec<Receipt> {
    let mut out = Vec::new();
    let Ok(rd) = std::fs::read_dir(&layout.root) else {
        return out;
    };
    for e in rd.flatten() {
        let p = e.path().join(RECEIPT);
        if let Ok(text) = std::fs::read_to_string(&p) {
            if let Ok(r) = serde_json::from_str::<Receipt>(&text) {
                out.push(r);
            }
        }
    }
    out.sort_by(|a, b| a.id.cmp(&b.id));
    out
}

/// Forget what an abandoned install left behind: partial downloads are kept (they
/// resume), staging folders are not.
pub fn clear_staging(layout: &Layout, id: &str) {
    let _ = std::fs::remove_dir_all(layout.staging(id));
}
