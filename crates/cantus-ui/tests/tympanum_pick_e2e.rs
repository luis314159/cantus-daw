//! Tympanum in the instrument picker (round 40's close; the owner, 2026-11-14: *"Add
//! Tympanum at once"*): the device picker lists it in both languages, beside Pulsus on the
//! Drums shelf; choosing it through `CantusApp::assemble` and `apply_pick` adds a track that
//! hosts a new Tympanum (the modelled studio kit, the generated voice), and a groove on that
//! track plays through the real engine wiring (a processor driven block by block).

use cantus_core::inst::tympanum::{KitRef, Voice, KIT_KEY, VOICE_KEY};
use cantus_core::{InstrumentKind, Project, Ticks, PPQ};
use cantus_engine::EngineHandle;
use cantus_ui::app::Files;
use cantus_ui::i18n::{set_thread_locale, Locale};
use cantus_ui::picker::{PickChoice, PickPurpose};
use cantus_ui::picker_catalog::{self, Category};
use cantus_ui::CantusApp;

#[test]
fn the_picker_lists_tympanum_in_both_languages_on_the_drums_shelf() {
    for (locale, what) in [
        (Locale::En, "Acoustic drummer"),
        (Locale::Es, "Baterista acústico"),
    ] {
        set_thread_locale(locale);
        let devices = picker_catalog::build(std::iter::empty());
        let d = devices
            .iter()
            .find(|d| d.choice == PickChoice::Instrument(InstrumentKind::AcousticDrums))
            .unwrap_or_else(|| panic!("{locale:?}: Tympanum is not in the picker"));
        assert_eq!(d.name, "Tympanum");
        assert_eq!(d.what, what);
        assert_eq!(d.slug, Some("tympanum"));
        assert!(d.in_category(Category::Drums));
        // Beside the drum machine.
        let pulsus = devices
            .iter()
            .position(|d| d.choice == PickChoice::Pulsus)
            .unwrap();
        let at = devices.iter().position(|x| x.key == d.key).unwrap();
        assert_eq!(at, pulsus + 1);
        assert!(picker_catalog::score(d, "bateria").is_some() || locale == Locale::En);
    }
    assert!(InstrumentKind::AcousticDrums.ready());
    assert!(InstrumentKind::ready_kinds().any(|k| k == InstrumentKind::AcousticDrums));
}

#[test]
fn choosing_tympanum_adds_a_track_that_plays_a_groove() {
    let data = tempfile::tempdir().unwrap();
    std::env::set_var("CANTUS_DATA_DIR", data.path());
    std::fs::write(
        data.path().join("settings.json"),
        r#"{"audio_setup":{"setup_done":true}}"#,
    )
    .unwrap();
    let files = Files::read(data.path(), Some(Locale::En));
    let ctx = egui::Context::default();
    let (engine, mut proc_) = EngineHandle::manual(48_000.0);
    let song = Project::new("drums");
    let mut app = CantusApp::assemble(&ctx, files, Some(song), engine, None, None, None, false);
    let before = app.project().tracks.len();
    let purpose = PickPurpose::AddInstrument { index: None };
    app.open_picker(purpose.clone());
    app.apply_pick(
        purpose,
        PickChoice::Instrument(InstrumentKind::AcousticDrums),
    );
    app.picker = None;
    assert_eq!(app.project().tracks.len(), before + 1);
    let track = app.project().tracks.last().unwrap().id;
    let spec = app
        .project()
        .track(track)
        .and_then(|t| t.instrument.native())
        .cloned()
        .expect("a native instrument");
    assert_eq!(spec.kind, InstrumentKind::AcousticDrums);
    // The default kit (the modelled studio kit, instant) and the generated voice.
    let kit = spec.text(KIT_KEY).and_then(KitRef::parse);
    assert!(
        kit.is_none() || kit == Some(KitRef::default_kit()),
        "a new Tympanum plays the default kit: {kit:?}"
    );
    assert_eq!(KitRef::default_kit(), KitRef::Modelled("studio".into()));
    assert!(spec.text(VOICE_KEY).is_none(), "the generated voice");
    assert_eq!(Voice::of(&spec), Voice::Generated);

    // A groove on the new track: kick, snare and closed hat on General MIDI's notes.
    let mut p = app.project().clone();
    let mut c = p.make_clip("Groove", Ticks::ZERO, Ticks::beats(4));
    for step in 0..16i64 {
        let at = Ticks(step * PPQ / 4);
        if step % 8 == 0 || step % 8 == 6 {
            c.notes.push(p.make_note(at, Ticks(PPQ / 4), 36, 110));
        }
        if step % 8 == 4 {
            c.notes.push(p.make_note(at, Ticks(PPQ / 4), 38, 115));
        }
        if step % 2 == 0 {
            c.notes.push(p.make_note(at, Ticks(PPQ / 4), 42, 90));
        }
    }
    assert!(app.exec(cantus_core::Command::InsertClip {
        track,
        clip: Box::new(c)
    }));
    app.sync_engine();
    app.seek(Ticks::ZERO);
    app.toggle_play();
    app.sync_engine();
    let frames = 48_000 * 2;
    let mut out = Vec::with_capacity(frames * 2);
    let since = std::time::Instant::now();
    while out.len() < frames * 2 {
        let mut buf = vec![0f32; 512 * 2];
        proc_.process(&mut buf, 2);
        out.extend_from_slice(&buf);
        assert!(since.elapsed().as_secs() < 120, "the groove never finished");
    }
    app.toggle_play();
    app.sync_engine();
    assert!(out.iter().all(|x| x.is_finite()));
    let peak = out.iter().fold(0.0f32, |m, x| m.max(x.abs()));
    assert!(peak > 0.01, "the groove is silent: peak {peak}");
    assert!(peak < 1.0, "the groove clips: peak {peak}");
    // Each beat's hits sound, not only the first: the second half is not silent.
    let half = &out[frames..];
    assert!(half.iter().fold(0.0f32, |m, x| m.max(x.abs())) > 0.01);
}
