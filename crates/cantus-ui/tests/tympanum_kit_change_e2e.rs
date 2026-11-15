//! Tympanum's kit changes end to end (r40:fix-f, review of 40a27bb7, findings 1, 2 and 7),
//! on the boot path — an app made by `CantusApp::assemble`, the kit loader it installs
//! reading Gogodze Phu Vol. II's published program files (`tests/fixtures/sfz/
//! gogodze_phu_vol_ii`, CC0, byte for byte) as an installed pack:
//!
//! - The studio kit split *Each mic to its own strip*, then Gogodze (12 mics → 7) and back
//!   (7 → 12): every strip named after the mic on it, none left with nothing, each mic on
//!   its own strip, the levels carried by name then family, the sub-groups by family —
//!   one undo step each way, redo the same, and the song saved and opened again the same.
//! - *Each mic to its own strip* again after two mics went back to Main: one step, the
//!   groups made again (it used to roll back: the groups were inserted past the end).
//! - *Pieces to strips* in Spanish names the strips in Spanish.
//!
//! Its own test binary: it points `CANTUS_DATA_DIR` at a temporary folder.

use cantus_core::inst::tympanum::KitRef;
use cantus_core::inst_out::{self, OutputField, OutputPlan};
use cantus_core::*;
use cantus_engine::{EngineHandle, Processor};
use cantus_ui::app::Files;
use cantus_ui::views::inst::tympanum;
use cantus_ui::CantusApp;
use std::path::Path;

fn copy_dir(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap();
    for e in std::fs::read_dir(from).unwrap().flatten() {
        let p = e.path();
        if p.is_dir() {
            copy_dir(&p, &to.join(e.file_name()));
        } else {
            std::fs::copy(&p, to.join(e.file_name())).unwrap();
        }
    }
}

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
        let pack = d.path().join("Content").join("gogodze-phu-vol-ii");
        copy_dir(
            &Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/sfz/gogodze_phu_vol_ii"),
            &pack,
        );
        let receipt = serde_json::json!({
            "id": "gogodze-phu-vol-ii", "version": 1,
            "name": {"en": "Gogodze Phu Vol. II", "es": "Gogodze Phu Vol. II"},
            "licence": "CC0-1.0",
            "licence_url": "https://creativecommons.org/publicdomain/zero/1.0/legalcode",
            "licence_file": "LICENSE", "credits": [],
            "instruments": [{"id": "kit", "name": {"en": "Gogodze", "es": "Gogodze"},
                "file": "Programs/Kit.sfz", "shelf": "drums", "gain_db": 0.0,
                "plays": "tympanum"}],
            "installed_at": 0, "files": 0, "bytes": 0
        });
        std::fs::write(pack.join("cantus-pack.json"), receipt.to_string()).unwrap();
        d
    })
    .path()
}

struct Rig {
    app: CantusApp,
    proc_: Processor,
    ctx: egui::Context,
    t: f64,
    // The scene workers' count is the process's: one test at a time.
    _serial: std::sync::MutexGuard<'static, ()>,
}

static SERIAL: std::sync::Mutex<()> = std::sync::Mutex::new(());

impl Rig {
    fn new(project: Project, locale: cantus_ui::i18n::Locale) -> Rig {
        let serial = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
        cantus_ui::i18n::set_thread_locale(locale);
        let files = Files::read(data(), Some(locale));
        let ctx = egui::Context::default();
        cantus_ui::design::install_fonts(&ctx);
        cantus_ui::design::install_style(&ctx);
        let (engine, proc_) = EngineHandle::manual(48_000.0);
        let app = CantusApp::assemble(&ctx, files, Some(project), engine, None, None, None, false);
        Rig {
            app,
            proc_,
            ctx,
            t: 0.0,
            _serial: serial,
        }
    }

    fn frame(&mut self, events: Vec<egui::Event>) {
        let mut buf = vec![0f32; 800 * 2];
        self.proc_.process(&mut buf, 2);
        self.t += 1.0 / 60.0;
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(1600.0, 900.0),
            )),
            time: Some(self.t),
            predicted_dt: 1.0 / 60.0,
            events,
            ..Default::default()
        };
        let app = &mut self.app;
        self.ctx
            .run_ui(input, |ui| app.frame(ui))
            .textures_delta
            .clear();
    }

    fn idle(&mut self, n: usize) {
        for _ in 0..n {
            self.frame(Vec::new());
        }
    }

    fn click(&mut self, at: egui::Pos2) {
        self.frame(vec![egui::Event::PointerMoved(at)]);
        for pressed in [true, false] {
            self.frame(vec![egui::Event::PointerButton {
                pos: at,
                button: egui::PointerButton::Primary,
                pressed,
                modifiers: Default::default(),
            }]);
        }
        self.idle(1);
    }

    /// A click on the Mixer view's one-click route `plan`.
    fn plan(&mut self, track: TrackId, plan: OutputPlan) {
        self.app.open_inst_editor(track);
        self.app.inst.view = cantus_ui::views::inst::View::Mixer;
        self.idle(3);
        let at = tympanum::plan_rect(&self.ctx, track, plan).expect("the route is drawn");
        self.click(at.center());
    }
}

/// A song with one Tympanum on the studio kit, Drums, on the first strip.
fn kit_song() -> (Project, TrackId) {
    let mut p = Project::new("Tympanum");
    let mut t = p.make_track("Drums");
    t.strip = Some(p.strips[0].id);
    t.instrument = InstrumentSpec::Native(NativeSpec::new(InstrumentKind::AcousticDrums));
    let id = t.id;
    p.tracks.push(t);
    (p, id)
}

fn gogodze() -> KitRef {
    KitRef::Pack {
        pack: "gogodze-phu-vol-ii".into(),
        kit: "kit".into(),
    }
}

/// What a split kit must be: every mic of its definition on a strip of its own, each strip
/// named after its mic, no strip made for a mic left with nothing, every sub-group one
/// family's.
fn assert_split(p: &Project, track: TrackId, mics: usize) {
    let n = p.track(track).unwrap().instrument.native().unwrap();
    let def = inst_out::definition(p, track);
    assert_eq!(def.len(), mics, "{def:?}");
    assert_eq!(
        n.outputs.len(),
        mics,
        "every mic on a strip: {:?}",
        n.outputs
    );
    assert_eq!(inst_out::strips_of(&n.outputs).len(), mics, "a strip each");
    for s in &n.outputs {
        let strip = p.strip(s.strip).expect("its strip");
        assert_eq!(strip.name, s.name, "the strip is named after its mic");
        assert!(def.iter().any(|o| o.id == s.id && o.name == s.name));
    }
    assert!(
        padout::orphans(p).is_empty(),
        "no strip left with nothing: {:?}",
        padout::orphans(p)
    );
    for g in &p.strip_groups {
        let fams: Vec<OutputFamily> = p
            .strips
            .iter()
            .filter(|s| s.group == Some(g.id))
            .flat_map(|s| n.outputs.iter().filter(move |o| o.strip == s.id))
            .map(|o| o.family)
            .collect();
        assert!(fams.len() > 1, "a group of one family's strips: {}", g.name);
        assert!(fams.iter().all(|f| *f == fams[0]), "{}: {fams:?}", g.name);
    }
}

/// The song as undo restores it: the id counter (new strips' ids are drawn from the live
/// song, outside any command) aside.
fn song_of(p: &Project) -> Project {
    let mut q = p.clone();
    q.next_id = 0;
    q
}

fn level(p: &Project, track: TrackId, id: OutputId) -> f32 {
    let n = p.track(track).unwrap().instrument.native().unwrap();
    n.get(&inst_out::key(id, OutputField::Level))
}

#[test]
fn a_kit_change_carries_a_split_mixer_both_ways_in_one_step() {
    let (p, drums) = kit_song();
    let mut r = Rig::new(p, cantus_ui::i18n::Locale::En);
    r.plan(drums, OutputPlan::EachOwn);
    assert_split(r.app.project(), drums, 12);
    let studio = inst_out::definition(r.app.project(), drums);
    let id_of = |name: &str| studio.iter().find(|o| o.name == name).unwrap().id;
    // The mixer: the snare's level, and a mic the new kit has no place for turned off.
    let snare = id_of("Snare Top");
    let floor2 = id_of("Floor 2");
    r.app.set_inst_value(
        drums,
        &inst_out::key(snare, OutputField::Level),
        -4.5,
        false,
    );
    r.app
        .set_inst_value(drums, &inst_out::key(floor2, OutputField::On), 0.0, false);
    let split = r.app.project().clone();

    // 12 → 7: one step.
    assert!(tympanum::kit_change::change_kit(
        &mut r.app,
        drums,
        &gogodze()
    ));
    let seven = r.app.project().clone();
    let n = seven.track(drums).unwrap().instrument.native().unwrap();
    assert_eq!(KitRef::of(n), gogodze());
    assert_split(&seven, drums, 7);
    let g = inst_out::definition(&seven, drums);
    let g_snare = g
        .iter()
        .find(|o| o.family == OutputFamily::Snare)
        .expect("Gogodze's snare mic");
    assert_eq!(level(&seven, drums, g_snare.id), -4.5, "carried by family");
    for o in &g {
        assert!(
            n.get(&inst_out::key(o.id, OutputField::On)) != 0.0,
            "no stale power key turns {} off",
            o.name
        );
    }
    r.app.undo();
    assert_eq!(
        song_of(r.app.project()),
        song_of(&split),
        "one undo step back to the studio kit"
    );
    r.app.redo();
    assert_eq!(song_of(r.app.project()), song_of(&seven), "redo the same");

    // 7 → 12: the studio's mics the kit lacked get strips of their own too.
    assert!(tympanum::kit_change::change_kit(
        &mut r.app,
        drums,
        &KitRef::Modelled("studio".into())
    ));
    let twelve = r.app.project().clone();
    assert_split(&twelve, drums, 12);
    let back = twelve.track(drums).unwrap().instrument.native().unwrap();
    assert!(
        back.outputs
            .iter()
            .any(|o| o.family == OutputFamily::Snare && level(&twelve, drums, o.id) == -4.5),
        "the snare's level came back with it"
    );
    r.app.undo();
    assert_eq!(song_of(r.app.project()), song_of(&seven));
    r.app.redo();
    assert_eq!(song_of(r.app.project()), song_of(&twelve));

    // Saved and opened again: the same mixer.
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("kit.cantus");
    cantus_project::save(r.app.project(), &path, "test").unwrap();
    let reopened = cantus_project::load(&path, false).unwrap().project;
    assert_split(&reopened, drums, 12);
    assert_eq!(
        reopened.track(drums).unwrap().instrument,
        twelve.track(drums).unwrap().instrument
    );
}

#[test]
fn each_mic_again_after_two_went_back_to_main_is_one_step() {
    let (p, drums) = kit_song();
    let mut r = Rig::new(p, cantus_ui::i18n::Locale::En);
    r.plan(drums, OutputPlan::EachOwn);
    let groups = r.app.project().strip_groups.len();
    assert!(groups > 0, "sub-groups by family");
    let def = inst_out::definition(r.app.project(), drums);
    for name in ["Tom 1", "Tom 2"] {
        let id = def.iter().find(|o| o.name == name).unwrap().id;
        let cmd = inst_out::set_route(r.app.project(), drums, id, None).unwrap();
        assert!(r.app.exec(cmd));
    }
    let before = r.app.project().clone();
    r.plan(drums, OutputPlan::EachOwn);
    let after = r.app.project().clone();
    assert_ne!(after, before, "the plan ran (it used to roll back)");
    let n = after.track(drums).unwrap().instrument.native().unwrap();
    assert_eq!(n.outputs.len(), 12, "every mic on a strip again");
    let toms: Vec<_> = n
        .outputs
        .iter()
        .filter(|o| o.family == OutputFamily::Toms)
        .map(|o| after.strip(o.strip).unwrap().group)
        .collect();
    assert!(
        toms.iter().all(|g| g.is_some() && *g == toms[0]),
        "the toms under one group: {toms:?}"
    );
    r.app.undo();
    assert_eq!(song_of(r.app.project()), song_of(&before), "one step");
    // And once more after a kit change.
    r.app.redo();
    assert!(tympanum::kit_change::change_kit(
        &mut r.app,
        drums,
        &gogodze()
    ));
    assert_split(r.app.project(), drums, 7);
}

#[test]
fn pieces_to_strips_names_the_strips_in_spanish() {
    let (p, drums) = kit_song();
    let mut r = Rig::new(p, cantus_ui::i18n::Locale::Es);
    r.plan(drums, OutputPlan::ByFamily);
    let p = r.app.project();
    let n = p.track(drums).unwrap().instrument.native().unwrap();
    let names: Vec<String> = inst_out::strips_of(&n.outputs)
        .iter()
        .map(|s| p.strip(*s).unwrap().name.clone())
        .collect();
    assert!(names.contains(&"Bombo".to_string()), "{names:?}");
    assert!(!names.contains(&"Kick".to_string()), "{names:?}");
    for f in [OutputFamily::Kick, OutputFamily::Snare, OutputFamily::Toms] {
        let es = cantus_ui::i18n::t(f.label_key());
        assert!(names.contains(&es), "{es} in {names:?}");
    }
}

/// Finding 3's Tympanum half: a preset naming no kit (a user's, A/B) keeps the kit, its
/// routes and its mixer; one naming another kit changes it as the kits list does.
#[test]
fn a_preset_keeps_the_kit_and_its_mixer_or_changes_kit_as_the_list_does() {
    let (p, drums) = kit_song();
    let mut r = Rig::new(p, cantus_ui::i18n::Locale::En);
    assert!(tympanum::kit_change::change_kit(
        &mut r.app,
        drums,
        &gogodze()
    ));
    r.plan(drums, OutputPlan::EachOwn);
    assert_split(r.app.project(), drums, 7);
    let first = r.app.native_of(drums).unwrap().outputs[0].id;
    r.app.set_inst_value(
        drums,
        &inst_out::key(first, OutputField::Level),
        -6.0,
        false,
    );
    let before = r.app.native_of(drums).unwrap();
    let mut preset = NativeSpec::new(InstrumentKind::AcousticDrums);
    preset.set("amb.level", -3.0);
    r.app.apply_inst_spec(drums, &preset, "cmd.fx_preset");
    let after = r.app.native_of(drums).unwrap();
    assert_eq!(KitRef::of(&after), gogodze(), "the kit kept");
    assert_eq!(after.outputs, before.outputs);
    assert_eq!(level(r.app.project(), drums, first), -6.0);
    assert_eq!(after.get("amb.level"), -3.0);
    // A preset saved on the studio kit: the kit changes, the mixer carried, one step.
    let mut studio = preset.clone();
    studio
        .set_text(
            cantus_core::inst::tympanum::KIT_KEY,
            Some("model:studio".into()),
        )
        .unwrap();
    let was = r.app.project().clone();
    r.app.apply_inst_spec(drums, &studio, "cmd.fx_preset");
    assert_split(r.app.project(), drums, 12);
    r.app.undo();
    assert_eq!(song_of(r.app.project()), song_of(&was), "one step");
}

/// Finding 6: each scene shown keeps a place in one shared worker (not a thread of its
/// own); a track deleted drops its scenes, another song drops them all — back to the
/// baseline.
#[test]
fn scenes_of_deleted_tracks_and_old_songs_are_freed() {
    let mut p = Project::new("Three kits");
    let mut ids = Vec::new();
    for i in 0..3 {
        let mut t = p.make_track(format!("Drums {i}"));
        t.instrument = InstrumentSpec::Native(NativeSpec::new(InstrumentKind::AcousticDrums));
        ids.push(t.id);
        p.tracks.push(t);
    }
    let mut r = Rig::new(p, cantus_ui::i18n::Locale::En);
    r.idle(2);
    let (base, _) = cantus_ui::scene::workers();
    for id in &ids {
        r.app.open_inst_editor(*id);
        r.app.inst.view = cantus_ui::views::inst::View::Play;
        r.idle(3);
    }
    let (shown, threads) = cantus_ui::scene::workers();
    assert!(shown >= base + 3, "a scene each: {shown} over {base}");
    assert!(threads <= 1, "one worker thread for all of them: {threads}");
    r.app.close_inst_editor();
    for id in &ids[..2] {
        assert!(r.app.exec(Command::RemoveTrack { id: *id }));
    }
    r.idle(2);
    assert!(
        r.app
            .inst
            .tympanum_scenes
            .keys()
            .all(|(t, _)| *t == ids[2].0),
        "only the kept track's scenes: {:?}",
        r.app.inst.tympanum_scenes.keys().collect::<Vec<_>>()
    );
    // Another song: none at all.
    let other = Project::new("Other");
    assert!(r.app.exec(Command::ReplaceProject {
        project: Box::new(other)
    }));
    r.idle(2);
    assert!(r.app.inst.tympanum_scenes.is_empty());
    assert_eq!(r.app.inst.chorda.scenes_kept(), 0);
    assert_eq!(cantus_ui::scene::workers().0, base, "back to the baseline");
}

/// Finding 9: the Out lists (Tympanum's mics, Chorda's perspectives — one list) never offer
/// a VCA, which `set_route` refuses (the menu used to close silently on one).
#[test]
fn the_out_list_offers_no_vca() {
    let (mut p, drums) = kit_song();
    p.strips[2].name = "Bus A".into();
    p.strips[3].name = "Drums VCA".into();
    p.strips[3].kind = cantus_core::vca::StripKind::Vca;
    let vca = p.strips[3].id;
    let bus = p.strips[2].id;
    let r = Rig::new(p, cantus_ui::i18n::Locale::En);
    let offered = cantus_ui::views::inst::inst_mixer::out_targets(r.app.project(), drums);
    assert!(offered.iter().any(|(_, s)| *s == Some(bus)), "{offered:?}");
    assert!(!offered.iter().any(|(_, s)| *s == Some(vca)), "{offered:?}");
    assert_eq!(offered[0].1, None, "Main first");
}

/// DRSKit's published kit files with a short 13-channel hit written for every file they
/// name (as `cantus-content`'s tests write them).
fn drskit_with_hits(to: &Path) -> std::path::PathBuf {
    let src = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../cantus-content/tests/fixtures/drskit_minimal");
    copy_dir(&src, to);
    let kit = cantus_content::drumgizmo::read_kit(to, "DRSKit_minimal.xml").unwrap();
    for f in cantus_content::drumgizmo::files(&kit) {
        let path = cantus_content::drumgizmo::path_of(to, &f);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        let spec = hound::WavSpec {
            channels: 13,
            sample_rate: 48_000,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };
        let mut w = hound::WavWriter::create(&path, spec).unwrap();
        for i in 0..4_800 {
            for c in 0..13 {
                let x = ((i as f32) * (0.02 + 0.01 * c as f32)).sin() * 0.3;
                w.write_sample((x * 32_000.0) as i16).unwrap();
            }
        }
        w.finalize().unwrap();
    }
    to.join("DRSKit_minimal.xml")
}

/// The coordinator's follow-up: *Install from a file…* of a DrumGizmo kit runs in the
/// background — the window keeps drawing frames while it unpacks and builds, the studio
/// kit keeps playing until the new one is in, then the track takes it; a cancelled install
/// leaves nothing behind and changes nothing.
#[test]
fn install_from_a_file_runs_in_the_background_and_a_cancel_leaves_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let xml = drskit_with_hits(&dir.path().join("DRSKit"));
    let (p, drums) = kit_song();
    let mut r = Rig::new(p, cantus_ui::i18n::Locale::En);
    r.app.open_inst_editor(drums);
    r.app.inst.view = cantus_ui::views::inst::View::Play;
    r.idle(2);
    tympanum::kits::install_kit_path(&mut r.app, drums, &xml);
    assert_eq!(r.app.samples.user_installs.len(), 1, "started, not done");
    let studio = KitRef::Modelled("studio".into());
    let mut busy_frames = 0;
    let mut slowest = std::time::Duration::ZERO;
    let until = std::time::Instant::now() + std::time::Duration::from_secs(120);
    while !r.app.samples.user_installs.is_empty() && std::time::Instant::now() < until {
        assert_eq!(
            KitRef::of(&r.app.native_of(drums).unwrap()),
            studio,
            "the old kit plays until the new one is in"
        );
        let t0 = std::time::Instant::now();
        r.frame(Vec::new());
        slowest = slowest.max(t0.elapsed());
        busy_frames += 1;
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    assert!(r.app.samples.user_installs.is_empty(), "it finished");
    assert!(
        busy_frames > 2,
        "frames kept coming while it installed: {busy_frames}"
    );
    assert!(
        slowest < std::time::Duration::from_secs(2),
        "no frame waited for the install: {slowest:?}"
    );
    let n = r.app.native_of(drums).unwrap();
    let KitRef::Pack { pack, .. } = KitRef::of(&n) else {
        panic!("the user's kit plays: {:?}", KitRef::of(&n));
    };
    assert!(pack.starts_with("user-"));
    assert_eq!(inst_out::definition(r.app.project(), drums).len(), 11);

    // A second file, cancelled at once: nothing left, nothing changed.
    let other = dir.path().join("Other");
    let xml2 = drskit_with_hits(&other);
    let before = r.app.project().clone();
    tympanum::kits::install_kit_path(&mut r.app, drums, &xml2);
    let id = r.app.samples.user_installs[0].id.clone();
    r.app.samples.content().cancel(&id);
    r.app.wait_user_installs(std::time::Duration::from_secs(60));
    assert!(r.app.samples.user_installs.is_empty());
    assert_eq!(
        song_of(r.app.project()),
        song_of(&before),
        "nothing changed"
    );
    let content = data().join("Content");
    let left: Vec<String> = std::fs::read_dir(&content)
        .unwrap()
        .flatten()
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| n == &id || n.starts_with(".user-staging-"))
        .collect();
    assert!(left.is_empty(), "nothing left behind: {left:?}");
}

/// The coordinator's follow-up: a modelled kit's mics are Cantus's, said in Spanish — the
/// strips *Each mic to its own strip* makes, the Mixer's strips and the mic panel — while
/// the song keeps each output's own name (a kit change still carries by it).
#[test]
fn a_modelled_kits_mics_are_named_in_spanish() {
    let (p, drums) = kit_song();
    let mut r = Rig::new(p, cantus_ui::i18n::Locale::Es);
    r.plan(drums, OutputPlan::EachOwn);
    let p = r.app.project();
    let n = p.track(drums).unwrap().instrument.native().unwrap();
    let strip_of = |name: &str| {
        let s = n.outputs.iter().find(|o| o.name == name).unwrap();
        p.strip(s.strip).unwrap().name.clone()
    };
    assert_eq!(strip_of("Kick Out"), "Bombo fuera");
    assert_eq!(strip_of("Floor 1"), "Base 1");
    assert_eq!(strip_of("Overheads"), "Aéreos");
    assert_eq!(strip_of("Room Near"), "Sala cerca");
    assert_eq!(strip_of("Hi-hat"), "Charles");
    let def = tympanum::def::def_of(&r.app.native_of(drums).unwrap());
    let said: Vec<String> = def
        .mics
        .iter()
        .map(|m| tympanum::def::mic_name(&def, m))
        .collect();
    assert!(said.contains(&"Sala mono".to_string()), "{said:?}");
    // A recorded kit keeps its pack's names.
    assert!(tympanum::kit_change::change_kit(
        &mut r.app,
        drums,
        &gogodze()
    ));
    let p = r.app.project();
    let n = p.track(drums).unwrap().instrument.native().unwrap();
    for o in &n.outputs {
        assert_eq!(
            p.strip(o.strip).unwrap().name,
            o.name,
            "the pack's own name"
        );
    }
}
