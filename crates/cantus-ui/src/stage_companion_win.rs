//! The companion on Windows (round 35, ADR 0091 §2): a layered window of true per-pixel
//! alpha (`cantus_rt::window::Layered`) shown from the pixmap `companion_paint` draws — no
//! OpenGL for it, so soft shadows and glows are real alpha on any GPU, OBS's Windows
//! Graphics Capture keeps them, and pixels of alpha 0 let the pointer through to what is
//! under the window. Its menu is the kit's, in a popup of its own (`companion_menu_win`)
//! that opens at once whether the main window is shown or minimised.
//!
//! **Cost for all day.** Drawn only when something changed and no more than 30 times a
//! second while the song plays (12 for the breath when stopped); the pixmap is kept and
//! reused; its CPU time a frame is measured and shown in its menu.
//!
//! **Where it is.** Kept in physical pixels (`StageSettings::native_pos`), opened directly
//! there at its monitor's scale and fitted onto a monitor that is (a monitor unplugged, a
//! scale changed). While the person resizes it by its corner it is drawn at the size the
//! system gives it — drawn at the stored one, `UpdateLayeredWindow` would size it back
//! under the pointer — and the new size is stored when they let go.

use super::companion::{sane_size, Companion, Shared, StageSettings, MIN_SIZE, TITLE};
use super::companion_menu_win::{MenuKit, MenuState, MenuWin};
use super::companion_paint::{draw_cached, ChipCache, Scene};
use super::vis_video::Setup;
use crate::app::CantusApp;
use cantus_rt::window::{fit_in, scale_at, work_area_at, Layered, Rect, WinEvent};
use std::sync::{Arc, Mutex};
use std::time::Instant;
use tiny_skia::Pixmap;

use super::{IDLE_RATE as IDLE_FPS, PLAY_RATE as PLAY_FPS};

/// The native window and what it keeps between frames.
pub struct Native {
    win: Layered,
    pm: Option<Pixmap>,
    drawn_at: f64,
    hover: bool,
    /// The settings it last drew with (a change redraws at once).
    drawn_settings: Option<StageSettings>,
    topmost: bool,
    chips: ChipCache,
    /// Its menu while open, and the context menus are drawn with between openings.
    menu: Option<MenuWin>,
    menu_kit: Option<MenuKit>,
    /// The size it was last shown at outside a sizing loop, and its scale then: a placement
    /// at another size and the same scale is the person's resize.
    shown: ((i32, i32), f32),
    /// The last size step, for the rectangle and the wanted size it was made for.
    fitted: Option<(Rect, (i32, i32), SizeStep)>,
}

impl std::fmt::Debug for Native {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Native").field("win", &self.win).finish()
    }
}

/// `size` (points) in physical pixels at `scale`.
fn px(size: [f32; 2], scale: f32) -> (i32, i32) {
    (
        (size[0] * scale).round().max(1.0) as i32,
        (size[1] * scale).round().max(1.0) as i32,
    )
}

/// A monitor's work area and scale (physical pixels per point) for a rectangle on it — the
/// nearest monitor when it is on none. The system's, or a test's layout.
pub(crate) type Monitors<'a> = &'a dyn Fn(Rect) -> (Rect, f32);

fn system_monitors(r: Rect) -> (Rect, f32) {
    (work_area_at(r).unwrap_or(r), scale_at(r))
}

/// `r` fitted into the work area of the monitor it is most on, its shape kept, at least the
/// smallest companion.
fn fit(r: Rect, monitors: Monitors) -> Rect {
    let (area, s) = monitors(r);
    fit_in(r, area, (MIN_SIZE[0].min(MIN_SIZE[1]) * s) as i32)
}

/// A companion of `size` points with its corner at `corner`: sized at the scale of the
/// monitor it lands on (not the one its corner is on — saved straddling a 100 % and a 150 %
/// monitor, it would open at the other one's size) and fitted into that monitor's work
/// area.
pub(crate) fn place_on(corner: (i32, i32), size: [f32; 2], monitors: Monitors) -> Rect {
    let size = sane_size(size);
    let (_, s0) = monitors((corner.0, corner.1, 1, 1));
    let (w, h) = px(size, s0);
    let r = fit((corner.0, corner.1, w, h), monitors);
    let (_, s1) = monitors(r);
    if (s1 - s0).abs() < 1e-3 {
        return r;
    }
    let (w, h) = px(size, s1);
    fit((r.0, r.1, w, h), monitors)
}

/// Where its corner goes when it opens (physical pixels): its stored corner; a settings
/// file from before physical positions, its points at the primary's scale (as that version
/// placed it); else near the bottom-right of the main window's monitor.
fn corner(ctx: &egui::Context, st: &StageSettings) -> (i32, i32) {
    if let Some([x, y]) = st.native_pos {
        return (x, y);
    }
    if let Some([x, y]) = st.pos {
        let s = scale_at((0, 0, 1, 1));
        return ((x * s) as i32, (y * s) as i32);
    }
    let ppp = ctx.pixels_per_point();
    let main = ctx
        .input(|i| i.viewport().outer_rect)
        .map(|r| {
            (
                (r.min.x * ppp) as i32,
                (r.min.y * ppp) as i32,
                (r.width() * ppp) as i32,
                (r.height() * ppp) as i32,
            )
        })
        .unwrap_or((0, 0, 1, 1));
    let area = work_area_at(main).unwrap_or((0, 0, 1600, 900));
    let s = scale_at(area);
    let (w, h) = px(sane_size(st.size), s);
    let gap = (40.0 * s) as i32;
    (area.0 + area.2 - w - gap, area.1 + area.3 - h - gap)
}

/// What the settings keep of a window placed at `r` (physical pixels) on a monitor of
/// `scale`: its corner as it is; its size in points (within reason) only when the person
/// `resized` it by its corner — moved, or fitted onto a smaller screen, it keeps the size
/// chosen (a preset of the menu stays that preset: its shape, and the menu's cycle).
pub(crate) fn placed(st: &mut StageSettings, r: Rect, scale: f32, resized: bool) {
    let scale = if scale.is_finite() && scale > 0.0 {
        scale
    } else {
        1.0
    };
    st.native_pos = Some([r.0, r.1]);
    if resized {
        st.size = sane_size([r.2 as f32 / scale, r.3 as f32 / scale]);
    }
}

/// What a frame does about the window's size: draw at `draw` (physical pixels), after
/// moving its corner to `to`.
#[derive(Copy, Clone, Debug, PartialEq)]
pub(crate) struct SizeStep {
    pub draw: (i32, i32),
    pub to: Option<(i32, i32)>,
}

/// The size a frame draws at, for a window now at `live` that wants `want` (its size in
/// points at its scale): while the person sizes it, the system's; minimised, nothing to
/// fit (its rectangle is the system's parked one, far off every screen — fitting it would
/// move its saved place); else `want`, fitted onto its monitor (shape kept), its corner
/// moved where that needs it.
pub(crate) fn size_step(
    minimised: bool,
    sizing: bool,
    live: Rect,
    want: (i32, i32),
    monitors: Monitors,
) -> SizeStep {
    if sizing {
        return SizeStep {
            draw: (live.2.max(1), live.3.max(1)),
            to: None,
        };
    }
    if minimised || (live.2, live.3) == want {
        return SizeStep {
            draw: want,
            to: None,
        };
    }
    let f = fit((live.0, live.1, want.0, want.1), monitors);
    SizeStep {
        draw: (f.2, f.3),
        to: ((f.0, f.1) != (live.0, live.1)).then_some((f.0, f.1)),
    }
}

/// Open the window where it was, sized and fitted onto the monitor it lands on.
fn open(ctx: &egui::Context, st: &StageSettings) -> Option<Native> {
    let (x, y, w, h) = place_on(corner(ctx, st), st.size, &system_monitors);
    let win = Layered::new(TITLE, x, y, w, h, st.on_top)?;
    let waker = ctx.clone();
    win.set_waker(Box::new(move || waker.request_repaint()));
    let scale = win.scale();
    Some(Native {
        win,
        pm: None,
        drawn_at: f64::NEG_INFINITY,
        hover: false,
        drawn_settings: None,
        topmost: st.on_top,
        chips: ChipCache::default(),
        menu: None,
        menu_kit: None,
        shown: ((w, h), scale),
        fitted: None,
    })
}

/// One frame of the native companion: its events, its drawing when due, its menu.
pub fn run(
    app: &mut CantusApp,
    c: &mut Companion,
    ctx: &egui::Context,
    shared: &Arc<Mutex<Shared>>,
    now: f64,
    playing: bool,
) {
    if c.native.is_none() {
        c.native = open(ctx, &app.settings.stage);
        if let Some(n) = c.native.as_ref() {
            // Where it really opened (fitted onto a monitor) is where it is kept; its size
            // stays the one chosen.
            let r = n.win.rect();
            let mut st = app.settings.stage.clone();
            placed(&mut st, r, n.win.scale(), false);
            if st != app.settings.stage {
                app.settings.stage = st;
                app.store_settings();
            }
        }
    }
    let Some(n) = c.native.as_mut() else {
        return;
    };
    let scale = n.win.scale();
    let mut changed_settings = false;
    let mut menu_at = None;
    for e in n.win.events() {
        match e {
            WinEvent::Placed { x, y, w, h } => {
                // Resized by the person: another size at the same scale (a new monitor's
                // scale resizes it too, and keeps the size in points).
                let ((sw, sh), ss) = n.shown;
                let resized = (w, h) != (sw, sh) && (scale - ss).abs() < 1e-3;
                placed(&mut app.settings.stage, (x, y, w, h), scale, resized);
                changed_settings = true;
            }
            WinEvent::Minimised(m) => {
                shared.lock().unwrap_or_else(|e| e.into_inner()).minimized = m;
            }
            WinEvent::Hover(h) => n.hover = h,
            WinEvent::Menu { x, y } => menu_at = Some((x, y)),
            WinEvent::Close => {
                app.settings.stage.companion_open = false;
                changed_settings = true;
            }
            // Being resized: the drawing below follows the window's own size.
            WinEvent::Resizing { .. } => {}
            _ => {}
        }
    }
    if changed_settings {
        app.store_settings();
    }
    if !app.settings.stage.companion_open {
        c.native = None;
        return;
    }
    let st = app.settings.stage.clone();
    if n.topmost != st.on_top {
        n.win.set_topmost(st.on_top);
        n.topmost = st.on_top;
    }
    let grip = (22.0 * scale) as i32;
    n.win.set_interactive(!st.streaming, !st.streaming, grip);
    let (min_w, min_h) = px(MIN_SIZE, scale);
    n.win.set_min_size(min_w, min_h);
    let minimised = n.win.is_minimised();

    // Its size: the window's own while the person sizes it (the system's sizing loop); the
    // chosen one otherwise, grown from its corner and kept whole on its monitor — the size
    // chosen is what stays stored, the corner moved where that needs it.
    let sizing = n.win.is_sizing();
    let live = n.win.rect();
    let want = px(st.size, scale);
    let step = match n.fitted {
        Some((r, w, step)) if r == live && w == want && !sizing && !minimised => step,
        _ => {
            let step = size_step(minimised, sizing, live, want, &system_monitors);
            if !sizing && !minimised {
                n.fitted = Some((live, want, step));
            }
            step
        }
    };
    if let Some((x, y)) = step.to {
        n.win.move_to(x, y);
        app.settings.stage.native_pos = Some([x, y]);
        app.store_settings();
    }
    if !sizing && !minimised {
        n.shown = (step.draw, scale);
    }
    let (w, h) = (step.draw.0 as u32, step.draw.1 as u32);
    let st = app.settings.stage.clone();
    let resized =
        n.pm.as_ref()
            .is_none_or(|p| p.width() != w || p.height() != h);

    // Draw when due: something changed, and no faster than the rate.
    let rate = if playing { PLAY_FPS } else { IDLE_FPS };
    let settings_changed = n.drawn_settings.as_ref() != Some(&st);
    let due = settings_changed || resized || now - n.drawn_at >= 0.95 / rate || now < n.drawn_at;
    if !minimised && due {
        draw(n, app, &st, shared, scale, (w, h), now);
    }
    // The next drawing: the main window's frames come at 60 while playing; stopped, one
    // wake-up for the breath.
    if !minimised && !playing {
        crate::views::workspace_ticker::wake_after(
            ctx,
            std::time::Duration::from_secs_f64(1.0 / IDLE_FPS),
        );
    }
    menu(n, ctx, shared, menu_at, settings_changed);
}

/// Draw and present the companion at `size` (physical pixels), its cost measured.
fn draw(
    n: &mut Native,
    app: &CantusApp,
    st: &StageSettings,
    shared: &Arc<Mutex<Shared>>,
    scale: f32,
    size: (u32, u32),
    now: f64,
) {
    let t0 = Instant::now();
    let (w, h) = size;
    if n.pm
        .as_ref()
        .is_none_or(|p| p.width() != w || p.height() != h)
    {
        n.pm = Pixmap::new(w, h);
    }
    let setup = Setup::from_theme(&app.project().name, false, st.backdrop.chroma().is_some());
    let mut sh = shared.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(pm) = n.pm.as_mut() {
        let scene = Scene {
            settings: st,
            setup: &setup,
            dancers: &sh.dancers,
            place_feet: !sh.follows,
            vis: &sh.vis,
            hover: n.hover,
            scale,
        };
        let drawn = draw_cached(pm, &scene, &mut n.chips);
        n.win.set_button(drawn.button);
        n.win.present(pm.data(), w, h);
    }
    n.drawn_at = now;
    n.drawn_settings = Some(st.clone());
    sh.redraws += 1;
    // Its cost: CPU time of a drawing and presenting, smoothed; its frame rate.
    let ms = t0.elapsed().as_secs_f32() * 1e3;
    let dt = (now - sh.last_pass) as f32;
    sh.last_pass = now;
    let cost = &mut sh.cost;
    cost.cpu_ms = if cost.frames == 0 {
        ms
    } else {
        cost.cpu_ms * 0.9 + ms * 0.1
    };
    if dt > 0.0 && dt < 2.0 {
        cost.fps = if cost.fps == 0.0 {
            1.0 / dt
        } else {
            cost.fps * 0.9 + 0.1 / dt
        };
    }
    cost.frames += 1;
}

/// Its menu: opened at `at` (a right-click, its ⋯ button) at once — in a pass or from
/// `logic` alike — and given its frame while open.
fn menu(
    n: &mut Native,
    ctx: &egui::Context,
    shared: &Arc<Mutex<Shared>>,
    at: Option<(i32, i32)>,
    settings_changed: bool,
) {
    if let Some(at) = at {
        if let Some(m) = n.menu.take() {
            n.menu_kit = Some(m.into_kit());
        }
        let kit = n.menu_kit.take().unwrap_or_else(|| MenuKit::new(ctx));
        match MenuWin::open(ctx, at, kit, shared) {
            Ok(m) => n.menu = Some(m),
            Err(kit) => n.menu_kit = Some(kit),
        }
    }
    let Some(m) = n.menu.as_mut() else {
        return;
    };
    if settings_changed {
        // A value stepped from the menu: shown in it at once.
        m.refresh();
    }
    let state = m.frame(shared);
    // A choice in it is applied by the next frame.
    if !shared
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .actions
        .is_empty()
    {
        ctx.request_repaint();
    }
    match state {
        MenuState::Closed => {
            if let Some(m) = n.menu.take() {
                n.menu_kit = Some(m.into_kit());
            }
        }
        MenuState::Open(Some(d)) => ctx.request_repaint_after(d),
        MenuState::Open(None) => {}
    }
}

#[cfg(test)]
mod tests {
    use super::super::companion::{Action, Companion, StageSettings, MIN_SIZE, SIZES};
    use super::super::companion_menu_win::{MenuKit, MenuState, MenuWin};
    use super::{place_on, placed, px, size_step, SizeStep};
    use cantus_rt::window::{Rect, WinEvent};

    /// Two monitors side by side as `MonitorFromRect` sees them: a 150 % primary
    /// (1920 × 1080, its taskbar at the foot) and a 100 % one to its right.
    fn two(r: Rect) -> (Rect, f32) {
        let a = ((0, 0, 1920, 1032), 1.5);
        let b = ((1920, 0, 1920, 1040), 1.0);
        let overlap = |m: Rect| {
            let w = (r.0 + r.2).min(m.0 + m.2) - r.0.max(m.0);
            let h = (r.1 + r.3).min(m.1 + m.3) - r.1.max(m.1);
            i64::from(w.max(0)) * i64::from(h.max(0))
        };
        let (oa, ob) = (overlap(a.0), overlap(b.0));
        if oa > ob || (oa == ob && r.0 < 1920) {
            a
        } else {
            b
        }
    }

    /// One 1080p screen at 125 %, its taskbar at the foot.
    fn laptop(_: Rect) -> (Rect, f32) {
        ((0, 0, 1920, 1032), 1.25)
    }

    /// Review #4: a corner saved on a 100 % monitor right of a 150 % primary is kept as
    /// the system gave it, and the size in points of the monitor it was on.
    #[test]
    fn a_placed_window_keeps_its_physical_corner_and_its_size_in_points() {
        let mut st = StageSettings::default();
        placed(&mut st, (2900, -40, 600, 690), 1.5, true);
        assert_eq!(st.native_pos, Some([2900, -40]));
        assert_eq!(st.size, [400.0, 460.0]);
        placed(&mut st, (1930, 200, 400, 460), 1.0, true);
        assert_eq!(st.native_pos, Some([1930, 200]));
        assert_eq!(st.size, [400.0, 460.0]);
    }

    /// Review #4: a window the system made a pixel or two wide is never kept that size.
    #[test]
    fn a_window_of_a_pixel_is_kept_at_the_least_size() {
        let mut st = StageSettings::default();
        placed(&mut st, (0, 0, 1, 2), 1.25, true);
        assert_eq!(st.size, MIN_SIZE);
        placed(&mut st, (0, 0, 400, 400), f32::NAN, true);
        assert_eq!(st.size, [400.0, 400.0]);
    }

    /// Last review #1: minimised, its rectangle is the system's parked one far off every
    /// screen — nothing is fitted, nothing moved, its saved place untouched.
    #[test]
    fn minimised_nothing_is_fitted_or_moved() {
        let parked = (-32000, -32000, 160, 28);
        let step = size_step(true, false, parked, (500, 575), &laptop);
        assert_eq!(
            step,
            SizeStep {
                draw: (500, 575),
                to: None
            }
        );
    }

    /// Last review #2: the vertical preset on a screen shorter than it is shrunk whole —
    /// still 9:16 — and only its corner is stored: the size kept is the preset, so the
    /// menu's next size is the wide one, not the first.
    #[test]
    fn a_preset_taller_than_the_screen_keeps_its_shape_and_its_place_in_the_cycle() {
        let vertical = SIZES[4];
        let want = px(vertical, 1.25);
        let live = (1200, 300, 500, 575);
        let step = size_step(false, false, live, want, &laptop);
        let (w, h) = step.draw;
        assert_eq!(h, 1032);
        assert!(
            (w as f32 / h as f32 - vertical[0] / vertical[1]).abs() < 0.003,
            "{w} × {h}"
        );
        let to = step.to.expect("moved up to fit");
        assert!(to.1 + h <= 1032 && to.0 + w <= 1920);
        // The system tells where it went (no resize of the person's): the preset stays.
        let mut st = StageSettings {
            size: vertical,
            ..Default::default()
        };
        placed(&mut st, (to.0, to.1, w, h), 1.25, false);
        assert_eq!(st.size, vertical);
        let i = SIZES.iter().position(|z| *z == st.size).expect("a preset");
        assert_eq!(SIZES[(i + 1) % SIZES.len()], SIZES[5], "the wide one next");
    }

    /// Last review #2: moved by the person, a window keeps the size chosen; resized by its
    /// corner, it keeps the new size.
    #[test]
    fn moved_it_keeps_its_size_resized_it_keeps_the_new_one() {
        let mut st = StageSettings {
            size: SIZES[2],
            ..Default::default()
        };
        placed(&mut st, (300, 200, 674, 749), 1.25, false);
        assert_eq!(st.size, SIZES[2]);
        placed(&mut st, (300, 200, 750, 800), 1.25, true);
        assert_eq!(st.size, [600.0, 640.0]);
    }

    /// Last review #4: saved with its corner on a 150 % monitor and most of it on the
    /// 100 % one beside it, it opens on the 100 % one at that one's size — not at the
    /// corner's 150 %, which it would have stored as a size half as big again.
    #[test]
    fn it_opens_at_the_scale_of_the_monitor_it_lands_on() {
        let r = place_on((1800, 100), [400.0, 460.0], &two);
        assert_eq!(r, (1920, 100, 400, 460));
        // On one monitor, the corner kept.
        assert_eq!(
            place_on((200, 100), [400.0, 460.0], &two),
            (200, 100, 600, 690)
        );
    }

    /// Review #3: the menu is a popup of ours — its clicks reach the kit's menu without
    /// any pass of the main window, a choice is queued, Esc closes it.
    #[test]
    fn the_menu_popup_takes_clicks_and_closes_on_escape() {
        crate::i18n::default_thread_locale(crate::i18n::Locale::En);
        let outer = egui::Context::default();
        crate::design::install_fonts(&outer);
        crate::design::install_style(&outer);
        let c = Companion::default();
        let shared = c.shared.clone();
        let mut m = match MenuWin::open(&outer, (200, 200), MenuKit::new(&outer), &shared) {
            Ok(m) => m,
            Err(_) => panic!("the popup opened"),
        };
        // Down the menu a few points at a time until a row answers: the second row is
        // the size, which steps.
        let mut seen = Vec::new();
        for i in 0..80 {
            let y = 6 + i * 6;
            assert!(!m.feed(&[
                WinEvent::Pointer { x: 60, y },
                WinEvent::Button {
                    down: true,
                    x: 60,
                    y
                },
                WinEvent::Button {
                    down: false,
                    x: 60,
                    y
                },
            ]));
            if m.frame(&shared) == MenuState::Closed {
                break;
            }
            let mut sh = shared.lock().unwrap();
            for a in sh.actions.drain(..) {
                if !seen.contains(&a) {
                    seen.push(a);
                }
            }
        }
        assert!(seen.contains(&Action::NextSize), "{seen:?}");
        assert!(seen.contains(&Action::ToggleOnTop), "{seen:?}");
        assert!(m.feed(&[WinEvent::Key(0x1B)]), "Esc closes it");
        let _kit = m.into_kit();
    }
}
