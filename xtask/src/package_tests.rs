//! The package task, run on stand-in inputs: the release's file list, what each zip holds,
//! the sums, and the installer script's install/uninstall symmetry. With NSIS on the
//! machine, a real installer is compiled from the same stand-ins.

use crate::archive;
use crate::nsis;
use crate::package::{self, Inputs, InstallerJob};
use cantus_update::release::{self, Flavor};
use std::path::Path;

const V: &str = "0.2.0";

fn write(p: &Path, bytes: &[u8]) {
    std::fs::create_dir_all(p.parent().unwrap()).unwrap();
    std::fs::write(p, bytes).unwrap();
}

/// Two plug-ins laid out as `xtask bundle` lays them out, two executables, the texts.
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
    write(&root.join("std/cantus.exe"), b"MZ standard");
    write(&root.join("asio/cantus.exe"), b"MZ asio");
    write(&root.join("LICENSE-GPL"), b"GNU GENERAL PUBLIC LICENSE");
    write(&root.join("LICENSE"), b"Cantus is dual-licensed by crate.");
    write(&root.join("README.txt"), b"Cantus {version}\nline two\n");
    Inputs {
        exes: vec![
            (Flavor::Standard, root.join("std/cantus.exe")),
            (Flavor::Asio, root.join("asio/cantus.exe")),
        ],
        bundled,
        plugins: names.iter().map(|s| s.to_string()).collect(),
        license: root.join("LICENSE-GPL"),
        copyright: root.join("LICENSE"),
        readme: root.join("README.txt"),
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
    assert_eq!(got, package::dist_names(V, &Flavor::ALL, false));
    assert_eq!(
        got,
        [
            "Cantus-0.2.0-plugins-windows-x64.zip",
            "Cantus-0.2.0-windows-x64-asio-portable.zip",
            "Cantus-0.2.0-windows-x64-portable.zip",
            "SHA256SUMS.txt",
        ]
    );
    assert_eq!(
        package::dist_names(V, &[Flavor::Standard], true),
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
        "cantus.exe",
        "portable.txt",
    ]
    .iter()
    .map(|s| format!("Cantus-0.2.0/{s}"))
    .collect();
    want.extend(plugin_files("Cantus-0.2.0/plugins/"));
    want.sort();
    assert_eq!(
        archive::names(&dist.join(release::portable_name(V, Flavor::Standard))),
        want
    );
    let asio = archive::names(&dist.join(release::portable_name(V, Flavor::Asio)));
    assert!(asio.iter().all(|n| n.starts_with("Cantus-0.2.0-asio/")));
    assert_eq!(asio.len(), want.len());

    // The plug-ins zip: no program, the texts, CLAP/ and VST3/.
    let mut want: Vec<String> = ["COPYRIGHT.txt", "LICENSE.txt", "README.txt"]
        .iter()
        .map(|s| format!("Cantus-0.2.0-plugins/{s}"))
        .collect();
    want.extend(plugin_files("Cantus-0.2.0-plugins/"));
    want.sort();
    assert_eq!(archive::names(&dist.join(release::plugins_name(V))), want);

    // Each flavour's own program; the readme carries the version, in CRLF.
    let staged_exe = std::fs::read(out.join("stage/asio/app/cantus.exe")).unwrap();
    assert_eq!(staged_exe, b"MZ asio");
    let readme = std::fs::read_to_string(out.join("stage/standard/app/README.txt")).unwrap();
    assert_eq!(readme, "Cantus 0.2.0\r\nline two\r\n");

    // The sums cover every other file, and match.
    let sums = release::parse_sums(&std::fs::read_to_string(dist.join("SHA256SUMS.txt")).unwrap());
    assert_eq!(sums.len(), 3);
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
    let staged = package::stage(&inputs, &inputs.exes[0].1, &dir.path().join("stage"), V);
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
        (4, 4)
    );
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
    // Both flavours: the ASIO one compiles the FlexASIO component too.
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
        package::dist_names(V, &Flavor::ALL, true).len()
    );
    for f in Flavor::ALL {
        let setup = dist.join(release::installer_name(V, f));
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
