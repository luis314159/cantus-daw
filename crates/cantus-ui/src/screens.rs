//! **The screens** (ADR 0148): what a screen is to Cantus, the set of screens a desk has, a
//! window's place on it, and the rules that bring a window back onto a screen that is there.
//!
//! Everything here is in the desktop's **physical pixels** — what the system and winit
//! speak — so a place means the same whatever scale each window draws at. The functions are
//! pure: the tests feed them screens as winit lists them, and the application calls them
//! when the screens or a window's place change (an event, never a timer).

use serde::{Deserialize, Serialize};

/// A screen as the system lists it (winit's `MonitorHandle`): its name, its size and its
/// place on the desktop in physical pixels, and the system's scale on it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ScreenKey {
    /// The system's name (`\\.\DISPLAY1` on Windows, the RandR output on X11, the
    /// localized name on macOS); empty when it gives none.
    #[serde(default)]
    pub name: String,
    pub size: [u32; 2],
    pub pos: [i32; 2],
    /// The system's scale, 1.0 = 100 %.
    #[serde(default = "one")]
    pub scale: f32,
}

fn one() -> f32 {
    1.0
}

impl ScreenKey {
    pub fn new(name: &str, size: [u32; 2], pos: [i32; 2], scale: f32) -> ScreenKey {
        ScreenKey {
            name: name.to_string(),
            size,
            pos,
            scale: if scale.is_finite() && scale > 0.0 {
                scale
            } else {
                1.0
            },
        }
    }

    /// Its rectangle on the desktop.
    pub fn rect(&self) -> PxRect {
        PxRect {
            x: self.pos[0],
            y: self.pos[1],
            w: self.size[0] as i32,
            h: self.size[1] as i32,
        }
    }

    /// Wider than 21 : 9 (2.33): an ultrawide screen.
    pub fn is_wide(&self) -> bool {
        self.size[1] > 0 && self.size[0] as f32 / self.size[1] as f32 >= 2.3
    }

    /// Taller than wide.
    pub fn is_portrait(&self) -> bool {
        self.size[1] > self.size[0]
    }

    fn area(&self) -> i64 {
        self.size[0] as i64 * self.size[1] as i64
    }
}

/// A rectangle in the desktop's physical pixels.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PxRect {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}

impl PxRect {
    pub fn right(&self) -> i32 {
        self.x.saturating_add(self.w)
    }

    pub fn bottom(&self) -> i32 {
        self.y.saturating_add(self.h)
    }

    pub fn centre(&self) -> (i32, i32) {
        (self.x + self.w / 2, self.y + self.h / 2)
    }

    /// The area this and `o` share, in pixels².
    pub fn overlap(&self, o: &PxRect) -> i64 {
        let w = (self.right().min(o.right()) - self.x.max(o.x)).max(0) as i64;
        let h = (self.bottom().min(o.bottom()) - self.y.max(o.y)).max(0) as i64;
        w * h
    }

    pub fn contains(&self, p: (i32, i32)) -> bool {
        p.0 >= self.x && p.0 < self.right() && p.1 >= self.y && p.1 < self.bottom()
    }
}

/// The set of screens connected, as one string: each screen's name, size and place, sorted
/// (the system's scale left out: changing it keeps the desk the same desk).
pub fn fingerprint(screens: &[ScreenKey]) -> String {
    let mut parts: Vec<String> = screens
        .iter()
        .map(|s| {
            format!(
                "{}@{}x{}{:+}{:+}",
                s.name, s.size[0], s.size[1], s.pos[0], s.pos[1]
            )
        })
        .collect();
    parts.sort();
    parts.join(";")
}

/// A window's place: its outer rectangle on the desktop, the screen it was on, and whether
/// it was maximised or in full screen.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Place {
    pub screen: ScreenKey,
    pub rect: PxRect,
    #[serde(default)]
    pub maximised: bool,
    #[serde(default)]
    pub full_screen: bool,
}

/// Which of `now` is the screen `saved` was: the same name, size and place; else the same
/// name and size (it was moved on the desktop); else the same size and place (the system
/// renamed it — Windows numbers its displays again after a dock); else none.
pub fn find_screen(saved: &ScreenKey, now: &[ScreenKey]) -> Option<usize> {
    let same = |a: &ScreenKey, f: &dyn Fn(&ScreenKey) -> bool| {
        now.iter().position(|s| f(s) && s.size == a.size)
    };
    same(saved, &|s| s.name == saved.name && s.pos == saved.pos)
        .or_else(|| same(saved, &|s| !saved.name.is_empty() && s.name == saved.name))
        .or_else(|| same(saved, &|s| s.pos == saved.pos))
}

/// The screen holding most of `r` (the one nearest its centre when none holds any of it).
pub fn screen_of(r: &PxRect, now: &[ScreenKey]) -> Option<usize> {
    let best = now
        .iter()
        .enumerate()
        .map(|(i, s)| (i, s.rect().overlap(r)))
        .max_by_key(|(_, a)| *a)?;
    if best.1 > 0 {
        return Some(best.0);
    }
    let (cx, cy) = r.centre();
    now.iter()
        .enumerate()
        .min_by_key(|(_, s)| {
            let (sx, sy) = s.rect().centre();
            (sx as i64 - cx as i64).pow(2) + (sy as i64 - cy as i64).pow(2)
        })
        .map(|(i, _)| i)
}

/// How much of a window's title bar must lie on its screen, in points (× the screen's scale
/// in pixels): enough to take hold of it and drag it.
pub const GRIP: f32 = 64.0;
/// The title bar's height that must be on the screen, in points.
pub const TITLE: f32 = 32.0;
/// The smallest a window is made, in points.
pub const MIN_WINDOW: [f32; 2] = [480.0, 320.0];

/// `place` brought onto the screens there are now: `None` when its screen is gone (its
/// areas dock back into the main window, ADR 0148 §8); else the place on that screen, moved
/// with the screen, its size clamped to the screen and at least [`GRIP`] points of its title
/// bar's middle on it.
pub fn settle(place: &Place, now: &[ScreenKey]) -> Option<Place> {
    let i = find_screen(&place.screen, now)?;
    let s = &now[i];
    let sr = s.rect();
    let mut r = place.rect;
    // The screen moved on the desktop: the window moves with it.
    r.x = r.x.saturating_add(s.pos[0] - place.screen.pos[0]);
    r.y = r.y.saturating_add(s.pos[1] - place.screen.pos[1]);
    let min_w = (MIN_WINDOW[0] * s.scale).round() as i32;
    let min_h = (MIN_WINDOW[1] * s.scale).round() as i32;
    r.w = r.w.clamp(min_w.min(sr.w), sr.w.max(1));
    r.h = r.h.clamp(min_h.min(sr.h), sr.h.max(1));
    let grip = (GRIP * s.scale).round() as i32;
    let title = (TITLE * s.scale).round() as i32;
    // The title bar's middle `grip` pixels inside the screen across…
    let mid = r.w / 2;
    let lo = sr.x - mid + grip / 2;
    let hi = sr.right() - mid - grip / 2;
    r.x = r.x.clamp(lo.min(hi), hi.max(lo));
    // … and its top inside the screen, the title bar's height under it.
    r.y = r.y.clamp(sr.y, (sr.bottom() - title).max(sr.y));
    Some(Place {
        screen: s.clone(),
        rect: r,
        maximised: place.maximised,
        full_screen: place.full_screen,
    })
}

/// A new window's place on screen `i`: maximised, its restored size the screen's middle
/// 80 %.
pub fn fresh_place(now: &[ScreenKey], i: usize) -> Option<Place> {
    let s = now.get(i)?;
    let sr = s.rect();
    let (w, h) = ((sr.w as f32 * 0.8) as i32, (sr.h as f32 * 0.8) as i32);
    Some(Place {
        screen: s.clone(),
        rect: PxRect {
            x: sr.x + (sr.w - w) / 2,
            y: sr.y + (sr.h - h) / 2,
            w,
            h,
        },
        maximised: true,
        full_screen: false,
    })
}

/// The screen a window for the other areas goes to: the largest screen the main window is
/// not on (a laptop's own panel is the smallest, so a second window goes to the external
/// one whichever is primary); ties go to the left. `None` with one screen.
pub fn other_screen(now: &[ScreenKey], main: Option<usize>) -> Option<usize> {
    now.iter()
        .enumerate()
        .filter(|(i, _)| Some(*i) != main)
        .max_by_key(|(_, s)| (s.area(), -(s.pos[0] as i64)))
        .map(|(i, _)| i)
}

/// With three screens or more: the screens left and right of the main window's, nearest
/// first (by their centres across the desktop). Either may be `None`.
pub fn sides(now: &[ScreenKey], main: usize) -> (Option<usize>, Option<usize>) {
    let Some(m) = now.get(main) else {
        return (None, None);
    };
    let mx = m.rect().centre().0;
    let mut left = None::<(usize, i32)>;
    let mut right = None::<(usize, i32)>;
    for (i, s) in now.iter().enumerate() {
        if i == main {
            continue;
        }
        let x = s.rect().centre().0;
        if x < mx && left.is_none_or(|(_, lx)| x > lx) {
            left = Some((i, x));
        } else if x >= mx && right.is_none_or(|(_, rx)| x < rx) {
            right = Some((i, x));
        }
    }
    (left.map(|l| l.0), right.map(|r| r.0))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn laptop() -> ScreenKey {
        ScreenKey::new(r"\\.\DISPLAY1", [2880, 1800], [0, 0], 2.0)
    }
    fn external() -> ScreenKey {
        ScreenKey::new(r"\\.\DISPLAY2", [1920, 1080], [2880, 300], 1.0)
    }

    fn on_external() -> Place {
        Place {
            screen: external(),
            rect: PxRect {
                x: 3000,
                y: 400,
                w: 1600,
                h: 900,
            },
            maximised: false,
            full_screen: false,
        }
    }

    #[test]
    fn a_window_whose_screen_is_gone_has_no_place() {
        assert_eq!(settle(&on_external(), &[laptop()]), None);
    }

    #[test]
    fn a_window_whose_screen_moved_on_the_desktop_moves_with_it() {
        let mut moved = external();
        moved.pos = [-1920, 0];
        let p = settle(&on_external(), &[laptop(), moved.clone()]).unwrap();
        assert_eq!(p.screen, moved);
        assert_eq!((p.rect.x, p.rect.y), (3000 - 2880 - 1920, 100));
    }

    #[test]
    fn windows_renumbered_displays_are_found_by_size_and_place() {
        let renamed = ScreenKey::new(r"\\.\DISPLAY7", [1920, 1080], [2880, 300], 1.0);
        assert_eq!(find_screen(&external(), &[laptop(), renamed]), Some(1));
    }

    /// The placement table (ADR 0148 §8): whatever the saved rectangle, the window lands
    /// with its title bar's middle on its screen and no larger than it.
    #[test]
    fn every_settled_window_can_be_taken_by_its_title_bar() {
        let screens = [laptop(), external()];
        let cases = [
            PxRect {
                x: 3000,
                y: 400,
                w: 1600,
                h: 900,
            }, // as it was
            PxRect {
                x: 9000,
                y: 400,
                w: 1600,
                h: 900,
            }, // far right of every screen
            PxRect {
                x: -5000,
                y: -5000,
                w: 800,
                h: 600,
            }, // far up and left
            PxRect {
                x: 2900,
                y: 1370,
                w: 800,
                h: 600,
            }, // below the screen's foot
            PxRect {
                x: 2880,
                y: 300,
                w: 9000,
                h: 9000,
            }, // larger than the screen
            PxRect {
                x: 3000,
                y: 400,
                w: 10,
                h: 10,
            }, // smaller than a window can be
            PxRect {
                x: i32::MAX - 10,
                y: i32::MIN + 10,
                w: 100,
                h: 100,
            },
        ];
        for r in cases {
            let p = settle(
                &Place {
                    rect: r,
                    ..on_external()
                },
                &screens,
            )
            .unwrap();
            let s = external().rect();
            assert!(p.rect.w <= s.w && p.rect.h <= s.h, "{r:?} → {:?}", p.rect);
            assert!(p.rect.w >= 480 && p.rect.h >= 320, "{r:?} → {:?}", p.rect);
            let grip = PxRect {
                x: p.rect.x + p.rect.w / 2 - 32,
                y: p.rect.y,
                w: 64,
                h: 32,
            };
            assert_eq!(grip.overlap(&s), 64 * 32, "{r:?} → {:?}", p.rect);
        }
    }

    #[test]
    fn the_scale_alone_changing_keeps_the_desk_the_same() {
        let mut laptop150 = laptop();
        laptop150.scale = 1.5;
        assert_eq!(
            fingerprint(&[laptop(), external()]),
            fingerprint(&[external(), laptop150])
        );
        assert_ne!(
            fingerprint(&[laptop()]),
            fingerprint(&[laptop(), external()])
        );
    }

    #[test]
    fn the_other_window_goes_to_the_largest_other_screen() {
        // The laptop's panel has more pixels than a 1080p screen: still "the other" is the
        // one the main window is not on.
        let s = [laptop(), external()];
        assert_eq!(other_screen(&s, Some(0)), Some(1));
        assert_eq!(other_screen(&s, Some(1)), Some(0));
        let big = ScreenKey::new("B", [3840, 2160], [-3840, 0], 1.5);
        assert_eq!(other_screen(&[laptop(), external(), big], Some(0)), Some(2));
        assert_eq!(other_screen(&[laptop()], Some(0)), None);
    }

    #[test]
    fn three_screens_have_a_left_and_a_right() {
        let left = ScreenKey::new("L", [1920, 1080], [-1920, 0], 1.0);
        let mid = ScreenKey::new("M", [2560, 1440], [0, 0], 1.0);
        let right = ScreenKey::new("R", [1920, 1080], [2560, 0], 1.0);
        assert_eq!(
            sides(&[right.clone(), mid.clone(), left.clone()], 1),
            (Some(2), Some(0))
        );
        assert_eq!(sides(&[mid], 0), (None, None));
    }

    #[test]
    fn wide_and_portrait_screens_are_told_apart() {
        assert!(ScreenKey::new("U", [3440, 1440], [0, 0], 1.0).is_wide());
        assert!(!external().is_wide());
        assert!(ScreenKey::new("P", [1080, 1920], [0, 0], 1.0).is_portrait());
    }

    #[test]
    fn a_window_lands_on_the_screen_holding_most_of_it() {
        let s = [laptop(), external()];
        let r = PxRect {
            x: 2700,
            y: 400,
            w: 800,
            h: 600,
        };
        assert_eq!(screen_of(&r, &s), Some(1));
        let nowhere = PxRect {
            x: 99_000,
            y: 0,
            w: 10,
            h: 10,
        };
        assert_eq!(screen_of(&nowhere, &s), Some(1));
    }
}
