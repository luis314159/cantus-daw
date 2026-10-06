//! The sample browser (ADR 0030): it lists installed packs and the user's folders, opens
//! them, filters by search; a click auditions; a drag onto a pad — aimed through the
//! geometry the views publish — makes the pad play the file in one undo step; a drop on
//! the rack makes a channel; the starter offer installs in one click; the content window
//! shows a pack's licence. No network: packs are written on the spot.

mod support;
use cantus_content::manifest::{Category, Credit, FileEntry, Manifest, Pack, Source, Text};
use cantus_content::Receipt;
use cantus_core::*;
use cantus_ui::samples::drop::{self, Target};
use cantus_ui::views::{pulsus_editor, samples_browser};
use cantus_ui::CantusApp;
use egui::{pos2, vec2, Event, Pos2, Rect};
use std::path::{Path, PathBuf};
use support::*;

const SIZE: egui::Vec2 = egui::vec2(1500.0, 900.0);

fn wav(path: &Path, freq: f32) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    let spec = hound::WavSpec { channels: 1, sample_rate: 44_100, bits_per_sample: 16, sample_format: hound::SampleFormat::Int };
    let mut w = hound::WavWriter::create(path, spec).unwrap();
    for i in 0..4_410 {
        w.write_sample(((i as f32 * freq / 44_100.0 * std::f32::consts::TAU).sin() * 12_000.0) as i16).unwrap();
    }
    w.finalize().unwrap();
}

struct Fixture {
    _dir: tempfile::TempDir,
    content: PathBuf,
    user: PathBuf,
}

/// A content folder with one installed pack, and a folder of the user's.
fn fixture() -> Fixture {
    let dir = tempfile::tempdir().unwrap();
    let content = dir.path().join("Content");
    let pack = content.join("test-kit");
    wav(&pack.join("Kicks/Kick 1.wav"), 60.0);
    wav(&pack.join("Kicks/Kick 2.wav"), 70.0);
    wav(&pack.join("Snares/Snare.wav"), 200.0);
    std::fs::write(pack.join("LICENSE"), "CC0").unwrap();
    let receipt = Receipt {
        id: "test-kit".into(),
        version: 1,
        name: Text { en: "Test Kit".into(), es: "Kit de prueba".into() },
        licence: "CC0-1.0".into(),
        licence_url: "https://creativecommons.org/publicdomain/zero/1.0/legalcode".into(),
        licence_file: "LICENSE".into(),
        credits: vec![],
        installed_at: 0,
        files: 4,
        bytes: 0,
    };
    std::fs::write(pack.join(cantus_content::install::RECEIPT), serde_json::to_string(&receipt).unwrap()).unwrap();
    let user = dir.path().join("My Sounds");
    wav(&user.join("My kick.wav"), 55.0);
    std::fs::write(user.join("notes.txt"), "not audio").unwrap();
    Fixture { content, user, _dir: dir }
}

fn browser_app(f: &Fixture, manifest: Manifest) -> CantusApp {
    let mut a = app(cantus_ui::starter_project());
    a.settings_path = f.content.parent().unwrap().join("settings.json");
    a.samples.content_root = Some(f.content.clone());
    a.samples.manifest = Some(manifest);
    a.settings.sample_folders = vec![f.user.clone()];
    a
}

fn empty_manifest() -> Manifest {
    Manifest { format: 1, updated: "2026-10-05".into(), packs: vec![] }
}

/// One frame of the window as the app draws it: the browser at the left, the Pulsus
/// editor at the right, then the end-of-frame drag handling.
fn frame(ctx: &egui::Context, a: &mut CantusApp, events: Vec<Event>) {
    let input = egui::RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, SIZE)), events, ..Default::default() };
    let mut out = ctx.run_ui(input, |ui| {
        let ctx = ui.ctx().clone();
        a.poll_samples(&ctx);
        let left = Rect::from_min_size(Pos2::ZERO, vec2(300.0, SIZE.y));
        let right = Rect::from_min_max(pos2(320.0, 0.0), SIZE.to_pos2());
        let mut l = ui.new_child(egui::UiBuilder::new().max_rect(left));
        samples_browser::show(a, &mut l);
        let mut r = ui.new_child(egui::UiBuilder::new().max_rect(right));
        pulsus_editor::window_in(a, &mut r);
        samples_browser::frame(a, &ctx);
    });
    out.textures_delta.clear();
}

fn click(ctx: &egui::Context, a: &mut CantusApp, at: Pos2) {
    frame(ctx, a, vec![Event::PointerMoved(at)]);
    frame(ctx, a, vec![Event::PointerMoved(at), press(at, true)]);
    frame(ctx, a, vec![Event::PointerMoved(at), press(at, false)]);
    frame(ctx, a, vec![]);
}

fn press(at: Pos2, pressed: bool) -> Event {
    Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed, modifiers: egui::Modifiers::NONE }
}

fn row_names(a: &CantusApp) -> Vec<String> {
    a.samples.geometry.as_ref().unwrap().rows.iter().map(|r| r.path.file_name().unwrap().to_string_lossy().into_owned()).collect()
}

fn row(a: &CantusApp, path: &Path) -> Rect {
    a.samples.geometry.as_ref().unwrap().row_of(path).unwrap_or_else(|| panic!("{path:?} is listed")).rect
}

#[test]
fn the_browser_lists_packs_and_folders_opens_them_and_filters() {
    let f = fixture();
    let mut a = browser_app(&f, empty_manifest());
    let ctx = ctx();
    frame(&ctx, &mut a, vec![]);
    let pack = f.content.join("test-kit");
    assert_eq!(row_names(&a), ["test-kit", "My Sounds"], "roots first, closed");

    let at = row(&a, &pack).center();
    click(&ctx, &mut a, at);
    assert_eq!(row_names(&a), ["test-kit", "Kicks", "Snares", "My Sounds"]);
    let at = row(&a, &pack.join("Kicks")).center();
    click(&ctx, &mut a, at);
    let at = row(&a, &f.user).center();
    click(&ctx, &mut a, at);
    assert_eq!(
        row_names(&a),
        ["test-kit", "Kicks", "Kick 1.wav", "Kick 2.wav", "Snares", "My Sounds", "My kick.wav"],
        "natural order, folders first, and nothing that is not audio"
    );

    a.samples.query = "kick".into();
    for _ in 0..400 {
        frame(&ctx, &mut a, vec![]);
        if !a.samples.indexing() && !a.samples.geometry.as_ref().unwrap().rows.is_empty() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    let mut hits = row_names(&a);
    hits.sort();
    assert_eq!(hits, ["Kick 1.wav", "Kick 2.wav", "My kick.wav"], "the search spans packs and folders");
    a.samples.query = "snare".into();
    frame(&ctx, &mut a, vec![]);
    assert_eq!(row_names(&a), ["Snare.wav"]);
}

#[test]
fn a_click_auditions_and_a_second_click_stops() {
    let f = fixture();
    let mut a = browser_app(&f, empty_manifest());
    a.samples.open.insert(f.user.clone());
    let ctx = ctx();
    frame(&ctx, &mut a, vec![]);
    let kick = f.user.join("My kick.wav");
    let at = row(&a, &kick).center();
    click(&ctx, &mut a, at);
    assert_eq!(a.samples.selected.as_deref(), Some(kick.as_path()));
    for _ in 0..400 {
        frame(&ctx, &mut a, vec![]);
        if a.samples.preview.playing.is_some() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    assert!(a.samples.preview.is_playing(&kick), "decoded off the interface thread, then played");
    let at = row(&a, &kick).center();
    click(&ctx, &mut a, at);
    assert!(a.samples.preview.playing.is_none(), "a second click stops it");
}

fn kit_of(a: &CantusApp, track: TrackId) -> PulsusParams {
    a.project().track(track).unwrap().instrument.pulsus().unwrap().clone()
}

#[test]
fn a_sample_dragged_onto_a_pad_becomes_its_sound_in_one_undo_step() {
    let f = fixture();
    let mut a = browser_app(&f, empty_manifest());
    let track = a.project().tracks[0].id;
    a.set_instrument(track, InstrumentSpec::Pulsus(PulsusParams::default()));
    pulsus_editor::open(&mut a, track);
    a.samples.open.insert(f.user.clone());
    let ctx = ctx();
    frame(&ctx, &mut a, vec![]);
    frame(&ctx, &mut a, vec![]);
    let kick = f.user.join("My kick.wav");
    let from = row(&a, &kick).center();
    // Where the pad is comes from what the editor published, never from its layout.
    let pad_rect = drop::targets(&ctx)
        .into_iter()
        .find_map(|(r, t)| (t == Target::Pad { track, pad: 1 }).then_some(r))
        .expect("the editor publishes its pads");
    let to = pad_rect.center();
    let history = a.bus.history().len();

    frame(&ctx, &mut a, vec![Event::PointerMoved(from)]);
    frame(&ctx, &mut a, vec![Event::PointerMoved(from), press(from, true)]);
    for i in 1..=8 {
        let p = from + (to - from) * (i as f32 / 8.0);
        frame(&ctx, &mut a, vec![Event::PointerMoved(p)]);
    }
    assert!(a.samples.drag.is_some(), "the row became a drag");
    assert_eq!(drop::hovered(&a, &ctx).map(|(_, t)| t), Some(Target::Pad { track, pad: 1 }), "the pad lights under the chip");
    frame(&ctx, &mut a, vec![Event::PointerMoved(to), press(to, false)]);
    frame(&ctx, &mut a, vec![]);

    let pad = &kit_of(&a, track).pads[1];
    assert_eq!(pad.sample.as_deref(), Some(kick.to_string_lossy().as_ref()));
    assert_eq!(pad.voice, PadVoice::Sample);
    assert_eq!(a.bus.history().len(), history + 1, "one undo step");
    a.undo();
    assert_ne!(kit_of(&a, track).pads[1].sample.as_deref(), Some(kick.to_string_lossy().as_ref()));
}

#[test]
fn a_sample_dropped_on_the_rack_becomes_a_channel_and_on_a_one_pad_channel_its_sound() {
    let f = fixture();
    let mut a = browser_app(&f, empty_manifest());
    let n = a.project().tracks.len();
    let kick = f.user.join("My kick.wav");
    drop::land(&mut a, &kick, Target::Rack, Rect::EVERYTHING, pos2(10.0, 10_000.0));
    assert_eq!(a.project().tracks.len(), n + 1);
    let t = a.project().tracks.last().unwrap();
    assert_eq!(t.name, "My kick");
    let kit = t.instrument.pulsus().expect("a sample channel is a one-pad kit");
    assert_eq!(kit.pads.len(), 1);
    assert_eq!(kit.pads[0].voice, PadVoice::Sample);
    assert!(t.strip.is_some(), "with a strip of its own");
    a.undo();
    assert_eq!(a.project().tracks.len(), n, "one undo step");
}

#[test]
fn the_starter_offer_installs_the_recommended_pack_in_one_click() {
    let f = fixture();
    // Nothing installed: a content folder of its own.
    let empty = f.content.parent().unwrap().join("Empty");
    let src = f.content.join("test-kit/Kicks/Kick 1.wav");
    let bytes = std::fs::read(&src).unwrap();
    let licence = f.content.join("test-kit/LICENSE");
    let entry = |p: &Path, to: &str| {
        let b = std::fs::read(p).unwrap();
        FileEntry {
            url: cantus_content::fetch::file_url(p),
            to: to.into(),
            size: b.len() as u64,
            sha256: cantus_content::fetch::sha256_file(p).unwrap(),
        }
    };
    let pack = Pack {
        id: "starter".into(),
        version: 1,
        name: Text { en: "Starter".into(), es: "Inicial".into() },
        description: Text { en: "d".into(), es: "d".into() },
        category: Category::Drums,
        recommended: true,
        licence: "CC0-1.0".into(),
        licence_url: "https://creativecommons.org/publicdomain/zero/1.0/legalcode".into(),
        licence_file: "LICENSE".into(),
        credits: vec![Credit { name: "t".into(), url: "https://example.org".into(), licence_url: "https://example.org".into() }],
        checked: "2026-10-05".into(),
        installed_size: bytes.len() as u64,
        source: Source::Files(vec![entry(&src, "Kicks/Kick.wav"), entry(&licence, "LICENSE")]),
    };
    let mut a = browser_app(&f, Manifest { format: 1, updated: "2026-10-05".into(), packs: vec![pack] });
    a.samples.content_root = Some(empty.clone());
    a.settings.sample_folders.clear();
    let ctx = ctx();
    frame(&ctx, &mut a, vec![]);
    let g = a.samples.geometry.clone().unwrap();
    let actions = g.starter_actions.expect("the offer shows while nothing is installed");
    click(&ctx, &mut a, pos2(actions.left() + 20.0, actions.center().y));
    for _ in 0..400 {
        frame(&ctx, &mut a, vec![]);
        if a.samples.content().is_installed("starter") {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    assert!(empty.join("starter/Kicks/Kick.wav").is_file());
    frame(&ctx, &mut a, vec![]);
    let g = a.samples.geometry.clone().unwrap();
    assert!(g.starter.is_none(), "the offer goes once a pack is installed");
    assert_eq!(row_names(&a), ["starter"], "and the pack is listed");
}

#[test]
fn the_content_window_shows_every_pack_with_its_licence() {
    let f = fixture();
    let mut a = browser_app(&f, Manifest::bundled());
    a.samples.window.open = true;
    let ctx = ctx();
    let text = paint(&ctx, SIZE, 3, |ui| {
        let ctx = ui.ctx().clone();
        cantus_ui::views::samples_content::window(&mut a, &ctx);
    });
    assert!(text.contains("Cantus Starter Drums"), "{text}");
    assert!(text.contains("CC0-1.0"), "{text}");
    assert!(text.contains("Virtuosity Drums"), "the credits are shown: {text}");
}
