//! An instrument's preset list is opaque in every look, and reaches its last preset
//! (GitHub issue #1's follow-up; the owner, looking at the first after-picture: "still
//! see-through"). Its own test binary: a theme installs the kit's tokens for the process.
//!
//! Through `CantusApp::assemble` on a 2 880 × 1 800 screen at Windows' 200 %, in Slate Night,
//! Slate Paper, Slate (Pewter), Paper (Chalk), Bordeaux, Lagoon, Print, Violet Paper and High
//! contrast light, once the list's opening fade has settled:
//!
//! - every row's empty right part is **the sheet's colour, opaque** (the theme's menu colour
//!   in the unit's face, alpha 255) — nothing of the editor under it shows;
//! - a colour painted over everything beneath the pop-ups shows through none of its points;
//! - the list lies inside the window and its last preset is painted whole in it; in a
//!   window too short for it (1 280 × 720 at 200 %) it scrolls, and the wheel brings the
//!   last preset into view, still inside the window.

mod paths_rig;

use cantus_core::{InstrumentKind, InstrumentSpec, NativeSpec, Project, TrackId};
use cantus_ui::kit::appearance::Mode;
use egui::{Color32, Id, Rect, Vec2};
use paths_rig::*;

fn with_unda() -> (Project, TrackId) {
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

const LOOKS: [&str; 9] = [
    "slate_night",
    "slate_paper",
    "slate",
    "paper",
    "bordeaux",
    "lagoon",
    "print",
    "violet_paper",
    "contrast_light",
];

/// The list open and settled: its rect.
fn open_list(r: &mut Rig, track: TrackId) -> Rect {
    let shell = cantus_ui::kit::shell::Shell::new(("inst", track.0)).id;
    let well = r
        .ctx
        .read_response(shell.with(("part", 1usize)))
        .map(|x| x.rect)
        .expect("the preset well is drawn");
    r.click(well.center());
    // The opening fade, whatever the theme's animation time.
    r.idle(40);
    assert_eq!(r.app.inst.popup, cantus_ui::views::inst::Popup::Presets);
    r.ctx
        .memory(|m| m.area_rect(Id::new(("inst-popup", track.0))))
        .expect("the list is open")
}

fn close(r: &mut Rig) {
    r.key(egui::Key::Escape, NONE);
    let s = r.ctx.content_rect();
    r.click(s.center_bottom() - egui::vec2(0.0, 30.0));
    r.idle(2);
}

/// The presets' names, the factory's in order.
fn names() -> Vec<String> {
    cantus_ui::views::inst::factory_presets(InstrumentKind::Wavetable)
        .iter()
        .map(|p| cantus_ui::i18n::t(p.label_key))
        .collect()
}

#[test]
fn the_preset_list_is_opaque_in_every_look_and_reaches_its_last_preset() {
    let (project, track) = with_unda();
    let mut r = Rig::new(project);
    r.screen = Some(Physical::maximised(Vec2::new(2880.0, 1800.0), 2.0));
    r.idle(4);
    r.app.open_inst_editor(track);
    r.idle(6);
    let names = names();
    let last = names.last().expect("Unda has presets").clone();
    let mut sheets: Vec<Color32> = Vec::new();
    for look in LOOKS {
        assert!(
            cantus_ui::kit::appearance::PRESETS
                .iter()
                .any(|(id, _)| *id == look),
            "{look} is a shipped look"
        );
        r.app.appearance.mode = Mode::Dark;
        r.app.appearance.dark.base = format!("preset:{look}");
        r.app.apply_appearance();
        r.idle(6);
        let list = open_list(&mut r, track);
        r.shot(&format!("inst-presets-{look}"));
        let screen = r.ctx.content_rect().expand(0.5);
        assert!(
            screen.contains_rect(list),
            "{look}: {list:?} runs off {screen:?}"
        );
        // The sheet's colour: the theme's menu colour in Unda's face.
        let sheet = {
            let _face = cantus_ui::kit::skin::wear(cantus_ui::kit::skin::of_instrument(
                InstrumentKind::Wavetable,
            ));
            cantus_ui::kit::Style::current().t.menu
        };
        assert_eq!(
            sheet.a(),
            255,
            "{look}: the menu colour is opaque: {sheet:?}"
        );
        sheets.push(sheet);
        let c = r.canvas();
        let ppp = r.ctx.pixels_per_point();
        let rows: Vec<Rect> = r
            .texts
            .iter()
            .filter(|t| names.contains(&t.text) && list.contains_rect(t.rect))
            .map(|t| t.rect)
            .collect();
        assert!(
            rows.len() > 10,
            "{look}: the presets are listed: {}",
            rows.len()
        );
        let mut off = Vec::new();
        for row in &rows {
            // The row's empty right part, clear of the sheet's edge and any scroll bar.
            for dx in [14.0, 24.0, 40.0] {
                let (x, y) = (list.right() - dx, row.center().y);
                let p = c.pixel((x * ppp) as usize, (y * ppp) as usize);
                let d = (0..3)
                    .map(|i| (p[i] as i32 - [sheet.r(), sheet.g(), sheet.b()][i] as i32).abs())
                    .max()
                    .unwrap();
                // The classic sheet runs a ±1.5 % gradient from top to foot (`menu::paint`):
                // within that of the menu colour, never the editor beneath.
                if d > 5 {
                    off.push((x, y, p));
                }
            }
        }
        assert!(
            off.is_empty(),
            "{look}: {} of {} points under the rows are not the sheet's {sheet:?}: {:?}",
            off.len(),
            rows.len() * 3,
            &off[..off.len().min(6)]
        );
        // Nothing beneath shows through any point of it.
        const PROBE: Color32 = Color32::from_rgb(255, 0, 255);
        r.probe = Some(PROBE);
        r.idle(1);
        let c = r.canvas();
        r.probe = None;
        let inner = list.shrink(3.0);
        let mut through = 0;
        let mut y = inner.top();
        while y < inner.bottom() {
            let mut x = inner.left();
            while x < inner.right() {
                let p = c.pixel((x * ppp) as usize, (y * ppp) as usize);
                if p[0] > 230 && p[1] < 40 && p[2] > 230 {
                    through += 1;
                }
                x += 5.0;
            }
            y += 5.0;
        }
        assert_eq!(
            through, 0,
            "{look}: the probe shows through {through} points"
        );
        // Whole: its last preset painted whole inside it.
        let end = r
            .texts
            .iter()
            .find(|t| t.text == last && list.contains_rect(t.rect) && t.seen == t.rect);
        assert!(
            end.is_some(),
            "{look}: {last} is not shown whole in {list:?}"
        );
        close(&mut r);
    }
    // The looks were installed: their sheets differ.
    sheets.sort_by_key(|c| c.to_array());
    sheets.dedup();
    assert!(
        sheets.len() >= 6,
        "the looks did not change the sheet: {sheets:?}"
    );
}

#[test]
fn in_a_short_window_the_preset_list_scrolls_to_its_last_preset() {
    let (project, track) = with_unda();
    let mut r = Rig::new(project);
    r.screen = Some(Physical {
        // 1 280 × 720 pixels at 200 %: a window 640 points wide and 360 tall.
        window: Vec2::new(1280.0, 720.0),
        monitor: Vec2::new(2880.0, 1800.0),
        native: 2.0,
    });
    r.idle(4);
    r.app.open_inst_editor(track);
    r.idle(6);
    let last = names().last().unwrap().clone();
    let list = open_list(&mut r, track);
    let screen = r.ctx.content_rect().expand(0.5);
    r.shot("inst-presets-short-window");
    assert!(screen.contains_rect(list), "{list:?} runs off {screen:?}");
    let seen = |r: &Rig| {
        let list = r
            .ctx
            .memory(|m| m.area_rect(Id::new(("inst-popup", track.0))))
            .unwrap();
        r.texts
            .iter()
            .any(|t| t.text == last && list.contains_rect(t.rect) && t.seen == t.rect)
    };
    assert!(!seen(&r), "taller than the window, its foot waits below");
    for _ in 0..60 {
        if seen(&r) {
            break;
        }
        r.wheel(list.center(), -3.0, NONE);
    }
    assert!(seen(&r), "the wheel brings {last} into view");
    r.shot("inst-presets-short-window-scrolled");
    let list = r
        .ctx
        .memory(|m| m.area_rect(Id::new(("inst-popup", track.0))))
        .unwrap();
    assert!(
        screen.contains_rect(list),
        "scrolled, {list:?} runs off {screen:?}"
    );
}
