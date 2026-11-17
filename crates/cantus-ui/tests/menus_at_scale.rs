//! Menus where they belong at any scale (GitHub issue #1's follow-up: beta.14 portable on a
//! 2 880 × 1 800 screen at Windows' 200 % — *"the instrument preset selection menus are
//! opening up transparent and far away from where they should be … in the right-click
//! context menu on the mixer channels, some of the sub-menus are popping up way higher
//! than they're supposed to"*).
//!
//! Through `CantusApp::assemble`, fed as egui-winit feeds a real window (`paths_rig`'s
//! [`Physical`]: sizes in points are the pixels over the system's scale times the zoom), at
//! the system's 100 %, 125 %, 150 % and 200 %, with the zoom the screen-following scale
//! picks there (0.9 at 200 % on 2 880 px) and a zoom of the user's own on top:
//!
//! - a mixer strip's right-click menu, each of its submenus hovered (the menu scrolled to
//!   reach the ones under the window's foot): the submenu lies beside its parent, level
//!   with its row (moved up only as far as the window's foot asks), inside the window, and
//!   shows every row it has room for — no submenu squeezed into a scroll;
//! - an instrument's preset list, its ⋯ menu and its sizes: each opens at the control that
//!   opened it, inside the window, on a sheet nothing beneath shows through.

mod paths_rig;

use cantus_ui::views::mixer::{self, Control};
use cantus_ui::views::workspace::Pane;
use egui::{Color32, Id, Rect, Vec2};
use paths_rig::*;

/// The screens the user's report spans: (system scale, zoom).
const SCALES: [(f32, f32); 6] = [
    (1.0, 1.0),
    (1.25, 1.0),
    (1.5, 1.0),
    (2.0, 1.0),
    // What the screen-following scale picks on 2 880 px at 200 %: 1.875 ÷ 2 → 0.9.
    (2.0, 0.9),
    // The user's own Ctrl + on top of 200 %.
    (2.0, 1.25),
];

/// The user's screen: 2 880 × 1 800, the window maximised.
const MONITOR: Vec2 = Vec2::new(2880.0, 1800.0);

fn rig(native: f32, zoom: f32) -> Rig {
    rig_with(cantus_ui::welcome_project(), native, zoom)
}

fn rig_with(project: cantus_core::Project, native: f32, zoom: f32) -> Rig {
    let mut r = Rig::new(project);
    r.screen = Some(Physical::maximised(MONITOR, native));
    if (zoom - 1.0).abs() > 1e-3 {
        r.app.apply_ui_scale(&r.ctx.clone(), zoom);
    }
    r.idle(4);
    assert!(
        (r.ctx.pixels_per_point() - native * zoom).abs() < 1e-3,
        "drawn at {} for {native} × {zoom}",
        r.ctx.pixels_per_point()
    );
    r
}

/// Every popup shown now, by its area.
fn popups(r: &Rig) -> Vec<(Id, Rect)> {
    let layers = r.ctx.memory(|m| m.areas().visible_layer_ids());
    layers
        .into_iter()
        .filter(|l| l.order == egui::Order::Foreground)
        .filter_map(|l| r.ctx.memory(|m| m.area_rect(l.id)).map(|a| (l.id, a)))
        .filter(|(_, a)| a.width() > 40.0 && a.height() > 20.0)
        .collect()
}

fn screen(r: &Rig) -> Rect {
    r.ctx.content_rect()
}

/// The strip menu's submenu rows, by their words.
const SUBS: [&str; 7] = [
    "mixer.menu.color",
    "effect.add",
    "mixer.menu.output",
    "mixer.menu.send_to",
    "comp.duck.menu",
    "mixer.menu.dock",
    "mixer.menu.group",
];

#[test]
fn a_strip_menus_submenus_open_beside_their_rows_at_every_scale() {
    for (native, zoom) in SCALES {
        let at = format!("{native} × {zoom}");
        let mut r = rig(native, zoom);
        r.app.workspace.reveal(Pane::Mixer);
        r.idle(4);
        let strip = r.app.project().strips[1].id;
        let head = mixer::control_rect(&r.ctx, Some(strip), Control::Head)
            .unwrap_or_else(|| panic!("{at}: the strip is on the desk"));
        r.right_click(head.center());
        r.idle(2);
        let shown = popups(&r);
        assert_eq!(
            shown.len(),
            1,
            "{at}: the strip menu opened alone: {shown:?}"
        );
        let (top_id, _) = shown[0];
        for key in SUBS {
            let word = cantus_ui::i18n::t(key);
            // Scrolled into view where the window is shorter than the menu.
            let mut row = None;
            for _ in 0..40 {
                let top = r.ctx.memory(|m| m.area_rect(top_id)).unwrap();
                row = r
                    .texts
                    .iter()
                    .find(|t| t.text == word && top.contains_rect(t.rect) && t.seen == t.rect)
                    .map(|t| t.rect);
                if row.is_some() {
                    break;
                }
                r.wheel(top.center(), -1.0, NONE);
            }
            let row = row.unwrap_or_else(|| panic!("{at}: {word} is in the strip menu"));
            r.hover(row.center());
            r.idle(3);
            let top = r.ctx.memory(|m| m.area_rect(top_id)).unwrap();
            let sub = popups(&r)
                .into_iter()
                .find(|(id, _)| *id != top_id)
                .unwrap_or_else(|| panic!("{at}: {word} opened its submenu"))
                .1;
            r.shot(&format!("strip-sub-{native}x{zoom}-{key}"));
            let s = screen(&r).expand(0.5);
            assert!(
                s.contains_rect(sub),
                "{at}: {word}'s submenu {sub:?} runs off {s:?}"
            );
            // Beside its parent: at its right edge, or at its left where the right has no room.
            assert!(
                (sub.left() - top.right()).abs() <= 6.0 || (sub.right() - top.left()).abs() <= 6.0,
                "{at}: {word}'s submenu {sub:?} is not beside the menu {top:?}"
            );
            // Level with its row: the row lies within the submenu's height, and the submenu
            // rose no further than the window's foot asks.
            assert!(
                sub.top() <= row.top() + 1.0 && sub.bottom() >= row.bottom() - 1.0,
                "{at}: {word}'s submenu {sub:?} is not level with its row {row:?}"
            );
            let lowest = (row.top() - 12.0).min(s.bottom() - 8.5 - sub.height());
            assert!(
                sub.top() >= lowest - 1.0,
                "{at}: {word}'s submenu rose to {} (its row at {}, the window's foot at {})",
                sub.top(),
                row.top(),
                s.bottom()
            );
            // Whole where the window holds it: no row of it hidden behind a scroll.
            if sub.height() < s.height() - 24.0 {
                let hidden: Vec<&str> = r
                    .texts
                    .iter()
                    .filter(|t| {
                        // Painted in a clip of the submenu's own (its scroll), not whole in it.
                        sub.expand(1.0).contains_rect(t.clip)
                            && !t.clip.expand(0.5).contains_rect(t.rect)
                    })
                    .map(|t| t.text.as_str())
                    .collect();
                assert!(
                    hidden.is_empty(),
                    "{at}: {word}'s submenu {sub:?} hides rows behind a scroll: {hidden:?}"
                );
            }
        }
        r.key(egui::Key::Escape, NONE);
        r.idle(2);
    }
}

/// The welcome song with an Unda (the wavetable synth) on a track of its own.
fn with_instrument() -> (cantus_core::Project, cantus_core::TrackId) {
    use cantus_core::{InstrumentKind, InstrumentSpec, NativeSpec};
    let mut p = cantus_ui::welcome_project();
    let mut t = p.make_track("Lead");
    let strip = p.make_strip("Lead");
    t.strip = Some(strip.id);
    p.strips.push(strip);
    t.instrument = InstrumentSpec::Native(NativeSpec::new(InstrumentKind::Wavetable));
    let id = t.id;
    p.tracks.push(t);
    (p, id)
}

/// Nothing beneath shows through `rect`: with the probe's colour over everything under the
/// popups, no pixel of it inside.
fn opaque(r: &mut Rig, rect: Rect) -> Result<(), String> {
    const PROBE: Color32 = Color32::from_rgb(255, 0, 255);
    r.probe = Some(PROBE);
    r.idle(1);
    let c = r.canvas();
    r.probe = None;
    let ppp = r.ctx.pixels_per_point();
    let inner = rect.shrink(3.0);
    let mut through = 0;
    let mut seen = 0;
    let mut y = inner.top();
    while y < inner.bottom() {
        let mut x = inner.left();
        while x < inner.right() {
            let (px, py) = ((x * ppp) as usize, (y * ppp) as usize);
            if px < c.width && py < c.height {
                seen += 1;
                let p = c.pixel(px, py);
                if p[0] > 230 && p[1] < 40 && p[2] > 230 {
                    through += 1;
                }
            }
            x += 4.0;
        }
        y += 4.0;
    }
    if seen > 0 && through == 0 {
        Ok(())
    } else {
        Err(format!("{through} of {seen} points show what lies beneath"))
    }
}

#[test]
fn an_instruments_preset_list_opens_at_its_well_on_an_opaque_sheet_at_every_scale() {
    for (native, zoom) in SCALES {
        let at = format!("{native} × {zoom}");
        let (project, track) = with_instrument();
        let mut r = rig_with(project, native, zoom);
        r.app.open_inst_editor(track);
        r.idle(4);
        let shell = cantus_ui::kit::shell::Shell::new(("inst", track.0)).id;
        let part = |r: &Rig, id: Id| r.ctx.read_response(id).map(|x| x.rect);
        let popup = |r: &Rig| {
            r.ctx
                .memory(|m| m.area_rect(Id::new(("inst-popup", track.0))))
                .filter(|_| r.app.inst.popup != cantus_ui::views::inst::Popup::None)
        };
        let s = screen(&r).expand(0.5);
        // The preset well: the list under it (or over it where the foot has no room), from
        // its left edge.
        let well = part(&r, shell.with(("part", 1usize))).expect("the preset well is drawn");
        r.click(well.center());
        r.idle(2);
        let list = popup(&r).unwrap_or_else(|| panic!("{at}: the well opened the list"));
        r.shot(&format!("inst-presets-{native}x{zoom}"));
        opaque(&mut r, list).unwrap_or_else(|e| panic!("{at}: the preset list: {e}"));
        assert!(
            s.contains_rect(list),
            "{at}: the preset list {list:?} runs off {s:?}"
        );
        assert!(
            (list.top() - well.bottom()).abs() <= 3.0 || (list.bottom() - well.top()).abs() <= 3.0,
            "{at}: the preset list {list:?} is not at its well {well:?}"
        );
        assert!(
            (list.left() - well.left()).abs() <= 3.0,
            "{at}: the preset list {list:?} does not start at its well {well:?}"
        );
        r.key(egui::Key::Escape, NONE);
        r.click(s.center() + egui::vec2(0.0, s.height() * 0.45));
        r.idle(2);
        // The ⋯ menu and the sizes end at their keys' right edges.
        for (key, what) in [
            (shell.with(("part", 11usize)), "the ⋯ menu"),
            (shell.with("size"), "the sizes"),
        ] {
            let k = part(&r, key).unwrap_or_else(|| panic!("{at}: {what}' key is drawn"));
            r.click(k.center());
            r.idle(2);
            let list = popup(&r).unwrap_or_else(|| panic!("{at}: {what} opened"));
            assert!(
                s.contains_rect(list),
                "{at}: {what} {list:?} runs off {s:?}"
            );
            assert!(
                (list.top() - k.bottom()).abs() <= 3.0 || (list.bottom() - k.top()).abs() <= 3.0,
                "{at}: {what} {list:?} is not at its key {k:?}"
            );
            assert!(
                list.right() <= k.right() + 3.0 && list.right() >= k.left(),
                "{at}: {what} {list:?} does not end at its key {k:?}"
            );
            opaque(&mut r, list).unwrap_or_else(|e| panic!("{at}: {what}: {e}"));
            r.click(k.center());
            r.idle(2);
            if r.app.inst.popup != cantus_ui::views::inst::Popup::None {
                r.click(s.center() + egui::vec2(0.0, s.height() * 0.45));
                r.idle(2);
            }
        }
    }
}
