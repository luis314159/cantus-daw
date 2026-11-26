//! The update notice and its window darken the app only while the window is open (the
//! owner, on 0.1.0-beta.16: "Se sigue viendo oscuro si doy click en cantus new version
//! available"). The notice at the bottom right laid the dialogs' full-window veil under
//! itself, so the app went dark whenever it showed — and showed again, veil and all, once
//! the window was closed with ×, *Later* or Esc; and the window sat under its own veil.
//!
//! Driven as a person drives it, through `CantusApp::assemble` and the frame loop, over the
//! real updater checking a network that is a table (GitHub's answer for a Cantus release):
//! every way the window closes leaves no veil, and the app takes clicks again.
//! `CANTUS_PATHS_SHOTS=<dir>` writes a picture of each state.

mod paths_rig;

use cantus_update::net::{Answer, Fetch, NetError};
use cantus_update::{Config, InstallKind, Status, Target, Updater, Version};
use egui::{Key, Pos2, Rect, Shape};
use paths_rig::*;
use std::path::Path;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// A Cantus release as GitHub answers for it (`cantus-update`'s recorded fixture).
const RELEASE: &str = include_str!("../../cantus-update/tests/fixtures/cantus-latest.json");
const PAGE: &str = "https://github.com/luis314159/cantus-daw/releases/tag/v0.2.0";
const NOTICE: &str = "Cantus 0.2.0 is available";

/// GitHub, offering 0.2.0; its downloads never arrive (an offline network).
struct Github;

impl Fetch for Github {
    fn get(&self, url: &str, _etag: Option<&str>) -> Result<Answer, NetError> {
        assert!(url.ends_with("/releases/latest"), "asked {url}");
        Ok(Answer {
            status: 200,
            etag: None,
            rate_reset: None,
            body: RELEASE.as_bytes().to_vec(),
        })
    }

    fn download(
        &self,
        _url: &str,
        _dest: &Path,
        _progress: &dyn Fn(u64, Option<u64>),
        _cancel: &AtomicBool,
    ) -> Result<(), NetError> {
        Err(NetError::Network("offline".into()))
    }
}

/// The pages *Release page* opened (instead of the browser).
static OPENED: Mutex<Vec<String>> = Mutex::new(Vec::new());

fn record(url: &str) {
    OPENED.lock().unwrap().push(url.to_owned());
}

/// An installed copy's updater, over its own state, has found 0.2.0: the notice shows.
fn offered(r: &mut Rig, state: &Path) {
    let config = Config {
        repo: cantus_update::updater::REPO.into(),
        current: Version::parse("0.1.0").unwrap(),
        kind: InstallKind::Installed,
        target: Some(Target::WindowsX64),
        state_path: state.join("update.json"),
        downloads: state.join("updates"),
        publisher: None,
        held_retry: Duration::from_millis(10),
    };
    let u = Updater::new(config, Arc::new(Github));
    u.check(true);
    r.app.update.attach(u, InstallKind::Installed);
    r.app.update.opener = record;
    for _ in 0..500 {
        r.idle(1);
        if r.text(NOTICE).is_some() {
            r.settle();
            return;
        }
        std::thread::sleep(Duration::from_millis(4));
    }
    panic!(
        "the notice never showed; status {:?}",
        r.app.update.status()
    );
}

/// Does `s` paint a see-through veil over the whole window?
fn veil(s: &Shape, screen: Rect) -> bool {
    match s {
        Shape::Rect(rs) => {
            let a = rs.fill.a();
            rs.rect.expand(0.5).contains_rect(screen) && a > 0 && a < 255
        }
        Shape::Vec(v) => v.iter().any(|s| veil(s, screen)),
        _ => false,
    }
}

/// Where in the last frame's paint order full-window veils were drawn.
fn veils(r: &Rig) -> Vec<usize> {
    let screen = Rect::from_min_size(Pos2::ZERO, r.size);
    r.shapes()
        .iter()
        .enumerate()
        .filter(|(_, c)| veil(&c.shape, screen))
        .map(|(i, _)| i)
        .collect()
}

fn says(s: &Shape, word: &str) -> bool {
    match s {
        Shape::Text(t) => t.galley.text().trim() == word,
        Shape::Vec(v) => v.iter().any(|s| says(s, word)),
        _ => false,
    }
}

/// Where in the last frame's paint order `word` was drawn last.
fn painted_at(r: &Rig, word: &str) -> Option<usize> {
    r.shapes().iter().rposition(|c| says(&c.shape, word))
}

/// The window is open: its title shows, the notice does not, and one veil lies under it.
fn open_and_veiled(r: &Rig, how: &str) {
    assert!(r.app.update.open, "{how}: the window is open");
    assert!(r.text(NOTICE).is_none(), "{how}: the notice hides");
    let title = painted_at(r, "Updates").expect("the window's title is painted");
    let v = veils(r);
    assert_eq!(
        v.len(),
        1,
        "{how}: one veil while the window is open, got {v:?}"
    );
    assert!(
        title > v[0],
        "{how}: the window is painted over its veil (title at {title}, veil at {})",
        v[0]
    );
}

/// The window is closed and nothing darkens the app.
fn closed_and_clear(r: &Rig, how: &str) {
    assert!(!r.app.update.open, "{how}: the window closed");
    assert!(r.text("Updates").is_none(), "{how}: no window");
    assert_eq!(
        veils(r),
        Vec::<usize>::new(),
        "{how}: no veil once the window is closed"
    );
}

/// The window's × (its title row's close key).
fn close_key(r: &mut Rig) -> Rect {
    let win = r
        .ctx
        .memory(|m| m.area_rect(egui::Id::new("cantus-update")))
        .expect("the window's place");
    let head = Rect::from_min_max(win.min, Pos2::new(win.max.x, win.min.y + 44.0));
    r.control_named(head, "Close")
}

#[derive(Copy, Clone, Debug)]
enum Way {
    Cross,
    Later,
    Escape,
    Skip,
    /// *Download*, which keeps the window, then Esc.
    Download,
    /// *Release page*, which keeps the window, then *Later*.
    Page,
}

#[test]
fn the_update_notice_and_window_leave_no_dark_veil() {
    let mut r = Rig::new(cantus_ui::welcome_project());
    r.begin(Pos2::new(700.0, 300.0));
    assert_eq!(veils(&r), Vec::<usize>::new(), "no veil at rest");
    for way in [
        Way::Cross,
        Way::Later,
        Way::Escape,
        Way::Skip,
        Way::Download,
        Way::Page,
    ] {
        let how = format!("{way:?}");
        let state = tempfile::tempdir().unwrap();
        offered(&mut r, state.path());
        r.shot(&format!("update-dim-{how}-1-notice"));
        assert!(!r.app.update.open);
        assert_eq!(
            veils(&r),
            Vec::<usize>::new(),
            "{how}: the notice alone darkens nothing"
        );
        let notice = r.must(NOTICE);
        r.click(notice.center());
        r.shot(&format!("update-dim-{how}-2-open"));
        open_and_veiled(&r, &how);
        // A second click where the notice was lays no second veil.
        r.click(notice.center());
        open_and_veiled(&r, &format!("{how}, clicked twice"));
        match way {
            Way::Cross => {
                let x = close_key(&mut r);
                r.click(x.center());
            }
            Way::Later => r.click(r.must("Later").center()),
            Way::Escape => r.key(Key::Escape, NONE),
            Way::Skip => r.click(r.must("Skip this version").center()),
            Way::Download => {
                r.click(r.must("Download").center());
                open_and_veiled(&r, "after Download");
                assert!(
                    !matches!(r.app.update.status(), Status::Available(_)),
                    "Download started: {:?}",
                    r.app.update.status()
                );
                r.key(Key::Escape, NONE);
            }
            Way::Page => {
                OPENED.lock().unwrap().clear();
                r.click(r.must("Release page").center());
                assert_eq!(*OPENED.lock().unwrap(), vec![PAGE.to_owned()]);
                open_and_veiled(&r, "after Release page");
                r.click(r.must("Later").center());
            }
        }
        r.shot(&format!("update-dim-{how}-3-closed"));
        closed_and_clear(&r, &how);
        // The notice comes back after ×, Later and Esc (still a newer Cantus), veil-less.
        let back = r.text(NOTICE).is_some();
        match way {
            Way::Cross | Way::Later | Way::Escape | Way::Page => {
                assert!(back, "{how}: the notice is back")
            }
            Way::Skip => assert!(!back, "{how}: a skipped version is not offered"),
            Way::Download => {}
        }
        // The app takes clicks again: the Help menu opens.
        r.click(r.must("Help").center());
        r.must("Check for updates…");
        r.key(Key::Escape, NONE);
        assert!(
            r.text("Check for updates…").is_none(),
            "{how}: the menu closed"
        );
        assert_eq!(veils(&r), Vec::<usize>::new(), "{how}: no veil at the end");
    }
}

/// The words painted after the window's last word ("Later") that fall on the window: what
/// draws over it.
fn over_the_window(r: &Rig) -> Vec<String> {
    let win = r
        .ctx
        .memory(|m| m.area_rect(egui::Id::new("cantus-update")))
        .expect("the window's place");
    let last = painted_at(r, "Later").expect("the window's buttons");
    let mut out = Vec::new();
    for (i, c) in r.shapes().iter().enumerate().skip(last + 1) {
        let mut words = Vec::new();
        texts_in(&c.shape, c.clip_rect, &mut words);
        for (w, at) in words {
            if at.intersects(win) {
                out.push(format!("#{i} {w:?} at {at:?}"));
            }
        }
    }
    out
}

fn texts_in(s: &Shape, clip: Rect, out: &mut Vec<(String, Rect)>) {
    match s {
        Shape::Text(t) => {
            let at = t.visual_bounding_rect().intersect(clip);
            if at.is_positive() {
                out.push((t.galley.text().to_owned(), at));
            }
        }
        Shape::Vec(v) => v.iter().for_each(|s| texts_in(s, clip, out)),
        _ => {}
    }
}

/// Nothing draws over the Updates window (the owner, beta.16: the stage companion's
/// "Neon Tide · 124 BPM" song chip floated over it — the companion is a see-through window
/// of its own, always on top, so it is held to `keeps_on_top`), on the project the program
/// opens on, wherever the pointer rests.
#[test]
fn nothing_draws_over_the_updates_window() {
    let mut r = Rig::new(cantus_ui::demos::startup_project());
    r.begin(Pos2::new(700.0, 300.0));
    let on_top = |r: &Rig| cantus_ui::stage::companion::keeps_on_top(&r.app.settings.stage, &r.ctx);
    assert!(
        r.app.settings.stage.on_top,
        "the companion is on top by default"
    );
    assert!(on_top(&r), "and stays there with no dialog up");
    let state = tempfile::tempdir().unwrap();
    offered(&mut r, state.path());
    assert!(on_top(&r), "the notice is no dialog");
    let notice = r.must(NOTICE);
    r.click(notice.center());
    assert!(r.app.update.open);
    assert!(
        !on_top(&r),
        "the companion steps back while the window is open"
    );
    let win = r
        .ctx
        .memory(|m| m.area_rect(egui::Id::new("cantus-update")))
        .unwrap();
    let mut seen = Vec::new();
    for at in [
        Pos2::new(200.0, 300.0),
        win.center(),
        Pos2::new(win.left() + 40.0, win.top() + 15.0),
        Pos2::new(560.0, 20.0),
        Pos2::new(60.0, 30.0),
        notice.center(),
    ] {
        r.hover(at);
        r.settle();
        assert!(!on_top(&r), "still behind, the pointer at {at:?}");
        let over = over_the_window(&r);
        if !over.is_empty() {
            seen.push(format!("pointer at {at:?}: {over:?}"));
        }
    }
    assert!(seen.is_empty(), "drawn over the Updates window: {seen:#?}");
    r.key(Key::Escape, NONE);
    assert!(!r.app.update.open);
    assert!(on_top(&r), "in front again once the window has closed");
}
