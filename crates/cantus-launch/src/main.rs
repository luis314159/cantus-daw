//! `cantus.exe` in a release: starts the build of Cantus made for this processor
//! (`cantus_launch::choose`) with the same arguments, waits for it and returns its exit
//! code — so the Start menu, the `.cantus` association, the installer's "run Cantus" and
//! a script piping `cantus --audio-drivers` all behave as if they ran the program itself,
//! and the installer's wait for `cantus.exe` to close covers the program too.
//!
//! `CANTUS_CPU=v2` (or `v3`, `v4`) asks for a level; `cantus --launcher-info` prints what
//! the processor has and what would start. When nothing can start — a processor below
//! x86-64-v2, a program missing — it says why in a message box, in English and Spanish.

#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]
#![deny(unsafe_code)]

use cantus_launch::{choose, detected, info, refusal_text, start_failed_text, FORCE_VAR};
use std::path::PathBuf;
use std::process::Command;

fn main() {
    let me = std::env::current_exe().unwrap_or_else(|_| PathBuf::from("cantus"));
    let dir = me
        .parent()
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));
    let args: Vec<std::ffi::OsString> = std::env::args_os().skip(1).collect();
    let forced = std::env::var(FORCE_VAR).ok();

    if args.first().is_some_and(|a| a == "--launcher-info") {
        print!("{}", info(&dir, &detected, forced.as_deref()));
        return;
    }
    let choice = match choose(&dir, &detected, &|p| p.is_file(), forced.as_deref()) {
        Ok(c) => c,
        Err(r) => fail(&refusal_text(&r)),
    };
    if let Some(note) = &choice.note {
        eprintln!("cantus launcher: {note}");
    }
    match Command::new(&choice.program).args(&args).status() {
        // Windows' exit codes are 32 bits; `ExitCode` would cut them to 8.
        Ok(status) => std::process::exit(status.code().unwrap_or(1)),
        Err(e) => fail(&start_failed_text(&choice.program, &e)),
    }
}

/// Say why Cantus cannot start — on the console when there is one, and in a message box,
/// since the released launcher has no console — and stop.
fn fail(text: &str) -> ! {
    eprintln!("{text}");
    message_box(text);
    std::process::exit(3)
}

#[cfg(windows)]
#[allow(unsafe_code)]
fn message_box(text: &str) {
    use windows_sys::Win32::UI::WindowsAndMessaging::{MessageBoxW, MB_ICONERROR, MB_OK};
    let wide = |s: &str| s.encode_utf16().chain([0]).collect::<Vec<u16>>();
    let (text, title) = (wide(text), wide("Cantus"));
    // SAFETY: both pointers are NUL-terminated UTF-16 buffers that outlive the call; a null
    // owner window is allowed.
    unsafe {
        MessageBoxW(
            std::ptr::null_mut(),
            text.as_ptr(),
            title.as_ptr(),
            MB_OK | MB_ICONERROR,
        );
    }
}

#[cfg(not(windows))]
fn message_box(_text: &str) {}
