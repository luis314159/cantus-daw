//! Review 3 of the tempo track in the application (ADR 0060): several windows moved,
//! nudged, copied or pasted across a tempo change still meet; a linked paste stays linked;
//! a loop keeps its ticks; the drag draws what lands; a slip follows the pointer; a
//! reversed waveform is what plays; a tempo edit keeps what plays at a clip's start.

use cantus_core::audio_time;
use cantus_core::tempo::TempoPoint;
use cantus_core::*;
use cantus_engine::EngineHandle;
use cantus_ui::app::Files;
use cantus_ui::keymap::Action;
use cantus_ui::views::arrangement::ClipDragResult;
use cantus_ui::CantusApp;

const RATE: f64 = 48_000.0;

fn pt(beat: i64, bpm: f64) -> TempoPoint {
    TempoPoint {
        tick: Ticks::beats(beat),
        bpm,
        ramp: false,
    }
}

/// The app as the desktop assembles it, on `project`.
fn assembled(project: Project) -> CantusApp {
    let data = tempfile::tempdir().unwrap();
    std::env::set_var("CANTUS_DATA_DIR", data.path());
    let files = Files::read(data.path(), Some(cantus_ui::i18n::Locale::En));
    let ctx = egui::Context::default();
    let mut app = CantusApp::assemble(
        &ctx,
        files,
        None,
        EngineHandle::dummy(),
        None,
        None,
        None,
        false,
    );
    app.exec(Command::ReplaceProject {
        project: Box::new(project),
    });
    app
}

/// 120 BPM to beat 8, then 60; one audio track whose windows are `(start, offset, length)`
/// beats on one 48 kHz take (slipped 37 frames).
fn song(windows: &[(i64, i64, i64)]) -> (CantusApp, TrackId, Vec<ClipId>) {
    let mut p = Project::new("song");
    p.tempo_points = vec![pt(0, 120.0), pt(8, 60.0)];
    let mut t = p.make_track("Audio");
    let id = AudioSourceId(p.alloc_id());
    p.audio_pool.push(AudioSource {
        id,
        name: "take".into(),
        path: "C:/take.wav".into(),
        sample_rate: RATE as u32,
        channels: 1,
        frames: 4_800_000,
        sha256: None,
        recorded: true,
    });
    let mut ids = Vec::new();
    for &(start, offset, length) in windows {
        let mut c = p.make_audio_clip("take", id, Ticks::beats(start), Ticks::beats(length));
        c.offset = Ticks::beats(offset);
        c.audio.as_mut().unwrap().slip_frames = 37;
        ids.push(c.id);
        t.clips.push(c);
    }
    let tid = t.id;
    p.tracks.push(t);
    (assembled(p), tid, ids)
}

fn clips(a: &CantusApp, t: TrackId) -> Vec<Clip> {
    let mut v = a.project().track(t).unwrap().clips.clone();
    v.sort_by_key(|c| c.start);
    v
}

fn frame(map: &TempoMap, c: &Clip, tick: f64) -> f64 {
    let a = c.audio.as_ref().unwrap();
    audio_time::frame_at(map, c.start, c.offset, f64::from(a.slip_frames), RATE, tick)
}

/// The windows that met still meet and play on as one take: the end of each is the start
/// of the next, on the timeline and in the file (within a frame).
fn meeting(a: &CantusApp, w: &[Clip]) {
    let map = a.project().tempo_map();
    for p in w.windows(2) {
        assert_eq!(
            p[0].end(),
            p[1].start,
            "{:?} and {:?} meet",
            p[0].start,
            p[1].start
        );
        let (x, y) = (
            frame(&map, &p[0], p[0].end().0 as f64),
            frame(&map, &p[1], p[1].start.0 as f64),
        );
        assert!((x - y).abs() <= 1.0, "the file goes on: {x} then {y}");
    }
}

/// Findings 2, 8, 17: a take split in two, the halves dragged, nudged and drag-copied
/// together across the tempo change, still meet and glue back.
#[test]
fn halves_moved_together_across_a_change_still_meet() {
    let (mut a, t, ids) = song(&[(0, 0, 4), (4, 4, 4)]);
    let sel: Vec<_> = ids.iter().map(|c| (t, *c)).collect();
    a.set_clip_selection(sel.clone());
    a.focus_arrangement();
    a.commit_clip_drag(
        sel[0],
        ClipDragResult::Move {
            delta: Ticks::beats(8),
            rows: 0,
            copy: false,
        },
    );
    let moved = clips(&a, t);
    assert_eq!(
        moved[0].start,
        Ticks::beats(8),
        "the block's anchor moves by the delta"
    );
    meeting(&a, &moved);
    // Glue gives one clip back.
    a.set_clip_selection(moved.iter().map(|c| (t, c.id)).collect());
    a.run_action(Action::GlueClips);
    assert_eq!(clips(&a, t).len(), 1, "glued");
    a.undo();
    a.undo();
    // A nudge by a bar and a drag-copy: the same.
    a.set_clip_selection(sel.clone());
    a.focus_arrangement();
    a.run_action(Action::NudgeRightBar);
    meeting(&a, &clips(&a, t));
    a.undo();
    a.set_clip_selection(sel.clone());
    a.commit_clip_drag(
        sel[0],
        ClipDragResult::Move {
            delta: Ticks::beats(8),
            rows: 0,
            copy: true,
        },
    );
    let copies: Vec<Clip> = clips(&a, t)
        .into_iter()
        .filter(|c| !ids.contains(&c.id))
        .collect();
    assert_eq!(copies.len(), 2);
    meeting(&a, &copies);
}

/// Findings 2, 8: the halves copied and pasted across the change still meet.
#[test]
fn halves_pasted_across_a_change_still_meet() {
    let (mut a, t, ids) = song(&[(0, 0, 4), (4, 4, 4)]);
    a.set_clip_selection(ids.iter().map(|c| (t, *c)).collect());
    a.focus_arrangement();
    a.run_action(Action::Copy);
    a.seek(Ticks::beats(8));
    a.paste_now();
    let pasted: Vec<Clip> = clips(&a, t)
        .into_iter()
        .filter(|c| !ids.contains(&c.id))
        .collect();
    assert_eq!(pasted.len(), 2);
    assert_eq!(pasted[0].start, Ticks::beats(8));
    meeting(&a, &pasted);
}

/// Finding 3: with linked copies on, a paste across the tempo change stays in its group.
#[test]
fn a_linked_paste_across_a_change_stays_linked() {
    let (mut a, t, ids) = song(&[(2, 0, 2)]);
    a.settings.link_copies = true;
    a.set_clip_selection(vec![(t, ids[0])]);
    a.focus_arrangement();
    a.run_action(Action::Copy);
    a.seek(Ticks::beats(12));
    a.paste_now();
    let all = clips(&a, t);
    assert_eq!(all.len(), 2);
    assert!(
        all[0].link.is_some() && all[0].link == all[1].link,
        "{:?}",
        all
    );
}

/// Finding 6: a looping audio clip moved across the change keeps its ticks.
#[test]
fn a_looping_clip_keeps_its_ticks() {
    let (mut a, t, ids) = song(&[(2, 0, 4)]);
    a.exec(Command::SetClipLoop {
        track: t,
        clip: ids[0],
        content_length: Some(Ticks::beats(1)),
    });
    a.set_clip_selection(vec![(t, ids[0])]);
    a.commit_clip_drag(
        (t, ids[0]),
        ClipDragResult::Move {
            delta: Ticks::beats(8),
            rows: 0,
            copy: false,
        },
    );
    let c = &clips(&a, t)[0];
    assert_eq!(
        (c.start, c.length, c.content_length),
        (Ticks::beats(10), Ticks::beats(4), Some(Ticks::beats(1)))
    );
}

/// Findings 18, 24: what a drag draws is what its release leaves.
#[test]
fn the_drag_draws_what_lands() {
    let (mut a, t, ids) = song(&[(2, 1, 4), (6, 5, 2)]);
    let sel: Vec<_> = ids.iter().map(|c| (t, *c)).collect();
    a.set_clip_selection(sel.clone());
    let result = ClipDragResult::Move {
        delta: Ticks::beats(8),
        rows: 0,
        copy: false,
    };
    let preview = cantus_ui::clips::drag_preview(a.project(), &sel, sel[0], result);
    a.commit_clip_drag(sel[0], result);
    for (r, c) in preview {
        let landed = a.project().track(r.0).unwrap().clip(r.1).unwrap();
        assert_eq!(
            (c.start, c.length, c.offset, &c.audio),
            (landed.start, landed.length, landed.offset, &landed.audio)
        );
    }
}

/// Finding 36: a slip by a beat moves the source under the window's start by a beat's
/// seconds there, whatever the tempo over its hidden part.
#[test]
fn a_slip_follows_the_pointer_at_the_window() {
    // The window at beat 10 (60 BPM), its hidden part back to beat 0 (mostly 120).
    let (mut a, t, ids) = song(&[(10, 10, 2)]);
    let map = a.project().tempo_map();
    let before = frame(&map, &clips(&a, t)[0], Ticks::beats(10).0 as f64);
    a.commit_clip_drag(
        (t, ids[0]),
        ClipDragResult::Slip {
            delta: Ticks::beats(1),
        },
    );
    let after = frame(&map, &clips(&a, t)[0], Ticks::beats(10).0 as f64);
    // Content one beat later: one second (a beat at 60) earlier in the file there.
    assert!((before - after - RATE).abs() <= 1.0, "{before} -> {after}");
}

/// Findings 4, 42: a reversed window's frame, as drawn, is what the engine and the pitch
/// editor read: the start's frame plus the seconds from the tick to the end.
#[test]
fn a_reversed_window_is_drawn_as_it_plays() {
    let (a, t, _) = song(&[(4, 4, 8)]);
    let c = clips(&a, t)[0].clone();
    let map = a.project().tempo_map();
    let ct = cantus_ui::pitch::ClipTime {
        start: c.start,
        offset: c.offset,
        slip: 37,
        rate: RATE as u32,
        map: std::sync::Arc::new(map.clone()),
        warp: None,
        reverse: true,
        end: c.end(),
    };
    for beat in [4.5, 6.0, 8.0, 10.0, 11.5] {
        let tick = beat * 960.0;
        let drawn = cantus_ui::views::arrangement::reversed_frame(
            &map,
            c.start,
            c.offset,
            37.0,
            RATE,
            c.end().0 as f64,
            tick,
        );
        let read = ct.frame(Ticks(tick as i64));
        assert!(
            (drawn - read).abs() < 1e-6,
            "beat {beat}: {drawn} vs {read}"
        );
    }
}

/// Findings 0, 13, 37 through the app: the top bar's tempo changed at bar 1 keeps what the
/// take at beat 10 plays at its start; one undo gives everything back.
#[test]
fn a_tempo_edit_keeps_what_a_clip_plays_at_its_start() {
    let (mut a, t, _) = song(&[(10, 10, 2)]);
    let before_project = a.project().clone();
    let at = |a: &CantusApp| {
        let map = a.project().tempo_map();
        let c = &clips(a, t)[0];
        frame(&map, c, c.start.0 as f64)
    };
    let before = at(&a);
    a.seek(Ticks::ZERO);
    a.set_tempo_now(100.0, "tempo");
    assert!((at(&a) - before).abs() <= 0.5);
    a.undo();
    assert_eq!(*a.project(), before_project);
}
