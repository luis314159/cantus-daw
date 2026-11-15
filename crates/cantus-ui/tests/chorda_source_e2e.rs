//! Chorda's recorded source end to end (round 40 phase 3, ADR 0125; `chorda-src`), through
//! `CantusApp::assemble` as the app starts: a piano installed with *Install from a file…*
//! (two perspectives and its noises, in the Salamander/bitKlavier naming, its SFZ written
//! by the piano build) is played by the Chorda on a track — through Exemplar's unit, the
//! same bits an Exemplar with its zones renders — each perspective split to a strip of its
//! own sums to the unsplit to the bit, the editor draws its source list and its Mixer, and
//! undo brings the model back to the bit.
//!
//! Its own test binary: it points `CANTUS_DATA_DIR` at a temporary folder.

use cantus_content::pianobuild::{self, Perspective, Scan};
use cantus_core::inst::chorda_source::{self as cs, SourceChoice};
use cantus_core::inst_out::OutputPlan;
use cantus_core::{InstrumentKind, InstrumentSpec, NativeSpec, Project, Snapshot, Ticks, TrackId};
use cantus_engine::{render, EngineHandle, RenderOptions, SampleCache};
use cantus_ui::app::Files;
use cantus_ui::CantusApp;
use std::path::{Path, PathBuf};
use std::sync::Arc;

fn data() -> &'static Path {
    static DIR: std::sync::OnceLock<tempfile::TempDir> = std::sync::OnceLock::new();
    DIR.get_or_init(|| {
        let d = tempfile::tempdir().unwrap();
        std::env::set_var("CANTUS_DATA_DIR", d.path());
        std::fs::write(
            d.path().join("settings.json"),
            r#"{"audio_setup":{"setup_done":true},"start_page_at_start":false}"#,
        )
        .unwrap();
        d
    })
    .path()
}

/// A one-second tone, 48 kHz stereo 24-bit.
fn wav(path: &Path, freq: f32, amp: f32) {
    let spec = hound::WavSpec {
        channels: 2,
        sample_rate: 48_000,
        bits_per_sample: 24,
        sample_format: hound::SampleFormat::Int,
    };
    let mut w = hound::WavWriter::create(path, spec).unwrap();
    for i in 0..48_000 {
        let v = amp * (i as f32 * freq / 48_000.0 * std::f32::consts::TAU).sin();
        let q = (v * 8_388_607.0) as i32;
        w.write_sample(q).unwrap();
        w.write_sample(q).unwrap();
    }
    w.finalize().unwrap();
}

/// A small piano in the shared naming: two perspectives ("Close", "Room") on outputs 0 and
/// 1, two layers on C4 and E4 each, and a hammer on C4 (Releases, output 16).
fn piano(root: &Path) -> PathBuf {
    let dir = root.join("My Piano");
    for (folder, amp) in [("close", 0.3f32), ("room", 0.1)] {
        let f = dir.join(folder);
        std::fs::create_dir_all(&f).unwrap();
        for (note, hz) in [("C4", 261.63f32), ("E4", 329.63)] {
            for v in 1..=2 {
                wav(&f.join(format!("{note}v{v}.wav")), hz, amp * v as f32 / 2.0);
            }
        }
        wav(&f.join("rel40.wav"), 900.0, 0.05);
    }
    let close = Scan::of_dir(&dir.join("close")).unwrap();
    let room = Scan::of_dir(&dir.join("room")).unwrap();
    let sfz = pianobuild::write_sfz(
        "My Piano",
        &[
            Perspective {
                folder: "close",
                name: "Close",
                output: 0,
                scan: &close,
            },
            Perspective {
                folder: "room",
                name: "Room",
                output: 1,
                scan: &room,
            },
        ],
    );
    let path = dir.join("My Piano.sfz");
    std::fs::write(&path, sfz).unwrap();
    path
}

fn app_with(content: &Path) -> (CantusApp, egui::Context) {
    cantus_ui::i18n::set_thread_locale(cantus_ui::i18n::Locale::En);
    let files = Files::read(data(), Some(cantus_ui::i18n::Locale::En));
    let ctx = egui::Context::default();
    cantus_ui::design::install_fonts(&ctx);
    cantus_ui::design::install_style(&ctx);
    let (engine, _proc) = EngineHandle::manual(48_000.0);
    let mut project = Project::new("piano");
    let mut t = project.make_track("Piano");
    t.strip = Some(project.strips[0].id);
    t.instrument = InstrumentSpec::Native(NativeSpec::new(InstrumentKind::Chorda));
    let mut clip = project.make_clip("Play", Ticks::ZERO, Ticks(3840));
    for (i, key) in [60u8, 64, 60].iter().enumerate() {
        let n = project.make_note(
            Ticks(i as i64 * 480),
            Ticks(400),
            *key,
            70 + 40 * (i as u8 % 2),
        );
        clip.notes.push(n);
    }
    t.clips.push(clip);
    project.tracks.push(t);
    let mut app = CantusApp::assemble(&ctx, files, Some(project), engine, None, None, None, false);
    app.samples.content_root = Some(content.to_path_buf());
    app.samples.manifest = Some(cantus_content::Manifest {
        format: cantus_content::manifest::FORMAT,
        updated: "2026-11-12".into(),
        packs: vec![],
    });
    (app, ctx)
}

fn track(app: &CantusApp) -> TrackId {
    app.project()
        .tracks
        .iter()
        .find(|t| t.name == "Piano")
        .unwrap()
        .id
}

/// The project's master, rendered offline as the export renders it.
fn master(project: &Project) -> Vec<f32> {
    let mut snap = Snapshot::compile(project, 1);
    let errors = SampleCache::new().attach(&mut snap);
    assert!(errors.is_empty(), "{errors:?}");
    render(
        Arc::new(snap),
        &RenderOptions {
            end: Some(Ticks(1920)),
            tail_seconds: 0.5,
            block_frames: 333,
            ..Default::default()
        },
    )
    .unwrap()
    .samples
}

fn bits(x: &[f32]) -> Vec<u32> {
    x.iter().map(|v| (v + 0.0).to_bits()).collect()
}

fn press(at: egui::Pos2, pressed: bool) -> egui::Event {
    egui::Event::PointerButton {
        pos: at,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: Default::default(),
    }
}

fn frame(app: &mut CantusApp, ctx: &egui::Context, t: f64, events: Vec<egui::Event>) -> String {
    let input = egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(1920.0, 1080.0),
        )),
        time: Some(t),
        predicted_dt: 1.0 / 60.0,
        events,
        ..Default::default()
    };
    let mut out = ctx.run_ui(input, |ui| app.frame(ui));
    out.textures_delta.clear();
    let mut text = String::new();
    for c in &out.shapes {
        if let egui::Shape::Text(s) = &c.shape {
            text.push_str(s.galley.text());
            text.push('\n');
        }
    }
    text
}

#[test]
fn a_piano_from_a_file_plays_through_chordas_recorded_source_and_undo_brings_the_model_back() {
    let dir = tempfile::tempdir().unwrap();
    let content = dir.path().join("Content");
    std::fs::create_dir_all(&content).unwrap();
    cantus_engine::inst::stream::hub().set_cache_dir(dir.path().join("cache"));
    let sfz = piano(dir.path());
    let (mut app, ctx) = app_with(&content);
    let tr = track(&app);
    let model = master(app.project());
    assert!(model.iter().any(|x| *x != 0.0), "the model plays");

    // Install from a file: a user pack, and the Chorda switched to it in one step.
    cantus_ui::views::inst::chorda_source::install_path(&mut app, tr, &sfz);
    app.wait_user_installs(std::time::Duration::from_secs(60));
    let spec = app
        .project()
        .track(tr)
        .unwrap()
        .instrument
        .native()
        .unwrap()
        .clone();
    let SourceChoice::Recorded(r) = cs::current_choice(&spec) else {
        panic!("recorded: {spec:?}");
    };
    assert!(r.pack.starts_with("user-"));
    let names: Vec<_> = cs::outputs(&spec)
        .iter()
        .map(|o| (o.id.0, o.name.clone()))
        .collect();
    assert_eq!(
        names,
        [
            (0, "Close".into()),
            (1, "Room".into()),
            (16, "Releases".into())
        ]
    );

    // It plays what an Exemplar with its zones plays, to the bit.
    let recorded = master(app.project());
    assert!(recorded.iter().any(|x| *x != 0.0));
    assert_ne!(bits(&recorded), bits(&model));
    let mut ex = app.project().clone();
    ex.tracks
        .iter_mut()
        .find(|t| t.id == tr)
        .unwrap()
        .instrument = InstrumentSpec::Native(cs::exemplar_of(&spec));
    assert_eq!(bits(&master(&ex)), bits(&recorded), "Exemplar's own unit");

    // The editor: Play with its source list, then the Mixer on the shared desk.
    app.open_inst_editor(tr);
    let mut text = String::new();
    for i in 0..3 {
        text = frame(&mut app, &ctx, i as f64 / 60.0, vec![]);
    }
    for w in [
        "Source",
        "Chorda 2",
        "Previous models",
        "Install from a file…",
        "My Piano",
    ] {
        assert!(text.contains(w), "{w} in the list: {text}");
    }
    app.inst.view = cantus_ui::views::inst::View::Mixer;
    for i in 3..6 {
        text = frame(&mut app, &ctx, i as f64 / 60.0, vec![]);
    }
    for w in [
        "Piano",
        "Close",
        "Room",
        "Releases",
        "Each mic to its own strip",
        "All to Main",
    ] {
        assert!(text.contains(w), "{w} in the mixer: {text}");
    }

    // *Each mic to its own strip*, clicked: a strip each, in output order, into the
    // piano's own strip — and the same master, to the bit.
    let at = cantus_ui::views::inst::chorda_source::card::plan_rect(&ctx, tr, OutputPlan::EachOwn)
        .expect("the button is drawn")
        .center();
    let strips_before = app.project().strips.len();
    let mut t = 6.0;
    for ev in [
        vec![egui::Event::PointerMoved(at)],
        vec![press(at, true)],
        vec![press(at, false)],
        vec![],
    ] {
        frame(&mut app, &ctx, t / 60.0, ev);
        t += 1.0;
    }
    let spec = app.native_of(tr).unwrap();
    let routed: Vec<(u16, String)> = spec
        .outputs
        .iter()
        .map(|o| (o.id.0, o.name.clone()))
        .collect();
    assert_eq!(
        routed,
        [
            (0, "Close".into()),
            (1, "Room".into()),
            (16, "Releases".into())
        ],
        "every output on a strip of its own"
    );
    assert_eq!(app.project().strips.len(), strips_before + 3);
    let own = app.project().track(tr).unwrap().strip;
    for o in &spec.outputs {
        assert_eq!(app.project().strip(o.strip).unwrap().output, own);
    }
    assert_eq!(
        bits(&master(app.project())),
        bits(&recorded),
        "split = unsplit"
    );
    // The desk now says where each plays.
    text = frame(&mut app, &ctx, t / 60.0, vec![]);
    assert!(text.contains("Own strip"), "{text}");

    // A mic's delay and *Add our room*: each changes the master, each back at zero/off is
    // the split master again to the bit (the unit then owns no line and no room).
    let split = master(app.project());
    let delay = cantus_core::inst_out::key(
        cantus_core::OutputId(1),
        cantus_core::inst_out::OutputField::Delay,
    );
    app.set_inst_value(tr, &delay, 3.0, false);
    assert_ne!(
        bits(&master(app.project())),
        bits(&split),
        "the Room mic later"
    );
    app.set_inst_value(tr, &delay, 0.0, false);
    assert_eq!(
        bits(&master(app.project())),
        bits(&split),
        "no delay: as before"
    );
    app.set_inst_value(tr, "rec.room.add", 3.0, false);
    let hall = master(app.project());
    assert_ne!(bits(&hall), bits(&split), "the hall added");
    app.set_inst_value(tr, "rec.room.add", 0.0, false);
    assert_eq!(bits(&master(app.project())), bits(&split), "off: as before");
    // Four undo steps put the delay and the room back where they were.
    for _ in 0..4 {
        app.undo();
    }

    // Undo the split and the switch: the model, to the bit.
    app.undo();
    app.undo();
    assert!(!cs::has_zones(
        app.project()
            .track(tr)
            .unwrap()
            .instrument
            .native()
            .unwrap()
    ));
    assert_eq!(
        bits(&master(app.project())),
        bits(&model),
        "the model as it was"
    );
}

/// *Install from a file…* reaches Tympanum's kits too (the shared `install_any`): a kit
/// that is no pack of the manifest — an SFZ and its sample — becomes one of the user's own
/// kits, the track plays it at once, and the kits list shows it under *Your own kits*.
#[test]
fn tympanums_install_from_a_file_takes_a_kit_the_manifest_does_not_know() {
    let dir = tempfile::tempdir().unwrap();
    let content = dir.path().join("Content");
    std::fs::create_dir_all(&content).unwrap();
    let kit = dir.path().join("Garage Kit");
    std::fs::create_dir_all(&kit).unwrap();
    wav(&kit.join("kick.wav"), 60.0, 0.5);
    let sfz = kit.join("Garage Kit.sfz");
    std::fs::write(&sfz, "<group> output=0\n<region> sample=kick.wav key=36\n").unwrap();
    let (mut app, ctx) = app_with(&content);
    let tr = track(&app);
    assert!(app.exec(cantus_core::Command::SetInstrument {
        track: tr,
        spec: Box::new(InstrumentSpec::Native(NativeSpec::new(
            InstrumentKind::AcousticDrums
        ))),
    }));
    cantus_ui::views::inst::tympanum::kits::install_kit_path(&mut app, tr, &sfz);
    app.wait_user_installs(std::time::Duration::from_secs(60));
    let spec = app.native_of(tr).unwrap();
    let cantus_core::inst::tympanum::KitRef::Pack { pack, kit } =
        cantus_core::inst::tympanum::KitRef::of(&spec)
    else {
        panic!("a pack's kit: {spec:?}");
    };
    assert!(pack.starts_with("user-"), "{pack}");
    let receipt =
        std::fs::read_to_string(content.join(&pack).join(cantus_content::install::RECEIPT))
            .unwrap();
    assert!(receipt.contains("LicenseRef-user"), "{receipt}");
    assert!(receipt.contains(&kit));
    app.open_inst_editor(tr);
    let mut text = String::new();
    for i in 0..4 {
        text = frame(&mut app, &ctx, i as f64 / 60.0, vec![]);
    }
    assert!(text.contains("Garage Kit"), "the kit in the list: {text}");
}

/// r40:fix-f (review of 40a27bb7, finding 3): a preset — the Mix field, its steppers, the
/// preset browser, all `apply_inst_spec` — and A/B keep the recorded piano playing, its
/// outputs' mix and routes and its look; they used to switch it back to the model.
#[test]
fn a_preset_and_ab_keep_a_recorded_piano_its_outputs_and_its_look() {
    use cantus_core::inst_out::{self, OutputField, OutputId};
    let dir = tempfile::tempdir().unwrap();
    let content = dir.path().join("Content");
    std::fs::create_dir_all(&content).unwrap();
    cantus_engine::inst::stream::hub().set_cache_dir(dir.path().join("cache"));
    let sfz = piano(dir.path());
    let (mut app, _ctx) = app_with(&content);
    let tr = track(&app);
    cantus_ui::views::inst::chorda_source::install_path(&mut app, tr, &sfz);
    app.wait_user_installs(std::time::Duration::from_secs(60));
    // The mix: Close's level, Room on a strip of its own, the piano drawn in white.
    let close = OutputId(0);
    app.set_inst_value(tr, &inst_out::key(close, OutputField::Level), -3.0, false);
    let free = app.project().first_free_strip().unwrap();
    let cmd = inst_out::set_route(app.project(), tr, OutputId(1), Some(free)).unwrap();
    assert!(app.exec(cmd));
    app.set_inst_text(
        tr,
        cantus_core::inst::chorda_look::FINISH_KEY,
        Some("white".into()),
    );
    let before = app.native_of(tr).unwrap();
    assert!(cs::ChordaSource::of(&before).is_recorded());
    let jazz = cantus_ui::views::inst::chorda::FACTORY_PRESETS
        .iter()
        .find(|p| p.label_key == "chorda.preset.jazz")
        .unwrap();
    let preset = cantus_ui::views::inst::preset_spec(InstrumentKind::Chorda, jazz);
    app.apply_inst_spec(tr, &preset, "cmd.fx_preset");
    let after = app.native_of(tr).unwrap();
    let kept = |s: &NativeSpec| {
        (
            s.text(cs::SOURCE_KEY).map(str::to_string),
            s.text
                .iter()
                .filter(|(k, _)| k.starts_with(cs::REC_PREFIX))
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect::<Vec<_>>(),
            s.outputs.clone(),
            s.get(&inst_out::key(close, OutputField::Level)),
            s.text(cantus_core::inst::chorda_look::FINISH_KEY)
                .map(str::to_string),
        )
    };
    assert_eq!(
        kept(&after),
        kept(&before),
        "still the piano, its mix, its look"
    );
    assert_eq!(after.get("hardness"), 0.1, "the preset's sound is set");
    // Undo is one step.
    app.undo();
    assert_eq!(app.native_of(tr).unwrap(), before);
    // A/B against a model Chorda (the A side taken before the piano came): still the piano.
    app.copy_inst_ab(tr, &NativeSpec::new(InstrumentKind::Chorda));
    app.toggle_inst_ab(tr);
    assert_eq!(kept(&app.native_of(tr).unwrap()), kept(&before));
    app.toggle_inst_ab(tr);
    assert_eq!(kept(&app.native_of(tr).unwrap()), kept(&before));
}
