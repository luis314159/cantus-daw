//! Round 26, the final review's look findings, through the whole window: a menu's colour
//! row inside its panel, the layout's name dialog a kit dialog in the middle of the window,
//! the hint bar with room for its words at a laptop's size, the strip rename prefilled and
//! clear of its neighbours, the Look tab's colour names whole in Spanish.

use cantus_engine::EngineHandle;
use cantus_ui::{starter_project, CantusApp};
use egui::{pos2, vec2, Event, Modifiers, Pos2};

struct Rig {
    ctx: egui::Context,
    app: CantusApp,
    size: egui::Vec2,
    frame: u64,
    pointer: Option<Pos2>,
    _dir: tempfile::TempDir,
}

impl Rig {
    fn new(size: egui::Vec2) -> Rig {
        cantus_ui::i18n::set_locale(cantus_ui::i18n::Locale::En);
        let ctx = egui::Context::default();
        cantus_ui::design::install_fonts(&ctx);
        cantus_ui::design::install_style(&ctx);
        cantus_ui::theme::apply(&cantus_ui::theme::slate(), &ctx);
        let dir = tempfile::tempdir().unwrap();
        let mut app = CantusApp::from_parts(Some(starter_project()), EngineHandle::dummy(), false);
        app.settings_path = dir.path().join("settings.json");
        app.layouts_path = dir.path().join("layouts.json");
        app.appearance_path = dir.path().join("appearance.json");
        app.keymap_path = dir.path().join("keymap.toml");
        Rig {
            ctx,
            app,
            size,
            frame: 0,
            pointer: None,
            _dir: dir,
        }
    }

    /// One frame; every painted text with where it lies.
    fn frame(&mut self, events: Vec<Event>) -> Vec<(String, egui::Rect)> {
        self.frame += 1;
        let mut ev = Vec::new();
        if let Some(p) = self.pointer {
            ev.push(Event::PointerMoved(p));
        }
        ev.extend(events);
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(Pos2::ZERO, self.size)),
            time: Some(self.frame as f64 / 60.0),
            predicted_dt: 1.0 / 60.0,
            events: ev,
            ..Default::default()
        };
        let app = &mut self.app;
        let mut out = self.ctx.run_ui(input, |ui| app.frame(ui));
        out.textures_delta.clear();
        fn walk(shape: &egui::Shape, out: &mut Vec<(String, egui::Rect)>) {
            match shape {
                // A galley cut short by its rows says so (its text is still the whole).
                egui::Shape::Text(t) => out.push((
                    format!(
                        "{}{}",
                        t.galley.text(),
                        if t.galley.elided { "…" } else { "" }
                    ),
                    t.galley.rect.translate(t.pos.to_vec2()),
                )),
                egui::Shape::Vec(v) => v.iter().for_each(|s| walk(s, out)),
                _ => {}
            }
        }
        let mut v = Vec::new();
        for c in &out.shapes {
            walk(&c.shape, &mut v);
        }
        v
    }

    fn idle(&mut self, n: usize) -> Vec<(String, egui::Rect)> {
        let mut t = Vec::new();
        for _ in 0..n {
            t = self.frame(Vec::new());
        }
        t
    }

    fn click(&mut self, p: Pos2, button: egui::PointerButton) {
        self.pointer = Some(p);
        self.frame(Vec::new());
        for pressed in [true, false] {
            self.frame(vec![Event::PointerButton {
                pos: p,
                button,
                pressed,
                modifiers: Modifiers::NONE,
            }]);
        }
    }
}

/// Visual 2: the clip menu's colour swatches lie inside a menu panel, not on the window
/// beside it.
#[test]
fn the_clip_menus_colour_row_is_inside_the_menu() {
    let mut r = Rig::new(vec2(1600.0, 900.0));
    r.idle(3);
    cantus_ui::kit::inspect::set_enabled(&r.ctx, true);
    let g = r.app.arrangement.geometry().expect("the timeline");
    let at = pos2(g.x(1.0), g.rows[0].top + 8.0);
    r.click(at, egui::PointerButton::Secondary);
    r.pointer = Some(pos2(5.0, 5.0));
    r.idle(3);
    let items = cantus_ui::kit::inspect::items(&r.ctx);
    let menus: Vec<egui::Rect> = items
        .iter()
        .filter(|i| i.part == "menu")
        .map(|i| i.rect)
        .collect();
    let swatches: Vec<egui::Rect> = items
        .iter()
        // The menu's (the toolbar's colour key, above the timeline, is a swatch too).
        .filter(|i| i.part == "swatch" && i.rect.top() > at.y)
        .map(|i| i.rect)
        .collect();
    assert!(!menus.is_empty(), "the clip menu opened");
    assert!(swatches.len() >= 9, "the colour row: {swatches:?}");
    for s in &swatches {
        assert!(
            menus.iter().any(|m| m.expand(0.5).contains_rect(*s)),
            "a swatch {s:?} outside every menu panel {menus:?}"
        );
    }
}

/// The clip browser's paint source says so when pointed at (`links.browser.chosen`); the
/// other tiles still say how to drag them.
#[test]
fn the_chosen_tile_says_it_is_the_paint_source() {
    let mut r = Rig::new(vec2(1600.0, 900.0));
    r.app.show_pane(cantus_ui::views::workspace::Pane::Clips);
    r.idle(3);
    let t0 = r.app.project().tracks[0].id;
    let c0 = r.app.project().tracks[0].clips[0].id;
    r.app.clip_browser.set_brush(Some((t0, c0)));
    r.idle(2);
    let tiles = r.app.clip_browser.tile_rects().to_vec();
    let chosen = tiles
        .iter()
        .find(|(k, _)| *k == (t0, c0))
        .expect("the chosen clip's tile")
        .1;
    r.pointer = Some(chosen.center());
    r.idle(2);
    let hint = cantus_ui::kit::hint::current(&r.ctx).expect("a hint");
    assert_eq!(
        hint.value,
        cantus_ui::i18n::t("links.browser.chosen"),
        "the paint source's hover: {hint:?}"
    );
}

/// Visual 4: the layout's name dialog is a kit dialog in the middle of the window — its
/// title without the menu's "…", its buttons the kit's (Save lit) — in both languages.
#[test]
fn the_layout_name_dialog_is_a_kit_dialog_in_the_middle() {
    for locale in [cantus_ui::i18n::Locale::En, cantus_ui::i18n::Locale::Es] {
        let mut r = Rig::new(vec2(1600.0, 900.0));
        cantus_ui::i18n::set_locale(locale);
        r.idle(3);
        cantus_ui::kit::inspect::set_enabled(&r.ctx, true);
        r.app.naming_layout = Some("Mix A".into());
        let texts = r.idle(4);
        let title = cantus_ui::i18n::t("layout.dialog_title");
        let (_, at) = texts
            .iter()
            .find(|(s, _)| *s == title)
            .unwrap_or_else(|| panic!("{locale:?}: no title {title:?} painted"));
        assert!(!title.contains('…'), "{title}");
        let c = at.center();
        assert!(
            (c.x - 800.0).abs() < 260.0 && (c.y - 450.0).abs() < 220.0,
            "{locale:?}: the dialog's title at {c:?}, not near the middle"
        );
        let buttons = cantus_ui::kit::inspect::items(&r.ctx)
            .into_iter()
            .filter(|i| i.part == "audio-button" && (i.rect.center().y - c.y).abs() < 200.0)
            .count();
        assert!(
            buttons >= 2,
            "{locale:?}: Save and Cancel are the kit's buttons"
        );
    }
    cantus_ui::i18n::set_locale(cantus_ui::i18n::Locale::En);
}

/// Visual 5: at a laptop's 1280 × 720 the hint bar shows a long hint whole (on two lines
/// where one is too short) — the band handle's reason when the desk holds the band, in both
/// languages, its advice first.
#[test]
fn a_long_hint_reads_whole_at_1280_by_720() {
    for locale in [cantus_ui::i18n::Locale::En, cantus_ui::i18n::Locale::Es] {
        let mut r = Rig::new(vec2(1280.0, 720.0));
        cantus_ui::i18n::set_locale(locale);
        r.app.central = cantus_ui::app::CentralView::Mixer;
        let mut p = r.app.mixer.prefs();
        p.band = cantus_ui::views::mixer::layout::BAND_MAX;
        r.app.mixer.set_prefs(p);
        r.idle(3);
        let handle = r
            .ctx
            .read_response(egui::Id::new("mixer-band-size"))
            .expect("the band's handle")
            .rect;
        r.pointer = Some(handle.center());
        let texts = r.idle(3);
        let want = cantus_ui::i18n::t("cables.hint.band_short");
        let flat = |s: &str| s.split_whitespace().collect::<Vec<_>>().join(" ");
        assert!(
            texts.iter().any(|(s, _)| flat(s) == flat(&want)),
            "{locale:?}: {want:?} not whole in the hint bar: {:?}",
            texts
                .iter()
                .filter(|(_, at)| at.top() < 60.0)
                .map(|(s, _)| s.as_str())
                .collect::<Vec<_>>()
        );
    }
    cantus_ui::i18n::set_locale(cantus_ui::i18n::Locale::En);
}

/// Visual 8: the Look tab's colours with a meaning name themselves whole in both languages
/// at 1280 × 720 (Spanish ones were cut mid-word with room beside them).
#[test]
fn the_colours_with_a_meaning_are_named_whole() {
    const ROLES: [&str; 12] = [
        "kit.token.accent.selection",
        "kit.token.accent.highlight",
        "kit.token.accent.focus",
        "kit.token.accent.led_on",
        "kit.token.accent.record",
        "kit.token.accent.brand",
        "kit.token.steps.odd_lit",
        "kit.token.steps.even_lit",
        "kit.token.meters.low",
        "kit.token.meters.high",
        "kit.token.meters.clip",
        "kit.token.text.primary",
    ];
    for locale in [cantus_ui::i18n::Locale::En, cantus_ui::i18n::Locale::Es] {
        let mut r = Rig::new(vec2(1280.0, 720.0));
        cantus_ui::i18n::set_locale(locale);
        r.app.show_theme_editor = true;
        r.app.appearance_page.tab = cantus_ui::views::appearance::Tab::Look;
        let texts = r.idle(4);
        for key in ROLES {
            let name = cantus_ui::i18n::t(key);
            assert!(
                texts.iter().any(|(s, _)| *s == name),
                "{locale:?}: {name:?} not whole; painted: {:?}",
                texts
                    .iter()
                    .filter(|(s, _)| s.ends_with('…'))
                    .map(|(s, _)| s.as_str())
                    .collect::<Vec<_>>()
            );
        }
    }
    cantus_ui::i18n::set_locale(cantus_ui::i18n::Locale::En);
}

/// The words painted inside the hint bar this frame (its panel as the kit registered it).
fn hint_bar_texts(r: &Rig, texts: &[(String, egui::Rect)]) -> Vec<String> {
    let panel = cantus_ui::kit::inspect::items(&r.ctx)
        .into_iter()
        .find(|i| i.part == "hint_panel")
        .expect("the hint bar")
        .rect;
    texts
        .iter()
        .filter(|(s, at)| !s.is_empty() && panel.expand(1.0).contains_rect(*at))
        .map(|(s, _)| s.clone())
        .collect()
}

/// Final review: every hint the desk, the arrangement, the piano roll (each tool) and the
/// rack can show reads whole in the hint bar at 1280 × 720, at 100 % and at 200 % (640 × 360
/// points), in both languages — the idle hint of each view and tool (in its essentials,
/// the whole on the bar's tooltip), and whatever the pointer finds on a sweep over the
/// window. A word cut with "…" that is not the hint's own fails.
#[test]
fn every_hint_reads_whole_at_1280_by_720() {
    use cantus_ui::app::{CentralView, EditFocus};
    use cantus_ui::views::piano_roll::Tool;
    for (locale, (w, h, step)) in [cantus_ui::i18n::Locale::En, cantus_ui::i18n::Locale::Es]
        .into_iter()
        .flat_map(|l| {
            [
                (l, (1280.0f32, 720.0f32, 24.0f32)),
                (l, (640.0, 360.0, 12.0)),
            ]
        })
    {
        let mut r = Rig::new(vec2(w, h));
        cantus_ui::i18n::set_locale(locale);
        cantus_ui::kit::inspect::set_enabled(&r.ctx, true);
        let mut known: std::collections::HashSet<String> = std::collections::HashSet::new();
        let mut cut = Vec::new();
        let mut check = |r: &Rig,
                         texts: &[(String, egui::Rect)],
                         what: &str,
                         known: &mut std::collections::HashSet<String>| {
            if let Some(h) = cantus_ui::kit::hint::current(&r.ctx) {
                known.insert(h.name.clone());
                known.insert(h.value.clone());
            }
            for s in hint_bar_texts(r, texts) {
                if s.ends_with('…') && !known.contains(&s) {
                    cut.push(format!("{what}: {s}"));
                }
            }
        };
        // The idle hint of every view and tool, the pointer off the window.
        let idle: Vec<(EditFocus, CentralView, Option<Tool>)> = vec![
            (EditFocus::Arrangement, CentralView::Mixer, None),
            (EditFocus::Mixer, CentralView::Mixer, None),
            (EditFocus::PianoRoll, CentralView::ChannelRack, None),
            (
                EditFocus::PianoRoll,
                CentralView::PianoRoll,
                Some(Tool::Draw),
            ),
            (
                EditFocus::PianoRoll,
                CentralView::PianoRoll,
                Some(Tool::Paint),
            ),
            (
                EditFocus::PianoRoll,
                CentralView::PianoRoll,
                Some(Tool::Select),
            ),
            (
                EditFocus::PianoRoll,
                CentralView::PianoRoll,
                Some(Tool::Delete),
            ),
            (
                EditFocus::PianoRoll,
                CentralView::PianoRoll,
                Some(Tool::Mute),
            ),
            (
                EditFocus::PianoRoll,
                CentralView::PianoRoll,
                Some(Tool::Slice),
            ),
            (
                EditFocus::PianoRoll,
                CentralView::PianoRoll,
                Some(Tool::Playback),
            ),
        ];
        for (focus, central, tool) in idle {
            r.app.central = central;
            if let Some(t) = tool {
                r.app.piano_roll.tool = t;
            }
            r.pointer = None;
            r.idle(2);
            r.app.focus = focus;
            let texts = r.idle(2);
            check(
                &r,
                &texts,
                &format!("{locale:?} {w}x{h} idle {focus:?} {central:?} {tool:?}"),
                &mut known,
            );
        }
        // The pointer over every part of the window, each lower view in turn — the piano
        // roll once per tool (its grid speaks for the tool), the arrangement painting once.
        let mut sweeps: Vec<(CentralView, Option<Tool>, bool)> = vec![
            (CentralView::Mixer, None, false),
            (CentralView::ChannelRack, None, false),
            (CentralView::Mixer, None, true),
        ];
        for tool in [
            Tool::Draw,
            Tool::Paint,
            Tool::Select,
            Tool::Delete,
            Tool::Mute,
            Tool::Slice,
            Tool::Playback,
        ] {
            sweeps.push((CentralView::PianoRoll, Some(tool), false));
        }
        for (central, tool, paint) in sweeps {
            r.app.show_central(central);
            if let Some(t) = tool {
                r.app.piano_roll.tool = t;
            }
            if paint != r.app.clip_browser.painting() {
                r.app.toggle_arrangement_paint();
            }
            r.idle(2);
            // The tool's own words, whole or in their essentials: the grid was hovered.
            let tool_words = tool.map(|t| {
                let key = t.hint_key();
                [
                    cantus_ui::i18n::t(key),
                    cantus_ui::i18n::t(&format!("{key}.short")),
                ]
            });
            let mut grid_seen = false;
            let mut y = h / 12.0;
            while y < h - 10.0 {
                let mut x = 4.0;
                while x < w {
                    r.pointer = Some(pos2(x, y));
                    let texts = r.frame(Vec::new());
                    if let (Some(words), Some(hh)) =
                        (&tool_words, cantus_ui::kit::hint::current(&r.ctx))
                    {
                        grid_seen |= words.contains(&hh.name);
                    }
                    check(
                        &r,
                        &texts,
                        &format!("{locale:?} {w}x{h} {central:?} at ({x}, {y})"),
                        &mut known,
                    );
                    x += step;
                }
                y += step;
            }
            // At 200 % in 720 pixels the piano roll's body has no height to point at.
            assert!(
                tool_words.is_none() || grid_seen || h < 400.0,
                "{locale:?} {w}x{h} {tool:?}: the sweep never pointed at the piano roll's grid"
            );
        }
        let mut distinct: Vec<&str> = cut
            .iter()
            .map(|c| c.split_once(": ").map_or(c.as_str(), |x| x.1))
            .collect();
        distinct.sort_unstable();
        distinct.dedup();
        assert!(
            cut.is_empty(),
            "{} cut, {} distinct: {:#?}; first: {:?}",
            cut.len(),
            distinct.len(),
            distinct,
            cut.first()
        );
    }
    cantus_ui::i18n::set_locale(cantus_ui::i18n::Locale::En);
}

/// Last pass, 2: a hint wrapped over both lines keeps its capability icons (the clip's
/// hover has a menu: its glyph stays at the bar's right).
#[test]
fn a_wrapped_hint_keeps_its_capability_icons() {
    for locale in [cantus_ui::i18n::Locale::En, cantus_ui::i18n::Locale::Es] {
        let mut r = Rig::new(vec2(1280.0, 720.0));
        cantus_ui::i18n::set_locale(locale);
        cantus_ui::kit::inspect::set_enabled(&r.ctx, true);
        r.idle(3);
        let g = r.app.arrangement.geometry().expect("the timeline");
        r.pointer = Some(pos2(g.x(2.0), g.rows[0].top + 20.0));
        r.idle(3);
        let h = cantus_ui::kit::hint::current(&r.ctx).expect("the clip's hint");
        assert!(h.caps.menu, "{locale:?}: the clip's hint has a menu: {h:?}");
        let items = cantus_ui::kit::inspect::items(&r.ctx);
        let panel = items
            .iter()
            .find(|i| i.part == "hint_panel")
            .expect("the hint bar")
            .rect;
        let icons = items
            .iter()
            .filter(|i| i.part == "hint_icon" && panel.contains_rect(i.rect))
            .count();
        assert_eq!(
            icons, 1,
            "{locale:?}: the menu's glyph in the bar ({:?})",
            h.name
        );
    }
    cantus_ui::i18n::set_locale(cantus_ui::i18n::Locale::En);
}

/// Last pass, 3: the inspector's pinned strip never runs a caption into its value
/// ("VOLUMEN+0.0 dB"), whatever the inspector's width with a side pane open, in both
/// languages: inside every readout, no two words touch.
#[test]
fn the_pinned_strip_never_runs_a_caption_into_its_value() {
    for locale in [cantus_ui::i18n::Locale::En, cantus_ui::i18n::Locale::Es] {
        let mut r = Rig::new(vec2(1280.0, 720.0));
        cantus_ui::i18n::set_locale(locale);
        cantus_ui::kit::inspect::set_enabled(&r.ctx, true);
        r.app.show_pane(cantus_ui::views::workspace::Pane::Devices);
        let mut w = 1160.0;
        while w <= 1420.0 {
            r.size = vec2(w, 720.0);
            let texts = r.idle(3);
            let items = cantus_ui::kit::inspect::items(&r.ctx);
            for well in items
                .iter()
                .filter(|i| i.part == "readout" && i.rect.left() > w * 0.6 && i.rect.top() > 360.0)
            {
                let inside: Vec<&(String, egui::Rect)> = texts
                    .iter()
                    .filter(|(s, at)| !s.trim().is_empty() && well.rect.contains_rect(*at))
                    .collect();
                for (i, a) in inside.iter().enumerate() {
                    for b in &inside[i + 1..] {
                        assert!(
                            !a.1.shrink(0.5).intersects(b.1.shrink(0.5)),
                            "{locale:?} at {w} pt: {:?} runs into {:?}",
                            a.0,
                            b.0
                        );
                    }
                }
            }
            w += 2.0;
        }
    }
    cantus_ui::i18n::set_locale(cantus_ui::i18n::Locale::En);
}

/// Review 4: the Delay row's tip and its key's hint read whole in the hint bar, in both
/// languages, at 1280 × 720 and at 200 % (the tip was cut where its advice was).
#[test]
fn the_delay_rows_hints_read_whole() {
    let flat = |s: &str| s.split_whitespace().collect::<Vec<_>>().join(" ");
    for locale in [cantus_ui::i18n::Locale::En, cantus_ui::i18n::Locale::Es] {
        for (w, h) in [(1280.0f32, 720.0f32), (640.0, 360.0)] {
            let mut r = Rig::new(vec2(w, h));
            cantus_ui::i18n::set_locale(locale);
            cantus_ui::kit::inspect::set_enabled(&r.ctx, true);
            r.pointer = None;
            r.idle(2);
            for words in [
                cantus_ui::i18n::tf("freeze.lateness.tip", &[("ms", &"96.0")]),
                cantus_ui::i18n::t("freeze.lateness.clear"),
            ] {
                cantus_ui::kit::hint::set_text(
                    &r.ctx,
                    &words,
                    "",
                    cantus_ui::kit::hint::Caps::NONE,
                );
                let texts = r.frame(Vec::new());
                let bar = hint_bar_texts(&r, &texts);
                assert!(
                    bar.iter().any(|s| flat(s) == flat(&words)),
                    "{locale:?} {w}x{h}: {words:?} not whole: {bar:?}"
                );
            }
        }
    }
    cantus_ui::i18n::set_locale(cantus_ui::i18n::Locale::En);
}
