//! The top bar never overlaps and never clips (ADR 0052, BACKLOG §6e): a property test over
//! widths × interface zooms × group sets × both languages, on the layout the bar really
//! uses (its groups measured in each language), then real frames of the bar checked for a
//! rect or a text that leaves it.

use cantus_engine::EngineHandle;
use cantus_ui::app::CantusApp;
use cantus_ui::i18n::{set_locale, Locale};
use cantus_ui::settings::TopBarSettings;
use cantus_ui::views::transport::{self, layout::Group};
use cantus_ui::welcome_project;
use egui::{Pos2, Rect, Vec2};

fn app() -> (CantusApp, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let mut a = CantusApp::from_parts(Some(welcome_project()), EngineHandle::dummy(), false);
    a.settings_path = dir.path().join("settings.json");
    a.layouts_path = dir.path().join("layouts.json");
    a.keymap_path = dir.path().join("keymap.toml");
    a.appearance_path = dir.path().join("appearance.json");
    (a, dir)
}

fn ctx() -> egui::Context {
    let ctx = egui::Context::default();
    cantus_ui::design::install_fonts(&ctx);
    cantus_ui::design::install_style(&ctx);
    ctx
}

/// A deterministic shuffle and subset of the groups: `seed` 0 is the default bar.
fn arrangement(seed: u64) -> TopBarSettings {
    if seed == 0 {
        return TopBarSettings::default();
    }
    let mut z = seed.wrapping_mul(0x9E37_79B9_7F4A_7C15);
    let mut next = || {
        z ^= z >> 29;
        z = z.wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z ^= z >> 32;
        z
    };
    let mut order: Vec<Group> = Group::ALL.to_vec();
    for i in (1..order.len()).rev() {
        let j = (next() % (i as u64 + 1)) as usize;
        order.swap(i, j);
    }
    let hidden = order
        .iter()
        .filter(|_| next() % 3 == 0)
        .map(|g| g.id().to_string())
        .collect();
    TopBarSettings {
        order: order.iter().map(|g| g.id().to_string()).collect(),
        hidden,
        meter_lufs: seed % 2 == 0,
    }
}

/// Every layout the bar can take, swept: no two parts overlap, none leaves the bar, none
/// is narrower than it needs (folding never shrinks a group), and every shown group is
/// either on the bar or in the ⋯ menu — in the owner's order.
#[test]
fn no_width_zoom_language_or_group_set_overlaps_or_clips() {
    let (mut a, _dir) = app();
    let ctx = ctx();
    let mut checked = 0usize;
    for locale in [Locale::En, Locale::Es] {
        set_locale(locale);
        for zoom in [0.75f32, 0.9, 1.0, 1.25, 1.5, 1.75, 2.0] {
            ctx.set_zoom_factor(zoom);
            for seed in 0..6u64 {
                a.settings.topbar = arrangement(seed);
                let shown: Vec<Group> = transport::shown_groups(&a);
                // One frame to measure in, as the bar does.
                let _ = ctx.run_ui(Default::default(), |ui| {
                    let wants = transport::wants(&a, ui.ctx());
                    assert_eq!(
                        wants.iter().map(|w| w.group).collect::<Vec<_>>(),
                        shown,
                        "the bar's groups are the owner's, in order"
                    );
                    for px in (800..=3840).step_by(23) {
                        let width = px as f32 / zoom;
                        let room = transport::room(ui.ctx(), width);
                        let l = transport::layout::layout(&room, &wants);
                        let what = format!("{locale:?} {zoom} seed {seed} {px}px: {l:?}");
                        // The left block, the groups and the ⋯ key, left to right, apart.
                        let mut spans = vec![l.left];
                        spans.extend(l.placed().iter().map(|(_, s)| *s));
                        spans.push(l.more);
                        assert!(spans[0].x0 >= 0.0, "{what}");
                        for w in spans.windows(2) {
                            assert!(w[0].x1 <= w[1].x0 + 1e-3, "overlap: {what}");
                        }
                        assert!(l.more.x1 <= width + 1e-3, "leaves the bar: {what}");
                        assert!(
                            l.left.width() + 1e-3 >= room.left_compact.min(room.left_min),
                            "{what}"
                        );
                        for (g, s) in l.placed() {
                            let want = wants.iter().find(|w| w.group == *g).unwrap();
                            assert!(s.width() + 1e-3 >= want.min, "{g:?} clipped: {what}");
                            assert!(s.width() <= want.max + 1e-3, "{g:?} too wide: {what}");
                        }
                        let mut seen: Vec<Group> = l.placed().iter().map(|(g, _)| *g).collect();
                        seen.extend_from_slice(l.folded());
                        seen.sort();
                        let mut all = shown.clone();
                        all.sort();
                        assert_eq!(seen, all, "{what}");
                        let order: Vec<Group> = l.placed().iter().map(|(g, _)| *g).collect();
                        let kept: Vec<Group> =
                            shown.iter().copied().filter(|g| order.contains(g)).collect();
                        assert_eq!(order, kept, "the owner's order: {what}");
                        // A bar of 800 px at 100 % keeps the transport.
                        if zoom <= 1.0 && shown.contains(&Group::Transport) {
                            assert!(l.span_of(Group::Transport).is_some(), "{what}");
                        }
                        checked += 1;
                    }
                });
            }
        }
    }
    ctx.set_zoom_factor(1.0);
    set_locale(Locale::En);
    eprintln!("{checked} layouts checked");
}

/// Real frames of the bar at the shots' widths and zooms, in both languages: the layout it
/// drew is inside the bar, and so is every text it painted.
#[test]
fn the_drawn_bar_keeps_every_part_and_word_inside_it() {
    let (mut a, _dir) = app();
    let ctx = ctx();
    for locale in [Locale::En, Locale::Es] {
        set_locale(locale);
        for zoom in [0.75f32, 1.0, 2.0] {
            ctx.set_zoom_factor(zoom);
            for px in [800.0f32, 1280.0, 1920.0, 2560.0] {
                let size = Vec2::new(px, 900.0) / zoom;
                let input = egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(Pos2::ZERO, size)),
                    ..Default::default()
                };
                let mut out = None;
                for _ in 0..3 {
                    let o = ctx.run_ui(input.clone(), |ui| {
                        egui::Panel::top(egui::Id::new("toolbar"))
                            .frame(egui::Frame::NONE)
                            .exact_size(transport::HEIGHT)
                            .show(ui, |ui| transport::show(&mut a, ui));
                    });
                    out = Some(o);
                }
                let mut out = out.unwrap();
                out.textures_delta.clear();
                let (bar, l) = transport::last_layout(&ctx).expect("the bar laid itself out");
                let what = format!("{locale:?} {zoom} {px}px");
                assert!((bar.width() - size.x).abs() < 1.0, "{what}: {bar:?}");
                for (g, s) in l.placed() {
                    assert!(
                        bar.left() + s.x0 >= bar.left() - 1e-3 && s.x1 <= bar.width() + 1e-3,
                        "{g:?} leaves the bar: {what}"
                    );
                }
                for c in &out.shapes {
                    if let egui::Shape::Text(t) = &c.shape {
                        let r = t.visual_bounding_rect();
                        if r.top() > bar.bottom() || t.galley.is_empty() {
                            continue; // a popup's text, not the bar's
                        }
                        assert!(
                            r.left() >= bar.left() - 0.5 && r.right() <= bar.right() + 0.5,
                            "{what}: {:?} at {r:?} leaves {bar:?}",
                            t.galley.text()
                        );
                    }
                }
            }
        }
    }
    ctx.set_zoom_factor(1.0);
    set_locale(Locale::En);
}
