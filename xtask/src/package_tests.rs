//! The package task, run on stand-in inputs: the release's file list, what each zip holds,
//! the sums, and the installer script's install/uninstall symmetry. With NSIS on the
//! machine, a real installer is compiled from the same stand-ins.

use crate::archive;
use crate::nsis;
use crate::package::{self, Inputs, InstallerJob};
use cantus_launch::Level;
use cantus_update::release;
use std::path::Path;

const V: &str = "0.2.0";

fn write(p: &Path, bytes: &[u8]) {
    std::fs::create_dir_all(p.parent().unwrap()).unwrap();
    std::fs::write(p, bytes).unwrap();
}

/// Two plug-ins laid out as `xtask bundle` lays them out, the launcher, the v2 and v3
/// programs, the scanner, the plug-in sandbox (round 29), the texts.
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
    write(&root.join("launch/cantus-launch.exe"), b"MZ launcher");
    write(&root.join("v2/cantus.exe"), b"MZ cantus v2");
    write(&root.join("v3/cantus.exe"), b"MZ cantus v3");
    write(&root.join("v2/cantus-scan.exe"), b"MZ scanner");
    write(&root.join("v2/cantus-sandbox.exe"), b"MZ sandbox");
    write(&root.join("LICENSE-GPL"), b"GNU GENERAL PUBLIC LICENSE");
    write(&root.join("LICENSE"), b"Cantus is dual-licensed by crate.");
    write(&root.join("README.txt"), b"Cantus {version}\nline two\n");
    write(&root.join("NOTICES.md"), b"# Third-party notices");
    write(&root.join("index.html"), b"<!doctype html>");
    Inputs {
        launcher: root.join("launch/cantus-launch.exe"),
        programs: vec![
            (Level::V2, root.join("v2/cantus.exe")),
            (Level::V3, root.join("v3/cantus.exe")),
        ],
        scanner: root.join("v2/cantus-scan.exe"),
        sandbox: root.join("v2/cantus-sandbox.exe"),
        bundled,
        plugins: names.iter().map(|s| s.to_string()).collect(),
        license: root.join("LICENSE-GPL"),
        copyright: root.join("LICENSE"),
        readme: root.join("README.txt"),
        docs: vec![
            ("THIRD-PARTY-NOTICES.txt".into(), root.join("NOTICES.md")),
            ("manual/en/index.html".into(), root.join("index.html")),
        ],
        check_metadata: false,
    }
}

/// A release whose signed files lack their product name and version stops (ADR 0048 §9).
#[test]
#[should_panic(expected = "without their version resource")]
fn a_release_stops_when_a_file_it_signs_has_no_version_resource() {
    let dir = tempfile::tempdir().unwrap();
    let mut inputs = stand_ins(dir.path());
    inputs.check_metadata = true;
    package::assemble(&inputs, &dir.path().join("package"), V, None, None);
}

/// … and goes on when every one carries them.
#[test]
fn a_release_whose_files_carry_their_version_resource_is_made() {
    let dir = tempfile::tempdir().unwrap();
    let mut inputs = stand_ins(dir.path());
    let res = crate::package::meta::tests::with_resource(V);
    let mut files = vec![
        inputs.launcher.clone(),
        inputs.scanner.clone(),
        inputs.sandbox.clone(),
    ];
    files.extend(inputs.programs.iter().map(|(_, p)| p.clone()));
    for n in &inputs.plugins {
        files.push(inputs.bundled.join(format!("{n}.clap")));
        files.push(
            inputs
                .bundled
                .join(format!("{n}.vst3/Contents/x86_64-win/{n}.vst3")),
        );
    }
    for f in &files {
        std::fs::write(f, &res).unwrap();
    }
    inputs.check_metadata = true;
    let dist = package::assemble(&inputs, &dir.path().join("package"), V, None, None);
    assert!(dist.join(release::portable_name(V)).is_file());
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
            "Cantus-windows-x64-setup.exe",
            "SHA256SUMS.txt",
        ]
    );
    // The installer under a name that never changes too (ADR 0048 §7), for the website's
    // `releases/latest/download/<name>` link; the updater still asks for the versioned one.
    assert_eq!(
        release::STABLE_INSTALLER_NAME,
        "Cantus-windows-x64-setup.exe"
    );
    assert_ne!(release::STABLE_INSTALLER_NAME, release::installer_name(V));

    // The portable zip: the launcher, both programs and the scanner at the top of one
    // folder, the marker, the plug-ins.
    let mut want: Vec<String> = [
        "COPYRIGHT.txt",
        "LICENSE.txt",
        "README.txt",
        "THIRD-PARTY-NOTICES.txt",
        "cantus-sandbox.exe",
        "cantus-scan.exe",
        "cantus-x86-64-v2.exe",
        "cantus-x86-64-v3.exe",
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

    // The plug-ins zip: no program (neither the launcher, the programs nor the scanner),
    // the texts, CLAP/ and VST3/. The plug-ins carry the same crates, so the notices go
    // with them; the manual does not.
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

    // cantus.exe is the launcher; each level's program under its own name; the readme
    // carries the version, in CRLF.
    let staged = |n: &str| std::fs::read(out.join("stage/app").join(n)).unwrap();
    assert_eq!(staged("cantus.exe"), b"MZ launcher");
    assert_eq!(staged("cantus-x86-64-v2.exe"), b"MZ cantus v2");
    assert_eq!(staged("cantus-x86-64-v3.exe"), b"MZ cantus v3");
    assert_eq!(staged("cantus-scan.exe"), b"MZ scanner");
    assert_eq!(staged("cantus-sandbox.exe"), b"MZ sandbox");
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
    let staged = package::stage(&inputs, &dir.path().join("stage"), V);
    let nsh = nsis::files_nsh_for(&staged, true);
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
        (9, 9)
    );
    // The manual's folders go with it, deepest first.
    let app = macro_body("CANTUS_APP_REMOVE");
    let at = |s: &str| {
        app.iter()
            .position(|l| l == s)
            .unwrap_or_else(|| panic!("{s}"))
    };
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

    // The installer and the uninstaller wait for every program of the folder to close —
    // the launcher, both levels, the scanner, the plug-in sandbox — not only cantus.exe.
    let busy = macro_body("CANTUS_PROGRAMS_BUSY");
    let waited: Vec<&str> = busy
        .iter()
        .filter_map(|l| l.strip_prefix("!insertmacro CANTUS_PROGRAM_BUSY \"$INSTDIR\\"))
        .filter_map(|l| l.strip_suffix('"'))
        .collect();
    assert_eq!(
        waited,
        [
            "cantus.exe",
            "cantus-x86-64-v2.exe",
            "cantus-x86-64-v3.exe",
            "cantus-scan.exe",
            "cantus-sandbox.exe"
        ]
    );
    let script =
        std::fs::read_to_string(crate::workspace().join("xtask/installer/cantus.nsi")).unwrap();
    let wait = &script[script.find("!macro WAIT_FOR_CANTUS").unwrap()..];
    assert!(wait[..wait.find("!macroend").unwrap()].contains("!insertmacro CANTUS_PROGRAMS_BUSY"));
}

/// The release's install folder: the launcher as `cantus.exe` (what the shortcut, the
/// association, "Open with" and the installer's "run" name), then each level's program,
/// the scanner, the texts; `--v4` adds one program.
#[test]
fn the_install_folder_holds_the_launcher_and_one_program_per_level() {
    assert_eq!(
        package::app_names(&crate::levels::release_levels(false)),
        [
            "cantus.exe",
            "cantus-x86-64-v2.exe",
            "cantus-x86-64-v3.exe",
            "cantus-scan.exe",
            "cantus-sandbox.exe",
            "LICENSE.txt",
            "COPYRIGHT.txt",
            "README.txt",
        ]
    );
    let v4 = package::app_names(&crate::levels::release_levels(true));
    assert!(v4.contains(&"cantus-x86-64-v4.exe".to_string()));
    assert_eq!(v4.len(), 9);
    let script =
        std::fs::read_to_string(crate::workspace().join("xtask/installer/cantus.nsi")).unwrap();
    for line in script.lines().filter(|l| {
        let l = l.trim_start();
        l.starts_with("CreateShortcut")
            || l.contains(r"shell\open\command")
            || l.contains("MUI_FINISHPAGE_RUN")
            || l.contains("DefaultIcon")
    }) {
        assert!(line.contains(r"$INSTDIR\cantus.exe"), "{line}");
        assert!(!line.contains("x86-64"), "{line}");
    }
    // Every shortcut is given the AppUserModelID the program sets for itself, so the
    // program's window groups (and pins) under the launcher's shortcut.
    let lines: Vec<&str> = script.lines().map(str::trim).collect();
    for (i, l) in lines.iter().enumerate() {
        if l.starts_with("CreateShortcut") {
            let lnk = l.split('"').nth(1).unwrap();
            assert!(
                lines[i + 1..]
                    .iter()
                    .take(4)
                    .any(|n| n.contains("CANTUS_SET_APP_ID") && n.contains(lnk)),
                "{l} has no app id"
            );
        }
    }
    let config = nsis::config_nsh(&nsis::Config {
        version: V.into(),
        outfile: "setup.exe".into(),
        icon: None,
        license: "LICENSE.txt".into(),
        uninst_sign: None,
        uninstaller: nsis::Uninstaller::Written,
    });
    assert!(config.contains(&format!("!define APP_ID \"{}\"\n", cantus_launch::APP_ID)));
    // The levels' names and flags, and the app's own answer to --version.
    assert_eq!(
        crate::levels::parse_list("v3,v2,v3").unwrap(),
        [Level::V2, Level::V3]
    );
    assert!(crate::levels::parse_list("v2,fast").is_err());
    assert!(crate::levels::says_level(
        "cantus 0.1.0 (x86-64-v3)\n",
        Level::V3
    ));
    assert!(!crate::levels::says_level(
        "cantus 0.1.0 (x86-64)\n",
        Level::V3
    ));
    assert!(!crate::levels::says_level(
        "cantus 0.1.0 (x86-64-v2)\n",
        Level::V3
    ));
    assert!(
        crate::levels::release_dir(Level::V3, "x86_64-pc-windows-msvc")
            .ends_with("cpu-x86-64-v3/x86_64-pc-windows-msvc/release")
    );
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
        uninstaller: nsis::Uninstaller::Written,
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
        // The stable name is the same file, byte for byte, and SHA256SUMS lists both.
        let stable = dist.join(release::STABLE_INSTALLER_NAME);
        assert_eq!(std::fs::read(&stable).unwrap(), bytes, "the same installer");
        let sums =
            release::parse_sums(&std::fs::read_to_string(dist.join(release::SUMS_NAME)).unwrap());
        assert_eq!(
            sums.get(release::STABLE_INSTALLER_NAME),
            sums.get(&release::installer_name(V))
        );
        assert!(sums.contains_key(release::STABLE_INSTALLER_NAME));
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

/// The three ways the installer comes by its uninstaller (ADR 0048 §8), in the script's
/// generated config and file list.
#[test]
fn the_uninstaller_is_written_prebuilt_or_made_alone() {
    let cfg = |u: nsis::Uninstaller, sign: Option<&str>| {
        nsis::config_nsh(&nsis::Config {
            version: V.into(),
            outfile: "setup.exe".into(),
            icon: None,
            license: "LICENSE.txt".into(),
            uninst_sign: sign.map(str::to_owned),
            uninstaller: u,
        })
    };
    // Written: signed mid-build when a local signer is set; nothing else defined.
    let written = cfg(
        nsis::Uninstaller::Written,
        Some("\"x.exe\" sign-file \"%1\""),
    );
    assert!(written.contains("!uninstfinalize"));
    assert!(!written.contains("PREBUILT_UNINSTALLER") && !written.contains("UNINSTALLER_ONLY"));
    // Prebuilt: carried as a file, never re-signed by NSIS.
    let prebuilt = cfg(
        nsis::Uninstaller::Prebuilt(r"C:\stage\uninstall.exe".into()),
        Some("ignored"),
    );
    assert!(prebuilt.contains("!define PREBUILT_UNINSTALLER \"C:\\stage\\uninstall.exe\""));
    assert!(!prebuilt.contains("!uninstfinalize"));
    // Writer only: the define the script's .onInit looks for.
    let writer = cfg(nsis::Uninstaller::WriterOnly, None);
    assert!(writer.contains("!define UNINSTALLER_ONLY"));
    // The script itself honours each.
    let nsi = std::fs::read_to_string(crate::workspace().join("xtask/installer/cantus.nsi"))
        .unwrap()
        .replace("\r\n", "\n");
    assert!(nsi.contains("!ifdef UNINSTALLER_ONLY\n  ; The uninstaller-only pass"));
    assert!(nsi.contains("File \"/oname=uninstall.exe\" \"${PREBUILT_UNINSTALLER}\""));

    // The writer carries no release file, but its uninstaller removes every one.
    let dir = tempfile::tempdir().unwrap();
    let inputs = stand_ins(dir.path());
    let staged = package::stage(&inputs, &dir.path().join("stage"), V);
    let full = nsis::files_nsh_for(&staged, true);
    let alone = nsis::files_nsh_for(&staged, false);
    assert!(full.contains("  File \""));
    assert!(!alone.contains("  File \""), "the writer carries no file");
    let deletes = |t: &str| {
        t.lines()
            .filter(|l| l.trim_start().starts_with("Delete"))
            .count()
    };
    assert_eq!(deletes(&full), deletes(&alone));
    assert!(deletes(&alone) > 5);
}

/// A stage read back from its folder (what SignPath returns) is the stage that was laid out.
#[test]
fn a_stage_reads_back_from_its_folder() {
    let dir = tempfile::tempdir().unwrap();
    let inputs = stand_ins(dir.path());
    let stage_dir = dir.path().join("stage");
    let staged = package::stage(&inputs, &stage_dir, V);
    std::fs::write(stage_dir.join(package::UNINSTALLER), b"MZ").unwrap();
    let back = package::Staged::from_dir(&stage_dir);
    let names = |l: &[archive::Entry]| {
        let mut v: Vec<String> = l.iter().map(|e| e.name.clone()).collect();
        v.sort();
        v
    };
    assert_eq!(names(&back.app), names(&staged.app));
    assert_eq!(names(&back.clap), names(&staged.clap));
    assert_eq!(names(&back.vst3), names(&staged.vst3));
    assert!(!back.app.iter().any(|e| e.name == package::UNINSTALLER));
}

/// With NSIS here: the uninstaller-only pass writes an uninstaller that names its product
/// and version (SignPath's metadata rule, ADR 0048 §9), and an installer that carries it
/// compiles from the same stage.
#[test]
#[cfg(windows)]
fn the_uninstaller_is_made_alone_and_carried_by_the_installer_when_nsis_is_here() {
    let Some(makensis) = nsis::makensis() else {
        eprintln!("NSIS not found: skipped");
        return;
    };
    let Some(icon) = makensis
        .parent()
        .map(|d| d.join(r"Contrib\Graphics\Icons\modern-install.ico"))
        .filter(|p| p.is_file())
    else {
        eprintln!("no NSIS icon to hand: skipped");
        return;
    };
    let dir = tempfile::tempdir().unwrap();
    let inputs = stand_ins(dir.path());
    let out = dir.path().join("package");
    let staged = package::stage(&inputs, &out.join("stage"), V);
    let un =
        package::write_uninstaller(&staged, &out.join("uninstaller"), &makensis, Some(&icon), V);
    let bytes = std::fs::read(&un).unwrap();
    assert_eq!(&bytes[..2], b"MZ");
    assert!(crate::package::meta::problems(std::slice::from_ref(&un), V).is_empty());
    let job = InstallerJob {
        makensis,
        icon,
        uninst_sign: None,
        env: Vec::new(),
        uninstaller: nsis::Uninstaller::Prebuilt(un),
    };
    let dist = package::package_staged(&staged, &out, V, Some(&job), None, false);
    let setup = std::fs::read(dist.join(release::installer_name(V))).unwrap();
    assert_eq!(&setup[..2], b"MZ");
    assert!(
        setup.len() > bytes.len(),
        "the installer carries the uninstaller"
    );
    let script = std::fs::read_to_string(out.join("nsis").join("config.nsh")).unwrap();
    assert!(script.contains("PREBUILT_UNINSTALLER"));
}

/// `--from-stage` with a relative folder, as the SignPath job calls it: the stage and every
/// file in it are named by absolute paths, which makensis needs (it resolves relative ones
/// from the script's folder).
#[test]
fn a_relative_stage_folder_becomes_absolute() {
    let here = std::env::current_dir().unwrap();
    let dir = tempfile::tempdir_in(&here).unwrap();
    let rel = dir.path().strip_prefix(&here).unwrap().join("stage");
    let inputs = stand_ins(dir.path());
    let _ = package::stage(&inputs, &here.join(&rel), V);
    assert!(rel.is_relative());
    let abs = package::stage_dir(&rel).unwrap();
    assert!(abs.is_absolute(), "{}", abs.display());
    let staged = package::Staged::from_dir(&abs);
    assert!(!staged.app.is_empty());
    for e in staged.app.iter().chain(&staged.clap).chain(&staged.vst3) {
        assert!(e.src.is_absolute(), "{}", e.src.display());
    }
    assert!(package::stage_dir(&rel.join("missing")).is_err());
}

/// `sign-file` checks the uninstaller against the version the job builds (the NSIS tests
/// build 0.2.0), and against the release's own when nothing says otherwise.
#[test]
fn sign_file_checks_the_version_being_built() {
    assert_eq!(package::sign_version(Some("0.2.0".into())), "0.2.0");
    assert_eq!(package::sign_version(None), package::VERSION);
    assert_eq!(package::sign_version(Some(" ".into())), package::VERSION);
}
