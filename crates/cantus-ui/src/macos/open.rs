//! Projects macOS asks Cantus to open — a `.cantus` double-clicked in Finder or dropped on
//! the Dock icon (round 30, ADR 0066). `cantus_launch::macopen` receives the Apple Event
//! and queues the paths; this takes them: at start, as the project to open (the same as a
//! path on the command line), and each frame after that, through *Save changes?* as
//! *Open* does ([`CantusApp::request_open_path`]). Plain Rust: tested on every system.

use crate::app::CantusApp;
use std::path::{Path, PathBuf};

fn is_project(p: &Path) -> bool {
    p.extension()
        .is_some_and(|e| e.eq_ignore_ascii_case(cantus_project::FILE_EXTENSION))
}

/// The project to open among `paths`: the last `.cantus` (several at once: the one chosen
/// last; Cantus holds one project).
pub fn project_of(paths: &[PathBuf]) -> Option<PathBuf> {
    paths.iter().rev().find(|p| is_project(p)).cloned()
}

/// At start: a project macOS asked for, unless the command line names one.
pub fn at_launch(project: &mut Option<PathBuf>) {
    if project.is_none() {
        *project = project_of(&cantus_launch::macopen::take());
    }
}

/// Each frame of the running app: a project asked for since, opened as *Open* opens one.
pub fn serve(app: &mut CantusApp, ctx: &egui::Context) {
    thread_local! {
        static WAKE_SET: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    }
    if !WAKE_SET.with(|w| w.replace(true)) {
        let wake = ctx.clone();
        cantus_launch::macopen::set_wake(Box::new(move || wake.request_repaint()));
    }
    if let Some(p) = project_of(&cantus_launch::macopen::take()) {
        app.request_open_path(p);
    }
}
