//! **The repaint log** (round 38, the scorecard): with `CANTUS_REPAINT_LOG` naming a file,
//! every frame of the main window writes one line to it — the seconds since start, how many
//! input events the frame had, the ticker's reasons and what it asked for, and where in the
//! code egui was asked for this frame (`Context::repaint_causes`). What keeps a window
//! drawing at rest is then read, not guessed. Nothing is done when the variable is unset.

use std::io::Write;
use std::sync::Mutex;

/// The variable that names the log's file.
pub const VAR: &str = "CANTUS_REPAINT_LOG";

static LOG: Mutex<Option<std::io::BufWriter<std::fs::File>>> = Mutex::new(None);
static OPENED: std::sync::Once = std::sync::Once::new();

/// One line for this frame (after the ticker's `finish`).
pub fn frame(ctx: &egui::Context) {
    OPENED.call_once(|| {
        if let Some(path) = std::env::var_os(VAR).filter(|p| !p.is_empty()) {
            if let Ok(f) = std::fs::File::create(path) {
                if let Ok(mut l) = LOG.lock() {
                    *l = Some(std::io::BufWriter::new(f));
                }
            }
        }
    });
    let Ok(mut log) = LOG.lock() else {
        return;
    };
    let Some(w) = log.as_mut() else {
        return;
    };
    let tick = crate::views::workspace_ticker::last(ctx);
    let events = ctx.input(|i| i.events.len());
    let causes: Vec<String> = ctx
        .repaint_causes()
        .iter()
        .map(|c| format!("{}:{}", c.file, c.line))
        .collect();
    let _ = writeln!(
        w,
        "{:.3}\tev={events}\treasons={:#x}\twake={:?}\tasked={:?}\t{}",
        crate::dance_live::wall_seconds(),
        tick.reasons,
        tick.wake,
        tick.requested,
        causes.join(" ")
    );
    let _ = w.flush();
}
