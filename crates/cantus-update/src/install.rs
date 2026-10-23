//! How this copy of Cantus was installed (on each system), where a portable copy keeps
//! its files, and applying an update when the app quits: Windows hands over to the
//! installer, an AppImage is replaced, a Mac opens the disk image (ADR 0067).

use std::path::{Path, PathBuf};

/// Beside `cantus.exe` in the portable zip: its presence makes the copy portable.
pub const PORTABLE_MARKER: &str = "portable.txt";
/// The installer writes this beside `cantus.exe`.
pub const UNINSTALLER: &str = "uninstall.exe";
/// A portable copy keeps settings, presets and autosaves here, beside the executable.
pub const PORTABLE_DATA: &str = "data";

/// What the marker says, for whoever opens it.
pub const PORTABLE_MARKER_TEXT: &str =
    "This copy of Cantus is portable: it keeps its settings, presets and \
autosaves in the \"data\" folder beside cantus.exe, and leaves nothing in the user's profile.\r\n\
Delete this file to make it use the profile like an installed Cantus.\r\n\
\r\n\
Esta copia de Cantus es portatil: guarda sus ajustes, presets y copias automaticas en la \
carpeta \"data\" junto a cantus.exe, y no deja nada en el perfil del usuario.\r\n\
Borra este archivo para que use el perfil como un Cantus instalado.\r\n";

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum InstallKind {
    /// Windows, installed by the setup program: updates install themselves.
    Installed,
    /// Windows, unzipped from the portable zip: updates are offered, the user unzips them.
    Portable,
    /// Linux, run from an AppImage (`$APPIMAGE`): an update replaces that file (ADR 0067).
    AppImage,
    /// Linux, installed by the system's package manager or Flatpak: it updates Cantus;
    /// the app only says a new version exists and where the AppImage is.
    Package,
    /// macOS, a `Cantus.app`: an update's disk image is downloaded, verified and opened
    /// for the user to drag the new app over the old one.
    MacBundle,
    /// Built from source (`cargo run`): no automatic checks.
    Development,
}

impl InstallKind {
    /// Windows's kind, from what sits beside the executable.
    pub fn detect(exe_dir: &Path) -> InstallKind {
        if exe_dir.join(PORTABLE_MARKER).is_file() {
            InstallKind::Portable
        } else if exe_dir.join(UNINSTALLER).is_file() {
            InstallKind::Installed
        } else {
            InstallKind::Development
        }
    }

    /// The kind of the copy at `exe` on `os` (`std::env::consts::OS`), given the
    /// AppImage this process runs from ([`own_appimage`]) and whether `$FLATPAK_ID` is set.
    pub fn detect_on(os: &str, exe: &Path, appimage: Option<&Path>, flatpak: bool) -> InstallKind {
        match os {
            "windows" => exe
                .parent()
                .map_or(InstallKind::Development, InstallKind::detect),
            "linux" => {
                if flatpak {
                    InstallKind::Package
                } else if built_from_source(exe) {
                    InstallKind::Development
                } else if appimage.is_some() {
                    InstallKind::AppImage
                } else {
                    InstallKind::Package
                }
            }
            "macos" if in_app_bundle(exe) => InstallKind::MacBundle,
            _ => InstallKind::Development,
        }
    }

    /// This process's own kind.
    pub fn current() -> InstallKind {
        let Ok(exe) = std::env::current_exe() else {
            return InstallKind::Development;
        };
        let appimage = current_appimage();
        let flatpak = std::env::var_os("FLATPAK_ID").is_some();
        InstallKind::detect_on(std::env::consts::OS, &exe, appimage.as_deref(), flatpak)
    }

    /// Whether this copy downloads an update and applies it itself (the others are shown
    /// the release's page).
    pub fn updates_itself(self) -> bool {
        matches!(
            self,
            InstallKind::Installed | InstallKind::AppImage | InstallKind::MacBundle
        )
    }
}

/// The AppImage this process runs from, or `None`. `$APPIMAGE` alone is not enough: every
/// child of any AppImage program inherits it (a terminal inside an AppImage editor), so a
/// Cantus started from there would take that other program's file for its own. It counts
/// only when this executable lies inside the AppImage's mounted folder (`$CANTUS_APPDIR`,
/// which Cantus's AppRun exports, or the runtime's `$APPDIR`), is not a cargo build, and
/// `$APPIMAGE` names a file.
pub fn own_appimage(
    exe: &Path,
    appimage: Option<&Path>,
    appdirs: &[Option<&Path>],
) -> Option<PathBuf> {
    let image = appimage.filter(|p| p.is_file())?;
    if built_from_source(exe) {
        return None;
    }
    let exe = exe.canonicalize().unwrap_or_else(|_| exe.to_path_buf());
    let inside = appdirs.iter().flatten().any(|d| {
        let d = d.canonicalize().unwrap_or_else(|_| d.to_path_buf());
        !d.as_os_str().is_empty() && exe.starts_with(&d)
    });
    inside.then(|| image.to_path_buf())
}

/// [`own_appimage`] for this process, from its environment.
pub fn current_appimage() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    let var = |k: &str| std::env::var_os(k).map(PathBuf::from);
    let (image, cantus_dir, dir) = (var("APPIMAGE"), var("CANTUS_APPDIR"), var("APPDIR"));
    own_appimage(
        &exe,
        image.as_deref(),
        &[cantus_dir.as_deref(), dir.as_deref()],
    )
}

/// `…/target/debug/cantus`, `…/target/x86_64-unknown-linux-gnu/release/cantus`, and the
/// workspace's tests and examples beside them (`…/debug/deps/app_flows-…`,
/// `…/release/examples/shoot`): a cargo build, not something a package manager put there.
pub fn built_from_source(exe: &Path) -> bool {
    let parts: Vec<_> = exe.components().map(|c| c.as_os_str()).collect();
    let mut up = parts.iter().rev().skip(1);
    let profile = match up.next() {
        Some(p) if *p == "deps" || *p == "examples" => up.next(),
        p => p,
    };
    profile.is_some_and(|p| *p == "debug" || *p == "release") && up.take(2).any(|p| *p == "target")
}

/// `…/Cantus.app/Contents/MacOS/cantus`.
pub fn in_app_bundle(exe: &Path) -> bool {
    let mut up = exe.ancestors().skip(1);
    let (Some(macos), Some(contents), Some(app)) = (up.next(), up.next(), up.next()) else {
        return false;
    };
    macos.file_name().is_some_and(|n| n == "MacOS")
        && contents.file_name().is_some_and(|n| n == "Contents")
        && app.extension().is_some_and(|e| e == "app")
}

/// The folder holding the running executable.
pub fn exe_dir() -> Option<PathBuf> {
    std::env::current_exe()
        .ok()?
        .parent()
        .map(Path::to_path_buf)
}

/// Where a portable copy keeps the user's files, or `None` when this copy is not
/// portable (it then uses the profile, `%LOCALAPPDATA%\Cantus`).
pub fn portable_data_dir() -> Option<PathBuf> {
    let dir = exe_dir()?;
    portable_data_dir_in(&dir)
}

pub fn portable_data_dir_in(exe_dir: &Path) -> Option<PathBuf> {
    exe_dir
        .join(PORTABLE_MARKER)
        .is_file()
        .then(|| exe_dir.join(PORTABLE_DATA))
}

/// Where downloads wait until they are installed.
pub fn updates_dir(data_dir: &Path) -> PathBuf {
    data_dir.join("updates")
}

/// The installer's command line for an update: silent (`/S`, NSIS's own switch), the
/// components chosen at the first install kept (`/UPDATE`), and Cantus opened again when
/// it is done if the user asked to restart now (`/RELAUNCH`).
pub fn installer_args(relaunch: bool) -> Vec<&'static str> {
    let mut a = vec!["/S", "/UPDATE"];
    if relaunch {
        a.push("/RELAUNCH");
    }
    a
}

/// Start the installer and let it outlive this process; it waits for `cantus.exe` to be
/// closed before replacing it.
pub fn launch(installer: &Path, relaunch: bool) -> std::io::Result<()> {
    let mut cmd = std::process::Command::new(installer);
    cmd.args(installer_args(relaunch));
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const DETACHED_PROCESS: u32 = 0x0000_0008;
        const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;
        cmd.creation_flags(DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP);
    }
    cmd.spawn().map(|_| ())
}

/// Apply a verified update file the way this copy's kind does it: Windows runs the
/// installer; an AppImage is replaced by the new one (and started again when `relaunch`);
/// a Mac opens the disk image for the user (replacing a signed `Cantus.app` in place, as
/// Sparkle does, is for later — ADR 0067 §3). The others have nothing to apply.
pub fn apply(kind: InstallKind, file: &Path, relaunch: bool) -> std::io::Result<()> {
    match kind {
        InstallKind::Installed => launch(file, relaunch),
        InstallKind::AppImage => {
            let current = current_appimage()
                .ok_or_else(|| std::io::Error::other("not running from an AppImage"))?;
            replace_appimage(&current, file)?;
            if relaunch {
                detached(relaunch_after_exit(&current, std::process::id()))?;
            }
            Ok(())
        }
        InstallKind::MacBundle => {
            let mut open = std::process::Command::new("open");
            open.arg(file);
            detached(open)
        }
        InstallKind::Portable | InstallKind::Package | InstallKind::Development => {
            Err(std::io::Error::new(
                std::io::ErrorKind::Unsupported,
                "this copy does not update itself",
            ))
        }
    }
}

/// Start `program` once process `pid` has exited: the new AppImage opens after this Cantus
/// has closed, never beside it (two would contend for the audio device and the autosave).
pub fn relaunch_after_exit(program: &Path, pid: u32) -> std::process::Command {
    let mut cmd = std::process::Command::new("sh");
    cmd.arg("-c")
        .arg("while kill -0 \"$1\" 2>/dev/null; do sleep 0.2; done; exec \"$0\"")
        .arg(program)
        .arg(pid.to_string());
    cmd
}

fn detached(mut cmd: std::process::Command) -> std::io::Result<()> {
    cmd.stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null());
    cmd.spawn().map(|_| ())
}

/// Where the replaced AppImage waits until the new one has started: `Cantus.AppImage.old`.
pub fn previous_appimage(current: &Path) -> PathBuf {
    let mut name = current.file_name().unwrap_or_default().to_os_string();
    name.push(".old");
    current.with_file_name(name)
}

/// Put the verified `new` AppImage where `current` is, atomically: the new file is moved
/// beside the old one (renamed when both are on one file system, else copied and the
/// download removed — it is not left in the updates folder), made executable, flushed, the old one is
/// kept as [`previous_appimage`] (a hard link, or a copy where links are refused), and a
/// rename swaps the new one in — at no moment is there no Cantus at that path. The running
/// program keeps its own (old) file open, so nothing changes under it. The old copy is
/// removed by [`forget_previous_appimage`] once the new one has started; until then the
/// user can rename it back. A folder Cantus may not write in (a system-wide `/opt`) fails
/// here with nothing changed, and the app points at the release's page.
pub fn replace_appimage(current: &Path, new: &Path) -> std::io::Result<()> {
    let dir = current
        .parent()
        .ok_or_else(|| std::io::Error::other("the AppImage has no folder"))?;
    let name = current.file_name().unwrap_or_default().to_string_lossy();
    let staged = dir.join(format!(".{name}.new-{}", std::process::id()));
    let mut moved = false;
    let result = (|| {
        if std::fs::rename(new, &staged).is_ok() {
            moved = true;
        } else {
            std::fs::copy(new, &staged)?;
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&staged, std::fs::Permissions::from_mode(0o755))?;
        }
        std::fs::OpenOptions::new()
            .write(true)
            .open(&staged)?
            .sync_all()?;
        let old = previous_appimage(current);
        let _ = std::fs::remove_file(&old);
        if std::fs::hard_link(current, &old).is_err() {
            std::fs::copy(current, &old)?;
        }
        std::fs::rename(&staged, current)
    })();
    match &result {
        // Nothing changed: the download is put back where it was, for another try.
        Err(_) if moved => {
            if std::fs::rename(&staged, new).is_err() {
                let _ = std::fs::remove_file(&staged);
            }
        }
        Err(_) => {
            let _ = std::fs::remove_file(&staged);
        }
        Ok(()) if !moved => {
            let _ = std::fs::remove_file(new);
        }
        Ok(()) => {}
    }
    result
}

/// Whether Cantus may write in `dir` (where its AppImage is): checked before an update is
/// downloaded, so a copy in a folder it may not change (a system-wide `/opt`) is told at
/// once, not at quit.
pub fn writable_dir(dir: &Path) -> bool {
    let probe = dir.join(format!(".cantus-write-test-{}", std::process::id()));
    let ok = std::fs::write(&probe, b"").is_ok();
    let _ = std::fs::remove_file(&probe);
    ok
}

/// The folder of the AppImage this process runs from, when it runs from one.
pub fn appimage_dir() -> Option<PathBuf> {
    current_appimage()?.parent().map(Path::to_path_buf)
}

/// The new AppImage has started: the old one it replaced is no longer needed. Returns
/// whether there was one.
pub fn forget_previous_appimage(current: &Path) -> bool {
    std::fs::remove_file(previous_appimage(current)).is_ok()
}

/// At start, for this process: an AppImage that replaced an older one removes it.
pub fn after_start(kind: InstallKind) {
    if kind == InstallKind::AppImage {
        if let Some(p) = current_appimage() {
            if forget_previous_appimage(&p) {
                log::info!("update: the previous AppImage was removed; this one started");
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_kind_is_read_from_what_sits_beside_the_executable() {
        let dir = tempfile::tempdir().unwrap();
        let d = dir.path();
        assert_eq!(InstallKind::detect(d), InstallKind::Development);
        assert_eq!(portable_data_dir_in(d), None);
        std::fs::write(d.join(UNINSTALLER), b"MZ").unwrap();
        assert_eq!(InstallKind::detect(d), InstallKind::Installed);
        std::fs::write(d.join(PORTABLE_MARKER), PORTABLE_MARKER_TEXT).unwrap();
        assert_eq!(InstallKind::detect(d), InstallKind::Portable);
        assert_eq!(portable_data_dir_in(d), Some(d.join("data")));
    }

    #[test]
    fn each_system_reads_its_own_kind() {
        let dir = tempfile::tempdir().unwrap();
        let d = dir.path();
        let image = d.join("Cantus.AppImage");
        std::fs::write(&image, b"\x7fELF").unwrap();
        let p = Path::new;
        // Linux: an AppImage, unless Flatpak runs it; a cargo build; anything else is a
        // package (the distribution's, or one built by hand into /usr/local).
        let mounted = p("/tmp/.mount_CantusX/usr/bin/cantus");
        assert_eq!(
            InstallKind::detect_on("linux", mounted, Some(image.as_path()), false),
            InstallKind::AppImage
        );
        assert_eq!(
            InstallKind::detect_on("linux", mounted, Some(image.as_path()), true),
            InstallKind::Package
        );
        // Even with $APPIMAGE set, a cargo build is a development copy.
        assert_eq!(
            InstallKind::detect_on(
                "linux",
                p("/home/a/cantus/target/debug/cantus"),
                Some(image.as_path()),
                false
            ),
            InstallKind::Development
        );
        for dev in [
            "/home/a/cantus/target/debug/cantus",
            "/home/a/cantus/target/release/cantus",
            "/home/a/cantus/target/x86_64-unknown-linux-gnu/release/cantus",
            // A test or an example of the workspace: a test that builds the app must not
            // ask GitHub for updates (Linux CI's idle-window test saw the check tick).
            "/home/runner/work/cantus-daw/cantus-daw/target/debug/deps/app_flows-0a1b2c3d",
            "/home/a/cantus/target/release/examples/shoot",
        ] {
            assert_eq!(
                InstallKind::detect_on("linux", p(dev), None, false),
                InstallKind::Development,
                "{dev}"
            );
        }
        assert_eq!(
            InstallKind::detect_on("linux", p("/opt/release/cantus"), None, false),
            InstallKind::Package
        );
        // macOS: inside an .app bundle, or built from source.
        assert_eq!(
            InstallKind::detect_on(
                "macos",
                p("/Applications/Cantus.app/Contents/MacOS/cantus"),
                None,
                false
            ),
            InstallKind::MacBundle
        );
        assert_eq!(
            InstallKind::detect_on(
                "macos",
                p("/Users/a/cantus/target/release/cantus"),
                None,
                false
            ),
            InstallKind::Development
        );
        // Windows reads what sits beside the program, as before.
        assert_eq!(
            InstallKind::detect_on("windows", &d.join("cantus.exe"), None, false),
            InstallKind::Development
        );
        std::fs::write(d.join(UNINSTALLER), b"MZ").unwrap();
        assert_eq!(
            InstallKind::detect_on("windows", &d.join("cantus.exe"), None, false),
            InstallKind::Installed
        );
        assert_eq!(
            InstallKind::detect_on("freebsd", p("/usr/local/bin/cantus"), None, false),
            InstallKind::Development
        );
        let updates: Vec<_> = [
            InstallKind::Installed,
            InstallKind::Portable,
            InstallKind::AppImage,
            InstallKind::Package,
            InstallKind::MacBundle,
            InstallKind::Development,
        ]
        .into_iter()
        .filter(|k| k.updates_itself())
        .collect();
        assert_eq!(
            updates,
            [
                InstallKind::Installed,
                InstallKind::AppImage,
                InstallKind::MacBundle
            ]
        );
    }

    /// `$APPIMAGE` is inherited by every child of any AppImage program: it names this
    /// Cantus's file only when the executable lies inside the mounted AppImage.
    #[test]
    fn only_the_appimage_this_program_runs_from_is_its_own() {
        let dir = tempfile::tempdir().unwrap();
        let d = dir.path();
        let image = d.join("Other.AppImage");
        std::fs::write(&image, b"ELF").unwrap();
        let mount = d.join(".mount_OtherX");
        let inside = mount.join("usr/bin/cantus");
        let img = Some(image.as_path());
        // Run from its own AppImage, through either variable.
        assert_eq!(
            own_appimage(&inside, img, &[None, Some(mount.as_path())]),
            Some(image.clone())
        );
        assert_eq!(
            own_appimage(&inside, img, &[Some(mount.as_path()), None]),
            Some(image.clone())
        );
        // A distribution's or /opt Cantus started from another AppImage's terminal.
        for outside in ["/usr/bin/cantus", "/opt/cantus/cantus"] {
            assert_eq!(
                own_appimage(Path::new(outside), img, &[None, Some(mount.as_path())]),
                None,
                "{outside}"
            );
            let kind = InstallKind::detect_on(
                "linux",
                Path::new(outside),
                own_appimage(Path::new(outside), img, &[None, Some(mount.as_path())]).as_deref(),
                false,
            );
            assert_eq!(kind, InstallKind::Package, "{outside}");
        }
        // No mounted folder named, or an empty one: not ours.
        assert_eq!(own_appimage(&inside, img, &[None, None]), None);
        assert_eq!(own_appimage(&inside, img, &[Some(Path::new(""))]), None);
        // A cargo build run from that terminal.
        let built = d.join(".mount_OtherX/target/debug/cantus");
        assert_eq!(own_appimage(&built, img, &[Some(mount.as_path())]), None);
        // A stale $APPIMAGE naming no file.
        let gone = d.join("gone");
        assert_eq!(
            own_appimage(&inside, Some(gone.as_path()), &[Some(mount.as_path())]),
            None
        );
    }

    #[test]
    fn an_appimage_is_replaced_whole_and_the_old_one_kept_until_the_new_one_starts() {
        let dir = tempfile::tempdir().unwrap();
        let d = dir.path();
        let current = d.join("Cantus.AppImage");
        std::fs::create_dir(d.join("updates")).unwrap();
        let new = d.join("updates").join("download.AppImage");
        std::fs::write(&current, b"old program").unwrap();
        std::fs::write(&new, b"new program").unwrap();
        replace_appimage(&current, &new).unwrap();
        assert_eq!(std::fs::read(&current).unwrap(), b"new program");
        // The download was moved, not copied: nothing is left in the updates folder.
        assert!(!new.exists());
        let old = previous_appimage(&current);
        assert_eq!(old, d.join("Cantus.AppImage.old"));
        assert_eq!(std::fs::read(&old).unwrap(), b"old program");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(&current).unwrap().permissions().mode();
            assert_eq!(mode & 0o111, 0o111, "the new AppImage is executable");
        }
        // Nothing staged is left behind.
        let names: Vec<String> = std::fs::read_dir(d)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        assert!(names.iter().all(|n| !n.contains(".new-")), "{names:?}");
        // A second update replaces the kept copy with the one being replaced now.
        std::fs::write(&new, b"newer program").unwrap();
        replace_appimage(&current, &new).unwrap();
        assert_eq!(std::fs::read(&old).unwrap(), b"new program");
        // The new one started: the old one goes.
        assert!(forget_previous_appimage(&current));
        assert!(!old.exists());
        assert!(!forget_previous_appimage(&current));
        assert_eq!(std::fs::read(&current).unwrap(), b"newer program");
    }

    #[test]
    fn a_folder_is_writable_or_not_and_the_probe_leaves_nothing() {
        let dir = tempfile::tempdir().unwrap();
        assert!(writable_dir(dir.path()));
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 0);
        assert!(!writable_dir(&dir.path().join("no such folder")));
    }

    #[test]
    fn a_failed_replacement_changes_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let d = dir.path();
        let current = d.join("Cantus.AppImage");
        std::fs::write(&current, b"old program").unwrap();
        // The download is missing: the copy fails before anything is touched.
        assert!(replace_appimage(&current, &d.join("missing")).is_err());
        assert_eq!(std::fs::read(&current).unwrap(), b"old program");
        assert!(!previous_appimage(&current).exists());
        assert_eq!(std::fs::read_dir(d).unwrap().count(), 1);
    }

    #[test]
    fn the_new_appimage_starts_after_this_cantus_has_closed() {
        let cmd = relaunch_after_exit(Path::new("/home/a/Cantus.AppImage"), 4242);
        assert_eq!(cmd.get_program(), "sh");
        let args: Vec<_> = cmd
            .get_args()
            .map(|a| a.to_string_lossy().into_owned())
            .collect();
        assert_eq!(args[0], "-c");
        assert!(args[1].starts_with("while kill -0 \"$1\""), "{}", args[1]);
        assert!(args[1].ends_with("exec \"$0\""), "{}", args[1]);
        assert_eq!(args[2..], ["/home/a/Cantus.AppImage", "4242"]);
    }

    /// The wait really waits: run where there is a POSIX shell (Linux, macOS).
    #[cfg(unix)]
    #[test]
    fn the_relaunch_waits_for_the_process_then_runs_the_program() {
        let dir = tempfile::tempdir().unwrap();
        let marker = dir.path().join("started");
        let program = dir.path().join("program.sh");
        std::fs::write(
            &program,
            format!(
                "#!/bin/sh
touch '{}'
",
                marker.display()
            ),
        )
        .unwrap();
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&program, std::fs::Permissions::from_mode(0o755)).unwrap();
        let mut sleeper = std::process::Command::new("sleep")
            .arg("0.6")
            .spawn()
            .unwrap();
        let mut waiter = relaunch_after_exit(&program, sleeper.id()).spawn().unwrap();
        std::thread::sleep(std::time::Duration::from_millis(250));
        assert!(!marker.exists(), "started while the old process lived");
        sleeper.wait().unwrap();
        assert!(waiter.wait().unwrap().success());
        assert!(marker.exists(), "started after it exited");
    }

    #[test]
    fn only_a_copy_that_updates_itself_applies_a_file() {
        let f = Path::new("Cantus-0.2.0-windows-x64-setup.exe");
        for k in [
            InstallKind::Portable,
            InstallKind::Package,
            InstallKind::Development,
        ] {
            let e = apply(k, f, false).unwrap_err();
            assert_eq!(e.kind(), std::io::ErrorKind::Unsupported, "{k:?}");
        }
    }

    #[test]
    fn an_update_installs_silently_keeping_the_first_choices() {
        assert_eq!(installer_args(false), ["/S", "/UPDATE"]);
        assert_eq!(installer_args(true), ["/S", "/UPDATE", "/RELAUNCH"]);
    }
}
