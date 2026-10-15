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
    assert!(matches!(cmd, Command::SetCompSeam { at: x, .. } if x == at - Ticks(120)));
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

#[test]
fn flatten_and_merge_bounces_the_comp_into_one_clip() {
    let dir = tempfile::tempdir().unwrap();
    let mut p = Project::default();
    let (t, _, _) = takes(&mut p, "Vox");
    let mut a = app(p);
    a.path = Some(dir.path().join("song.cantus"));
    a.comp_swipe(t, 1, beats(2), beats(4));
    let before = a.project().clone();
    assert!(a.merge_comp(t));
    let tr = a.project().track(t).unwrap();
    assert_eq!(tr.clips.len(), 1, "one clip, no takes");
    let c = &tr.clips[0];
    assert_eq!((c.start, c.end()), (Ticks::ZERO, beats(8)));
    let src = a
        .project()
        .audio_source(c.audio.as_ref().unwrap().source)
        .unwrap();
    assert!(
        std::path::Path::new(&src.path).exists(),
        "the bounce is on disk"
    );
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
    let row = geo.rows[0].clone();
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
