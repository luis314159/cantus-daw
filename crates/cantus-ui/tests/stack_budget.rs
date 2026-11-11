//! The boot path fits Windows' main thread (round 39's start-up crash).
//!
//! The integrated tree's release build overflowed the 1 MB main thread at start (`thread
//! 'main' has overflowed its stack`): the processor and the application were large values
//! built and moved by value in frames the release build inlines into one another. Tests run
//! on 2 MB threads, so the suite never saw it. This assembles the application as the boot's
//! last stage does, on a thread with less than Windows' main-thread stack (the rest is the
//! window's own frames), and holds the application's size; the engine's constructors are
//! held in `cantus-engine/tests/stack_budget.rs`. Its own test binary: it points
//! `CANTUS_DATA_DIR` at a temporary folder.

use cantus_engine::EngineHandle;
use cantus_ui::app::Files;
use cantus_ui::CantusApp;

/// Windows' main thread is 1 MB; the window's event loop and the splash's frame take some.
const STACK: usize = 768 << 10;

#[test]
fn the_boot_assembles_the_application_within_the_main_threads_stack() {
    let data = tempfile::tempdir().unwrap();
    std::env::set_var("CANTUS_DATA_DIR", data.path());
    let dir = data.path().to_path_buf();
    let stack = std::env::var("CANTUS_STACK_PROBE")
        .ok()
        .and_then(|v| v.parse::<usize>().ok())
        .map_or(STACK, |k| k << 10);
    std::thread::Builder::new()
        .name("main-sized".into())
        .stack_size(stack)
        .spawn(move || {
            cantus_ui::i18n::set_thread_locale(cantus_ui::i18n::Locale::En);
            let files = Files::read(&dir, Some(cantus_ui::i18n::Locale::En));
            let ctx = egui::Context::default();
            // As the boot's assemble stage: the start-up song, the application boxed.
            let app = Box::new(CantusApp::assemble(
                &ctx,
                files,
                Some(cantus_ui::demos::startup_project()),
                EngineHandle::dummy(),
                None,
                None,
                None,
                false,
            ));
            drop(app);
        })
        .expect("spawn")
        .join()
        .expect("assembling overflowed the stack");
}

#[test]
fn the_application_is_small_to_move() {
    let app = std::mem::size_of::<CantusApp>();
    eprintln!("CantusApp {app} B");
    // 107 KB at round 39: built boxed, in a frame of its own (`Boot::assemble`).
    assert!(app <= 128 << 10, "CantusApp is {app} B inline");
}
