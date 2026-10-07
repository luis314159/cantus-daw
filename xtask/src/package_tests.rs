//! The package task, run on stand-in inputs: the release's file list, what each zip holds,
//! the sums, and the installer script's install/uninstall symmetry. With NSIS on the
//! machine, a real installer is compiled from the same stand-ins.

use crate::archive;
use crate::nsis;
use crate::package::{self, Inputs, InstallerJob};
use cantus_update::release;
use std::path::Path;

const V: &str = "0.2.0";

fn write(p: &Path, bytes: &[u8]) {
    std::fs::create_dir_all(p.parent().unwrap()).unwrap();
    std::fs::write(p, bytes).unwrap();
}

/// Two plug-ins laid out as `xtask bundle` lays them out, the program, the texts.
fn stand_ins(root: &Path) -> Inputs {
    let bundled = root.join("bundled");
    let names = ["Cantus Aequalis", "Cantus Porta"];
    for n in names {
        write(&bundled.join(format!("{n}.clap")), b"MZ clap");
        write(
            &bundled.join(format!("{n}.vst3/Contents/x86_64-win/{n}.vst3")),
            b"MZ vst3",
        );
        write(
            &bundled.join(format!("{n}.vst3/Contents/Resources/moduleinfo.json")),
            b"{}",
        );
    }
    write(&root.join("app/cantus.exe"), b"MZ cantus");
    write(&root.join("LICENSE-GPL"), b"GNU GENERAL PUBLIC LICENSE");
    write(&root.join("LICENSE"), b"Cantus is dual-licensed by crate.");
    write(&root.join("README.txt"), b"Cantus {version}\nline two\n");
    write(&root.join("NOTICES.md"), b"# Third-party notices");
    write(&root.join("index.html"), b"<!doctype html>");
    Inputs {
        exe: root.join("app/cantus.exe"),
        bundled,
        plugins: names.iter().map(|s| s.to_string()).collect(),
        license: root.join("LICENSE-GPL"),
        copyright: root.join("LICENSE"),
        readme: root.join("README.txt"),
        docs: vec![
            ("THIRD-PARTY-NOTICES.txt".into(), root.join("NOTICES.md")),
            ("manual/en/index.html".into(), root.join("index.html")),
        ],
    }
}

fn plugin_files(prefix: &str) -> Vec<String> {
    let mut v = Vec::new();
    for n in ["Cantus Aequalis", "Cantus Porta"] {
        v.push(format!("{prefix}CLAP/{n}.clap"));
        v.push(format!(
            "{prefix}VST3/{n}.vst3/Contents/Resources/moduleinfo.json"
        ));
        v.push(format!(
            "{prefix}VST3/{n}.vst3/Contents/x86_64-win/{n}.vst3"
        ));
    }
    v
}

#[test]
fn the_package_task_produces_the_expected_file_list() {
    let dir = tempfile::tempdir().unwrap();
    let inputs = stand_ins(dir.path());
    let out = dir.path().join("package");
    let dist = package::assemble(&inputs, &out, V, None, None);

    let got: Vec<String> = archive::walk(&dist).into_iter().map(|e| e.name).collect();
    assert_eq!(got, package::dist_names(V, false));
    // One Windows edition, every driver in it (round 21): no "-asio" files.
    assert_eq!(
        got,
        [
            "Cantus-0.2.0-plugins-windows-x64.zip",
            "Cantus-0.2.0-windows-x64-portable.zip",
            "SHA256SUMS.txt",
        ]
    );
    assert_eq!(
        package::dist_names(V, true),
        [
            "Cantus-0.2.0-plugins-windows-x64.zip",
            "Cantus-0.2.0-windows-x64-portable.zip",
            "Cantus-0.2.0-windows-x64-setup.exe",
            "SHA256SUMS.txt",
        ]
    );

    // The portable zip: the app at the top of one folder, the marker, the plug-ins.
    let mut want: Vec<String> = [
        "COPYRIGHT.txt",
        "LICENSE.txt",
        "README.txt",
        "THIRD-PARTY-NOTICES.txt",
        "cantus.exe",
        "manual/en/index.html",
        "portable.txt",
    ]
    .iter()
    .map(|s| format!("Cantus-0.2.0/{s}"))
    .collect();
    want.extend(plugin_files("Cantus-0.2.0/plugins/"));
    want.sort();
    assert_eq!(archive::names(&dist.join(release::portable_name(V))), want);

    // The plug-ins zip: no program, the texts, CLAP/ and VST3/.
    // The plug-ins carry the same crates, so the notices go with them; the manual does not.
    let mut want: Vec<String> = [
        "COPYRIGHT.txt",
        "LICENSE.txt",
        "README.txt",
        "THIRD-PARTY-NOTICES.txt",
    ]
    .iter()
        .map(|s| format!("Cantus-0.2.0-plugins/{s}"))
        .collect();
    want.extend(plugin_files("Cantus-0.2.0-plugins/"));
    want.sort();
    assert_eq!(archive::names(&dist.join(release::plugins_name(V))), want);

    // The program; the readme carries the version, in CRLF.
    let staged_exe = std::fs::read(out.join("stage/app/cantus.exe")).unwrap();
    assert_eq!(staged_exe, b"MZ cantus");
    let readme = std::fs::read_to_string(out.join("stage/app/README.txt")).unwrap();
    assert_eq!(readme, "Cantus 0.2.0\r\nline two\r\n");

    // The sums cover every other file (the two zips here), and match.
    let sums = release::parse_sums(&std::fs::read_to_string(dist.join("SHA256SUMS.txt")).unwrap());
    assert_eq!(sums.len(), 2);
    for (name, hex) in &sums {
        assert_eq!(
            &cantus_update::verify::sha256_file(&dist.join(name)).unwrap(),
            hex
        );
    }
}

#[test]
fn the_uninstaller_removes_exactly_what_the_installer_writes() {
    let dir = tempfile::tempdir().unwrap();
    let inputs = stand_ins(dir.path());
    let staged = package::stage(&inputs, &inputs.exe, &dir.path().join("stage"), V);
    let nsh = nsis::files_nsh(&staged);
    let macro_body = |name: &str| -> Vec<String> {
        let start = nsh.find(&format!("!macro {name}\n")).unwrap();
        let end = start + nsh[start..].find("!macroend").unwrap();
        nsh[start..end]
            .lines()
            .skip(1)
            .map(|l| l.trim().to_owned())
            .collect()
    };
    let files = |m: &str| {
        macro_body(m)
            .iter()
            .filter(|l| l.starts_with("File "))
            .count()
    };
    let deletes = |m: &str| {
        macro_body(m)
            .iter()
            .filter(|l| l.starts_with("Delete "))
            .count()
    };
    assert_eq!(
        (files("CANTUS_APP_FILES"), deletes("CANTUS_APP_REMOVE")),
        (6, 6)
    );
    // The manual's folders go with it, deepest first.
    let app = macro_body("CANTUS_APP_REMOVE");
    let at = |s: &str| app.iter().position(|l| l == s).unwrap_or_else(|| panic!("{s}"));
    assert!(at("RMDir \"$INSTDIR\\manual\\en\"") < at("RMDir \"$INSTDIR\\manual\""));
    assert_eq!(
        (files("CANTUS_CLAP_FILES"), deletes("CANTUS_CLAP_REMOVE")),
        (2, 2)
    );
    assert_eq!(
        (files("CANTUS_VST3_FILES"), deletes("CANTUS_VST3_REMOVE")),
        (4, 4)
    );

    // The roots are NSIS expressions, written as they are (`$$INSTDIR` would be a folder
    // literally named "$INSTDIR").
    assert!(!nsh.contains("$$"), "{nsh}");
    assert!(macro_body("CANTUS_APP_FILES").contains(&"SetOutPath \"$INSTDIR\"".to_string()));
    assert!(macro_body("CANTUS_CLAP_REMOVE").contains(&"RMDir \"${CLAP_DIR}\"".to_string()));
    let vst3 = macro_body("CANTUS_VST3_REMOVE");
    assert!(vst3.contains(
        &"Delete \"${VST3_DIR}\\Cantus Porta.vst3\\Contents\\x86_64-win\\Cantus Porta.vst3\""
            .to_string()
    ));
    // Folders go deepest first, only once empty (no /r), and the Cantus folder last.
    let rmdirs: Vec<&String> = vst3.iter().filter(|l| l.starts_with("RMDir")).collect();
    assert!(rmdirs.iter().all(|l| !l.contains("/r")));
    assert_eq!(rmdirs.last().unwrap().as_str(), "RMDir \"${VST3_DIR}\"");
    let pos = |s: &str| rmdirs.iter().position(|l| l.ends_with(s)).unwrap();
    assert!(
        pos("Cantus Porta.vst3\\Contents\\x86_64-win\"") < pos("Cantus Porta.vst3\\Contents\"")
    );
    assert!(pos("Cantus Porta.vst3\\Contents\"") < pos("Cantus Porta.vst3\""));
    assert!(
        !nsh.contains("RMDir \"$INSTDIR\"\n"),
        "the script removes $INSTDIR itself, last"
    );
    assert!(nsh.contains("!define EST_SIZE_KB "));
}

#[test]
fn installer_strings_come_from_the_locale_area_in_both_languages() {
    let ws = crate::workspace();
    let read = |l: &str| {
        std::fs::read_to_string(ws.join(format!("crates/cantus-ui/locales/areas/update.{l}.txt")))
            .unwrap()
    };
    let nsh = nsis::strings_nsh(&read("en"), &read("es"));
    let names = |lang: &str| -> Vec<String> {
        nsh.lines()
            .filter(|l| l.contains(&format!("${{LANG_{lang}}}")))
            .map(|l| l.split_whitespace().nth(1).unwrap().to_owned())
            .collect()
    };
    assert_eq!(names("ENGLISH"), names("SPANISH"));
    // Every string the script names exists.
    let script = std::fs::read_to_string(ws.join("xtask/installer/cantus.nsi")).unwrap();
    let mut used: Vec<&str> = script
        .match_indices("$(CANTUS_")
        .map(|(i, _)| {
            let rest = &script[i + 2..];
            &rest[..rest.find(')').unwrap()]
        })
        .collect();
    used.sort();
    used.dedup();
    for u in used {
        assert!(names("ENGLISH").iter().any(|n| n == u), "{u} has no string");
    }
    assert!(nsh.contains("LangString CANTUS_PROJECT_TYPE ${LANG_SPANISH} \"Proyecto de Cantus\""));
    assert_eq!(nsis::escape("a$b \"c\"\nd"), "a$$b $\\\"c$\\\"$\\r$\\nd");
    assert_eq!(nsis::version4("0.2.0-rc.1"), "0.2.0.0");
}

/// With NSIS on this machine (`CANTUS_MAKENSIS`, the PATH, or the portable copy the
/// packaging doc describes), compile a real installer from the stand-ins.
#[test]
fn an_installer_compiles_when_nsis_is_here() {
    let Some(makensis) = nsis::makensis() else {
        eprintln!("NSIS not found: skipped");
        return;
    };
    let dir = tempfile::tempdir().unwrap();
    // The one edition, with its FlexASIO component.
    let inputs = stand_ins(dir.path());
    let out = dir.path().join("package");
    // NSIS's own icon stands in for the app's (which the app's build script renders).
    let Some(icon) = makensis
        .parent()
        .map(|d| d.join(r"Contrib\Graphics\Icons\modern-install.ico"))
        .filter(|p| p.is_file())
    else {
        eprintln!("no NSIS icon to hand: skipped");
        return;
    };
    // `CANTUS_TEST_SIGN_WITH=<xtask.exe>` (with `sign-test-cert` done): the uninstaller is
    // signed through NSIS's `!uninstfinalize` hook, as a signed release does it.
    let (uninst_sign, env) = match std::env::var("CANTUS_TEST_SIGN_WITH") {
        Ok(xtask) => {
            let Ok(crate::sign::Signer::Pfx { file, password }) =
                crate::sign::test_signer(&crate::target_dir())
            else {
                panic!("CANTUS_TEST_SIGN_WITH needs `xtask sign-test-cert` first");
            };
            (
                Some(format!("\"{xtask}\" sign-file \"%1\"")),
                vec![
                    ("CANTUS_SIGN".to_string(), "pfx".to_string()),
                    ("CANTUS_SIGN_PFX".to_string(), file.display().to_string()),
                    ("CANTUS_SIGN_PFX_PASSWORD".to_string(), password),
                    ("CANTUS_SIGN_TIMESTAMP".to_string(), "none".to_string()),
                ],
            )
        }
        Err(_) => (None, Vec::new()),
    };
    let job = InstallerJob {
        makensis,
        icon,
        uninst_sign,
        env,
    };
    let dist = package::assemble(&inputs, &out, V, Some(&job), None);
    assert_eq!(
        archive::walk(&dist).len(),
        package::dist_names(V, true).len()
    );
    {
        let setup = dist.join(release::installer_name(V));
        let bytes = std::fs::read(&setup).unwrap();
        assert_eq!(&bytes[..2], b"MZ", "a Windows program");
        assert!(bytes.len() > 30_000);
        // `CANTUS_KEEP_INSTALLER=<dir>` keeps them, to install and uninstall by hand.
        if let Some(keep) = std::env::var_os("CANTUS_KEEP_INSTALLER") {
            let keep = std::path::PathBuf::from(keep);
            std::fs::create_dir_all(&keep).unwrap();
            std::fs::copy(&setup, keep.join(setup.file_name().unwrap())).unwrap();
        }
    }
}

/// The SDK's checks: the pinned hash, a proxy's page refused, the SDK found in a tree,
/// and the built program's answer read.
#[test]
fn the_asio_sdk_is_checked_and_found() {
    use crate::asio_sdk;
    // A proxy's quarantine page and any other file are refused, with a reason.
    let page = b"<!DOCTYPE html><html><title>Zscaler</title></html>";
    assert!(asio_sdk::refuse_zip(page).unwrap().contains("web page"));
    assert!(asio_sdk::refuse_zip(b"PK\x03\x04 not the sdk")
        .unwrap()
        .contains("SHA-256"));
    assert_eq!(asio_sdk::SDK_SHA256.len(), 64);
    // The workflow pins the same hash.
    let wf =
        std::fs::read_to_string(crate::workspace().join(".github/workflows/release.yml")).unwrap();
    assert!(
        wf.contains(asio_sdk::SDK_SHA256),
        "release.yml pins another SDK hash"
    );
    // The zip's top folder is found however it is named.
    let dir = tempfile::tempdir().unwrap();
    let sdk = dir.path().join("asiosdk_2.3.4_2025-10-15/ASIOSDK");
    write(&sdk.join("common/asio.h"), b"//");
    write(&sdk.join("host/asiodrivers.h"), b"//");
    assert_eq!(asio_sdk::find_sdk_in(dir.path()), Some(sdk.clone()));
    assert!(asio_sdk::is_sdk(&sdk));
    assert_eq!(
        asio_sdk::find_sdk_in(&dir.path().join("asiosdk_2.3.4_2025-10-15/ASIOSDK/common")),
        None
    );
    // Unpacked from a zip into the cache layout.
    let z = dir.path().join("sdk.zip");
    archive::zip(&z, "ASIOSDK_2.3.4", &archive::walk(&sdk));
    let cache = dir.path().join("cache");
    let got = asio_sdk::unpack(&z, &cache).unwrap();
    assert_eq!(got, cache.join(asio_sdk::SDK_DIR));
    assert!(asio_sdk::is_sdk(&got));
    // `cantus --audio-drivers`.
    assert!(asio_sdk::lists_asio(
        "audio drivers: WASAPI, ASIO\nasio sdk: linked\n"
    ));
    assert!(!asio_sdk::lists_asio(
        "audio drivers: WASAPI\nasio sdk: not linked\n"
    ));
    assert!(!asio_sdk::lists_asio(""));
}
