//! *Help ▸ About Cantus* (round 22): the kit's window says who made Cantus, its version,
//! its licence, the Bible translations' credits and the trademark notices, in both
//! languages; its buttons open the notices, the privacy statement and the manual; and
//! every document it points at exists in the repository.

mod support;

use cantus_ui::docs::{self, Doc};
use cantus_ui::i18n::Locale;
use cantus_ui::views::about::{self, Link};
use egui::{Pos2, Rect, Vec2};

const SIZE: Vec2 = Vec2::new(1280.0, 860.0);

/// The text shapes one frame painted, with where they are.
fn texts(out: &egui::FullOutput) -> Vec<(String, Rect)> {
    fn walk(shape: &egui::Shape, out: &mut Vec<(String, Rect)>) {
        match shape {
            egui::Shape::Text(t) => out.push((
                t.galley.text().to_owned(),
                t.galley.rect.translate(t.pos.to_vec2()),
            )),
            egui::Shape::Vec(v) => v.iter().for_each(|s| walk(s, out)),
            _ => {}
        }
    }
    let mut v = Vec::new();
    for s in &out.shapes {
        walk(&s.shape, &mut v);
    }
    v
}

/// Run one frame of the About window with `events`; the texts and what was clicked.
fn frame(
    app: &mut cantus_ui::CantusApp,
    ctx: &egui::Context,
    t: f64,
    events: Vec<egui::Event>,
) -> (Vec<(String, Rect)>, Option<Link>) {
    let input = egui::RawInput {
        screen_rect: Some(Rect::from_min_size(Pos2::ZERO, SIZE)),
        time: Some(t),
        events,
        ..Default::default()
    };
    let mut link = None;
    let mut out = ctx.run_ui(input, |ui| {
        let ctx = ui.ctx().clone();
        link = about::show(app, &ctx);
    });
    out.textures_delta.clear();
    (texts(&out), link)
}

fn joined(t: &[(String, Rect)]) -> String {
    t.iter()
        .map(|(s, _)| s.as_str())
        .collect::<Vec<_>>()
        .join(" | ")
}

#[test]
fn the_window_names_the_maker_the_version_the_licence_and_the_credits() {
    for (locale, words) in [
        (
            Locale::En,
            [
                "About Cantus",
                "Created by Luis Ángel Almazán",
                "GNU General Public License",
                "Third-party notices",
                "Privacy",
                "registered trademark of Steinberg",
                "Colossians 3:23",
            ],
        ),
        (
            Locale::Es,
            [
                "Acerca de Cantus",
                "Creado por Luis Ángel Almazán",
                "Licencia Pública General de GNU",
                "Avisos de terceros",
                "Privacidad",
                "marca registrada de Steinberg",
                "Colosenses 3:23",
            ],
        ),
    ] {
        cantus_ui::i18n::set_locale(locale);
        let mut app = support::app(cantus_core::Project::new("About"));
        let ctx = support::ctx();
        app.show_about = true;
        let mut painted = String::new();
        for i in 0..3 {
            painted = joined(&frame(&mut app, &ctx, i as f64 * 0.1, vec![]).0);
        }
        for w in words {
            assert!(painted.contains(w), "{locale:?}: {w:?} not in {painted}");
        }
        assert!(painted.contains(cantus_ui::app::APP_VERSION), "{painted}");
        assert!(painted.contains("lalmazan.com"));
        // The RVR1960's notice, word for word, in either language.
        assert!(
            painted.contains("Reina-Valera 1960 © Sociedades Bíblicas en América Latina, 1960."),
            "{painted}"
        );
        cantus_ui::i18n::set_locale(Locale::En);
    }
}

#[test]
fn its_buttons_name_the_documents_and_esc_closes_it() {
    let mut app = support::app(cantus_core::Project::new("About"));
    let ctx = support::ctx();
    app.show_about = true;
    let mut t = 0.0;
    frame(&mut app, &ctx, t, vec![]);
    t += 0.1;
    let (shown, _) = frame(&mut app, &ctx, t, vec![]);
    let click = |r: Rect| {
        let p = r.center();
        vec![
            egui::Event::PointerMoved(p),
            egui::Event::PointerButton {
                pos: p,
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: Default::default(),
            },
            egui::Event::PointerButton {
                pos: p,
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: Default::default(),
            },
        ]
    };
    for (label, doc) in [
        ("Third-party notices", Doc::Notices),
        ("Privacy", Doc::Privacy),
        ("Licence (GPL-3.0)", Doc::Licence),
        ("Manual", Doc::Manual),
        ("Report a bug", Doc::ReportBug),
    ] {
        let rect = shown
            .iter()
            .find(|(s, _)| s == label)
            .map(|(_, r)| *r)
            .unwrap_or_else(|| panic!("no {label:?} button in {}", joined(&shown)));
        t += 0.5;
        let (_, link) = frame(&mut app, &ctx, t, click(rect));
        assert_eq!(link, Some(Link::Doc(doc)), "{label}");
        assert!(app.show_about, "a link leaves the window open");
    }
    t += 0.5;
    let esc = egui::Event::Key {
        key: egui::Key::Escape,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: Default::default(),
    };
    frame(&mut app, &ctx, t, vec![esc]);
    assert!(!app.show_about, "Esc closes About");
}

#[test]
fn every_document_about_points_at_exists_in_the_repository() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let prefix = format!("{}/blob/master/", docs::REPO);
    for doc in [Doc::Notices, Doc::Privacy, Doc::Licence, Doc::Manual] {
        for locale in [Locale::En, Locale::Es] {
            let url = docs::target(doc, None, locale);
            let rel = url
                .strip_prefix(&prefix)
                .unwrap_or_else(|| panic!("{doc:?} is {url}"));
            assert!(root.join(rel).is_file(), "{rel} is missing");
        }
    }
    // The motto is read from the shipped translations, cited.
    let (text, citation) = about::motto(cantus_ui::verse::Spanish::Rvr1960);
    assert!(text.starts_with('“') && text.ends_with("…”"), "{text}");
    assert!(citation.contains("3:23"), "{citation}");
}
