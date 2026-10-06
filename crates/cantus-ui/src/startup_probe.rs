//! Start-up, measured from inside (round 20, `docs/perf/benchmark.md` §2.6). With
//! `CANTUS_STARTUP_REPORT=<file>` in the environment, the application is wrapped in a
//! probe that writes, once the main window has drawn its second frame, how long each
//! stage took since `run` began — then closes. The benchmark starts the real executable
//! with `LOCALAPPDATA` (and `XDG_DATA_HOME`/`HOME`) pointed at a fresh folder, so the
//! user's settings are neither read nor written and the start is a first start.
//!
//! Without the variable nothing here runs.

use crate::boot::Shell;
use std::path::PathBuf;
use std::time::Instant;

/// The variable that switches the probe on; its value is where the report goes.
pub const VAR: &str = "CANTUS_STARTUP_REPORT";

pub struct Probe {
    inner: Shell,
    started: Instant,
    path: PathBuf,
    /// Milliseconds since `run` began: the splash's first frame built, the application
    /// assembled (its first frame), its second frame (everything settled once).
    splash_ms: Option<f64>,
    app_ms: Option<f64>,
    frames: u32,
}

impl Probe {
    pub fn new(inner: Shell, started: Instant, path: PathBuf) -> Probe {
        Probe {
            inner,
            started,
            path,
            splash_ms: None,
            app_ms: None,
            frames: 0,
        }
    }

    fn ms(&self) -> f64 {
        self.started.elapsed().as_secs_f64() * 1e3
    }
}

impl eframe::App for Probe {
    fn ui(&mut self, root: &mut egui::Ui, frame: &mut eframe::Frame) {
        self.inner.ui(root, frame);
        if self.splash_ms.is_none() {
            self.splash_ms = Some(self.ms());
        }
        if matches!(self.inner, Shell::Running(_)) {
            self.frames += 1;
            if self.frames == 1 {
                self.app_ms = Some(self.ms());
            }
            if self.frames == 2 {
                let text = format!(
                    "{{\"splash_ms\": {:.1}, \"app_ms\": {:.1}, \"second_frame_ms\": {:.1}}}\n",
                    self.splash_ms.unwrap_or(0.0),
                    self.app_ms.unwrap_or(0.0),
                    self.ms()
                );
                let _ = std::fs::write(&self.path, text);
                root.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
            }
            root.ctx().request_repaint();
        }
    }
}
