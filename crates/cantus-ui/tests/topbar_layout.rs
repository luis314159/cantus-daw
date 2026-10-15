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
        .filter(|_| next().is_multiple_of(3))
        .map(|g| g.id().to_string())
        .collect();
    TopBarSettings {
        order: order.iter().map(|g| g.id().to_string()).collect(),
        hidden,
        meter_lufs: seed.is_multiple_of(2),
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
                let mut o = ctx.run_ui(Default::default(), |ui| {
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
                        // The left block, the groups (each in its slot: the bar's height,
                        // or one of its two rows) and the ⋯ key: no two overlap.
                        use transport::layout::{Slot, Span};
                        let rows = |s: Slot| match s {
                            Slot::Full => (0u8, 2u8),
                            Slot::Top => (0, 1),
                            Slot::Bottom => (1, 2),
                        };
                        let mut parts: Vec<(Span, Slot)> = vec![(l.left, Slot::Full)];
                        parts.extend(l.placed().iter().map(|p| (p.span, p.slot)));
                        parts.push((l.more, Slot::Full));
                        for (i, (a, sa)) in parts.iter().enumerate() {
                            assert!(a.x0 >= 0.0 && a.x1 <= width + 1e-3, "leaves: {what}");
                            for (b, sb) in &parts[i + 1..] {
                                let x = a.x1.min(b.x1) - a.x0.max(b.x0);
                                let (ya, yb) = (rows(*sa), rows(*sb));
                                let y = ya.0 < yb.1 && yb.0 < ya.1;
                                assert!(x <= 1e-3 || !y, "overlap: {what}");
                            }
                        }
                        assert!(l.left.width() + 1e-3 >= room.left_min, "{what}");
                        for p in l.placed() {
                            let want = wants.iter().find(|w| w.group == p.group).unwrap();
                            let g = p.group;
                            assert!(p.span.width() + 1e-3 >= want.min, "{g:?} clipped: {what}");
                            assert!(p.span.width() <= want.max + 1e-3, "{g:?} too wide: {what}");
                            assert_eq!(p.slot == Slot::Full, want.tall, "{g:?}: {what}");
                        }
                        let mut seen: Vec<Group> = l.placed().iter().map(|p| p.group).collect();
                        seen.extend_from_slice(l.folded());
                        seen.sort();
                        let mut all = shown.clone();
                        all.sort();
                        assert_eq!(seen, all, "{what}");
                        let order: Vec<Group> = l.placed().iter().map(|p| p.group).collect();
                        let kept: Vec<Group> = shown
                            .iter()
                            .copied()
                            .filter(|g| order.contains(g))
                            .collect();
                        assert_eq!(order, kept, "the owner's order: {what}");
                        // A bar of 800 px at 100 % keeps the transport.
                        if zoom <= 1.0 && shown.contains(&Group::Transport) {
                            assert!(l.span_of(Group::Transport).is_some(), "{what}");
                        }
                        checked += 1;
                    }
                });
                o.textures_delta.clear();
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
                    if let Some(mut old) = out.replace(o) {
                        old.textures_delta.clear();
                    }
                }
                let mut out = out.unwrap();
                out.textures_delta.clear();
                let (bar, l) = transport::last_layout(&ctx).expect("the bar laid itself out");
                let what = format!("{locale:?} {zoom} {px}px");
                assert!((bar.width() - size.x).abs() < 1.0, "{what}: {bar:?}");
                for p in l.placed() {
                    let (g, s) = (p.group, p.span);
                    assert!(
                        bar.left() + s.x0 >= bar.left() - 1e-3 && s.x1 <= bar.width() + 1e-3,
                        "{g:?} leaves the bar: {what}"
                    );
                }
                for c in &out.shapes {
                    if let egui::Shape::Text(t) = &c.shape {
                        let r = t.visual_bounding_rect();
                        if !r.is_finite() || r.top() > bar.bottom() || t.galley.text().is_empty() {
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

/// Each group, drawn alone in a rect of the width it asked for and the bar's height, keeps
/// every word inside that rect — in both languages, at every zoom — and its shapes too (a
/// lamp's glow or a key's drop may reach 3 pt past it, never more).
#[test]
fn every_group_keeps_to_its_own_width() {
    let (mut a, _dir) = app();
    let ctx = ctx();
    for locale in [Locale::En, Locale::Es] {
        set_locale(locale);
        for zoom in [0.75f32, 1.0, 2.0] {
            ctx.set_zoom_factor(zoom);
            for (variant, g) in (0..2).flat_map(|v| Group::ALL.map(|g| (v, g))) {
                // The second pass: LUFS on the meter, the song in pattern mode, the typing
                // keyboard on, a track armed.
                a.settings.topbar.meter_lufs = variant == 1;
                a.keyboard_piano = variant == 1;
                if a.project().pattern_mode != (variant == 1) {
                    a.run_action(cantus_ui::keymap::Action::TogglePatternMode);
                }
                let input = egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1600.0, 400.0))),
                    ..Default::default()
                };
                let mut out = ctx.run_ui(input, |ui| {
                    let w = transport::wants(&a, ui.ctx())
                        .into_iter()
                        .find(|w| w.group == g)
                        .expect("the default bar shows every group");
                    let rect = Rect::from_min_size(
                        Pos2::new(100.0, 100.0),
                        // A short group in one of the bar's two rows (as `slot_y` gives it).
                        Vec2::new(w.min, if w.tall { transport::HEIGHT } else { 24.0 }),
                    );
                    ui.ctx()
                        .data_mut(|d| d.insert_temp(egui::Id::new("group-rect"), rect));
                    transport::draw_group(g, &mut a, ui, rect);
                });
                out.textures_delta.clear();
                let rect: Rect = ctx
                    .data(|d| d.get_temp(egui::Id::new("group-rect")))
                    .unwrap();
                let what = format!("{g:?} {locale:?} {zoom} variant {variant}");
                for c in &out.shapes {
                    let (r, slack, text) = match &c.shape {
                        egui::Shape::Text(t) => {
                            (t.visual_bounding_rect(), 0.5, t.galley.text().to_string())
                        }
                        s => (s.visual_bounding_rect(), 3.0, String::new()),
                    };
                    assert!(
                        !text.contains('\u{2026}'),
                        "{what}: {text:?} was shortened: the group is too narrow for its words"
                    );
                    if !r.is_finite() || r.area() == 0.0 {
                        continue;
                    }
                    assert!(
                        rect.expand(slack).contains_rect(r),
                        "{what}: {text:?} at {r:?} leaves {rect:?}"
                    );
                }
            }
        }
    }
    ctx.set_zoom_factor(1.0);
    set_locale(Locale::En);
}

/// The groups' widths at 100 %, both languages (a record for the ADR):
/// `cargo test -p cantus-ui --test topbar_layout widths -- --ignored --nocapture`.
#[test]
#[ignore]
fn widths() {
    let (a, _dir) = app();
    let ctx = ctx();
    for locale in [Locale::En, Locale::Es] {
        set_locale(locale);
        let mut o = ctx.run_ui(Default::default(), |ui| {
            let w = transport::wants(&a, ui.ctx());
            let r = transport::room(ui.ctx(), 1920.0);
            let total: f32 = w.iter().map(|w| w.min + r.gap).sum();
            eprintln!(
                "{locale:?}: {w:?}\nroom {r:?}\ngroups {total} + left {}",
                r.left_min
            );
            for px in [800.0f32, 1280.0, 1600.0] {
                let l = transport::layout::layout(&transport::room(ui.ctx(), px), &w);
                eprintln!("{locale:?} folded at {px}: {:?}", l.folded());
            }
        });
        o.textures_delta.clear();
    }
}

/// At 1920 px and 100 %, the default bar shows every group — in English and in Spanish
/// (review 1: five folded) — and at 1280 px the most important ones stay.
#[test]
fn the_default_bar_shows_every_group_at_1920() {
    let (a, _dir) = app();
    let ctx = ctx();
    ctx.set_zoom_factor(1.0);
    for locale in [Locale::En, Locale::Es] {
        set_locale(locale);
        let mut o = ctx.run_ui(Default::default(), |ui| {
            let wants = transport::wants(&a, ui.ctx());
            let l = transport::layout::layout(&transport::room(ui.ctx(), 1920.0), &wants);
            assert!(l.folded().is_empty(), "{locale:?} folds {:?}", l.folded());
            let l = transport::layout::layout(&transport::room(ui.ctx(), 1280.0), &wants);
            for g in [Group::Transport, Group::Tempo, Group::Position, Group::Mode] {
                assert!(l.span_of(g).is_some(), "{locale:?} 1280: {g:?} folded");
            }
        });
        o.textures_delta.clear();
    }
    set_locale(Locale::En);
}

/// The clock is as wide as its widest position in the metre (review 1: 3/2 and 12/8
/// shortened it with "…" from bar 10).
#[test]
fn the_clock_reads_whole_in_every_metre() {
    let (mut a, _dir) = app();
    let ctx = ctx();
    for (n, d) in [(4u8, 4u8), (3, 2), (12, 8), (32, 2), (7, 16)] {
        a.exec(cantus_core::Command::SetTimeSignature {
            time_signature: cantus_core::TimeSignature::new(n, d),
        });
        // Bar 123, the last beat, the last tick.
        let ts = a.project().time_signature;
        let at = ts.ticks_per_bar() * 122
            + ts.ticks_per_beat() * (n as i64 - 1)
            + cantus_core::Ticks(ts.ticks_per_beat().0 - 1);
        a.seek(at);
        let mut out = ctx.run_ui(Default::default(), |ui| {
            let w = transport::wants(&a, ui.ctx())
                .into_iter()
                .find(|w| w.group == Group::Position)
                .unwrap();
            let rect =
                Rect::from_min_size(Pos2::new(50.0, 50.0), Vec2::new(w.min, transport::HEIGHT));
            transport::draw_group(Group::Position, &mut a, ui, rect);
        });
        out.textures_delta.clear();
        let texts: Vec<String> = out
            .shapes
            .iter()
            .filter_map(|c| match &c.shape {
                egui::Shape::Text(t) => Some(t.galley.text().to_string()),
                _ => None,
            })
            .collect();
        assert!(
            texts.iter().all(|t| !t.contains('\u{2026}')),
            "{n}/{d}: {texts:?}"
        );
        assert!(
            texts.iter().any(|t| t.starts_with("123.")),
            "{n}/{d}: {texts:?}"
        );
    }
}

// ---- allocations ---------------------------------------------------------------------

struct Counting;

thread_local! {
    static COUNTING: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    static ALLOCS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

unsafe impl std::alloc::GlobalAlloc for Counting {
    unsafe fn alloc(&self, l: std::alloc::Layout) -> *mut u8 {
        if COUNTING.try_with(|c| c.get()).unwrap_or_default() {
            let _ = ALLOCS.try_with(|c| c.set(c.get() + 1));
        }
        unsafe { std::alloc::System.alloc(l) }
    }
    unsafe fn dealloc(&self, p: *mut u8, l: std::alloc::Layout) {
        unsafe { std::alloc::System.dealloc(p, l) }
    }
    unsafe fn realloc(&self, p: *mut u8, l: std::alloc::Layout, n: usize) -> *mut u8 {
        if COUNTING.try_with(|c| c.get()).unwrap_or_default() {
            let _ = ALLOCS.try_with(|c| c.set(c.get() + 1));
        }
        unsafe { std::alloc::System.realloc(p, l, n) }
    }
}

#[global_allocator]
static A: Counting = Counting;

/// The bar's quietest warm frame, in allocations.
fn warm_allocs(a: &mut CantusApp, ctx: &egui::Context) -> usize {
    let input = egui::RawInput {
        screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1920.0, 300.0))),
        ..Default::default()
    };
    let mut least = usize::MAX;
    for pass in 0..12 {
        let mut n = 0;
        let mut o = ctx.run_ui(input.clone(), |ui| {
            let r = ui.max_rect();
            let top = Rect::from_min_size(r.min, Vec2::new(r.width(), transport::HEIGHT));
            let mut t = ui.new_child(egui::UiBuilder::new().max_rect(top));
            ALLOCS.with(|c| c.set(0));
            COUNTING.with(|c| c.set(true));
            transport::show(a, &mut t);
            COUNTING.with(|c| c.set(false));
            n = ALLOCS.with(|c| c.get());
        });
        o.textures_delta.clear();
        if pass >= 4 {
            least = least.min(n);
        }
    }
    least
}

/// An order of the owner's own costs the bar nothing more a frame than the default one
/// (review 1: it was resolved into a new list every frame).
#[test]
fn the_owners_order_allocates_nothing_more() {
    set_locale(Locale::En);
    let (mut a, _dir) = app();
    let ctx = ctx();
    let default = warm_allocs(&mut a, &ctx);
    let mut order: Vec<String> = Group::ALL.iter().map(|g| g.id().to_string()).collect();
    order.reverse();
    a.settings.topbar.order = order;
    let custom = warm_allocs(&mut a, &ctx);
    eprintln!("bar allocations a warm frame: default order {default}, the owner's {custom}");
    assert!(custom <= default, "default {default}, custom {custom}");
}

/// The bar's triangles at 1920 px (a record for the frame budget):
/// `cargo test -p cantus-ui --test topbar_layout bar_triangles -- --ignored --nocapture`.
#[test]
#[ignore]
fn bar_triangles() {
    set_locale(Locale::En);
    let (mut a, _dir) = app();
    let ctx = ctx();
    let input = egui::RawInput {
        screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1920.0, 300.0))),
        ..Default::default()
    };
    for _ in 0..3 {
        let mut out = ctx.run_ui(input.clone(), |ui| {
            let r = ui.max_rect();
            let top = Rect::from_min_size(r.min, Vec2::new(r.width(), transport::HEIGHT));
            let mut t = ui.new_child(egui::UiBuilder::new().max_rect(top));
            transport::show(&mut a, &mut t);
        });
        let ppp = out.pixels_per_point;
        let prims = ctx.tessellate(std::mem::take(&mut out.shapes), ppp);
        let tris: usize = prims
            .iter()
            .map(|p| match &p.primitive {
                egui::epaint::Primitive::Mesh(m) => m.indices.len() / 3,
                _ => 0,
            })
            .sum();
        out.textures_delta.clear();
        eprintln!("bar at 1920 px: {tris} triangles");
    }
}

#[test]
#[ignore]
fn group_triangles() {
    set_locale(Locale::En);
    let (mut a, _dir) = app();
    let ctx = ctx();
    for g in Group::ALL {
        let mut tris = 0;
        for _ in 0..2 {
            let mut out = ctx.run_ui(Default::default(), |ui| {
                let w = transport::wants(&a, ui.ctx())
                    .into_iter()
                    .find(|w| w.group == g)
                    .unwrap();
                let h = if w.tall { transport::HEIGHT } else { 24.0 };
                let rect = Rect::from_min_size(Pos2::new(10.0, 10.0), Vec2::new(w.min, h));
                transport::draw_group(g, &mut a, ui, rect);
            });
            let ppp = out.pixels_per_point;
            let prims = ctx.tessellate(std::mem::take(&mut out.shapes), ppp);
            tris = prims
                .iter()
                .map(|p| match &p.primitive {
                    egui::epaint::Primitive::Mesh(m) => m.indices.len() / 3,
                    _ => 0,
                })
                .sum::<usize>();
            out.textures_delta.clear();
        }
        eprintln!("{g:?}: {tris}");
    }
}
