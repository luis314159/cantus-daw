//! Crash recovery (round 22, *soak*; ADR 0006, ADR 0025 §8).
//!
//! The autosave (ADR 0006) only helps if somebody offers it back. Each running Cantus
//! keeps a **session journal** in `<data>/sessions/<id>.json` next to a `<id>.lock` it
//! holds locked for as long as it lives — the operating system releases the lock when the
//! process ends however it ends, so a journal whose lock can be taken belongs to a session
//! that is gone. A journal says which project was open, whether it had unsaved changes,
//! where its last autosave went, and the recording passes whose takes are not yet in a
//! saved or autosaved file (the writer flushes each WAV's header every half second, ADR
//! 0025 §8). A clean exit removes the journal; a crash leaves it, and so does a close with
//! unsaved changes (they were autosaved on the way out, and are offered back next time).
//!
//! At start the boot reads the orphans (`orphans`) and the application offers them
//! (`Offer`, drawn by `views::recovery`): the autosave in the place of the project, as one
//! undo step over the last save, and the takes recorded since, laid out as a finished pass
//! would have been (`record_pass::build`), their headers repaired to every whole frame on
//! disk (`repair_wav`). *Discard* forgets the journal but deletes no file.

use cantus_core::audio::TakeMode;
use cantus_core::{Ticks, TrackId};
use serde::{Deserialize, Serialize};
use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

/// The journal's own format version.
pub const JOURNAL_VERSION: u32 = 1;

/// What a session tells its successor (one file per running Cantus).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Journal {
    pub version: u32,
    pub pid: u32,
    /// Seconds since the Unix epoch.
    pub started_at: u64,
    pub app_version: String,
    pub project_name: String,
    /// The project's file, once it has one.
    pub project_path: Option<PathBuf>,
    /// The project had changes not in its file when the journal was last written.
    pub dirty: bool,
    /// The last autosave that was written, and when (Unix seconds).
    pub autosave: Option<PathBuf>,
    pub autosaved_at: Option<u64>,
    /// The session ended normally, its unsaved changes kept in the autosave.
    pub closed: bool,
    /// Recording passes whose takes are not yet in a saved or autosaved file.
    pub passes: Vec<JournalPass>,
}

/// A recording pass as the journal keeps it: enough to lay its takes out again.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct JournalPass {
    pub record_start: Ticks,
    /// Where the transport started (before `record_start` by the count-in): the files'
    /// first frame.
    pub transport_start: Ticks,
    pub punch: Option<(Ticks, Ticks)>,
    pub loop_region: Option<(Ticks, Ticks)>,
    pub loop_takes: bool,
    pub mode: TakeMode,
    pub tempo_bpm: f64,
    pub sample_rate: u32,
    /// The round-trip latency the takes are moved earlier by, in frames.
    pub latency_frames: i64,
    pub takes: Vec<JournalTake>,
    /// The pass ended and its clips are in the project (not yet in a file on disk).
    pub finished: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct JournalTake {
    pub track: TrackId,
    pub path: PathBuf,
}

fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Where the journals live under the application's data folder.
pub fn sessions_dir(data_dir: &Path) -> PathBuf {
    data_dir.join("sessions")
}

/// This process's journal, and the lock that says it is alive.
#[derive(Debug)]
pub struct Session {
    json: PathBuf,
    lock_path: PathBuf,
    lock: Option<File>,
    pub journal: Journal,
    written: String,
}

impl Session {
    /// Start a journal in `dir` (made if missing) and hold its lock.
    pub fn start(dir: &Path, app_version: &str) -> io::Result<Session> {
        fs::create_dir_all(dir)?;
        let pid = std::process::id();
        let millis = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or(0);
        let id = format!("{pid}-{millis}");
        let lock_path = dir.join(format!("{id}.lock"));
        let lock = OpenOptions::new()
            .create(true)
            .truncate(true)
            .read(true)
            .write(true)
            .open(&lock_path)?;
        lock.try_lock().map_err(io::Error::other)?;
        let mut s = Session {
            json: dir.join(format!("{id}.json")),
            lock_path,
            lock: Some(lock),
            journal: Journal {
                version: JOURNAL_VERSION,
                pid,
                started_at: now_secs(),
                app_version: app_version.to_string(),
                ..Default::default()
            },
            written: String::new(),
        };
        s.sync()?;
        Ok(s)
    }

    pub fn journal_path(&self) -> &Path {
        &self.json
    }

    /// Write the journal if it changed since the last write: a temporary file renamed
    /// over the old one, so a crash mid-write leaves the previous journal whole.
    pub fn sync(&mut self) -> io::Result<()> {
        let text = serde_json::to_string_pretty(&self.journal).map_err(io::Error::other)?;
        if text == self.written {
            return Ok(());
        }
        let tmp = self.json.with_extension("json.tmp");
        {
            let mut f = File::create(&tmp)?;
            f.write_all(text.as_bytes())?;
            f.sync_all()?;
        }
        fs::rename(&tmp, &self.json)?;
        self.written = text;
        Ok(())
    }

    /// The session ends normally. With `unsaved` work (kept in the autosave) the journal
    /// stays, marked closed, to offer it next time; otherwise it is removed.
    pub fn end(mut self, unsaved: bool) {
        if unsaved && self.journal.autosave.is_some() {
            self.journal.closed = true;
            let _ = self.sync();
            self.lock = None;
            let _ = fs::remove_file(&self.lock_path);
        } else {
            self.lock = None;
            let _ = fs::remove_file(&self.json);
            let _ = fs::remove_file(&self.lock_path);
        }
    }
}

/// A journal left by a session that is gone.
#[derive(Clone, Debug, PartialEq)]
pub struct Orphan {
    pub journal: Journal,
    pub json: PathBuf,
}

impl Orphan {
    /// Forget it: the journal and its lock go; no project or audio file is touched.
    pub fn forget(&self) {
        let _ = fs::remove_file(&self.json);
        let _ = fs::remove_file(self.json.with_extension("lock"));
    }
}

/// Whether the session that owns `lock` is still running (its lock is held).
fn alive(lock: &Path) -> bool {
    let Ok(f) = OpenOptions::new().read(true).write(true).open(lock) else {
        return false;
    };
    match f.try_lock() {
        Ok(()) => {
            let _ = f.unlock();
            false
        }
        Err(_) => true,
    }
}

/// Every journal in `dir` whose session is gone, oldest first. A journal that cannot be
/// read is removed (it can offer nothing).
pub fn orphans(dir: &Path) -> Vec<Orphan> {
    let Ok(entries) = fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for e in entries.flatten() {
        let path = e.path();
        if path.extension().and_then(|x| x.to_str()) != Some("json") {
            continue;
        }
        if alive(&path.with_extension("lock")) {
            continue;
        }
        match fs::read_to_string(&path)
            .ok()
            .and_then(|t| serde_json::from_str::<Journal>(&t).ok())
        {
            Some(journal) => out.push(Orphan { journal, json: path }),
            None => {
                let _ = fs::remove_file(&path);
                let _ = fs::remove_file(path.with_extension("lock"));
            }
        }
    }
    out.sort_by_key(|o| o.journal.started_at);
    out
}

/// Make a WAV that a crash cut short readable to its last whole frame on disk: the
/// header's sizes are set from the file's length (the writer updates them only every half
/// second, and the frames written since are on disk too). Returns the frames the file now
/// says it holds; a file that is not a WAV this can read is left alone (an error).
pub fn repair_wav(path: &Path) -> io::Result<u64> {
    let mut f = OpenOptions::new().read(true).write(true).open(path)?;
    let len = f.metadata()?.len();
    let mut head = [0u8; 12];
    f.read_exact(&mut head)?;
    if &head[0..4] != b"RIFF" || &head[8..12] != b"WAVE" {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "not a WAV"));
    }
    let mut at = 12u64;
    let mut block_align = 0u64;
    loop {
        if at + 8 > len {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "no data chunk"));
        }
        let mut ch = [0u8; 8];
        f.seek(SeekFrom::Start(at))?;
        f.read_exact(&mut ch)?;
        let size = u32::from_le_bytes([ch[4], ch[5], ch[6], ch[7]]) as u64;
        match &ch[0..4] {
            b"fmt " => {
                let mut fmt = [0u8; 16];
                f.read_exact(&mut fmt)?;
                block_align = u16::from_le_bytes([fmt[12], fmt[13]]) as u64;
            }
            b"data" => {
                if block_align == 0 {
                    return Err(io::Error::new(io::ErrorKind::InvalidData, "no format"));
                }
                let start = at + 8;
                let on_disk = (len - start) / block_align * block_align;
                if on_disk > size || size == u32::MAX as u64 {
                    let data = on_disk.min(u32::MAX as u64 / block_align * block_align);
                    f.seek(SeekFrom::Start(at + 4))?;
                    f.write_all(&(data as u32).to_le_bytes())?;
                    f.seek(SeekFrom::Start(4))?;
                    f.write_all(&((start - 8 + data) as u32).to_le_bytes())?;
                    f.sync_all()?;
                    return Ok(data / block_align);
                }
                return Ok(size.min(on_disk) / block_align);
            }
            _ => {}
        }
        at += 8 + size + (size & 1);
    }
}
