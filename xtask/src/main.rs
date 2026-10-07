//! Cantus's build tasks.
//!
//! ```text
//! cargo run -p xtask -- bundle                  # all seven, CLAP and VST3, into target/bundled/
//! cargo run -p xtask -- bundle porta terminus   # only those
//! cargo run -p xtask -- bundle --v3             # the x86-64-v3 flavour, into target/bundled-x86-64-v3/
//! cargo run -p xtask -- bundle --no-build       # only lay out what is already built
//! cargo run -p xtask -- package                 # the Windows release files (package.rs)
//! cargo run -p xtask -- package --v4            # with the x86-64-v4 (AVX-512) program too
//! cargo run -p xtask -- null-hashes             # v2 and v3 render the same bits (null_hashes.rs)
//! cargo run -p xtask -- asio-sdk                # fetch and check the ASIO SDK (asio_sdk.rs)
//! cargo run -p xtask -- sign-test-cert          # a self-signed certificate (sign.rs)
//! cargo run -p xtask -- version                 # the workspace's version
//! cargo run -p xtask -- perf                    # the performance gate (perf.rs)
//! cargo run -p xtask -- perf --ab v2,v3         # two levels' builds, in alternating runs
//! ```
//!
//! Each plug-in is built on its own (`cargo build -p cantus-plugins --profile plugin
//! --no-default-features --features <name>`), so each binary lists one plug-in, and then
//! laid out in each standard's bundle shape (ADR 0020):
//!
//! - `Cantus <Name>.clap` — on Windows and Linux the library itself, renamed; on macOS a
//!   bundle (`Contents/MacOS/Cantus <Name>`, `Info.plist`).
//! - `Cantus <Name>.vst3/Contents/x86_64-win/Cantus <Name>.vst3` (Windows),
//!   `…/Contents/x86_64-linux/Cantus <Name>.so` (Linux), `…/Contents/MacOS/Cantus <Name>`
//!   (macOS), with `Contents/Resources/moduleinfo.json` so a VST 3.7.5+ host can list it
//!   without loading it.

mod archive;
mod asio_sdk;
mod levels;
#[cfg(test)]
mod levels_tests;
mod nsis;
mod null_hashes;
mod package;
#[cfg(test)]
mod package_tests;
mod perf;
mod sign;

use cantus_plugins::desc::{self, Descriptor};
use std::path::{Path, PathBuf};
use std::process::Command;

/// The cargo features that compile each plug-in in (`crates/cantus-plugins/Cargo.toml`),
/// with its own editor where it has one.
fn feature(d: &Descriptor) -> &'static str {
    match d.slug {
        "aequalis" => "eq",
        "comprimo" => "compressor",
        "terminus" => "limiter",
        "spatium" => "space",
        "multiplex" => "multiband",
        "sibilus" => "deesser",
        "porta" => "gate,editor",
        "latitudo" => "stereo,editor",
        "culmen" => "maximizer,editor",
        "fervor" => "saturator,editor",
        other => panic!("no feature for {other}"),
    }
}

pub(crate) fn workspace() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("xtask sits in the workspace")
        .to_path_buf()
}

pub(crate) fn target_dir() -> PathBuf {
    std::env::var_os("CARGO_TARGET_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| workspace().join("target"))
}

/// The library cargo writes for the `plugin` profile.
fn built_library(target: &Path) -> PathBuf {
    let dir = target.join("plugin");
    if cfg!(windows) {
        dir.join("cantus_plugins.dll")
    } else if cfg!(target_os = "macos") {
        dir.join("libcantus_plugins.dylib")
    } else {
        dir.join("libcantus_plugins.so")
    }
}

pub(crate) fn run(cmd: &mut Command) {
    eprintln!("> {cmd:?}");
    let status = cmd.status().expect("run cargo");
    if !status.success() {
        eprintln!("failed: {status}");
        std::process::exit(1);
    }
}

pub(crate) fn copy(from: &Path, to: &Path) {
    if let Some(parent) = to.parent() {
        std::fs::create_dir_all(parent).expect("make the bundle's folders");
    }
    std::fs::copy(from, to)
        .unwrap_or_else(|e| panic!("copy {} to {}: {e}", from.display(), to.display()));
}

fn info_plist(d: &Descriptor, package: &str, id_suffix: &str) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleDevelopmentRegion</key><string>English</string>
  <key>CFBundleExecutable</key><string>{name}</string>
  <key>CFBundleIdentifier</key><string>{id}.{id_suffix}</string>
  <key>CFBundleName</key><string>{name}</string>
  <key>CFBundlePackageType</key><string>{package}</string>
  <key>CFBundleShortVersionString</key><string>{version}</string>
  <key>CFBundleVersion</key><string>{version}</string>
  <key>CFBundleSignature</key><string>????</string>
</dict>
</plist>
"#,
        name = d.full_name,
        id = d.clap_id,
        version = desc::VERSION,
    )
}

/// Lay out one plug-in's two bundles from its built library.
fn lay_out(d: &Descriptor, library: &Path, out: &Path) {
    let name = d.full_name;
    let clap = out.join(format!("{name}.clap"));
    let vst3 = out.join(format!("{name}.vst3"));
    let _ = std::fs::remove_dir_all(&vst3);
    let _ = std::fs::remove_file(&clap);
    let _ = std::fs::remove_dir_all(&clap);
    let contents = vst3.join("Contents");
    if cfg!(target_os = "macos") {
        copy(library, &clap.join("Contents/MacOS").join(name));
        std::fs::write(
            clap.join("Contents/Info.plist"),
            info_plist(d, "BNDL", "clap"),
        )
        .unwrap();
        std::fs::write(clap.join("Contents/PkgInfo"), "BNDL????").unwrap();
        copy(library, &contents.join("MacOS").join(name));
        std::fs::write(contents.join("Info.plist"), info_plist(d, "BNDL", "vst3")).unwrap();
        std::fs::write(contents.join("PkgInfo"), "BNDL????").unwrap();
    } else if cfg!(windows) {
        copy(library, &clap);
        copy(
            library,
            &contents.join("x86_64-win").join(format!("{name}.vst3")),
        );
    } else {
        copy(library, &clap);
        copy(
            library,
            &contents.join("x86_64-linux").join(format!("{name}.so")),
        );
    }
    let resources = contents.join("Resources");
    std::fs::create_dir_all(&resources).unwrap();
    std::fs::write(
        resources.join("moduleinfo.json"),
        cantus_plugins::vst3::moduleinfo_json(d),
    )
    .unwrap();
    eprintln!("  {}\n  {}", clap.display(), vst3.display());
}

pub(crate) fn bundle(args: &[String]) {
    let v3 = args.iter().any(|a| a == "--v3");
    let build = !args.iter().any(|a| a == "--no-build");
    let wanted: Vec<&str> = args
        .iter()
        .filter(|a| !a.starts_with("--"))
        .map(String::as_str)
        .collect();
    let plugins: Vec<&Descriptor> = desc::ALL
        .iter()
        .filter(|d| wanted.is_empty() || wanted.contains(&d.slug) || wanted.contains(&d.name))
        .collect();
    if plugins.is_empty() {
        eprintln!(
            "no such plug-in; the family is {}",
            desc::ALL
                .iter()
                .map(|d| d.slug)
                .collect::<Vec<_>>()
                .join(", ")
        );
        std::process::exit(2);
    }
    // The v3 flavour builds in a target of its own, so the two never invalidate each other.
    let target = if v3 {
        target_dir().join("plugin-x86-64-v3")
    } else {
        target_dir()
    };
    let out = target_dir().join(if v3 { "bundled-x86-64-v3" } else { "bundled" });
    std::fs::create_dir_all(&out).unwrap();
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".into());
    for d in plugins {
        eprintln!("{} ({})", d.full_name, feature(d));
        if build {
            let mut cmd = Command::new(&cargo);
            cmd.current_dir(workspace())
                .args([
                    "build",
                    "-p",
                    "cantus-plugins",
                    "--lib",
                    "--profile",
                    "plugin",
                ])
                .args(["--no-default-features", "--features", feature(d)])
                .env("CARGO_TARGET_DIR", &target);
            if v3 {
                let flags = std::env::var("RUSTFLAGS").unwrap_or_default();
                cmd.env(
                    "RUSTFLAGS",
                    format!("{flags} -C target-cpu=x86-64-v3").trim(),
                );
            }
            run(&mut cmd);
        }
        let library = built_library(&target);
        if !library.exists() {
            eprintln!("{} is not built", library.display());
            std::process::exit(1);
        }
        lay_out(d, &library, &out);
    }
    eprintln!("bundles in {}", out.display());
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("bundle") => bundle(&args[1..]),
        Some("package") => package::package(&args[1..]),
        Some("asio-sdk") => asio_sdk::command(),
        Some("sign-file") => package::sign_file(&args[1..]),
        Some("sign-test-cert") => sign::make_test_cert(&target_dir()),
        Some("version") => println!("{}", package::VERSION),
        Some("perf") => perf::perf(&args[1..]),
        Some("null-hashes") => null_hashes::null_hashes(&args[1..]),
        _ => {
            eprintln!("usage: cargo run -p xtask -- bundle [names…] [--v3] [--no-build]");
            eprintln!("       cargo run -p xtask -- package [--no-build] [--no-installer] [--sign|--sign-test] [--v4]");
            eprintln!("       cargo run -p xtask -- null-hashes [--levels v2,v3,v4] [--baseline] [--no-build]");
            eprintln!("       cargo run -p xtask -- asio-sdk");
            eprintln!("       cargo run -p xtask -- sign-test-cert | sign-file <path> | version");
            eprintln!("       cargo run -p xtask -- perf [--full] [--save] [--only …] [--cpu v3] [--ab v2,v3 --pairs N]");
            std::process::exit(2);
        }
    }
}
