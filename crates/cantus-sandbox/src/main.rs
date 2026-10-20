//! `cantus-sandbox`: the helper process that hosts third-party plug-ins out of Cantus's own
//! process (round 29, ADR 0063), so one that crashes or hangs takes this process down and
//! never the session.
//!
//! Cantus starts one per group of isolated plug-ins (*Together*, *By vendor*,
//! *Individually*) as `cantus-sandbox --serve <protocol> --crt-fma3 on|off --group <key>
//! --lobby <name>`,
//! talks to it on its standard pipes (`cantus_host::bridge::proto`) and hands it each
//! block of audio through shared memory (`cantus_host::bridge::slot`). Inside, the plug-ins
//! run under `cantus-host`'s own CLAP and VST3 backends, unchanged:
//!
//! - the **main thread** ([`serve`]) loads them, answers Cantus's calls, pumps the
//!   plug-ins' windows (and COM, for VST3 editors), forwards their requests and changed
//!   values, and says it is alive every 200 ms;
//! - a small pool of **processing threads** ([`worker`]) serves every instance through the
//!   lobby Cantus made for this helper — a short spin after each burst of jobs, then parked
//!   on the lobby's bell — runs the plug-ins' audio halves with the same floating-point
//!   environment as Cantus's thread, and reports what each took.
//!
//! The plug-ins' own printing goes nowhere (the standard handles are moved off the
//! protocol's pipes first), a crash ends the process at once without the system's error
//! dialog, and the process ends when Cantus does: its input closes, and on Windows Cantus's
//! job object ends it, on Linux the kernel's parent-death signal.

mod platform;
mod serve;
mod worker;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let arg = |name: &str| {
        args.iter()
            .position(|a| a == name)
            .and_then(|i| args.get(i + 1))
            .cloned()
    };
    let Some(protocol) = arg("--serve").and_then(|p| p.parse::<u32>().ok()) else {
        eprintln!(
            "cantus-sandbox hosts plug-ins for Cantus; it is started by Cantus, not by hand."
        );
        std::process::exit(2);
    };
    // Another build's Cantus: served anyway, so its handshake reads this helper's protocol
    // and says which (review #40), instead of seeing a process that ended.
    let _ = protocol;
    platform::quiet_crashes();
    platform::die_with_parent();
    let pipes = platform::take_pipes();
    // The same floating-point world as Cantus's own process, so a plug-in renders the same
    // bits here as it would there (ADR 0031 R9).
    match arg("--crt-fma3").as_deref() {
        Some("off") => {
            cantus_rt::init_process();
        }
        _ => {
            cantus_rt::thread::opt_out_of_power_throttling();
        }
    }
    platform::init_com();
    let group = arg("--group").unwrap_or_default();
    let lobby = arg("--lobby").unwrap_or_default();
    let code = serve::run(pipes, &group, &lobby);
    std::process::exit(code);
}
