//! A project whose sound pack is missing (removed, renamed, files deleted, or opened on
//! another computer) says so plainly — which pack, which tracks, and what to do: install it
//! (again) from *Sound packs…*, use this computer's copy when it is at another place, or
//! find the folder — while the rest of the project loads and plays (round 26). Every part
//! that plays pack files is watched: Exemplar zones, Pulsus pads and layers, audio clips.
//! The disk is checked off the interface thread, and only when something changed.

mod support;

use cantus_content::manifest::{FileEntry, PackInstrument, Shelf, Source, Text};
use cantus_content::Receipt;
use cantus_core::inst::sampler::{Zone, MAX_ZONES};
use cantus_core::{Command, InstrumentKind, InstrumentSpec, NativeSpec};
use egui::{Pos2, Rect, Vec2};
use std::path::{Path, PathBuf};
use support::*;

const SIZE: Vec2 = Vec2::new(1280.0, 720.0);

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

/// One frame as the app runs it for this: the installs polled, then the browser's frame
/// (the cards and *Sound packs…*).
fn frame(
    a: &mut cantus_ui::CantusApp,
    ctx: &egui::Context,
    events: Vec<egui::Event>,
) -> Vec<(String, Rect)> {
    let t = ctx.input(|i| i.time) + 0.1;
    let input = egui::RawInput {
        screen_rect: Some(Rect::from_min_size(Pos2::ZERO, SIZE)),
        time: Some(t),
        events,
        ..Default::default()
    };
    let mut out = ctx.run_ui(input, |ui| {
        let ctx = ui.ctx().clone();
        a.poll_samples(&ctx);
        cantus_ui::views::samples_browser::frame(a, &ctx);
    });
    out.textures_delta.clear();
    texts(&out)
}

/// Frames until the scan (on its worker) and any install have finished.
fn settle(a: &mut cantus_ui::CantusApp, ctx: &egui::Context) -> Vec<(String, Rect)> {
    frame(a, ctx, vec![]);
    for _ in 0..400 {
        if !a.samples.missing.busy() && !a.samples.content().busy() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
        frame(a, ctx, vec![]);
    }
    frame(a, ctx, vec![]);
    frame(a, ctx, vec![])
}

fn click(r: Rect) -> Vec<egui::Event> {
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
}

fn joined(t: &[(String, Rect)]) -> String {
    t.iter()
        .map(|(s, _)| s.as_str())
        .collect::<Vec<_>>()
        .join(" | ")
}

fn find(t: &[(String, Rect)], label: &str) -> Rect {
    t.iter()
        .find(|(s, _)| s == label)
        .map(|(_, r)| *r)
        .unwrap_or_else(|| panic!("no {label:?} in {}", joined(t)))
}

fn slash(p: &Path) -> String {
    p.to_string_lossy().replace('\\', "/")
}

/// An Exemplar whose zones play `files`.
fn exemplar(files: &[String]) -> InstrumentSpec {
    let mut spec = NativeSpec::new(InstrumentKind::Sampler);
    for (n, f) in files.iter().enumerate() {
        let key = 36 + (n % 80) as u8;
        let z = Zone::new(Some(f.clone()), key, key, key);
        for (k, v) in z.values_for(n) {
            spec.set(&k, v);
        }
        for (k, text) in z.texts_for(n) {
            spec.set_text(&k, Some(text)).unwrap();
        }
    }
    InstrumentSpec::Native(spec)
}

const PACK: &str = "fixture-grand-piano";

fn pack_manifest() -> cantus_content::Manifest {
    let mut m = cantus_content::Manifest::parse(include_str!(
        "../../cantus-content/tests/fixtures/manifest-cc-by.json"
    ))
    .unwrap();
    let p = m.packs.iter_mut().find(|p| p.id == PACK).unwrap();
    p.instruments = vec![PackInstrument {
        id: "grand".into(),
        name: Text {
            en: "Fixture Grand".into(),
            es: "Cola de prueba".into(),
        },
        file: "Piano.sfz".into(),
        shelf: Shelf::Keys,
        layers: None,
        gain_db: 0.0,
        voices: None,
    }];
    m
}

fn in_pack(content: &Path, rest: &str) -> String {
    slash(&content.join(PACK).join(rest))
}

/// The project as opened: the starter session plus a track `Piano` playing `files`.
fn opened(content: &Path, files: &[String]) -> cantus_ui::CantusApp {
    opened_with(content, |a| {
        let mut t = a.bus.new_track("Piano");
        t.instrument = exemplar(files);
        let len = a.project().tracks.len();
        assert!(a.exec(Command::InsertTrack {
            index: len,
            track: Box::new(t),
        }));
    })
}

fn opened_with(
    content: &Path,
    build: impl FnOnce(&mut cantus_ui::CantusApp),
) -> cantus_ui::CantusApp {
    let mut a = app(cantus_ui::starter_project());
    a.samples.content_root = Some(content.to_path_buf());
    a.samples.manifest = Some(pack_manifest());
    build(&mut a);
    let p = a.project().clone();
    a.replace_project(p);
    a
}

fn sample_paths(a: &cantus_ui::CantusApp) -> Vec<String> {
    let t = a
        .project()
        .tracks
        .iter()
        .find(|t| t.name == "Piano")
        .unwrap();
    let InstrumentSpec::Native(n) = &t.instrument else {
        panic!()
    };
    (0..MAX_ZONES)
        .filter_map(|i| Zone::of(n, i).and_then(|z| z.sample))
        .collect()
}

/// What an install leaves: `files` in the pack's folder and its receipt.
fn install_here(content: &Path, files: &[&str]) -> PathBuf {
    let pack = content.join(PACK);
    for f in files.iter().chain(["README.txt"].iter()) {
        let p = pack.join(f);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(&p, b"x").unwrap();
    }
    let m = pack_manifest();
    let p = m.pack(PACK).unwrap();
    let r = Receipt {
        id: p.id.clone(),
        version: 1,
        name: p.name.clone(),
        licence: p.licence.clone(),
        licence_url: p.licence_url.clone(),
        licence_file: p.licence_file.clone(),
        credits: p.credits.clone(),
        attribution: p.attribution.clone(),
        instruments: p.instruments.clone(),
        installed_at: 0,
        files: files.len() as u64 + 1,
        bytes: 3,
    };
    std::fs::write(
        pack.join(cantus_content::install::RECEIPT),
        serde_json::to_string(&r).unwrap(),
    )
    .unwrap();
    pack
}

#[test]
fn a_missing_pack_is_named_with_its_tracks_and_installed_from_sound_packs() {
    let dir = tempfile::tempdir().unwrap();
    let content = dir.path().join("Content");
    let files = vec![
        in_pack(&content, "samples/C4v1.flac"),
        in_pack(&content, "samples/C4v2.flac"),
    ];
    for (locale, words, action) in [
        (
            cantus_ui::i18n::Locale::En,
            ["Sound pack not installed", "Fixture Grand Piano", "“Piano”"],
            "Install in Sound packs…",
        ),
        (
            cantus_ui::i18n::Locale::Es,
            [
                "Paquete de sonidos no instalado",
                "Piano de cola de prueba",
                "«Piano»",
            ],
            "Instalar en Paquetes de sonidos…",
        ),
    ] {
        cantus_ui::i18n::set_thread_locale(locale);
        let mut a = opened(&content, &files);
        let ctx = ctx();
        let shown = settle(&mut a, &ctx);
        let all = joined(&shown);
        for w in words {
            assert!(all.contains(w), "{locale:?}: {w:?} not in {all}");
        }
        frame(&mut a, &ctx, click(find(&shown, action)));
        assert!(a.samples.window.open, "Sound packs opens");
        assert_eq!(
            a.samples.window.selected.as_deref(),
            Some(PACK),
            "at the pack"
        );
    }
    cantus_ui::i18n::set_thread_locale(cantus_ui::i18n::Locale::En);
}

#[test]
fn the_card_steps_aside_for_sound_packs_and_its_clicks_reach_the_window() {
    let dir = tempfile::tempdir().unwrap();
    let content = dir.path().join("Content");
    let mut a = opened(&content, &[in_pack(&content, "samples/C4v1.flac")]);
    let ctx = ctx();
    let shown = settle(&mut a, &ctx);
    let card = find(&shown, "Sound pack not installed");
    frame(&mut a, &ctx, click(find(&shown, "Install in Sound packs…")));
    let shown = settle(&mut a, &ctx);
    let all = joined(&shown);
    assert!(
        !all.contains("Sound pack not installed"),
        "the card steps aside: {all}"
    );
    // A pack row of the window where the card was (its lower left) takes the click.
    let row = shown
        .iter()
        .filter(|(s, r)| (s == "Fixture Kit" || s == "Fixture Choir") && r.top() > 0.0)
        .map(|(s, r)| (s.clone(), *r))
        .max_by(|x, y| x.1.top().total_cmp(&y.1.top()))
        .expect("a pack row");
    frame(&mut a, &ctx, click(row.1));
    let want = if row.0 == "Fixture Kit" {
        "fixture-cc0-kit"
    } else {
        "fixture-choir"
    };
    assert_eq!(
        a.samples.window.selected.as_deref(),
        Some(want),
        "card was at {card:?}"
    );
}

#[test]
fn the_rest_of_the_project_still_plays() {
    let dir = tempfile::tempdir().unwrap();
    let content = dir.path().join("Content");
    let missing = in_pack(&content, "samples/C4v1.flac");
    let a = opened(&content, std::slice::from_ref(&missing));
    let mut snap = cantus_core::Snapshot::compile(a.project(), 0);
    let errors = cantus_engine::SampleCache::new().attach(&mut snap);
    assert!(
        errors.iter().all(|(p, _)| slash(p) == missing),
        "only the pack's file fails: {errors:?}"
    );
    let r = cantus_engine::render(
        std::sync::Arc::new(snap),
        &cantus_engine::RenderOptions::default(),
    )
    .unwrap();
    assert!(
        r.samples.iter().any(|s| s.abs() > 1e-3),
        "the other tracks play"
    );
}

#[test]
fn a_project_from_another_computer_uses_the_copy_installed_here_in_one_step() {
    let dir = tempfile::tempdir().unwrap();
    let content = dir.path().join("Content");
    let pack = install_here(&content, &["samples/C4v1.flac", "samples/C4v2.flac"]);
    let theirs: Vec<String> = ["C4v1.flac", "C4v2.flac"]
        .iter()
        .map(|f| format!("C:/Users/someone/AppData/Local/Cantus/Content/{PACK}/samples/{f}"))
        .collect();
    let mut a = opened(&content, &theirs);
    let ctx = ctx();
    let shown = settle(&mut a, &ctx);
    assert!(
        joined(&shown).contains("installed here"),
        "{}",
        joined(&shown)
    );
    frame(&mut a, &ctx, click(find(&shown, "Use the installed pack")));
    for p in sample_paths(&a) {
        assert!(
            Path::new(&p).is_file() && p.starts_with(&slash(&pack)),
            "{p}"
        );
    }
    let all = joined(&settle(&mut a, &ctx));
    assert!(!all.contains("Use the installed pack"), "{all}");
    a.undo();
    assert_eq!(sample_paths(&a), theirs, "one undo step");
}

#[test]
fn a_partly_served_project_relinks_what_it_can_and_then_says_the_pack_is_incomplete() {
    let dir = tempfile::tempdir().unwrap();
    let content = dir.path().join("Content");
    install_here(&content, &["samples/C4v1.flac"]);
    let theirs: Vec<String> = ["C4v1.flac", "C4v2.flac"]
        .iter()
        .map(|f| format!("D:/Other/Cantus/Content/{PACK}/samples/{f}"))
        .collect();
    let mut a = opened(&content, &theirs);
    let ctx = ctx();
    let shown = settle(&mut a, &ctx);
    let all = joined(&shown);
    assert!(
        !all.contains("not installed"),
        "never 'not installed' when it is: {all}"
    );
    frame(&mut a, &ctx, click(find(&shown, "Use the installed pack")));
    let all = joined(&settle(&mut a, &ctx));
    assert!(all.contains("Sound pack incomplete"), "{all}");
    assert!(all.contains("Install again in Sound packs…"), "{all}");
}

#[test]
fn files_deleted_from_an_installed_pack_say_incomplete_and_a_reinstall_brings_them_back() {
    let dir = tempfile::tempdir().unwrap();
    let content = dir.path().join("Content");
    // The pack, installed for real from files (`file://`), so a reinstall is the same version.
    let src = tempfile::tempdir().unwrap();
    let mut entries = Vec::new();
    for f in ["samples/C4v1.flac", "README.txt"] {
        let p = src.path().join(f.replace('/', "_"));
        std::fs::write(&p, b"fLaC").unwrap();
        entries.push(FileEntry {
            url: cantus_content::fetch::file_url(&p),
            to: f.into(),
            size: 4,
            sha256: cantus_content::fetch::sha256_file(&p).unwrap(),
        });
    }
    let mut m = pack_manifest();
    m.packs.iter_mut().find(|p| p.id == PACK).unwrap().source = Source::Files(entries);
    m.packs
        .iter_mut()
        .find(|p| p.id == PACK)
        .unwrap()
        .instruments
        .clear();
    let file = in_pack(&content, "samples/C4v1.flac");
    let mut a = opened(&content, std::slice::from_ref(&file));
    a.samples.manifest = Some(m.clone());
    a.samples
        .set_content(cantus_content::Manager::new(&content, m));
    let ctx = ctx();
    a.samples.content().install(PACK);
    let all = joined(&settle(&mut a, &ctx));
    assert!(!all.contains("Sound pack"), "installed: no card: {all}");
    // A file deleted from the pack's folder.
    std::fs::remove_file(&file).unwrap();
    assert!(
        a.engine.samples.load(Path::new(&file)).is_err(),
        "the engine failed it"
    );
    // Opened again (the scan runs when a project opens).
    let p = a.project().clone();
    a.replace_project(p);
    let all = joined(&settle(&mut a, &ctx));
    assert!(all.contains("Sound pack incomplete"), "{all}");
    assert!(!all.contains("not installed"), "{all}");
    // Installed again, the same version: the card goes and the engine tries the file again
    // at once, with no edit in between.
    a.samples.content().install(PACK);
    let all = joined(&settle(&mut a, &ctx));
    assert!(!all.contains("Sound pack"), "{all}");
    assert!(!matches!(
        a.engine.samples.status(Path::new(&file)),
        cantus_engine::SampleStatus::Failed(_)
    ));
}

#[test]
fn not_now_puts_the_card_away_for_this_project() {
    let dir = tempfile::tempdir().unwrap();
    let content = dir.path().join("Content");
    let mut a = opened(&content, &[in_pack(&content, "samples/C4v1.flac")]);
    let ctx = ctx();
    let shown = settle(&mut a, &ctx);
    frame(&mut a, &ctx, click(find(&shown, "Not now")));
    let all = joined(&settle(&mut a, &ctx));
    assert!(!all.contains("Sound pack not installed"), "{all}");
    // A project without a pack file shows none.
    let b_dir = tempfile::tempdir().unwrap();
    let mut b = app(cantus_ui::starter_project());
    b.samples.content_root = Some(b_dir.path().to_path_buf());
    b.samples.manifest = Some(pack_manifest());
    let all = joined(&settle(&mut b, &support::ctx()));
    assert!(!all.contains("Sound pack"), "{all}");
}

#[test]
fn a_pack_under_any_content_folder_is_recognised() {
    let dir = tempfile::tempdir().unwrap();
    let content = dir.path().join("Library");
    let mut a = opened(&content, &[in_pack(&content, "samples/C4v1.flac")]);
    let all = joined(&settle(&mut a, &ctx()));
    assert!(all.contains("Sound pack not installed"), "{all}");
}

/// Pulsus pads and their layers, and audio clips, play pack files too: they are named,
/// and repointed with the rest.
#[test]
fn drum_pads_layers_and_audio_clips_are_watched_too() {
    let dir = tempfile::tempdir().unwrap();
    let content = dir.path().join("Content");
    let pack = install_here(
        &content,
        &["Kicks/Kick.wav", "Snares/Snare.wav", "Loops/Loop.wav"],
    );
    let theirs =
        |rest: &str| format!("C:/Users/someone/AppData/Local/Cantus/Content/{PACK}/{rest}");
    let mut a = opened_with(&content, |a| {
        // A kit whose first pad plays a pack sample and whose second has a pack layer.
        let mut t = a.bus.new_track("Drums");
        let mut kit = cantus_core::PulsusParams::default();
        kit.pads[0].sample = Some(theirs("Kicks/Kick.wav"));
        kit.pads[1].layers = vec![cantus_core::pads::PadLayer {
            sample: theirs("Snares/Snare.wav"),
            vel_lo: 1,
            vel_hi: 127,
            xfade: 0,
            tune: 0.0,
            level_db: 0.0,
            pan: 0.0,
            start_ms: 0.0,
            reverse: false,
        }];
        t.instrument = InstrumentSpec::Pulsus(kit);
        let len = a.project().tracks.len();
        assert!(a.exec(Command::InsertTrack {
            index: len,
            track: Box::new(t),
        }));
        // An audio source from the pack.
        let id = cantus_core::AudioSourceId(a.bus.alloc_id());
        assert!(a.exec(Command::InsertAudioSource {
            source: cantus_core::AudioSource {
                id,
                name: "Loop".into(),
                path: theirs("Loops/Loop.wav"),
                sample_rate: 48_000,
                channels: 2,
                frames: 48_000,
                sha256: None,
                recorded: false,
            },
        }));
    });
    let ctx = ctx();
    let shown = settle(&mut a, &ctx);
    let all = joined(&shown);
    assert!(all.contains("“Drums”") && all.contains("“Loop”"), "{all}");
    frame(&mut a, &ctx, click(find(&shown, "Use the installed pack")));
    let p = a.project();
    let kit = p
        .tracks
        .iter()
        .find(|t| t.name == "Drums")
        .unwrap()
        .instrument
        .pulsus()
        .unwrap();
    let mine = slash(&pack);
    assert!(kit.pads[0].sample.as_deref().unwrap().starts_with(&mine));
    assert!(kit.pads[1].layers[0].sample.starts_with(&mine));
    assert!(
        p.audio_pool.iter().all(|s| s.path.starts_with(&mine)),
        "{:?}",
        p.audio_pool
    );
    let all = joined(&settle(&mut a, &ctx));
    assert!(!all.contains("Sound pack"), "{all}");
}

/// *Find the folder…*: a pack whose folder was renamed is found where the user points.
#[test]
fn finding_the_folder_repoints_the_files_it_holds() {
    let dir = tempfile::tempdir().unwrap();
    let content = dir.path().join("Content");
    install_here(&content, &["samples/C4v1.flac"]);
    let renamed = content.join("my piano");
    std::fs::rename(content.join(PACK), &renamed).unwrap();
    let file = in_pack(&content, "samples/C4v1.flac");
    let mut a = opened(&content, std::slice::from_ref(&file));
    let ctx = ctx();
    let all = joined(&settle(&mut a, &ctx));
    assert!(
        all.contains("Sound pack incomplete"),
        "a renamed folder: {all}"
    );
    assert!(all.contains("Find the folder…"), "{all}");
    assert!(!cantus_ui::views::samples_content::missing::relink_to(
        &mut a,
        PACK,
        &dir.path().join("elsewhere")
    ));
    assert!(cantus_ui::views::samples_content::missing::relink_to(
        &mut a, PACK, &renamed
    ));
    assert_eq!(
        sample_paths(&a),
        vec![slash(&renamed.join("samples/C4v1.flac"))]
    );
}

/// The disk is never checked on the interface's time, and edits that do not touch a sample
/// path start no scan: a 639-file piano and sixty edits cost the frame a hash.
#[test]
fn edits_start_no_scan_and_a_frame_stays_cheap_with_a_639_file_pack() {
    let dir = tempfile::tempdir().unwrap();
    let content = dir.path().join("Content");
    let names: Vec<String> = (0..639).map(|i| format!("samples/s{i}.flac")).collect();
    let refs: Vec<&str> = names.iter().map(String::as_str).collect();
    install_here(&content, &refs);
    let files: Vec<String> = names.iter().map(|n| in_pack(&content, n)).collect();
    let mut a = opened(&content, &files);
    let ctx = ctx();
    settle(&mut a, &ctx);
    let scans = a.samples.missing.scans;
    let track = a.project().tracks[0].id;
    let mut worst = std::time::Duration::ZERO;
    for i in 0..60 {
        assert!(a.exec(Command::SetTrackName {
            id: track,
            name: format!("Lead {}", i % 2),
        }));
        let t = std::time::Instant::now();
        frame(&mut a, &ctx, vec![]);
        worst = worst.max(t.elapsed());
    }
    assert_eq!(
        a.samples.missing.scans, scans,
        "no scan for edits that touch no sample"
    );
    assert!(!a.samples.missing.busy());
    assert!(
        worst < std::time::Duration::from_millis(8),
        "a frame with 639 pack files costs {worst:?}"
    );
    // Changing a sample path does start one, off the frame.
    let piano = a
        .project()
        .tracks
        .iter()
        .find(|t| t.name == "Piano")
        .unwrap()
        .id;
    assert!(a.exec(Command::SetInstrumentText {
        track: piano,
        key: "z0.sample".into(),
        text: Some(in_pack(&content, "samples/gone.flac")),
    }));
    let all = joined(&settle(&mut a, &ctx));
    assert_eq!(a.samples.missing.scans, scans + 1);
    assert!(all.contains("Sound pack incomplete"), "{all}");
}
