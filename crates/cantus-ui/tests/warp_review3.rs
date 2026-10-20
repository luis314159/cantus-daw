//! Warping from the interface under what round 29's third review found (ADR 0061 §12): a
//! linked pair under one tempo stays linked through *Follow* off and on, an analysis landing
//! during another gesture waits for it, a bounce written under another tempo is stale, a
//! reversed clip is quantised and drawn where it is heard, halves with an inert warp glue,
//! a first edit leaves a ramped, trimmed or reversed window alone, the late-render warning
//! survives every job's own messages, a background import lands only when it is safe, and
//! the words say what is so.

mod support;

use cantus_core::tempo::TempoPoint;
use cantus_core::warp::{Warp, WarpOp};
use cantus_core::{audio_time, Clip, ClipId, Command, Project, Ticks, TrackId};
use cantus_dsp::beat::testkit::{click_track, drum_loop};
use cantus_ui::CantusApp;
use std::path::Path;
use std::time::{Duration, Instant};
use support::app;

fn wav(path: &Path, rate: u32, x: &[f32]) {
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: rate,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut w = hound::WavWriter::create(path, spec).unwrap();
    for v in x {
        w.write_sample((v.clamp(-1.0, 1.0) * 32767.0).round() as i16)
            .unwrap();
    }
    w.finalize().unwrap();
}

fn project(bpm: f64) -> Project {
    let mut p = Project::empty("Warp");
    p.tempo_bpm = bpm;
    let t = p.make_track("Keys");
    p.tracks.push(t);
    p
}

fn audio_clips(a: &CantusApp) -> Vec<(TrackId, Clip)> {
    a.project()
        .tracks
        .iter()
        .flat_map(|t| t.clips.iter().map(move |c| (t.id, c.clone())))
        .filter(|(_, c)| c.audio.is_some())
        .collect()
}

fn settle(a: &mut CantusApp) {
    let t = Instant::now();
    while a.poll_warp() && t.elapsed() < Duration::from_secs(60) {
        std::thread::sleep(Duration::from_millis(10));
    }
}

fn analysed(a: &CantusApp, r: (TrackId, ClipId)) {
    let t = Instant::now();
    while a.warp_analysis(r).is_none() && t.elapsed() < Duration::from_secs(60) {
        std::thread::sleep(Duration::from_millis(10));
    }
    assert!(a.warp_analysis(r).is_some(), "the analysis never came");
}

/// A loop file (`bars` bars of drums at `bpm`, exactly) in `dir`.
fn loop_file(dir: &Path, name: &str, bpm: f64, bars: usize, seed: u64) -> std::path::PathBuf {
    let path = dir.join(name);
    let mut m = drum_loop(44_100.0, bpm, bars, 0.0, 0.0, seed);
    m.audio
        .truncate((60.0 / bpm * (4 * bars) as f64 * 44_100.0).round() as usize);
    wav(&path, 44_100, &m.audio);
    path
}

fn clip(a: &CantusApp, r: (TrackId, ClipId)) -> Clip {
    a.project().track(r.0).unwrap().clip(r.1).unwrap().clone()
}

/// The source frame clip `r` plays at timeline tick `t` (forwards).
fn heard(a: &CantusApp, r: (TrackId, ClipId), t: Ticks) -> f64 {
    let c = clip(a, r);
    let region = c.audio.as_ref().unwrap();
    let rate = a.project().audio_source(region.source).unwrap().sample_rate;
    audio_time::clip_frame_at(
        &a.project().tempo_map(),
        c.start,
        c.offset,
        region,
        rate as f64,
        t.0 as f64,
    )
}

fn point(tick: Ticks, bpm: f64, ramp: bool) -> TempoPoint {
    TempoPoint { tick, bpm, ramp }
}

/// Two linked windows of one file: the clip at `a_at`, its copy at `b_at`.
fn linked(a: &mut CantusApp, a_at: Ticks, b_at: Ticks) -> ((TrackId, ClipId), (TrackId, ClipId)) {
    let (t, c) = audio_clips(a).remove(0);
    a.exec(Command::SetClipBounds {
        track: t,
        clip: c.id,
        start: a_at,
        length: Ticks::beats(8),
    });
    let c = clip(a, (t, c.id));
    let link = cantus_core::LinkId(a.bus.alloc_id());
    let mut copy = c.clone();
    copy.id = ClipId(a.bus.alloc_id());
    copy.start = b_at;
    copy.link = Some(link);
    a.exec(Command::SetClipLink {
        track: t,
        clip: c.id,
        link: Some(link),
    });
    a.exec(Command::InsertClip {
        track: t,
        clip: Box::new(copy.clone()),
    });
    ((t, c.id), (t, copy.id))
}

/// Finding 1: a linked loop pair under one tempo, *Follow* off then on: both still linked
/// and following, with the markers they had.
#[test]
fn follow_off_then_on_keeps_a_linked_pair_linked_under_one_tempo() {
    let dir = tempfile::tempdir().unwrap();
    let path = loop_file(dir.path(), "drums 97.wav", 97.0, 4, 11);
    let mut a = app(project(120.0));
    a.import_audio(&[path], Ticks::ZERO, None);
    settle(&mut a);
    let (ra, rb) = linked(&mut a, Ticks::ZERO, Ticks::beats(32));
    let w0 = a.clip_warp(ra).expect("warped by itself").clone();
    assert!(w0.follow && w0.markers.len() >= 4);
    assert!(a.set_warp_follow(ra, false));
    assert!(a.set_warp_follow(ra, true));
    for r in [ra, rb] {
        assert!(clip(&a, r).link.is_some(), "{r:?} unlinked");
        let w = a.clip_warp(r).unwrap();
        assert!(w.follow, "{r:?} not following");
        assert_eq!(w.markers, w0.markers, "{r:?}: other markers");
    }
}

/// Finding 2: an import's analysis landing while another gesture is open (a fader drag)
/// waits for it: one undo takes back the whole drag, the next the warp.
#[test]
fn an_analysis_landing_mid_drag_waits_for_it() {
    let dir = tempfile::tempdir().unwrap();
    let path = loop_file(dir.path(), "drums 97.wav", 97.0, 4, 5);
    let mut a = app(project(120.0));
    let gain0 = a.project().master.gain_db;
    a.import_audio(&[path], Ticks::ZERO, None);
    let (t, c) = audio_clips(&a).remove(0);
    let r = (t, c.id);
    let key = cantus_core::MergeKey::of(&("test-drag", 2));
    a.exec_merging(Command::SetMasterGain { gain_db: -3.0 }, key);
    analysed(&a, r);
    for _ in 0..20 {
        a.poll_warp();
        std::thread::sleep(Duration::from_millis(5));
    }
    assert!(a.clip_warp(r).is_none(), "landed in the middle of the drag");
    a.exec_merging(Command::SetMasterGain { gain_db: -6.0 }, key);
    a.end_gesture();
    settle(&mut a);
    assert!(a.clip_warp(r).is_some_and(|w| w.follow), "never landed");
    a.undo();
    assert!(a.clip_warp(r).is_none(), "the warp first");
    assert_eq!(a.project().master.gain_db, -6.0, "the drag whole");
    a.undo();
    assert_eq!(
        a.project().master.gain_db,
        gain0,
        "one undo takes the whole drag back"
    );
}

/// Finding 3: a bounce written while the tempo is changed lands nowhere: the clip, a
/// following one left as it was, would play a render laid for the old tempo.
#[test]
fn a_bounce_written_under_another_tempo_is_stale() {
    let dir = tempfile::tempdir().unwrap();
    let path = loop_file(dir.path(), "drums 97.wav", 97.0, 2, 3);
    let mut a = app(project(120.0));
    a.import_audio(&[path], Ticks::ZERO, None);
    settle(&mut a);
    let (t, c) = audio_clips(&a).remove(0);
    let r = (t, c.id);
    assert!(a.clip_warp(r).is_some_and(|w| w.follow));
    let before = clip(&a, r);
    assert!(a.bounce_warped(r));
    a.exec(Command::SetTempo { bpm: 100.0 });
    let t0 = Instant::now();
    while a.warp.bounce.is_some() && t0.elapsed() < Duration::from_secs(60) {
        a.poll_warp();
        std::thread::sleep(Duration::from_millis(10));
    }
    assert!(a.warp.bounce.is_none(), "the bounce never finished");
    let now = clip(&a, r);
    assert_eq!(now, before, "landed on a clip laid for another tempo");
    assert!(a.clip_warp(r).is_some_and(|w| w.follow));
}

/// Finding 5: halves of an unwarped clip, one left with a warp that changes nothing (a mode
/// picked, a transposition dragged back to zero), glue.
#[test]
fn a_half_with_a_warp_that_changes_nothing_glues() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("clicks.wav");
    wav(&path, 48_000, &click_track(48_000.0, 100.0, 16, 0.25).audio);
    let mut a = app(project(120.0));
    a.import_audio(std::slice::from_ref(&path), Ticks::beats(12), None);
    settle(&mut a);
    let (t, c) = audio_clips(&a).remove(0);
    let cut = Ticks::beats(18);
    let mut right = c.clone();
    right.id = ClipId(a.bus.alloc_id());
    right.start = cut;
    right.length = c.end() - cut;
    right.offset = c.offset + (cut - c.start);
    a.exec(Command::Batch {
        label_key: "test.split".into(),
        commands: vec![
            Command::SetClipBounds {
                track: t,
                clip: c.id,
                start: c.start,
                length: cut - c.start,
            },
            Command::InsertClip {
                track: t,
                clip: Box::new(right.clone()),
            },
        ],
    });
    // A transposition dragged to +2 and back in one gesture: the warp stays, inert.
    assert!(a.set_warp_cents((t, right.id), 200, true));
    assert!(a.set_warp_cents((t, right.id), 0, true));
    a.end_gesture();
    let inert = a.clip_warp((t, right.id)).cloned();
    assert!(inert.is_some_and(|w| !w.follow && w.cents == 0));
    let l = clip(&a, (t, c.id));
    let rr = clip(&a, (t, right.id));
    assert!(l.audio.as_ref().unwrap().warp.is_none());
    let glued = cantus_ui::audio_clips::glue_in(a.project(), t, &[&l, &rr]);
    assert!(glued.is_some(), "the halves glue");
}

/// Finding 4: *Quantise audio* on a reversed following clip across a tempo step pulls each
/// hit to the line it is heard on as the engine plays it — mirrored through the map.
#[test]
fn quantise_hears_a_reversed_clip_across_a_tempo_step() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("clicks 120.wav");
    wav(&path, 48_000, &click_track(48_000.0, 120.0, 2, 0.1).audio);
    let q = cantus_ui::warp::QuantiseSettings {
        grid: Ticks(240),
        strength: 1.0,
        threshold: 0.1,
    };
    let mut p = project(120.0);
    p.tempo_points = vec![
        point(Ticks::ZERO, 120.0, false),
        point(Ticks::beats(4), 90.0, false),
    ];
    let mut a = app(p);
    a.import_audio(std::slice::from_ref(&path), Ticks::ZERO, None);
    settle(&mut a);
    let (t, c) = audio_clips(&a).remove(0);
    let r = (t, c.id);
    let mut region = c.audio.clone().unwrap();
    region.reverse = true;
    a.exec(Command::SetClipAudio {
        track: t,
        clip: r.1,
        audio: Some(region),
    });
    a.exec(Command::SetClipBounds {
        track: t,
        clip: r.1,
        start: Ticks::ZERO,
        length: Ticks::beats(8),
    });
    analysed(&a, r);
    assert!(a.set_warp_follow(r, true));
    assert_eq!(a.quantise_audio(&[r], q), 1);
    let c = clip(&a, r);
    let w = a.clip_warp(r).unwrap().clone();
    let map = a.project().tempo_map();
    let (s, e) = (c.start.0 as f64, c.end().0 as f64);
    let origin = (c.start - c.offset).0 as f64;
    let mut checked = 0;
    for f in a.warp_analysis(r).unwrap().onsets.clone() {
        let forward = origin + w.tick_at(f);
        if forward <= s + 240.0 || forward >= e - 240.0 {
            continue;
        }
        // Heard as the engine plays it: the render of the window read backwards.
        let heard = map.tick_after_seconds(s, map.seconds_between_f64(forward, e));
        let off = heard - (heard / 240.0).round() * 240.0;
        assert!(off.abs() < 1.0, "a hit heard {off:+} ticks off its line");
        checked += 1;
    }
    assert!(checked >= 4, "{checked}");
}

/// A `.dawproject` of a one-track project named `name`, carrying a small audio file.
fn package(dir: &Path, name: &str) -> std::path::PathBuf {
    let mut p = Project::new(name);
    let mut t = p.make_track("Audio");
    let take = dir.join(format!("{name} take.wav"));
    wav(&take, 48_000, &click_track(48_000.0, 120.0, 2, 0.1).audio);
    let id = cantus_core::AudioSourceId(p.alloc_id());
    p.audio_pool.push(cantus_core::AudioSource {
        id,
        name: "take".into(),
        path: take.to_string_lossy().into_owned(),
        sample_rate: 48_000,
        channels: 1,
        frames: 48_000 * 4,
        sha256: None,
        recorded: false,
    });
    t.clips
        .push(p.make_audio_clip("take", id, Ticks::ZERO, Ticks::beats(4)));
    p.tracks.push(t);
    let file = dir.join(format!("{name}.dawproject"));
    cantus_project::dawproject::export_to(&p, "test", None, &file).unwrap();
    file
}

/// Poll an import until its reading is done (it may still wait to land).
fn read_through(a: &mut CantusApp, before: &str) {
    let t = Instant::now();
    while t.elapsed() < Duration::from_secs(5) && a.project().name == before {
        cantus_ui::views::interop::poll_import(a, None);
        std::thread::sleep(Duration::from_millis(10));
    }
}

/// Finding 22: a `.dawproject` read in the background lands only when it is safe — never
/// during a pass (the take stays in the project it was recorded into), never over a
/// project opened since (said, its audio let go), and a quit stops it, its audio let go.
#[test]
fn a_background_import_lands_only_when_it_is_safe() {
    let dir = tempfile::tempdir().unwrap();
    // During a pass: it waits for the pass to end.
    let file = package(dir.path(), "Imported one");
    let landed = cantus_ui::warp::media_dir("Imported one.dawproject");
    let mut a = app(cantus_ui::starter_project());
    let first = a.project().name.clone();
    a.start_recording();
    assert!(cantus_ui::views::interop::import_dawproject_from(
        &mut a, &file
    ));
    read_through(&mut a, &first);
    assert_eq!(a.project().name, first, "replaced mid-recording");
    assert!(a.is_recording());
    a.stop_recording();
    read_through(&mut a, &first);
    assert_eq!(
        a.project().name,
        "Imported one",
        "landed once the pass ended"
    );
    // Another project opened since: never replaced, said.
    let file = package(dir.path(), "Imported two");
    let mut a = app(cantus_ui::starter_project());
    let media = cantus_ui::warp::media_dir("Imported two.dawproject");
    assert!(cantus_ui::views::interop::import_dawproject_from(
        &mut a, &file
    ));
    a.replace_project(Project::new("Opened since"));
    let t = Instant::now();
    while a.interop.importing.is_some() && t.elapsed() < Duration::from_secs(5) {
        cantus_ui::views::interop::poll_import(&mut a, None);
        std::thread::sleep(Duration::from_millis(10));
    }
    assert_eq!(a.project().name, "Opened since");
    assert!(a.status.error, "said");
    assert!(!media.exists(), "its copied audio let go");
    // A quit: stopped, its audio let go.
    let file = package(dir.path(), "Imported three");
    let mut a = app(cantus_ui::starter_project());
    let media = cantus_ui::warp::media_dir("Imported three.dawproject");
    assert!(cantus_ui::views::interop::import_dawproject_from(
        &mut a, &file
    ));
    cantus_ui::views::interop::stop_import(&mut a);
    assert!(a.interop.importing.is_none());
    assert!(!media.exists(), "its copied audio let go");
    let _ = std::fs::remove_dir_all(landed);
}

/// Finding 45: an import's own errors reach the interface in its language; a damaged ZIP64
/// record is "damaged", never "not supported".
#[test]
fn an_import_error_is_said_in_the_interfaces_language() {
    use cantus_project::dawproject::{zip::ZipError, DawProjectError};
    cantus_ui::i18n::default_thread_locale(cantus_ui::i18n::Locale::Es);
    let said = cantus_ui::views::interop::import_error(&DawProjectError::Zip(ZipError::TooLarge(
        "project.xml".into(),
    )));
    assert_eq!(said, "project.xml es demasiado grande para leerlo");
    let said = cantus_ui::views::interop::import_error(&DawProjectError::Zip(ZipError::Damaged));
    assert_eq!(said, "el archivo está dañado o incompleto");
    cantus_ui::i18n::default_thread_locale(cantus_ui::i18n::Locale::En);
}

/// Findings 35 and 39: an export compiled while a warped clip's render was on its way says
/// so when it lands — through the export's own entry point, after its own words.
#[test]
fn a_late_render_is_said_when_an_export_lands() {
    use cantus_ui::views::export;
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("clicks.wav");
    wav(&path, 48_000, &click_track(48_000.0, 100.0, 4, 0.25).audio);
    let mut a = app(project(120.0));
    a.import_audio(std::slice::from_ref(&path), Ticks::ZERO, None);
    settle(&mut a);
    let (t, c) = audio_clips(&a).remove(0);
    a.engine.samples.warp().hold_finals(true);
    a.engine.set_render_patience(Duration::from_millis(20));
    assert!(a.set_warp_cents((t, c.id), 300, false));
    export::open(&mut a, None);
    let d = a.export.dialog.as_mut().unwrap();
    d.scope = cantus_export::Scope::Song;
    d.loudness.target_lufs = None;
    d.tail_seconds = 0.0;
    assert!(export::start(&mut a, &dir.path().join("out.wav")));
    export::wait(&mut a);
    a.engine.samples.warp().hold_finals(false);
    let late = cantus_ui::i18n::t("warp.status.late");
    assert!(a.status.error, "said as a warning: {}", a.status.text);
    assert!(a.status.text.ends_with(&late), "{}", a.status.text);
    let last = a.export.last.clone().unwrap().unwrap();
    assert!(last.ends_with(&late), "the dialog's last word too: {last}");
}

/// Finding 39: the note goes with a job's own message — a freeze's or a merge's "done" —
/// as a warning, never replaced by it.
#[test]
fn a_jobs_done_message_carries_the_late_note() {
    let mut a = app(project(120.0));
    a.say_done("Frozen".into(), true);
    assert!(a.status.error);
    assert_eq!(
        a.status.text,
        format!("Frozen {}", cantus_ui::i18n::t("warp.status.late"))
    );
    a.say_done("Frozen".into(), false);
    assert!(!a.status.error);
    assert_eq!(a.status.text, "Frozen");
}

/// Finding 44: a clip whose analysis found no steady tempo (a one-shot) is said to have
/// none — never "still being analysed" — and the project tempo cannot be taken from it.
#[test]
fn a_clip_with_no_steady_tempo_says_so() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("one shot.wav");
    let mut x = vec![0.0f32; 48_000];
    for (i, v) in x.iter_mut().enumerate().skip(100).take(2_000) {
        *v = 0.8 * (1.0 - (i - 100) as f32 / 2_000.0) * if i % 2 == 0 { 1.0 } else { -1.0 };
    }
    wav(&path, 48_000, &x);
    let mut a = app(project(120.0));
    a.import_audio(std::slice::from_ref(&path), Ticks::ZERO, None);
    settle(&mut a);
    let (t, c) = audio_clips(&a).remove(0);
    let r = (t, c.id);
    analysed(&a, r);
    assert_eq!(
        a.warp_analysis(r).unwrap().bpm(),
        0.0,
        "no tempo in a one-shot"
    );
    assert!(!a.can_take_tempo_from(r), "offered with nothing to take");
    assert!(!a.project_tempo_from_clip(r));
    assert_eq!(a.status.text, cantus_ui::i18n::t("warp.status.no_tempo"));
}

/// One frame of the warp editor on clip `r`, `size` points, with `events`.
fn editor(
    a: &mut CantusApp,
    ctx: &egui::Context,
    r: (TrackId, ClipId),
    size: egui::Vec2,
    events: Vec<egui::Event>,
) {
    let input = egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, size)),
        events,
        ..Default::default()
    };
    ctx.run_ui(input, |ui| {
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE)
            .show(ui, |ui| cantus_ui::views::warp::show(a, ui, r.0, r.1));
    })
    .textures_delta
    .clear();
}

/// Finding 43: in a pane narrower than the bar's widest item (the Mode chips), the bar
/// scrolls sideways: every chip is reached.
#[test]
fn the_warp_bar_scrolls_in_a_narrow_pane() {
    use cantus_ui::views::warp::BarItem;
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("clicks.wav");
    wav(&path, 48_000, &click_track(48_000.0, 100.0, 4, 0.25).audio);
    let mut a = app(project(120.0));
    a.import_audio(std::slice::from_ref(&path), Ticks::ZERO, None);
    settle(&mut a);
    let (t, c) = audio_clips(&a).remove(0);
    let r = (t, c.id);
    a.open_warp_editor(r);
    let ctx = support::ctx();
    let size = egui::vec2(300.0, 500.0);
    for _ in 0..2 {
        editor(&mut a, &ctx, r, size, vec![]);
    }
    let mode = |a: &CantusApp| {
        a.warp
            .view
            .bar_rects
            .iter()
            .find(|(i, _)| *i == BarItem::Mode)
            .unwrap()
            .1
    };
    let before = mode(&a);
    assert!(before.right() > size.x, "wider than the pane: {before:?}");
    // Scrolled sideways over the bar: its far end comes into the pane.
    for _ in 0..20 {
        editor(
            &mut a,
            &ctx,
            r,
            size,
            vec![
                egui::Event::PointerMoved(egui::pos2(150.0, before.center().y)),
                egui::Event::MouseWheel {
                    unit: egui::MouseWheelUnit::Point,
                    delta: egui::vec2(-60.0, 0.0),
                    phase: egui::TouchPhase::Move,
                    modifiers: egui::Modifiers::NONE,
                },
            ],
        );
    }
    let after = mode(&a);
    assert!(after.right() <= size.x + 0.5, "never reached: {after:?}");
}

/// Finding 38: a first warping edit — a marker added, *Follow* pressed — on a clip that
/// straddles a 60→180 BPM ramp and a step, trimmed 3 beats and 13 ticks in, forwards and
/// reversed, leaves what plays alone: every place inside the window within half a frame.
#[test]
fn a_first_edit_on_a_ramped_trimmed_or_reversed_clip_leaves_what_plays_alone() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("long.wav");
    wav(&path, 48_000, &click_track(48_000.0, 100.0, 12, 0.25).audio);
    for reverse in [false, true] {
        for edit in 0..2 {
            let mut p = project(120.0);
            p.tempo_points = vec![
                point(Ticks::ZERO, 120.0, false),
                point(Ticks::beats(4), 60.0, true),
                point(Ticks::beats(12), 180.0, false),
                point(Ticks::beats(16), 100.0, false),
            ];
            let mut a = app(p);
            a.import_audio(std::slice::from_ref(&path), Ticks::beats(2), None);
            settle(&mut a);
            let (t, c) = audio_clips(&a).remove(0);
            let r = (t, c.id);
            assert!(a.clip_warp(r).is_none(), "not warped by itself");
            let trim = Ticks::beats(3) + Ticks(13);
            a.exec(Command::SetClipBounds {
                track: t,
                clip: r.1,
                start: c.start + trim,
                length: Ticks::beats(18),
            });
            a.exec(Command::SetClipOffset {
                track: t,
                clip: r.1,
                offset: c.offset + trim,
            });
            if reverse {
                let mut region = clip(&a, r).audio.unwrap();
                region.reverse = true;
                a.exec(Command::SetClipAudio {
                    track: t,
                    clip: r.1,
                    audio: Some(region),
                });
            }
            let c = clip(&a, r);
            let at: Vec<Ticks> = (1..=9)
                .map(|k| c.start + Ticks(c.length.0 * k / 10 + 9))
                .collect();
            let before: Vec<f64> = at.iter().map(|t| heard(&a, r, *t)).collect();
            if edit == 0 {
                assert!(a.add_warp_marker(r, Ticks::beats(5), None));
                // On the content tick asked (the clip's content ticks as they are now).
                let at = Ticks::beats(5);
                let w = a.clip_warp(r).unwrap();
                assert!(
                    w.markers.iter().any(|m| !m.auto && m.tick == at),
                    "{:?}",
                    w.markers.iter().filter(|m| !m.auto).collect::<Vec<_>>()
                );
            } else {
                assert!(a.set_warp_follow(r, true));
            }
            assert!(a.clip_warp(r).is_some_and(|w| w.follow));
            for (t, f) in at.iter().zip(&before) {
                let d = heard(&a, r, *t) - f;
                assert!(
                    d.abs() <= 0.5,
                    "reversed {reverse}, edit {edit}: {d:+} frames at tick {}",
                    t.0
                );
            }
        }
    }
}
