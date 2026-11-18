//! The top bar's menus open under their words at every window size (fx/final's menu
//! audit: with the several-screens rows, View grew taller than the room under the bar of a
//! 900-point window and egui opened it at the word's side). Its own test binary: a look is
//! installed for the process.
//!
//! Through `CantusApp::assemble`, in 1 280 × 720, 1 600 × 900 and 1 920 × 1 080 windows at
//! 100 %, and on GitHub issue #1's 2 880 × 1 800 screen at Windows' 200 %, in Slate (Night),
//! Slate Paper and High contrast light: each of File, Edit, Clip, View, Options and Help
//! opens **under its word**, over its span, inside the window, opaque and legible
//! (`popup_audit::inspect`); a menu taller than the room there opens at its head and
//! scrolls, and the wheel brings its foot into view without the menu moving.

mod paths_rig;
mod popup_audit;

use egui::{PointerButton, Rect, Vec2};
use paths_rig::*;
use popup_audit::*;

const MENUS: [&str; 6] = [
    "menu.file",
    "menu.edit",
    "menu.clip",
    "menu.view",
    "ship.menu.options",
    "menu.help",
];

/// (name, the window in physical pixels, the monitor, the system's scale).
const SIZES: [(&str, Vec2, Vec2, f32); 4] = [
    (
        "1280x720",
        Vec2::new(1280.0, 720.0),
        Vec2::new(1280.0, 720.0),
        1.0,
    ),
    (
        "1600x900",
        Vec2::new(1600.0, 900.0),
        Vec2::new(1600.0, 900.0),
        1.0,
    ),
    (
        "1920x1080",
        Vec2::new(1920.0, 1080.0),
        Vec2::new(1920.0, 1080.0),
        1.0,
    ),
    (
        "2880x1800-200",
        Vec2::new(2880.0, 1800.0 - 96.0),
        Vec2::new(2880.0, 1800.0),
        2.0,
    ),
];

/// The words seen in `sheet`, with where they are (a scroll moves them).
fn words(r: &Rig, sheet: Rect) -> Vec<(String, i32)> {
    r.texts
        .iter()
        .filter(|t| t.seen.is_positive() && sheet.contains_rect(t.seen))
        .map(|t| (t.text.trim().to_string(), t.rect.top().round() as i32))
        .filter(|(w, _)| !w.is_empty())
        .collect()
}

#[test]
fn the_top_bars_menus_open_under_their_words_at_every_size() {
    let mut r = Rig::new(cantus_ui::starter_project());
    r.idle(4);
    let mut out: Vec<Finding> = Vec::new();
    let mut seen = 0;
    let mut scrolled = 0;
    for (size, window, monitor, native) in SIZES {
        r.screen = Some(Physical {
            window,
            monitor,
            native,
        });
        r.idle(6);
        for (look, light) in LOOKS {
            wear(&mut r, look, light);
            for key in MENUS {
                let screen = size.to_string();
                let Some(bar) = cantus_ui::views::transport::last_layout(&r.ctx).map(|(b, _)| b)
                else {
                    panic!("{size}: the top bar is drawn");
                };
                let word = cantus_ui::i18n::t(key);
                let Some(at) = r
                    .texts
                    .iter()
                    .rev()
                    .find(|t| {
                        t.text.trim() == word && t.seen == t.rect && bar.contains_rect(t.rect)
                    })
                    .map(|t| t.rect)
                else {
                    // A narrow bar folds its words into the ≡ key: nothing to open here.
                    continue;
                };
                let was = floating(&r);
                r.press(at.center(), PointerButton::Primary);
                r.idle(24);
                let opened = opened_since(&r, &was);
                let Some(sheet) = opened
                    .iter()
                    .map(|(_, r)| *r)
                    .max_by(|a, b| a.area().total_cmp(&b.area()))
                else {
                    out.push(Finding {
                        look,
                        screen,
                        field: word.clone(),
                        what: "did not open".into(),
                    });
                    continue;
                };
                seen += 1;
                let under = sheet.top() >= at.bottom() - 1.0 && sheet.top() - at.bottom() <= 14.0;
                let span = sheet.left() <= at.right() && sheet.right() >= at.left();
                if !(under && span) {
                    out.push(Finding {
                        look,
                        screen: screen.clone(),
                        field: word.clone(),
                        what: format!("not under its word {at:?}: {sheet:?}"),
                    });
                }
                inspect(&mut r, &opened, sheet, look, &screen, &word, &mut out);
                r.shot(&format!("topbar-{size}-{look}-{word}"));
                // Taller than its room: it scrolls, and the wheel brings its foot up while
                // the menu stays where it is.
                let room = cantus_ui::kit::menu::room_under(&r.ctx, at, 2.0);
                if sheet.height() >= room - 4.0 {
                    scrolled += 1;
                    // It opens at its head, every time (it was scrolled to its foot the
                    // last time it was open): the wheel up shows nothing more.
                    let opened_at = words(&r, sheet);
                    for _ in 0..10 {
                        r.wheel(sheet.center(), 4.0, NONE);
                    }
                    let before = words(&r, sheet);
                    if before != opened_at {
                        out.push(Finding {
                            look,
                            screen: screen.clone(),
                            field: word.clone(),
                            what: "opened scrolled, not at its head".into(),
                        });
                    }
                    for _ in 0..40 {
                        r.wheel(sheet.center(), -4.0, NONE);
                    }
                    let after = words(&r, sheet);
                    let moved = opened_since(&r, &was)
                        .iter()
                        .map(|(_, r)| *r)
                        .max_by(|a, b| a.area().total_cmp(&b.area()));
                    if after == before {
                        out.push(Finding {
                            look,
                            screen: screen.clone(),
                            field: word.clone(),
                            what: "taller than its room and the wheel shows nothing more".into(),
                        });
                    }
                    if moved != Some(sheet) {
                        out.push(Finding {
                            look,
                            screen: screen.clone(),
                            field: word.clone(),
                            what: format!("moved while it scrolled: {moved:?} from {sheet:?}"),
                        });
                    }
                    r.shot(&format!("topbar-{size}-{look}-{word}-scrolled"));
                }
                r.key(egui::Key::Escape, NONE);
                r.idle(6);
            }
        }
    }
    assert!(seen >= 60, "only {seen} menus were opened");
    println!("{seen} menus opened, {scrolled} scrolled");
    assert!(
        out.is_empty(),
        "{} problems:\n{}",
        out.len(),
        out.iter().map(Finding::line).collect::<Vec<_>>().join("\n")
    );
}
