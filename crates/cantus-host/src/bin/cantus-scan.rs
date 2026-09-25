//! `cantus-scan <format> <file>`: read one plug-in file and print what it holds.
//!
//! Run by Cantus, one process per file (`cantus_host::scan::Scanner`), so a plug-in that
//! crashes or hangs while it loads takes this process down and not the application. The
//! answer is the last line that starts with `CANTUS-SCAN-RESULT `; anything the plug-in
//! prints before it is ignored.

fn main() {
    quiet_crashes();
    let mut args = std::env::args().skip(1);
    let (Some(format), Some(path)) = (args.next(), args.next()) else {
        eprintln!("usage: cantus-scan <clap|vst3> <file>");
        std::process::exit(2);
    };
    let output = match cantus_host::scan::parse_format(&format).and_then(cantus_host::backend_for) {
        None => cantus_host::scan::ScanOutput::Error(format!("format {format} is not hosted")),
        Some(backend) => match backend.scan(std::path::Path::new(&path)) {
            Ok(list) => cantus_host::scan::ScanOutput::Plugins(list),
            Err(e) => cantus_host::scan::ScanOutput::Error(e.to_string()),
        },
    };
    // The plug-in has been unloaded by now: one that crashes on its way out is blocked,
    // as it would crash Cantus at the same point.
    println!();
    println!("{}", output.to_line());
}

/// A crash in a plug-in ends this process at once, without the system's error dialog
/// waiting for someone to click it.
#[cfg(windows)]
fn quiet_crashes() {
    use windows_sys::Win32::System::Diagnostics::Debug::{
        SetErrorMode, SEM_FAILCRITICALERRORS, SEM_NOGPFAULTERRORBOX, SEM_NOOPENFILEERRORBOX,
    };
    // SAFETY: sets this process's error mode; no pointers involved.
    unsafe {
        SetErrorMode(SEM_FAILCRITICALERRORS | SEM_NOGPFAULTERRORBOX | SEM_NOOPENFILEERRORBOX);
    }
}

#[cfg(not(windows))]
fn quiet_crashes() {}
