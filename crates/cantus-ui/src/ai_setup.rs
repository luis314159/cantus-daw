//! *Copy setup for…* (ADR 0081 §10, `research/ai/mcp-server.md` §6): the snippet each MCP
//! client wants, with the door's address and token — or, for clients that start a program,
//! the stdio bridge beside `cantus.exe`, which reads the token itself — and a Claude
//! Desktop extension (`.mcpb`: a zip of `manifest.json` and the bridge, MCPB manifest 0.3,
//! `server.type = "binary"`).

use serde_json::{json, Value};
use std::path::{Path, PathBuf};

/// The clients Settings ▸ AI has a button for.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Client {
    ClaudeCode,
    ClaudeDesktop,
    Cursor,
    VsCode,
    GeminiCli,
    LmStudio,
}

impl Client {
    pub const ALL: [Client; 6] = [
        Client::ClaudeCode,
        Client::ClaudeDesktop,
        Client::Cursor,
        Client::VsCode,
        Client::GeminiCli,
        Client::LmStudio,
    ];

    /// Its name (a product name: the same in every language).
    pub fn name(self) -> &'static str {
        match self {
            Client::ClaudeCode => "Claude Code",
            Client::ClaudeDesktop => "Claude Desktop",
            Client::Cursor => "Cursor",
            Client::VsCode => "VS Code",
            Client::GeminiCli => "Gemini CLI",
            Client::LmStudio => "LM Studio",
        }
    }

    /// Where the snippet goes, as a locale key.
    pub fn where_key(self) -> &'static str {
        match self {
            Client::ClaudeCode => "ai.setup.where.claude_code",
            Client::ClaudeDesktop => "ai.setup.where.claude_desktop",
            Client::Cursor => "ai.setup.where.cursor",
            Client::VsCode => "ai.setup.where.vscode",
            Client::GeminiCli => "ai.setup.where.gemini",
            Client::LmStudio => "ai.setup.where.lmstudio",
        }
    }
}

/// The bridge program beside this one (`cantus-mcp.exe` on Windows).
pub fn bridge_path() -> PathBuf {
    let name = if cfg!(windows) {
        "cantus-mcp.exe"
    } else {
        "cantus-mcp"
    };
    std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(Path::to_path_buf))
        .unwrap_or_default()
        .join(name)
}

/// How a client starts the bridge: a program and its arguments.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Launch {
    pub command: String,
    pub args: Vec<String>,
}

impl Launch {
    /// As one line, for the page's note.
    pub fn line(&self) -> String {
        std::iter::once(self.command.as_str())
            .chain(self.args.iter().map(String::as_str))
            .collect::<Vec<_>>()
            .join(" ")
    }
}

/// How this install's bridge is started (`env` reads the environment): an AppImage's by
/// the image itself (`Cantus.AppImage --mcp-bridge` — its mount point changes at every
/// launch, its own path does not), a Flatpak's through `flatpak run`, any other the
/// program beside this one.
pub fn bridge_launch(bridge: &Path, env: impl Fn(&str) -> Option<String>) -> Launch {
    if let Some(image) = env("APPIMAGE").filter(|p| !p.is_empty()) {
        return Launch {
            command: image,
            args: vec!["--mcp-bridge".into()],
        };
    }
    if let Some(id) = env("FLATPAK_ID").filter(|p| !p.is_empty()) {
        return Launch {
            command: "flatpak".into(),
            args: vec!["run".into(), "--command=cantus-mcp".into(), id],
        };
    }
    Launch {
        command: bridge.display().to_string(),
        args: Vec::new(),
    }
}

/// [`bridge_launch`] for this process.
pub fn this_launch() -> Launch {
    bridge_launch(&bridge_path(), |k| std::env::var(k).ok())
}

/// The door's URL.
pub fn url(port: u16) -> String {
    format!("http://127.0.0.1:{port}/mcp")
}

fn pretty(v: &Value) -> String {
    serde_json::to_string_pretty(v).expect("JSON")
}

/// What to paste for `client`.
pub fn snippet(client: Client, port: u16, token: &str, bridge: &Launch) -> String {
    let url = url(port);
    let auth = format!("Bearer {token}");
    match client {
        Client::ClaudeCode => format!(
            "claude mcp add --transport http cantus {url} --header \"Authorization: {auth}\""
        ),
        Client::ClaudeDesktop => pretty(&json!({
            "mcpServers": {"cantus": {"command": bridge.command, "args": bridge.args}}
        })),
        Client::Cursor | Client::LmStudio => pretty(&json!({
            "mcpServers": {"cantus": {"url": url, "headers": {"Authorization": auth}}}
        })),
        Client::VsCode => pretty(&json!({
            "servers": {"cantus": {"type": "http", "url": url, "headers": {"Authorization": auth}}}
        })),
        Client::GeminiCli => pretty(&json!({
            "mcpServers": {"cantus": {"httpUrl": url, "headers": {"Authorization": auth}}}
        })),
    }
}

/// The `.mcpb` manifest for the bridge as `server/<bridge name>`; `home` is the settings
/// folder the bridge must read (given explicitly, so a portable install is found too).
pub fn mcpb_manifest(version: &str, bridge_name: &str, home: &Path) -> Value {
    let platform = if cfg!(windows) {
        "win32"
    } else if cfg!(target_os = "macos") {
        "darwin"
    } else {
        "linux"
    };
    json!({
        "manifest_version": "0.3",
        "name": "cantus",
        "display_name": "Cantus",
        "version": version,
        "description": "Drive the Cantus DAW: tracks, notes, the mixer, effects, automation — every change one undo step you can see and undo in Cantus.",
        "author": {"name": "Luis Ángel Almazán", "url": "https://lalmazan.com"},
        "homepage": "https://github.com/luis314159/cantus-daw",
        "license": "GPL-3.0-or-later",
        "server": {
            "type": "binary",
            "entry_point": format!("server/{bridge_name}"),
            "mcp_config": {
                "command": format!("${{__dirname}}/server/{bridge_name}"),
                "args": [],
                "env": {"CANTUS_DATA_DIR": home.display().to_string()}
            }
        },
        "tools_generated": true,
        "compatibility": {"platforms": [platform]}
    })
}

/// A Claude Desktop extension: `manifest.json` and the bridge, in a zip.
pub fn mcpb(version: &str, bridge: &Path, home: &Path) -> std::io::Result<Vec<u8>> {
    let program = std::fs::read(bridge)?;
    let name = bridge
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "cantus-mcp".into());
    let manifest = pretty(&mcpb_manifest(version, &name, home));
    Ok(zip_store(&[
        ("manifest.json", manifest.as_bytes(), false),
        (&format!("server/{name}"), &program, true),
    ]))
}

/// CRC-32 (IEEE 802.3), as zip uses.
pub fn crc32(data: &[u8]) -> u32 {
    let mut crc = 0xFFFF_FFFFu32;
    for &b in data {
        crc ^= u32::from(b);
        for _ in 0..8 {
            let mask = (crc & 1).wrapping_neg();
            crc = (crc >> 1) ^ (0xEDB8_8320 & mask);
        }
    }
    !crc
}

/// A zip of stored (uncompressed) entries: `(name, bytes, executable)`.
pub fn zip_store(entries: &[(&str, &[u8], bool)]) -> Vec<u8> {
    let mut out = Vec::new();
    let mut central = Vec::new();
    let le16 = |v: &mut Vec<u8>, x: u16| v.extend_from_slice(&x.to_le_bytes());
    let le32 = |v: &mut Vec<u8>, x: u32| v.extend_from_slice(&x.to_le_bytes());
    for (name, data, exec) in entries {
        let offset = out.len() as u32;
        let crc = crc32(data);
        let size = data.len() as u32;
        // Local header.
        le32(&mut out, 0x0403_4b50);
        le16(&mut out, 20);
        le16(&mut out, 0x0800); // UTF-8 names
        le16(&mut out, 0); // stored
        le16(&mut out, 0);
        le16(&mut out, 0x21); // 1980-01-01
        le32(&mut out, crc);
        le32(&mut out, size);
        le32(&mut out, size);
        le16(&mut out, name.len() as u16);
        le16(&mut out, 0);
        out.extend_from_slice(name.as_bytes());
        out.extend_from_slice(data);
        // Central directory entry (made by Unix, so the mode is kept).
        le32(&mut central, 0x0201_4b50);
        le16(&mut central, (3 << 8) | 20);
        le16(&mut central, 20);
        le16(&mut central, 0x0800);
        le16(&mut central, 0);
        le16(&mut central, 0);
        le16(&mut central, 0x21);
        le32(&mut central, crc);
        le32(&mut central, size);
        le32(&mut central, size);
        le16(&mut central, name.len() as u16);
        le16(&mut central, 0);
        le16(&mut central, 0);
        le16(&mut central, 0);
        le16(&mut central, 0);
        let mode: u32 = if *exec { 0o100755 } else { 0o100644 };
        le32(&mut central, mode << 16);
        le32(&mut central, offset);
        central.extend_from_slice(name.as_bytes());
    }
    let start = out.len() as u32;
    let len = central.len() as u32;
    out.extend_from_slice(&central);
    le32(&mut out, 0x0605_4b50);
    le16(&mut out, 0);
    le16(&mut out, 0);
    le16(&mut out, entries.len() as u16);
    le16(&mut out, entries.len() as u16);
    le32(&mut out, len);
    le32(&mut out, start);
    le16(&mut out, 0);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_client_gets_a_snippet_it_can_read() {
        let path = Path::new("C:/Program Files/Cantus/cantus-mcp.exe");
        let bridge = bridge_launch(path, |_| None);
        for c in Client::ALL {
            let s = snippet(c, 39571, "abc", &bridge);
            match c {
                Client::ClaudeCode => {
                    assert!(s.starts_with(
                        "claude mcp add --transport http cantus http://127.0.0.1:39571/mcp"
                    ));
                    assert!(s.contains("Authorization: Bearer abc"));
                }
                Client::ClaudeDesktop => {
                    let v: Value = serde_json::from_str(&s).unwrap();
                    assert_eq!(
                        v["mcpServers"]["cantus"]["command"],
                        path.display().to_string()
                    );
                    assert_eq!(v["mcpServers"]["cantus"]["args"], json!([]));
                    assert!(!s.contains("abc"), "the bridge reads the token itself");
                }
                Client::VsCode => {
                    let v: Value = serde_json::from_str(&s).unwrap();
                    assert_eq!(v["servers"]["cantus"]["type"], "http");
                    assert_eq!(
                        v["servers"]["cantus"]["headers"]["Authorization"],
                        "Bearer abc"
                    );
                }
                Client::GeminiCli => {
                    let v: Value = serde_json::from_str(&s).unwrap();
                    assert_eq!(
                        v["mcpServers"]["cantus"]["httpUrl"],
                        "http://127.0.0.1:39571/mcp"
                    );
                }
                Client::Cursor | Client::LmStudio => {
                    let v: Value = serde_json::from_str(&s).unwrap();
                    assert_eq!(
                        v["mcpServers"]["cantus"]["url"],
                        "http://127.0.0.1:39571/mcp"
                    );
                }
            }
        }
    }

    /// An AppImage's bridge is the image itself with `--mcp-bridge` (its mount point moves
    /// at every launch); a Flatpak's goes through `flatpak run`.
    #[test]
    fn a_packaged_bridge_is_started_the_way_its_package_allows() {
        let path = Path::new("/tmp/.mount_CantusX/usr/bin/cantus-mcp");
        let image = |k: &str| (k == "APPIMAGE").then(|| "/home/u/Apps/Cantus.AppImage".into());
        let l = bridge_launch(path, image);
        assert_eq!(l.command, "/home/u/Apps/Cantus.AppImage");
        assert_eq!(l.args, vec!["--mcp-bridge"]);
        let flat = |k: &str| (k == "FLATPAK_ID").then(|| "com.cantusdaw.Cantus".into());
        let l = bridge_launch(path, flat);
        assert_eq!(
            l.line(),
            "flatpak run --command=cantus-mcp com.cantusdaw.Cantus"
        );
        let s = snippet(Client::ClaudeDesktop, 1, "t", &l);
        let v: Value = serde_json::from_str(&s).unwrap();
        assert_eq!(v["mcpServers"]["cantus"]["command"], "flatpak");
        assert_eq!(v["mcpServers"]["cantus"]["args"][1], "--command=cantus-mcp");
        assert_eq!(
            bridge_launch(path, |_| None).line(),
            path.display().to_string()
        );
    }

    #[test]
    fn the_extension_is_a_zip_with_its_manifest_and_the_bridge() {
        assert_eq!(crc32(b"123456789"), 0xCBF4_3926, "the standard check value");
        let dir = std::env::temp_dir().join(format!("cantus-mcpb-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let bridge = dir.join("cantus-mcp.exe");
        std::fs::write(&bridge, b"MZ fake program").unwrap();
        let z = mcpb("0.1.0-beta.9", &bridge, Path::new("/home/x")).unwrap();
        assert_eq!(&z[..4], b"PK\x03\x04");
        let text = String::from_utf8_lossy(&z);
        assert!(text.contains("manifest.json") && text.contains("server/cantus-mcp.exe"));
        let m = mcpb_manifest("0.1.0-beta.9", "cantus-mcp.exe", Path::new("/home/x"));
        assert_eq!(m["server"]["type"], "binary");
        assert_eq!(m["version"], "0.1.0-beta.9", "semver, pre-release and all");
        assert_eq!(
            m["server"]["mcp_config"]["command"],
            "${__dirname}/server/cantus-mcp.exe"
        );
        assert_eq!(
            m["server"]["mcp_config"]["env"]["CANTUS_DATA_DIR"],
            "/home/x"
        );
        // The end record names two entries and points at the central directory.
        let end = z.len() - 22;
        assert_eq!(&z[end..end + 4], &0x0605_4b50u32.to_le_bytes());
        assert_eq!(u16::from_le_bytes([z[end + 10], z[end + 11]]), 2);
        let cd = u32::from_le_bytes([z[end + 16], z[end + 17], z[end + 18], z[end + 19]]) as usize;
        assert_eq!(&z[cd..cd + 4], &0x0201_4b50u32.to_le_bytes());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
