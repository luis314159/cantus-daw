//! A window's pass (ADR 0148): the application lent to it on the UI thread, its input given
//! its own screen's scale, its dock drawn with the views as they are.

use crate::app::CantusApp;
use crate::screens::PxRect;
use crate::views::workspace_ticker as ticker;
use crate::windows::{Dock, Seen};
use egui::{Context, RawInput, ViewportId};
use std::cell::RefCell;

thread_local! {
    /// The application, while a window's pass borrows it (`boot::Shell` lends it before the
    /// pass and takes it back at its next call; the tests do the same).
    static LENT: RefCell<Option<Box<CantusApp>>> = const { RefCell::new(None) };
}

/// Lend the application to the windows' passes on this thread.
pub fn lend(app: Box<CantusApp>) {
    LENT.with(|c| *c.borrow_mut() = Some(app));
}

/// Take the application back.
pub fn take_back() -> Option<Box<CantusApp>> {
    LENT.with(|c| c.borrow_mut().take())
}

/// Run `f` with the lent application; `None` when it is not lent (or already borrowed: a
/// pass inside a pass — a native dialog's message loop — draws nothing and asks again).
pub fn with_lent<R>(f: impl FnOnce(&mut CantusApp) -> R) -> Option<R> {
    LENT.with(|c| {
        let mut b = c.try_borrow_mut().ok()?;
        b.as_mut().map(|a| f(a))
    })
}

/// The callback every window's deferred viewport runs: its key, nothing else (so it is
/// `Send + Sync + 'static`); the application from the UI thread's cell.
pub fn callback(key: u32) -> impl Fn(&mut egui::Ui, egui::ViewportClass) + Send + Sync + 'static {
    move |ui, _class| {
        if with_lent(|app| app.window_pass(ui, key)).is_none() {
            ui.ctx()
                .request_repaint_after(std::time::Duration::from_millis(16));
        }
    }
}

/// A window's input before its pass (ADR 0148 §5): what the system says of it noted
/// ([`Seen`], in physical pixels), and its scale made its own — its `native_pixels_per_point`
/// multiplied by *its scale ÷ egui's zoom* and every point it carries divided by the same,
/// so the window lays out at its screen's interface scale while egui's zoom never moves.
/// `boot::Shell::raw_input_hook` calls it for every window's pass; the main window's input
/// is left as it is.
pub fn adjust_input(app: &mut CantusApp, ctx: &Context, raw: &mut RawInput) {
    let id = raw.viewport_id;
    let Some(i) = app.windows.index_of(id) else {
        return;
    };
    let zoom = ctx.zoom_factor();
    let Some(info) = raw.viewports.get_mut(&id) else {
        return;
    };
    let native = info.native_pixels_per_point.unwrap_or(1.0).max(0.25);
    let px = |r: egui::Rect| PxRect {
        x: (r.min.x * zoom * native).round() as i32,
        y: (r.min.y * zoom * native).round() as i32,
        w: (r.width() * zoom * native).round() as i32,
        h: (r.height() * zoom * native).round() as i32,
    };
    let seen = Seen {
        outer: info.outer_rect.map(px),
        native,
        minimized: info.minimized.unwrap_or(false),
        focused: info.focused.unwrap_or(false),
        maximized: info.maximized.unwrap_or(false),
        fullscreen: info.fullscreen.unwrap_or(false),
    };
    let monitor_px = info
        .monitor_size
        .map(|m| crate::screen_scale::monitor_px(m.x, zoom, native));
    let w = &mut app.windows.list[i];
    if w.seen.native != seen.native || (w.shown && w.seen.outer != seen.outer) {
        app.windows.stale |= w.seen.native != seen.native;
    }
    w.seen = seen;
    // Its own scale: the user's for this window, else its screen's by the follow rule,
    // as far from the main window's as the two screens' rules are (a user's own scale for
    // the main window carries over).
    let rule = monitor_px.map(|px| crate::screen_scale::for_screen(px, native));
    let main_rule = app.windows.main_rule.max(0.25);
    let wanted = w
        .scale
        .or(rule.map(|r| (zoom * r / main_rule).clamp(0.5, 3.0)))
        .unwrap_or(w.zoom.max(0.25));
    w.zoom = wanted;
    let k = wanted / zoom.max(0.25);
    w.factor = k;
    if (k - 1.0).abs() < 1e-4 {
        return;
    }
    info.native_pixels_per_point = Some(native * k);
    let shrink = |r: egui::Rect| egui::Rect::from_min_max(r.min / k, r.max / k);
    info.inner_rect = info.inner_rect.map(shrink);
    info.outer_rect = info.outer_rect.map(shrink);
    info.monitor_size = info.monitor_size.map(|m| m / k);
    raw.screen_rect = raw.screen_rect.map(shrink);
    for e in &mut raw.events {
        match e {
            egui::Event::PointerMoved(p) => *p = (p.to_vec2() / k).to_pos2(),
            egui::Event::PointerButton { pos, .. } => *pos = (pos.to_vec2() / k).to_pos2(),
            egui::Event::Touch { pos, .. } => *pos = (pos.to_vec2() / k).to_pos2(),
            egui::Event::MouseWheel {
                unit: egui::MouseWheelUnit::Point,
                delta,
                ..
            } => *delta /= k,
            _ => {}
        }
    }
}

/// Whether this pass's input was the user's doing in this window: a press, a key, typed
/// text, the clipboard's keys (the window then hosts the dialogs and the editors opened).
pub fn interacted(ctx: &Context) -> bool {
    ctx.input(|inp| {
        inp.events.iter().any(|e| {
            matches!(
                e,
                egui::Event::PointerButton { pressed: true, .. }
                    | egui::Event::Key { pressed: true, .. }
                    | egui::Event::Text(_)
                    | egui::Event::Copy
                    | egui::Event::Cut
                    | egui::Event::Paste(_)
            )
        })
    })
}

/// Whether `pane` moves for one of `reasons` (the ticker's bits): what a window must draw
/// while the song plays, a meter falls or an analyser runs.
pub fn moves(pane: crate::views::workspace::Pane, reasons: u16) -> bool {
    use crate::views::workspace::Pane as P;
    use ticker::Reason as R;
    let any = |rs: &[R]| rs.iter().any(|r| reasons & *r as u16 != 0);
    match pane {
        P::Arrangement | P::PianoRoll | P::Score | P::Session => any(&[R::Playback, R::Recording]),
        P::Mixer | P::Inspector | P::ChannelRack => any(&[R::Playback, R::Recording, R::Meters]),
        P::Device(_) => any(&[R::Playback, R::Recording, R::Meters, R::Live]),
        P::Stage => any(&[R::Playback, R::Animation, R::Meters]),
        P::Devices | P::Browser | P::Clips | P::Routing | P::Controllers | P::History => false,
    }
}

impl CantusApp {
    /// Ask each window whose areas move for the main window's reasons for its next frame —
    /// the main window's pass is the windows' clock; a window holding only the browser draws
    /// nothing while the song plays.
    pub(crate) fn drive_windows(&self, ctx: &Context, tick: ticker::Tick) {
        if tick.reasons == 0 {
            return;
        }
        for (i, w) in self.windows.list.iter().enumerate() {
            if w.closing || w.seen.minimized {
                continue;
            }
            let ws = self.dock(Dock::Window(i));
            let visible = ws
                .dock
                .iter_leaves()
                .filter_map(|(_, l)| l.tabs.get(l.active.0));
            let mut any = false;
            for p in visible {
                any |= moves(*p, tick.reasons);
            }
            if any {
                ctx.request_repaint_after_for(ticker::FRAME, w.id());
            }
        }
    }

    /// Ready the dock about to be drawn: what the other windows hold, whether it takes the
    /// device editors opened now, where its tabs can go.
    pub(crate) fn prepare_dock(&mut self, viewport: ViewportId) {
        let here = self.here();
        let mut elsewhere = Vec::new();
        for (d, w) in self.docks() {
            if d != here {
                elsewhere.extend(w.panes());
            }
        }
        let targets = if self.workspace.menu_open() {
            self.move_targets(here)
        } else {
            Vec::new()
        };
        let host = self.windows.host;
        let host_alive = host == ViewportId::ROOT || self.windows.index_of(host).is_some();
        let ws = &mut self.workspace;
        ws.elsewhere = elsewhere;
        ws.adds_devices = host == viewport || (!host_alive && viewport == ViewportId::ROOT);
        ws.move_targets = targets;
    }

    /// Whether the dialogs and pickers draw in `viewport`'s pass: the window the user last
    /// clicked or typed in, the main window when that one is gone.
    pub(crate) fn hosts_overlays(&self, viewport: ViewportId) -> bool {
        let host = self.windows.host;
        if host == viewport {
            return true;
        }
        viewport == ViewportId::ROOT && self.windows.index_of(host).is_none()
    }

    /// One pass of the window with key `key` (its deferred viewport's callback; the main
    /// window's pass calls it inside an egui window when viewports are embedded, `embedded`).
    pub fn window_pass(&mut self, ui: &mut egui::Ui, key: u32) {
        let embedded = ui.ctx().embed_viewports();
        self.window_pass_in(ui, key, embedded);
    }

    pub(crate) fn window_pass_in(&mut self, ui: &mut egui::Ui, key: u32, embedded: bool) {
        let ctx = ui.ctx().clone();
        let Some(i) = self.windows.index(key) else {
            return;
        };
        if self.windows.list[i].closing || self.windows.current.is_some() {
            return;
        }
        // Its close button: its areas go back to the main window.
        if !embedded && ctx.input(|inp| inp.viewport().close_requested()) {
            self.windows.list[i].closing = true;
            ctx.request_repaint_of(ViewportId::ROOT);
            return;
        }
        let viewport = if embedded {
            self.windows.list[i].id()
        } else {
            ctx.viewport_id()
        };
        self.windows.list[i].passes += 1;
        let began = std::time::Instant::now();
        let interacted = interacted(&ctx);
        if interacted {
            self.windows.host = viewport;
        }
        let revision = self.bus.revision();
        if !embedded {
            let now = ctx.input(|inp| inp.time);
            let seen = crate::window_watch::Sample::of(&ctx);
            let prompted = crate::window_watch::prompted(&ctx);
            let _ = self.windows.list[i].watch.observe(now, seen, prompted);
        }
        // Its dock in, the main window's into its slot (see `crate::windows`).
        std::mem::swap(&mut self.workspace, &mut self.windows.list[i].workspace);
        self.windows.current = Some(i);
        self.prepare_dock(viewport);
        if !embedded {
            self.global_shortcuts(&ctx);
        }
        let s = crate::kit::Style::current();
        if self.windows.list[i].bar {
            let h = (crate::views::transport::HEIGHT * s.t.density).round();
            egui::Panel::top(egui::Id::new("cantus-window-bar"))
                .frame(egui::Frame::NONE)
                .exact_size(h)
                .resizable(false)
                .show(ui, |ui| crate::views::window_bar::show(self, ui, i));
        }
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE.fill(s.t.panel))
            .show(ui, |ui| crate::views::workspace::show(self, ui));
        if !embedded && self.hosts_overlays(viewport) {
            self.overlays(&ctx);
        }
        if !embedded {
            self.every_pass(&ctx);
        }
        crate::views::window_bar::menu(self, &ctx, i);
        crate::views::window_bar::groups_menu(self, &ctx, i);
        crate::views::window_bar::rename_dialog(self, &ctx, i);
        self.windows.current = None;
        std::mem::swap(&mut self.workspace, &mut self.windows.list[i].workspace);
        // A change made here: the engine hears it now, the other windows draw it once.
        self.sync_engine();
        if !embedded {
            if interacted || self.bus.revision() != revision {
                ctx.request_repaint_of(ViewportId::ROOT);
                for w in &self.windows.list {
                    if w.id() != viewport {
                        ctx.request_repaint_of(w.id());
                    }
                }
            }
            ticker::finish(&ctx);
        }
        let key = self.windows.list[i].key;
        if let Some(s) = self.windows.stats.as_mut() {
            s.note(Some(key), began.elapsed().as_secs_f64());
        }
    }
}
