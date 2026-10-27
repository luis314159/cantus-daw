//! Round 31: a test that picks a language picks it for its own thread. Each test runs on a
//! thread of its own; the process's language (`i18n::set_locale`, the application's) is
//! what every thread that never chose reads, so one test writing it made another test's
//! English text come out Spanish now and then (`tempo_review6_ui`, on Windows and Linux).

use cantus_ui::i18n::{self, Locale};

/// Choosing for a thread never reaches another thread.
#[test]
fn a_threads_language_stays_its_own() {
    let before = std::thread::spawn(i18n::locale).join().unwrap();
    let switcher = std::thread::spawn(|| {
        for k in 0..2_000 {
            i18n::set_thread_locale(if k % 2 == 0 { Locale::Es } else { Locale::En });
        }
        i18n::set_thread_locale(Locale::Es);
        i18n::locale()
    });
    let mut seen = Vec::new();
    for _ in 0..2_000 {
        seen.push(std::thread::spawn(i18n::locale).join().unwrap());
    }
    assert_eq!(switcher.join().unwrap(), Locale::Es, "the switcher's own");
    assert!(
        seen.iter().all(|l| *l == before),
        "another thread's language moved"
    );
}

/// The places that may write the process's language (`i18n::set_locale`): the
/// application's own (its boot, `Files::read`, the verse's launch read, `CantusApp::set_locale`
/// behind *Settings*), a plug-in editor in another host's process, and the bench tools.
const WRITERS: &[&str] = &[
    "cantus-ui/src/i18n.rs",
    "cantus-ui/src/app.rs",
    "cantus-ui/src/lib.rs",
    "cantus-ui/src/verse.rs",
    "cantus-plugins/src/editor.rs",
    "cantus-bench/src/",
];

/// Whether a line calls the process-wide `set_locale` (not `set_thread_locale`, not the
/// app's `CantusApp::set_locale` method, not its definition) or imports it under any name.
fn writes_process_language(line: &str) -> bool {
    let code = line.split("//").next().unwrap_or("");
    let bytes = code.as_bytes();
    let call = code.match_indices("set_locale(").any(|(i, _)| {
        let before = if i == 0 { b' ' } else { bytes[i - 1] };
        before != b'.' && before != b'_' && !code[..i].trim_end().ends_with("fn")
    });
    let import = code.trim_start().starts_with("use ")
        && code.contains("i18n")
        && code
            .split(|c: char| !c.is_alphanumeric() && c != '_')
            .any(|w| w == "set_locale");
    call || import
}

/// No test — of any crate, in `tests/` or in a lib's `#[cfg(test)]` modules — nor any other
/// code but [`WRITERS`] writes the process's language: tests use `set_thread_locale` (the
/// application's own switch, `CantusApp::set_locale`, stays testable — it is the user's
/// path). Every `src/` and `tests/` file of every crate is read, an aliased import caught.
#[test]
fn no_test_writes_the_process_language() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    let mut found = Vec::new();
    for krate in std::fs::read_dir(&root).unwrap().flatten() {
        let mut stack = vec![krate.path().join("src"), krate.path().join("tests")];
        while let Some(dir) = stack.pop() {
            let Ok(entries) = std::fs::read_dir(&dir) else {
                continue;
            };
            for e in entries.flatten() {
                let p = e.path();
                if p.is_dir() {
                    stack.push(p);
                    continue;
                }
                if p.extension().is_none_or(|x| x != "rs") || p.ends_with("locale_isolation.rs") {
                    continue;
                }
                let shown = p.to_string_lossy().replace(char::from(92), "/");
                if WRITERS.iter().any(|w| shown.contains(w)) {
                    continue;
                }
                let text = std::fs::read_to_string(&p).unwrap_or_default();
                for (n, line) in text.lines().enumerate() {
                    if writes_process_language(line) {
                        found.push(format!("{shown}:{}", n + 1));
                    }
                }
            }
        }
    }
    assert!(
        found.is_empty(),
        "the process's language written at: {found:#?}"
    );
}

/// The scan's own reading: what it catches and what it lets by.
#[test]
fn the_scan_tells_a_process_write_from_a_threads() {
    for caught in [
        "    i18n::set_locale(Locale::Es);",
        "    crate::i18n::set_locale(l);",
        "    set_locale(Locale::En);",
        "    use cantus_ui::i18n::{set_locale, t};",
        "    use crate::i18n::{t, set_locale as pick};",
    ] {
        assert!(writes_process_language(caught), "{caught}");
    }
    for let_by in [
        "    i18n::set_thread_locale(Locale::Es);",
        "    app.set_locale(Locale::Es);",
        "    pub fn set_locale(&mut self, locale: Locale) {",
        "    use cantus_ui::i18n::{set_thread_locale, t};",
        "    // i18n::set_locale(x) in a comment",
    ] {
        assert!(!writes_process_language(let_by), "{let_by}");
    }
}
