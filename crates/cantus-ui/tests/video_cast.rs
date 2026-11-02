//! The video's dancers (the integrated tree's review): an imported character still being
//! read is waited for and drawn — never the default in its place — one that cannot be read
//! stops the export in words, a hidden dancer is left out, the preview is the moment's own
//! render (automation included), and *Export video…* is a palette action.
//! Its own test binary: it points `CANTUS_DATA_DIR` at a temporary folder.

use cantus_core::inst::dancer::{self, Move};
use cantus_core::*;
use cantus_engine::EngineHandle;
use cantus_ui::i18n::{self, Locale};
use cantus_ui::video_export::{self as vx, job, Format};
use cantus_ui::CantusApp;

fn data() -> &'static std::path::Path {
    static DIR: std::sync::OnceLock<tempfile::TempDir> = std::sync::OnceLock::new();
    DIR.get_or_init(|| {
        let d = tempfile::tempdir().unwrap();
        std::env::set_var("CANTUS_DATA_DIR", d.path());
        d
    })
    .path()
}

static ONE: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn one() -> std::sync::MutexGuard<'static, ()> {
    let g = ONE.lock().unwrap_or_else(|e| e.into_inner());
    i18n::set_thread_locale(Locale::En);
    let _ = data();
    g
}

/// 120 BPM, four bars of a lead, and a dancer whose spec `edit` sets; clapping over bars
/// 3–4 by automation when `clap`.
fn song(edit: impl Fn(&mut NativeSpec), dancers: usize, clap: bool) -> (Project, Vec<TrackId>) {
    let mut p = Project::new("dance");
    p.tempo_bpm = 120.0;
    let mut lead = p.make_track("Lead");
    let mut c = p.make_clip("beats", Ticks::ZERO, Ticks::beats(16));
    for b in 0..16 {
        c.notes
            .push(p.make_note(Ticks::beats(b), Ticks::EIGHTH, 60, 90));
    }
    lead.clips.push(c);
    p.tracks.push(lead);
    let mut ids = Vec::new();
    for i in 0..dancers {
        let mut t = p.make_track(format!("Dancer {i}"));
        let mut spec = NativeSpec::new(InstrumentKind::Dancer);
        if i == 0 {
            edit(&mut spec);
        }
        t.instrument = InstrumentSpec::Native(spec);
        ids.push(t.id);
        if clap && i == 0 {
            let value = dancer::move_value(Move::Clap) / 16.0;
            let auto = p.make_automation_clip(
                "Clap",
                AutoTarget::Instrument {
                    track: t.id,
                    key: "move".into(),
                },
                Ticks::beats(8),
                Ticks::beats(8),
                vec![
                    AutoPoint {
                        tick: Ticks::ZERO,
                        value,
                        tension: 0.0,
                        shape: Shape::Hold,
                    },
                    AutoPoint {
                        tick: Ticks::beats(8),
                        value: 0.0,
                        tension: 0.0,
                        shape: Shape::Hold,
                    },
                ],
            );
            t.clips.push(auto);
        }
        p.tracks.push(t);
    }
    (p, ids)
}

fn app_with(p: Project, folder: &std::path::Path) -> CantusApp {
    let mut a = CantusApp::from_parts(Some(p), EngineHandle::dummy(), false);
    a.settings_path = folder.join("settings.json");
    a.path = Some(folder.join("song.cantus"));
    a
}

fn small_mov(a: &CantusApp) -> vx::VideoSettings {
    let mut d = a.settings.video.clone();
    d.format = Format::Mov;
    d.shape = vx::Shape::Custom;
    d.custom = (720, 1280);
    d.title = false;
    d
}

/// An imported sprite sheet copied into the project's folder; its `file:` key.
fn imported(project: &std::path::Path) -> String {
    use cantus_dance::import::{package, samples};
    let src = tempfile::tempdir().unwrap();
    samples::write(src.path(), &samples::aseprite("pal")).unwrap();
    let p = src.path().join("pal.json");
    let found = cantus_dance::import::inspect(&p).unwrap();
    let settings = cantus_dance::import::guessed(&found);
    let licence = found.licence.clone();
    let rel = package::copy_into(project, &p, &found, &settings, &licence).unwrap();
    format!("file:{rel}")
}

#[test]
fn an_import_still_loading_is_waited_for_and_drawn() {
    let _g = one();
    let folder = tempfile::tempdir().unwrap();
    let id = imported(folder.path());
    let (p, ids) = song(
        |s| {
            s.set_text(dancer::CHARACTER_KEY, Some(id.clone())).unwrap();
        },
        1,
        false,
    );
    let mut a = app_with(p, folder.path());
    let d = small_mov(&a);
    vx::start(&mut a, &folder.path().join("waits"), &d, false).unwrap();
    assert_eq!(
        a.video.progress().map(|p| p.0),
        Some("video.phase.characters"),
        "the first ask finds it loading: the export waits"
    );
    assert!(a.export_in_progress());
    vx::wait(&mut a);
    assert!(matches!(a.video.last, Some(Ok(_))), "{:?}", a.video.last);
    match job::resolve_dancers(&mut a) {
        job::Cast::Ready(c) => {
            assert_eq!(c.len(), 1);
            assert_eq!(c[0].track, ids[0]);
            assert_eq!(
                c[0].character.info().id,
                id,
                "the imported character, not the default"
            );
        }
        _ => panic!("resolved"),
    }
    assert!(folder.path().join("waits.mov").is_file());
}

#[test]
fn a_character_that_cannot_be_read_stops_the_export_in_words() {
    let _g = one();
    let folder = tempfile::tempdir().unwrap();
    let key = "file:Characters/gone/gone.json".to_string();
    let (p, _) = song(
        |s| {
            s.set_text(dancer::CHARACTER_KEY, Some(key.clone()))
                .unwrap();
        },
        1,
        false,
    );
    let mut a = app_with(p, folder.path());
    let d = small_mov(&a);
    let r = vx::start(&mut a, &folder.path().join("gone"), &d, false);
    vx::wait(&mut a);
    let said = match (r, &a.video.last) {
        (Err(e), _) => e,
        (Ok(()), Some(Err(e))) => e.clone(),
        other => panic!("{other:?}"),
    };
    assert!(said.contains("gone.json"), "{said}");
    assert!(!folder.path().join("gone.mov").exists());
}

#[test]
fn a_hidden_dancer_is_left_out() {
    let _g = one();
    let folder = tempfile::tempdir().unwrap();
    let (p, ids) = song(
        |s| {
            s.set("view.mode", 2.0);
        },
        2,
        false,
    );
    let mut a = app_with(p, folder.path());
    let shown: Vec<TrackId> = job::dancers(&mut a).iter().map(|d| d.track).collect();
    assert_eq!(shown, vec![ids[1]], "only the one that shows");
    match job::resolve_dancers(&mut a) {
        job::Cast::Ready(c) => assert_eq!(c.len(), 1),
        _ => panic!("built-in characters are ready"),
    }
}

#[test]
fn the_preview_is_the_moments_own_render_with_its_automation() {
    let _g = one();
    let folder = tempfile::tempdir().unwrap();
    let (p, ids) = song(|_| {}, 1, true);
    let mut a = app_with(p, folder.path());
    vx::open(&mut a);
    let d = a.video.dialog.clone().unwrap();
    a.video.scrub = 5.0; // bar 3: the automation claps there
    let t0 = std::time::Instant::now();
    let tl = loop {
        match vx::preview_timelines(&mut a, &d) {
            vx::PreviewState::Ready(t) => break t,
            vx::PreviewState::Failed(e) => panic!("{e}"),
            vx::PreviewState::Rendering => {}
        }
        assert!(t0.elapsed().as_secs() < 30, "rendered in time");
        std::thread::sleep(std::time::Duration::from_millis(20));
    };
    let (_, frames) = tl
        .iter()
        .find(|(t, _)| *t == ids[0])
        .expect("the dancer's timeline");
    let f = cantus_video::dance::frame_at(frames, 5.0);
    let clap = dancer::move_value(Move::Clap);
    assert!(
        (f.value("move").unwrap() - clap).abs() < 0.5,
        "{:?}",
        f.value("move")
    );
}

#[test]
fn export_video_is_a_palette_action_that_opens_the_dialog() {
    let _g = one();
    let folder = tempfile::tempdir().unwrap();
    let (p, _) = song(|_| {}, 1, false);
    let mut a = app_with(p, folder.path());
    let action = cantus_ui::keymap::Action::from_id("export-video").expect("listed");
    assert_ne!(
        i18n::t(&action.label_key()),
        action.label_key(),
        "it has words"
    );
    a.run_action(action);
    assert!(a.video.dialog.is_some());
}

/// One frame of the dialog, timed.
fn timed_frame(ctx: &egui::Context, a: &mut CantusApp) -> std::time::Duration {
    let input = egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(1400.0, 900.0),
        )),
        ..Default::default()
    };
    let t = std::time::Instant::now();
    let mut out = ctx.run_ui(input, |ui| vx::frame(a, ui.ctx()));
    out.textures_delta.clear();
    t.elapsed()
}

fn ctx() -> egui::Context {
    let ctx = egui::Context::default();
    cantus_ui::design::install_fonts(&ctx);
    cantus_ui::design::install_style(&ctx);
    ctx
}

/// The preview's render is compiled and rendered on its worker: the dialog's frames stay
/// quick while it runs, and its answer arrives (the review of the fix rounds #1).
#[test]
fn the_preview_never_compiles_or_renders_on_the_interfaces_thread() {
    use std::sync::atomic::Ordering;
    let _g = one();
    let folder = tempfile::tempdir().unwrap();
    let (p, _) = song(|_| {}, 1, true);
    let mut a = app_with(p, folder.path());
    vx::open(&mut a);
    let ctx = ctx();
    let first = timed_frame(&ctx, &mut a);
    assert!(first.as_secs_f64() < 2.0, "the first frame took {first:?}");
    let d = a.video.dialog.clone().unwrap();
    let t0 = std::time::Instant::now();
    loop {
        let f = timed_frame(&ctx, &mut a);
        assert!(f.as_secs_f64() < 1.0, "a frame took {f:?}");
        if let vx::PreviewState::Ready(_) = vx::preview_timelines(&mut a, &d) {
            break;
        }
        assert!(t0.elapsed().as_secs() < 30, "rendered in time");
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    assert_eq!(*job::PREVIEW_THREAD.lock().unwrap(), "cantus-video-preview");
    let _ = job::PREVIEW_LIVE.load(Ordering::SeqCst);
}

/// Scrubbing keeps one preview render at a time; closing the dialog stops it.
#[test]
fn scrubbing_keeps_one_render_and_closing_cancels_it() {
    use std::sync::atomic::Ordering;
    let _g = one();
    let folder = tempfile::tempdir().unwrap();
    let (p, _) = song(|_| {}, 1, true);
    let mut a = app_with(p, folder.path());
    vx::open(&mut a);
    let d = a.video.dialog.clone().unwrap();
    job::PREVIEW_MOST.store(0, Ordering::SeqCst);
    for i in 0..30 {
        a.video.scrub = 0.25 * f64::from(i % 28);
        let _ = vx::preview_timelines(&mut a, &d);
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    assert!(
        job::PREVIEW_MOST.load(Ordering::SeqCst) <= 1,
        "one at a time"
    );
    // Let the last one end, then start one afresh and close the dialog at once.
    let wait_idle = || {
        let t0 = std::time::Instant::now();
        while job::PREVIEW_LIVE.load(Ordering::SeqCst) > 0 {
            assert!(t0.elapsed().as_secs() < 10, "the worker ends");
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
    };
    wait_idle();
    a.video.real = None;
    let cancelled = job::PREVIEW_CANCELLED.load(Ordering::SeqCst);
    a.video.scrub = 3.0;
    assert_eq!(
        vx::preview_timelines(&mut a, &d),
        vx::PreviewState::Rendering
    );
    vx::close(&mut a);
    assert_eq!(
        job::PREVIEW_CANCELLED.load(Ordering::SeqCst),
        cancelled + 1,
        "closing stops what runs"
    );
    wait_idle();
    assert!(a.video.real.is_none());
}

/// An export waiting for its characters belongs to its project: New (or Open) meanwhile
/// drops it, said, and nothing is written (the review of the fix rounds #5).
#[test]
fn a_waiting_export_is_dropped_when_the_project_changes() {
    let _g = one();
    let folder = tempfile::tempdir().unwrap();
    let id = imported(folder.path());
    let (p, _) = song(
        |s| {
            s.set_text(dancer::CHARACTER_KEY, Some(id.clone())).unwrap();
        },
        1,
        false,
    );
    let mut a = app_with(p, folder.path());
    let d = small_mov(&a);
    vx::start(&mut a, &folder.path().join("old"), &d, false).unwrap();
    assert_eq!(
        a.video.progress().map(|p| p.0),
        Some("video.phase.characters")
    );
    a.new_project();
    vx::wait(&mut a);
    assert!(
        !folder.path().join("old.mov").exists(),
        "not made under the old name"
    );
    assert_eq!(a.status.text, i18n::t("video.status.dropped"));
}
