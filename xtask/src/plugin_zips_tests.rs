//! Each effect's plug-ins on their own (round 42, `plugin_zips.rs`): their names, what each
//! zip holds, the release workflow that makes, checks and publishes them on every system,
//! and the website that links them — kept one.

use crate::archive;
use crate::plugin_zips::{self, Source};
use cantus_plugins::desc;
use cantus_update::release::{self, Target};
use std::collections::HashSet;
use std::path::Path;

fn read(rel: &str) -> String {
    std::fs::read_to_string(crate::workspace().join(rel))
        .unwrap_or_else(|e| panic!("read {rel}: {e}"))
        .replace("\r\n", "\n")
}

fn put(path: &Path, text: &str) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, text).unwrap();
}

/// The text of one job of a workflow: from its key to the next job's.
fn job(yaml: &str, name: &str) -> String {
    let key = format!("\n  {name}:\n");
    let at = yaml.find(&key).unwrap_or_else(|| panic!("no job {name}"));
    let rest = &yaml[at + key.len()..];
    let end = rest
        .lines()
        .scan(0usize, |pos, l| {
            let here = *pos;
            *pos += l.len() + 1;
            Some((here, l))
        })
        .find(|(_, l)| l.starts_with("  ") && !l.starts_with("   ") && l.trim_end().ends_with(':'))
        .map_or(rest.len(), |(p, _)| p);
    rest[..end].to_string()
}

fn before(text: &str, a: &str, b: &str) -> bool {
    match (text.find(a), text.find(b)) {
        (Some(x), Some(y)) => x < y,
        _ => false,
    }
}

#[test]
fn every_effect_has_a_stable_plain_zip_name_on_every_system() {
    let mut all = HashSet::new();
    for t in Target::ALL {
        for d in desc::ALL.iter() {
            let n = release::plugin_zip_name(d.slug, t);
            assert_eq!(n, format!("Cantus-plugin-{}-{}.zip", d.slug, t.platform()));
            assert!(all.insert(n.clone()), "{n} twice");
        }
        assert!(all.insert(release::plugins_all_name(t)));
        assert_eq!(plugin_zips::names(t).len(), desc::ALL.len() + 1);
    }
    assert_eq!(
        release::plugin_zip_name("aequalis", Target::WindowsX64),
        "Cantus-plugin-aequalis-windows-x64.zip"
    );
    assert_eq!(
        release::plugins_all_name(Target::MacUniversal),
        "Cantus-plugins-all-macos-universal.zip"
    );
    let v = crate::package::VERSION;
    let others: HashSet<String> = Target::ALL
        .iter()
        .flat_map(|t| [t.file_name(v), t.stable_name().to_string()])
        .chain([
            release::plugins_name(v),
            release::portable_name(v),
            release::SUMS_NAME.to_string(),
        ])
        .collect();
    for n in &all {
        // Plain names GitHub keeps as they are, with no version: `releases/latest/download/`.
        assert!(
            n.bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'.'),
            "{n}"
        );
        assert!(!n.contains(v), "{n} names a version");
        assert!(!others.contains(n), "{n} is another release file's name");
    }
}

#[test]
fn a_mac_zip_holds_both_formats_runnable_and_the_texts() {
    let dir = tempfile::tempdir().unwrap();
    let d = &desc::ALL[0];
    let name = d.full_name;
    let bundled = dir.path().join("bundled");
    for ext in ["clap", "vst3"] {
        let b = bundled.join(format!("{name}.{ext}/Contents"));
        put(&b.join("MacOS").join(name), "program");
        put(&b.join("Info.plist"), "plist");
    }
    put(
        &bundled.join(format!("{name}.vst3/Contents/Resources/moduleinfo.json")),
        "{}",
    );
    let out = dir.path().join("out");
    let src = Source::of(&bundled);
    assert_eq!(src.clap, bundled);
    let wrote = plugin_zips::write(
        &out,
        &dir.path().join("work"),
        &src,
        Target::MacUniversal,
        "0.2.0",
        &[d],
    );
    let one = release::plugin_zip_name(d.slug, Target::MacUniversal);
    let all = release::plugins_all_name(Target::MacUniversal);
    let mut want = vec![one.clone(), all.clone()];
    want.sort();
    assert_eq!(wrote, want);
    let top = format!("{name} 0.2.0");
    let mut expect: Vec<String> = [
        "COPYRIGHT.txt".to_string(),
        "LICENSE.txt".into(),
        "README.txt".into(),
        "THIRD-PARTY-NOTICES.txt".into(),
        format!("CLAP/{name}.clap/Contents/Info.plist"),
        format!("CLAP/{name}.clap/Contents/MacOS/{name}"),
        format!("VST3/{name}.vst3/Contents/Info.plist"),
        format!("VST3/{name}.vst3/Contents/MacOS/{name}"),
        format!("VST3/{name}.vst3/Contents/Resources/moduleinfo.json"),
    ]
    .iter()
    .map(|n| format!("{top}/{n}"))
    .collect();
    expect.sort();
    assert_eq!(archive::names(&out.join(&one)), expect);
    // The programs keep their executable bit; the rest does not get one.
    let f = std::fs::File::open(out.join(&one)).unwrap();
    let mut z = zip::ZipArchive::new(f).unwrap();
    for i in 0..z.len() {
        let e = z.by_index(i).unwrap();
        let mode = e.unix_mode().unwrap_or(0o644) & 0o777;
        let runs = e.name().contains("/Contents/MacOS/");
        assert_eq!(mode, if runs { 0o755 } else { 0o644 }, "{}", e.name());
    }
    let mut readme = String::new();
    std::io::Read::read_to_string(
        &mut z.by_name(&format!("{top}/README.txt")).unwrap(),
        &mut readme,
    )
    .unwrap();
    for (_, you, everyone) in plugin_zips::folders(Target::MacUniversal) {
        assert!(
            readme.contains(you) && readme.contains(everyone),
            "{readme}"
        );
    }
    assert!(readme.contains("Open Anyway") && readme.contains("com.apple.quarantine"));
    assert!(readme.contains("GPL-3.0-or-later") && readme.contains("trademarks"));
    // The zip with all holds the same plug-in under its own top folder.
    assert!(archive::names(&out.join(&all)).contains(&format!(
        "Cantus plug-ins 0.2.0/CLAP/{name}.clap/Contents/MacOS/{name}"
    )));
}

#[test]
fn a_windows_zip_comes_from_the_signed_stage() {
    let dir = tempfile::tempdir().unwrap();
    let d = &desc::ALL[1];
    let name = d.full_name;
    let stage = dir.path().join("stage");
    put(&stage.join(format!("clap/{name}.clap")), "library");
    put(
        &stage.join(format!("vst3/{name}.vst3/Contents/x86_64-win/{name}.vst3")),
        "library",
    );
    let src = Source::of(&stage);
    assert_eq!(src.clap, stage.join("clap"));
    let out = dir.path().join("dist");
    plugin_zips::write(
        &out,
        &dir.path().join("work"),
        &src,
        Target::WindowsX64,
        "0.2.0",
        &[d],
    );
    let names = archive::names(&out.join(release::plugin_zip_name(d.slug, Target::WindowsX64)));
    assert!(names.contains(&format!("{name} 0.2.0/CLAP/{name}.clap")));
    assert!(names.contains(&format!(
        "{name} 0.2.0/VST3/{name}.vst3/Contents/x86_64-win/{name}.vst3"
    )));
    let readme = plugin_zips::readme(Target::WindowsX64, "0.2.0", &[name]);
    assert!(readme.contains("\r\n") && readme.contains("Unblock"));
    // The installer's README names the same folders.
    let installer = read("xtask/installer/README.txt");
    for (_, you, everyone) in plugin_zips::folders(Target::WindowsX64) {
        assert!(
            installer.contains(you) && installer.contains(everyone),
            "{you}"
        );
    }
}

/// Every system's job makes the zips from the bundles it built (Windows from the stage it
/// signed), before its sums; the publish job refuses a release that lacks one on any system.
#[test]
fn the_release_workflow_makes_checks_and_publishes_every_plug_in_zip() {
    let wf = read(".github/workflows/release.yml");
    let package = job(&wf, "package");
    let make = "cargo run -p xtask -- plugin-zips target/package/dist --from target/package/stage --no-build --sums";
    assert!(package.contains(make), "the package job does not make them");
    assert!(before(
        &package,
        "The installer passes the program's own update check",
        make
    ));
    assert!(before(&package, make, "actions/upload-artifact"));

    let linux = job(&wf, "linux");
    let make = "cargo run -q --release -p xtask -- plugin-zips upload --no-build";
    assert!(linux.contains(make), "the Linux job does not make them");
    assert!(before(
        &linux,
        "bash packaging/linux/build-appimage.sh",
        make
    ));
    assert!(before(&linux, make, "sha256sum -- *"));

    let mac = job(&wf, "macos");
    let make = "cargo run -q --release -p xtask -- plugin-zips upload\n";
    assert!(
        mac.contains(make),
        "the Mac job does not build and make them"
    );
    for t in plugin_zips::MAC_TRIPLES {
        assert!(mac.contains(t), "the Mac job lacks the {t} target");
    }
    assert!(before(&mac, "bash packaging/macos/build-dmg.sh", make));
    assert!(before(&mac, make, "shasum -a 256 -- *"));

    let publish = job(&wf, "publish");
    let (one, all) = (
        release::plugin_zip_name("$slug", Target::WindowsX64).replace("windows-x64", "$p"),
        release::plugins_all_name(Target::WindowsX64).replace("windows-x64", "$p"),
    );
    assert!(
        publish.contains(&format!("test -s \"dist/{one}\"")),
        "{one}"
    );
    assert!(
        publish.contains(&format!("test -s \"dist/{all}\"")),
        "{all}"
    );
    let p = |t: Target| t.platform();
    assert!(publish.contains(&format!(
        "platforms=\"{} {}\"",
        p(Target::WindowsX64),
        p(Target::LinuxX64)
    )));
    assert!(publish.contains(&format!(
        "platforms=\"$platforms {}\"",
        p(Target::MacUniversal)
    )));
}

/// The website's Plug-ins pages link the same names, through the same repository, for the
/// same plug-ins, and tell the same folders as the zips' README.
#[test]
fn the_site_links_every_plug_in_by_its_stable_name_and_folders() {
    let site = read("web/src/plugins.rs");
    assert!(site.contains(&format!(
        "\"https://github.com/{}/releases/latest/download/\"",
        cantus_update::updater::REPO
    )));
    assert!(site.contains("\"Cantus-plugin-{slug}-{platform}.zip\""));
    assert!(site.contains("\"Cantus-plugins-all-{platform}.zip\""));
    for t in Target::ALL {
        assert!(site.contains(&format!("\"{}\"", t.platform())), "{t:?}");
        for (_, you, everyone) in plugin_zips::folders(t) {
            assert!(site.contains(you) && site.contains(everyone), "{you}");
        }
    }
    // The effects the site calls plug-ins are the ones exported, and the page's slug is the
    // plug-in's.
    let data = read("web/src/units_data.rs");
    let mut file = None;
    let mut on_site = HashSet::new();
    for line in data.lines() {
        let l = line.trim();
        if let Some(f) = l.strip_prefix("file: \"") {
            file = f.split('"').next().map(str::to_string);
        }
        if l == "plugin: true," {
            on_site.insert(file.clone().expect("a unit's file before its plug-in flag"));
        }
    }
    let exported: HashSet<String> = desc::ALL.iter().map(|d| d.slug.to_string()).collect();
    assert_eq!(on_site, exported);
}
