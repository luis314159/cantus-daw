//! The real launcher, started as `cantus.exe` from a folder laid out like a release, with
//! copies of `cmd.exe` standing in for the two programs: which one starts, the arguments
//! passed through untouched, the exit code given back, and the refusal when none is there.

#![cfg(all(windows, target_arch = "x86_64"))]

use cantus_launch::Level;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

/// `cantus.exe` (the launcher) beside stand-ins for `programs`.
fn release(dir: &Path, programs: &[Level]) -> PathBuf {
    let launcher = dir.join("cantus.exe");
    std::fs::copy(env!("CARGO_BIN_EXE_cantus-launch"), &launcher).unwrap();
    let cmd =
        PathBuf::from(std::env::var("ComSpec").unwrap_or(r"C:\Windows\System32\cmd.exe".into()));
    for l in programs {
        std::fs::copy(&cmd, dir.join(l.program())).unwrap();
    }
    launcher
}

/// Run the launcher with `args`; the stand-in echoes the command line it was started with.
fn run(launcher: &Path, forced: Option<&str>, args: &[&str]) -> Output {
    let mut c = Command::new(launcher);
    c.args(args).env_remove(cantus_launch::FORCE_VAR);
    if let Some(f) = forced {
        c.env(cantus_launch::FORCE_VAR, f);
    }
    c.output().expect("run the launcher")
}

fn text(b: &[u8]) -> String {
    String::from_utf8_lossy(b).into_owned()
}

#[test]
fn the_launcher_starts_this_processors_program_with_the_same_arguments() {
    let top = cantus_launch::cpu_level(&cantus_launch::detected).expect("a v2 machine");
    let tmp = tempfile::tempdir().unwrap();
    let launcher = release(tmp.path(), &[Level::V2, Level::V3]);
    let expected = top.min(Level::V3);

    // `cmd /C echo %CMDCMDLINE%` prints the command line cmd was started with: the
    // program's own path, and the arguments exactly as the launcher received them.
    let out = run(
        &launcher,
        None,
        &["/C", "echo", "%CMDCMDLINE%", "song.cantus"],
    );
    assert!(out.status.success(), "{}", text(&out.stderr));
    let line = text(&out.stdout);
    assert!(line.contains(&expected.program()), "{line}");
    assert!(line.contains("song.cantus"), "{line}");

    // The exit code comes back, all 32 bits of it.
    let out = run(&launcher, None, &["/C", "exit", "7"]);
    assert_eq!(out.status.code(), Some(7));
    let out = run(&launcher, None, &["/C", "exit", "300"]);
    assert_eq!(out.status.code(), Some(300));

    // CANTUS_CPU=v2 steps down.
    let out = run(&launcher, Some("v2"), &["/C", "echo", "%CMDCMDLINE%"]);
    assert!(text(&out.stdout).contains(&Level::V2.program()));

    // --launcher-info is the launcher's own.
    let out = run(&launcher, None, &["--launcher-info"]);
    let info = text(&out.stdout);
    assert!(info.contains(&format!("processor: {top}")), "{info}");
    assert!(
        info.contains(&format!(
            "starts: {}",
            tmp.path().join(expected.program()).display()
        )),
        "{info}"
    );
}

#[test]
fn without_a_program_the_launcher_reports_that_nothing_starts() {
    let tmp = tempfile::tempdir().unwrap();
    let launcher = release(tmp.path(), &[]);
    // Starting would show the refusal's message box, which a test cannot dismiss; the
    // report says the same thing without starting (`refusal_text` is tested in choose.rs).
    let out = run(&launcher, None, &["--launcher-info"]);
    assert!(
        text(&out.stdout).contains("starts nothing"),
        "{}",
        text(&out.stdout)
    );
}
