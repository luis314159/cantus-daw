//! Where the look is kept: `appearance.json`, next to the settings (ADR 0024 §2).
//!
//! - **Reading** at startup (on the splash's thread): the appearance file; else a
//!   `theme.json` from before (migrated, never deleted); else the default.
//! - **Migration**: a `theme.json` that is exactly a shipped preset (what earlier releases
//!   wrote on first run) becomes that preset for its mode — no user theme is invented, and
//!   the default mode is left alone; a theme the user changed becomes one of their own
//!   themes, assigned to the mode it was made for (`kit::appearance`).
//! - **Watching**: the file is the user's to edit by hand; a change on disk is shown
//!   within a second.

use crate::kit::appearance::{Appearance, Mode, PRESETS};
use crate::theme::Theme;
use egui::Context;
use std::path::{Path, PathBuf};
use std::time::{Instant, SystemTime};

/// Where the appearance lives.
pub fn path(dir: &Path) -> PathBuf {
    dir.join("appearance.json")
}

/// Where the theme lived before.
pub fn legacy_path(dir: &Path) -> PathBuf {
    dir.join("theme.json")
}

/// Read the appearance kept in `dir` (see the module docs). Returns it and the path it is
/// to be written to.
pub fn load(dir: &Path) -> (Appearance, PathBuf) {
    let p = path(dir);
    if let Ok(text) = std::fs::read_to_string(&p) {
        match Appearance::from_json(&text) {
            Ok(a) => return (a, p),
            Err(e) => log::warn!("appearance.json is not usable ({e}); using the defaults"),
        }
    }
    if let Ok(text) = std::fs::read_to_string(legacy_path(dir)) {
        match migrate(&text) {
            Ok(a) => return (a, p),
            Err(e) => log::warn!("theme.json is not usable ({e}); using the defaults"),
        }
    }
    (Appearance::default(), p)
}

/// An appearance from the text of an old `theme.json` (or of an appearance file).
pub fn migrate(text: &str) -> Result<Appearance, String> {
    let v: serde_json::Value = serde_json::from_str(text).map_err(|e| e.to_string())?;
    if v.get("version").is_some() {
        return Appearance::from_json(text);
    }
    let mut t: Theme = serde_json::from_value(v).map_err(|e| e.to_string())?;
    t.fill_missing();
    for (id, make) in PRESETS {
        if make().palette() == t.palette() && t.knobs.is_neutral() {
            let mut a = Appearance::default();
            let mode = if t.dark { Mode::Dark } else { Mode::Light };
            let base = format!("preset:{id}");
            // The preset a mode ships with is no choice at all: leave the mode alone.
            if a.slot(mode).base != base {
                a.assign(mode, base);
                a.mode = mode;
            }
            a.logo = t.logo;
            return Ok(a);
        }
    }
    Ok(Appearance::from_legacy(t))
}

/// Write the appearance.
pub fn save(a: &Appearance, path: &Path) -> Result<(), String> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    std::fs::write(path, a.to_json()).map_err(|e| e.to_string())
}

/// Whether the operating system is in dark mode (unknown counts as light).
pub fn system_dark(ctx: &Context) -> bool {
    ctx.system_theme() == Some(egui::Theme::Dark)
}

/// Notices when the appearance file changes on disk.
#[derive(Debug, Default)]
pub struct Watch {
    path: Option<PathBuf>,
    mtime: Option<SystemTime>,
    last: Option<Instant>,
}

impl Watch {
    pub fn new(path: PathBuf) -> Watch {
        let mtime = std::fs::metadata(&path).and_then(|m| m.modified()).ok();
        Watch {
            path: Some(path),
            mtime,
            last: Some(Instant::now()),
        }
    }

    /// The application wrote the file itself: that is not a change to pick up.
    pub fn saw_write(&mut self, path: &Path) {
        self.mtime = std::fs::metadata(path).and_then(|m| m.modified()).ok();
    }

    /// The file as it now reads, when it changed since the last look (at most once a
    /// second).
    pub fn poll(&mut self) -> Option<Result<Appearance, String>> {
        let path = self.path.as_ref()?;
        if self.last.is_some_and(|l| l.elapsed().as_secs_f32() < 1.0) {
            return None;
        }
        self.last = Some(Instant::now());
        let mtime = std::fs::metadata(path).and_then(|m| m.modified()).ok();
        if mtime.is_some() && mtime != self.mtime {
            self.mtime = mtime;
            Some(
                std::fs::read_to_string(path)
                    .map_err(|e| e.to_string())
                    .and_then(|t| Appearance::from_json(&t)),
            )
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme;

    #[test]
    fn a_theme_json_written_as_a_preset_becomes_that_preset() {
        // What earlier releases wrote on first run: Paper, the light default.
        let paper = serde_json::to_string(&theme::paper()).unwrap();
        let a = migrate(&paper).unwrap();
        assert!(
            a.themes.is_empty(),
            "no user theme invented for an untouched file"
        );
        assert_eq!(
            a.mode,
            Appearance::default().mode,
            "the default mode is left alone"
        );
        // A dark preset the user chose is kept, and shown.
        let ink = serde_json::to_string(&theme::ink_and_gold()).unwrap();
        let a = migrate(&ink).unwrap();
        assert_eq!(a.mode, Mode::Dark);
        assert_eq!(a.dark.base, "preset:ink_gold");
    }

    #[test]
    fn a_theme_the_user_changed_becomes_their_own() {
        let mut t = theme::paper();
        t.name = "Mine".into();
        t.panel = "#123456".into();
        t.kit.clear();
        let a = migrate(&serde_json::to_string(&t).unwrap()).unwrap();
        assert_eq!(a.themes.len(), 1);
        assert_eq!(a.light.base, "user:Mine");
        assert_eq!(a.resolve(false).panel, "#123456");
    }

    #[test]
    fn the_file_is_found_migrated_and_written_back() {
        let dir = tempfile::tempdir().unwrap();
        let (a, p) = load(dir.path());
        assert_eq!(a, Appearance::default(), "nothing on disk: the default");
        std::fs::write(
            legacy_path(dir.path()),
            serde_json::to_string(&theme::parchment()).unwrap(),
        )
        .unwrap();
        let (a, _) = load(dir.path());
        assert_eq!(a.light.base, "preset:parchment");
        save(&a, &p).unwrap();
        let (back, _) = load(dir.path());
        assert_eq!(back, a);
        assert!(
            legacy_path(dir.path()).exists(),
            "the old file is never deleted"
        );
    }
}
