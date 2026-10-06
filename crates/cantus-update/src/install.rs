//! How this copy of Cantus was installed, where a portable copy keeps its files, and
//! handing over to the installer when the app quits.

use std::path::{Path, PathBuf};

/// Beside `cantus.exe` in the portable zip: its presence makes the copy portable.
pub const PORTABLE_MARKER: &str = "portable.txt";
/// The installer writes this beside `cantus.exe`.
pub const UNINSTALLER: &str = "uninstall.exe";
/// A portable copy keeps settings, presets and autosaves here, beside the executable.
pub const PORTABLE_DATA: &str = "data";

/// What the marker says, for whoever opens it.
pub const PORTABLE_MARKER_TEXT: &str = "This copy of Cantus is portable: it keeps its settings, presets and \
autosaves in the \"data\" folder beside cantus.exe, and leaves nothing in the user's profile.\r\n\
Delete this file to make it use the profile like an installed Cantus.\r\n\
\r\n\
Esta copia de Cantus es portatil: guarda sus ajustes, presets y copias automaticas en la \
carpeta \"data\" junto a cantus.exe, y no deja nada en el perfil del usuario.\r\n\
Borra este archivo para que use el perfil como un Cantus instalado.\r\n";

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum InstallKind {
    /// Installed by the setup program: updates install themselves.
    Installed,
    /// Unzipped from the portable zip: updates are offered, the user unzips them.
    Portable,
    /// Built from source (`cargo run`): no automatic checks.
    Development,
}

impl InstallKind {
    pub fn detect(exe_dir: &Path) -> InstallKind {
        if exe_dir.join(PORTABLE_MARKER).is_file() {
            InstallKind::Portable
        } else if exe_dir.join(UNINSTALLER).is_file() {
            InstallKind::Installed
        } else {
            InstallKind::Development
        }
    }

    /// This process's own kind.
    pub fn current() -> InstallKind {
        exe_dir().map_or(InstallKind::Development, |d| InstallKind::detect(&d))
    }
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
    fn an_update_installs_silently_keeping_the_first_choices() {
        assert_eq!(installer_args(false), ["/S", "/UPDATE"]);
        assert_eq!(installer_args(true), ["/S", "/UPDATE", "/RELAUNCH"]);
    }
}
