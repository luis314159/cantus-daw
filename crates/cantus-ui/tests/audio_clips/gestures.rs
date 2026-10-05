//! Fades and gain by hand, through the handles the arrangement publishes; normalise;
//! split and glue; a trim that stops at the end of the file.

use super::{empty_app, import, region, wav};
use crate::support::ctx;
use cantus_core::{FadeCurve, Ticks};
use cantus_ui::{views, CantusApp};
use egui::{pos2, Pos2};
use std::time::Duration;

fn show(a: &mut CantusApp, ctx: &egui::Context, events: Vec<egui::Event>) {
    let input = egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(
            Pos2::ZERO,
            egui::vec2(1200.0, 400.0),
        )),
        events,
        ..Default::default()
    };
    ctx.run_ui(input, |ui| {
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE)
            .show(ui, |ui| views::arrangement::show(a, ui));
    })
    .textures_delta
    .clear();
}

/// Press at what `aim` picks from the frame just drawn, travel, release.
fn drag(a: &mut CantusApp, aim: impl Fn(&CantusApp) -> (Pos2, Pos2)) {
    let ctx = ctx();
    let button = |pos: Pos2, pressed: bool| egui::Event::PointerButton {
        pos,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: egui::Modifiers::NONE,
    };
    show(a, &ctx, vec![]);
    show(a, &ctx, vec![]);
    let (from, to) = aim(a);
    show(a, &ctx, vec![egui::Event::PointerMoved(from)]);
    show(
        a,
        &ctx,
        vec![egui::Event::PointerMoved(from), button(from, true)],
    );
    for i in 1..=4 {
        let at = from + (to - from) * (i as f32 / 4.0);
        show(a, &ctx, vec![egui::Event::PointerMoved(at)]);
    }
    show(
        a,
        &ctx,
        vec![egui::Event::PointerMoved(to), button(to, false)],
    );
    show(a, &ctx, vec![]);
}

fn one_clip(
    peak: f32,
) -> (
    tempfile::TempDir,
    CantusApp,
    (cantus_core::TrackId, cantus_core::ClipId),
) {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("loop.wav");
    // Two seconds: four beats at 120 BPM.
    wav(&file, 48_000, 1, 96_000, |i, _| {
        if i % 2 == 0 {
            peak
        } else {
            -peak
        }
    });
    let mut a = empty_app();
    let r = import(&mut a, &file);
    assert!(a.peaks.wait_idle(Duration::from_secs(20)));
    (dir, a, r)
}

#[test]
fn dragging_the_fade_in_handle_sets_the_fade_in_one_undo_step() {
    let (_dir, mut a, r) = one_clip(0.5);
    let undo_before = a.bus.history().len();
    drag(&mut a, |a| {
        let h = a
            .arrangement
            .audio_handles(r)
            .expect("the clip drew its handles");
        let g = a.arrangement.geometry().unwrap();
        (
            h.fade_in.center(),
            h.fade_in.center() + egui::vec2(g.px_per_beat, 0.0),
        )
    });
    let fade = region(&a, r).fade_in;
    assert!(
        (fade.0 - 960).abs() <= 2,
        "a beat's drag makes a beat of fade, got {fade:?}"
    );
    assert_eq!(
        a.bus.history().len(),
        undo_before + 1,
        "one gesture, one step"
    );
    a.undo();
    assert_eq!(region(&a, r).fade_in, Ticks::ZERO);
}

#[test]
fn dragging_the_fade_out_and_the_gain() {
    let (_dir, mut a, r) = one_clip(0.5);
    drag(&mut a, |a| {
        let h = a.arrangement.audio_handles(r).unwrap();
        let g = a.arrangement.geometry().unwrap();
        (
            h.fade_out.center(),
            h.fade_out.center() - egui::vec2(g.px_per_beat * 0.5, 0.0),
        )
    });
    let fade = region(&a, r).fade_out;
    assert!((fade.0 - 480).abs() <= 2, "half a beat, got {fade:?}");
    let undo_before = a.bus.history().len();
    drag(&mut a, |a| {
        let h = a.arrangement.audio_handles(r).unwrap();
        (h.gain.center(), h.gain.center() + egui::vec2(0.0, 30.0))
    });
    let gain = region(&a, r).gain_db;
    assert!(
        (gain + 6.0).abs() < 0.01,
        "30 points down is -6 dB, got {gain}"
    );
    assert_eq!(a.bus.history().len(), undo_before + 1);
    a.undo();
    assert_eq!(region(&a, r).gain_db, 0.0);
    assert!(
        region(&a, r).fade_out > Ticks::ZERO,
        "the fade is a step of its own"
    );
}

#[test]
fn normalise_sets_the_gain_the_peaks_call_for() {
    let (_dir, mut a, r) = one_clip(0.5);
    assert_eq!(views::arrangement::normalize_selection(&mut a), 1);
    let q = (0.5f32 * 32767.0).round() / 32768.0;
    let want = -0.1 - 20.0 * q.log10();
    let gain = region(&a, r).gain_db;
    assert!((gain - want).abs() < 0.01, "{gain} vs {want}");
    a.undo();
    assert_eq!(region(&a, r).gain_db, 0.0);
}

#[test]
fn split_keeps_the_fades_outside_and_glue_gives_the_clip_back() {
    let (_dir, mut a, r) = one_clip(0.5);
    let mut faded = region(&a, r);
    faded.fade_in = Ticks::beats(1);
    faded.fade_out = Ticks::beats(1);
    faded.fade_out_curve = FadeCurve::EqualPower;
    a.exec(cantus_core::Command::SetClipAudio {
        track: r.0,
        clip: r.1,
        audio: Some(faded.clone()),
    });
    let before = a.project().track(r.0).unwrap().clip(r.1).unwrap().clone();
    a.seek(Ticks::beats(2));
    a.split_clips_at_playhead();
    let clips = a.project().track(r.0).unwrap().clips.clone();
    assert_eq!(clips.len(), 2);
    let (l, rt) = (&clips[0], &clips[1]);
    let (la, ra) = (l.audio.as_ref().unwrap(), rt.audio.as_ref().unwrap());
    assert_eq!((la.fade_in, la.fade_out), (Ticks::beats(1), Ticks::ZERO));
    assert_eq!((ra.fade_in, ra.fade_out), (Ticks::ZERO, Ticks::beats(1)));
    assert_eq!(
        rt.offset,
        Ticks::beats(2),
        "the right half shows the file from there"
    );
    a.set_clip_selection(vec![(r.0, l.id), (r.0, rt.id)]);
    a.glue_clips();
    let after = a.project().track(r.0).unwrap();
    assert_eq!(after.clips.len(), 1);
    let glued = &after.clips[0];
    assert_eq!(
        (glued.start, glued.length, glued.offset),
        (before.start, before.length, before.offset)
    );
    assert_eq!(glued.audio, before.audio, "fades, curves and all");
}

#[test]
fn glue_refuses_audio_windows_that_do_not_continue_each_other() {
    let (_dir, mut a, r) = one_clip(0.5);
    a.seek(Ticks::beats(2));
    a.split_clips_at_playhead();
    let right = a.project().track(r.0).unwrap().clips[1].id;
    // Slip the right half: the two no longer continue the file.
    a.exec(cantus_core::Command::SetClipOffset {
        track: r.0,
        clip: right,
        offset: Ticks::beats(1),
    });
    a.set_clip_selection(vec![(r.0, r.1), (r.0, right)]);
    a.glue_clips();
    assert_eq!(a.project().track(r.0).unwrap().clips.len(), 2);
}

#[test]
fn a_trim_stops_at_the_end_of_the_file() {
    let (_dir, mut a, r) = one_clip(0.5);
    drag(&mut a, |a| {
        let g = a.arrangement.geometry().unwrap();
        let edge = g.x(4.0) - 3.0;
        // Low on the edge: the top corner is the fade handle.
        let y = g.rows[1].top + g.rows[1].height - 5.0;
        (pos2(edge, y), pos2(edge + 4.0 * g.px_per_beat, y))
    });
    let clip = a.project().track(r.0).unwrap().clip(r.1).unwrap();
    assert_eq!(
        clip.length,
        Ticks::beats(4),
        "the window may not pass the file's end"
    );
}
