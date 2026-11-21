//! Fides in the instrument picker (round 41, ADR 0150): the device picker lists it in both
//! languages on the Keys shelf; choosing it through `CantusApp::assemble` and `apply_pick`
//! adds a track that hosts a new Fides (a plucked nylon string in a guitar's body, its
//! model stored), a phrase on that track plays through the real engine wiring (a processor
//! driven block by block) and is heard; a factory preset applied as the browser applies it
//! plays a marimba through the same path; and its editor opens in Play, Edit and Matrix.

use cantus_core::{InstrumentKind, Project, Ticks, PPQ};
use cantus_engine::EngineHandle;
use cantus_ui::app::Files;
use cantus_ui::i18n::{set_thread_locale, Locale};
use cantus_ui::picker::{PickChoice, PickPurpose};
use cantus_ui::picker_catalog::{self, Category};
use cantus_ui::CantusApp;

#[test]
#[ignore = "runs once Fides is in ALL — the owner's listening verdict (docs/round41/fides.md)"]
fn the_picker_lists_fides_in_both_languages_on_the_keys_shelf() {
    for (locale, what) in [
        (Locale::En, "Resonators: strings, bars, membranes"),
        (Locale::Es, "Resonadores: cuerdas, barras, membranas"),
    ] {
        set_thread_locale(locale);
        let devices = picker_catalog::build(std::iter::empty());
        let d = devices
            .iter()
            .find(|d| d.choice == PickChoice::Instrument(InstrumentKind::Resonator))
            .unwrap_or_else(|| panic!("{locale:?}: Fides is not in the picker"));
        assert_eq!(d.name, "Fides");
        assert_eq!(d.what, what);
        assert_eq!(d.slug, Some("fides"));
        assert!(d.in_category(Category::Keys));
        assert!(d.in_category(Category::OtherInstruments));
    }
    assert!(InstrumentKind::Resonator.ready());
    assert!(InstrumentKind::ready_kinds().any(|k| k == InstrumentKind::Resonator));
}

fn app_with_song(data: &std::path::Path) -> (CantusApp, cantus_engine::Processor) {
    std::env::set_var("CANTUS_DATA_DIR", data);
    std::fs::write(
        data.join("settings.json"),
        r#"{"audio_setup":{"setup_done":true}}"#,
    )
    .unwrap();
    let files = Files::read(data, Some(Locale::En));
    let ctx = egui::Context::default();
    let (engine, proc_) = EngineHandle::manual(48_000.0);
    let song = Project::new("fides");
    let app = CantusApp::assemble(&ctx, files, Some(song), engine, None, None, None, false);
    (app, proc_)
}

/// Play the song from the start for `seconds` through the processor: the output.
fn play(app: &mut CantusApp, proc_: &mut cantus_engine::Processor, seconds: usize) -> Vec<f32> {
    app.sync_engine();
    app.seek(Ticks::ZERO);
    app.toggle_play();
    app.sync_engine();
    let frames = 48_000 * seconds;
    let mut out = Vec::with_capacity(frames * 2);
    let since = std::time::Instant::now();
    while out.len() < frames * 2 {
        let mut buf = vec![0f32; 512 * 2];
        proc_.process(&mut buf, 2);
        out.extend_from_slice(&buf);
        assert!(since.elapsed().as_secs() < 120, "the phrase never finished");
    }
    app.toggle_play();
    app.sync_engine();
    out
}

#[test]
fn choosing_fides_adds_a_track_that_plays_a_phrase_and_a_preset() {
    let data = tempfile::tempdir().unwrap();
    let (mut app, mut proc_) = app_with_song(data.path());
    let before = app.project().tracks.len();
    let purpose = PickPurpose::AddInstrument { index: None };
    app.open_picker(purpose.clone());
    app.apply_pick(purpose, PickChoice::Instrument(InstrumentKind::Resonator));
    app.picker = None;
    assert_eq!(app.project().tracks.len(), before + 1);
    let track = app.project().tracks.last().unwrap().id;
    let spec = app
        .project()
        .track(track)
        .and_then(|t| t.instrument.native())
        .cloned()
        .expect("a native instrument");
    assert_eq!(spec.kind, InstrumentKind::Resonator);
    // Its model is stored with it (ADR 0059's rule), and it plays a plucked string.
    assert_eq!(
        spec.params.get("model"),
        Some(&cantus_core::inst::fides::MODEL)
    );
    assert_eq!(spec.get("exc.type"), 0.0);
    assert_eq!(spec.get("a.type"), 0.0);

    // A phrase on the new track: an arpeggio and a held chord.
    let mut p = app.project().clone();
    let mut c = p.make_clip("Phrase", Ticks::ZERO, Ticks::beats(4));
    for (i, pitch) in [48u8, 55, 60, 64, 67, 72].iter().enumerate() {
        c.notes
            .push(p.make_note(Ticks(i as i64 * PPQ / 2), Ticks(PPQ / 2), *pitch, 100));
    }
    for pitch in [52u8, 59, 64] {
        c.notes
            .push(p.make_note(Ticks(3 * PPQ), Ticks(PPQ), pitch, 90));
    }
    assert!(app.exec(cantus_core::Command::InsertClip {
        track,
        clip: Box::new(c)
    }));
    let out = play(&mut app, &mut proc_, 3);
    assert!(out.iter().all(|x| x.is_finite()));
    let peak = out.iter().fold(0.0f32, |m, x| m.max(x.abs()));
    assert!(peak > 0.01, "the phrase is silent: peak {peak}");
    assert!(peak < 1.0, "the phrase clips: peak {peak}");
    // The chord at the third beat sounds too, not only the first notes.
    let late = &out[48_000 * 3 / 2 * 2..48_000 * 2 * 2];
    assert!(late.iter().fold(0.0f32, |m, x| m.max(x.abs())) > 0.005);

    // The marimba preset applied as the preset browser applies it: one undo step, and the
    // same phrase is heard through it.
    let marimba = cantus_ui::views::inst::fides::FACTORY_PRESETS
        .iter()
        .find(|p| p.label_key == "fides.preset.marimba")
        .expect("the marimba preset");
    let target = cantus_ui::views::inst::preset_spec(InstrumentKind::Resonator, marimba);
    app.apply_inst_spec(track, &target, "cmd.fx_preset");
    let now = app
        .project()
        .track(track)
        .and_then(|t| t.instrument.native())
        .cloned()
        .unwrap();
    assert_eq!(now.get("a.type"), 2.0, "a marimba bar");
    assert_eq!(now.get("exc.type"), 3.0, "struck by a mallet");
    let out = play(&mut app, &mut proc_, 3);
    let peak = out.iter().fold(0.0f32, |m, x| m.max(x.abs()));
    assert!(peak > 0.01 && peak < 1.0, "the marimba: peak {peak}");
}

/// A probe (round 41's review fixes): every factory preset's audition phrase and its five
/// notes rendered through the engine at its defaults, each render's bits hashed — and, with
/// `FIDES_BITS_DIR`, written there as raw `f32`; with `FIDES_BITS_BASE` too, each compared
/// with the render of the same name there (the largest difference, in dB under full scale).
/// `cargo test -p cantus-ui --test fides_pick_e2e probe -- --ignored --nocapture`
#[test]
#[ignore = "a probe: the presets' bits, before and after a DSP change"]
fn probe_the_factory_presets_bits() {
    use cantus_core::{InstrumentSpec, NativeSpec, Snapshot};
    use cantus_ui::preset_audition as aud;
    use cantus_ui::preset_catalog::{entries, Payload, Unit};
    let render = |p: &Project, end: Ticks| -> Vec<f32> {
        let snap = std::sync::Arc::new(Snapshot::compile(p, 0));
        cantus_engine::render(
            snap,
            &cantus_engine::RenderOptions {
                sample_rate: 48_000,
                start: Ticks::ZERO,
                end: Some(end),
                tail_seconds: aud::TAIL,
                compensate_latency: true,
                ..Default::default()
            },
        )
        .expect("a render")
        .samples
    };
    let notes = |spec: &NativeSpec| {
        let mut p = aud::phrase_project(
            InstrumentSpec::Native(spec.clone()),
            cantus_ui::preset_tags::Role::Keys,
        );
        let mut clip = p.make_clip("notes", Ticks::ZERO, Ticks::beats(20));
        for (i, key) in [36u8, 48, 60, 72, 84].iter().enumerate() {
            let at = Ticks::beats(4 * i as i64);
            clip.notes.push(p.make_note(at, Ticks::beats(3), *key, 100));
        }
        p.tracks[0].clips = vec![clip];
        render(&p, Ticks::beats(20))
    };
    let dir = std::env::var_os("FIDES_BITS_DIR").map(std::path::PathBuf::from);
    let base = std::env::var_os("FIDES_BITS_BASE").map(std::path::PathBuf::from);
    set_thread_locale(Locale::En);
    let unit = Unit::Instrument(InstrumentKind::Resonator);
    for (i, e) in entries(unit, false).iter().enumerate() {
        let Payload::Native(spec) = &e.payload else {
            continue;
        };
        let name = e.id.trim_start_matches("fides.preset.");
        let p = aud::phrase_project(InstrumentSpec::Native(spec.clone()), e.role());
        let phrase = render(&p, Ticks::beats((4.0 * aud::BARS) as i64));
        for (what, x) in [("phrase", phrase), ("notes", notes(spec))] {
            let hash = x.iter().fold(0xcbf2_9ce4_8422_2325u64, |h, v| {
                (h ^ v.to_bits() as u64).wrapping_mul(0x0100_0000_01b3)
            });
            let file = format!("{:02}-{name}-{what}.f32", i + 1);
            let mut diff = String::new();
            if let Some(b) = &base {
                let old: Vec<f32> = std::fs::read(b.join(&file))
                    .unwrap_or_default()
                    .chunks_exact(4)
                    .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
                    .collect();
                let d = if old.len() != x.len() {
                    f32::INFINITY
                } else {
                    old.iter()
                        .zip(&x)
                        .fold(0f32, |m, (a, b)| m.max((a - b).abs()))
                };
                diff = if d == 0.0 {
                    "  same bits".into()
                } else {
                    format!(
                        "  differs, at most {:.1} dBFS",
                        20.0 * cantus_dsp::math::log10(d.max(1e-30))
                    )
                };
            }
            println!("{file}: {hash:016x}{diff}");
            if let Some(d) = &dir {
                std::fs::create_dir_all(d).unwrap();
                let bytes: Vec<u8> = x.iter().flat_map(|v| v.to_le_bytes()).collect();
                std::fs::write(d.join(&file), bytes).unwrap();
            }
        }
    }
}
