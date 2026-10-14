//! The window's and the taskbar's icon, in the colour the appearance names (ADR 0046).
//!
//! The icon is the website's mark (`kit::mark`), rendered here at the exact sizes the
//! window's monitor wants and handed to the window: on Windows both of its icons
//! (`cantus_launch::taskicon` — the taskbar's and the title bar's), elsewhere through
//! eframe's viewport command. It is redone only when the colour or the monitor's scale
//! changes, so a steady frame does nothing.

use crate::kit::mark::{mark_svg, MarkColours};
use std::time::{Duration, Instant};

/// Whether Windows draws the taskbar light, as last read — what the automatic icon colour
/// follows. Read at most every two seconds, and only by the real window's frame (tests
/// that drive the views see a dark taskbar unless they say otherwise).
#[derive(Debug, Default)]
pub struct Taskbar {
    light: bool,
    last: Option<Instant>,
}

impl Taskbar {
    /// Whether the taskbar is light (false until read, and off Windows).
    pub fn light(&self) -> bool {
        self.light
    }

    /// Set what the taskbar is (tests, and [`Self::poll`]); returns whether it changed.
    pub fn set(&mut self, light: bool) -> bool {
        std::mem::replace(&mut self.light, light) != light
    }

    /// Whether the setting has been read at all (tests: the running window polls it).
    pub fn polled(&self) -> bool {
        self.last.is_some()
    }

    /// Read Windows' setting if two seconds have passed; returns whether it changed.
    pub fn poll(&mut self) -> bool {
        if self
            .last
            .is_some_and(|l| l.elapsed() < Duration::from_secs(2))
        {
            return false;
        }
        self.last = Some(Instant::now());
        self.set(cantus_launch::taskicon::light_taskbar().unwrap_or(false))
    }
}

/// What the window shows, and what it should show.
#[derive(Debug, Default)]
pub struct WindowIcon {
    want: Option<MarkColours>,
    /// The colours and sizes last given to the window.
    applied: Option<(MarkColours, u32, u32)>,
}

impl WindowIcon {
    /// Ask for the icon in these colours (applied on the next [`Self::sync`]).
    pub fn want(&mut self, c: MarkColours) {
        self.want = Some(c);
    }

    /// The colours asked for last.
    pub fn wanted(&self) -> Option<MarkColours> {
        self.want
    }

    /// The colours and sizes the window was last given.
    pub fn applied(&self) -> Option<(MarkColours, u32, u32)> {
        self.applied
    }

    /// Give the window the icon asked for, if it does not have it yet. `window` is the
    /// native window (eframe's `Frame`).
    pub fn sync(&mut self, ctx: &egui::Context, window: &impl raw_window_handle::HasWindowHandle) {
        let Some(c) = self.want else {
            return;
        };
        let hwnd = native_handle(window);
        let (big, small) = cantus_launch::taskicon::icon_sizes(hwnd.unwrap_or(0));
        if self.applied == Some((c, big, small)) {
            return;
        }
        // Whatever happens next, this colour at these sizes is not tried again every frame.
        self.applied = Some((c, big, small));
        if let Some(hwnd) = hwnd {
            if let (Some(b), Some(s)) = (png(&c, big), png(&c, small)) {
                if cantus_launch::taskicon::set_window_icons(hwnd, &b, big, &s, small) {
                    return;
                }
            }
            if cfg!(windows) {
                log::warn!("the window's icon could not be changed");
                return;
            }
        }
        if let Some(icon) = icon_data(&c, big) {
            ctx.send_viewport_cmd(egui::ViewportCommand::Icon(Some(std::sync::Arc::new(icon))));
        }
    }
}

/// The Win32 window handle, as an integer; `None` elsewhere.
fn native_handle(window: &impl raw_window_handle::HasWindowHandle) -> Option<isize> {
    match window.window_handle().ok()?.as_raw() {
        raw_window_handle::RawWindowHandle::Win32(h) => Some(h.hwnd.get()),
        _ => None,
    }
}

/// The mark in `c` at `px` pixels (premultiplied).
pub fn render(c: &MarkColours, px: u32) -> Option<resvg::tiny_skia::Pixmap> {
    use resvg::{tiny_skia, usvg};
    let tree = usvg::Tree::from_str(&mark_svg(px, c), &usvg::Options::default()).ok()?;
    let mut pix = tiny_skia::Pixmap::new(px, px)?;
    let scale = px as f32 / tree.size().width().max(tree.size().height());
    resvg::render(
        &tree,
        tiny_skia::Transform::from_scale(scale, scale),
        &mut pix.as_mut(),
    );
    Some(pix)
}

/// The mark in `c` at `px` pixels, as a PNG image.
pub fn png(c: &MarkColours, px: u32) -> Option<Vec<u8>> {
    render(c, px)?.encode_png().ok()
}

/// The mark in `c` at `px` pixels as an icon (straight RGBA, as window icons want it).
pub fn icon_data(c: &MarkColours, px: u32) -> Option<egui::IconData> {
    let pix = render(c, px)?;
    let rgba = pix
        .pixels()
        .iter()
        .flat_map(|p| {
            let s = p.demultiply();
            [s.red(), s.green(), s.blue(), s.alpha()]
        })
        .collect();
    Some(egui::IconData {
        rgba,
        width: px,
        height: px,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_mark_renders_at_every_icon_size_with_nothing_behind_it() {
        for px in [16, 20, 24, 32, 40, 48, 64, 256] {
            let pix = render(&MarkColours::ORIGINAL, px).expect("renders");
            assert_eq!((pix.width(), pix.height()), (px, px));
            // No tile: the corners are transparent, and something is drawn.
            let a = |x: u32, y: u32| pix.pixel(x, y).unwrap().alpha();
            assert_eq!(a(0, px - 1), 0, "{px}: the bottom-left corner is empty");
            assert_eq!(a(px / 2, px / 2), 0, "{px}: the C is open in the middle");
            let drawn = pix.pixels().iter().filter(|p| p.alpha() > 128).count();
            let share = drawn as f32 / (px * px) as f32;
            assert!(
                (0.12..0.6).contains(&share),
                "{px}: {:.0} % of the square is drawn",
                share * 100.0
            );
            assert!(png(&MarkColours::ORIGINAL, px)
                .unwrap()
                .starts_with(b"\x89PNG"));
        }
        let icon = icon_data(&MarkColours::from_one([0x2f, 0x7c, 0xf6]), 32).unwrap();
        assert_eq!(icon.rgba.len(), 32 * 32 * 4);
    }

    #[test]
    fn a_colour_change_is_applied_once() {
        // Off a real window (no Win32 handle) the icon goes through eframe's command.
        struct NoWindow;
        impl raw_window_handle::HasWindowHandle for NoWindow {
            fn window_handle(
                &self,
            ) -> Result<raw_window_handle::WindowHandle<'_>, raw_window_handle::HandleError>
            {
                Err(raw_window_handle::HandleError::Unavailable)
            }
        }
        let ctx = egui::Context::default();
        let mut w = WindowIcon::default();
        w.sync(&ctx, &NoWindow);
        assert_eq!(w.applied(), None, "nothing asked for, nothing done");
        let blue = MarkColours::from_one([0x2f, 0x7c, 0xf6]);
        w.want(blue);
        w.sync(&ctx, &NoWindow);
        let first = w.applied();
        assert_eq!(first.map(|a| a.0), Some(blue));
        w.sync(&ctx, &NoWindow);
        assert_eq!(w.applied(), first);
    }
}
