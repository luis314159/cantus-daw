//! Every menu of the app opens opaque, beside what opened it, its submenus beside their
//! rows (beta.14's audit, after the dropdowns: "close the gaps"). Its own test binary: a
//! look is installed for the process.
//!
//! Through `CantusApp::assemble`, one app per look — Slate (Night), Slate Paper and High
//! contrast light: the top bar's six menus (File … Help), and the right-click menus of an
//! arrangement track's header, a clip, a desk strip, an inspector parameter, a browser
//! sound and a dock tab, and the piano roll's ⋯ menu (a right-click on a note erases it, as
//! in FL: no menu there). Each is opened the way a user does and is
//!
//! - **opaque** — the probe colour painted over every layer beneath it shows through no
//!   point of its sheet; inside the window; no two rows' words overlap, none is cut at its
//!   side (`popup_audit::inspect`);
//! - **where it was asked for** — a right-click's menu at the pointer (within 24 pt of it),
//!   a key's menu under or above its key, over its span;
//! - and every submenu row is hovered (clicked when hovering opens nothing): the submenu is
//!   held to the same, and lies **beside its menu, level with its row** (raised only as far
//!   as the window's foot asks).

mod paths_rig;
mod popup_audit;

use cantus_ui::views::mixer::{self, Control};
use cantus_ui::views::workspace::Pane;
use egui::{LayerId, PointerButton, Pos2, Rect};
use paths_rig::*;
use popup_audit::*;

/// What a menu was opened from.
#[derive(Copy, Clone, Debug)]
enum Anchor {
    /// A right-click at a point: the menu opens at the pointer.
    Pointer(Pos2),
    /// A key or a word: the menu opens under it (or above it).
    Key(Rect),
}

/// A menu to open: where to click, which button, and what anchors it.
struct Target {
    at: Pos2,
    button: PointerButton,
    anchor: Anchor,
}

fn right(at: Pos2) -> Target {
    Target {
        at,
        button: PointerButton::Secondary,
        anchor: Anchor::Pointer(at),
    }
}

fn key(r: Rect) -> Target {
    Target {
        at: r.center(),
        button: PointerButton::Primary,
        anchor: Anchor::Key(r),
    }
}

/// A sound in a folder of the test's own, for the browser's file menu.
fn sound_folder() -> tempfile::TempDir {
    let d = tempfile::tempdir().unwrap();
    let frames = 4_800u32;
    let data = frames * 2;
    let mut b: Vec<u8> = Vec::new();
    b.extend_from_slice(b"RIFF");
    b.extend_from_slice(&(36 + data).to_le_bytes());
    b.extend_from_slice(b"WAVEfmt ");
    b.extend_from_slice(&16u32.to_le_bytes());
    b.extend_from_slice(&1u16.to_le_bytes());
    b.extend_from_slice(&1u16.to_le_bytes());
    b.extend_from_slice(&48_000u32.to_le_bytes());
    b.extend_from_slice(&96_000u32.to_le_bytes());
    b.extend_from_slice(&2u16.to_le_bytes());
    b.extend_from_slice(&16u16.to_le_bytes());
    b.extend_from_slice(b"data");
    b.extend_from_slice(&data.to_le_bytes());
    b.resize(b.len() + data as usize, 0);
    std::fs::write(d.path().join("audit kick.wav"), b).unwrap();
    d
}

/// Words painted whole inside `region`, the topmost.
fn words_in(r: &Rig, w: &str, region: Rect) -> Option<Rect> {
    r.texts
        .iter()
        .rev()
        .find(|t| t.text.trim() == w && t.seen == t.rect && region.contains_rect(t.rect))
        .map(|t| t.rect)
}

/// Every menu to open, by name: how to get there, and where to click.
/// How a menu is reached: the way there, and where to click (none when it cannot be).
type Find = fn(&mut Rig) -> Option<Target>;

fn targets() -> Vec<(&'static str, Find)> {
    let mut v: Vec<(&'static str, Find)> = vec![
        ("top-file", |r| bar_word(r, "menu.file")),
        ("top-edit", |r| bar_word(r, "menu.edit")),
        ("top-clip", |r| bar_word(r, "menu.clip")),
        ("top-view", |r| bar_word(r, "menu.view")),
        ("top-options", |r| bar_word(r, "ship.menu.options")),
        ("top-help", |r| bar_word(r, "menu.help")),
    ];
    v.push(("track-header", |r| {
        r.app.workspace.reveal(Pane::Arrangement);
        r.settle();
        let leaf = r.leaf_of(Pane::Arrangement)?;
        let name = r.app.project().tracks[0].name.clone();
        let at = words_in(r, &name, leaf)?;
        Some(right(at.right_center() + egui::vec2(15.0, 0.0)))
    }));
    v.push(("clip", |r| {
        r.app.workspace.reveal(Pane::Arrangement);
        r.settle();
        let leaf = r.leaf_of(Pane::Arrangement)?;
        let p = r.app.project();
        let name = p.tracks.iter().flat_map(|t| &t.clips).next()?.name.clone();
        let at = words_in(r, &name, leaf)?;
        Some(right(at.center() + egui::vec2(0.0, 14.0)))
    }));
    v.push(("piano-roll-menu", |r| {
        let t = r.app.project().tracks[0].id;
        r.app.select_track(t);
        r.app.workspace.reveal(Pane::PianoRoll);
        r.settle();
        let leaf = r.leaf_of(Pane::PianoRoll)?;
        let bar = Rect::from_min_size(leaf.min, egui::vec2(leaf.width(), 64.0));
        let k = r.control(bar, &cantus_ui::i18n::t("pianoroll.menu"))?;
        Some(key(k))
    }));
    v.push(("desk-strip", |r| {
        r.app.workspace.reveal(Pane::Mixer);
        r.settle();
        let strip = r.app.project().strips.get(1)?.id;
        let head = mixer::control_rect(&r.ctx, Some(strip), Control::Head)?;
        Some(right(head.center()))
    }));
    v.push(("inspector-parameter", |r| {
        let t = r.app.project().tracks[0].id;
        r.app.select_track(t);
        r.app.workspace.reveal(Pane::Inspector);
        r.settle();
        let leaf = r.leaf_of(Pane::Inspector)?;
        let at = words_in(r, &cantus_ui::i18n::t("param.cutoff"), leaf)
            .or_else(|| words_in(r, "Cutoff", leaf))?;
        Some(right(egui::pos2(leaf.right() - 80.0, at.center().y)))
    }));
    v.push(("browser-sound", |r| {
        r.app.workspace.reveal(Pane::Browser);
        r.settle();
        let leaf = r.leaf_of(Pane::Browser)?;
        let at = r
            .texts
            .iter()
            .rev()
            .find(|t| t.text.starts_with("audit kick") && leaf.contains_rect(t.seen))
            .map(|t| t.seen)?;
        Some(right(at.center()))
    }));
    v.push(("dock-tab", |r| {
        r.app.workspace.reveal(Pane::Mixer);
        r.settle();
        Some(right(r.tab(Pane::Mixer).center()))
    }));
    v
}

/// A word of the top bar's menus.
fn bar_word(r: &mut Rig, k: &str) -> Option<Target> {
    let bar = cantus_ui::views::transport::last_layout(&r.ctx).map(|(b, _)| b)?;
    let at = words_in(r, &cantus_ui::i18n::t(k), bar)?;
    Some(key(at.expand2(egui::vec2(6.0, 0.0))))
}

/// Shut whatever is open: Escape, then a click on the status line's empty end.
fn shut(r: &mut Rig, opened: &[(LayerId, Rect)]) {
    r.key(egui::Key::Escape, NONE);
    r.idle(6);
    let still = |r: &Rig| {
        floating(r)
            .iter()
            .any(|(l, _)| opened.iter().any(|(o, _)| o == l))
    };
    if still(r) {
        let w = r.ctx.content_rect();
        r.click(egui::pos2(w.center().x, w.bottom() - 4.0));
        r.idle(6);
    }
}

/// The menu lists drawn now.
fn lists(r: &mut Rig) -> Vec<cantus_ui::kit::widgets::Listed> {
    cantus_ui::kit::widgets::record_lists(true);
    r.idle(1);
    let l = cantus_ui::kit::widgets::take_lists();
    cantus_ui::kit::widgets::record_lists(false);
    l
}

/// Open the menu `name` and measure it and its submenus.
fn audit(
    r: &mut Rig,
    look: &'static str,
    name: &str,
    find: fn(&mut Rig) -> Option<Target>,
    out: &mut Vec<Finding>,
    log: &mut Vec<String>,
) {
    let bad = |out: &mut Vec<Finding>, field: &str, what: String| {
        out.push(Finding {
            look,
            screen: name.to_string(),
            field: field.to_string(),
            what,
        })
    };
    let Some(t) = find(r) else {
        bad(out, "", "could not be reached".into());
        return;
    };
    let was = floating(r);
    r.press(t.at, t.button);
    r.idle(24);
    let opened = opened_since(r, &was);
    if opened.is_empty() {
        bad(out, "", format!("opened nothing at {:?}", t.at));
        return;
    }
    let sheet = opened
        .iter()
        .map(|(_, r)| *r)
        .max_by(|a, b| a.area().total_cmp(&b.area()))
        .unwrap();
    match t.anchor {
        Anchor::Pointer(p) => {
            let d = sheet.distance_to_pos(p);
            if d > 24.0 {
                bad(
                    out,
                    "",
                    format!("{d:.0} pt from the pointer {p:?}: {sheet:?}"),
                );
            }
        }
        Anchor::Key(k) => {
            let under = sheet.top() >= k.bottom() - 2.0 && sheet.top() - k.bottom() <= 12.0;
            let over = sheet.bottom() <= k.top() + 2.0 && k.top() - sheet.bottom() <= 12.0;
            let span = sheet.right() >= k.left() && sheet.left() <= k.right();
            if !(under || over) || !span {
                bad(out, "", format!("not at its key {k:?}: {sheet:?}"));
            }
        }
    }
    inspect(r, &opened, sheet, look, name, "", out);
    r.shot(&format!("menu-{look}-{name}"));
    log.push(format!("{look} {name}: menu {sheet:?}"));
    // Its submenus: every row with a chevron, inside the sheet.
    let window = r.ctx.content_rect();
    let rows: Vec<(Rect, Rect)> = lists(r)
        .into_iter()
        .filter(|(l, _)| sheet.intersects(*l))
        .flat_map(|(l, subs)| subs.into_iter().map(move |s| (l, s)))
        .filter(|(_, s)| sheet.contains_rect(*s))
        .collect();
    for (parent, row) in rows {
        let label = r
            .texts
            .iter()
            .rev()
            .find(|t| row.contains_rect(t.seen) && !t.text.trim().is_empty())
            .map_or_else(String::new, |t| t.text.trim().to_string());
        let before = floating(r);
        r.hover(row.center());
        r.idle(20);
        let mut sub = opened_since(r, &before);
        if sub.is_empty() {
            r.click(row.center());
            r.idle(20);
            sub = opened_since(r, &before);
        }
        if sub.is_empty() {
            // A page of the same menu (the track header's *Height*, *Automation*, *More*):
            // the sheet shows the page in its place, held to the same.
            let now: Vec<(LayerId, Rect)> = floating(r)
                .into_iter()
                .filter(|(l, _)| opened.iter().any(|(o, _)| o == l))
                .collect();
            let Some(page) = now
                .iter()
                .map(|(_, r)| *r)
                .max_by(|a, b| a.area().total_cmp(&b.area()))
            else {
                // A row with a chevron that closes its menu and opens nothing (beta.14: the
                // track header's *Height*, *Automation* and *More* pages).
                bad(
                    out,
                    &label,
                    "its row closed the menu and opened nothing".into(),
                );
                log.push(format!("{look} {name} ▸ {label}: opened nothing"));
                let Some(t) = find(r) else { break };
                r.press(t.at, t.button);
                r.idle(24);
                continue;
            };
            inspect(r, &now, page, look, name, &label, out);
            log.push(format!("{look} {name} ▸ {label}: page {page:?}"));
            let tag: String = label
                .chars()
                .filter(|c| c.is_ascii_alphanumeric())
                .take(16)
                .collect();
            r.shot(&format!("menu-{look}-{name}-page-{tag}"));
            shut(r, &now);
            let Some(t) = find(r) else { break };
            r.press(t.at, t.button);
            r.idle(24);
            continue;
        }
        let ss = sub
            .iter()
            .map(|(_, r)| *r)
            .max_by(|a, b| a.area().total_cmp(&b.area()))
            .unwrap();
        let menu_stays = floating(r)
            .iter()
            .any(|(l, _)| opened.iter().any(|(o, _)| o == l));
        if ss.area() > 0.5 * window.area() {
            // A window (the device picker): not a menu.
            log.push(format!("{look} {name} ▸ {label}: opens a window {ss:?}"));
        } else if !menu_stays {
            // The row's own popup, in the menu's place (*Modulate…*): beside what the menu
            // was opened on, never over it.
            if ss.contains(t.at) {
                bad(
                    out,
                    &label,
                    format!("its popup {ss:?} covers what it is for, at {:?}", t.at),
                );
            }
            inspect(r, &sub, ss, look, name, &label, out);
            log.push(format!(
                "{look} {name} ▸ {label}: submenu (its popup) {ss:?}"
            ));
        } else {
            let beside_right =
                ss.left() >= parent.right() - 8.0 && ss.left() <= parent.right() + 16.0;
            let beside_left =
                ss.right() <= parent.left() + 8.0 && ss.right() >= parent.left() - 16.0;
            if !(beside_right || beside_left) {
                bad(
                    out,
                    &label,
                    format!("submenu {ss:?} not beside its menu {parent:?}"),
                );
            }
            let level = (ss.top() - row.top()).abs() <= 10.0;
            let raised_by_foot = ss.bottom() >= window.bottom() - 16.0 && ss.top() < row.top();
            if !(level || raised_by_foot) {
                bad(
                    out,
                    &label,
                    format!("submenu {ss:?} not level with its row {row:?}"),
                );
            }
            inspect(r, &sub, ss, look, name, &label, out);
            log.push(format!("{look} {name} ▸ {label}: submenu {ss:?}"));
        }
        let tag: String = label
            .chars()
            .filter(|c| c.is_ascii_alphanumeric())
            .take(16)
            .collect();
        r.shot(&format!("menu-{look}-{name}-{tag}"));
        if !menu_stays {
            // The row took the menu away: shut what it opened, open the menu again.
            shut(r, &sub);
            let Some(t) = find(r) else { break };
            r.press(t.at, t.button);
            r.idle(24);
        }
    }
    let now = floating(r);
    shut(r, &now);
}

#[test]
fn every_menu_opens_opaque_where_it_was_asked_its_submenus_beside_their_rows() {
    let sounds = sound_folder();
    let mut r = Rig::new(cantus_ui::starter_project());
    r.app.add_sample_folder(sounds.path().to_path_buf());
    r.idle(4);
    let mut out = Vec::new();
    let mut log = Vec::new();
    for (look, light) in LOOKS {
        wear(&mut r, look, light);
        for (name, find) in targets() {
            audit(&mut r, look, name, find, &mut out, &mut log);
        }
    }
    println!("{}", log.join("\n"));
    let menus = log.iter().filter(|l| l.contains(": menu ")).count();
    let subs = log
        .iter()
        .filter(|l| l.contains(": submenu ") || l.contains(": page "))
        .count();
    assert!(
        menus >= 30 && subs >= 12,
        "the audit opened only {menus} menus and {subs} submenus:\n{}",
        log.join("\n")
    );
    assert!(
        out.is_empty(),
        "{} problems in {menus} menus and {subs} submenus:\n{}",
        out.len(),
        out.iter().map(Finding::line).collect::<Vec<_>>().join("\n")
    );
}
