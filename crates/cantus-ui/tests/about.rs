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
        // The RVR1960's notice, word for word, in either language — once the build carries
        // it (its publisher's written permission, `verse::RVR1960_PERMITTED`); until then
        // About credits only the translations it shows.
        let notice = "Reina-Valera 1960 © Sociedades Bíblicas en América Latina, 1960.";
        assert_eq!(
            painted.contains(notice),
            cantus_ui::verse::RVR1960_PERMITTED,
            "{painted}"
        );
        assert!(painted.contains("Reina-Valera 1909"), "{painted}");
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

/// ADR 0042 §3: *About ▸ Credits* names who made every sound pack — CC BY packs with
/// their full credit line, CC0 packs as a courtesy line — beside the Bible translations'
/// credits, in both languages.
#[test]
fn credits_name_every_sound_pack_cc_by_in_full_beside_the_bible_translations() {
    let m = cantus_content::Manifest::parse(include_str!(
        "../../cantus-content/tests/fixtures/manifest-cc-by.json"
    ))
    .expect("the fixture manifest");
    // The credits in each language: the composed line in the reader's words, a licensor's
    // own line word for word with the licence's link after it.
    let en = [
        m.pack("fixture-grand-piano")
            .unwrap()
            .credit_line()
            .unwrap(),
        m.pack("fixture-choir").unwrap().credit_full().unwrap(),
    ];
    assert!(en[1].contains("https://creativecommons.org/licenses/by/4.0/legalcode"));
    let es = [
        "«Fixture Grand Piano V3», de Alex Example".to_string(),
        "Fixture Choir by The Example Singers (example.org/choir), CC BY 4.0 — licencia: CC BY 4.0, https://creativecommons.org/licenses/by/4.0/legalcode".to_string(),
    ];
    for (locale, words) in [
        (
            Locale::En,
            [
                "CREDITS",
                "Sound packs",
                "credit required",
                "credited as a courtesy",
            ],
        ),
        (
            Locale::Es,
            [
                "CRÉDITOS",
                "Paquetes de sonidos",
                "crédito obligatorio",
                "acreditado por cortesía",
            ],
        ),
    ] {
        cantus_ui::i18n::set_locale(locale);
        let dir = tempfile::tempdir().unwrap();
        // An installed CC BY pack today's list no longer offers keeps "credit required".
        let gone = m.pack("fixture-choir").unwrap();
        let receipt = cantus_content::Receipt {
            id: "gone-choir".into(),
            version: 1,
            name: cantus_content::manifest::Text {
                en: "Old Choir".into(),
                es: "Coro antiguo".into(),
            },
            licence: gone.licence.clone(),
            licence_url: gone.licence_url.clone(),
            licence_file: gone.licence_file.clone(),
            credits: gone.credits.clone(),
            attribution: gone.attribution.clone(),
            instruments: vec![],
            installed_at: 0,
            files: 1,
            bytes: 1,
        };
        let pack = dir.path().join("gone-choir");
        std::fs::create_dir_all(&pack).unwrap();
        std::fs::write(
            pack.join(cantus_content::install::RECEIPT),
            serde_json::to_string(&receipt).unwrap(),
        )
        .unwrap();
        let mut app = support::app(cantus_core::Project::new("About"));
        app.samples.content_root = Some(dir.path().to_path_buf());
        app.samples.manifest = Some(m.clone());
        let ctx = support::ctx();
        app.show_about = true;
        let mut painted = String::new();
        for i in 0..3 {
            painted = joined(&frame(&mut app, &ctx, i as f64 * 0.1, vec![]).0);
        }
        for w in words {
            assert!(painted.contains(w), "{locale:?}: {w:?} not in {painted}");
        }
        let lines = if locale == Locale::Es { &es } else { &en };
        for l in lines {
            assert!(
                painted.contains(l.as_str()),
                "{locale:?}: {l} not in {painted}"
            );
        }
        let (gone_head, courtesy_kit) = match locale {
            Locale::Es => (
                "Coro antiguo · CC BY 4.0 · crédito obligatorio · instalado, ya no se ofrece",
                "Kit de prueba · CC0 1.0 · acreditado por cortesía",
            ),
            _ => (
                "Old Choir · CC BY 4.0 · credit required · installed, no longer offered",
                "Fixture Kit · CC0 1.0 · credited as a courtesy",
            ),
        };
        assert!(painted.contains(gone_head), "{locale:?}: {painted}");
        assert!(painted.contains(courtesy_kit), "{locale:?}: {painted}");
        let dedication = match locale {
            Locale::Es => "«Fixture Drums», de Example Studios (https://example.org/drums), obra dedicada al dominio público con CC0 1.0",
            _ => "“Fixture Drums” by Example Studios (https://example.org/drums), dedicated to the public domain under CC0 1.0",
        };
        assert!(painted.contains(dedication), "{locale:?}: {painted}");
        assert!(
            painted.contains("Example Studios"),
            "the CC0 kit's courtesy credit: {painted}"
        );
        assert!(painted.contains("Reina-Valera 1909"), "{painted}");
        cantus_ui::i18n::set_locale(Locale::En);
    }
}

/// *Credits* beside the documents scrolls the window to its credits, wherever the list's
/// length has put them.
#[test]
fn the_credits_button_scrolls_to_the_credits() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = support::app(cantus_core::Project::new("About"));
    app.samples.content_root = Some(dir.path().to_path_buf());
    app.samples.manifest = Some(cantus_content::Manifest::bundled());
    let ctx = support::ctx();
    app.show_about = true;
    let mut t = 0.0;
    frame(&mut app, &ctx, t, vec![]);
    t += 0.1;
    let (shown, _) = frame(&mut app, &ctx, t, vec![]);
    let at = |shown: &[(String, Rect)], label: &str| {
        shown
            .iter()
            .find(|(s, _)| s == label)
            .map(|(_, r)| *r)
            .unwrap_or_else(|| panic!("no {label:?} in {}", joined(shown)))
    };
    let window_top = (SIZE.y - 700.0) * 0.5;
    let button = at(&shown, "Credits");
    let p = button.center();
    let click = vec![
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
    ];
    t += 0.5;
    let (_, link) = frame(&mut app, &ctx, t, click);
    assert_eq!(link, Some(Link::Credits));
    let mut shown = Vec::new();
    for _ in 0..4 {
        t += 0.1;
        shown = frame(&mut app, &ctx, t, vec![]).0;
    }
    let band = at(&shown, "CREDITS");
    assert!(
        band.top() < window_top + 32.0 + 14.0 + 30.0,
        "the band is at the top of the window's body: {band:?}"
    );
    // The API the menu can call does the same.
    let mut app = support::app(cantus_core::Project::new("About"));
    app.samples.content_root = Some(dir.path().to_path_buf());
    app.samples.manifest = Some(cantus_content::Manifest::bundled());
    let ctx = support::ctx();
    about::show_credits(&mut app, &ctx);
    assert!(app.show_about);
    let mut shown = Vec::new();
    for i in 0..8 {
        shown = frame(&mut app, &ctx, i as f64 * 0.1, vec![]).0;
    }
    let band = at(&shown, "CREDITS");
    assert!(band.top() < window_top + 32.0 + 14.0 + 30.0, "{band:?}");
}

/// *Help ▸ Credits* opens About at its credits.
#[test]
fn help_credits_opens_about_at_the_credits() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = support::app(cantus_core::Project::new("About"));
    app.samples.content_root = Some(dir.path().to_path_buf());
    app.samples.manifest = Some(cantus_content::Manifest::bundled());
    let ctx = support::ctx();
    let mut t = 0.0;
    let mut run = |app: &mut cantus_ui::CantusApp, events: Vec<egui::Event>| {
        t += 0.25;
        let input = egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, SIZE)),
            time: Some(t),
            events,
            ..Default::default()
        };
        let mut out = ctx.run_ui(input, |root| {
            egui::Panel::top(egui::Id::new("t"))
                .show(root, |ui| cantus_ui::views::transport::show(app, ui));
            let ctx = root.ctx().clone();
            let _ = about::show(app, &ctx);
        });
        out.textures_delta.clear();
        texts(&out)
    };
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
    let find = |shown: &[(String, Rect)], label: &str| {
        shown
            .iter()
            .find(|(s, _)| s == label)
            .map(|(_, r)| *r)
            .unwrap_or_else(|| panic!("no {label:?} in {}", joined(shown)))
    };
    let shown = run(&mut app, vec![]);
    let help = find(&shown, "HELP");
    let shown = run(&mut app, click(help));
    let shown = if shown.iter().any(|(s, _)| s == "Credits") {
        shown
    } else {
        run(&mut app, vec![])
    };
    let credits = find(&shown, "Credits");
    run(&mut app, click(credits));
    assert!(app.show_about, "About opens");
    let mut shown = Vec::new();
    for _ in 0..6 {
        shown = run(&mut app, vec![]);
    }
    let window_top = (SIZE.y - 700.0) * 0.5;
    let band = find(&shown, "CREDITS");
    assert!(band.top() < window_top + 32.0 + 14.0 + 30.0, "{band:?}");
}
