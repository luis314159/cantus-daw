//! Review 4 of the tempo track in the application (ADR 0060 §4): clips moved, nudged,
//! drag-copied or pasted together across a tempo change keep their bars — only the windows
//! of one take keep their shape in seconds, and three of them at an uneven ratio still meet;
//! a linked paste of a trimmed clip with fades stays in its group; a cut and paste with
//! *Link copies* on plays the seconds it played; a reversed clip widened past its file's
//! start reveals silence and keeps what it showed.

use cantus_core::audio_time;
use cantus_core::tempo::TempoPoint;
use cantus_core::*;
use cantus_engine::EngineHandle;
use cantus_ui::app::Files;
use cantus_ui::keymap::Action;
use cantus_ui::views::arrangement::ClipDragResult;
use cantus_ui::CantusApp;

const RATE: f64 = 48_000.0;
const B: i64 = 960;

fn pt(beat: i64, bpm: f64) -> TempoPoint {
    TempoPoint {
        tick: Ticks::beats(beat),
        bpm,
        ramp: false,
    }
}

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

/// Tempo `points`; an audio track with windows `(start, length, offset)` in ticks of one
/// 48 kHz take (fine offset 37); a MIDI track with clips `(start, length)` in ticks.
fn song(
    points: Vec<TempoPoint>,
    windows: &[(i64, i64, i64)],
    midi: &[(i64, i64)],
) -> (CantusApp, TrackId, TrackId) {
    let mut p = Project::new("song");
    p.tempo_points = points;
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
    for &(start, length, offset) in windows {
        let mut c = p.make_audio_clip("take", id, Ticks(start), Ticks(length));
        c.offset = Ticks(offset);
        c.audio.as_mut().unwrap().slip_frames = 37;
        t.clips.push(c);
    }
    let mut m = p.make_track("Keys");
    for &(start, length) in midi {
        m.clips.push(p.make_clip("m", Ticks(start), Ticks(length)));
    }
    let (ta, tm) = (t.id, m.id);
    p.tracks.push(t);
    p.tracks.push(m);
    (assembled(p), ta, tm)
}

fn clips(a: &CantusApp, t: TrackId) -> Vec<Clip> {
    let mut v = a.project().track(t).unwrap().clips.clone();
    v.sort_by_key(|c| c.start);
    v
}

fn all(a: &CantusApp, t: TrackId) -> Vec<(TrackId, ClipId)> {
    clips(a, t).iter().map(|c| (t, c.id)).collect()
}

fn frame(map: &TempoMap, c: &Clip, tick: f64) -> f64 {
    let a = c.audio.as_ref().unwrap();
    audio_time::frame_at(map, c.start, c.offset, f64::from(a.slip_frames), RATE, tick)
}

/// The windows meet, play on to the frame and share one origin and fine offset.
fn one_take(a: &CantusApp, w: &[Clip]) {
    let map = a.project().tempo_map();
    for p in w.windows(2) {
        assert_eq!(p[0].end(), p[1].start, "they meet");
        assert_eq!(
            p[0].start - p[0].offset,
            p[1].start - p[1].offset,
            "one origin"
        );
        let (x, y) = (
            frame(&map, &p[0], p[0].end().0 as f64),
            frame(&map, &p[1], p[1].start.0 as f64),
        );
        assert_eq!(x, y, "the file goes on");
    }
}

fn drag(a: &mut CantusApp, sel: Vec<(TrackId, ClipId)>, delta: Ticks, copy: bool) {
    a.set_clip_selection(sel.clone());
    a.focus_arrangement();
    a.commit_clip_drag(
        sel[0],
        ClipDragResult::Move {
            delta,
            rows: 0,
            copy,
        },
    );
}

/// Findings 18, 25, 31, 40: an audio clip and a MIDI clip that started together, and an
/// audio clip on its own earlier, moved, drag-copied, nudged and pasted across the change:
/// each lands its delta later, on the bars it had.
#[test]
fn clips_that_are_not_one_take_keep_their_bars() {
    let pts = vec![pt(0, 120.0), pt(8, 60.0)];
    let (mut a, ta, tm) = song(
        pts.clone(),
        &[(0, 2 * B, 0), (4 * B, 2 * B, 7 * B)],
        &[(4 * B, 2 * B)],
    );
    let sel = |a: &CantusApp| {
        let mut s = all(a, ta);
        s.extend(all(a, tm));
        s
    };
    let starts = |a: &CantusApp| -> Vec<Ticks> {
        let mut v: Vec<Ticks> = clips(a, ta)
            .iter()
            .chain(&clips(a, tm))
            .map(|c| c.start)
            .collect();
        v.sort();
        v
    };
    let s = sel(&a);
    drag(&mut a, s, Ticks::beats(8), false);
    assert_eq!(
        starts(&a),
        vec![Ticks::beats(8), Ticks::beats(12), Ticks::beats(12)]
    );
    a.undo();
    let s = sel(&a);
    drag(&mut a, s, Ticks::beats(8), true);
    let copies: Vec<Ticks> = clips(&a, ta)
        .iter()
        .map(|c| c.start)
        .filter(|s| *s >= Ticks::beats(8))
        .collect();
    assert_eq!(copies, vec![Ticks::beats(8), Ticks::beats(12)]);
    a.undo();
    let s = sel(&a);
    a.set_clip_selection(s);
    a.focus_arrangement();
    a.run_action(Action::NudgeRightBar);
    assert_eq!(
        starts(&a),
        vec![Ticks::beats(4), Ticks::beats(8), Ticks::beats(8)]
    );
    a.undo();
    // Copied and pasted at beat 8.
    let (mut a, ta, tm) = song(
        pts,
        &[(0, 2 * B, 0), (4 * B, 2 * B, 7 * B)],
        &[(4 * B, 2 * B)],
    );
    let s = sel(&a);
    a.set_clip_selection(s);
    a.focus_arrangement();
    a.run_action(Action::Copy);
    a.seek(Ticks::beats(8));
    a.paste_now();
    let pasted: Vec<Ticks> = clips(&a, ta)
        .iter()
        .chain(&clips(&a, tm))
        .map(|c| c.start)
        .filter(|s| *s >= Ticks::beats(8))
        .collect();
    let mut pasted = pasted;
    pasted.sort();
    assert_eq!(
        pasted,
        vec![Ticks::beats(8), Ticks::beats(12), Ticks::beats(12)]
    );
}

/// Finding 3: a take cut in three, moved and pasted where the tempo is 100 (a ratio that
/// rounds): the windows still meet, play on and share their origin.
#[test]
fn three_windows_at_an_uneven_ratio_still_meet() {
    let pts = vec![pt(0, 120.0), pt(8, 100.0)];
    let w = [(0, 1000, 0), (1000, 1000, 1000), (2000, 1000, 2000)];
    let (mut a, ta, _) = song(pts.clone(), &w, &[]);
    let s = all(&a, ta);
    drag(&mut a, s, Ticks::beats(16), false);
    one_take(&a, &clips(&a, ta));
    a.undo();
    let s = all(&a, ta);
    drag(&mut a, s, Ticks::beats(16), true);
    let copies: Vec<Clip> = clips(&a, ta)
        .into_iter()
        .filter(|c| c.start >= Ticks::beats(16))
        .collect();
    assert_eq!(copies.len(), 3);
    one_take(&a, &copies);
    let (mut a, ta, _) = song(pts, &w, &[]);
    let s = all(&a, ta);
    a.set_clip_selection(s);
    a.focus_arrangement();
    a.run_action(Action::Copy);
    a.seek(Ticks::beats(16));
    a.paste_now();
    let pasted: Vec<Clip> = clips(&a, ta)
        .into_iter()
        .filter(|c| c.start >= Ticks::beats(16))
        .collect();
    assert_eq!(pasted.len(), 3);
    one_take(&a, &pasted);
}

/// Finding 28: a trimmed clip whose fades straddle the change, copied and pasted across it
/// with *Link copies* on, joins its group: the copy is linked, the group holds, and the
/// original is untouched.
#[test]
fn a_linked_paste_of_a_trimmed_clip_with_fades_stays_in_its_group() {
    let (mut a, ta, _) = song(
        vec![pt(0, 120.0), pt(8, 60.0)],
        &[(6 * B, 4 * B, 1001)],
        &[],
    );
    let id = clips(&a, ta)[0].id;
    let mut c = clips(&a, ta)[0].clone();
    let au = c.audio.as_mut().unwrap();
    au.fade_in = Ticks::beats(3);
    au.fade_out = Ticks::beats(3);
    a.exec(Command::SetClipAudio {
        track: ta,
        clip: id,
        audio: c.audio,
    });
    let original = clips(&a, ta)[0].audio.clone();
    a.settings.link_copies = true;
    a.set_clip_selection(vec![(ta, id)]);
    a.focus_arrangement();
    a.run_action(Action::Copy);
    a.seek(Ticks::beats(14));
    a.paste_now();
    let both = clips(&a, ta);
    assert_eq!(both.len(), 2);
    assert!(
        both[1].link.is_some() && both[0].link == both[1].link,
        "linked"
    );
    cantus_core::links::check(a.project()).expect("the group holds");
    assert_eq!(both[0].audio, original, "the original untouched");
}

/// Finding 5: cut and pasted across the change with *Link copies* on (no group to join),
/// the clip plays the seconds it played: its frame at its start and its fades' seconds.
#[test]
fn a_cut_and_paste_with_link_copies_on_plays_what_it_played() {
    let (mut a, ta, _) = song(
        vec![pt(0, 120.0), pt(8, 60.0)],
        &[(2 * B, 2 * B, 1001)],
        &[],
    );
    let id = clips(&a, ta)[0].id;
    let mut c = clips(&a, ta)[0].clone();
    c.audio.as_mut().unwrap().fade_in = Ticks(B);
    a.exec(Command::SetClipAudio {
        track: ta,
        clip: id,
        audio: c.audio,
    });
    let map = a.project().tempo_map();
    let was = clips(&a, ta)[0].clone();
    let at_start = frame(&map, &was, was.start.0 as f64);
    let fade_s = map.seconds_between(was.start, was.start + Ticks(B));
    a.settings.link_copies = true;
    a.set_clip_selection(vec![(ta, id)]);
    a.focus_arrangement();
    a.run_action(Action::Cut);
    a.seek(Ticks::beats(12));
    a.paste_now();
    let now = clips(&a, ta)[0].clone();
    assert!(now.link.is_none());
    assert!(
        (frame(&map, &now, now.start.0 as f64) - at_start).abs() <= 0.5,
        "its frame"
    );
    let fade = map.seconds_between(now.start, now.start + now.audio.as_ref().unwrap().fade_in);
    assert!(
        (fade - fade_s).abs() <= 0.5 * 60.0 / (60.0 * 960.0) + 1e-9,
        "its fade's seconds"
    );
}

/// Finding 6: a reversed clip under a tempo track widened at its right edge as far as its
/// file allows keeps what it showed (its offset may go below zero: the fine offset says the
/// rest).
#[test]
fn a_reversed_clip_widened_keeps_what_it_showed() {
    // Hidden part at 120 (beats 0–2), the window at beats 2–4, 60 from beat 4.
    let (mut a, ta, _) = song(
        vec![pt(0, 120.0), pt(4, 60.0)],
        &[(2 * B, 2 * B, 2 * B)],
        &[],
    );
    let id = clips(&a, ta)[0].id;
    let mut c = clips(&a, ta)[0].clone();
    c.audio.as_mut().unwrap().reverse = true;
    a.exec(Command::SetClipAudio {
        track: ta,
        clip: id,
        audio: c.audio,
    });
    let map = a.project().tempo_map();
    let reversed = |c: &Clip, tick: f64| {
        let first = frame(&map, c, c.start.0 as f64);
        first + map.seconds_between_f64(tick, c.end().0 as f64) * RATE
    };
    let was = clips(&a, ta)[0].clone();
    let shown = reversed(&was, 3.0 * B as f64);
    let mut sel = vec![(ta, id)];
    a.set_clip_selection(std::mem::take(&mut sel));
    a.commit_clip_drag(
        (ta, id),
        ClipDragResult::TrimEnd {
            delta: Ticks::beats(2),
        },
    );
    let now = clips(&a, ta)[0].clone();
    assert_eq!(now.end(), Ticks::beats(6), "widened");
    assert!(
        (reversed(&now, 3.0 * B as f64) - shown).abs() <= 0.5,
        "what it showed stays"
    );
}

/// Findings 7, 22, 35: the drag's landing is built from the selection's unwarped audio
/// alone (the MIDI clips land at their start plus the delta, as drawn without it), and it
/// is what the release leaves.
#[test]
fn the_drag_preview_is_the_audio_alone_and_what_lands() {
    let midi: Vec<(i64, i64)> = (0..400).map(|i| (i * B / 2, B / 4)).collect();
    let w = [(0, 1000, 0), (1000, 1000, 1000)];
    let (mut a, ta, tm) = song(vec![pt(0, 120.0), pt(8, 100.0)], &w, &midi);
    let mut sel = all(&a, ta);
    sel.extend(all(&a, tm));
    a.set_clip_selection(sel.clone());
    let result = ClipDragResult::Move {
        delta: Ticks::beats(16),
        rows: 0,
        copy: false,
    };
    let preview = cantus_ui::clips::drag_preview(a.project(), &sel, sel[0], result);
    assert_eq!(preview.len(), 2, "the two windows, no MIDI clip");
    a.commit_clip_drag(sel[0], result);
    for (r, c) in preview {
        let landed = a.project().track(r.0).unwrap().clip(r.1).unwrap();
        assert_eq!(
            (c.start, c.length, c.offset, &c.audio),
            (landed.start, landed.length, landed.offset, &landed.audio)
        );
    }
}

/// Review 5 (finding 11): a take moved, drag-copied or pasted across the change keeps its
/// shape in seconds — its first window at its start plus the delta, every window playing
/// the frames it played at both its edges (within the half tick an edge on whole ticks
/// allows) — and a take with a gap keeps the gap's seconds.
#[test]
fn a_moved_take_keeps_its_shape_in_seconds() {
    let pts = vec![pt(0, 120.0), pt(8, 100.0)];
    for w in [
        vec![(0, 1000, 0), (1000, 1000, 1000), (2000, 1000, 2000)],
        vec![(0, 1000, 0), (3000, 1000, 3000)],
    ] {
        for how in 0..3 {
            let (mut a, ta, _) = song(pts.clone(), &w, &[]);
            let before_map = a.project().tempo_map();
            let was = clips(&a, ta);
            let delta = Ticks::beats(16);
            match how {
                0 | 1 => {
                    let s = all(&a, ta);
                    drag(&mut a, s, delta, how == 1);
                }
                _ => {
                    let s = all(&a, ta);
                    a.set_clip_selection(s);
                    a.focus_arrangement();
                    a.run_action(Action::Copy);
                    a.seek(delta);
                    a.paste_now();
                }
            }
            let map = a.project().tempo_map();
            let landed: Vec<Clip> = clips(&a, ta)
                .into_iter()
                .filter(|c| c.start >= delta)
                .collect();
            assert_eq!(landed.len(), was.len(), "{how}");
            assert_eq!(
                landed[0].start,
                was[0].start + delta,
                "{how}: the first by the delta"
            );
            for (o, n) in was.iter().zip(&landed) {
                for (to, tn) in [(o.start, n.start), (o.end(), n.end())] {
                    let (x, y) = (
                        frame(&before_map, o, to.0 as f64),
                        frame(&map, n, tn.0 as f64),
                    );
                    // Edges lie on whole ticks: within half a tick there (and half a frame).
                    let half_tick = 0.5 * 60.0 / (100.0 * 960.0) * RATE;
                    assert!(
                        (x - y).abs() <= half_tick + 0.5,
                        "{how}: {x} became {y} at an edge"
                    );
                }
            }
            // The seconds between the windows' starts, a gap's included.
            for k in 1..was.len() {
                let s0 = before_map.seconds_between(was[0].start, was[k].start);
                let s1 = map.seconds_between(landed[0].start, landed[k].start);
                assert!(
                    (s0 - s1).abs() <= 0.5 * 60.0 / (100.0 * 960.0) + 1e-9,
                    "{how}: {s0} vs {s1}"
                );
            }
        }
    }
}

/// Review 6 (findings 3, 6): a take with a linked piece that a tempo edit moved by whole
/// ticks — met or with a gap — moved and pasted across the change still keeps its shape:
/// the seams play on and a gap keeps its seconds (within the linked piece's half tick and
/// a frame).
#[test]
fn a_take_with_a_linked_piece_moves_as_one() {
    let pts = vec![pt(0, 120.0), pt(8, 100.0)];
    for (w, linked) in [
        // A pre-roll before the first window, so the edit re-anchors them.
        (
            vec![(1500, 1000, 1500), (2500, 1000, 2500), (3500, 1000, 3500)],
            1usize,
        ),
        (vec![(1500, 1000, 1500), (4500, 1000, 4500)], 1),
    ] {
        for paste in [false, true] {
            let (mut a, ta, _) = song(pts.clone(), &w, &[]);
            let ids: Vec<ClipId> = clips(&a, ta).iter().map(|c| c.id).collect();
            a.exec(Command::SetClipLink {
                track: ta,
                clip: ids[linked],
                link: Some(LinkId(9_999)),
            });
            a.exec(Command::SetTempoPoint {
                index: 0,
                point: TempoPoint {
                    tick: Ticks::ZERO,
                    bpm: 121.3,
                    ramp: false,
                },
            });
            let before_map = a.project().tempo_map();
            let was = clips(&a, ta);
            let delta = Ticks::beats(16);
            let s = all(&a, ta);
            if paste {
                a.settings.link_copies = false;
                a.set_clip_selection(s);
                a.focus_arrangement();
                a.run_action(Action::Copy);
                a.seek(delta);
                a.paste_now();
            } else {
                drag(&mut a, s, delta, false);
            }
            let map = a.project().tempo_map();
            let landed: Vec<Clip> = clips(&a, ta)
                .into_iter()
                .filter(|c| c.start >= delta)
                .collect();
            assert_eq!(landed.len(), was.len());
            let reach =
                0.5 * 60.0 / (100.0 * 960.0) * RATE + 0.5 * 60.0 / (121.3 * 960.0) * RATE + 1.0;
            for k in 1..was.len() {
                let s0 = before_map.seconds_between(was[0].start, was[k].start);
                let s1 = map.seconds_between(landed[0].start, landed[k].start);
                assert!(
                    (s0 - s1).abs() * RATE <= reach,
                    "paste {paste}: {s0} vs {s1}"
                );
                let (x, y) = (
                    frame(&map, &landed[k - 1], landed[k - 1].end().0 as f64),
                    frame(&map, &landed[k], landed[k].start.0 as f64),
                );
                if landed[k - 1].end() == landed[k].start {
                    assert!(
                        (x - y).abs() <= reach,
                        "paste {paste}: the seam {x} then {y}"
                    );
                }
            }
        }
    }
}
