//! Installing a pack: download into `.downloads`, verify, unpack into `.staging/<id>`,
//! check the licence is there, write the receipt, and only then move the folder into
//! place — so a pack folder either is whole or does not exist (ADR 0030).

use crate::fetch::{self, FetchError, Transport};
use crate::manifest::{Attribution, Credit, Pack, PackInstrument, Source, Text, ZipSource};
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
    fn staging_root(&self) -> PathBuf {
        self.root.join(".staging")
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
    /// A CC BY pack's credit line, as the manifest gave it at install (ADR 0042 §3).
    /// Receipts written before round 26 have none and still load.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub attribution: Option<Attribution>,
    /// The instruments the pack offers (round 26); none in older receipts.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub instruments: Vec<PackInstrument>,
    /// Seconds since the Unix epoch.
    pub installed_at: u64,
    pub files: u64,
    pub bytes: u64,
}

impl Receipt {
    /// The credit line the pack was installed with, if its licence asks for one.
    pub fn credit_line(&self) -> Option<String> {
        self.attribution
            .as_ref()
            .map(|a| a.line(&self.licence, &self.licence_url))
    }
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
    /// A pack built at install (round 28): hits mixed of `total`.
    Building {
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
    #[error("another Cantus is installing this pack; try again when it has finished")]
    Busy,
    #[error("not enough free space: the install needs {need} bytes and the disk has {free}")]
    NoSpace { need: u64, free: u64 },
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
    /// The free space on the volume a folder is on (`None`: the system cannot say).
    pub free_space: &'a dyn Fn(&Path) -> Option<u64>,
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

/// One install's own staging folder, `.staging/<id>.<process>.<n>`, with the pack's lock
/// file `.staging/<id>.lock` held exclusively for as long as it lives: a second install of
/// the same pack (another Cantus, say) is refused, and the crash cleanup
/// ([`clear_all_staging`]) never touches a folder whose lock is held. Dropped without
/// [`Staging::commit`], the folder goes.
pub struct Staging {
    dir: PathBuf,
    lock: Option<std::fs::File>,
    keep: bool,
}

impl Staging {
    /// Take `id`'s lock and make a fresh folder; [`InstallError::Busy`] while another
    /// install of it runs.
    pub fn begin(layout: &Layout, id: &str) -> Result<Staging, InstallError> {
        static N: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let root = layout.staging_root();
        std::fs::create_dir_all(&root)?;
        let lock = std::fs::OpenOptions::new()
            .create(true)
            .truncate(false)
            .write(true)
            .open(root.join(format!("{id}.lock")))?;
        match lock.try_lock() {
            Ok(()) => {}
            Err(std::fs::TryLockError::WouldBlock) => return Err(InstallError::Busy),
            Err(std::fs::TryLockError::Error(e)) => return Err(e.into()),
        }
        let n = N.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let dir = root.join(format!("{id}.{}.{n}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir)?;
        Ok(Staging {
            dir,
            lock: Some(lock),
            keep: false,
        })
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// The folder was moved into place: nothing to remove.
    fn commit(mut self) {
        self.keep = true;
    }

    /// Let the lock go and leave the folder, as a crashed process does (tests).
    pub fn abandon(mut self) {
        self.keep = true;
        self.lock = None;
    }
}

impl Drop for Staging {
    fn drop(&mut self) {
        if !self.keep {
            let _ = std::fs::remove_dir_all(&self.dir);
        }
    }
}

/// How many files are under `dir`, at any depth, and their bytes.
fn size_of(dir: &Path) -> (u64, u64) {
    let (mut n, mut bytes) = (0, 0);
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        for e in std::fs::read_dir(&d).into_iter().flatten().flatten() {
            match e.file_type() {
                Ok(t) if t.is_dir() => stack.push(e.path()),
                Ok(_) => {
                    n += 1;
                    bytes += e.metadata().map(|m| m.len()).unwrap_or(0);
                }
                Err(_) => {}
            }
        }
    }
    (n, bytes)
}

/// How many files are under `dir`, at any depth.
fn count_files(dir: &Path) -> u64 {
    let mut n = 0;
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        for e in std::fs::read_dir(&d).into_iter().flatten().flatten() {
            match e.file_type() {
                Ok(t) if t.is_dir() => stack.push(e.path()),
                Ok(_) => n += 1,
                Err(_) => {}
            }
        }
    }
    n
}

/// Install (or update) `pack`. Blocking: the manager runs it on a worker thread.
pub fn install(
    job: &Job,
    pack: &Pack,
    report: &mut dyn FnMut(Phase),
) -> Result<Receipt, InstallError> {
    // Before anything is fetched: room for the whole install at its peak (a download
    // already complete takes no more).
    let have_archive = archive_path(job, pack).is_some_and(|a| a.is_file());
    let need = pack.space_needed()
        - if have_archive {
            pack.download_size()
        } else {
            0
        };
    if let Some(free) = (job.free_space)(&job.layout.root) {
        if free < need {
            return Err(InstallError::NoSpace { need, free });
        }
    }
    let guard = Staging::begin(job.layout, &pack.id)?;
    let staging = guard.dir().to_path_buf();
    // A pack built at install unpacks to more than it weighs once built.
    let budget = budget(
        pack.build
            .as_ref()
            .map_or(pack.installed_size, |b| b.unpacked_size),
    );
    // A pack built at install unpacks in the first half of the unpacking's share of the
    // progress and builds in the second ([`Phase::Building`]).
    let builds = pack.build.is_some();
    let mut unpacking = |p: Phase| {
        report(match p {
            Phase::Unpacking { done, total } if builds => Phase::Unpacking {
                done,
                total: total.saturating_mul(2),
            },
            p => p,
        })
    };
    let (files, bytes) = match &pack.source {
        Source::Files(list) => install_files(job, list, &staging, &mut unpacking)?,
        Source::Zip(z) => install_archive(job, z, Archive::Zip, budget, &staging, &mut unpacking)?,
        Source::TarGz(z) => {
            install_archive(job, z, Archive::TarGz, budget, &staging, &mut unpacking)?
        }
    };
    if !staging.join(&pack.licence_file).is_file() {
        return Err(InstallError::NoLicence(pack.licence_file.clone()));
    }
    // What is on disk is what was unpacked: nothing lost, nothing extra.
    let on_disk = count_files(&staging);
    if on_disk != files {
        return Err(InstallError::Archive(format!(
            "{files} files were unpacked but {on_disk} are in the folder; it was not installed"
        )));
    }
    let (files, bytes) = match &pack.build {
        Some(b) => {
            let credit = pack.credit_full().unwrap_or_else(|| pack.name.en.clone());
            crate::kitbuild::build(b, &staging, &credit, job.cancel, &mut |done, total| {
                report(Phase::Building { done, total })
            })
            .map_err(|e| {
                if job.cancel.load(std::sync::atomic::Ordering::Relaxed) {
                    InstallError::Fetch(FetchError::Cancelled)
                } else {
                    InstallError::Archive(format!("the kit cannot be built: {e}"))
                }
            })?;
            if !staging.join(&pack.licence_file).is_file() {
                return Err(InstallError::NoLicence(pack.licence_file.clone()));
            }
            size_of(&staging)
        }
        None => (files, bytes),
    };
    let receipt = Receipt {
        id: pack.id.clone(),
        version: pack.version,
        name: pack.name.clone(),
        licence: pack.licence.clone(),
        licence_url: pack.licence_url.clone(),
        licence_file: pack.licence_file.clone(),
        credits: pack.credits.clone(),
        attribution: pack.attribution.clone(),
        instruments: pack.instruments.clone(),
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
    guard.commit();
    // The download goes only now: a build that failed, was cancelled or was cut short by a
    // crash finds it again next time instead of fetching it once more.
    if let Some(a) = archive_path(job, pack) {
        let _ = std::fs::remove_file(a);
    }
    Ok(receipt)
}

/// Where an archive pack's download is kept until the pack is in place.
fn archive_path(job: &Job, pack: &Pack) -> Option<PathBuf> {
    let (z, ext) = match &pack.source {
        Source::Zip(z) => (z, "zip"),
        Source::TarGz(z) => (z, "tar.gz"),
        Source::Files(_) => return None,
    };
    Some(job.layout.downloads().join(format!("{}.{ext}", z.sha256)))
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

/// The most an archive pack may unpack to: what the manifest says it weighs installed,
/// a quarter more, and a megabyte — so a damaged or hostile archive (a gzip bomb, a header
/// that claims terabytes) is stopped, not written until the disk is full.
pub fn budget(installed_size: u64) -> u64 {
    // Each entry also costs `ENTRY_COST` against it (`charge`), so an archive of countless
    // empty files is stopped too.
    installed_size
        .saturating_add(installed_size / 4)
        .saturating_add(1 << 20)
}

/// What each unpacked entry costs against the budget besides its bytes: an NTFS file
/// record, so countless empty files are stopped as surely as one huge one.
pub const ENTRY_COST: u64 = 1024;

/// Charge one entry of `size` bytes against `budget` (`used` so far, entries included).
fn charge(used: &mut u64, size: u64, budget: u64) -> Result<(), InstallError> {
    let cost = size.saturating_add(ENTRY_COST);
    if cost > budget.saturating_sub(*used) {
        return Err(too_big(budget));
    }
    *used += ENTRY_COST;
    Ok(())
}

/// Create `out`, which must not exist yet: staging starts empty, so a file already there
/// is a second entry the file system itself maps onto the same file (an alias no name
/// rule foresaw), and the archive is refused.
fn create_new(out: &Path) -> Result<std::fs::File, InstallError> {
    std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(out)
        .map_err(|e| match e.kind() {
            std::io::ErrorKind::AlreadyExists => InstallError::Archive(format!(
                "two entries land on the same file, {}; the archive is refused",
                out.display()
            )),
            _ => e.into(),
        })
}

/// Charge each folder `rel` needs and no entry made before against the budget, like a
/// file: an archive of countless folders is stopped as surely as one of countless files.
fn charge_dirs(
    dirs: &mut std::collections::HashSet<String>,
    rel: &str,
    used: &mut u64,
    budget: u64,
) -> Result<(), InstallError> {
    let mut at = 0;
    while let Some(i) = rel[at..].find('/') {
        let dir = &rel[..at + i];
        if dirs.insert(crate::manifest::file_key(dir)) {
            if ENTRY_COST > budget.saturating_sub(*used) {
                return Err(too_big(budget));
            }
            *used += ENTRY_COST;
        }
        at += i + 1;
    }
    Ok(())
}

/// Refuse a second entry that would be the same file on Windows (case, trailing dots and
/// spaces: `LICENSE`, `license`, `LICENSE.`).
fn claim(seen: &mut std::collections::HashSet<String>, rel: &str) -> Result<(), InstallError> {
    if seen.insert(crate::manifest::file_key(rel)) {
        Ok(())
    } else {
        Err(InstallError::Archive(format!(
            "two entries land on the same file, {rel}; the archive is refused"
        )))
    }
}

/// The archive formats a pack may come in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Archive {
    Zip,
    TarGz,
}

fn install_archive(
    job: &Job,
    z: &ZipSource,
    kind: Archive,
    budget: u64,
    staging: &Path,
    report: &mut dyn FnMut(Phase),
) -> Result<(u64, u64), InstallError> {
    let ext = match kind {
        Archive::Zip => "zip",
        Archive::TarGz => "tar.gz",
    };
    let archive = job.layout.downloads().join(format!("{}.{ext}", z.sha256));
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
    // From an empty folder: nothing of an earlier, interrupted attempt is kept.
    let _ = std::fs::remove_dir_all(staging);
    std::fs::create_dir_all(staging)?;
    let r = match kind {
        Archive::Zip => unpack(&archive, z, budget, staging, job.cancel, report),
        Archive::TarGz => unpack_tar_gz(&archive, z, budget, staging, job.cancel, report),
    };
    if r.is_err() {
        // A failed or cancelled unpack (the download is done and kept): no half pack stays.
        let _ = std::fs::remove_dir_all(staging);
    }
    // The download stays until the pack is in place (`install`).
    r
}

/// Write `from` into `out`, refusing to go past `budget` bytes in all (`used` so far).
fn copy_within(
    from: &mut dyn std::io::Read,
    out: &Path,
    used: &mut u64,
    budget: u64,
) -> Result<u64, InstallError> {
    let mut w = create_new(out)?;
    let room = budget.saturating_sub(*used);
    let n = std::io::copy(
        &mut std::io::Read::take(from, room.saturating_add(1)),
        &mut w,
    )?;
    if n > room {
        return Err(too_big(budget));
    }
    *used += n;
    Ok(n)
}

fn too_big(budget: u64) -> InstallError {
    InstallError::Archive(format!(
        "the archive unpacks to more than the manifest says ({} MB at most); it was not installed",
        budget / 1_000_000
    ))
}

/// Unpack the entries `z` keeps into `staging`, at most `budget` bytes. Entry names that
/// would climb out of the folder are refused (zip-slip).
pub fn unpack(
    archive: &Path,
    z: &ZipSource,
    budget: u64,
    staging: &Path,
    cancel: &AtomicBool,
    report: &mut dyn FnMut(Phase),
) -> Result<(u64, u64), InstallError> {
    let file = std::fs::File::open(archive)?;
    let mut zip = zip::ZipArchive::new(file).map_err(|e| InstallError::Archive(e.to_string()))?;
    let total = zip.len() as u64;
    let (mut files, mut bytes) = (0u64, 0u64);
    // What the budget has spent (bytes and entries) and the files written, by Windows name.
    let mut used = 0u64;
    let mut seen = std::collections::HashSet::new();
    let mut dirs = std::collections::HashSet::new();
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
        claim(&mut seen, rel)?;
        charge_dirs(&mut dirs, rel, &mut used, budget)?;
        charge(&mut used, entry.size(), budget)?;
        let out = staging.join(rel);
        if let Some(dir) = out.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let n = copy_within(&mut entry, &out, &mut used, budget)?;
        bytes += n;
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

/// Unpack a `.tar.gz` as [`unpack`] does a zip: the entries `z` keeps, at most `budget`
/// bytes, nothing outside `staging`. A kept entry that is a link (or anything but a file
/// or a folder) is refused: a pack is plain files.
pub fn unpack_tar_gz(
    archive: &Path,
    z: &ZipSource,
    budget: u64,
    staging: &Path,
    cancel: &AtomicBool,
    report: &mut dyn FnMut(Phase),
) -> Result<(u64, u64), InstallError> {
    let file = std::fs::File::open(archive)?;
    let total = file.metadata().map(|m| m.len()).unwrap_or(0);
    let counted = std::cell::Cell::new(0u64);
    let mut gz = flate2::read::MultiGzDecoder::new(Counted {
        inner: std::io::BufReader::new(file),
        n: &counted,
    });
    let (mut files, mut bytes) = (0u64, 0u64);
    let mut used = 0u64;
    let mut names = std::collections::HashSet::new();
    let mut dirs = std::collections::HashSet::new();
    let mut failure: Option<InstallError> = None;
    let mut seen = 0u64;
    let r = crate::tar::walk(&mut gz, &mut |entry, read| {
        if cancel.load(std::sync::atomic::Ordering::Relaxed) {
            failure = Some(FetchError::Cancelled.into());
            return Err("cancelled".into());
        }
        seen += 1;
        if seen.is_multiple_of(16) {
            report(Phase::Unpacking {
                done: counted.get(),
                total,
            });
        }
        let name = entry.name.trim_start_matches("./");
        let Some(rel) = name.strip_prefix(z.strip.as_str()) else {
            return Ok(());
        };
        let rel = rel.trim_end_matches('/');
        if rel.is_empty() || entry.kind == crate::tar::Kind::Dir || !keeps(z, rel) {
            return Ok(());
        }
        let fail = |e: InstallError, failure: &mut Option<InstallError>| {
            let msg = e.to_string();
            *failure = Some(e);
            Err(msg)
        };
        if let Err(e) = crate::manifest::check_path(rel) {
            return fail(
                InstallError::Archive(format!("unsafe entry name {}: {e}", entry.name)),
                &mut failure,
            );
        }
        if entry.kind != crate::tar::Kind::File {
            return fail(
                InstallError::Archive(format!(
                    "{} is a link or a special file; a pack holds plain files",
                    entry.name
                )),
                &mut failure,
            );
        }
        if let Err(e) = claim(&mut names, rel) {
            return fail(e, &mut failure);
        }
        if let Err(e) = charge_dirs(&mut dirs, rel, &mut used, budget) {
            return fail(e, &mut failure);
        }
        if let Err(e) = charge(&mut used, entry.size, budget) {
            return fail(e, &mut failure);
        }
        let out = staging.join(rel);
        let made = (|| -> Result<(), InstallError> {
            if let Some(dir) = out.parent() {
                std::fs::create_dir_all(dir)?;
            }
            let mut w = create_new(&out)?;
            let mut written = 0u64;
            read(&mut |chunk| {
                use std::io::Write;
                written += chunk.len() as u64;
                w.write_all(chunk).map_err(|e| e.to_string())
            })
            .map_err(InstallError::Archive)?;
            bytes += written;
            used += written;
            Ok(())
        })();
        match made {
            Ok(()) => {
                files += 1;
                Ok(())
            }
            Err(e) => fail(e, &mut failure),
        }
    });
    if let Err(msg) = r {
        return Err(failure.unwrap_or(InstallError::Archive(msg)));
    }
    report(Phase::Unpacking { done: total, total });
    Ok((files, bytes))
}

/// A reader that counts the compressed bytes read, for the progress bar.
struct Counted<'a, R> {
    inner: R,
    n: &'a std::cell::Cell<u64>,
}

impl<R: std::io::Read> std::io::Read for Counted<'_, R> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let k = self.inner.read(buf)?;
        self.n.set(self.n.get() + k as u64);
        Ok(k)
    }
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

/// Staging folders a crash left behind, cleared when a manager starts: every folder
/// whose pack lock nobody holds. A live install's (this Cantus or another) is left alone.
pub fn clear_all_staging(layout: &Layout) {
    let root = layout.staging_root();
    let Ok(rd) = std::fs::read_dir(&root) else {
        return;
    };
    for e in rd.flatten() {
        if !e.file_type().is_ok_and(|t| t.is_dir()) {
            continue;
        }
        let name = e.file_name().to_string_lossy().into_owned();
        let id = name.split('.').next().unwrap_or(&name).to_string();
        let Ok(lock) = std::fs::OpenOptions::new()
            .create(true)
            .truncate(false)
            .write(true)
            .open(root.join(format!("{id}.lock")))
        else {
            continue;
        };
        if lock.try_lock().is_ok() {
            let _ = std::fs::remove_dir_all(e.path());
        }
    }
}
