//! Windows of our own for the stage (round 35, ADR 0091 §2).
//!
//! On Windows the stage's companion window is a layered window of true per-pixel alpha,
//! drawn from the application's own pixels ([`Layered`], `window_layered.rs`): a GL
//! window's alpha reaches the desktop through DWM only on some drivers (on a machine where
//! one GPU draws and another shows, it arrives black), while `UpdateLayeredWindow` with
//! `ULW_ALPHA` does not depend on the GPU at all. Elsewhere the compositor honours a GL
//! window's alpha and nothing here is needed.

#[cfg(windows)]
#[path = "window_layered.rs"]
mod layered;
#[cfg(windows)]
pub use layered::{scale_at, work_area_at};
#[cfg(windows)]
pub use layered::{Layered, WinEvent, CLASS};

/// A rectangle of the screen in physical pixels: x, y, width, height.
pub type Rect = (i32, i32, i32, i32);

/// `r` made at least `min` × `min` and moved (shrunk only if it is larger, both sides by
/// the same factor: its shape kept) so that all of it lies in `area` — a monitor's work
/// area: a window restored where a monitor no longer is, or saved at another scale, or a
/// vertical 9:16 taller than a laptop's screen, stays whole where it can be seen and
/// grabbed.
pub fn fit_in(r: Rect, area: Rect, min: i32) -> Rect {
    let (ax, ay, aw, ah) = area;
    let (aw, ah) = (aw.max(1), ah.max(1));
    let (w, h) = (r.2.max(min), r.3.max(min));
    let k = (aw as f64 / w as f64).min(ah as f64 / h as f64).min(1.0);
    let w = ((w as f64 * k).floor() as i32).clamp(1, aw);
    let h = ((h as f64 * k).floor() as i32).clamp(1, ah);
    let x = r.0.clamp(ax, ax + aw - w);
    let y = r.1.clamp(ay, ay + ah - h);
    (x, y, w, h)
}

#[cfg(test)]
mod tests {
    use super::fit_in;

    const SCREEN: (i32, i32, i32, i32) = (0, 0, 1920, 1040);

    #[test]
    fn a_rectangle_on_the_screen_is_kept() {
        assert_eq!(
            fit_in((100, 200, 400, 450), SCREEN, 64),
            (100, 200, 400, 450)
        );
    }

    #[test]
    fn a_rectangle_off_the_screen_comes_back_whole() {
        // Saved on a monitor to the right that is gone, or at 150 % read at 100 %.
        assert_eq!(
            fit_in((2600, 900, 400, 450), SCREEN, 64),
            (1520, 590, 400, 450)
        );
        assert_eq!(fit_in((-900, -300, 400, 450), SCREEN, 64), (0, 0, 400, 450));
    }

    #[test]
    fn a_window_of_a_pixel_grows_to_the_least_and_a_huge_one_fits() {
        assert_eq!(fit_in((10, 10, 1, 2), SCREEN, 64), (10, 10, 64, 64));
        assert_eq!(
            fit_in((300, 300, 5000, 4000), SCREEN, 64),
            (300, 0, 1300, 1040)
        );
    }

    /// Review: a vertical 540 × 960 (points) at 125 % on a 1080p screen (1920 × 1032 of
    /// work area) is 675 × 1200: shrunk whole, still 9:16 — not 675 × 1032.
    #[test]
    fn a_shape_taller_than_the_screen_keeps_its_shape() {
        let (_, _, w, h) = fit_in((100, 0, 675, 1200), (0, 0, 1920, 1032), 64);
        assert_eq!(h, 1032);
        assert!(
            (w as f64 / h as f64 - 675.0 / 1200.0).abs() < 0.002,
            "{w} × {h}"
        );
    }

    #[test]
    fn a_monitor_left_of_the_primary_has_negative_coordinates() {
        let left = (-1280, 0, 1280, 984);
        assert_eq!(fit_in((-100, 50, 400, 450), left, 64), (-400, 50, 400, 450));
    }
}
