//! Review 2 of the tempo track (ADR 0060): an unwarped audio clip's edits in the application
//! under a tempo track — a trim, a reversed trim, a split glued back, a move and a
//! drag-copy (the same clip a paste makes), a duplicate — play the frames they played;
//! copy and paste through the app keep a clip inside one tempo exactly (finding 20); a bar
//! nudge is the bar the clip is in (47); an export between markers never falls back to the
//! whole song (17, 36, 49).

use cantus_core::audio_time;
use cantus_core::tempo::{MetreChange, TempoPoint};
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

/// 120 BPM to beat 8, then 60; one track with an audio clip over beats `from`..`to`, a beat
/// into its 48 kHz file, slipped 37 frames (reversed with `reverse`).
fn song(from: i64, to: i64, reverse: bool) -> (CantusApp, (TrackId, ClipId)) {
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
    let mut c = p.make_audio_clip("take", id, Ticks::beats(from), Ticks::beats(to - from));
    c.offset = Ticks::beats(1);
    let a = c.audio.as_mut().unwrap();
    a.slip_frames = 37;
    a.reverse = reverse;
    let r = (t.id, c.id);
    t.clips.push(c);
    p.tracks.push(t);
    (assembled(p), r)
}

fn clip(a: &CantusApp, r: (TrackId, ClipId)) -> Clip {
    a.project().track(r.0).unwrap().clip(r.1).unwrap().clone()
}

/// The source frame a clip plays at `tick`, as the engine plays it.
fn frame(map: &TempoMap, c: &Clip, tick: f64) -> f64 {
    let a = c.audio.as_ref().unwrap();
    let slip = a.slip_frames as f64;
    if a.reverse {
        let first = audio_time::frame_at(map, c.start, c.offset, slip, RATE, c.start.0 as f64);
        first + map.seconds_between_f64(tick, c.end().0 as f64) * RATE
    } else {
        audio_time::frame_at(map, c.start, c.offset, slip, RATE, tick)
    }
}

fn same_frames(map: &TempoMap, whole: &Clip, piece: &Clip) {
    let (a, b) = (piece.start.0 as f64, piece.end().0 as f64);
    for t in [a, 0.5 * (a + b), b - 1.0] {
        let (w, p) = (frame(map, whole, t), frame(map, piece, t));
        assert!(
            (w - p).abs() <= 1.0,
            "at tick {t}: {w} before the edit, {p} after"
        );
    }
}

#[test]
fn a_trim_across_a_tempo_change_keeps_what_plays() {
    for reverse in [false, true] {
        let (mut a, r) = song(0, 16, reverse);
        let map = a.project().tempo_map();
        let whole = clip(&a, r);
        a.commit_clip_drag(
            r,
            ClipDragResult::TrimStart {
                delta: Ticks::beats(10),
            },
        );
        same_frames(&map, &whole, &clip(&a, r));
        if !reverse {
            // Forward: the origin and the fine offset exactly (review 3, finding 19).
            let c = clip(&a, r);
            assert_eq!(c.start - c.offset, whole.start - whole.offset);
            assert_eq!(c.audio.as_ref().unwrap().slip_frames, 37);
        }
        a.undo();
        assert_eq!(clip(&a, r), whole);
        a.commit_clip_drag(
            r,
            ClipDragResult::TrimEnd {
                delta: -Ticks::beats(3),
            },
        );
        same_frames(&map, &whole, &clip(&a, r));
    }
}

#[test]
fn a_split_across_a_tempo_change_glues_back_to_the_clip() {
    for reverse in [false, true] {
        let (mut a, r) = song(0, 16, reverse);
        let map = a.project().tempo_map();
        let whole = clip(&a, r);
        a.set_clip_selection(vec![r]);
        a.seek(Ticks::beats(11));
        a.split_clips_at_playhead();
        let pieces = a.project().tracks[0].clips.clone();
        assert_eq!(pieces.len(), 2);
        for p in &pieces {
            same_frames(&map, &whole, p);
        }
        let all: Vec<_> = pieces.iter().map(|c| (r.0, c.id)).collect();
        a.set_clip_selection(all);
        a.focus_arrangement();
        a.run_action(Action::GlueClips);
        let back = &a.project().tracks[0].clips;
        assert_eq!(back.len(), 1, "glued");
        same_frames(&map, &whole, &back[0]);
        if !reverse {
            assert_eq!(back[0], whole, "the split glued back is the clip");
        }
    }
}

/// Finding 3: a move and a drag-copy across the tempo change give the clip a paste to the
/// same tick gives — the same seconds of its file (to a bar line: a paste lands on the
/// grid).
#[test]
fn a_move_a_drag_copy_and_a_paste_to_one_tick_agree() {
    let (mut a, r) = song(2, 6, false);
    let whole = clip(&a, r);
    a.set_clip_selection(vec![r]);
    a.focus_arrangement();
    a.run_action(Action::Copy);
    a.seek(Ticks::beats(12));
    a.paste_now();
    let pasted = a.project().tracks[0]
        .clips
        .iter()
        .find(|c| c.start == Ticks::beats(12))
        .cloned()
        .unwrap_or_else(|| {
            panic!(
                "pasted at beat 12: {:?} {:?}",
                a.project()
                    .tracks
                    .iter()
                    .map(|t| t.clips.iter().map(|c| c.start).collect::<Vec<_>>())
                    .collect::<Vec<_>>(),
                a.status.text
            )
        });
    a.undo();
    a.set_clip_selection(vec![r]);
    a.commit_clip_drag(
        r,
        ClipDragResult::Move {
            delta: Ticks::beats(10),
            rows: 0,
            copy: true,
        },
    );
    let copied = a.project().tracks[0]
        .clips
        .iter()
        .find(|c| c.start == Ticks::beats(12))
        .cloned()
        .expect("drag-copied to beat 12");
    let view = |c: &Clip| {
        (
            c.offset,
            c.length,
            c.audio
                .clone()
                .map(|a| (a.slip_frames, a.fade_in, a.fade_out)),
        )
    };
    assert_eq!(view(&copied), view(&pasted));
    a.undo();
    a.set_clip_selection(vec![r]);
    a.commit_clip_drag(
        r,
        ClipDragResult::Move {
            delta: Ticks::beats(10),
            rows: 0,
            copy: false,
        },
    );
    assert_eq!(view(&clip(&a, r)), view(&pasted));
    // The same seconds of the file at its start: a beat at 120 in, 37 frames slipped.
    let map = a.project().tempo_map();
    let moved = clip(&a, r);
    let at = frame(&map, &moved, moved.start.0 as f64);
    let was = frame(&map, &whole, whole.start.0 as f64);
    assert!((at - was).abs() <= 1.0, "{at} vs {was}");
}

/// A duplicate after itself, across the tempo change, plays its file as the original does.
#[test]
fn a_duplicate_across_a_tempo_change_plays_the_same_seconds() {
    let (mut a, r) = song(4, 8, false);
    let whole = clip(&a, r);
    a.set_clip_selection(vec![r]);
    a.focus_arrangement();
    a.run_action(Action::Duplicate);
    let map = a.project().tempo_map();
    let copy = a.project().tracks[0]
        .clips
        .iter()
        .find(|c| c.id != r.1)
        .cloned()
        .unwrap();
    assert_eq!(copy.start, whole.end());
    let secs = |c: &Clip| map.seconds_between(c.start, c.end());
    // Within half a tick at the slower tempo (review 3, finding 20).
    let half_tick = 0.5 * 60.0 / (60.0 * 960.0);
    assert!(
        (secs(&copy) - secs(&whole)).abs() <= half_tick,
        "{} vs {}",
        secs(&copy),
        secs(&whole)
    );
    let (f0, f1) = (
        frame(&map, &copy, copy.start.0 as f64),
        frame(&map, &whole, whole.start.0 as f64),
    );
    assert!((f0 - f1).abs() <= 1.0);
}

/// Finding 20: copy and paste through the app inside one held tempo keep an audio clip's
/// offset, length and fades exactly — a clip copy and a time-range copy.
#[test]
fn copy_and_paste_inside_one_tempo_keep_the_clip() {
    let (mut a, r) = song(10, 12, false);
    {
        let mut c = clip(&a, r);
        c.audio.as_mut().unwrap().fade_in = Ticks(240);
        c.audio.as_mut().unwrap().fade_out = Ticks(480);
        a.exec(Command::SetClipAudio {
            track: r.0,
            clip: r.1,
            audio: c.audio,
        });
    }
    let whole = clip(&a, r);
    let view = |c: &Clip| {
        (
            c.offset,
            c.length,
            c.audio
                .clone()
                .map(|a| (a.slip_frames, a.fade_in, a.fade_out)),
        )
    };
    a.set_clip_selection(vec![r]);
    a.focus_arrangement();
    a.run_action(Action::Copy);
    assert_eq!(
        a.clipboard.board.as_ref().unwrap().at,
        Ticks::beats(10),
        "a clip copy's origin"
    );
    a.seek(Ticks::beats(16));
    a.paste_now();
    let pasted = a.project().tracks[0]
        .clips
        .iter()
        .find(|c| c.start == Ticks::beats(16))
        .cloned()
        .unwrap();
    assert_eq!(view(&pasted), view(&whole));
    a.undo();
    a.set_clip_selection(Vec::new());
    a.set_time_selection(Ticks::beats(10), Ticks::beats(12));
    a.focus_arrangement();
    a.run_action(Action::Copy);
    // The copy says where it was taken (the range's start): a paste measures the clip's
    // seconds there.
    let board = a.clipboard.board.clone().expect("copied");
    assert_eq!(board.at, Ticks::beats(10), "a time-range copy's origin");
}

/// Finding 47: a bar nudge moves a clip by the bar it is in, not the bar at the playhead.
#[test]
fn a_bar_nudge_is_the_bar_the_clip_is_in() {
    let mut p = Project::new("nudge");
    p.metre_changes = vec![
        MetreChange {
            tick: Ticks::ZERO,
            signature: TimeSignature::new(4, 4),
        },
        MetreChange {
            tick: Ticks(30_720),
            signature: TimeSignature::new(7, 8),
        },
    ];
    let mut t = p.make_track("Keys");
    let c = p.make_clip("c", Ticks(3840), Ticks::beats(4));
    let r = (t.id, c.id);
    t.clips.push(c);
    p.tracks.push(t);
    let mut a = assembled(p);
    a.seek(Ticks(31_000));
    a.set_clip_selection(vec![r]);
    a.focus_arrangement();
    a.run_action(Action::NudgeRightBar);
    assert_eq!(clip(&a, r).start, Ticks(7680), "a bar of 4/4, not of 7/8");
}

/// Findings 17, 36 and 49: an export between one marker and itself (or a marker gone) is
/// refused with a word, never turned into the whole song.
#[test]
fn an_export_between_markers_never_falls_back_to_the_song() {
    let (mut a, _) = song(0, 4, false);
    let m = a.add_marker(Ticks::beats(2)).unwrap();
    let mut d = a.settings.export.clone();
    d.scope = cantus_export::Scope::Markers { from: m.0, to: m.0 };
    let dir = tempfile::tempdir().unwrap();
    let started = cantus_ui::views::export::start_with(&mut a, &dir.path().join("x.wav"), Some(d));
    assert!(!started, "no export of the song in its place");
    assert!(!a.export.busy());
}

/// Review 3 (finding 20): a clip whose fades straddle the tempo change, moved, drag-copied
/// and duplicated where one tempo holds: its window and both fades last the seconds they
/// did, within half a tick there.
#[test]
fn moved_fades_and_lengths_keep_their_seconds() {
    let (mut a, r) = song(6, 10, false);
    // A linked copy shares its group's fades in ticks (ADR 0037): the copy here is free.
    a.settings.link_copies = false;
    {
        let mut c = clip(&a, r);
        let au = c.audio.as_mut().unwrap();
        au.fade_in = Ticks::beats(3);
        au.fade_out = Ticks::beats(3);
        a.exec(Command::SetClipAudio {
            track: r.0,
            clip: r.1,
            audio: c.audio,
        });
    }
    let whole = clip(&a, r);
    let map = a.project().tempo_map();
    let seconds = |c: &Clip| {
        let au = c.audio.as_ref().unwrap();
        (
            map.seconds_between(c.start, c.end()),
            map.seconds_between(c.start, c.start + au.fade_in),
            map.seconds_between(c.end() - au.fade_out, c.end()),
        )
    };
    let was = seconds(&whole);
    let half_tick = 0.5 * 60.0 / (60.0 * 960.0);
    let near = |a: (f64, f64, f64), b: (f64, f64, f64), what: &str| {
        for (x, y) in [(a.0, b.0), (a.1, b.1), (a.2, b.2)] {
            assert!((x - y).abs() <= half_tick + 1e-12, "{what}: {a:?} vs {b:?}");
        }
    };
    for copy in [false, true] {
        a.set_clip_selection(vec![r]);
        a.commit_clip_drag(
            r,
            ClipDragResult::Move {
                delta: Ticks::beats(8),
                rows: 0,
                copy,
            },
        );
        let moved = a.project().tracks[0]
            .clips
            .iter()
            .find(|c| c.start == Ticks::beats(14))
            .cloned()
            .expect("at beat 14");
        near(
            seconds(&moved),
            was,
            if copy { "drag-copy" } else { "move" },
        );
        a.undo();
    }
}
