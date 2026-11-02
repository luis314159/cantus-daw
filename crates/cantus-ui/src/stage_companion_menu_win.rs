//! The companion's menu on Windows (round 35, ADR 0091 §2): the kit's menu, drawn by an
//! egui context of its own on the CPU (`crate::raster`) into a small layered popup at the
//! pointer — a window of ours, not an egui viewport.
//!
//! Why not a viewport: while the main window is minimised (a streamer's usual set-up) eframe
//! runs no pass at all, only `App::logic`, and a viewport can only be opened from a pass —
//! the menu could not open until the main window came back, and then at a stale place. This
//! one opens from `logic` as from a pass, at once, where it was asked for, on the monitor
//! it was asked on (at that monitor's scale), fitted inside its work area.
//!
//! It draws only when something changed (the pointer moved, a click, a value stepped) and
//! closes on a choice that is done with it, on Esc, or when another window is chosen.

use super::companion::{Action, Shared};
use crate::raster::{Canvas, Textures};
use cantus_rt::window::{fit_in, scale_at, work_area_at, Layered, WinEvent};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// The popup's title (OBS lists windows by title; this one is never a source).
const TITLE: &str = "Cantus Stage menu";
/// `VK_ESCAPE`.
const VK_ESCAPE: u32 = 0x1B;

/// The context the menu is drawn with and the textures it uploaded: kept between openings
/// (installing the fonts and uploading their atlas is the slow part of a first opening).
pub struct MenuKit {
    ctx: egui::Context,
    textures: Textures,
}

impl MenuKit {
    pub fn new(outer: &egui::Context) -> MenuKit {
        let ctx = egui::Context::default();
        crate::design::install_fonts(&ctx);
        crate::design::install_style(&ctx);
        MenuKit {
            ctx,
            textures: Textures::default(),
        }
        .themed(outer)
    }

    /// The main window's theme (it may have changed since the last opening).
    fn themed(self, outer: &egui::Context) -> MenuKit {
        let theme = outer.theme();
        self.ctx.set_theme(theme);
        self.ctx
            .set_visuals_of(theme, outer.style_of(theme).visuals.clone());
        self
    }
}

/// An open menu.
pub struct MenuWin {
    win: Layered,
    kit: MenuKit,
    /// Its size, points, and its pixels per point (its monitor's scale × the zoom).
    size: egui::Vec2,
    ppp: f32,
    events: Vec<egui::Event>,
    start: Instant,
    /// When egui asked to be drawn again (seconds since `start`), if it did.
    due: Option<f64>,
    drawn: bool,
    rgba: Vec<u8>,
}

/// After a frame of the menu.
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum MenuState {
    /// Still open; draw it again after this long if nothing happens first.
    Open(Option<Duration>),
    Closed,
}

/// Where a `w` × `h` menu asked for at `at` goes: below and right of the pointer, flipped
/// above or left where it would leave `area`, and inside it.
pub fn menu_rect(
    at: (i32, i32),
    w: i32,
    h: i32,
    area: (i32, i32, i32, i32),
) -> (i32, i32, i32, i32) {
    let (mut x, mut y) = at;
    if x + w > area.0 + area.2 {
        x -= w;
    }
    if y + h > area.1 + area.3 {
        y -= h;
    }
    fit_in((x, y, w, h), area, 1)
}

impl MenuWin {
    /// Open the menu at `at` (the screen's physical pixels), drawn with `kit` from what
    /// `shared` holds. On failure the kit comes back.
    pub fn open(
        outer: &egui::Context,
        at: (i32, i32),
        kit: MenuKit,
        shared: &Arc<Mutex<Shared>>,
    ) -> Result<MenuWin, MenuKit> {
        let kit = kit.themed(outer);
        let ppp = scale_at((at.0, at.1, 1, 1)) * outer.zoom_factor();
        let size = super::companion_view::menu_size(outer);
        let (w, h) = ((size.x * ppp).ceil() as i32, (size.y * ppp).ceil() as i32);
        let area = work_area_at((at.0, at.1, 1, 1)).unwrap_or((at.0, at.1, w, h));
        let (x, y, w, h) = menu_rect(at, w, h, area);
        let Some(win) = Layered::popup(TITLE, x, y, w, h) else {
            return Err(kit);
        };
        let waker = outer.clone();
        win.set_waker(Box::new(move || waker.request_repaint()));
        kit.ctx.set_pixels_per_point(ppp);
        let mut m = MenuWin {
            win,
            kit,
            size: egui::vec2(w as f32, h as f32) / ppp,
            ppp,
            events: Vec::new(),
            start: Instant::now(),
            due: None,
            drawn: false,
            rgba: Vec::new(),
        };
        // Drawn before it is made the active window: never an empty frame on screen.
        m.frame(shared);
        m.win.activate();
        Ok(m)
    }

    /// Draw it again at its next frame (a value it shows changed).
    pub fn refresh(&mut self) {
        self.due = Some(0.0);
    }

    /// The context it draws with, back for the next opening.
    pub fn into_kit(self) -> MenuKit {
        self.kit
    }

    /// Hand it what happened to its window: the pointer, a click, Esc, another window
    /// chosen. `true` when that closes it.
    pub fn feed(&mut self, events: &[WinEvent]) -> bool {
        let at = |x: i32, y: i32| egui::pos2(x as f32 / self.ppp, y as f32 / self.ppp);
        for e in events {
            match *e {
                WinEvent::Pointer { x, y } => {
                    self.events.push(egui::Event::PointerMoved(at(x, y)));
                }
                WinEvent::Button { down, x, y } => {
                    let pos = at(x, y);
                    self.events.push(egui::Event::PointerMoved(pos));
                    self.events.push(egui::Event::PointerButton {
                        pos,
                        button: egui::PointerButton::Primary,
                        pressed: down,
                        modifiers: egui::Modifiers::default(),
                    });
                }
                WinEvent::Hover(false) => self.events.push(egui::Event::PointerGone),
                WinEvent::Key(VK_ESCAPE) | WinEvent::Active(false) | WinEvent::Close => {
                    return true;
                }
                _ => {}
            }
        }
        false
    }

    /// One frame: its window's events, a drawing when something changed.
    pub fn frame(&mut self, shared: &Arc<Mutex<Shared>>) -> MenuState {
        let events = self.win.events();
        if self.feed(&events) {
            return MenuState::Closed;
        }
        let now = self.start.elapsed().as_secs_f64();
        let asked = self.due.is_some_and(|d| now >= d);
        if self.drawn && self.events.is_empty() && !asked {
            return MenuState::Open(
                self.due
                    .map(|d| Duration::from_secs_f64((d - now).max(0.0))),
            );
        }
        let mut closes = false;
        let mut delay = None;
        // The first opening runs twice: egui lays a new window out on its second pass.
        for _ in 0..if self.drawn { 1 } else { 2 } {
            let (c, d) = self.pass(shared, now);
            closes |= c;
            delay = d;
        }
        if closes {
            return MenuState::Closed;
        }
        self.present();
        self.drawn = true;
        // Drawn again at least once a second: its cost line is live.
        let delay = delay.map_or(Duration::from_secs(1), |d| d.min(Duration::from_secs(1)));
        self.due = Some(now + delay.as_secs_f64());
        MenuState::Open(Some(delay))
    }

    fn pass(&mut self, shared: &Arc<Mutex<Shared>>, now: f64) -> (bool, Option<Duration>) {
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, self.size)),
            time: Some(now),
            focused: true,
            events: std::mem::take(&mut self.events),
            ..Default::default()
        };
        let mut sh = shared.lock().unwrap_or_else(|e| e.into_inner());
        let before = sh.actions.len();
        let mut out = self.kit.ctx.run_ui(input, |ui| {
            super::companion_view::menu(ui, &mut sh);
        });
        let closes = sh.actions[before..].iter().any(|a| {
            matches!(
                a,
                Action::Close | Action::CopyObsSetup | Action::ExportVideo
            )
        });
        drop(sh);
        self.kit.textures.update(&out.textures_delta);
        out.textures_delta.clear();
        let delay = out
            .viewport_output
            .get(&egui::ViewportId::ROOT)
            .map(|v| v.repaint_delay)
            .filter(|d| *d < Duration::from_secs(1));
        let ppp = out.pixels_per_point;
        let prims = self
            .kit
            .ctx
            .tessellate(std::mem::take(&mut out.shapes), ppp);
        let (w, h) = (
            (self.size.x * ppp).round() as usize,
            (self.size.y * ppp).round() as usize,
        );
        let s = crate::kit::Style::current();
        let mut canvas = Canvas::new(w, h, s.t.menu);
        canvas.paint(&prims, &self.kit.textures, ppp);
        self.rgba.clear();
        self.rgba.reserve(w * h * 4);
        for y in 0..h {
            for x in 0..w {
                self.rgba.extend_from_slice(&canvas.pixel(x, y));
            }
        }
        (closes, delay)
    }

    fn present(&mut self) {
        let w = (self.size.x * self.ppp).round() as u32;
        let h = (self.size.y * self.ppp).round() as u32;
        self.win.present(&self.rgba, w, h);
    }
}

#[cfg(test)]
mod tests {
    use super::menu_rect;

    const AREA: (i32, i32, i32, i32) = (0, 0, 1920, 1040);

    #[test]
    fn the_menu_opens_below_right_of_the_pointer() {
        assert_eq!(menu_rect((500, 300), 375, 480, AREA), (500, 300, 375, 480));
    }

    #[test]
    fn near_the_screen_s_edges_it_flips_and_stays_on_it() {
        // At the bottom-right corner (where the companion opens): above and left.
        assert_eq!(
            menu_rect((1900, 1000), 375, 480, AREA),
            (1525, 520, 375, 480)
        );
        // On a second monitor to the left, near its top.
        let left = (-1280, 0, 1280, 984);
        assert_eq!(menu_rect((-20, 10), 375, 480, left), (-395, 10, 375, 480));
    }
}
