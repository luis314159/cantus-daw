//! Warping from the interface under what round 29's review found (ADR 0061): a first edit
//! leaves what plays alone, a re-laid warp keeps the window on its audio, the import rule's
//! bars and undo step, recordings never warped by themselves, the tempo in force, Del and Q
//! where they belong, the band only on a warped clip, Quantise over several clips, a linked
//! copy stretched on its own, and imported media never written over.

mod support;

use cantus_core::tempo::TempoPoint;
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

/// The source frame clip `r` plays at timeline tick `t`.
fn heard(a: &CantusApp, r: (TrackId, ClipId), t: Ticks) -> f64 {
    let c = a.project().track(r.0).unwrap().clip(r.1).unwrap();
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

fn two_tempos(p: &mut Project) {
    p.tempo_points = vec![
        TempoPoint {
            tick: Ticks::ZERO,
            bpm: 120.0,
            ramp: false,
        },
        TempoPoint {
            tick: Ticks::beats(16),
            bpm: 90.0,
            ramp: false,
        },
    ];
}

/// Under a tempo track, a clip in the slower section: a transposition leaves it at its
/// natural speed (not following); a first marker pins exactly what it plays.
#[test]
fn a_first_warp_edit_leaves_what_plays_alone() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("clicks.wav");
    let m = click_track(48_000.0, 100.0, 8, 0.25);
    wav(&path, 48_000, &m.audio);
    let mut p = project(120.0);
    two_tempos(&mut p);
    let mut a = app(p);
    a.import_audio(std::slice::from_ref(&path), Ticks::beats(32), None);
    settle(&mut a);
    let (t, c) = audio_clips(&a).remove(0);
    let r = (t, c.id);
    let at = [
        c.start,
        c.start + Ticks::beats(3),
        c.start + Ticks::beats(7),
    ];
    let before: Vec<f64> = at.iter().map(|t| heard(&a, r, *t)).collect();
    assert!(a.set_warp_cents(r, -300, false));
    let w = a.clip_warp(r).unwrap();
    assert!(!w.follow, "a transposition does not warp the clip");
    for (t, f) in at.iter().zip(&before) {
        assert!((heard(&a, r, *t) - f).abs() < 1e-6, "{t:?}");
    }
    a.undo();
    assert!(a.add_warp_marker(r, Ticks::beats(2), None));
    assert!(a.clip_warp(r).unwrap().follow);
    for (t, f) in at.iter().zip(&before) {
        let d = heard(&a, r, *t) - f;
        assert!(d.abs() < 1.0, "a first marker moved {t:?} by {d:+} frames");
    }
}

/// *Detect tempo* on a window trimmed from the left: it still starts on the audio it did.
#[test]
fn detect_tempo_keeps_a_trimmed_window_on_its_audio() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("clicks 100.wav");
    let m = click_track(48_000.0, 100.0, 8, 0.25);
    wav(&path, 48_000, &m.audio);
    let mut a = app(project(120.0));
    a.import_audio(std::slice::from_ref(&path), Ticks::ZERO, None);
    settle(&mut a);
    let (t, c) = audio_clips(&a).remove(0);
    let r = (t, c.id);
    assert!(a.clip_warp(r).is_none(), "not a loop: not warped by itself");
    a.exec(Command::SetClipOffset {
        track: t,
        clip: r.1,
        offset: Ticks::beats(6),
    });
    analysed(&a, r);
    let start = a.project().track(t).unwrap().clip(r.1).unwrap().start;
    let f0 = heard(&a, r, start);
    assert!(a.warp_by_analysis(r));
    let f1 = heard(&a, r, start);
    assert!(
        (f1 - f0).abs() < 2.0,
        "the window moved {:+} frames",
        f1 - f0
    );
}

/// A 4/4 loop dropped into a 3/4 song keeps its sixteen beats.
#[test]
fn a_loop_in_three_four_keeps_its_four_four_bars() {
    let dir = tempfile::tempdir().unwrap();
    let path = loop_file(dir.path(), "drums 92.wav", 92.0, 4, 3);
    let mut p = project(128.0);
    p.time_signature.numerator = 3;
    let mut a = app(p);
    a.import_audio(&[path], Ticks::ZERO, None);
    settle(&mut a);
    let (_, c) = audio_clips(&a).remove(0);
    assert!(c.audio.as_ref().unwrap().warp.is_some(), "warped by itself");
    assert_eq!(c.length, Ticks::beats(16));
}

/// Two loops dropped at once: both warp themselves, and one undo takes the whole import
/// back.
#[test]
fn a_many_file_import_is_one_undo_step() {
    let dir = tempfile::tempdir().unwrap();
    let a1 = loop_file(dir.path(), "a 92.wav", 92.0, 2, 4);
    let a2 = loop_file(dir.path(), "b 92.wav", 92.0, 2, 5);
    let mut a = app(project(128.0));
    a.import_audio(&[a1, a2], Ticks::ZERO, None);
    settle(&mut a);
    let clips = audio_clips(&a);
    assert_eq!(clips.len(), 2);
    assert!(clips
        .iter()
        .all(|(_, c)| c.audio.as_ref().unwrap().warp.is_some()));
    a.undo();
    assert!(audio_clips(&a).is_empty(), "one step");
}

/// A recovered recording (its track gone, imported again) is a recording: never warped.
#[test]
fn a_recovered_take_is_never_warped_by_itself() {
    let dir = tempfile::tempdir().unwrap();
    let path = loop_file(dir.path(), "take 92.wav", 92.0, 4, 6);
    let mut a = app(project(128.0));
    a.import_audio_as(&[path], Ticks::ZERO, None, true);
    settle(&mut a);
    let (t, c) = audio_clips(&a).remove(0);
    analysed(&a, (t, c.id));
    settle(&mut a);
    let c = a.project().track(t).unwrap().clip(c.id).unwrap().clone();
    let region = c.audio.unwrap();
    assert!(region.warp.is_none());
    assert!(a.project().audio_source(region.source).unwrap().recorded);
}

/// *Set the project tempo from this clip* sets the tempo where the clip is.
#[test]
fn the_project_tempo_is_set_where_the_clip_is() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("clicks.wav");
    wav(&path, 48_000, &click_track(48_000.0, 100.0, 8, 0.25).audio);
    let mut p = project(120.0);
    two_tempos(&mut p);
    let mut a = app(p);
    a.import_audio(std::slice::from_ref(&path), Ticks::beats(32), None);
    settle(&mut a);
    let (t, c) = audio_clips(&a).remove(0);
    let r = (t, c.id);
    assert!(a.set_warp_tempo(r, 100.0, false));
    assert!(a.project_tempo_from_clip(r));
    let pts = &a.project().tempo_points;
    assert_eq!(pts[0].bpm, 120.0, "the song's start is untouched");
    assert!((pts[1].bpm - 100.0).abs() < 0.01, "{}", pts[1].bpm);
}

/// A marker selected earlier does not take Del once other clips are selected.
#[test]
fn a_stale_marker_selection_does_not_take_del() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("clicks.wav");
    wav(&path, 48_000, &click_track(48_000.0, 120.0, 4, 0.0).audio);
    let mut a = app(project(120.0));
    a.import_audio(&[path.clone(), path], Ticks::ZERO, None);
    settle(&mut a);
    let clips = audio_clips(&a);
    let (r1, r2) = ((clips[0].0, clips[0].1.id), (clips[1].0, clips[1].1.id));
    assert!(a.add_warp_marker(r1, Ticks::beats(2), None));
    assert!(a.warp.selected.is_some());
    let markers = a.clip_warp(r1).unwrap().markers.len();
    a.set_clip_selection(vec![r2]);
    a.focus_arrangement();
    a.run_action(cantus_ui::keymap::Action::Delete);
    assert_eq!(
        a.clip_warp(r1).unwrap().markers.len(),
        markers,
        "the marker stays"
    );
    assert!(
        a.project().track(r2.0).unwrap().clip(r2.1).is_none(),
        "the selected clip went"
    );
}

/// In the clip editor's Warp tab, the pitch editor's keys are not live.
#[test]
fn the_warp_tab_takes_the_pitch_editors_keys_away() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("clicks.wav");
    wav(&path, 48_000, &click_track(48_000.0, 120.0, 4, 0.0).audio);
    let mut a = app(project(120.0));
    a.import_audio(&[path], Ticks::ZERO, None);
    settle(&mut a);
    let (t, c) = audio_clips(&a).remove(0);
    a.open_warp_editor((t, c.id));
    assert_eq!(a.focus, cantus_ui::app::EditFocus::PianoRoll);
    assert!(a.pitch_editing().is_none());
}

/// *Warp from here* at a place outside the clip is said, and nothing changes.
#[test]
fn warp_from_here_outside_the_clip_is_said() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("clicks.wav");
    wav(&path, 48_000, &click_track(48_000.0, 100.0, 8, 0.25).audio);
    let mut a = app(project(120.0));
    a.import_audio(std::slice::from_ref(&path), Ticks::beats(8), None);
    settle(&mut a);
    let (t, c) = audio_clips(&a).remove(0);
    let r = (t, c.id);
    analysed(&a, r);
    assert!(!a.warp_from_here(r, Ticks::ZERO));
    assert!(a.clip_warp(r).is_none());
    assert!(a.warp_from_here(r, c.start + Ticks::beats(4)));
}

/// *Quantise audio* over several clips: one undo step for all.
#[test]
fn quantise_audio_over_several_clips_is_one_step() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("clicks.wav");
    let mut m = click_track(48_000.0, 120.0, 4, 0.0);
    // Each click a little late.
    m.audio.rotate_right(600);
    wav(&path, 48_000, &m.audio);
    let mut a = app(project(120.0));
    a.import_audio(&[path.clone(), path], Ticks::ZERO, None);
    settle(&mut a);
    let clips: Vec<(TrackId, ClipId)> = audio_clips(&a).iter().map(|(t, c)| (*t, c.id)).collect();
    for r in &clips {
        analysed(&a, *r);
    }
    let q = a.warp.quantise;
    assert_eq!(a.quantise_audio(&clips, q), 2);
    assert!(clips.iter().all(|r| a.clip_warp(*r).is_some()));
    a.undo();
    assert!(clips.iter().all(|r| a.clip_warp(*r).is_none()), "one step");
}

/// An Alt-stretch of a linked copy stretches that copy alone.
#[test]
fn a_linked_copy_stretches_on_its_own() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("clicks.wav");
    wav(&path, 48_000, &click_track(48_000.0, 120.0, 2, 0.0).audio);
    let mut a = app(project(120.0));
    a.import_audio(&[path], Ticks::ZERO, None);
    settle(&mut a);
    let (t, c) = audio_clips(&a).remove(0);
    // A linked copy beside it.
    let link = cantus_core::LinkId(a.bus.alloc_id());
    let mut copy = c.clone();
    copy.id = ClipId(a.bus.alloc_id());
    copy.start = c.start + c.length;
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
    let alt = egui::Modifiers {
        alt: true,
        ..Default::default()
    };
    cantus_ui::views::arrangement::warp_api::modifiers(&mut a, alt);
    a.commit_clip_drag(
        (t, c.id),
        cantus_ui::views::arrangement::ClipDragResult::TrimEnd { delta: c.length },
    );
    assert!(a.clip_warp((t, c.id)).is_some());
    assert!(a.clip_warp((t, copy.id)).is_none(), "the copy is untouched");
    let anchor = a.project().track(t).unwrap().clip(c.id).unwrap();
    assert!(anchor.link.is_none(), "made unique");
}

/// The band of a clip that is not warped takes no press (a drag there moves the clip);
/// once it follows the tempo, it does.
#[test]
fn the_band_of_an_unwarped_clip_takes_no_press() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("clicks.wav");
    wav(&path, 48_000, &click_track(48_000.0, 120.0, 4, 0.0).audio);
    let mut a = app(project(120.0));
    a.import_audio(&[path], Ticks::ZERO, None);
    settle(&mut a);
    let (t, c) = audio_clips(&a).remove(0);
    let r = (t, c.id);
    let band_hit = |a: &mut CantusApp| {
        let ctx = support::ctx();
        support::paint(&ctx, egui::vec2(1400.0, 700.0), 3, |ui| {
            cantus_ui::views::arrangement::show(a, ui);
        });
        let h = a.arrangement.audio_handles(r).expect("the clip's handles");
        let hits = h.warp.expect("its warp geometry");
        let p = egui::pos2(hits.band.center().x + 3.0, hits.band.center().y);
        hits.at(p)
    };
    assert_eq!(band_hit(&mut a), None);
    assert!(a.set_warp_follow(r, true));
    assert!(matches!(
        band_hit(&mut a),
        Some(cantus_ui::views::arrangement::AudioHandle::WarpBand(_))
    ));
}

/// A bake that traces audio, asked while a warp render is on its way, waits beside the
/// window (it never blocks the frame) and runs once the render is in.
#[test]
fn a_bake_waits_for_renders_on_their_way() {
    use cantus_core::modulator::{ModHost, ModKind};
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("clicks.wav");
    wav(&path, 48_000, &click_track(48_000.0, 100.0, 4, 0.0).audio);
    let mut a = app(cantus_ui::starter_project());
    a.import_audio(std::slice::from_ref(&path), Ticks::ZERO, None);
    settle(&mut a);
    let (t, c) = audio_clips(&a).remove(0);
    let s = a.project().track(t).unwrap().strip;
    let target = cantus_core::AutoTarget::StripPan(s.expect("the starter track has a strip"));
    let id = a
        .add_modulator(ModHost::Track(t), ModKind::Follower, Some(target.clone()))
        .unwrap();
    a.set_time_selection(Ticks::ZERO, Ticks::beats(4));
    // A warp whose render is held back: the engine is loading.
    a.engine.samples.warp().hold_finals(true);
    let mut w = cantus_core::warp::Warp::identity(48_000, 100.0, 0);
    w.cents = -200;
    a.commit_warp((t, c.id), Some(w), cantus_core::warp::WarpOp::Set, false);
    a.sync_engine();
    assert!(a.engine.samples.loading(), "the render is on its way");
    let steps = a.bus.history().len();
    assert!(a.bake_route(id, &target));
    assert!(a.warp.pending_bake.is_some(), "waiting, not blocking");
    assert_eq!(a.bus.history().len(), steps);
    a.engine.samples.warp().hold_finals(false);
    let t0 = Instant::now();
    while a.warp.pending_bake.is_some() && t0.elapsed() < Duration::from_secs(60) {
        a.engine.samples.poll_loaded();
        a.poll_warp();
        std::thread::sleep(Duration::from_millis(10));
    }
    assert!(a.warp.pending_bake.is_none());
}
