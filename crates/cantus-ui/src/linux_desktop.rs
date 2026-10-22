//! Linux's desktop conventions (ADR 0065): where the user's files live (the XDG base
//! directories), which window system Cantus's own window uses, and the move of an older
//! build's folder to the new places.
//!
//! - **Configuration** (settings, look, layouts, keymap, the updater's state) in
//!   `$XDG_CONFIG_HOME/cantus` (`~/.config/cantus`); **data** (presets, recordings,
//!   autosaves, caches, downloaded sounds) in `$XDG_DATA_HOME/cantus`
//!   (`~/.local/share/cantus`). Builds before round 30 kept both in `~/.local/share/Cantus`:
//!   the first start of this one moves that folder to the data place and copies its
//!   top-level files (the settings and the like) to the configuration place — in steps that
//!   resume where they stopped; until the move is complete, the old folder stays in use.
//! - **X11 for Cantus's own window**, through XWayland on a Wayland desktop: a plug-in's
//!   editor can only embed into an X11 window, and an editor is kept above Cantus's window
//!   only when both are X11 windows (`WM_TRANSIENT_FOR`). `CANTUS_WAYLAND=1` asks for a
//!   native Wayland window instead (editors then open as plain windows of their own); an X
//!   server that does not answer (a stale `DISPLAY`) with Wayland there means Wayland too.

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

/// The configuration folder (after the move of an older build's folder, when it is done).
pub fn config_dir() -> PathBuf {
    places(&env).0
}

/// The data folder (after the move of an older build's folder, when it is done).
pub fn data_dir() -> PathBuf {
    places(&env).1
}

/// (configuration, data): the XDG places, once the older folder is moved; the older folder
/// for both while the move is not complete (an error, or a folder of that name already in
/// the way), so nothing is ever written beside data left behind. The move is tried once a
/// run, the places read each call (tests change the environment).
fn places(env: &dyn Fn(&str) -> Option<OsString>) -> (PathBuf, PathBuf) {
    let (old, config, data) = (old_dir_with(env), config_dir_with(env), data_dir_with(env));
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| {
        if let Err(e) = migrate(&old, &config, &data) {
            log::warn!("moving the older build's folder to the XDG places: {e}");
        }
    });
    if moved_or_none(&old, &data) {
        (config, data)
    } else {
        (old.clone(), old)
    }
}

/// True when there is no older folder left to move (or it is the data folder itself, on a
/// file system that ignores case).
fn moved_or_none(old: &Path, data: &Path) -> bool {
    !old.is_dir() || same_folder(old, data)
}

/// Where the settings are gathered before they go into place (a sibling of `config`).
fn staging(config: &Path) -> PathBuf {
    config.with_extension("moving")
}

/// Move `old` (everything in one folder) to `data`, and copy its top-level files to
/// `config`. Three steps, each safe to run again, in an order that makes the last one the
/// sign of completion: (1) the top-level files copied into a staging folder beside
/// `config` (anew after any interruption), (2) `old` renamed to `data` (atomic), (3) the
/// staging folder renamed to `config`. A `config` that exists means done; an `old` and a
/// different `data` both there is not ours to merge, and is left (the old folder stays in
/// use, [`places`]). True when the move completed now.
pub fn migrate(old: &Path, config: &Path, data: &Path) -> std::io::Result<bool> {
    if config.exists() {
        return Ok(false);
    }
    let staged = staging(config);
    if old.is_dir() {
        let same = data.exists() && same_folder(old, data);
        if data.exists() && !same {
            return Ok(false);
        }
        // 1. The settings, into the staging folder.
        if staged.exists() {
            std::fs::remove_dir_all(&staged)?;
        }
        std::fs::create_dir_all(&staged)?;
        for entry in std::fs::read_dir(old)? {
            let entry = entry?;
            if entry.file_type()?.is_file() {
                std::fs::copy(entry.path(), staged.join(entry.file_name()))?;
            }
        }
        // 2. The data, moved in one rename.
        if !same {
            if let Some(parent) = data.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::rename(old, data)?;
        }
    } else if !staged.is_dir() {
        // Nothing to move and nothing begun.
        return Ok(false);
    }
    // 3. The settings into place: last, so a configuration folder means the move is done.
    if let Some(parent) = config.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::rename(&staged, config)?;
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

/// The desktop entry's id, its icon's name and the file type's name (packaging/linux/).
pub const APP_ID: &str = "com.cantusdaw.Cantus";

/// Run as an AppImage (`$APPIMAGE`, set by its runtime; `$CANTUS_APPDIR`, by its AppRun):
/// put Cantus in the desktop's applications menu — its `.desktop` file pointing at this
/// AppImage, its icons and the `.cantus` file type — in the user's own XDG folders, once
/// ([`integrate`]). Never with `CANTUS_NO_DESKTOP_ENTRY=1`.
pub fn integrate_appimage() {
    if env("CANTUS_NO_DESKTOP_ENTRY").is_some() {
        return;
    }
    let (Some(image), Some(appdir)) = (env("APPIMAGE"), env("CANTUS_APPDIR")) else {
        return;
    };
    let share = base(&env, "XDG_DATA_HOME", ".local/share");
    let flag = config_dir().join("desktop-entry");
    match integrate(Path::new(&image), Path::new(&appdir), &share, &flag) {
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

/// One argument of an `Exec=` line, as the Desktop Entry specification wants it: in double
/// quotes, with `"`, `` ` ``, `$` and `\` escaped by a backslash, `%` doubled — and then
/// the string type's own rule on top, a backslash doubled (so a literal backslash is four).
pub fn exec_arg(arg: &str) -> String {
    let mut quoted = String::from("\"");
    for c in arg.chars() {
        match c {
            '"' | '`' | '$' | '\\' => {
                quoted.push('\\');
                quoted.push(c);
            }
            '%' => quoted.push_str("%%"),
            _ => quoted.push(c),
        }
    }
    quoted.push('"');
    quoted.replace('\\', "\\\\")
}

/// A value of the string type (a backslash doubled, line breaks written `\n`).
fn string_value(s: &str) -> String {
    s.replace('\\', "\\\\").replace('\n', "\\n")
}

/// Write the menu entry for `image` from `appdir`'s files into `share` (the user's
/// `$XDG_DATA_HOME`), remembering in `flag` that it was offered: once only — an entry the
/// user removed stays removed — and again only to follow the AppImage to a new path while
/// the entry is still there. True when it was written now.
pub fn integrate(image: &Path, appdir: &Path, share: &Path, flag: &Path) -> std::io::Result<bool> {
    let entry = share.join("applications").join(format!("{APP_ID}.desktop"));
    let here = image.display().to_string();
    if let Ok(offered) = std::fs::read_to_string(flag) {
        if !entry.exists() || offered.trim_end_matches('\n') == here {
            return Ok(false);
        }
    }
    let exec = format!("Exec={} %f", exec_arg(&here));
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
    text.push_str(&format!("X-AppImage-Path={}\n", string_value(&here)));
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
    if let Some(dir) = flag.parent() {
        std::fs::create_dir_all(dir)?;
    }
    std::fs::write(flag, format!("{here}\n"))?;
    Ok(true)
}

/// Whether the X server `display` names answers: its socket in `socket_dir`
/// (`/tmp/.X11-unix/X<n>`, or the abstract socket of that name) for a local display, port
/// 6000 + n for `host:n`. A `DISPLAY` left over from another session names a server that
/// is gone.
pub fn x_server_answers(display: &str, socket_dir: &Path) -> bool {
    let Some((host, rest)) = display.rsplit_once(':') else {
        return false;
    };
    let Some(n) = rest.split('.').next().and_then(|n| n.parse::<u16>().ok()) else {
        return false;
    };
    if host.is_empty() || host == "unix" {
        use std::os::unix::net::UnixStream;
        let path = socket_dir.join(format!("X{n}"));
        if UnixStream::connect(&path).is_ok() {
            return true;
        }
        use std::os::linux::net::SocketAddrExt;
        return std::os::unix::net::SocketAddr::from_abstract_name(
            path.to_string_lossy().as_bytes(),
        )
        .and_then(|a| UnixStream::connect_addr(&a))
        .is_ok();
    }
    use std::net::ToSocketAddrs;
    (host, 6000 + n)
        .to_socket_addrs()
        .ok()
        .and_then(|mut a| a.next())
        .is_some_and(|a| {
            std::net::TcpStream::connect_timeout(&a, std::time::Duration::from_secs(1)).is_ok()
        })
}

/// Whether Cantus's own window should be an X11 one: an X server is named (`DISPLAY`, also
/// set by XWayland), native Wayland was not asked for (`CANTUS_WAYLAND=1`), and the X
/// server answers — or there is no Wayland to fall back to.
pub fn prefer_x11_with(
    env: &dyn Fn(&str) -> Option<OsString>,
    answers: &dyn Fn(&str) -> bool,
) -> bool {
    let Some(display) = env("DISPLAY") else {
        return false;
    };
    if env("CANTUS_WAYLAND").is_some_and(|v| v == "1") {
        return false;
    }
    answers(&display.to_string_lossy()) || env("WAYLAND_DISPLAY").is_none()
}

/// eframe's hook that makes winit open an X11 window, when [`prefer_x11_with`] says so.
pub fn event_loop_hook() -> Option<eframe::EventLoopBuilderHook> {
    let answers = |d: &str| x_server_answers(d, Path::new("/tmp/.X11-unix"));
    if !prefer_x11_with(&env, &answers) {
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
    fn x11_unless_wayland_is_asked_for_or_the_x_server_is_gone() {
        let yes = |_: &str| true;
        let no = |_: &str| false;
        assert!(prefer_x11_with(&env_of(&[("DISPLAY", ":0")]), &yes));
        let both = env_of(&[("DISPLAY", ":0"), ("WAYLAND_DISPLAY", "wayland-0")]);
        assert!(prefer_x11_with(&both, &yes));
        // A stale DISPLAY beside a working Wayland: Wayland.
        assert!(!prefer_x11_with(&both, &no));
        // No Wayland to fall back to: X11 still (it will say why it cannot start).
        assert!(prefer_x11_with(&env_of(&[("DISPLAY", ":0")]), &no));
        let asked = env_of(&[("DISPLAY", ":0"), ("CANTUS_WAYLAND", "1")]);
        assert!(!prefer_x11_with(&asked, &yes));
        assert!(!prefer_x11_with(&env_of(&[("WAYLAND_DISPLAY", "w")]), &yes));
    }

    #[test]
    fn an_x_server_answers_on_its_socket() {
        let dir = tempfile::tempdir().unwrap();
        let _server = std::os::unix::net::UnixListener::bind(dir.path().join("X5")).unwrap();
        assert!(x_server_answers(":5", dir.path()));
        assert!(x_server_answers(":5.0", dir.path()));
        assert!(x_server_answers("unix:5", dir.path()));
        assert!(!x_server_answers(":6", dir.path()), "no server there");
        assert!(!x_server_answers("garbage", dir.path()));
    }

    #[test]
    fn exec_arguments_are_escaped_as_the_specification_says() {
        assert_eq!(
            exec_arg("/opt/Cantus.AppImage"),
            r#""/opt/Cantus.AppImage""#
        );
        assert_eq!(exec_arg("/a b/$x"), r#""/a b/\\$x""#);
        assert_eq!(exec_arg("q\"`"), r#""q\\"\\`""#);
        // A literal backslash is four; a percent sign doubled.
        assert_eq!(exec_arg(r"a\b 100%"), r#""a\\\\b 100%%""#);
    }

    fn appdir(t: &Path) -> PathBuf {
        let appdir = t.join("AppDir");
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
        appdir
    }

    #[test]
    fn an_appimage_offers_its_menu_entry_once_and_respects_a_removal() {
        let t = tempfile::tempdir().unwrap();
        let appdir = appdir(t.path());
        let share = t.path().join("share");
        let flag = t.path().join("config/desktop-entry");
        let entry = share.join(format!("applications/{APP_ID}.desktop"));
        let image = Path::new("/home/ana/Apps/Cantus $1.AppImage");
        assert!(integrate(image, &appdir, &share, &flag).unwrap());
        let text = std::fs::read_to_string(&entry).unwrap();
        assert!(
            text.contains(r#"Exec="/home/ana/Apps/Cantus \\$1.AppImage" %f"#),
            "{text}"
        );
        assert!(!text.contains("TryExec"), "{text}");
        assert!(share
            .join(format!("icons/hicolor/48x48/apps/{APP_ID}.png"))
            .is_file());
        assert!(share.join(format!("mime/packages/{APP_ID}.xml")).is_file());
        // Again, the same file: nothing written.
        assert!(!integrate(image, &appdir, &share, &flag).unwrap());
        // Moved: the entry follows.
        let moved = Path::new("/opt/Cantus.AppImage");
        assert!(integrate(moved, &appdir, &share, &flag).unwrap());
        // Removed by the user: it stays removed, wherever the AppImage goes.
        std::fs::remove_file(&entry).unwrap();
        assert!(!integrate(moved, &appdir, &share, &flag).unwrap());
        assert!(!integrate(image, &appdir, &share, &flag).unwrap());
        assert!(!entry.exists());
    }

    fn old_folder(t: &Path) -> (PathBuf, PathBuf, PathBuf) {
        let old = t.join("share/Cantus");
        std::fs::create_dir_all(old.join("presets")).unwrap();
        std::fs::write(old.join("settings.json"), b"{}").unwrap();
        std::fs::write(old.join("presets/a.cpreset"), b"x").unwrap();
        (old, t.join("config/cantus"), t.join("share/cantus"))
    }

    fn moved(config: &Path, data: &Path) {
        assert_eq!(std::fs::read(config.join("settings.json")).unwrap(), b"{}");
        assert_eq!(std::fs::read(data.join("presets/a.cpreset")).unwrap(), b"x");
    }

    #[test]
    fn an_older_builds_folder_moves_once() {
        let t = tempfile::tempdir().unwrap();
        let (old, config, data) = old_folder(t.path());
        assert!(!migrate(&t.path().join("none"), &config, &data).unwrap());
        assert!(
            !moved_or_none(&old, &data),
            "the old folder in use until moved"
        );
        assert!(migrate(&old, &config, &data).unwrap());
        moved(&config, &data);
        assert!(!old.exists() && !staging(&config).exists());
        assert!(moved_or_none(&old, &data));
        assert!(!migrate(&old, &config, &data).unwrap(), "once");
    }

    /// Cut during step 1 (the settings half copied): the next start does it all again.
    #[test]
    fn a_move_cut_while_copying_the_settings_resumes() {
        let t = tempfile::tempdir().unwrap();
        let (old, config, data) = old_folder(t.path());
        std::fs::create_dir_all(staging(&config)).unwrap();
        std::fs::write(staging(&config).join("settings.json"), b"{").unwrap();
        assert!(migrate(&old, &config, &data).unwrap());
        moved(&config, &data);
    }

    /// Cut after step 2 (the data moved, the settings still staged): the next start puts
    /// the settings into place.
    #[test]
    fn a_move_cut_after_the_data_moved_resumes() {
        let t = tempfile::tempdir().unwrap();
        let (old, config, data) = old_folder(t.path());
        std::fs::create_dir_all(staging(&config)).unwrap();
        std::fs::copy(
            old.join("settings.json"),
            staging(&config).join("settings.json"),
        )
        .unwrap();
        std::fs::rename(&old, &data).unwrap();
        assert!(migrate(&old, &config, &data).unwrap());
        moved(&config, &data);
        assert!(moved_or_none(&old, &data));
    }

    /// A data folder already in the way: nothing is moved and the old folder stays in use,
    /// so nothing is written beside what was left behind.
    #[test]
    fn a_move_that_cannot_happen_keeps_the_old_folder_in_use() {
        let t = tempfile::tempdir().unwrap();
        let (old, config, data) = old_folder(t.path());
        std::fs::create_dir_all(&data).unwrap();
        assert!(!migrate(&old, &config, &data).unwrap());
        assert!(!config.exists() && old.join("settings.json").is_file());
        assert!(!moved_or_none(&old, &data));
    }
}
