//! What the scans found: every candidate file, what it holds, or why it is not used. Cached
//! as JSON under the app's data folder, keyed by path and modification time, so a start
//! with nothing new installed reads the cache and loads nothing.

use crate::backend::{Format, PluginInfo, PluginKind};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// The outcome of scanning one file.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum ScanStatus {
    /// Read; its plug-ins are in [`DbEntry::plugins`].
    Ok,
    /// The file said it is not a plug-in Cantus can use (no entry, no factory…). Retried
    /// only when the file changes.
    Failed(String),
    /// The scanner crashed or hung on it. Never loaded, never retried, until the user asks
    /// for a rescan of it.
    Blocked(String),
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DbEntry {
    pub path: PathBuf,
    pub format: Format,
    /// Modification time, nanoseconds since the Unix epoch, and size: with the path, what
    /// says the file is the one that was scanned.
    pub modified: u64,
    pub size: u64,
    pub status: ScanStatus,
    #[serde(default)]
    pub plugins: Vec<PluginInfo>,
}

/// The plug-in database.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct PluginDb {
    /// Folders the user added to the standard ones.
    #[serde(default)]
    pub folders: Vec<PathBuf>,
    #[serde(default)]
    pub entries: Vec<DbEntry>,
    /// Plug-ins that stopped working while Cantus ran them (round 29, ADR 0063),
    /// remembered across sessions and shown in the plug-in manager.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub crashes: Vec<CrashRecord>,
}

/// A plug-in that crashed or hung while it ran (isolated, so Cantus went on).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CrashRecord {
    pub format: Format,
    pub id: String,
    pub name: String,
    /// Times it stopped working, in every session.
    pub count: u32,
    /// When it last did, seconds since the Unix epoch.
    pub last: u64,
}

/// A file's modification time (ns since the epoch) and size; `None` if it cannot be read.
pub fn stamp(path: &Path) -> Option<(u64, u64)> {
    let meta = std::fs::metadata(path).ok()?;
    let modified = meta
        .modified()
        .ok()?
        .duration_since(std::time::UNIX_EPOCH)
        .ok()?
        .as_nanos() as u64;
    Some((modified, meta.len()))
}

impl PluginDb {
    /// Read the database; an absent or unreadable file is an empty one.
    pub fn load(path: &Path) -> PluginDb {
        std::fs::read_to_string(path)
            .ok()
            .and_then(|t| serde_json::from_str(&t).ok())
            .unwrap_or_default()
    }

    /// Write the database (through a temporary file, so a crash never leaves half of one).
    pub fn save(&self, path: &Path) -> std::io::Result<()> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let text = serde_json::to_string_pretty(self).map_err(std::io::Error::other)?;
        let tmp = path.with_extension("json.tmp");
        std::fs::write(&tmp, text)?;
        std::fs::rename(&tmp, path)
    }

    pub fn entry(&self, path: &Path) -> Option<&DbEntry> {
        self.entries.iter().find(|e| e.path == path)
    }

    /// Put `entry` in place of any other for its path.
    pub fn put(&mut self, entry: DbEntry) {
        self.entries.retain(|e| e.path != entry.path);
        self.entries.push(entry);
        self.entries.sort_by(|a, b| a.path.cmp(&b.path));
    }

    /// Whether the cached entry for `path` is still good: same file, and not something the
    /// user asked to have looked at again.
    pub fn is_fresh(&self, path: &Path, stamp: (u64, u64)) -> bool {
        self.entry(path).is_some_and(|e| {
            matches!(e.status, ScanStatus::Blocked(_))
                || (e.modified == stamp.0 && e.size == stamp.1)
        })
    }

    /// The names of the usable plug-ins in `self` that `before` did not have (by format
    /// and id), in [`Self::plugins`]' order: what a scan found new.
    pub fn newly_found(&self, before: &PluginDb) -> Vec<String> {
        let known: std::collections::HashSet<(Format, &str)> = before
            .plugins()
            .into_iter()
            .map(|p| (p.format, p.id.as_str()))
            .collect();
        // One name once: a plug-in found as CLAP and as VST3 is one new plug-in (review 4).
        let mut out: Vec<String> = Vec::new();
        for p in self.plugins() {
            if !known.contains(&(p.format, p.id.as_str())) && !out.contains(&p.name) {
                out.push(p.name.clone());
            }
        }
        out
    }

    /// Every usable plug-in, sorted by vendor then name.
    pub fn plugins(&self) -> Vec<&PluginInfo> {
        let mut out: Vec<&PluginInfo> = self
            .entries
            .iter()
            .filter(|e| e.status == ScanStatus::Ok)
            .flat_map(|e| e.plugins.iter())
            .collect();
        out.sort_by(|a, b| {
            (a.vendor.to_lowercase(), a.name.to_lowercase())
                .cmp(&(b.vendor.to_lowercase(), b.name.to_lowercase()))
        });
        out
    }

    /// The effects (what an insert slot can hold).
    pub fn effects(&self) -> Vec<&PluginInfo> {
        self.plugins()
            .into_iter()
            .filter(|p| p.kind == PluginKind::Effect && p.main_outputs > 0)
            .collect()
    }

    /// A plug-in by format and id, from a file that is not blocked.
    pub fn find(&self, format: Format, id: &str) -> Option<&PluginInfo> {
        self.entries
            .iter()
            .filter(|e| e.status == ScanStatus::Ok)
            .flat_map(|e| e.plugins.iter())
            .find(|p| p.format == format && p.id == id)
    }

    /// The files that crashed or hung the scanner.
    pub fn blocked(&self) -> Vec<&DbEntry> {
        self.entries
            .iter()
            .filter(|e| matches!(e.status, ScanStatus::Blocked(_)))
            .collect()
    }

    /// The files that are not plug-ins Cantus can use.
    pub fn failed(&self) -> Vec<&DbEntry> {
        self.entries
            .iter()
            .filter(|e| matches!(e.status, ScanStatus::Failed(_)))
            .collect()
    }

    /// What a loaded plug-in says it is, in place of what the scan guessed (a VST3 scan
    /// read from `moduleinfo.json` guesses buses and editor without loading anything).
    /// Returns whether anything changed.
    pub fn learn(&mut self, info: &PluginInfo) -> bool {
        for e in &mut self.entries {
            if e.path != info.path || e.format != info.format {
                continue;
            }
            if let Some(p) = e.plugins.iter_mut().find(|p| p.id == info.id) {
                if p != info {
                    *p = info.clone();
                    return true;
                }
                return false;
            }
        }
        false
    }

    /// A plug-in stopped working: counted, with the time.
    pub fn remember_crash(&mut self, info: &PluginInfo) {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_secs());
        match self
            .crashes
            .iter_mut()
            .find(|c| c.format == info.format && c.id == info.id)
        {
            Some(c) => {
                c.count += 1;
                c.last = now;
                c.name = info.name.clone();
            }
            None => self.crashes.push(CrashRecord {
                format: info.format,
                id: info.id.clone(),
                name: info.name.clone(),
                count: 1,
                last: now,
            }),
        }
    }

    /// What the database remembers of a plug-in's crashes.
    pub fn crash_record(&self, format: Format, id: &str) -> Option<&CrashRecord> {
        self.crashes
            .iter()
            .find(|c| c.format == format && c.id == id)
    }

    /// The user cleared a plug-in's crash record (it was fixed).
    pub fn forget_crashes(&mut self, format: Format, id: &str) {
        self.crashes.retain(|c| !(c.format == format && c.id == id));
    }

    /// Forget what is known about `path`, so the next scan reads it again — the user's
    /// "rescan" for a blocked plug-in.
    pub fn forget(&mut self, path: &Path) {
        self.entries.retain(|e| e.path != path);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn info(sidechain: bool) -> PluginInfo {
        PluginInfo {
            format: Format::Vst3,
            id: "0123456789ABCDEF0123456789ABCDEF".into(),
            name: "Guess".into(),
            vendor: "Maker".into(),
            version: "1".into(),
            path: PathBuf::from("x.vst3"),
            kind: PluginKind::Effect,
            main_inputs: 2,
            main_outputs: 2,
            sidechain,
            editor: false,
            features: Vec::new(),
        }
    }

    #[test]
    fn a_loaded_plugins_own_description_replaces_the_scans_guess() {
        let mut db = PluginDb::default();
        db.put(DbEntry {
            path: PathBuf::from("x.vst3"),
            format: Format::Vst3,
            modified: 1,
            size: 2,
            status: ScanStatus::Ok,
            plugins: vec![info(false)],
        });
        assert!(!db.learn(&info(false)), "nothing new");
        assert!(db.learn(&info(true)));
        assert!(db.find(Format::Vst3, &info(true).id).unwrap().sidechain);
        let mut elsewhere = info(true);
        elsewhere.path = PathBuf::from("y.vst3");
        assert!(!db.learn(&elsewhere), "another file is another entry");
    }

    #[test]
    fn a_scan_names_only_the_plug_ins_it_found_new() {
        let entry = |path: &str, status: ScanStatus, plugins: Vec<PluginInfo>| DbEntry {
            path: PathBuf::from(path),
            format: Format::Vst3,
            modified: 1,
            size: 1,
            status,
            plugins,
        };
        let named = |id: &str, name: &str| PluginInfo {
            id: id.into(),
            name: name.into(),
            ..info(false)
        };
        let mut before = PluginDb::default();
        before.put(entry(
            "a.vst3",
            ScanStatus::Ok,
            vec![named("A", "Valhalla")],
        ));
        let mut after = before.clone();
        assert!(after.newly_found(&before).is_empty(), "nothing new");
        after.put(entry(
            "b.vst3",
            ScanStatus::Ok,
            vec![named("B", "Surge XT")],
        ));
        after.put(entry("c.vst3", ScanStatus::Ok, vec![named("C", "Odin 2")]));
        // A blocked file's plug-ins are not usable: not "new".
        after.put(entry(
            "d.vst3",
            ScanStatus::Blocked("crashed".into()),
            vec![named("D", "Broken")],
        ));
        assert_eq!(after.newly_found(&before), ["Odin 2", "Surge XT"]);
        // The same plug-in found as CLAP and as VST3 is named once.
        let mut both = before.clone();
        both.put(entry(
            "s.vst3",
            ScanStatus::Ok,
            vec![named("S3", "Surge XT")],
        ));
        both.put(DbEntry {
            format: Format::Clap,
            ..entry(
                "s.clap",
                ScanStatus::Ok,
                vec![PluginInfo {
                    format: Format::Clap,
                    ..named("org.surge", "Surge XT")
                }],
            )
        });
        assert_eq!(both.newly_found(&before), ["Surge XT"]);
        // A plug-in that moved to another file is not new.
        let mut moved = PluginDb::default();
        moved.put(entry(
            "elsewhere.vst3",
            ScanStatus::Ok,
            vec![named("A", "Valhalla")],
        ));
        assert!(moved.newly_found(&before).is_empty());
    }
}
