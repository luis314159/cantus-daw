//! Round 26: the Look tab's base-theme menus carry each theme's preview swatch (ADR 0046,
//! amended), as the Appearance page's theme rows do — painted with that theme's own
//! tokens, beside its name, never over it.

use cantus_engine::EngineHandle;
use cantus_ui::kit::appearance::{Mode, PRESETS};
use cantus_ui::views::{appearance_look, appearance_panels};
use cantus_ui::{starter_project, CantusApp};
use egui::{pos2, Color32, Pos2, Rect};

fn walk(shape: &egui::Shape, verts: &mut Vec<(Pos2, Color32)>, texts: &mut Vec<(String, Rect)>) {
    match shape {
        egui::Shape::Mesh(m) => verts.extend(m.vertices.iter().map(|v| (v.pos, v.color))),
        egui::Shape::Text(t) => texts.push((
            t.galley.text().to_string(),
            t.galley.rect.translate(t.pos.to_vec2()),
        )),
        egui::Shape::Vec(v) => v.iter().for_each(|s| walk(s, verts, texts)),
        _ => {}
    }
}

#[test]
fn the_base_theme_menus_carry_each_themes_swatch() {
    for locale in cantus_ui::i18n::Locale::ALL {
        for ppp in [0.75f32, 2.0] {
            cantus_ui::i18n::set_thread_locale(locale);
            let ctx = egui::Context::default();
            cantus_ui::design::install_fonts(&ctx);
            cantus_ui::design::install_style(&ctx);
            cantus_ui::theme::apply(&cantus_ui::theme::default_theme(), &ctx);
            let mut app =
                CantusApp::from_parts(Some(starter_project()), EngineHandle::dummy(), false);
            for mode in [Mode::Dark, Mode::Light] {
                app.appearance_page.base_menu = Some((mode, pos2(60.0, 40.0)));
                let size = egui::vec2(1280.0, 1400.0) / ppp;
                let mut shapes = Vec::new();
                for frame in 0..30 {
                    let input = egui::RawInput {
                        screen_rect: Some(Rect::from_min_size(Pos2::ZERO, size)),
                        time: Some(frame as f64 / 60.0),
                        ..Default::default()
                    };
                    ctx.set_pixels_per_point(ppp);
                    let mut out = ctx.run_ui(input, |ui| {
                        appearance_panels::popups(&mut app, ui.ctx());
                    });
                    out.textures_delta.clear();
                    shapes = out.shapes;
                }
                let swatches = appearance_panels::base_menu_swatches(&ctx);
                assert_eq!(
                    swatches.len(),
                    PRESETS.len() + app.appearance.themes.len(),
                    "{locale:?} {ppp}: a swatch for every base theme"
                );
                let (mut verts, mut texts) = (Vec::new(), Vec::new());
                for c in &shapes {
                    walk(&c.shape, &mut verts, &mut texts);
                }
                for (i, (base, sw)) in swatches.iter().enumerate() {
                    assert!(sw.width() >= 24.0 && sw.height() >= 10.0, "{base}: {sw:?}");
                    for (_, other) in &swatches[i + 1..] {
                        assert!(!sw.intersects(*other), "{base}: swatches overlap");
                    }
                    // Painted with the theme's own tokens.
                    let t = appearance_look::preview_tokens(&mut app, base);
                    let inside = |c: Color32| {
                        verts
                            .iter()
                            .any(|(p, vc)| sw.expand(0.5).contains(*p) && *vc == c)
                    };
                    for (what, c) in [
                        ("ink", t.ink),
                        ("toolbar", t.toolbar),
                        ("selection", t.selection),
                    ] {
                        if !inside(c) {
                            let mut seen: Vec<Color32> = verts
                                .iter()
                                .filter(|(p, _)| sw.expand(0.5).contains(*p))
                                .map(|(_, c)| *c)
                                .collect();
                            seen.dedup();
                            panic!("{locale:?} {ppp} {base}: its {what} {c:?} not in its swatch {sw:?}: {seen:?}");
                        }
                    }
                    // Beside the names, never over them, and no name cut.
                    for (word, r) in &texts {
                        assert!(
                            !r.shrink(0.5).intersects(*sw),
                            "{base}: {word:?} under a swatch"
                        );
                        assert!(!word.contains('\u{2026}'), "{locale:?} {ppp}: {word:?} cut");
                    }
                }
            }
        }
    }
    cantus_ui::i18n::set_thread_locale(cantus_ui::i18n::Locale::En);
}

/// Review finding 9: text selected in a field — the typed level opens with its value
/// selected — reads in every theme. The kit's `text.selection` token is the fill behind
/// selected text, the primary text keeps its colour on it (4.5:1 at least, 7:1 in the
/// high-contrast pair), the fill shows on the field's well, and egui's visuals take both
/// from the theme, so every text field draws its selection with them.
#[test]
fn every_theme_selects_text_legibly() {
    use cantus_ui::kit::color::contrast;
    for (id, make) in PRESETS {
        let theme = make();
        let t = theme.kit_tokens();
        let need = if id.starts_with("high_contrast") {
            7.0
        } else {
            4.5
        };
        let c = contrast(t.text, t.text_select);
        assert!(c >= need, "{id}: text on its selection {c:.2}:1");
        let w = contrast(t.text_select, t.well);
        assert!(w >= 1.4, "{id}: the selection on the well {w:.2}:1");
        let ctx = egui::Context::default();
        cantus_ui::theme::apply(&theme, &ctx);
        let v = ctx.global_style().visuals.clone();
        assert_eq!(
            v.selection.bg_fill, t.text_select,
            "{id}: egui's selection fill"
        );
        assert_eq!(
            v.selection.stroke.color, t.text,
            "{id}: egui's selected text"
        );
    }
}

/// Second review, finding 14: under the theme knobs the selection fill stays off the
/// field's well (≥ 1.3:1) while the text on it keeps 4.5:1 — every preset, every knob at
/// its ends and halfway.
#[test]
fn the_selection_fill_shows_under_every_knob() {
    use cantus_ui::kit::color::contrast;
    use cantus_ui::kit::ThemeKnobs;
    for (id, make) in PRESETS {
        let base = make().kit_tokens();
        for v in [-1.0f32, -0.5, 0.5, 1.0] {
            for knobs in [
                ThemeKnobs {
                    brightness: v,
                    ..Default::default()
                },
                ThemeKnobs {
                    contrast: v,
                    ..Default::default()
                },
                ThemeKnobs {
                    text: v,
                    ..Default::default()
                },
                ThemeKnobs {
                    saturation: v,
                    ..Default::default()
                },
                ThemeKnobs {
                    hue: v * 180.0,
                    ..Default::default()
                },
            ] {
                let t = base.with_knobs(&knobs);
                let c = contrast(t.text, t.text_select);
                let w = contrast(t.text_select, t.well);
                assert!(c >= 4.5, "{id} {knobs:?}: text on the fill {c:.2}:1");
                assert!(w >= 1.3, "{id} {knobs:?}: the fill on the well {w:.2}:1");
            }
        }
    }
}

/// Second review, finding 13: `theme::apply` gives egui the selection of the theme it
/// applies, even while another thread applies another theme (the tokens are installed
/// process-wide; apply must not read them back).
#[test]
fn apply_gives_egui_its_own_themes_selection() {
    use cantus_ui::theme;
    let stop = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let other = {
        let stop = stop.clone();
        std::thread::spawn(move || {
            let ctx = egui::Context::default();
            let night = theme::cantus_night();
            while !stop.load(std::sync::atomic::Ordering::Relaxed) {
                theme::apply(&night, &ctx);
            }
        })
    };
    let paper = theme::cantus_paper();
    let want = paper.kit_tokens();
    let ctx = egui::Context::default();
    let mut wrong = 0;
    for _ in 0..2000 {
        theme::apply(&paper, &ctx);
        let v = ctx.global_style().visuals.clone();
        if v.selection.bg_fill != want.text_select {
            wrong += 1;
        }
    }
    stop.store(true, std::sync::atomic::Ordering::Relaxed);
    other.join().unwrap();
    assert_eq!(
        wrong, 0,
        "{wrong} of 2000 applies gave egui another theme's selection"
    );
}

/// Second review, findings 12 and 15: every text field in the views is the kit's — a
/// well of ours under a frameless `TextEdit` — so egui's focus outline (which shares its
/// colour with the selected text) is drawn nowhere; and the selection fill is named and
/// grouped as a fill, apart from every other token's name, in both languages.
#[test]
fn every_text_field_is_the_kits_and_the_fill_is_named_as_one() {
    let src = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut stack = vec![src];
    let mut bare = Vec::new();
    while let Some(dir) = stack.pop() {
        for e in std::fs::read_dir(&dir).unwrap().flatten() {
            let p = e.path();
            if p.is_dir() {
                stack.push(p);
            } else if p.extension().is_some_and(|x| x == "rs") {
                // Third review: one field for the whole app — no egui text input
                // anywhere but in the kit's field (`kit/field.rs`): no `TextEdit` (by any
                // name), no `DragValue` (its edit mode is egui's framed box), no
                // `text_edit_*` shorthand.
                if p.ends_with("kit/field.rs") {
                    continue;
                }
                let text = std::fs::read_to_string(&p).unwrap();
                for (i, l) in text.lines().enumerate() {
                    let code = l.split("//").next().unwrap_or("");
                    let bad = code.contains("TextEdit::")
                        || code.contains("TextEdit as ")
                        || code.contains("use egui::TextEdit")
                        || code.contains("DragValue")
                        || code.contains("text_edit_singleline")
                        || code.contains("text_edit_multiline");
                    if bad && !code.contains("TextEditState") {
                        bare.push(format!("{}:{}", p.display(), i + 1));
                    }
                }
            }
        }
    }
    assert!(
        bare.is_empty(),
        "text inputs outside the kit's field: {bare:#?}"
    );
    use cantus_ui::kit::{Group, Tok};
    assert_eq!(
        Tok::TextSelect.group(),
        Group::Surfaces,
        "a fill, with the surfaces"
    );
    for locale in cantus_ui::i18n::Locale::ALL {
        cantus_ui::i18n::set_thread_locale(locale);
        let name = |t: Tok| cantus_ui::i18n::t(&format!("kit.token.{}", t.key()));
        let mine = name(Tok::TextSelect);
        for &t in Tok::ALL {
            if t != Tok::TextSelect {
                assert_ne!(name(t), mine, "{locale:?}: {t:?} is named like the fill");
            }
        }
    }
    cantus_ui::i18n::set_thread_locale(cantus_ui::i18n::Locale::En);
}
