//! `cantus-ml`: the helper process that runs Cantus's machine-learning models (round 34,
//! ADR 0085), so a model that crashes, hangs or fails on a graphics driver ends this
//! process and never the session.
//!
//! Cantus starts it as `cantus-ml --serve <protocol>` when a job needs it and talks to it
//! on its standard pipes (`cantus_ml::proto`): jobs started, their progress, their end,
//! a cancel at any time. The process ends when Cantus does: its input closes, and on
//! Windows Cantus's job object ends it, on Linux the kernel's parent-death signal.

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let serve = args
        .iter()
        .position(|a| a == "--serve")
        .and_then(|i| args.get(i + 1))
        .and_then(|p| p.parse::<u32>().ok());
    if serve.is_none() {
        eprintln!("cantus-ml runs models for Cantus; it is started by Cantus, not by hand.");
        std::process::exit(2);
    }
    // ONNX Runtime's official builds send usage events to Microsoft unless this is set
    // before the library loads (ADR 0085). The client sets it too.
    std::env::set_var("ORT_DISABLE_TELEMETRY", "1");
    // Cantus's tests: a helper slow to start, as one an antivirus checks on its first run.
    if let Some(ms) = std::env::var("CANTUS_ML_START_DELAY_MS")
        .ok()
        .and_then(|v| v.parse::<u64>().ok())
    {
        std::thread::sleep(std::time::Duration::from_millis(ms.min(60_000)));
    }
    cantus_ml::platform::quiet_crashes();
    cantus_ml::platform::die_with_parent();
    let pipes = cantus_ml::platform::take_pipes();
    let test_hooks = std::env::var("CANTUS_ML_TEST_HOOKS").as_deref() == Ok("1");
    std::process::exit(cantus_ml::helper::serve(pipes, test_hooks));
}
