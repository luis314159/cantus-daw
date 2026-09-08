//! Corruption-resistant file I/O: temp file → fsync → verify → rename (ADR 0006).

use crate::format::{self, FormatError, ProjectFile, FILE_EXTENSION};
use cantus_core::Project;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum StoreError {
    #[error("io: {0}")]
    Io(#[from] io::Error),
    #[error(transparent)]
    Format(#[from] FormatError),
    #[error("verification of the written file failed: {0}")]
    Verify(String),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SaveReport {
    pub path: PathBuf,
    pub bytes: usize,
    pub backup: Option<PathBuf>,
}

fn tmp_path(path: &Path) -> PathBuf {
    let mut p = path.as_os_str().to_owned();
    p.push(".tmp");
    PathBuf::from(p)
}

pub fn backup_path(path: &Path) -> PathBuf {
    let mut p = path.as_os_str().to_owned();
    p.push(".bak");
    PathBuf::from(p)
}

pub fn autosave_path(path: &Path) -> PathBuf {
    let mut p = path.as_os_str().to_owned();
    p.push(".autosave");
    PathBuf::from(p)
}

/// Save atomically. The previous file (if any) survives as `<name>.bak`.
pub fn save(project: &Project, path: &Path, app_version: &str) -> Result<SaveReport, StoreError> {
    let text = ProjectFile::wrap(project.clone(), app_version)?.to_json()?;
    write_verified(&text, path, true)
}

/// Autosave next to the project (or to `fallback_dir` for unsaved projects). Does not
/// touch the real project file or its backup.
pub fn autosave(
    project: &Project,
    project_path: Option<&Path>,
    fallback_dir: &Path,
    app_version: &str,
) -> Result<SaveReport, StoreError> {
    let target = match project_path {
        Some(p) => autosave_path(p),
        None => {
            fs::create_dir_all(fallback_dir)?;
            let safe: String = project
                .name
                .chars()
                .map(|c| {
                    if c.is_alphanumeric() || c == '-' || c == '_' {
                        c
                    } else {
                        '_'
                    }
                })
                .collect();
            fallback_dir.join(format!("{safe}.{FILE_EXTENSION}.autosave"))
        }
    };
    let text = ProjectFile::wrap(project.clone(), app_version)?.to_json()?;
    write_verified(&text, &target, false)
}

fn write_verified(text: &str, path: &Path, keep_backup: bool) -> Result<SaveReport, StoreError> {
    if let Some(dir) = path.parent() {
        if !dir.as_os_str().is_empty() {
            fs::create_dir_all(dir)?;
        }
    }
    let tmp = tmp_path(path);
    {
        let mut f = fs::File::create(&tmp)?;
        io::Write::write_all(&mut f, text.as_bytes())?;
        f.sync_all()?;
    }
    // Never trust a write we have not read back.
    let back = fs::read_to_string(&tmp)?;
    format::parse(&back, true).map_err(|e| StoreError::Verify(e.to_string()))?;

    let mut backup = None;
    if keep_backup && path.exists() {
        let bak = backup_path(path);
        if bak.exists() {
            fs::remove_file(&bak)?;
        }
        fs::rename(path, &bak)?;
        backup = Some(bak);
    } else if path.exists() {
        fs::remove_file(path)?;
    }
    fs::rename(&tmp, path)?;
    Ok(SaveReport {
        path: path.to_path_buf(),
        bytes: text.len(),
        backup,
    })
}

pub struct Loaded {
    pub project: Project,
    pub migrated_from: Option<u32>,
    pub integrity_verified: bool,
    pub app_version: String,
}

pub fn load(path: &Path, verify_integrity: bool) -> Result<Loaded, StoreError> {
    let text = fs::read_to_string(path)?;
    let r = format::parse(&text, verify_integrity)?;
    Ok(Loaded {
        project: r.file.project,
        migrated_from: r.migrated_from,
        integrity_verified: r.integrity_verified,
        app_version: r.file.app_version,
    })
}

/// If a project file is unreadable, try its backup, then its autosave.
pub fn recovery_candidates(path: &Path) -> Vec<PathBuf> {
    [backup_path(path), autosave_path(path)]
        .into_iter()
        .filter(|p| p.exists())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use cantus_core::Ticks;

    fn demo() -> Project {
        let mut p = Project::new("Demo");
        let mut t = p.make_track("Lead");
        let mut c = p.make_clip("c", Ticks::ZERO, Ticks::beats(4));
        c.notes
            .push(p.make_note(Ticks::ZERO, Ticks::QUARTER, 60, 100));
        t.clips.push(c);
        p.tracks.push(t);
        p
    }

    #[test]
    fn save_load_roundtrip_and_backup() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("song.cantus");
        let p = demo();
        let r1 = save(&p, &path, "t").unwrap();
        assert!(r1.backup.is_none());
        assert!(!tmp_path(&path).exists());
        let loaded = load(&path, true).unwrap();
        assert_eq!(loaded.project, p);
        assert!(loaded.integrity_verified);

        let mut p2 = p.clone();
        p2.tempo_bpm = 90.0;
        let r2 = save(&p2, &path, "t").unwrap();
        assert_eq!(r2.backup, Some(backup_path(&path)));
        assert_eq!(load(&backup_path(&path), true).unwrap().project, p);
        assert_eq!(load(&path, true).unwrap().project, p2);
    }

    #[test]
    fn corrupt_file_reports_and_recovery_finds_backup() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("song.cantus");
        save(&demo(), &path, "t").unwrap();
        save(&demo(), &path, "t").unwrap();
        fs::write(&path, "{ this is not json").unwrap();
        assert!(load(&path, true).is_err());
        let c = recovery_candidates(&path);
        assert_eq!(c, vec![backup_path(&path)]);
        assert!(load(&c[0], true).is_ok());
    }

    #[test]
    fn autosave_for_unsaved_project_goes_to_fallback_dir() {
        let dir = tempfile::tempdir().unwrap();
        let mut p = demo();
        p.name = "My Song / Take 2".into();
        let r = autosave(&p, None, &dir.path().join("autosaves"), "t").unwrap();
        assert!(r.path.starts_with(dir.path()));
        assert!(r
            .path
            .file_name()
            .unwrap()
            .to_str()
            .unwrap()
            .starts_with("My_Song"));
        assert_eq!(load(&r.path, true).unwrap().project, p);
    }
}
