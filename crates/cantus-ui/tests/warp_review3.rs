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
