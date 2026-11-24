//! r42:strings-rec — a zone's articulation, bow and controller crossfade edited in Exemplar's
//! zone card (its *Playing* page, ADR 0158): each edit a command the engine hears, undone to
//! the bit, and the card's readouts in words (an articulation's and a bow's abbreviation, the
//! controller, the curve) in both languages.

use cantus_core::inst::sampler::zf;
use cantus_core::{InstrumentKind, InstrumentSpec, NativeSpec, Project};
use cantus_engine::EngineHandle;
use cantus_ui::views::inst::sampler::{set_zone_fields, write_zone_value as write_zone};
use cantus_ui::CantusApp;

fn app() -> (CantusApp, cantus_core::TrackId) {
    let mut p = Project::new("zone");
    let mut t = p.make_track("Strings");
    let mut spec = NativeSpec::new(InstrumentKind::Sampler);
    spec.set("z0.gen", 1.0).unwrap();
    t.instrument = InstrumentSpec::Native(spec);
    let id = t.id;
    p.tracks.push(t);
    (
        CantusApp::from_parts(Some(p), EngineHandle::dummy(), false),
        id,
    )
}

fn spec(app: &CantusApp, t: cantus_core::TrackId) -> NativeSpec {
    app.project()
        .track(t)
        .unwrap()
        .instrument
        .native()
        .unwrap()
        .clone()
}

#[test]
fn a_zones_articulation_bow_and_crossfade_are_edited_with_undo() {
    let (mut app, t) = app();
    let before = spec(&app, t);
    for (f, v) in [
        (zf::ART, 5.0),
        (zf::BOW, 2.0),
        (zf::XFCC, 1.0),
        (zf::XFINHI, 96.0),
        (zf::XFCURVE, 1.0),
    ] {
        let s = spec(&app, t);
        set_zone_fields(&mut app, t, &s, 0, &[(f, v)], false);
    }
    let after = spec(&app, t);
    for (k, v) in [
        ("z0.art", 5.0),
        ("z0.bow", 2.0),
        ("z0.xfcc", 1.0),
        ("z0.xfinhi", 96.0),
        ("z0.xfcurve", 1.0),
    ] {
        assert_eq!(after.get(k), v, "{k}");
    }
    for _ in 0..5 {
        app.undo();
    }
    assert_eq!(spec(&app, t), before, "undone to what it was");
}

#[test]
fn the_playing_page_reads_in_words_in_both_languages() {
    let read = |f: usize, v: f32| {
        let mut b = String::new();
        write_zone(f, v, &mut b);
        b
    };
    cantus_ui::i18n::set_thread_locale(cantus_ui::i18n::Locale::En);
    assert_eq!(read(zf::ART, 5.0), "Pizz.");
    assert_eq!(read(zf::ART, 0.0), "Any");
    assert_eq!(read(zf::BOW, 1.0), "Down");
    assert_eq!(read(zf::XFCC, -1.0), "Off");
    assert_eq!(read(zf::XFCC, 1.0), "CC 1");
    assert_eq!(read(zf::XFCURVE, 0.0), "Power");
    cantus_ui::i18n::set_thread_locale(cantus_ui::i18n::Locale::Es);
    assert_eq!(read(zf::ART, 1.0), "Sost.");
    assert_eq!(read(zf::BOW, 2.0), "Arriba");
    assert_eq!(read(zf::XFCURVE, 1.0), "Ganancia");
}
