//! Warping from the interface (round 29, ADR 0061): the import rule (a loop warps itself
//! in the import's own undo step; anything else stays at natural speed), markers added,
//! moved and deleted, *Quantise audio*, the arrangement's warp handles, and *Bounce
//! warped audio*.

mod support;

use cantus_core::warp::{Warp, WarpMode};
use cantus_core::{Clip, ClipId, Project, Ticks, TrackId, PPQ};
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

/// Poll the app's warp jobs until none waits (or a minute passes).
fn settle(a: &mut CantusApp) {
    let t = Instant::now();
    while a.poll_warp() && t.elapsed() < Duration::from_secs(60) {
        std::thread::sleep(Duration::from_millis(10));
    }
}

/// The analysis of clip `r`, waited for.
fn analysed(a: &CantusApp, r: (TrackId, ClipId)) {
    let t = Instant::now();
    while a.warp_analysis(r).is_none() && t.elapsed() < Duration::from_secs(60) {
        std::thread::sleep(Duration::from_millis(10));
    }
    assert!(a.warp_analysis(r).is_some(), "the analysis never came");
}

#[test]
fn a_loop_imported_warps_itself_in_the_imports_undo_step() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("drums 92.wav");
    let mut m = drum_loop(44_100.0, 92.0, 4, 0.0, 0.0, 3);
    m.audio
        .truncate((60.0 / 92.0 * 16.0 * 44_100.0_f64).round() as usize);
    wav(&path, 44_100, &m.audio);
    let mut a = app(project(128.0));
    let before = a.project().clone();
    assert_eq!(a.import_audio(&[path], Ticks::ZERO, None), 1);
    settle(&mut a);
    let clips = audio_clips(&a);
    let (_, c) = &clips[0];
    let w = c
        .audio
        .as_ref()
        .unwrap()
        .warp
        .as_ref()
        .expect("warped by itself");
    assert!(w.follow);
    assert_eq!(w.mode, WarpMode::Rhythm);
    assert!((w.tempo() - 92.0).abs() < 0.05, "{}", w.tempo());
    // Four bars at the project's tempo.
    assert_eq!(c.length, Ticks(16 * PPQ));
    // One undo step: the import and its warp together.
    a.undo();
    assert_eq!(a.project().tracks, before.tracks);
}

#[test]
fn a_file_that_is_not_a_loop_stays_at_natural_speed() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("clicks.wav");
    // Eight bars with a lead-in and a tail: not a whole number of bars.
    let m = click_track(48_000.0, 100.0, 8, 0.31);
    wav(&path, 48_000, &m.audio);
    let mut a = app(project(120.0));
    a.import_audio(&[path], Ticks::ZERO, None);
    settle(&mut a);
    let (t, c) = audio_clips(&a).remove(0);
    assert!(c.audio.as_ref().unwrap().warp.is_none());
    // The analysis is there for the editor and *Quantise audio*.
    analysed(&a, (t, c.id));
}

#[test]
fn a_marker_is_added_moved_and_deleted_by_its_key() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("clicks.wav");
    let m = click_track(48_000.0, 120.0, 4, 0.0);
    wav(&path, 48_000, &m.audio);
    let mut a = app(project(120.0));
    a.import_audio(&[path], Ticks::ZERO, None);
    settle(&mut a);
    let (t, c) = audio_clips(&a).remove(0);
    let r = (t, c.id);
    assert!(a.add_warp_marker(r, Ticks::beats(2), None));
    let w = a.clip_warp(r).cloned().unwrap();
    let i = w.marker_at(Ticks::beats(2)).unwrap();
    assert_eq!(a.warp.selected, Some((r, i)));
    // Moved: the audio between its neighbours stretches.
    let moved = w.move_marker(i, Ticks::beats(3));
    assert!(a.commit_warp(r, Some(moved), cantus_core::warp::WarpOp::MoveMarker, false));
    assert_eq!(a.clip_warp(r).unwrap().markers[i].tick, Ticks::beats(3));
    // Del removes the selected marker, not the clip.
    a.run_action(cantus_ui::keymap::Action::Delete);
    assert!(a.clip_warp(r).unwrap().marker_at(Ticks::beats(3)).is_none());
    assert_eq!(audio_clips(&a).len(), 1);
}

#[test]
fn quantise_pulls_a_loose_take_onto_the_grid() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("loose.wav");
    // Clicks on quarters at 120 BPM, each up to 20 ms early or late.
    let rate = 48_000.0f32;
    let mut x = vec![0.0f32; (rate * 5.0) as usize];
    let offs = [0.0, 0.018, -0.015, 0.02, -0.012, 0.01, -0.019, 0.014];
    for (k, o) in offs.iter().enumerate() {
        let at = ((0.1 + k as f32 * 0.5 + o) * rate) as usize;
        for j in 0..(0.02 * rate) as usize {
            let t = j as f32 / rate;
            x[at + j] +=
                0.7 * (2.0 * std::f32::consts::PI * 1_500.0 * t).sin() * (-t / 0.004).exp();
        }
    }
    wav(&path, 48_000, &x);
    let mut a = app(project(120.0));
    a.import_audio(&[path], Ticks::ZERO, None);
    settle(&mut a);
    let (t, c) = audio_clips(&a).remove(0);
    let r = (t, c.id);
    analysed(&a, r);
    let q = cantus_ui::warp::QuantiseSettings {
        grid: Ticks(PPQ),
        strength: 1.0,
        threshold: 0.1,
    };
    // The grid is laid from tick 0; the hits start 0.1 s (a fifth of a beat) in.
    assert_eq!(a.quantise_audio(&[r], q), 1);
    let an = a.warp_analysis(r).unwrap();
    let c = a.project().track(t).unwrap().clip(c.id).unwrap().clone();
    let map = a.project().tempo_map();
    for f in &an.onsets {
        let tick = cantus_core::audio_time::clip_tick_at_frame(
            &map,
            c.start,
            c.offset,
            c.audio.as_ref().unwrap(),
            48_000.0,
            *f,
        );
        let off = tick - (tick / PPQ as f64).round() * PPQ as f64;
        assert!(off.abs() <= 1.0, "a hit {off} ticks off the grid");
    }
}

#[test]
fn the_arrangement_draws_warp_markers_to_grab() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("clicks.wav");
    let m = click_track(48_000.0, 120.0, 4, 0.0);
    wav(&path, 48_000, &m.audio);
    let mut a = app(project(120.0));
    a.import_audio(&[path], Ticks::ZERO, None);
    settle(&mut a);
    let (t, c) = audio_clips(&a).remove(0);
    let r = (t, c.id);
    let w = Warp::identity(48_000, 120.0, 0)
        .add_marker(Ticks::beats(2))
        .unwrap();
    a.commit_warp(
        r,
        Some(w.move_marker(1, Ticks::beats(3))),
        cantus_core::warp::WarpOp::Set,
        false,
    );
    a.select_clip(t, c.id);
    let ctx = support::ctx();
    support::paint(&ctx, egui::vec2(1400.0, 700.0), 3, |ui| {
        cantus_ui::views::arrangement::show(&mut a, ui);
    });
    let h = a.arrangement.audio_handles(r).expect("the clip's handles");
    let hits = h
        .warp
        .unwrap_or_else(|| panic!("no warp handles; wave {:?}", h.wave));
    // The marker on the clip's first frame sits on its edge; the moved one inside.
    assert!(!hits.marker_xs().is_empty(), "the moved marker is drawn");
    let x = *hits.marker_xs().last().unwrap();
    let y = hits.band.center().y;
    assert_eq!(
        hits.at(egui::pos2(x, y)),
        Some(cantus_ui::views::arrangement::AudioHandle::WarpMarker(1))
    );
}

#[test]
fn bounce_warped_audio_leaves_the_clip_playing_a_file_of_its_own() {
    let dir = tempfile::tempdir().unwrap();
    std::env::set_var("CANTUS_DATA_DIR", dir.path());
    let path = dir.path().join("clicks.wav");
    let m = click_track(48_000.0, 100.0, 4, 0.0);
    wav(&path, 48_000, &m.audio);
    let mut a = app(project(120.0));
    a.import_audio(std::slice::from_ref(&path), Ticks::ZERO, None);
    settle(&mut a);
    let (t, c) = audio_clips(&a).remove(0);
    let r = (t, c.id);
    let mut w = Warp::identity(48_000, 100.0, 0);
    w.cents = -300;
    a.commit_warp(r, Some(w), cantus_core::warp::WarpOp::Set, false);
    assert!(a.bounce_warped(r));
    settle(&mut a);
    let c = a.project().track(t).unwrap().clip(r.1).unwrap().clone();
    let region = c.audio.unwrap();
    assert!(region.warp.is_none(), "it plays the bounce, unwarped");
    let s = a.project().audio_source(region.source).unwrap();
    assert_ne!(Path::new(&s.path), path.as_path());
    assert!(Path::new(&s.path).exists() || dir.path().join(&s.path).exists());
    // One undo step back to the warped clip.
    a.undo();
    assert!(a.clip_warp(r).is_some());
}

#[test]
fn alt_dragging_the_right_edge_stretches_the_clip() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("clicks.wav");
    let m = click_track(48_000.0, 120.0, 2, 0.0);
    wav(&path, 48_000, &m.audio);
    let mut a = app(project(120.0));
    a.import_audio(&[path], Ticks::ZERO, None);
    settle(&mut a);
    let (t, c) = audio_clips(&a).remove(0);
    let r = (t, c.id);
    let alt = egui::Modifiers {
        alt: true,
        ..Default::default()
    };
    cantus_ui::views::arrangement::warp_api::modifiers(&mut a, alt);
    a.commit_clip_drag(
        r,
        cantus_ui::views::arrangement::ClipDragResult::TrimEnd { delta: c.length },
    );
    let after = a.project().track(t).unwrap().clip(r.1).unwrap().clone();
    assert_eq!(after.length, c.length + c.length, "twice as long");
    let w = a.clip_warp(r).expect("stretched through a warp");
    // The file's last frame is heard at the new end.
    let end = w.tick_at(m.audio.len() as f64);
    assert!(
        (end - after.length.0 as f64).abs() < 0.5 * PPQ as f64,
        "{end}"
    );
    a.undo();
    assert_eq!(
        a.project().track(t).unwrap().clip(r.1).unwrap().length,
        c.length
    );
}
