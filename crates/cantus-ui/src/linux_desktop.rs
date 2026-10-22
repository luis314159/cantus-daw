//! Linux's desktop conventions (ADR 0065): where the user's files live (the XDG base
//! directories), which window system Cantus's own window uses, and the move of an older
//! build's folder to the new places.
//!
//! - **Configuration** (settings, look, layouts, keymap, the updater's state) in
//!   `$XDG_CONFIG_HOME/cantus` (`~/.config/cantus`); **data** (presets, recordings,
//!   autosaves, downloaded sounds) in `$XDG_DATA_HOME/cantus` (`~/.local/share/cantus`).
//!   Builds before round 30 kept both in `~/.local/share/Cantus`: the first start of this
//!   one moves that folder to the data place and copies its top-level files (the settings
//!   and the like) to the configuration place.
//! - **X11 for Cantus's own window**, through XWayland on a Wayland desktop: a plug-in's
//!   editor can only embed into an X11 window, and an editor is kept above Cantus's window
//!   only when both are X11 windows (`WM_TRANSIENT_FOR`). `CANTUS_WAYLAND=1` asks for a
//!   native Wayland window instead (editors then open as plain windows of their own).

use std::ffi::OsString;
use std::path::{Path, PathBuf};

/// The XDG place `var` names, or `$HOME/<fallback>` (the specification's default). An
/// empty or relative value is ignored, as the specification says.
fn base(env: &dyn Fn(&str) -> Option<OsString>, var: &str, fallback: &str) -> PathBuf {
    if let Some(dir) = env(var).map(PathBuf::from).filter(|p| p.is_absolute()) {
        return dir;
    }
    env("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir)
        .join(fallback)
}

/// `$XDG_CONFIG_HOME/cantus`.
pub fn config_dir_with(env: &dyn Fn(&str) -> Option<OsString>) -> PathBuf {
    base(env, "XDG_CONFIG_HOME", ".config").join("cantus")
}

/// `$XDG_DATA_HOME/cantus`.
pub fn data_dir_with(env: &dyn Fn(&str) -> Option<OsString>) -> PathBuf {
    base(env, "XDG_DATA_HOME", ".local/share").join("cantus")
}

/// Where builds before round 30 kept everything: `$XDG_DATA_HOME/Cantus`.
pub fn old_dir_with(env: &dyn Fn(&str) -> Option<OsString>) -> PathBuf {
    base(env, "XDG_DATA_HOME", ".local/share").join("Cantus")
}

fn env(var: &str) -> Option<OsString> {
    std::env::var_os(var).filter(|v| !v.is_empty())
}

/// The configuration folder, after the one-time move of an older build's folder.
pub fn config_dir() -> PathBuf {
    migrate_once();
    config_dir_with(&env)
}

/// The data folder, after the one-time move of an older build's folder.
pub fn data_dir() -> PathBuf {
    migrate_once();
    data_dir_with(&env)
}

fn migrate_once() {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| {
        if let Err(e) = migrate(
            &old_dir_with(&env),
            &config_dir_with(&env),
            &data_dir_with(&env),
        ) {
            log::warn!("moving the older build's folder to the XDG places: {e}");
        }
    });
}

/// Move `old` (everything in one folder) to `data`, and copy its top-level files to
/// `config` — when `old` exists and neither new place does. On a case-insensitive file
/// system `old` and `data` are one folder: then only the copy is made.
pub fn migrate(old: &Path, config: &Path, data: &Path) -> std::io::Result<bool> {
    if !old.is_dir() || config.exists() {
        return Ok(false);
    }
    let same = data.exists() && same_folder(old, data);
    if data.exists() && !same {
        return Ok(false);
    }
    std::fs::create_dir_all(config)?;
    for entry in std::fs::read_dir(old)? {
        let entry = entry?;
        if entry.file_type()?.is_file() {
            std::fs::copy(entry.path(), config.join(entry.file_name()))?;
        }
    }
    if !same {
        if let Some(parent) = data.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::rename(old, data)?;
    }
    log::info!(
        "moved {} to {} (data) and {} (settings)",
        old.display(),
        data.display(),
        config.display()
    );
    Ok(true)
}

fn same_folder(a: &Path, b: &Path) -> bool {
    match (a.canonicalize(), b.canonicalize()) {
        (Ok(a), Ok(b)) => a == b,
        _ => false,
    }
}

/// Run as an AppImage (`$APPIMAGE`, set by its runtime; `$CANTUS_APPDIR`, by its AppRun):
/// put Cantus in the desktop's applications menu — its `.desktop` file pointing at this
/// AppImage, its icons, and the `.cantus` file type — in the user's own XDG folders, so it
/// is found and opens projects like an installed program. Done when the menu has no
/// Cantus yet or its entry names another file (the AppImage moved); never with
/// `CANTUS_NO_DESKTOP_ENTRY=1`. The updater replaces the AppImage at the same path, so the
/// entry stays right across updates.
pub fn integrate_appimage() {
    if env("CANTUS_NO_DESKTOP_ENTRY").is_some() {
        return;
    }
    let (Some(image), Some(appdir)) = (env("APPIMAGE"), env("CANTUS_APPDIR")) else {
        return;
    };
    let share = base(&env, "XDG_DATA_HOME", ".local/share");
    match integrate(Path::new(&image), Path::new(&appdir), &share) {
        Ok(true) => {
            log::info!(
                "added Cantus to the applications menu ({})",
                share.display()
            );
            for (tool, dir) in [
                ("update-desktop-database", share.join("applications")),
                ("update-mime-database", share.join("mime")),
            ] {
                let _ = std::process::Command::new(tool)
                    .arg(dir)
                    .stdout(std::process::Stdio::null())
                    .stderr(std::process::Stdio::null())
                    .spawn();
            }
        }
        Ok(false) => {}
        Err(e) => log::warn!("adding Cantus to the applications menu: {e}"),
    }
}

/// The desktop entry's id, its icon's name and the file type's name (packaging/linux/).
pub const APP_ID: &str = "com.cantusdaw.Cantus";

/// Write the menu entry for `image` from `appdir`'s files into `share` (the user's
/// `$XDG_DATA_HOME`); false when an entry for this very file is there already.
pub fn integrate(image: &Path, appdir: &Path, share: &Path) -> std::io::Result<bool> {
    let entry = share.join("applications").join(format!("{APP_ID}.desktop"));
    let exec = format!("Exec=\"{}\" %f", image.display());
    if std::fs::read_to_string(&entry).is_ok_and(|t| t.lines().any(|l| l == exec)) {
        return Ok(false);
    }
    let source = std::fs::read_to_string(appdir.join(format!("{APP_ID}.desktop")))?;
    let mut text = String::new();
    for line in source.lines() {
        if line.starts_with("Exec=") {
            text.push_str(&exec);
        } else if line.starts_with("TryExec=") {
            continue;
        } else {
            text.push_str(line);
        }
        text.push('\n');
    }
    text.push_str(&format!("X-AppImage-Path={}\n", image.display()));
    std::fs::create_dir_all(entry.parent().unwrap_or(share))?;
    std::fs::write(&entry, text)?;
    // The icons, every size the AppImage carries.
    let icons = appdir.join("usr/share/icons/hicolor");
    if let Ok(sizes) = std::fs::read_dir(&icons) {
        for size in sizes.flatten() {
            let from = size.path().join("apps").join(format!("{APP_ID}.png"));
            if from.is_file() {
                let to = share
                    .join("icons/hicolor")
                    .join(size.file_name())
                    .join("apps");
                std::fs::create_dir_all(&to)?;
                std::fs::copy(&from, to.join(format!("{APP_ID}.png")))?;
            }
        }
    }
    let mime = appdir.join(format!("usr/share/mime/packages/{APP_ID}.xml"));
    if mime.is_file() {
        let to = share.join("mime/packages");
        std::fs::create_dir_all(&to)?;
        std::fs::copy(&mime, to.join(format!("{APP_ID}.xml")))?;
    }
    Ok(true)
}

/// Whether Cantus's own window should be an X11 one: an X server is there (`DISPLAY`, also
/// set by XWayland) and native Wayland was not asked for (`CANTUS_WAYLAND=1`).
pub fn prefer_x11_with(env: &dyn Fn(&str) -> Option<OsString>) -> bool {
    env("DISPLAY").is_some() && env("CANTUS_WAYLAND").is_none_or(|v| v != "1")
}

/// eframe's hook that makes winit open an X11 window, when [`prefer_x11_with`] says so.
pub fn event_loop_hook() -> Option<eframe::EventLoopBuilderHook> {
    if !prefer_x11_with(&env) {
        return None;
    }
    Some(Box::new(|builder| {
        use winit::platform::x11::EventLoopBuilderExtX11;
        builder.with_x11();
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn env_of(pairs: &[(&str, &str)]) -> impl Fn(&str) -> Option<OsString> {
        let map: HashMap<String, String> = pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect();
        move |k| map.get(k).filter(|v| !v.is_empty()).map(OsString::from)
    }

    #[test]
    fn the_xdg_places_and_their_defaults() {
        let e = env_of(&[("HOME", "/home/ana")]);
        assert_eq!(config_dir_with(&e), Path::new("/home/ana/.config/cantus"));
        assert_eq!(
            data_dir_with(&e),
            Path::new("/home/ana/.local/share/cantus")
        );
        assert_eq!(old_dir_with(&e), Path::new("/home/ana/.local/share/Cantus"));
        let e = env_of(&[
            ("HOME", "/home/ana"),
            ("XDG_CONFIG_HOME", "/cfg"),
            ("XDG_DATA_HOME", "/data"),
        ]);
        assert_eq!(config_dir_with(&e), Path::new("/cfg/cantus"));
        assert_eq!(data_dir_with(&e), Path::new("/data/cantus"));
        // A relative value is not a place (the specification): the default is used.
        let e = env_of(&[("HOME", "/home/ana"), ("XDG_DATA_HOME", "rel")]);
        assert_eq!(
            data_dir_with(&e),
            Path::new("/home/ana/.local/share/cantus")
        );
    }

    #[test]
    fn x11_unless_wayland_is_asked_for() {
        assert!(prefer_x11_with(&env_of(&[("DISPLAY", ":0")])));
        assert!(prefer_x11_with(&env_of(&[
            ("DISPLAY", ":0"),
            ("WAYLAND_DISPLAY", "wayland-0")
        ])));
        assert!(!prefer_x11_with(&env_of(&[
            ("DISPLAY", ":0"),
            ("CANTUS_WAYLAND", "1")
        ])));
        assert!(!prefer_x11_with(&env_of(&[(
            "WAYLAND_DISPLAY",
            "wayland-0"
        )])));
    }

    #[test]
    fn an_appimage_puts_itself_in_the_menu_once() {
        let t = tempfile::tempdir().unwrap();
        let appdir = t.path().join("AppDir");
        std::fs::create_dir_all(appdir.join("usr/share/icons/hicolor/48x48/apps")).unwrap();
        std::fs::create_dir_all(appdir.join("usr/share/mime/packages")).unwrap();
        std::fs::write(
            appdir.join(format!("{APP_ID}.desktop")),
            "[Desktop Entry]\nName=Cantus\nExec=cantus %f\nTryExec=cantus\nIcon=com.cantusdaw.Cantus\n",
        )
        .unwrap();
        std::fs::write(
            appdir.join(format!("usr/share/icons/hicolor/48x48/apps/{APP_ID}.png")),
            b"png",
        )
        .unwrap();
        std::fs::write(
            appdir.join(format!("usr/share/mime/packages/{APP_ID}.xml")),
            b"<x/>",
        )
        .unwrap();
        let share = t.path().join("share");
        let image = Path::new("/home/ana/Apps/Cantus-0.2.0-linux-x86_64.AppImage");
        assert!(integrate(image, &appdir, &share).unwrap());
        let entry =
            std::fs::read_to_string(share.join(format!("applications/{APP_ID}.desktop"))).unwrap();
        assert!(entry.contains("Exec=\"/home/ana/Apps/Cantus-0.2.0-linux-x86_64.AppImage\" %f"));
        assert!(!entry.contains("TryExec"), "{entry}");
        assert!(entry.contains("Icon=com.cantusdaw.Cantus"));
        assert!(share
            .join(format!("icons/hicolor/48x48/apps/{APP_ID}.png"))
            .is_file());
        assert!(share.join(format!("mime/packages/{APP_ID}.xml")).is_file());
        // The same file again: nothing to do. Moved elsewhere: the entry follows.
        assert!(!integrate(image, &appdir, &share).unwrap());
        assert!(integrate(Path::new("/opt/Cantus.AppImage"), &appdir, &share).unwrap());
    }

    #[test]
    fn an_older_builds_folder_moves_once() {
        let home = tempfile::tempdir().unwrap();
        let old = home.path().join("share/Cantus");
        let config = home.path().join("config/cantus");
        let data = home.path().join("share/cantus");
        std::fs::create_dir_all(old.join("presets")).unwrap();
        std::fs::write(old.join("settings.json"), b"{}").unwrap();
        std::fs::write(old.join("presets/a.cpreset"), b"x").unwrap();
        // Nothing to do without the old folder, or once a new place exists.
        assert!(!migrate(&home.path().join("none"), &config, &data).unwrap());
        assert!(migrate(&old, &config, &data).unwrap());
        assert_eq!(std::fs::read(config.join("settings.json")).unwrap(), b"{}");
        assert_eq!(std::fs::read(data.join("presets/a.cpreset")).unwrap(), b"x");
        assert!(!migrate(&old, &config, &data).unwrap(), "once");
    }
}
