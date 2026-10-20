//! Warping from the interface under what round 29's second review found (ADR 0061 §11):
//! *Follow* off and on keeps the markers and every window's audio, a warp that does not
//! follow holds no markers and says its detected tempo, halves transposed apart glue, a
//! reversed following clip is edited by its ticks, linked copies keep their audio (and a
//! copy under another tempo is made unique), *Quantise* hears what is heard, a job never
//! lands on another project, and an import's analysis seals only its own step.

mod support;

use cantus_core::tempo::TempoPoint;
use cantus_core::warp::{Warp, WarpMode, WarpOp};
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

/// The source frame clip `r` plays at timeline tick `t`.
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

/// 120 BPM, and 90 from beat 16.
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

/// A 97 BPM loop warped by itself into a 120 BPM song, trimmed a bar in.
fn warped_loop(dir: &Path) -> (CantusApp, (TrackId, ClipId)) {
    let path = loop_file(dir, "drums 97.wav", 97.0, 4, 11);
    let mut a = app(project(120.0));
    a.import_audio(&[path], Ticks::ZERO, None);
    settle(&mut a);
    let (t, c) = audio_clips(&a).remove(0);
    let r = (t, c.id);
    let w = a.clip_warp(r).expect("warped by itself").clone();
    assert!(w.follow && w.markers.len() >= 4, "{w:?}");
    a.exec(Command::SetClipOffset {
        track: t,
        clip: r.1,
        offset: Ticks::beats(4),
    });
    (a, r)
}

/// Findings 1, 27, 45: *Follow* off then on keeps every marker the warp had (and its
/// tempo) — never natural markers in their place.
#[test]
fn follow_off_then_on_keeps_the_markers() {
    let dir = tempfile::tempdir().unwrap();
    let (mut a, r) = warped_loop(dir.path());
    let w0 = a.clip_warp(r).unwrap().clone();
    assert!(a.set_warp_follow(r, false));
    let off = a.clip_warp(r).unwrap().clone();
    assert!(!off.follow);
    assert_eq!(off.markers, w0.markers, "kept while it does not follow");
    assert!(a.set_warp_follow(r, true));
    let on = a.clip_warp(r).unwrap().clone();
    assert!(on.follow);
    assert_eq!(on.markers, w0.markers);
    assert_eq!(on.bpm, w0.bpm);
}

/// Finding 6: *Follow* off, and *Reset warp*, keep a trimmed warped window on the audio it
/// played at its start; *Follow* on again too (to within a tick, its offset's unit).
#[test]
fn follow_off_and_reset_keep_a_trimmed_window_on_its_audio() {
    let dir = tempfile::tempdir().unwrap();
    let (mut a, r) = warped_loop(dir.path());
    let start = clip(&a, r).start;
    let f0 = heard(&a, r, start);
    assert!(a.set_warp_follow(r, false));
    let d = heard(&a, r, start) - f0;
    assert!(d.abs() < 1.0, "Follow off moved the window {d:+} frames");
    assert!(a.set_warp_follow(r, true));
    let per_tick = a.clip_warp(r).unwrap().tempo();
    let tick = 44_100.0 * 60.0 / (per_tick * 960.0);
    let d = heard(&a, r, start) - f0;
    assert!(d.abs() <= tick, "Follow on again moved it {d:+} frames");
    assert!(a.reset_warp(r));
    let d = heard(&a, r, start) - f0;
    assert!(d.abs() <= tick + 1.0, "Reset moved it {d:+} frames");
}

/// Findings 2, 32, 48: a transposition or a mode on an unwarped clip holds no markers;
/// the clip's tempo is what the analysis found, and *Set the project tempo from this
/// clip* sets that — under any later tempo edit.
#[test]
fn a_warp_that_does_not_follow_holds_no_markers_and_says_its_detected_tempo() {
    let dir = tempfile::tempdir().unwrap();
    let path = loop_file(dir.path(), "take.wav", 120.0, 4, 12);
    let mut a = app(project(92.0));
    // A recording: never warped by itself.
    a.import_audio_as(&[path], Ticks::ZERO, None, true);
    settle(&mut a);
    let (t, c) = audio_clips(&a).remove(0);
    let r = (t, c.id);
    analysed(&a, r);
    assert!(a.set_warp_mode(r, WarpMode::Rhythm));
    assert!(a.set_warp_cents(r, 200, false));
    let w = a.clip_warp(r).unwrap().clone();
    assert!(!w.follow && w.markers.is_empty(), "{w:?}");
    let detected = a.warp_analysis(r).unwrap().bpm();
    assert!((detected - 120.0).abs() < 1.0, "{detected}");
    // A tempo edit later changes nothing it says.
    a.exec(Command::SetTempo { bpm: 90.0 });
    let said = a.clip_source_tempo(r).unwrap();
    assert!((said - detected).abs() < 1e-9, "{said}");
    assert!(a.project_tempo_from_clip(r));
    assert!(
        (a.project().tempo_bpm - detected).abs() < 0.01,
        "{}",
        a.project().tempo_bpm
    );
}

/// Finding 3: halves of a split transposed apart (each its own first edit, under tempos
/// of their own) still glue.
#[test]
fn halves_transposed_apart_still_glue() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("clicks.wav");
    wav(&path, 48_000, &click_track(48_000.0, 100.0, 16, 0.25).audio);
    let mut p = project(120.0);
    two_tempos(&mut p);
    let mut a = app(p);
    a.import_audio(std::slice::from_ref(&path), Ticks::beats(12), None);
    settle(&mut a);
    let (t, c) = audio_clips(&a).remove(0);
    // Halves at beat 18 (the right one under the second tempo), as a split makes them.
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
    assert!(a.set_warp_cents((t, c.id), 300, false));
    assert!(a.set_warp_cents((t, right.id), 300, false));
    let l = clip(&a, (t, c.id));
    let rr = clip(&a, (t, right.id));
    let glued = cantus_ui::audio_clips::glue_in(a.project(), t, &[&l, &rr]);
    assert!(glued.is_some(), "the halves glue");
}

/// Finding 4: a reversed clip that follows the tempo, under a tempo track, is trimmed by
/// its ticks (content ticks are exact for it), and its halves glue again.
#[test]
fn a_reversed_following_clip_is_edited_by_its_ticks() {
    use cantus_ui::audio_clips::{glue_in, reversed_edit, WindowEdit};
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("clicks.wav");
    wav(&path, 48_000, &click_track(48_000.0, 120.0, 16, 0.25).audio);
    let mut p = project(120.0);
    two_tempos(&mut p);
    let mut a = app(p);
    a.import_audio(std::slice::from_ref(&path), Ticks::ZERO, None);
    settle(&mut a);
    let (t, c) = audio_clips(&a).remove(0);
    let r = (t, c.id);
    assert!(a.set_warp_follow(r, true));
    let mut region = clip(&a, r).audio.unwrap();
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
        length: Ticks::beats(32),
    });
    let c = clip(&a, r);
    // The right edge back a beat in the slower section: the offset by that beat.
    let cmds = reversed_edit(a.project(), t, &c, WindowEdit::TrimEnd, -Ticks::beats(1));
    let offset = cmds.iter().find_map(|x| match x {
        Command::SetClipOffset { offset, .. } => Some(*offset),
        _ => None,
    });
    assert_eq!(offset, Some(c.offset + Ticks::beats(1)));
    assert!(!cmds
        .iter()
        .any(|x| matches!(x, Command::SetClipAudio { .. })));
    // The left edge: the offset stays.
    let cmds = reversed_edit(a.project(), t, &c, WindowEdit::TrimStart, Ticks::beats(1));
    assert!(!cmds.iter().any(|x| matches!(
        x,
        Command::SetClipOffset { .. } | Command::SetClipAudio { .. }
    )));
    // Halves by the tick arithmetic glue again.
    let cut = Ticks::beats(16);
    let mut left = c.clone();
    left.length = cut - c.start;
    left.offset = c.offset + (c.end() - cut);
    let mut right = c.clone();
    right.id = ClipId(a.bus.alloc_id());
    right.start = cut;
    right.length = c.end() - cut;
    assert!(glue_in(a.project(), t, &[&left, &right]).is_some());
}

/// Two linked windows of one file: the clip at `a_at`, its copy at `b_at`, the copy
/// trimmed `b_off` in.
fn linked(
    a: &mut CantusApp,
    a_at: Ticks,
    b_at: Ticks,
    b_off: Ticks,
) -> ((TrackId, ClipId), (TrackId, ClipId)) {
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
    copy.offset = b_off;
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

/// Finding 5: *Detect tempo* on one of two linked copies keeps both windows on the audio
/// they played at their starts (each its own offset).
#[test]
fn detect_tempo_keeps_every_linked_window_on_its_audio() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("clicks 100.wav");
    wav(&path, 48_000, &click_track(48_000.0, 100.0, 8, 0.25).audio);
    let mut a = app(project(120.0));
    a.import_audio(std::slice::from_ref(&path), Ticks::ZERO, None);
    settle(&mut a);
    let (ra, rb) = linked(&mut a, Ticks::beats(4), Ticks::beats(40), Ticks::beats(6));
    analysed(&a, ra);
    let fa = heard(&a, ra, clip(&a, ra).start);
    let fb = heard(&a, rb, clip(&a, rb).start);
    assert!(a.warp_by_analysis(ra));
    assert!(
        a.clip_warp(rb).is_some_and(|w| w.follow),
        "the copy shares it"
    );
    let tick = 48_000.0 * 60.0 / (100.0 * 960.0);
    for (r, f) in [(ra, fa), (rb, fb)] {
        let d = heard(&a, r, clip(&a, r).start) - f;
        assert!(d.abs() <= tick, "{r:?} moved {d:+} frames");
    }
}

/// Finding 31: the first following edit of a linked copy under another tempo than its
/// copy's makes it unique — the copy plays exactly as it did; under one tempo, both stay
/// linked and play as they did.
#[test]
fn a_first_following_edit_of_a_linked_copy_never_moves_its_copy() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("clicks.wav");
    wav(&path, 48_000, &click_track(48_000.0, 100.0, 8, 0.25).audio);
    let mut p = project(120.0);
    two_tempos(&mut p);
    let mut a = app(p);
    a.import_audio(std::slice::from_ref(&path), Ticks::ZERO, None);
    settle(&mut a);
    let (ra, rb) = linked(&mut a, Ticks::ZERO, Ticks::beats(20), Ticks::ZERO);
    let probe = |a: &CantusApp, r| {
        let s = clip(a, r).start;
        (0..8)
            .map(|k| heard(a, r, s + Ticks::beats(k)))
            .collect::<Vec<f64>>()
    };
    let (pa, pb) = (probe(&a, ra), probe(&a, rb));
    assert!(a.set_warp_follow(ra, true));
    assert!(clip(&a, ra).link.is_none(), "made unique");
    assert!(a.clip_warp(rb).is_none(), "its copy is untouched");
    for (x, y) in probe(&a, rb).iter().zip(&pb) {
        assert!((x - y).abs() < 1e-6);
    }
    for (x, y) in probe(&a, ra).iter().zip(&pa) {
        assert!((x - y).abs() < 1.0, "{x} against {y}");
    }
    // Under one tempo the copies follow together.
    let mut a = app(project(120.0));
    a.import_audio(std::slice::from_ref(&path), Ticks::ZERO, None);
    settle(&mut a);
    let (ra, rb) = linked(&mut a, Ticks::ZERO, Ticks::beats(20), Ticks::beats(2));
    let pb = probe(&a, rb);
    assert!(a.set_warp_follow(ra, true));
    assert!(clip(&a, ra).link.is_some());
    assert!(a.clip_warp(rb).is_some_and(|w| w.follow));
    for (x, y) in probe(&a, rb).iter().zip(&pb) {
        assert!((x - y).abs() < 1.0, "{x} against {y}");
    }
}

/// Finding 9: *Quantise audio* on a reversed clip pulls each hit to the line it is heard
/// on; on a loop, only the hits of its period (what is heard) are taken.
#[test]
fn quantise_hears_a_reversed_clip_and_a_loops_period() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("clicks 120.wav");
    // A click every beat from 0.1 s (192 ticks at 120 BPM).
    wav(&path, 48_000, &click_track(48_000.0, 120.0, 2, 0.1).audio);
    let q = cantus_ui::warp::QuantiseSettings {
        grid: Ticks::beats(1),
        strength: 1.0,
        threshold: 0.1,
    };
    // Reversed, 4 beats and 120 ticks long: every hit heard off a line.
    let mut a = app(project(120.0));
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
        length: Ticks::beats(4) + Ticks(120),
    });
    analysed(&a, r);
    assert_eq!(a.quantise_audio(&[r], q), 1);
    let c = clip(&a, r);
    let w = a.clip_warp(r).unwrap().clone();
    let onsets = a.warp_analysis(r).unwrap().onsets.clone();
    let mut checked = 0;
    for f in onsets {
        let content = w.tick_at(f);
        let (lo, hi) = (c.offset + Ticks(240), c.offset + c.length - Ticks(240));
        if content <= lo.0 as f64 || content >= hi.0 as f64 {
            continue;
        }
        let heard = (c.start + c.offset + c.length).0 as f64 - content;
        let off = heard - (heard / 960.0).round() * 960.0;
        assert!(off.abs() < 1.0, "a hit heard {off:+} ticks off its line");
        checked += 1;
    }
    assert!(checked >= 2, "{checked}");
    // A loop of two beats, eight long: only the period's two hits.
    let mut a = app(project(120.0));
    a.import_audio(std::slice::from_ref(&path), Ticks::ZERO, None);
    settle(&mut a);
    let (t, c) = audio_clips(&a).remove(0);
    let r = (t, c.id);
    a.exec(Command::SetClipLoop {
        track: t,
        clip: r.1,
        content_length: Some(Ticks::beats(2)),
    });
    a.exec(Command::SetClipBounds {
        track: t,
        clip: r.1,
        start: Ticks::ZERO,
        length: Ticks::beats(8),
    });
    analysed(&a, r);
    assert_eq!(a.quantise_audio(&[r], q), 1);
    let w = a.clip_warp(r).unwrap();
    let onsets = &a.warp_analysis(r).unwrap().onsets;
    let pulled = w
        .markers
        .iter()
        .filter(|m| !m.auto && onsets.iter().any(|o| (m.frame - o).abs() < 1.0))
        .count();
    assert_eq!(pulled, 2, "{:?}", w.markers);
}

/// Finding 30: an import's analysis that lands after another project is opened never
/// warps that project's clip of the same ids.
#[test]
fn an_import_job_never_lands_on_another_project() {
    let dir = tempfile::tempdir().unwrap();
    let path = loop_file(dir.path(), "drums 92.wav", 92.0, 4, 13);
    let mut a = app(project(128.0));
    a.import_audio(&[path], Ticks::ZERO, None);
    // The same project, opened again before the analysis lands.
    let again = a.project().clone();
    assert!(audio_clips(&a)[0].1.audio.as_ref().unwrap().warp.is_none());
    a.replace_project(again);
    settle(&mut a);
    let (_, c) = audio_clips(&a).remove(0);
    assert!(
        c.audio.unwrap().warp.is_none(),
        "not warped by the old import"
    );
}

/// Finding 7: an import's analysis settling seals only the import's own step — a drag
/// going on meanwhile stays one step.
#[test]
fn an_import_settling_never_splits_a_drag() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("long.wav");
    // Not a loop: the analysis is declined.
    wav(&path, 48_000, &click_track(48_000.0, 100.0, 8, 0.25).audio);
    let mut a = app(project(120.0));
    let gain0 = a.project().master.gain_db;
    a.import_audio(std::slice::from_ref(&path), Ticks::ZERO, None);
    let key = cantus_core::MergeKey::of(&("test-drag", 1));
    a.exec_merging(Command::SetMasterGain { gain_db: -3.0 }, key);
    settle(&mut a);
    a.exec_merging(Command::SetMasterGain { gain_db: -6.0 }, key);
    a.undo();
    let g = a.project().master.gain_db;
    assert_eq!(g, gain0, "one undo takes the whole drag back");
}

/// The warp a gesture starts from on a clip whose own does not follow: the markers it
/// kept; holding none, natural markers.
#[test]
fn a_warp_start_takes_kept_markers_or_natural_ones() {
    let dir = tempfile::tempdir().unwrap();
    let (mut a, r) = warped_loop(dir.path());
    let w0 = a.clip_warp(r).unwrap().clone();
    let mut off = w0.clone();
    off.follow = false;
    assert!(a.commit_warp(r, Some(off), WarpOp::Follow, false));
    let s = a.warp_start(r).unwrap();
    assert_eq!(s.warp.markers, w0.markers);
    assert!(s.base.is_some());
    let mut bare = Warp::new(44_100, 97.0);
    bare.follow = false;
    assert!(a.commit_warp(r, Some(bare), WarpOp::Set, false));
    let s = a.warp_start(r).unwrap();
    assert!(s.warp.follow && s.warp.markers.len() >= 2);
    assert_eq!(s.shift, Ticks::ZERO);
}

/// One frame of the warp editor on clip `r`, `size` points, with `events`.
fn editor(
    a: &mut CantusApp,
    ctx: &egui::Context,
    r: (TrackId, ClipId),
    size: egui::Vec2,
    events: Vec<egui::Event>,
    _modifiers: egui::Modifiers,
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

/// A clip in the warp editor.
fn in_editor(dir: &Path) -> (CantusApp, (TrackId, ClipId)) {
    let path = dir.join("clicks.wav");
    wav(&path, 48_000, &click_track(48_000.0, 100.0, 4, 0.25).audio);
    let mut a = app(project(120.0));
    a.import_audio(std::slice::from_ref(&path), Ticks::ZERO, None);
    settle(&mut a);
    let (t, c) = audio_clips(&a).remove(0);
    let r = (t, c.id);
    a.open_warp_editor(r);
    (a, r)
}

/// Finding 44: Ctrl+wheel over the editor zooms it (egui hands it over as a zoom factor,
/// never a scroll), about the pointer.
#[test]
fn ctrl_and_the_wheel_zoom_the_warp_editor() {
    let dir = tempfile::tempdir().unwrap();
    let (mut a, r) = in_editor(dir.path());
    let ctx = support::ctx();
    let size = egui::vec2(1200.0, 500.0);
    editor(&mut a, &ctx, r, size, vec![], egui::Modifiers::NONE);
    let wave = a.warp.view.wave.expect("the waveform drawn");
    let at = wave.center();
    for _ in 0..3 {
        editor(
            &mut a,
            &ctx,
            r,
            size,
            vec![
                egui::Event::PointerMoved(at),
                egui::Event::MouseWheel {
                    unit: egui::MouseWheelUnit::Line,
                    delta: egui::vec2(0.0, 1.0),
                    phase: egui::TouchPhase::Move,
                    modifiers: egui::Modifiers::COMMAND,
                },
            ],
            egui::Modifiers::COMMAND,
        );
    }
    assert!(a.warp.view.zoom > 1.01, "zoom {}", a.warp.view.zoom);
}

/// Finding 47: every key of the editor's bar is laid inside the pane — at an ordinary
/// width and at a narrow one (it takes a second row, never pushes *Reset* off screen).
#[test]
fn the_warp_bar_fits_its_pane() {
    let dir = tempfile::tempdir().unwrap();
    let (mut a, r) = in_editor(dir.path());
    for width in [1400.0f32, 1000.0, 720.0] {
        let ctx = support::ctx();
        let size = egui::vec2(width, 500.0);
        for _ in 0..2 {
            editor(&mut a, &ctx, r, size, vec![], egui::Modifiers::NONE);
        }
        let rects = &a.warp.view.bar_rects;
        assert_eq!(rects.len(), 11, "{rects:?}");
        for (item, rect) in rects {
            assert!(
                rect.left() >= -0.5 && rect.right() <= width + 0.5,
                "{item:?} at {rect:?} in a {width} point pane"
            );
        }
        let reset = rects
            .iter()
            .find(|(i, _)| *i == cantus_ui::views::warp::BarItem::Reset)
            .unwrap();
        assert!(reset.1.right() <= width);
    }
}

/// Findings 25, 29: an offline render compiled before a warped clip's render was in says
/// so — the file holds its quick version, never silently.
#[test]
fn a_render_compiled_before_its_warp_renders_says_so() {
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
    let late = a.engine.renders_late();
    let p = a.project().clone();
    let _ = a.engine.compile_for_render(&p, 1);
    a.say_late(late);
    a.engine.samples.warp().hold_finals(false);
    assert!(a.status.error, "said as a warning");
    assert_eq!(a.status.text, cantus_ui::i18n::t("warp.status.late"));
}

/// Finding 53: a marker's speed is read against the tempo in force where it is heard — a
/// clip under a later, slower tempo that follows it as it played reads ×1.
#[test]
fn a_markers_speed_is_read_against_the_tempo_in_force() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("clicks.wav");
    wav(&path, 48_000, &click_track(48_000.0, 100.0, 4, 0.25).audio);
    let mut p = project(120.0);
    two_tempos(&mut p);
    let mut a = app(p);
    a.import_audio(std::slice::from_ref(&path), Ticks::beats(20), None);
    settle(&mut a);
    let (t, c) = audio_clips(&a).remove(0);
    let r = (t, c.id);
    assert!(a.set_warp_follow(r, true));
    let w = a.clip_warp(r).unwrap().clone();
    let i = w.markers.iter().position(|m| !m.auto).unwrap();
    let k = cantus_ui::views::arrangement::warp_api::marker_speed(&a, r, i).unwrap();
    assert!((k - 1.0).abs() < 1e-6, "×{k}");
}

/// Findings 3, 32: a mode picked or a transposition set on a clip that was not warped
/// leaves no markers — so its export says nothing about markers lost.
#[test]
fn a_mode_pick_exports_without_a_markers_report() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("clicks.wav");
    wav(&path, 48_000, &click_track(48_000.0, 100.0, 4, 0.25).audio);
    let mut p = project(120.0);
    two_tempos(&mut p);
    let mut a = app(p);
    a.import_audio(std::slice::from_ref(&path), Ticks::beats(8), None);
    settle(&mut a);
    let (t, c) = audio_clips(&a).remove(0);
    assert!(a.set_warp_mode((t, c.id), WarpMode::Rhythm));
    assert!(a.set_warp_cents((t, c.id), 200, false));
    let out = cantus_project::dawproject::export_with(a.project(), "test", None);
    assert!(!out.report.has("interop.report.x_warp_markers"));
    assert!(out.report.has("interop.report.x_warp_transpose"));
}

/// Finding 55: a report line that counts one says it in the singular ("One take of …",
/// never "1 take(s)"), in both languages.
#[test]
fn a_report_line_of_one_is_singular() {
    let item = |n: &str| cantus_project::ReportItem {
        severity: cantus_project::Severity::Dropped,
        key: "interop.report.x_takes",
        args: vec![("track", "Vox".into()), ("n", n.into())],
    };
    let _ = app(project(120.0));
    let one = cantus_ui::views::interop::report_line(&item("1"));
    let two = cantus_ui::views::interop::report_line(&item("2"));
    assert!(one.starts_with("One take of Vox"), "{one}");
    assert!(two.starts_with("2 takes of Vox"), "{two}");
    assert!(!one.contains("(s)") && !two.contains("(s)"));
}
