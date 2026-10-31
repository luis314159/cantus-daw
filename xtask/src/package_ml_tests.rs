//! r34:stems — the ML helper and the Visual C++ runtime in the release (ADR 0085, review 1
//! #1): staged beside the program, the helper signed and checked like every program, the
//! runtime's DLLs left as Microsoft signed them, and none of it in the plug-ins' zip.

use crate::archive::Entry;
use crate::package::{self, Inputs};
use cantus_launch::Level;
use std::path::Path;

fn write(p: &Path, bytes: &[u8]) {
    std::fs::create_dir_all(p.parent().unwrap()).unwrap();
    std::fs::write(p, bytes).unwrap();
}

fn inputs(root: &Path) -> Inputs {
    for f in [
        "l.exe",
        "c.exe",
        "s.exe",
        "x.exe",
        "ml.exe",
        "b.exe",
        "LICENSE-GPL",
        "LICENSE",
        "README.txt",
    ] {
        write(&root.join(f), b"MZ");
    }
    for d in package::CRT {
        write(&root.join("crt").join(d), b"MZ dll");
    }
    let n = "Cantus Aequalis";
    write(&root.join("bundled").join(format!("{n}.clap")), b"MZ clap");
    write(
        &root
            .join("bundled")
            .join(format!("{n}.vst3/Contents/x86_64-win/{n}.vst3")),
        b"MZ vst3",
    );
    Inputs {
        launcher: root.join("l.exe"),
        programs: vec![(Level::V2, root.join("c.exe"))],
        scanner: root.join("s.exe"),
        sandbox: root.join("x.exe"),
        bridge: root.join("b.exe"), // r34:mcp — the MCP bridge (integration)
        beside: std::iter::once((package::ML_HELPER.to_string(), root.join("ml.exe")))
            .chain(
                package::CRT
                    .iter()
                    .map(|d| (format!("{}/{d}", package::ML_DIR), root.join("crt").join(d))),
            )
            .collect(),
        bundled: root.join("bundled"),
        plugins: vec![n.to_string()],
        license: root.join("LICENSE-GPL"),
        copyright: root.join("LICENSE"),
        readme: root.join("README.txt"),
        docs: vec![],
        check_metadata: false,
    }
}

#[test]
fn the_helper_and_the_runtime_sit_beside_the_program() {
    let dir = tempfile::tempdir().unwrap();
    let staged = package::stage(&inputs(dir.path()), &dir.path().join("stage"), "0.2.0");
    let names: Vec<&str> = staged.app.iter().map(|e: &Entry| e.name.as_str()).collect();
    assert!(names.contains(&"ml/cantus-ml.exe"));
    // The runtime beside the helper, in its folder, and nowhere else: not in the install
    // folder, where every program and the plug-ins they load would find it (integration
    // review #1).
    for d in package::CRT {
        assert!(names.contains(&format!("ml/{d}").as_str()), "{d}");
        assert!(dir.path().join("stage/app/ml").join(d).is_file());
        assert!(
            !dir.path().join("stage/app").join(d).exists(),
            "{d} at the top"
        );
    }
    assert!(!names.iter().any(|n| {
        let l = n.to_ascii_lowercase();
        (l.ends_with(".dll") || l.ends_with("cantus-ml.exe")) && !l.starts_with("ml/")
    }));
    // The installer writes them there, waits for the helper, and removes the folder.
    let nsh = crate::nsis::files_nsh_for(&staged, true);
    assert!(nsh.contains(r#"SetOutPath "$INSTDIR\ml""#), "{nsh}");
    assert!(nsh.contains(r#"Delete "$INSTDIR\ml\vcruntime140.dll""#));
    assert!(nsh.contains(r#"CANTUS_PROGRAM_BUSY "$INSTDIR\ml\cantus-ml.exe""#));
    assert!(nsh.contains(r#"RMDir "$INSTDIR\ml""#));
    let signed = staged.binaries();
    assert!(signed.iter().any(|p| p.ends_with("cantus-ml.exe")));
    assert!(!signed
        .iter()
        .any(|p| p.extension().is_some_and(|e| e == "dll")));
    let plugins = package::plugin_entries(&staged);
    assert!(!plugins
        .iter()
        .any(|e| e.name.ends_with(".dll") || e.name == "cantus-ml.exe"));
}

/// The build machine's runtime is found and is new enough (Windows with Visual Studio's
/// build tools, as every machine that makes a release has).
#[cfg(windows)]
#[test]
fn the_build_machines_runtime_is_found() {
    if std::env::var_os("CANTUS_VC_REDIST").is_none()
        && !Path::new(r"C:\Program Files (x86)\Microsoft Visual Studio").exists()
        && !Path::new(r"C:\Program Files\Microsoft Visual Studio").exists()
    {
        return;
    }
    let found = package::find_crt().unwrap();
    assert_eq!(found.len(), 4);
    assert!(found.iter().all(|(_, p)| p.is_file()));
}

/// Linux and macOS ship the helper too: the AppImage, the Flatpak, the disk image (signed
/// with the entitlements that let it load Microsoft's library) and the release's build.
#[test]
fn every_system_packages_the_helper() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    let read = |p: &str| std::fs::read_to_string(root.join(p)).unwrap();
    let appimage = read("packaging/linux/build-appimage.sh");
    assert!(appimage.contains("\"$release/cantus-ml\""));
    assert!(read("packaging/linux/flatpak/com.cantusdaw.Cantus.yml").contains("/app/bin/cantus-ml"));
    let dmg = read("packaging/macos/build-dmg.sh");
    // The bundle's binaries list holds the helper (beside the MCP bridge, in any order).
    let bins = dmg.lines().find(|l| l.starts_with("bins=(")).unwrap();
    assert!(bins.split(['(', ')', ' ']).any(|b| b == "cantus-ml"));
    assert!(dmg.contains("\"$app/Contents/MacOS/cantus-ml\""));
    assert!(read("packaging/macos/Cantus.entitlements").contains("disable-library-validation"));
    assert!(read(".github/workflows/release.yml").contains("-p cantus-ml"));
}

/// A DLL's bytes with a `VS_FIXEDFILEINFO` of `major.minor.build.0`.
fn versioned(major: u16, minor: u16, build: u16) -> Vec<u8> {
    let mut b = b"MZ padding".to_vec();
    b.extend(0xFEEF_04BDu32.to_le_bytes());
    b.extend(0x0001_0000u32.to_le_bytes());
    b.extend(((major as u32) << 16 | minor as u32).to_le_bytes());
    b.extend(((build as u32) << 16).to_le_bytes());
    b.extend([0u8; 32]);
    b
}

/// The runtime's version is read from the DLL itself, wherever the folder came from
/// (`CANTUS_VC_REDIST` included): an older one, or one without a version, is refused
/// (review 2 #16).
#[test]
fn the_runtimes_own_version_is_checked() {
    let dir = tempfile::tempdir().unwrap();
    for d in &package::CRT[1..] {
        write(&dir.path().join(d), b"MZ dll");
    }
    let vc = dir.path().join(package::CRT[0]);
    write(&vc, &versioned(14, 44, 35211));
    assert_eq!(package::dll_version(&vc), Some((14, 44, 35211, 0)));
    assert_eq!(package::crt_files(dir.path()).unwrap().len(), 4);
    write(&vc, &versioned(14, 38, 33135));
    let e = package::crt_files(dir.path()).unwrap_err();
    assert!(
        e.contains("14.38.33135") && e.contains("older than 14.40"),
        "{e}"
    );
    write(&vc, b"MZ no version");
    assert!(package::crt_files(dir.path()).is_err());
}

/// The notice says the runtime stays under Microsoft's terms, not the GPL, and links them.
#[test]
fn the_runtimes_notice_keeps_microsofts_terms() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let head = std::fs::read_to_string(root.join("notices/head.md")).unwrap();
    let at = head.find("## The Microsoft Visual C++ runtime").unwrap();
    let section = &head[at..at
        + head[at..]
            .find(
                "
## ",
            )
            .unwrap_or(head.len() - at)];
    assert!(section.contains("not under the GPL"), "{section}");
    assert!(section.contains("https://visualstudio.microsoft.com/license-terms/"));
}

/// Every crate is named in the licence summary under the licence its manifest declares
/// (`cantus-ml` and `cantus-sandbox` were missing — integration review #5).
#[test]
fn every_crate_is_in_the_licence_summary() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    let licence = std::fs::read_to_string(root.join("LICENSE")).unwrap();
    let split = licence.find("MPL-2.0").unwrap();
    let gpl_end = licence.find("GPL-3.0-or-later").unwrap();
    let (mpl, gpl) = (&licence[..split], &licence[split..gpl_end]);
    for dir in std::fs::read_dir(root.join("crates")).unwrap().flatten() {
        let Ok(manifest) = std::fs::read_to_string(dir.path().join("Cargo.toml")) else {
            continue;
        };
        let field = |k: &str| {
            manifest
                .lines()
                .find(|l| l.starts_with(&format!("{k} =")))
                .and_then(|l| l.split('"').nth(1))
                .map(str::to_owned)
        };
        // By its folder, as the summary names them (`cantus-app` is the package `cantus`).
        let name = dir.file_name().to_string_lossy().into_owned();
        let Some(lic) = field("license") else {
            continue;
        };
        let listed = |block: &str| {
            block
                .split(|c: char| !(c.is_alphanumeric() || c == '-'))
                .any(|w| w == name)
        };
        match lic.as_str() {
            "MPL-2.0" => assert!(listed(mpl), "{name} not under MPL-2.0 in LICENSE"),
            "GPL-3.0-or-later" => assert!(listed(gpl), "{name} not under the GPL in LICENSE"),
            other => panic!("{name}: {other}"),
        }
    }
    // (The README's table is a document CI does not run for — ADR 0048 §2 — so no test
    // reads it; its `cantus-ml` row is checked by eye.)
}
