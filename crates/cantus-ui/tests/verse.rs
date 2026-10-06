//! The verse at start (BACKLOG §6c): every file parses and holds the same passages, every
//! language has its translation, the Spanish list stays inside the RVR1960's quotation
//! allowance, the switch hides it, the language switch shows the same passage in the
//! other translation, and every passage fits the splash and the card at 75 % and 200 %.

mod support;

use cantus_core::Project;
use cantus_ui::boot::{self, SplashVerse};
use cantus_ui::i18n::{self, Locale};
use cantus_ui::settings::Settings;
use cantus_ui::verse::{self, Launch, Reference, Shown, Spanish, Translation, VerseSettings};
use cantus_ui::views::verse as view;
use egui::{Pos2, Rect, Vec2};
use std::collections::HashMap;

/// Verses per book (the Protestant canon, KJV/RV versification), from the RV1909 file
/// of eBible.org: what "half of a book" is measured against.
const BOOK_VERSES: [(&str, u32); 66] = [
    ("GEN", 1533),
    ("EXO", 1213),
    ("LEV", 859),
    ("NUM", 1288),
    ("DEU", 959),
    ("JOS", 658),
    ("JDG", 618),
    ("RUT", 85),
    ("1SA", 810),
    ("2SA", 695),
    ("1KI", 816),
    ("2KI", 719),
    ("1CH", 942),
    ("2CH", 822),
    ("EZR", 280),
    ("NEH", 406),
    ("EST", 167),
    ("JOB", 1070),
    ("PSA", 2461),
    ("PRO", 915),
    ("ECC", 222),
    ("SNG", 117),
    ("ISA", 1292),
    ("JER", 1364),
    ("LAM", 154),
    ("EZK", 1273),
    ("DAN", 357),
    ("HOS", 197),
    ("JOL", 73),
    ("AMO", 146),
    ("OBA", 21),
    ("JON", 48),
    ("MIC", 105),
    ("NAM", 47),
    ("HAB", 56),
    ("ZEP", 53),
    ("HAG", 38),
    ("ZEC", 211),
    ("MAL", 55),
    ("MAT", 1071),
    ("MRK", 678),
    ("LUK", 1151),
    ("JHN", 879),
    ("ACT", 1007),
    ("ROM", 433),
    ("1CO", 437),
    ("2CO", 257),
    ("GAL", 149),
    ("EPH", 155),
    ("PHP", 104),
    ("COL", 95),
    ("1TH", 89),
    ("2TH", 47),
    ("1TI", 113),
    ("2TI", 83),
    ("TIT", 46),
    ("PHM", 25),
    ("HEB", 303),
    ("JAS", 108),
    ("1PE", 105),
    ("2PE", 61),
    ("1JN", 105),
    ("2JN", 13),
    ("3JN", 14),
    ("JUD", 25),
    ("REV", 404),
];

fn with_locale<R>(l: Locale, f: impl FnOnce() -> R) -> R {
    i18n::set_locale(l);
    f()
}

#[test]
fn every_verse_file_parses_and_is_not_empty() {
    for t in Translation::ALL {
        let v = verse::parse(t.source()).unwrap_or_else(|e| panic!("{t:?}: {e}"));
        assert!(v.len() >= 200, "{t:?} has only {} passages", v.len());
        assert_eq!(t.verses().len(), v.len());
        for p in &v {
            assert!(
                !p.text.contains("  "),
                "{t:?} {:?}: double space",
                p.reference
            );
            assert!(!p.text.contains('['), "{t:?} {:?}: a bracket", p.reference);
            assert!(
                !p.text.contains('<') && !p.text.contains('&'),
                "{t:?} {:?}: markup",
                p.reference
            );
        }
    }
}

#[test]
fn every_locale_has_a_translation_in_its_language() {
    for l in Locale::ALL {
        for s in Spanish::ALL {
            let t = Translation::for_locale(l, s);
            assert_eq!(t.locale(), l, "{l:?} reads {t:?}");
            assert!(!t.verses().is_empty(), "{t:?} is empty");
        }
    }
    assert_eq!(
        Translation::for_locale(Locale::Es, Spanish::Rvr1960),
        Translation::Rvr1960
    );
    assert_eq!(
        Translation::for_locale(Locale::Es, Spanish::Rv1909),
        Translation::Rv1909
    );
    assert_eq!(
        Translation::for_locale(Locale::En, Spanish::Rvr1960),
        Translation::Web
    );
}

#[test]
fn every_translation_holds_the_same_passages_in_the_same_order() {
    let en: Vec<Reference> = Translation::Web
        .verses()
        .iter()
        .map(|v| v.reference)
        .collect();
    for t in [Translation::Rvr1960, Translation::Rv1909] {
        let other: Vec<Reference> = t.verses().iter().map(|v| v.reference).collect();
        assert_eq!(en, other, "{t:?} differs from the WEB's list");
    }
    assert_eq!(verse::references(), en);
}

#[test]
fn the_spanish_list_stays_inside_the_rvr1960_allowance() {
    // American Bible Society's fair use: under 500 verses, never half of a book. Keep a
    // wide margin on both.
    let books: HashMap<&str, u32> = BOOK_VERSES.into_iter().collect();
    let mut per_book: HashMap<&str, u32> = HashMap::new();
    let mut total = 0u32;
    for v in Translation::Rvr1960.verses() {
        let n = u32::from(v.reference.count());
        total += n;
        *per_book.entry(v.reference.book).or_default() += n;
    }
    assert!(
        total < 500,
        "{total} RVR1960 verses: the allowance is under 500"
    );
    assert!(
        total <= 400,
        "{total} RVR1960 verses: keep well under the 500 allowed"
    );
    for (book, n) in per_book {
        let of = *books
            .get(book)
            .unwrap_or_else(|| panic!("unknown book {book}"));
        let share = f64::from(n) / f64::from(of);
        assert!(
            share < 0.15,
            "{book}: {n} of {of} verses ({:.0} %)",
            share * 100.0
        );
    }
}

#[test]
fn every_book_is_named_in_both_languages() {
    for l in Locale::ALL {
        with_locale(l, || {
            for r in verse::references() {
                let name = r.book_name();
                assert!(
                    !name.starts_with("verse.book."),
                    "{l:?}: no name for {}",
                    r.book
                );
            }
        });
    }
    with_locale(Locale::Es, || {
        let r = Reference::parse("LAM 3:22-23").unwrap();
        assert_eq!(r.label(), "Lamentaciones 3:22\u{2013}23");
    });
    with_locale(Locale::En, || {
        assert_eq!(
            Reference::parse("COL 3:23").unwrap().label(),
            "Colossians 3:23"
        );
    });
}

#[test]
fn references_parse_and_bad_lines_are_refused() {
    let r = Reference::parse("PSA 150:3").unwrap();
    assert_eq!((r.book, r.chapter, r.first, r.last), ("PSA", 150, 3, 3));
    assert_eq!(Reference::parse("NUM 6:24-26").unwrap().count(), 3);
    for bad in ["Psalm 1:1", "PSA 1", "PSA 1:0", "PSA 1:3-2", "PSA x:1"] {
        assert!(Reference::parse(bad).is_none(), "{bad} parsed");
    }
    assert!(verse::parse("PSA 1:1 Blessed").is_err());
    assert!(verse::parse("PSA 1:1 | a\nPSA 1:1 | b").is_err());
    assert!(verse::parse("# a comment\n\nPSA 1:1 | a").unwrap().len() == 1);
}

#[test]
fn the_motto_is_there_in_every_translation() {
    let col = Reference::parse("COL 3:23").unwrap();
    assert!(Translation::Web
        .passage(col)
        .unwrap()
        .text
        .contains("heartily"));
    assert!(Translation::Rvr1960
        .passage(col)
        .unwrap()
        .text
        .contains("de corazón"));
    assert!(Translation::Rv1909
        .passage(col)
        .unwrap()
        .text
        .contains("de ánimo"));
}

#[test]
fn the_switch_is_on_by_default_and_hides_the_verse() {
    assert!(Settings::default().verse.show, "on by default");
    // A settings file from before the verse existed: on.
    let old: Settings = serde_json::from_str(r#"{"locale":"es"}"#).unwrap();
    assert!(old.verse.show);
    assert!(Launch::from_settings(&Settings::default(), 7)
        .shown
        .is_some());
    let off = Settings {
        verse: VerseSettings {
            show: false,
            ..Default::default()
        },
        ..Default::default()
    };
    assert_eq!(Launch::from_settings(&off, 7).shown, None);

    // Read from the file, as the launch does.
    let dir = tempfile::tempdir().unwrap();
    off.save(&cantus_ui::settings::settings_path(dir.path()))
        .unwrap();
    assert_eq!(Launch::read(dir.path(), Some(Locale::En), 7).shown, None);

    // In the window: the switch turned off hides the card at once.
    let mut app = support::app(Project::new("t"));
    app.verse = Some(view::Card::new(Reference::parse("PSA 96:1").unwrap()));
    app.settings.verse.show = false;
    let ctx = support::ctx();
    support::paint(&ctx, Vec2::new(1280.0, 800.0), 1, |ui| {
        let ctx = ui.ctx().clone();
        assert!(view::frame(&mut app, &ctx).is_none());
    });
    assert!(app.verse.is_none());
}

#[test]
fn dont_show_again_turns_the_switch_off_and_says_where_it_is() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = support::app(Project::new("t"));
    app.settings_path = cantus_ui::settings::settings_path(dir.path());
    app.verse = Some(view::Card::new(Reference::parse("PSA 96:1").unwrap()));
    view::turn_off(&mut app);
    assert!(app.verse.is_none());
    assert!(!app.settings.verse.show);
    assert!(!Settings::load(&app.settings_path).verse.show, "remembered");
}

#[test]
fn picks_spread_over_the_list() {
    let n = verse::references().len();
    let mut seen = std::collections::HashSet::new();
    for seed in 0..2000u64 {
        seen.insert(verse::pick(seed).unwrap());
    }
    assert!(
        seen.len() > n * 9 / 10,
        "{} of {n} passages ever picked",
        seen.len()
    );
}

/// The card painted for `app` in a window of `size` points: what it painted, and the text.
fn card_frame(
    app: &mut cantus_ui::CantusApp,
    ctx: &egui::Context,
    size: Vec2,
) -> (Option<view::Painted>, String) {
    let mut painted = None;
    // Two frames: an area is measured, unseen, on its first.
    let text = support::paint(ctx, size, 2, |ui| {
        let ctx = ui.ctx().clone();
        painted = view::frame(app, &ctx);
    });
    (painted, text)
}

#[test]
fn switching_the_language_shows_the_same_passage_in_the_other_translation() {
    let col = Reference::parse("COL 3:23").unwrap();
    let mut app = support::app(Project::new("t"));
    app.verse = Some(view::Card::new(col));
    let ctx = support::ctx();
    i18n::set_locale(Locale::En);
    let (p, en) = card_frame(&mut app, &ctx, Vec2::new(1280.0, 800.0));
    assert!(p.is_some(), "the card shows");
    assert!(en.contains("COLOSSIANS 3:23 \u{00b7} WEB"), "{en}");
    assert!(en.contains("heartily"), "{en}");

    // Settings ▸ Language, live: the same passage, in Spanish, from the RVR1960.
    i18n::set_locale(Locale::Es);
    let (_, es) = card_frame(&mut app, &ctx, Vec2::new(1280.0, 800.0));
    assert!(es.contains("COLOSENSES 3:23 \u{00b7} RVR1960"), "{es}");
    assert!(es.contains("de corazón"), "{es}");
    assert!(es.contains("No volver a mostrar"), "{es}");
    assert_eq!(
        app.verse.map(|c| c.reference),
        Some(col),
        "the same passage"
    );

    // The public-domain 1909, one setting away.
    app.settings.verse.spanish = Spanish::Rv1909;
    let (_, old) = card_frame(&mut app, &ctx, Vec2::new(1280.0, 800.0));
    assert!(old.contains("COLOSENSES 3:23 \u{00b7} RV1909"), "{old}");
    assert!(old.contains("de ánimo"), "{old}");

    // And back.
    i18n::set_locale(Locale::En);
    let (_, back) = card_frame(&mut app, &ctx, Vec2::new(1280.0, 800.0));
    assert!(back.contains("COLOSSIANS 3:23 \u{00b7} WEB"), "{back}");
}

#[test]
fn the_splash_follows_the_language_too() {
    let r = Reference::parse("PSA 96:1").unwrap();
    let shown = Shown {
        reference: r,
        spanish: Spanish::Rvr1960,
    };
    let ctx = support::ctx();
    for (l, cite) in [
        (Locale::En, "PSALM 96:1 \u{00b7} WEB"),
        (Locale::Es, "SALMOS 96:1 \u{00b7} RVR1960"),
    ] {
        i18n::set_locale(l);
        let text = support::paint(&ctx, boot::size(true), 2, |ui| {
            let _ = boot::paint_splash_with(ui, 0.5, SplashVerse::Shown(shown));
        });
        assert!(text.contains(cite), "{l:?}: {text}");
    }
}

/// Every passage of `t`, as `l` shows it.
fn each_shown(f: &mut impl FnMut(Locale, Translation, Shown)) {
    for (l, s) in [
        (Locale::En, Spanish::Rvr1960),
        (Locale::Es, Spanish::Rvr1960),
        (Locale::Es, Spanish::Rv1909),
    ] {
        let t = Translation::for_locale(l, s);
        i18n::set_locale(l);
        for v in t.verses() {
            f(
                l,
                t,
                Shown {
                    reference: v.reference,
                    spanish: s,
                },
            );
        }
    }
}

fn inside(inner: Rect, outer: Rect) -> bool {
    outer.expand(0.51).contains_rect(inner)
}

#[test]
fn every_passage_fits_the_splash_at_75_and_200_percent_in_both_languages() {
    for ppp in [0.75f32, 2.0] {
        let ctx = support::ctx();
        ctx.set_pixels_per_point(ppp);
        let mut worst = 0usize;
        each_shown(&mut |l, t, shown| {
            let mut got = None;
            support::paint(&ctx, boot::size(true), 1, |ui| {
                got = boot::paint_splash_with(ui, 0.5, SplashVerse::Shown(shown));
            });
            let p = got.unwrap_or_else(|| panic!("{l:?} {t:?}: nothing painted"));
            let what = format!("{:?} in {t:?} at {ppp}×", shown.reference);
            worst = worst.max(p.lines);
            assert!(p.lines <= view::MAX_LINES, "{what}: {} lines", p.lines);
            assert!(!p.shortened, "{what}: shortened");
            assert!(inside(p.text, p.area), "{what}: the text leaves the band");
            assert!(
                inside(p.reference, p.area),
                "{what}: the reference leaves the band"
            );
            assert!(inside(p.hide, p.area), "{what}: the link leaves the band");
            assert!(
                p.reference.top() >= p.text.bottom() - 0.5,
                "{what}: reference over the text"
            );
            assert!(
                p.hide.top() >= p.reference.bottom() - 0.5,
                "{what}: the link beside the reference"
            );
            assert!(
                !p.hide.intersects(p.reference.shrink(0.5)),
                "{what}: link over the reference"
            );
            assert!(
                !p.hide.intersects(p.text.shrink(0.5)),
                "{what}: link over the text"
            );
        });
        assert!(worst >= 3, "the longest passage should need a few lines");
    }
}

#[test]
fn every_passage_fits_the_card_at_75_and_200_percent_in_both_languages() {
    // The window of a 1280 × 800-pixel screen, in points at each zoom; and a small one.
    for (ppp, window) in [
        (0.75f32, Vec2::new(1706.0, 1066.0)),
        (2.0, Vec2::new(640.0, 400.0)),
        (2.0, Vec2::new(512.0, 320.0)),
    ] {
        let ctx = support::ctx();
        ctx.set_pixels_per_point(ppp);
        let screen = Rect::from_min_size(Pos2::ZERO, window);
        let top = cantus_ui::views::transport::HEIGHT;
        // Inside a frame: the fonts are there.
        support::paint(&ctx, window, 1, |ui| {
            let ctx = ui.ctx().clone();
            each_shown(&mut |_, t, shown| {
                let (rect, f) = view::card_layout(&ctx, screen, shown);
                let what = format!("{:?} in {t:?} at {ppp}× in {window:?}", shown.reference);
                assert!(f.fits(), "{what}: {} lines", f.lines.lines.len());
                assert!(inside(rect, screen), "{what}: the card leaves the window");
                assert!(rect.top() >= top, "{what}: the card covers the toolbar");
            });
        });
    }
    // And painted, the longest of each translation.
    for (l, s) in [
        (Locale::En, Spanish::Rvr1960),
        (Locale::Es, Spanish::Rvr1960),
        (Locale::Es, Spanish::Rv1909),
    ] {
        let t = Translation::for_locale(l, s);
        let longest = t.verses().iter().max_by_key(|v| v.text.len()).unwrap();
        for ppp in [0.75f32, 2.0] {
            i18n::set_locale(l);
            let mut app = support::app(Project::new("t"));
            app.settings.verse.spanish = s;
            app.verse = Some(view::Card::new(longest.reference));
            let ctx = support::ctx();
            ctx.set_pixels_per_point(ppp);
            let (p, _) = card_frame(&mut app, &ctx, Vec2::new(640.0, 400.0));
            let p = p.expect("the card shows");
            let what = format!("{:?} in {t:?} at {ppp}×", longest.reference);
            assert!(!p.shortened, "{what}: shortened");
            assert!(
                inside(p.text, p.area) && inside(p.reference, p.area) && inside(p.hide, p.area),
                "{what}: {p:?}"
            );
        }
    }
}

#[test]
fn the_card_goes_by_itself_after_a_while() {
    let mut app = support::app(Project::new("t"));
    app.verse = Some(view::Card {
        reference: Reference::parse("PSA 96:1").unwrap(),
        since: Some(-view::CARD_SECONDS - 1.0),
    });
    let ctx = support::ctx();
    let (p, _) = card_frame(&mut app, &ctx, Vec2::new(1280.0, 800.0));
    assert!(p.is_none());
    assert!(app.verse.is_none());
}
