//! **The window's watchdog** (GitHub issue #1, ADR 0146 — provisional number, renumber at
//! the merge). A user's window "kept flickering along both axes" on a machine we do not
//! have; the cause found (the screen-following scale measuring itself, `screen_scale`) is
//! fixed, and this watch is what catches the next loop of its kind on a machine we will
//! never see.
//!
//! Every frame the main window's state — its inner and outer rectangles, the system's
//! scale, our zoom, maximised and full screen, the monitor — is compared with the frame
//! before. A change no input asked for that **returns** the window to a state it had a
//! moment ago (A → B → A) is the signature of a loop: a user dragging a border makes new
//! sizes, not the same two again and again. [`TRIP_RETURNS`] such returns within
//! [`TRIP_WINDOW_S`] trip the watch: the transitions go to Cantus's log (`log::warn!`;
//! `logs\cantus.log` in the data folder, `crates/cantus-app`), and the caller breaks the
//! loop — the screen-following scale stops, the zoom in effect stays, and geometry
//! commands sent through [`send_geometry`] are dropped for the rest of the session.
//!
//! Cheap enough to be always on: a few comparisons a frame, fixed arrays, nothing
//! allocated until it has something to write. With `CANTUS_WINDOW_TRACE=1` every change
//! is logged too, and every scale the screen-following decides.

use egui::{Rect, Vec2};

/// The variable that writes every change of the window's state to the log, not only a
/// tripped watch.
pub const VAR_TRACE: &str = "CANTUS_WINDOW_TRACE";
/// Returns to an earlier state, unasked for, that trip the watch…
pub const TRIP_RETURNS: usize = 6;
/// …within this many seconds.
pub const TRIP_WINDOW_S: f64 = 1.0;
/// How many earlier states a return is looked for in.
const RECENT: usize = 4;
/// How many transitions the log keeps for its report.
const TRAIL: usize = 16;

/// The main window's state in one frame, as egui-winit reported it.
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct Sample {
    /// Inner and outer rectangles in points.
    pub inner: Option<Rect>,
    pub outer: Option<Rect>,
    /// The system's scale (`native_pixels_per_point`) and our zoom.
    pub native_ppp: Option<f32>,
    pub zoom: f32,
    pub maximized: Option<bool>,
    pub fullscreen: Option<bool>,
    /// The monitor's size in points.
    pub monitor: Option<Vec2>,
}

impl Sample {
    /// This frame's, from the context.
    pub fn of(ctx: &egui::Context) -> Sample {
        let zoom = ctx.zoom_factor();
        ctx.input(|i| {
            let v = i.viewport();
            Sample {
                inner: v.inner_rect,
                outer: v.outer_rect,
                native_ppp: v.native_pixels_per_point,
                zoom,
                maximized: v.maximized,
                fullscreen: v.fullscreen,
                monitor: v.monitor_size,
            }
        })
    }

    /// The inner size in physical pixels: what the user sees jump.
    fn inner_px(&self) -> Option<Vec2> {
        let ppp = self.zoom * self.native_ppp.unwrap_or(1.0);
        self.inner.map(|r| r.size() * ppp)
    }

    fn line(&self, at: f64, prompted: bool, out: &mut String) {
        use std::fmt::Write as _;
        let px = self.inner_px();
        let _ = write!(
            out,
            "t={at:.3}s zoom={:.3} native={:?} inner_px={:?} inner={:?} outer={:?} maximized={:?} fullscreen={:?} monitor={:?}{}",
            self.zoom,
            self.native_ppp,
            px.map(|v| (v.x.round(), v.y.round())),
            self.inner.map(|r| (r.min.x, r.min.y, r.width(), r.height())),
            self.outer.map(|r| (r.min.x, r.min.y, r.width(), r.height())),
            self.maximized,
            self.fullscreen,
            self.monitor.map(|v| (v.x, v.y)),
            if prompted { " (input)" } else { "" },
        );
    }
}

/// What one frame's observation found.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Verdict {
    /// Nothing changed.
    Quiet,
    /// The state changed (asked for, or new).
    Changed,
    /// The state went back to one it had a moment ago, with no input asking.
    Returned,
    /// This frame tripped the watch: the loop must be broken.
    Tripped,
}

/// The watch itself. One for the main window.
#[derive(Debug)]
pub struct Watch {
    last: Option<Sample>,
    /// Earlier distinct states, newest first (the last one not among them).
    recent: [Option<Sample>; RECENT],
    /// When the latest returns happened (a ring).
    returns: [f64; TRIP_RETURNS],
    returns_head: usize,
    returns_len: usize,
    /// The latest transitions (a ring), for the report.
    trail: [Option<(f64, Sample, bool)>; TRAIL],
    trail_head: usize,
    tripped_at: Option<f64>,
    trace: bool,
}

impl Default for Watch {
    fn default() -> Watch {
        Watch::new(false)
    }
}

impl Watch {
    pub fn new(trace: bool) -> Watch {
        Watch {
            last: None,
            recent: [None; RECENT],
            returns: [0.0; TRIP_RETURNS],
            returns_head: 0,
            returns_len: 0,
            trail: [None; TRAIL],
            trail_head: 0,
            tripped_at: None,
            trace,
        }
    }

    /// The watch the desktop's window has: tracing when `CANTUS_WINDOW_TRACE` is set.
    pub fn for_window() -> Watch {
        Watch::new(trace_asked())
    }

    pub fn tracing(&self) -> bool {
        self.trace
    }

    pub fn tripped(&self) -> bool {
        self.tripped_at.is_some()
    }

    /// One frame's state at `now` seconds; `prompted` when the frame had input that may
    /// have changed the window (a click, a key, the wheel). Returns what it found; the
    /// caller breaks the loop on [`Verdict::Tripped`].
    pub fn observe(&mut self, now: f64, s: Sample, prompted: bool) -> Verdict {
        let Some(last) = self.last else {
            self.last = Some(s);
            self.push_trail(now, s, prompted);
            self.trace_line(now, s, prompted);
            return Verdict::Changed;
        };
        if last == s {
            return Verdict::Quiet;
        }
        let returned = self.recent.iter().flatten().any(|r| *r == s);
        // The last state joins the recent ones (newest first, no duplicates).
        if !self.recent.iter().flatten().any(|r| *r == last) {
            self.recent.rotate_right(1);
            self.recent[0] = Some(last);
        }
        self.last = Some(s);
        self.push_trail(now, s, prompted);
        self.trace_line(now, s, prompted);
        if !returned || prompted {
            return Verdict::Changed;
        }
        self.returns[self.returns_head] = now;
        self.returns_head = (self.returns_head + 1) % TRIP_RETURNS;
        self.returns_len = (self.returns_len + 1).min(TRIP_RETURNS);
        if self.tripped_at.is_some() || self.returns_len < TRIP_RETURNS {
            return Verdict::Returned;
        }
        // The oldest of the ring is the one about to be overwritten.
        let oldest = self.returns[self.returns_head];
        if now - oldest > TRIP_WINDOW_S {
            return Verdict::Returned;
        }
        self.tripped_at = Some(now);
        log::warn!("{}", self.report());
        Verdict::Tripped
    }

    fn push_trail(&mut self, now: f64, s: Sample, prompted: bool) {
        self.trail[self.trail_head] = Some((now, s, prompted));
        self.trail_head = (self.trail_head + 1) % TRAIL;
    }

    fn trace_line(&mut self, now: f64, s: Sample, prompted: bool) {
        if self.trace {
            let mut line = String::from("window: ");
            s.line(now, prompted, &mut line);
            log::info!("{line}");
        }
    }

    /// A line for the trace (the screen-following scale's decisions); nothing otherwise.
    pub fn note(&self, what: std::fmt::Arguments<'_>) {
        if self.trace {
            log::info!("window: {what}");
        }
    }

    /// The trip's report: what returned how often, and the latest transitions, oldest
    /// first.
    pub fn report(&self) -> String {
        let mut out = format!(
            "window watchdog: the window's state went back and forth {TRIP_RETURNS} times within {TRIP_WINDOW_S} s with no input; the screen-following scale and further window-geometry commands are stopped for this session. Latest transitions:"
        );
        for i in 0..TRAIL {
            if let Some((at, s, prompted)) = self.trail[(self.trail_head + i) % TRAIL] {
                out.push_str("\n  ");
                s.line(at, prompted, &mut out);
            }
        }
        out
    }
}

/// Whether `CANTUS_WINDOW_TRACE` asks for every change of the window to be logged.
pub fn trace_asked() -> bool {
    std::env::var_os(VAR_TRACE).is_some_and(|v| !v.is_empty() && v != "0")
}

/// Cantus's log file: `<data folder>/logs/cantus.log` — beside the program in `data\logs\`
/// for the portable copy, `%LOCALAPPDATA%\Cantus\logs\` once installed, the XDG data
/// folder's `logs/` on Linux. A release on Windows has no console, so this file is where a
/// user's report finds what the log said (issue #1).
pub fn log_path(data_dir: &std::path::Path) -> std::path::PathBuf {
    data_dir.join("logs").join("cantus.log")
}

/// This start's log file, the last start's kept beside it as `cantus.previous.log`;
/// `None` when the folder cannot be written (the log then goes to the console alone).
pub fn open_log(data_dir: &std::path::Path) -> Option<std::fs::File> {
    let path = log_path(data_dir);
    std::fs::create_dir_all(path.parent()?).ok()?;
    if path.is_file() {
        let _ = std::fs::rename(&path, path.with_file_name("cantus.previous.log"));
    }
    std::fs::File::create(&path).ok()
}

/// Whether this frame's input may have moved or resized the window: a pointer button, a
/// key, the wheel, a zoom gesture, a file dropped, the window's own close or focus.
pub fn prompted(ctx: &egui::Context) -> bool {
    ctx.input(|i| {
        i.events.iter().any(|e| {
            matches!(
                e,
                egui::Event::PointerButton { .. }
                    | egui::Event::Key { .. }
                    | egui::Event::MouseWheel { .. }
                    | egui::Event::Zoom(_)
                    | egui::Event::WindowFocused(_)
            )
        }) || !i.raw.dropped_files.is_empty()
    })
}

/// A command that moves, sizes, maximises or decorates the window, sent unless the watch
/// has tripped (then dropped: the loop it may be part of is broken).
pub fn send_geometry(ctx: &egui::Context, watch: &Watch, cmd: egui::ViewportCommand) -> bool {
    if watch.tripped() {
        log::warn!("window watchdog: dropped {cmd:?}");
        return false;
    }
    ctx.send_viewport_cmd(cmd);
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui::{pos2, vec2};

    fn at(zoom: f32, w: f32) -> Sample {
        Sample {
            inner: Some(Rect::from_min_size(pos2(0.0, 0.0), vec2(w, 800.0) / zoom)),
            outer: Some(Rect::from_min_size(pos2(0.0, 0.0), vec2(w, 830.0) / zoom)),
            native_ppp: Some(1.0),
            zoom,
            maximized: Some(true),
            fullscreen: Some(false),
            monitor: Some(vec2(1920.0, 1080.0) / zoom),
        }
    }

    /// A synthetic oscillation — the zoom 1.0 ↔ 1.25 every frame, as issue #1's — trips
    /// the watch within a second, once, and its report names both states.
    #[test]
    fn an_oscillation_with_no_input_trips_the_watch() {
        let mut w = Watch::default();
        let mut tripped = None;
        for f in 0..120 {
            let z = if f % 2 == 0 { 1.0 } else { 1.25 };
            let v = w.observe(f as f64 / 60.0, at(z, 1920.0), false);
            if v == Verdict::Tripped {
                assert!(tripped.is_none(), "it trips once");
                tripped = Some(f);
            }
        }
        let f = tripped.expect("the loop is caught");
        assert!(f <= 60, "within a second at 60 frames a second: frame {f}");
        assert!(w.tripped());
        let report = w.report();
        assert!(
            report.contains("zoom=1.000") && report.contains("zoom=1.250"),
            "{report}"
        );
        assert!(report.contains("went back and forth"), "{report}");
        assert!(
            report.contains("inner_px=Some((1920.0, 800.0))"),
            "{report}"
        );
    }

    /// The same loop at the pace issue #1's had through egui (each value held two frames,
    /// the zoom applied a pass late) trips it too.
    #[test]
    fn a_two_frame_oscillation_trips_the_watch() {
        let mut w = Watch::default();
        let tripped = (0..120).find(|f| {
            let z = if (f / 2) % 2 == 0 { 1.0 } else { 1.25 };
            w.observe(*f as f64 / 60.0, at(z, 1920.0), false) == Verdict::Tripped
        });
        assert!(tripped.is_some_and(|f| f <= 60), "{tripped:?}");
    }

    /// A user dragging the window's border makes new sizes, never trips; nor does a loop
    /// of changes every frame that each came with input (Ctrl + and Ctrl − pressed).
    #[test]
    fn a_resize_drag_and_asked_for_changes_never_trip() {
        let mut w = Watch::default();
        for f in 0..600 {
            let width = 1000.0 + (f as f32 * 3.0);
            assert_ne!(
                w.observe(f as f64 / 60.0, at(1.0, width), false),
                Verdict::Tripped
            );
        }
        let mut w = Watch::default();
        for f in 0..600 {
            let z = if f % 2 == 0 { 1.0 } else { 1.25 };
            assert_ne!(
                w.observe(f as f64 / 60.0, at(z, 1920.0), true),
                Verdict::Tripped
            );
        }
        assert!(!w.tripped());
    }

    /// Returns spread thinly (a user maximising and restoring now and then) stay under the
    /// rate that trips it.
    #[test]
    fn slow_returns_stay_under_the_rate() {
        let mut w = Watch::default();
        for f in 0..40 {
            let z = if f % 2 == 0 { 1.0 } else { 1.25 };
            assert_ne!(
                w.observe(f as f64 * 0.5, at(z, 1920.0), false),
                Verdict::Tripped
            );
        }
        assert!(!w.tripped());
    }

    /// A tripped watch drops geometry commands; a quiet one sends them.
    #[test]
    fn a_tripped_watch_drops_geometry_commands() {
        let ctx = egui::Context::default();
        let mut w = Watch::default();
        let maximised = |out: &mut egui::FullOutput| {
            out.textures_delta.clear();
            out.viewport_output[&egui::ViewportId::ROOT]
                .commands
                .iter()
                .filter(|c| matches!(c, egui::ViewportCommand::Maximized(_)))
                .count()
        };
        let mut out = ctx.run_ui(egui::RawInput::default(), |ui| {
            assert!(send_geometry(
                ui.ctx(),
                &w,
                egui::ViewportCommand::Maximized(true)
            ));
        });
        assert_eq!(maximised(&mut out), 1);
        for f in 0..30 {
            let z = if f % 2 == 0 { 1.0 } else { 1.25 };
            w.observe(f as f64 / 60.0, at(z, 1920.0), false);
        }
        assert!(w.tripped());
        let mut out = ctx.run_ui(egui::RawInput::default(), |ui| {
            assert!(!send_geometry(
                ui.ctx(),
                &w,
                egui::ViewportCommand::Maximized(true)
            ));
        });
        assert_eq!(maximised(&mut out), 0);
    }

    #[test]
    fn the_log_file_keeps_the_last_starts() {
        let dir = tempfile::tempdir().unwrap();
        let path = log_path(dir.path());
        {
            use std::io::Write as _;
            let mut f = open_log(dir.path()).unwrap();
            writeln!(f, "first start").unwrap();
        }
        let _second = open_log(dir.path()).unwrap();
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "");
        let prev = path.with_file_name("cantus.previous.log");
        assert_eq!(std::fs::read_to_string(prev).unwrap(), "first start\n");
    }
}
