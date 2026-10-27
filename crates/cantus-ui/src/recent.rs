//! The recent projects (round 31, ADR 0072): the last projects opened or saved, newest
//! first, kept in `recent.toml` beside the settings — what the start page and the command
//! palette list. A file that is gone stays listed until it is opened (and then is said and
//! dropped) or the list moves past it: a drive that is away comes back.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// How many projects are remembered.
pub const KEPT: usize = 12;

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Recent {
    #[serde(default)]
    pub projects: Vec<PathBuf>,
    /// Where the list is kept; `None` before the start-up wiring (tests keep theirs off disk).
    #[serde(skip)]
    pub path: Option<PathBuf>,
}

impl Recent {
    /// The list kept at `path` (empty when there is none or it cannot be read).
    pub fn load(path: PathBuf) -> Recent {
        let mut r: Recent = std::fs::read_to_string(&path)
            .ok()
            .and_then(|t| toml::from_str(&t).ok())
            .unwrap_or_default();
        r.projects.truncate(KEPT);
        r.path = Some(path);
        r
    }

    /// `file` was opened or saved: first in the list, once.
    pub fn note(&mut self, file: &Path) {
        let file = std::path::absolute(file).unwrap_or_else(|_| file.to_path_buf());
        self.projects.retain(|p| !same(p, &file));
        self.projects.insert(0, file);
        self.projects.truncate(KEPT);
        self.store();
    }

    /// `file` could not be opened: out of the list.
    pub fn forget(&mut self, file: &Path) {
        let before = self.projects.len();
        self.projects.retain(|p| !same(p, file));
        if self.projects.len() != before {
            self.store();
        }
    }

    fn store(&self) {
        let Some(path) = &self.path else {
            return;
        };
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        match toml::to_string(self) {
            Ok(text) => {
                // Through a temporary file: a crash never leaves half of it (review 8).
                let tmp = path.with_extension("toml.tmp");
                let written = std::fs::write(&tmp, text).and_then(|_| std::fs::rename(&tmp, path));
                if let Err(e) = written {
                    log::warn!("recent projects: could not save {}: {e}", path.display());
                }
            }
            Err(e) => log::warn!("recent projects: {e}"),
        }
    }
}

/// Two paths of one file (Windows' paths ignore case).
fn same(a: &Path, b: &Path) -> bool {
    if cfg!(windows) {
        a.to_string_lossy().to_lowercase() == b.to_string_lossy().to_lowercase()
    } else {
        a == b
    }
}

/// Where the list is kept.
pub fn path(config: &Path) -> PathBuf {
    config.join("recent.toml")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_newest_comes_first_once_and_the_list_is_kept() {
        let dir = tempfile::tempdir().unwrap();
        let file = path(dir.path());
        let mut r = Recent::load(file.clone());
        let a = dir.path().join("a.cantus");
        let b = dir.path().join("b.cantus");
        r.note(&a);
        r.note(&b);
        r.note(&a);
        assert_eq!(r.projects, vec![a.clone(), b.clone()]);
        for i in 0..20 {
            r.note(&dir.path().join(format!("{i}.cantus")));
        }
        assert_eq!(r.projects.len(), KEPT);
        let again = Recent::load(file);
        assert_eq!(again.projects, r.projects, "kept on disk");
        let mut again = again;
        let first = again.projects[0].clone();
        again.forget(&first);
        assert!(!again.projects.contains(&first));
    }
}
