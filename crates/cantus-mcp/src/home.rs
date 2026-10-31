//! Where the HTTP door says it is open, and its token (ADR 0081 §9): small files in the
//! `mcp` folder of the user's own settings folder — the same folder `cantus_ui::app::config_dir()` names (a test
//! there holds the two equal), so the stdio bridge beside `cantus.exe` finds what the
//! running app wrote without depending on the application.
//!
//! - `mcp-token`: the bearer token, made once per install, regenerable; readable by the
//!   user only (mode 0600 on Linux and macOS; on Windows the folder is the user's own
//!   `%LOCALAPPDATA%\Cantus`).
//! - `mcp-door.json`: `{port, pid, version}` while the door is open; removed when it closes.

use serde_json::{json, Value};
use std::ffi::OsString;
use std::io;
use std::path::{Path, PathBuf};

/// The door's port when the user has chosen none: fixed and documented (the manual, the
/// setup snippets).
pub const DEFAULT_PORT: u16 = 39_571;
pub const TOKEN_FILE: &str = "mcp-token";
pub const DOOR_FILE: &str = "mcp-door.json";
/// The marker and folder of a portable install (`cantus_update::install`'s, kept equal by a
/// test in `cantus-ui`).
pub const PORTABLE_MARKER: &str = "portable.txt";
pub const PORTABLE_DATA: &str = "data";

fn env(var: &str) -> Option<OsString> {
    std::env::var_os(var).filter(|v| !v.is_empty())
}

/// The settings folder, as the application computes it, seen from an executable in
/// `exe_dir` (the bridge sits beside `cantus.exe`).
pub fn dir_for(exe_dir: Option<&Path>, env: &dyn Fn(&str) -> Option<OsString>) -> PathBuf {
    let portable = exe_dir
        .filter(|d| d.join(PORTABLE_MARKER).is_file())
        .map(|d| d.join(PORTABLE_DATA));
    let overridden = env("CANTUS_DATA_DIR").map(PathBuf::from);
    if let Some(d) = overridden.or(portable) {
        return d;
    }
    #[cfg(target_os = "linux")]
    {
        let base = env("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .filter(|p| p.is_absolute())
            .unwrap_or_else(|| {
                env("HOME")
                    .map(PathBuf::from)
                    .unwrap_or_else(std::env::temp_dir)
                    .join(".config")
            });
        base.join("cantus")
    }
    #[cfg(not(target_os = "linux"))]
    {
        #[cfg(target_os = "macos")]
        if let Some(home) = env("HOME") {
            return PathBuf::from(home).join("Library/Application Support/Cantus");
        }
        env("LOCALAPPDATA")
            .or_else(|| env("XDG_DATA_HOME"))
            .map(PathBuf::from)
            .or_else(|| env("HOME").map(|h| PathBuf::from(h).join(".local/share")))
            .unwrap_or_else(std::env::temp_dir)
            .join("Cantus")
    }
}

/// The settings folder for this process.
pub fn dir() -> PathBuf {
    let exe = std::env::current_exe().ok();
    dir_for(exe.as_deref().and_then(Path::parent), &env)
}

/// The folder inside the settings folder that holds the door's token, its clients' tokens
/// and its file — made private, and only it: the settings folder itself is left as the
/// application keeps it (review 5 #2: an elevated run, a portable drive moved between PCs).
pub const SECRETS_DIR: &str = "mcp";

/// [`SECRETS_DIR`] in the settings folder `dir`.
pub fn secrets(dir: &Path) -> PathBuf {
    dir.join(SECRETS_DIR)
}

/// The settings folder, made if needed, and private enough for a secret: this user's,
/// tightened to owner-only, nobody else able to swap it (review 3 #8, review 4 #1, #4).
pub fn private_folder(dir: &Path) -> io::Result<()> {
    cantus_rt::private::secure_dir(dir)
        .map_err(|why| io::Error::new(io::ErrorKind::PermissionDenied, why))
}

fn not_mine(path: &Path) -> io::Error {
    io::Error::new(
        io::ErrorKind::PermissionDenied,
        format!("{} is not this user's", path.display()),
    )
}

/// Whether `path` exists and is not this user's (a planted file).
fn foreign(path: &Path) -> io::Result<bool> {
    if std::fs::symlink_metadata(path).is_err() {
        return Ok(false);
    }
    Ok(!cantus_rt::private::owned_by_me(path)?)
}

/// Write `contents` readable by the user only, atomically: a temporary created owner-only
/// from its first instant (never one another user left there), filled, renamed over a file
/// that is this user's (review 4 #4).
pub fn write_private(path: &Path, contents: &[u8]) -> io::Result<()> {
    use std::io::Write;
    if let Some(parent) = path.parent() {
        private_folder(parent)?;
    }
    if foreign(path)? {
        return Err(not_mine(path));
    }
    let tmp = path.with_extension("tmp");
    if std::fs::symlink_metadata(&tmp).is_ok() {
        if foreign(&tmp)? {
            return Err(not_mine(&tmp));
        }
        std::fs::remove_file(&tmp)?;
    }
    {
        let mut f = cantus_rt::private::create_secret(&tmp)?;
        f.write_all(contents)?;
        f.sync_all()?;
    }
    std::fs::rename(&tmp, path)
}

/// A new token: 256 bits from the operating system's random source, hex.
pub fn new_token() -> String {
    crate::ids::random_hex(32)
}

/// The install's token, made the first time.
pub fn token(dir: &Path) -> io::Result<String> {
    private_folder(&secrets(dir))?;
    let path = secrets(dir).join(TOKEN_FILE);
    if foreign(&path)? {
        return Err(not_mine(&path));
    }
    if let Ok(t) = std::fs::read_to_string(&path) {
        let t = t.trim();
        if t.len() >= 32 && t.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Ok(t.to_string());
        }
    }
    regenerate_token(dir)
}

/// The install's token if one is there and this user's (the bridge: it never makes one).
pub fn token_if_any(dir: &Path) -> Option<String> {
    let path = secrets(dir).join(TOKEN_FILE);
    if foreign(&path).ok()? {
        return None;
    }
    let t = std::fs::read_to_string(&path).ok()?;
    let t = t.trim();
    (t.len() >= 32 && t.bytes().all(|b| b.is_ascii_hexdigit())).then(|| t.to_string())
}

/// A new token in place of the old, and every client's own made again: every client must
/// be set up again.
pub fn regenerate_token(dir: &Path) -> io::Result<String> {
    let t = new_token();
    write_private(&secrets(dir).join(TOKEN_FILE), t.as_bytes())?;
    let _ = std::fs::remove_file(secrets(dir).join(CLIENT_TOKENS_FILE));
    Ok(t)
}

/// Each client's own token (*Copy setup for…*), so each is an agent of its own to Cantus
/// (review 2 #11): `{"Cursor": "<hex>", …}`, readable by the user only.
pub const CLIENT_TOKENS_FILE: &str = "mcp-clients.json";

fn client_tokens(dir: &Path) -> serde_json::Map<String, Value> {
    let path = secrets(dir).join(CLIENT_TOKENS_FILE);
    if foreign(&path).unwrap_or(true) && std::fs::symlink_metadata(&path).is_ok() {
        return serde_json::Map::new();
    }
    std::fs::read_to_string(path)
        .ok()
        .and_then(|s| serde_json::from_str::<Value>(&s).ok())
        .and_then(|v| v.as_object().cloned())
        .unwrap_or_default()
}

/// `client`'s own token, made the first time.
pub fn client_token(dir: &Path, client: &str) -> io::Result<String> {
    let mut all = client_tokens(dir);
    if let Some(t) = all.get(client).and_then(Value::as_str) {
        return Ok(t.to_string());
    }
    let t = new_token();
    all.insert(client.to_string(), json!(t));
    write_private(
        &secrets(dir).join(CLIENT_TOKENS_FILE),
        Value::Object(all).to_string().as_bytes(),
    )?;
    Ok(t)
}

/// Every token the door accepts, with the name its agents go by: `default` (the bridge's,
/// a hand-made setup) and each client's.
pub fn tokens(dir: &Path) -> io::Result<Vec<(String, String)>> {
    let mut out = vec![("default".to_string(), token(dir)?)];
    for (name, t) in client_tokens(dir) {
        if let Some(t) = t.as_str().filter(|t| t.len() >= 32) {
            let clean: String = name
                .chars()
                .filter(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == ' ')
                .take(32)
                .collect();
            out.push((clean.replace(' ', "-").to_ascii_lowercase(), t.to_string()));
        }
    }
    Ok(out)
}

/// What `mcp-door.json` says.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DoorFile {
    pub port: u16,
    pub pid: u32,
    pub version: String,
}

pub fn write_door(dir: &Path, door: &DoorFile) -> io::Result<()> {
    let v = json!({"port": door.port, "pid": door.pid, "version": door.version});
    write_private(&secrets(dir).join(DOOR_FILE), v.to_string().as_bytes())
}

pub fn read_door(dir: &Path) -> Option<DoorFile> {
    let v: Value =
        serde_json::from_str(&std::fs::read_to_string(secrets(dir).join(DOOR_FILE)).ok()?).ok()?;
    Some(DoorFile {
        port: u16::try_from(v["port"].as_u64()?).ok()?,
        pid: u32::try_from(v["pid"].as_u64().unwrap_or(0)).ok()?,
        version: v["version"].as_str().unwrap_or("").to_string(),
    })
}

/// The door closed: its file goes (only if it is still this process's).
pub fn remove_door(dir: &Path) {
    if read_door(dir).is_some_and(|d| d.pid == std::process::id()) {
        let _ = std::fs::remove_file(secrets(dir).join(DOOR_FILE));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_token_is_made_once_and_regenerated_on_demand() {
        let d = std::env::temp_dir().join(format!("cantus-mcp-home-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        let a = token(&d).unwrap();
        assert_eq!(a.len(), 64);
        assert_eq!(token(&d).unwrap(), a, "kept");
        let b = regenerate_token(&d).unwrap();
        assert_ne!(a, b);
        assert_eq!(token(&d).unwrap(), b);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(secrets(&d).join(TOKEN_FILE))
                .unwrap()
                .permissions()
                .mode();
            assert_eq!(mode & 0o077, 0, "readable by the user only");
        }
        // Review 4 #2, #4, review 5 #2: on Windows the `mcp` folder and the token carry a
        // protected DACL giving this user (by SID) full control, read back from the
        // system; the settings folder itself is left as it was (inherited, not protected).
        #[cfg(windows)]
        {
            let sddl = cantus_rt::private::dacl_sddl;
            let folder = sddl(&secrets(&d)).unwrap();
            assert!(folder.starts_with("D:P(A;OICI;FA;;;S-1-5-"), "{folder}");
            let file = sddl(&secrets(&d).join(TOKEN_FILE)).unwrap();
            assert!(file.starts_with("D:P(A;;FA;;;S-1-5-"), "{file}");
            assert!(
                !sddl(&d).unwrap().starts_with("D:P"),
                "the settings folder untouched"
            );
        }
        assert_eq!(token_if_any(&d), Some(b.clone()));
        let door = DoorFile {
            port: 1234,
            pid: std::process::id(),
            version: "x".into(),
        };
        write_door(&d, &door).unwrap();
        assert_eq!(read_door(&d), Some(door));
        remove_door(&d);
        assert_eq!(read_door(&d), None);
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn a_portable_install_and_the_override_are_honoured() {
        let d = std::env::temp_dir().join(format!("cantus-mcp-portable-{}", std::process::id()));
        std::fs::create_dir_all(&d).unwrap();
        std::fs::write(d.join(PORTABLE_MARKER), "x").unwrap();
        let none = |_: &str| None;
        assert_eq!(dir_for(Some(&d), &none), d.join(PORTABLE_DATA));
        let over = |v: &str| (v == "CANTUS_DATA_DIR").then(|| OsString::from("/over"));
        assert_eq!(dir_for(Some(&d), &over), PathBuf::from("/over"));
        let _ = std::fs::remove_dir_all(&d);
    }

    /// Review 3 #8, review 4 #1: our own folder left open (a umask of 002, or by hand) is
    /// tightened before a secret goes in; one another user could swap (its parent open to
    /// all, not sticky) keeps none.
    #[cfg(unix)]
    #[test]
    fn a_folder_others_may_write_keeps_no_token() {
        use std::os::unix::fs::PermissionsExt;
        let base = std::env::temp_dir().join(format!("cantus-mcp-open-{}", std::process::id()));
        let d = base.join("cantus");
        std::fs::create_dir_all(secrets(&d)).unwrap();
        std::fs::set_permissions(secrets(&d), std::fs::Permissions::from_mode(0o775)).unwrap();
        assert!(token(&d).is_ok(), "tightened, not refused");
        let mode = std::fs::metadata(secrets(&d)).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o700);
        std::fs::remove_file(secrets(&d).join(TOKEN_FILE)).unwrap();
        // The settings folder (the `mcp` folder's parent) open to all, not sticky.
        std::fs::set_permissions(&d, std::fs::Permissions::from_mode(0o777)).unwrap();
        assert!(token(&d).is_err());
        assert!(!secrets(&d).join(TOKEN_FILE).exists());
        assert_eq!(token_if_any(&d), None);
        std::fs::set_permissions(&d, std::fs::Permissions::from_mode(0o700)).unwrap();
        let _ = std::fs::remove_dir_all(&base);
    }
}
