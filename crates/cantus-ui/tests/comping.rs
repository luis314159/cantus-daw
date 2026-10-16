//! Comping, Logic / Studio One-class (ADR 0058): seams that crossfade and move as a unit,
//! pitch edits kept through a swipe, alternate comps (switch, new, rename, delete) that
//! come back whole, flatten and flatten-and-merge, grouped comping, quick swipe — each one
//! undo step, undone exactly.

use cantus_core::audio::AudioSource;
use cantus_core::pitch::{PitchEdit, PitchKey, PitchNote};
use cantus_core::*;
use cantus_engine::EngineHandle;
use cantus_ui::{comping, CantusApp};

fn beats(n: i64) -> Ticks {
    Ticks::beats(n)
}

fn source(p: &mut Project, name: &str) -> AudioSourceId {
    let id = AudioSourceId(p.alloc_id());
    p.audio_pool.push(AudioSource {
        id,
        name: name.into(),
        path: format!("C:/rec/{name}.wav"),
        sample_rate: 48_000,
        channels: 1,
        frames: 480_000,
        sha256: None,
        recorded: true,
    });
    id
}

/// Takes A (lane 1) and B (lane 2) over two bars, B playing on lane 0.
fn takes(p: &mut Project, name: &str) -> (TrackId, AudioSourceId, AudioSourceId) {
    let t = p.make_track(name);
    let tid = t.id;
    p.tracks.push(t);
    let a = source(p, &format!("{name} A"));
    let b = source(p, &format!("{name} B"));
    for (src, lane) in [(a, 1u8), (b, 2), (b, 0)] {
        let mut c = p.make_audio_clip("take", src, Ticks::ZERO, beats(8));
        c.lane = lane;
        p.track_mut(tid).unwrap().clips.push(c);
    }
    p.track_mut(tid).unwrap().takes_open = true;
    (tid, a, b)
}

fn app(p: Project) -> CantusApp {
    cantus_ui::i18n::set_locale(cantus_ui::i18n::Locale::En);
    CantusApp::from_parts(Some(p), EngineHandle::dummy(), false)
}

fn lane0(a: &CantusApp, t: TrackId) -> Vec<(i64, i64, AudioSourceId, i64)> {
    let mut v: Vec<_> = a
        .project()
        .track(t)
        .unwrap()
        .clips
        .iter()
        .filter(|c| c.lane == 0)
        .map(|c| {
            let au = c.audio.as_ref().unwrap();
            (c.start.0, c.end().0, au.source, au.xfade.0)
        })
        .collect();
    v.sort();
    v
}

fn edit() -> PitchEdit {
    PitchEdit {
        rate: 48_000,
        frames: 480_000,
        analyser: 1,
        key: PitchKey { tonic: 0, scale: 0 },
        notes: vec![PitchNote::sung(0, 1_000, 40_000, 60.4, -12.0)],
        bypass: false,
    }
}

#[test]
fn a_swipe_makes_seams_that_crossfade_by_the_projects_default() {
    let mut p = Project::default();
    let (t, a, b) = takes(&mut p, "Vox");
    let mut a_ = app(p);
    let x = comping::xfade_ticks(a_.project());
    assert_eq!(x, Ticks(19), "10 ms at 120 BPM");
    let steps = a_.bus.history().len();
    a_.comp_swipe(t, 1, beats(2), beats(4));
    assert_eq!(a_.bus.history().len(), steps + 1, "one undo step");
    // Both new seams crossfade; the window from the first bar's B has none (no seam).
    assert_eq!(
        lane0(&a_, t),
        vec![(0, 1920, b, 0), (1920, 3840, a, x.0), (3840, 7680, b, x.0)]
    );
    // A crossfade set by hand is kept by the next swipe.
    a_.set_seam_xfade(t, None, Ticks(40));
    a_.comp_swipe(t, 2, beats(5), beats(6));
    let seams: Vec<i64> = comping::seams(a_.project().track(t).unwrap())
        .iter()
        .map(|s| s.3 .0)
        .collect();
    assert!(seams.contains(&40), "{seams:?}");
}

#[test]
fn a_swipe_that_grows_a_window_keeps_its_pitch_edits() {
    let mut p = Project::default();
    let (t, _, _) = takes(&mut p, "Vox");
    let mut a = app(p);
    a.comp_swipe(t, 1, beats(2), beats(4));
    // The comp's window on take A is edited in the pitch editor…
    let w = a
        .project()
        .track(t)
        .unwrap()
        .clips
        .iter()
        .find(|c| c.lane == 0 && c.start == beats(2))
        .unwrap()
        .id;
    a.exec(Command::SetClipPitch {
        track: t,
        clip: w,
        pitch: Some(Box::new(edit())),
        op: cantus_core::pitch::PitchOp::Notes,
    });
    // …then grown by a second stroke next to it: the edits stay.
    a.comp_swipe(t, 1, beats(4), beats(5));
    let grown = a
        .project()
        .track(t)
        .unwrap()
        .clips
        .iter()
        .find(|c| c.lane == 0 && c.start == beats(2))
        .unwrap();
    assert_eq!(grown.end(), beats(5));
    assert_eq!(
        grown.audio.as_ref().unwrap().pitch.as_deref(),
        Some(&edit())
    );
}

#[test]
fn a_seam_moves_as_a_unit_from_its_handle_and_from_a_windows_edge() {
    let mut p = Project::default();
    let (t, _, _) = takes(&mut p, "Vox");
    let mut a = app(p);
    a.comp_swipe(t, 1, beats(2), beats(4));
    let before = a.project().clone();
    let (l, r, at, _) = comping::seams(a.project().track(t).unwrap())[0];
    assert_eq!(at, beats(2));
    let steps = a.bus.history().len();
    for k in 1..=10 {
        a.move_comp_seam(t, l, r, at + Ticks(k * 30), true);
    }
    a.end_gesture();
    assert_eq!(a.bus.history().len(), steps + 1, "a drag is one step");
    let tr = a.project().track(t).unwrap();
    assert_eq!(tr.clip(l).unwrap().end(), at + Ticks(300));
    assert_eq!(tr.clip(r).unwrap().start, at + Ticks(300));
    a.undo();
    assert_eq!(a.project(), &before);
    // A window's edge trimmed in the arrangement at a seam moves the seam too.
    let clip = a.project().track(t).unwrap().clip(r).unwrap().clone();
    let cmd = comping::seam_trim(a.project(), t, &clip, true, Ticks(-120)).unwrap();
    assert!(
        matches!(cmd.as_slice(), [Command::SetCompSeam { at: x, .. }] if *x == at - Ticks(120))
    );
    a.commit_clip_drag(
        (t, r),
        cantus_ui::views::arrangement::ClipDragResult::TrimStart { delta: Ticks(-120) },
    );
    assert_eq!(
        a.project().track(t).unwrap().clip(l).unwrap().end(),
        at - Ticks(120)
    );
}

#[test]
fn alternate_comps_switch_and_come_back_whole() {
    let mut p = Project::default();
    let (t, _, _) = takes(&mut p, "Vox");
    let mut a = app(p);
    a.comp_swipe(t, 1, beats(2), beats(4));
    let w = a
        .project()
        .track(t)
        .unwrap()
        .clips
        .iter()
        .find(|c| c.lane == 0 && c.start == beats(2))
        .unwrap()
        .id;
    a.exec(Command::SetClipPitch {
        track: t,
        clip: w,
        pitch: Some(Box::new(edit())),
        op: cantus_core::pitch::PitchOp::Notes,
    });
    let comp_a = lane0(&a, t);
    // New comp B (a copy, played), changed.
    a.duplicate_comp(t);
    let tr = a.project().track(t).unwrap();
    assert_eq!(tr.comps.len(), 2);
    assert_eq!((tr.active_comp, tr.comps[1].name.as_str()), (1, "B"));
    a.promote_take_lane(t, 2);
    assert_ne!(lane0(&a, t), comp_a);
    let comp_b = lane0(&a, t);
    // Back to A: exactly what A was, its pitch edits too.
    let steps = a.bus.history().len();
    a.switch_comp(t, 0);
    assert_eq!(a.bus.history().len(), steps + 1);
    assert_eq!(lane0(&a, t), comp_a);
    assert!(a
        .project()
        .track(t)
        .unwrap()
        .clips
        .iter()
        .any(|c| c.lane == 0 && c.audio.as_ref().unwrap().pitch.is_some()));
    a.switch_comp(t, 1);
    assert_eq!(lane0(&a, t), comp_b);
    a.rename_comp(t, 1, "Chorus best");
    assert_eq!(a.project().track(t).unwrap().comps[1].name, "Chorus best");
    // A comp that does not play can go; the last one left is the plain lane again.
    let before = a.project().clone();
    a.delete_comp(t, 0);
    assert!(a.project().track(t).unwrap().comps.is_empty());
    a.undo();
    assert_eq!(a.project(), &before);
}

#[test]
fn unused_takes_are_the_ones_no_comp_plays() {
    let mut p = Project::default();
    let (t, a_src, _) = takes(&mut p, "Vox");
    let mut a = app(p);
    // Comp A plays take A; comp B plays take B: both takes stay.
    a.promote_take_lane(t, 1);
    a.duplicate_comp(t);
    a.promote_take_lane(t, 2);
    a.delete_unused_takes(t);
    let tr = a.project().track(t).unwrap();
    assert_eq!(tr.clips.iter().filter(|c| c.lane > 0).count(), 2);
    assert!(a.project().audio_source(a_src).is_some());
    // The source a kept window plays cannot leave the pool.
    assert!(a
        .bus
        .execute(Command::RemoveAudioSource { id: a_src }, Origin::User)
        .is_err());
}

#[test]
fn flatten_keeps_the_comp_and_lets_the_takes_go() {
    let mut p = Project::default();
    let (t, a_src, b_src) = takes(&mut p, "Vox");
    let mut a = app(p);
    a.promote_take_lane(t, 1);
    let comp = lane0(&a, t);
    let before = a.project().clone();
    a.flatten_comp(t);
    let tr = a.project().track(t).unwrap();
    assert!(tr.clips.iter().all(|c| c.lane == 0));
    assert!(!tr.takes_open && tr.comps.is_empty());
    assert_eq!(lane0(&a, t), comp);
    assert!(a.project().audio_source(a_src).is_some());
    assert!(a.project().audio_source(b_src).is_none(), "nothing plays B");
    a.undo();
    assert_eq!(a.project(), &before);
}

/// A stereo file of `frames` frames holding (l, r) in every one.
fn constant_wav(path: &std::path::Path, frames: usize, l: f32, r: f32) {
    let spec = hound::WavSpec {
        channels: 2,
        sample_rate: 48_000,
        bits_per_sample: 32,
        sample_format: hound::SampleFormat::Float,
    };
    let mut w = hound::WavWriter::create(path, spec).unwrap();
    for _ in 0..frames {
        w.write_sample(l).unwrap();
        w.write_sample(r).unwrap();
    }
    w.finalize().unwrap();
}

/// Run the merge's frames until it is done (it waits for loading, then renders on a
/// worker).
fn finish_merge(a: &mut CantusApp) {
    for _ in 0..2_000 {
        a.poll_merge();
        if !a.merging() {
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    panic!("the merge never finished");
}

/// Review 1: the bounce is what the comp plays — take A (all left) to the seam, take B
/// (all right) after, the seam's crossfade equal-power between — not merely a file.
#[test]
fn flatten_and_merge_bounces_the_comp_into_one_clip() {
    let dir = tempfile::tempdir().unwrap();
    let mut p = Project {
        tempo_bpm: 120.0,
        ..Default::default()
    };
    let t = p.make_track("Vox");
    let tid = t.id;
    p.tracks.push(t);
    let frames = 8 * 24_000;
    let mut srcs = Vec::new();
    for (name, l, r) in [("A", 1.0, 0.0), ("B", 0.0, 1.0)] {
        let path = dir.path().join(format!("Vox {name}.wav"));
        constant_wav(&path, frames, l, r);
        let id = AudioSourceId(p.alloc_id());
        p.audio_pool.push(AudioSource {
            id,
            name: format!("Vox {name}"),
            path: path.to_string_lossy().into_owned(),
            sample_rate: 48_000,
            channels: 2,
            frames: frames as u64,
            sha256: None,
            recorded: true,
        });
        srcs.push(id);
    }
    for (src, lane) in [(srcs[0], 1u8), (srcs[1], 2), (srcs[1], 0)] {
        let mut c = p.make_audio_clip("take", src, Ticks::ZERO, beats(8));
        c.lane = lane;
        p.track_mut(tid).unwrap().clips.push(c);
    }
    p.track_mut(tid).unwrap().takes_open = true;
    let mut a = app(p);
    a.path = Some(dir.path().join("song.cantus"));
    a.comp_swipe(tid, 1, Ticks::ZERO, beats(4));
    let before = a.project().clone();
    assert!(a.merge_comp(tid));
    finish_merge(&mut a);
    let tr = a.project().track(tid).unwrap();
    assert_eq!(tr.clips.len(), 1, "one clip, no takes");
    let c = &tr.clips[0];
    assert_eq!((c.start, c.end()), (Ticks::ZERO, beats(8)));
    let src = a
        .project()
        .audio_source(c.audio.as_ref().unwrap().source)
        .unwrap();
    assert_eq!(src.name, "Vox comp");
    let mut reader = hound::WavReader::open(&src.path).expect("the bounce is on disk");
    let samples: Vec<f32> = reader.samples::<f32>().map(|x| x.unwrap()).collect();
    let rate = a.engine.sample_rate;
    let n = samples.len() / 2;
    assert_eq!(n as f64, (4.0 * rate).round(), "8 beats at 120 BPM");
    let at = |f: usize| (samples[2 * f], samples[2 * f + 1]);
    let seam = n / 2;
    let (l0, r0) = at(100);
    assert!(l0 > 0.5 && r0 == 0.0, "take A before the seam: {l0} {r0}");
    let (l1, r1) = at(n - 100);
    assert!(r1 > 0.5 && l1 == 0.0, "take B after it: {l1} {r1}");
    assert!((l0 - r1).abs() < 1e-6);
    let x = (0.010 * rate) as usize;
    let mut crossed = 0;
    for f in seam - x..seam + x {
        let (l, r) = at(f);
        assert!(
            (l * l + r * r - l0 * l0).abs() < 1e-4,
            "frame {f}: equal power, {l} {r}"
        );
        crossed += usize::from(l > 0.0 && r > 0.0);
    }
    assert!(crossed > x / 2, "the seam crossfades ({crossed} frames)");
    a.undo();
    assert_eq!(a.project().tracks, before.tracks);
}

#[test]
fn grouped_tracks_comp_together() {
    let mut p = Project::default();
    let (kick, _, _) = takes(&mut p, "Kick");
    let (snare, _, _) = takes(&mut p, "Snare");
    let (solo, _, _) = takes(&mut p, "Solo");
    // Kick and snare on strips in one mixer group (a kit recorded together).
    let g = p.make_strip_group("Drums", Color::PALETTE[1]);
    let gid = g.id;
    p.strip_groups.push(g);
    for t in [kick, snare] {
        let s = p.track(t).unwrap().strip.unwrap();
        p.strip_mut(s).unwrap().group = Some(gid);
    }
    let mut a = app(p);
    let steps = a.bus.history().len();
    a.comp_swipe(kick, 1, beats(2), beats(4));
    assert_eq!(a.bus.history().len(), steps + 1, "one step for the group");
    let swiped = |a: &CantusApp, t| lane0(a, t).iter().any(|w| w.0 == 1920);
    assert!(swiped(&a, kick) && swiped(&a, snare));
    assert!(!swiped(&a, solo));
    // Off: a swipe is the one track's.
    a.toggle_comp_grouped();
    assert!(!a.project().recording.comp_grouped);
    a.comp_swipe(kick, 1, beats(5), beats(6));
    assert!(lane0(&a, kick).iter().any(|w| w.0 == 4800));
    assert!(!lane0(&a, snare).iter().any(|w| w.0 == 4800));
}

#[test]
fn quick_swipe_is_a_project_switch_one_step_each_way() {
    let mut a = app(Project::default());
    assert!(a.project().recording.quick_swipe);
    let steps = a.bus.history().len();
    a.toggle_quick_swipe();
    assert!(!a.project().recording.quick_swipe);
    assert_eq!(a.bus.history().len(), steps + 1);
    a.undo();
    assert!(a.project().recording.quick_swipe);
}

/// The real view: a frame of the arrangement with these events.
fn show(a: &mut CantusApp, ctx: &egui::Context, events: Vec<egui::Event>) {
    let input = egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(1400.0, 700.0),
        )),
        events,
        ..Default::default()
    };
    ctx.run_ui(input, |ui| {
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE)
            .show(ui, |ui| cantus_ui::views::arrangement::show(a, ui));
    })
    .textures_delta
    .clear();
}

/// Press at `from`, travel to `to`, let go — in the arrangement.
fn drag(a: &mut CantusApp, ctx: &egui::Context, from: egui::Pos2, to: egui::Pos2) {
    let button = |pos, pressed| egui::Event::PointerButton {
        pos,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: egui::Modifiers::NONE,
    };
    show(a, ctx, vec![egui::Event::PointerMoved(from)]);
    show(
        a,
        ctx,
        vec![egui::Event::PointerMoved(from), button(from, true)],
    );
    for i in 1..=4 {
        let at = from + (to - from) * (i as f32 / 4.0);
        show(a, ctx, vec![egui::Event::PointerMoved(at)]);
    }
    show(
        a,
        ctx,
        vec![egui::Event::PointerMoved(to), button(to, false)],
    );
    show(a, ctx, vec![]);
}

/// From the real arrangement: a swipe across a take lane comps it in, and the seam's
/// handle at the foot of the playing lane moves the seam, both sides.
#[test]
fn the_arrangement_swipes_and_moves_a_seam_by_hand() {
    let mut p = Project::default();
    let (t, a_src, _) = takes(&mut p, "Vox");
    let mut a = app(p);
    a.arrangement.snap = false;
    let ctx = egui::Context::default();
    cantus_ui::design::install_fonts(&ctx);
    cantus_ui::design::install_style(&ctx);
    show(&mut a, &ctx, vec![]);
    let geo = a.arrangement.geometry().expect("drawn");
    let row = geo.rows[0];
    // Take lane 1 sits under the playing lane.
    let lane1 = row.top + row.height + 6.0;
    drag(
        &mut a,
        &ctx,
        egui::pos2(geo.x(2.0), lane1),
        egui::pos2(geo.x(4.0), lane1),
    );
    let windows = lane0(&a, t);
    assert!(
        windows
            .iter()
            .any(|w| w.2 == a_src && w.0 >= 1900 && w.0 <= 1940),
        "{windows:?}"
    );
    let (_, _, at, _) = comping::seams(a.project().track(t).unwrap())[0];
    let x = geo.x(at.0 as f32 / PPQ as f32);
    let foot = row.top + row.height - 6.0;
    drag(
        &mut a,
        &ctx,
        egui::pos2(x, foot),
        egui::pos2(x + geo.px_per_beat, foot),
    );
    let (_, _, moved, _) = comping::seams(a.project().track(t).unwrap())[0];
    assert!(moved > at + Ticks(PPQ / 2), "{at:?} → {moved:?}");
}

/// The lane-0 window on take lane `lane`'s source starting at `at`.
fn window_at(a: &CantusApp, t: TrackId, at: Ticks) -> ClipId {
    a.project()
        .track(t)
        .unwrap()
        .clips
        .iter()
        .find(|c| c.lane == 0 && c.start == at)
        .unwrap()
        .id
}

/// Review 1: exploding the takes does not copy the track's alternate comps (their kept
/// windows' ids would be twice in the project): comps still switch on the track after.
#[test]
fn explode_leaves_the_comps_with_their_track() {
    let mut p = Project::default();
    let (t, _, _) = takes(&mut p, "Vox");
    let mut a = app(p);
    a.comp_swipe(t, 1, beats(2), beats(4));
    a.duplicate_comp(t);
    a.promote_take_lane(t, 2);
    let tracks = a.project().tracks.len();
    a.explode_takes(t);
    assert_eq!(a.project().tracks.len(), tracks + 2, "the takes exploded");
    assert!(a
        .project()
        .tracks
        .iter()
        .filter(|x| x.id != t)
        .all(|x| x.comps.is_empty()));
    let steps = a.bus.history().len();
    a.switch_comp(t, 0);
    assert_eq!(a.bus.history().len(), steps + 1, "the switch still applies");
}

/// Review 1: a comp window slipped (or moved) is still the comp's: switching away and
/// back brings it back whole.
#[test]
fn a_slipped_window_stays_in_its_comp() {
    let mut p = Project::default();
    let (t, _, _) = takes(&mut p, "Vox");
    let mut a = app(p);
    a.comp_swipe(t, 1, beats(2), beats(4));
    let w = window_at(&a, t, beats(2));
    let offset = a.project().track(t).unwrap().clip(w).unwrap().offset;
    a.exec(Command::SetClipOffset {
        track: t,
        clip: w,
        offset: offset + Ticks(12),
    });
    let comp_a: Vec<_> = {
        let mut v: Vec<_> = a
            .project()
            .track(t)
            .unwrap()
            .clips
            .iter()
            .filter(|c| c.lane == 0)
            .map(|c| (c.start, c.length, c.offset))
            .collect();
        v.sort();
        v
    };
    a.duplicate_comp(t);
    a.comp_swipe(t, 2, beats(2), beats(4));
    a.switch_comp(t, 0);
    let mut back: Vec<_> = a
        .project()
        .track(t)
        .unwrap()
        .clips
        .iter()
        .filter(|c| c.lane == 0)
        .map(|c| (c.start, c.length, c.offset))
        .collect();
    back.sort();
    assert_eq!(
        back, comp_a,
        "comp A comes back whole, the slipped window in it"
    );
}

/// Review 1: grouped comping covers the seams too — a seam moved, a seam's crossfade, the
/// comps switched and made — on every track of the group, each one undo step.
#[test]
fn a_group_moves_its_seams_and_switches_its_comps_together() {
    let mut p = Project::default();
    let (kick, _, _) = takes(&mut p, "Kick");
    let (snare, _, _) = takes(&mut p, "Snare");
    let g = p.make_strip_group("Drums", Color::PALETTE[1]);
    let gid = g.id;
    p.strip_groups.push(g);
    for t in [kick, snare] {
        let s = p.track(t).unwrap().strip.unwrap();
        p.strip_mut(s).unwrap().group = Some(gid);
    }
    let mut a = app(p);
    a.comp_swipe(kick, 1, beats(2), beats(4));
    let seam = |a: &CantusApp, t: TrackId| {
        comping::seams(a.project().track(t).unwrap())
            .into_iter()
            .map(|s| (s.2, s.3))
            .collect::<Vec<_>>()
    };
    assert_eq!(seam(&a, kick), seam(&a, snare));
    // The kick's seam at beat 2 dragged to 2.5: the snare's goes with it, one step.
    let (l, r, _, _) = comping::seams(a.project().track(kick).unwrap())[0];
    let steps = a.bus.history().len();
    let to = beats(2) + Ticks(PPQ / 2);
    assert!(a.move_comp_seam(kick, l, r, to, false));
    assert_eq!(a.bus.history().len(), steps + 1);
    assert_eq!(seam(&a, kick)[0].0, to);
    assert_eq!(seam(&a, snare), seam(&a, kick));
    // Its crossfade: both.
    let r = comping::seams(a.project().track(kick).unwrap())[0].1;
    a.set_seam_xfade(kick, Some(r), Ticks(40));
    assert_eq!(seam(&a, snare), seam(&a, kick));
    // A new comp and a switch: both tracks.
    a.duplicate_comp(kick);
    assert_eq!(a.project().track(snare).unwrap().comps.len(), 2);
    a.switch_comp(kick, 0);
    assert_eq!(a.project().track(snare).unwrap().active_comp, 0);
}

/// Review 1: a swipe that grows a window over several windows of the same take, each with
/// its own pitch edits, keeps them all — each over its own stretch.
#[test]
fn a_swipe_over_several_edited_windows_keeps_every_ones_edits() {
    let mut p = Project::default();
    let (t, _, _) = takes(&mut p, "Vox");
    let mut a = app(p);
    a.comp_swipe(t, 1, beats(0), beats(2));
    a.comp_swipe(t, 1, beats(4), beats(6));
    let first = edit();
    let mut second = edit();
    second.notes = vec![PitchNote::sung(0, 100_000, 130_000, 62.2, -10.0)];
    for (at, e) in [(beats(0), first), (beats(4), second)] {
        let w = window_at(&a, t, at);
        a.exec(Command::SetClipPitch {
            track: t,
            clip: w,
            pitch: Some(Box::new(e)),
            op: cantus_core::pitch::PitchOp::Notes,
        });
    }
    a.comp_swipe(t, 1, beats(0), beats(6));
    let grown = a
        .project()
        .track(t)
        .unwrap()
        .clips
        .iter()
        .find(|c| c.lane == 0 && c.start == beats(0))
        .unwrap();
    assert_eq!(grown.end(), beats(6));
    let notes = &grown.audio.as_ref().unwrap().pitch.as_ref().unwrap().notes;
    let spans: Vec<_> = notes.iter().map(|n| (n.src_start, n.src_end)).collect();
    assert_eq!(
        spans,
        vec![(1_000, 40_000), (100_000, 130_000)],
        "both edits"
    );
}

/// Review 1: *crossfade for every seam* is one undo step (the default and the seams).
#[test]
fn every_seams_crossfade_is_one_undo_step() {
    let mut p = Project::default();
    let (t, _, _) = takes(&mut p, "Vox");
    let mut a = app(p);
    a.comp_swipe(t, 1, beats(2), beats(4));
    let before = a.project().clone();
    let steps = a.bus.history().len();
    a.set_comp_xfade_default(t, 20.0);
    assert_eq!(a.bus.history().len(), steps + 1);
    assert_eq!(a.project().recording.comp_xfade_ms, 20.0);
    assert!(lane0(&a, t)
        .iter()
        .filter(|w| w.3 > 0)
        .all(|w| w.3 == comping::xfade_ticks(a.project()).0));
    a.undo();
    assert_eq!(a.project(), &before);
}

/// Review 1: at the most comps a track holds, *New comp* says so and does nothing.
#[test]
fn new_comp_at_the_limit_says_so() {
    let mut p = Project::default();
    let (t, _, _) = takes(&mut p, "Vox");
    let mut a = app(p);
    a.comp_swipe(t, 1, beats(2), beats(4));
    for _ in 0..cantus_core::comp::MAX_COMPS {
        a.duplicate_comp(t);
    }
    assert_eq!(
        a.project().track(t).unwrap().comps.len(),
        cantus_core::comp::MAX_COMPS
    );
    let steps = a.bus.history().len();
    a.duplicate_comp(t);
    assert_eq!(a.bus.history().len(), steps);
    assert!(
        a.status.error && a.status.text.contains("26"),
        "{}",
        a.status.text
    );
}

/// Review 2: a copy of a window pasted far from its takes (the good chorus again at bar
/// 9) plays the take's source, but it is no window of the comp: a switch leaves it on the
/// playing lane, and the merge's range ends where the comp's windows do.
#[test]
fn a_windows_copy_away_from_its_takes_is_not_the_comps() {
    let mut p = Project::default();
    let (t, _, _) = takes(&mut p, "Vox");
    let mut a = app(p);
    a.comp_swipe(t, 1, beats(2), beats(4));
    a.duplicate_comp(t);
    let w = window_at(&a, t, beats(2));
    let window = a.project().track(t).unwrap().clip(w).unwrap().clone();
    let mut copy = a.bus.new_clip("copy", beats(32), window.length);
    copy.audio = window.audio.clone();
    copy.offset = window.offset;
    a.exec(Command::InsertClip {
        track: t,
        clip: Box::new(copy),
    });
    let tr = a.project().track(t).unwrap();
    let windows = comping::comp_windows(tr);
    assert!(
        windows.iter().all(|c| c.start < beats(8)),
        "the far copy is no window: {:?}",
        windows.iter().map(|c| c.start).collect::<Vec<_>>()
    );
    a.switch_comp(t, 0);
    assert!(
        lane0(&a, t).iter().any(|c| c.0 == beats(32).0),
        "the copy stays on the playing lane: {:?}",
        lane0(&a, t)
    );
    a.switch_comp(t, 1);
    assert_eq!(
        lane0(&a, t).iter().filter(|c| c.0 == beats(32).0).count(),
        1,
        "and is not doubled coming back"
    );
}

/// Integration review: a comp window moved off its take (to bar 9) still plays that take:
/// *Delete unused takes* keeps the take.
#[test]
fn a_take_a_moved_window_still_plays_is_not_unused() {
    let mut p = Project::default();
    let (t, a_src, _) = takes(&mut p, "Vox");
    let mut a = app(p);
    a.comp_swipe(t, 1, beats(2), beats(4));
    let w = window_at(&a, t, beats(2));
    let length = a.project().track(t).unwrap().clip(w).unwrap().length;
    a.exec(Command::SetClipBounds {
        track: t,
        clip: w,
        start: beats(32),
        length,
    });
    a.delete_unused_takes(t);
    let tr = a.project().track(t).unwrap();
    assert!(
        tr.clips
            .iter()
            .any(|c| c.lane > 0 && c.audio.as_ref().is_some_and(|x| x.source == a_src)),
        "take A stays: a window still plays it"
    );
}

/// Integration review: a comp window in a link group (a linked copy on another track) is
/// kept by an alternate comp as its own clip — a later edit of the group does not make
/// the switch back refused.
#[test]
fn a_linked_window_kept_by_a_comp_comes_back_after_its_group_changed() {
    let mut p = Project::default();
    let (t, _, _) = takes(&mut p, "Vox");
    let link = LinkId(p.alloc_id());
    let other = p.make_track("Double");
    let oid = other.id;
    p.tracks.push(other);
    let window = {
        let w = p
            .track_mut(t)
            .unwrap()
            .clips
            .iter_mut()
            .find(|c| c.lane == 0)
            .unwrap();
        w.link = Some(link);
        w.clone()
    };
    let mut copy = window.clone();
    copy.id = ClipId(p.alloc_id());
    p.track_mut(oid).unwrap().clips.push(copy.clone());
    let mut a = app(p);
    a.duplicate_comp(t);
    a.rename_clip((oid, copy.id), "Double take".into());
    let comp_b = lane0(&a, t);
    let steps = a.bus.history().len();
    a.switch_comp(t, 0);
    assert_eq!(a.bus.history().len(), steps + 1, "the switch back took");
    a.switch_comp(t, 1);
    assert_eq!(lane0(&a, t), comp_b);
}
