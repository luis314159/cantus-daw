//! Cut, copy and paste (ADR 0036), driven the way the real window drives them: the whole
//! `CantusApp::frame` with the events egui-winit sends. On every desktop egui-winit turns
//! Ctrl+C, Ctrl+X and Ctrl+V into `Event::Copy`, `Event::Cut` and `Event::Paste(text)` and
//! sends **no** `Event::Key` for them — so a test that injected key events (as the old
//! ones did) passed while the keys did nothing in the app.

mod support;

use cantus_core::clipboard::{Clipboard, Payload, FORMAT};
use cantus_core::*;
use cantus_ui::app::EditFocus;
use cantus_ui::keymap::Action;
use cantus_ui::{starter_project, CantusApp};
use support::{app, ctx};

const W: f32 = 1400.0;
const H: f32 = 900.0;

/// One frame of the whole window with `events` (Ctrl held, as egui-winit reports it).
fn frame(a: &mut CantusApp, ctx: &egui::Context, events: Vec<egui::Event>) -> egui::FullOutput {
    frame_with(a, ctx, events, |_| {})
}

/// One frame, and `also` run inside it after the window — what a menu click does.
fn frame_with(
    a: &mut CantusApp,
    ctx: &egui::Context,
    events: Vec<egui::Event>,
    mut also: impl FnMut(&mut CantusApp),
) -> egui::FullOutput {
    let mut ev = vec![egui::Event::ModifiersChanged(egui::Modifiers::COMMAND)];
    ev.extend(events);
    let input = egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(W, H),
        )),
        events: ev,
        time: Some(ctx.input(|i| i.time) + 1.0 / 60.0),
        predicted_dt: 1.0 / 60.0,
        ..Default::default()
    };
    let mut out = ctx.run_ui(input, |ui| {
        a.frame(ui);
        also(a);
    });
    out.textures_delta.clear();
    out
}

/// What the frame wrote to the system clipboard, if anything.
fn copied(out: &egui::FullOutput) -> Option<String> {
    out.platform_output
        .commands
        .iter()
        .rev()
        .find_map(|c| match c {
            egui::OutputCommand::CopyText(t) => Some(t.clone()),
            _ => None,
        })
}

/// Whether the frame asked the window for the system clipboard.
fn asked_for_paste(out: &egui::FullOutput) -> bool {
    out.viewport_output
        .values()
        .any(|v| v.commands.contains(&egui::ViewportCommand::RequestPaste))
}

fn notes(a: &CantusApp) -> Vec<Note> {
    let (t, c) = (a.selected_track.unwrap(), a.selected_clip.unwrap());
    a.project().track(t).unwrap().clip(c).unwrap().notes.clone()
}

fn board(a: &CantusApp) -> &Clipboard {
    a.clipboard
        .board
        .as_ref()
        .expect("the app keeps what was copied")
}

fn all_clips(a: &CantusApp) -> Vec<(TrackId, Clip)> {
    a.project()
        .tracks
        .iter()
        .flat_map(|t| t.clips.iter().map(move |c| (t.id, c.clone())))
        .collect()
}

fn clip_of(a: &CantusApp, r: (TrackId, ClipId)) -> Clip {
    a.project().track(r.0).unwrap().clip(r.1).unwrap().clone()
}

fn normalised(p: &Project) -> Project {
    let mut p = p.clone();
    p.next_id = 0;
    p
}

/// The pitches heard at each tick, straight from the snapshot the engine would get.
fn heard(a: &CantusApp) -> Vec<(i64, u8)> {
    let s = Snapshot::compile(a.project(), 0);
    let mut v: Vec<(i64, u8)> = s
        .tracks
        .iter()
        .flat_map(|t| t.events.iter())
        .filter(|e| e.is_on())
        .map(|e| (e.tick.0, e.pitch))
        .collect();
    v.sort_unstable();
    v
}

#[test]
fn ctrl_c_arrives_as_an_event_and_copies_to_the_system_clipboard() {
    let mut a = app(starter_project());
    let ctx = ctx();
    a.focus_piano_roll();
    a.selected_notes = vec![notes(&a)[1].id, notes(&a)[2].id];
    let out = frame(&mut a, &ctx, vec![egui::Event::Copy]);
    let text = copied(&out).expect("Ctrl+C wrote nothing to the system clipboard");
    assert!(
        text.starts_with(&format!("{{\"format\":\"{FORMAT}\"")),
        "not our format: {}",
        &text[..60.min(text.len())]
    );
    assert_eq!(board(&a).len(), 2, "the in-app clipboard was not set");
    assert!(a.status.text.contains('2'), "status: {}", a.status.text);
}

#[test]
fn copy_with_nothing_selected_copies_the_whole_clip() {
    let mut a = app(starter_project());
    let ctx = ctx();
    a.focus_piano_roll();
    a.selected_notes.clear();
    frame(&mut a, &ctx, vec![egui::Event::Copy]);
    let Payload::Notes { notes: copied, .. } = &board(&a).payload else {
        panic!("a piano-roll copy is notes");
    };
    assert_eq!(
        copied.len(),
        4,
        "FL copies the whole clip when nothing is selected"
    );
    // Relative to the clip's left edge, so a paste keeps each note on its beat.
    let starts: Vec<Ticks> = copied.iter().map(|n| n.start).collect();
    let orig: Vec<Ticks> = notes(&a).iter().map(|n| n.start).collect();
    assert_eq!(starts, orig);
}

#[test]
fn ctrl_x_cuts_in_one_undo_step_named_cut() {
    let mut a = app(starter_project());
    let ctx = ctx();
    let before = normalised(a.project());
    a.focus_piano_roll();
    a.selected_notes = vec![notes(&a)[0].id, notes(&a)[3].id];
    frame(&mut a, &ctx, vec![egui::Event::Cut]);
    assert_eq!(notes(&a).len(), 2);
    assert!(
        a.selected_notes.is_empty(),
        "the cut notes are still selected"
    );
    assert_eq!(a.bus.undo_label_key(), Some("cmd.cut_notes"));
    a.undo();
    assert_eq!(normalised(a.project()), before);

    // Clips, in the arrangement.
    let (t, c) = (a.project().tracks[0].id, a.project().tracks[0].clips[0].id);
    a.focus_arrangement();
    a.select_clip(t, c);
    let out = frame(&mut a, &ctx, vec![egui::Event::Cut]);
    assert!(copied(&out).is_some());
    assert!(
        a.project().tracks[0].clips.is_empty(),
        "the clip was not cut"
    );
    assert_eq!(a.bus.undo_label_key(), Some("cmd.cut_clips"));
    a.undo();
    assert_eq!(normalised(a.project()), before);
}

#[test]
fn event_paste_with_our_text_pastes_and_foreign_text_falls_back_to_the_app_clipboard() {
    let mut a = app(starter_project());
    let ctx = ctx();
    a.focus_piano_roll();
    let text = copied(&frame(&mut a, &ctx, vec![egui::Event::Copy])).unwrap();
    a.seek(Ticks::beats(8));
    frame(&mut a, &ctx, vec![egui::Event::Paste(text.clone())]);
    assert_eq!(notes(&a).len(), 8, "Ctrl+V with our text pasted nothing");
    assert_eq!(a.bus.undo_label_key(), Some("cmd.paste"));
    assert_eq!(a.selected_notes.len(), 4, "the pasted notes are selected");
    let first = notes(&a)
        .iter()
        .filter(|n| a.selected_notes.contains(&n.id))
        .map(|n| n.start)
        .min();
    assert_eq!(first, Some(Ticks::beats(8)), "pasted off the playhead");

    // Text copied in another program: what Cantus copied last is pasted.
    frame(
        &mut a,
        &ctx,
        vec![egui::Event::Paste("hello from Notepad".into())],
    );
    assert_eq!(notes(&a).len(), 12);

    // A clipboard from a newer Cantus: refused politely, nothing pasted.
    let newer = text.replacen(FORMAT, "cantus-clip/9", 1);
    frame(&mut a, &ctx, vec![egui::Event::Paste(newer)]);
    assert_eq!(notes(&a).len(), 12);
    assert!(a.status.error, "no word about the newer clipboard");
}

#[test]
fn every_clipboard_label_resolves_in_both_locales() {
    use cantus_ui::i18n::{set_thread_locale, t, Locale};
    let keys = [
        "cmd.cut_notes",
        "cmd.cut_clips",
        "cmd.cut_time",
        "cmd.paste",
        "cmd.paste_clips",
        "cmd.duplicate_clips",
        "clipboard.status.copied_notes",
        "clipboard.status.copied_clips",
        "clipboard.status.copied_time",
        "clipboard.status.nothing",
        "clipboard.status.empty",
        "clipboard.status.no_clip",
        "clipboard.status.clips_in_piano_roll",
        "clipboard.status.automation_skipped",
        "clipboard.status.newer",
        "clipboard.status.older",
    ];
    set_thread_locale(Locale::En);
    let english: Vec<String> = keys.iter().map(|k| t(k)).collect();
    set_thread_locale(Locale::Es);
    for (k, en) in keys.iter().zip(&english) {
        assert_ne!(en, k, "{k} has no English text");
        // `t` falls back to English, so a missing Spanish line would read as English.
        assert_ne!(&t(k), en, "{k} has no Spanish text");
    }
    set_thread_locale(Locale::En);
}

/// Three tracks: a note clip, an audio clip on a recorded file, an automation clip.
fn rich_project() -> Project {
    let mut p = starter_project();
    let mut t1 = p.make_track("Audio");
    let sid = AudioSourceId(p.alloc_id());
    p.audio_pool.push(AudioSource {
        id: sid,
        name: "take".into(),
        path: "C:/nowhere/take.wav".into(),
        sample_rate: 48_000,
        channels: 2,
        frames: 480_000,
        sha256: None,
        recorded: true,
    });
    let mut wave = p.make_audio_clip("take", sid, Ticks::beats(4), Ticks::beats(8));
    wave.audio.as_mut().unwrap().fade_in = Ticks::beats(1);
    t1.clips.push(wave);
    let gain_of = t1.id;
    p.tracks.push(t1);
    let auto = p.make_automation_clip(
        "gain",
        AutoTarget::TrackGain(gain_of),
        Ticks::ZERO,
        Ticks::beats(8),
        vec![AutoPoint::new(Ticks::ZERO, 0.5)],
    );
    p.tracks[1].clips.push(auto);
    p
}

#[test]
fn the_clipboard_survives_a_restart_and_crosses_projects() {
    let ctx_a = ctx();
    let mut a = app(rich_project());
    a.focus_arrangement();
    a.select_all_clips();
    let text = copied(&frame(&mut a, &ctx_a, vec![egui::Event::Copy])).unwrap();
    drop(a);

    // Another run of the app, another project: only the text is left.
    let ctx_b = ctx();
    let mut other = starter_project();
    let extra = other.make_track("Second");
    other.tracks.push(extra);
    other.tempo_bpm = 120.0;
    let mut b = app(other);
    assert!(b.clipboard.board.is_none());
    b.focus_arrangement();
    let t1 = b.project().tracks[1].id;
    b.select_track(t1);
    b.seek(Ticks::beats(32));
    let before = all_clips(&b).len();
    frame(&mut b, &ctx_b, vec![egui::Event::Paste(text)]);
    let after = all_clips(&b);
    assert_eq!(
        after.len(),
        before + 2,
        "the note clip and the audio clip should land"
    );
    let source = b
        .project()
        .audio_pool
        .first()
        .expect("the audio file came along");
    assert!(!source.recorded, "a save would move another project's file");
    assert!(
        std::path::Path::new(&source.path).is_absolute(),
        "{}",
        source.path
    );
    assert!(
        after.iter().all(|(_, c)| c.automation.is_none()),
        "an automation clip pointed into another project"
    );
    assert!(
        b.status.text.contains('1'),
        "nothing said the automation clip was left out: {}",
        b.status.text
    );
    assert_eq!(b.bus.undo_label_key(), Some("cmd.paste_clips"));
    b.undo();
    assert!(b.project().audio_pool.is_empty(), "undo left the source");
}

#[test]
fn cut_then_paste_clips_in_the_arrangement_pastes_them_back() {
    let mut a = app(rich_project());
    let ctx = ctx();
    let before = heard(&a);
    let (t0, c0) = (a.project().tracks[0].id, a.project().tracks[0].clips[0].id);
    a.focus_arrangement();
    a.select_clip(t0, c0);
    frame(&mut a, &ctx, vec![egui::Event::Cut]);
    assert!(a.project().tracks[0].clips.is_empty());
    // Nothing is selected now: the paste must not need a selection.
    a.seek(Ticks::ZERO);
    a.select_track(t0);
    frame(&mut a, &ctx, vec![egui::Event::Paste("not ours".into())]);
    assert_eq!(
        a.project().tracks[0].clips.len(),
        1,
        "cut then paste lost the clip"
    );
    assert_eq!(heard(&a), before);
    assert_eq!(a.focus, EditFocus::Arrangement);
}

#[test]
fn paste_onto_an_empty_track_works() {
    let mut a = app(starter_project());
    let (t0, c0) = (a.project().tracks[0].id, a.project().tracks[0].clips[0].id);
    a.add_track();
    let t1 = a.project().tracks[1].id;
    a.focus_arrangement();
    a.select_clip(t0, c0);
    a.run_action(Action::Copy);
    a.select_track(t1);
    assert!(a.selected_clip.is_none());
    a.seek(Ticks::beats(16));
    a.run_action(Action::Paste);
    let on_t1 = &a.project().track(t1).unwrap().clips;
    assert_eq!(on_t1.len(), 1, "pasting onto an empty track did nothing");
    assert_eq!(on_t1[0].start, Ticks::beats(16));
}

#[test]
fn the_empty_timeline_menu_pastes_where_it_was_opened() {
    let mut a = app(starter_project());
    let ctx = ctx();
    let (t0, c0) = (a.project().tracks[0].id, a.project().tracks[0].clips[0].id);
    a.add_track();
    a.add_track();
    let t2 = a.project().tracks[2].id;
    a.focus_arrangement();
    a.select_clip(t0, c0);
    a.run_action(Action::Copy);
    a.select_track(t0);
    // The menu was opened on the third track, at bar 9 and a bit; snap is on.
    a.arrangement.snap = true;
    a.arrangement.grid = Ticks::beats(4);
    a.arrangement.menu_at = Some((Ticks::beats(33) + Ticks(5), Some(t2)));
    cantus_ui::views::arrangement::paste_at_menu_point(&mut a);
    frame(&mut a, &ctx, vec![]);
    let on_t2 = &a.project().track(t2).unwrap().clips;
    assert_eq!(on_t2.len(), 1, "the menu's paste went elsewhere");
    assert_eq!(
        on_t2[0].start,
        Ticks::beats(32),
        "not snapped down to the bar"
    );
    // The point is used once.
    assert!(a.arrangement.menu_at.is_none());
}

#[test]
fn the_piano_roll_takes_the_edit_focus_when_clicked() {
    use cantus_ui::views;
    let mut a = app(starter_project());
    let ctx = ctx();
    a.focus_arrangement();
    a.piano_roll.tool = views::piano_roll::Tool::Select;
    let at = egui::pos2(600.0, 300.0);
    for (i, ev) in [
        vec![egui::Event::PointerMoved(at)],
        vec![egui::Event::PointerButton {
            pos: at,
            button: egui::PointerButton::Primary,
            pressed: true,
            modifiers: egui::Modifiers::NONE,
        }],
        vec![egui::Event::PointerButton {
            pos: at,
            button: egui::PointerButton::Primary,
            pressed: false,
            modifiers: egui::Modifiers::NONE,
        }],
    ]
    .into_iter()
    .enumerate()
    {
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(1200.0, 600.0),
            )),
            events: ev,
            time: Some(i as f64 / 60.0),
            ..Default::default()
        };
        ctx.run_ui(input, |ui| {
            egui::CentralPanel::default()
                .frame(egui::Frame::NONE)
                .show(ui, |ui| views::piano_roll::show(&mut a, ui));
        })
        .textures_delta
        .clear();
    }
    assert_eq!(
        a.focus,
        EditFocus::PianoRoll,
        "a click in the piano roll left the focus"
    );
    frame(&mut a, &ctx, vec![egui::Event::Copy]);
    assert!(
        matches!(board(&a).payload, Payload::Notes { .. }),
        "Ctrl+C after a click in the piano roll copied clips"
    );
}

/// Copy the starter clip in the arrangement and paste it with the playhead at `at`.
fn arrangement_paste_at(at: Ticks, snap: bool, time: Option<(Ticks, Ticks)>) -> Ticks {
    let mut a = app(starter_project());
    let (t0, c0) = (a.project().tracks[0].id, a.project().tracks[0].clips[0].id);
    a.add_track();
    let t1 = a.project().tracks[1].id;
    a.focus_arrangement();
    a.select_clip(t0, c0);
    a.run_action(Action::Copy);
    a.select_track(t1);
    a.arrangement.grid = Ticks::beats(1);
    a.arrangement.snap = snap;
    if let Some((from, to)) = time {
        a.set_time_selection(from, to);
    }
    a.seek(at);
    a.run_action(Action::Paste);
    let on_t1 = &a.project().track(t1).unwrap().clips;
    assert_eq!(on_t1.len(), 1);
    on_t1[0].start
}

#[test]
fn paste_at_the_playhead_snaps_down_not_to_nearest() {
    let at = Ticks((PPQ as f64 * 3.6) as i64);
    assert_eq!(arrangement_paste_at(at, true, None), Ticks::beats(3));
}

#[test]
fn snap_off_pastes_at_the_exact_playhead() {
    let at = Ticks((PPQ as f64 * 3.6) as i64);
    assert_eq!(arrangement_paste_at(at, false, None), at);
}

#[test]
fn paste_lands_at_the_time_selection_start() {
    let sel = Some((Ticks::beats(12), Ticks::beats(20)));
    assert_eq!(
        arrangement_paste_at(Ticks::beats(2), true, sel),
        Ticks::beats(12)
    );
}

#[test]
fn notes_paste_into_the_piano_roll_at_the_snapped_playhead() {
    let mut a = app(starter_project());
    a.focus_piano_roll();
    a.selected_notes = vec![notes(&a)[0].id];
    a.run_action(Action::Copy);
    a.piano_roll.grid = Ticks::beats(1);
    a.piano_roll.snap = true;
    a.seek(Ticks((PPQ as f64 * 5.5) as i64));
    a.run_action(Action::Paste);
    let n = notes(&a)
        .into_iter()
        .find(|n| a.selected_notes.contains(&n.id))
        .unwrap();
    assert_eq!(n.start, Ticks::beats(5));
}

/// Four tracks, each with a clip over the range 4..8 beats: notes every beat, a looping
/// note clip, an audio clip with fades and a reversed audio clip.
fn range_project() -> Project {
    let mut p = starter_project();
    p.tracks[0].clips[0].notes.clear();
    for b in 0..16 {
        let n = p.make_note(Ticks::beats(b), Ticks::QUARTER, 60 + b as u8, 100);
        p.tracks[0].clips[0].notes.push(n);
    }
    let mut looped = p.make_track("Loop");
    let mut clip = p.make_clip("loop", Ticks::beats(2), Ticks::beats(10));
    clip.content_length = Some(Ticks::beats(3));
    for b in 0..3 {
        let n = p.make_note(Ticks::beats(b), Ticks::QUARTER, 40 + b as u8, 90);
        clip.notes.push(n);
    }
    looped.clips.push(clip);
    p.tracks.push(looped);
    let sid = AudioSourceId(p.alloc_id());
    p.audio_pool.push(AudioSource {
        id: sid,
        name: "w".into(),
        path: "C:/nowhere/w.wav".into(),
        sample_rate: 48_000,
        channels: 1,
        frames: 48_000 * 20,
        sha256: None,
        recorded: false,
    });
    for reverse in [false, true] {
        let mut t = p.make_track(if reverse { "Rev" } else { "Wave" });
        let mut c = p.make_audio_clip("w", sid, Ticks::beats(2), Ticks::beats(8));
        let r = c.audio.as_mut().unwrap();
        r.fade_in = Ticks::beats(1);
        r.fade_out = Ticks::beats(1);
        r.reverse = reverse;
        t.clips.push(c);
        p.tracks.push(t);
    }
    p
}

#[test]
fn copying_a_time_range_crops_clips_across_tracks() {
    let mut a = app(range_project());
    a.focus_arrangement();
    a.set_time_selection(Ticks::beats(4), Ticks::beats(8));
    a.run_action(Action::Copy);
    let Payload::Clips { rows, span } = &board(&a).payload else {
        panic!("a time range copies clips");
    };
    assert_eq!(*span, Some(Ticks::beats(4)));
    assert_eq!(rows.len(), 4, "every track's clip over the range");
    for r in rows {
        assert_eq!(
            r.clip.start,
            Ticks::ZERO,
            "{}: not relative to the range",
            r.clip.name
        );
        assert_eq!(
            r.clip.length,
            Ticks::beats(4),
            "{}: not cropped",
            r.clip.name
        );
    }
    assert_eq!(rows[0].clip.offset, Ticks::beats(4));
    // The cut edges are hard: the fades stay with the outer pieces.
    let wave = rows[2].clip.audio.as_ref().unwrap();
    assert_eq!((wave.fade_in, wave.fade_out), (Ticks::ZERO, Ticks::ZERO));
    assert_eq!(
        a.project().tracks[0].clips.len(),
        1,
        "a copy changed the project"
    );
}

#[test]
fn cutting_a_time_range_splits_removes_and_undoes_exactly() {
    let mut a = app(range_project());
    let before = normalised(a.project());
    let sounded = heard(&a);
    a.focus_arrangement();
    let (from, to) = (Ticks::beats(4), Ticks::beats(8));
    a.set_time_selection(from, to);
    a.run_action(Action::Cut);
    assert_eq!(a.bus.undo_label_key(), Some("cmd.cut_time"));
    let inside = |t: i64| t >= from.0 && t < to.0;
    let expect: Vec<(i64, u8)> = sounded
        .iter()
        .copied()
        .filter(|(t, _)| !inside(*t))
        .collect();
    assert_eq!(
        heard(&a),
        expect,
        "the cut took the wrong notes, or moved the rest"
    );
    for (t, c) in all_clips(&a) {
        assert!(
            c.end() <= from || c.start >= to,
            "a clip on {t:?} still covers the cut range: {}..{}",
            c.start.0,
            c.end().0
        );
    }
    // Paste it straight back: what was heard is heard again.
    a.seek(from);
    a.select_track(a.project().tracks[0].id);
    a.run_action(Action::Paste);
    assert_eq!(heard(&a), sounded, "cut and paste back is not a round trip");
    a.undo();
    a.undo();
    assert_eq!(
        normalised(a.project()),
        before,
        "undo did not restore the project"
    );
}

#[test]
fn last_touched_selection_decides_between_time_range_and_clips() {
    let mut a = app(range_project());
    let (t1, c1) = (a.project().tracks[1].id, a.project().tracks[1].clips[0].id);
    a.focus_arrangement();
    a.set_time_selection(Ticks::beats(4), Ticks::beats(8));
    a.select_clip(t1, c1);
    a.run_action(Action::Copy);
    let Payload::Clips { rows, span } = &board(&a).payload else {
        panic!()
    };
    assert_eq!(
        (rows.len(), *span),
        (1, None),
        "a stale range took over a clip copy"
    );
    assert_eq!(
        rows[0].clip.length,
        Ticks::beats(10),
        "the clip was cropped"
    );
    a.set_time_selection(Ticks::beats(4), Ticks::beats(8));
    a.run_action(Action::Copy);
    let Payload::Clips { rows, span } = &board(&a).payload else {
        panic!()
    };
    assert_eq!((rows.len(), *span), (4, Some(Ticks::beats(4))));
}

/// One frame with no Ctrl held: what egui-winit delivers when it answers a menu's
/// `RequestPaste` (the user's hand is on the mouse).
fn frame_no_ctrl(a: &mut CantusApp, ctx: &egui::Context, events: Vec<egui::Event>) {
    let mut ev = vec![egui::Event::ModifiersChanged(egui::Modifiers::NONE)];
    ev.extend(events);
    let input = egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(W, H),
        )),
        events: ev,
        time: Some(ctx.input(|i| i.time) + 1.0 / 60.0),
        predicted_dt: 1.0 / 60.0,
        ..Default::default()
    };
    ctx.run_ui(input, |ui| a.frame(ui)).textures_delta.clear();
}

/// eframe queues a `RequestPaste` sent in frame N after that frame's paint, carries it out
/// in frame N+1's paint (after the app ran) and so delivers the answer in frame N+2. A
/// paste that gave up after one frame pasted from memory and then pasted the late answer
/// again, at another place, as a second undo step.
#[test]
fn menu_paste_waits_for_the_answer_eframe_sends_two_frames_later_and_pastes_once() {
    let mut a = app(starter_project());
    let ctx = ctx();
    a.focus_piano_roll();
    let text = copied(&frame(&mut a, &ctx, vec![egui::Event::Copy])).unwrap();
    let n0 = notes(&a).len();
    let steps0 = a.bus.history().len();

    // Edit ▸ Paste: asked in frame N, nothing in N+1, the answer in N+2.
    let out = frame_with(&mut a, &ctx, vec![], |a| a.run_action(Action::Paste));
    assert!(
        asked_for_paste(&out),
        "the menu did not ask for the system clipboard"
    );
    frame_no_ctrl(&mut a, &ctx, vec![]);
    assert_eq!(notes(&a).len(), n0, "gave up on the answer after one frame");
    frame_no_ctrl(&mut a, &ctx, vec![egui::Event::Paste(text.clone())]);
    for _ in 0..10 {
        frame_no_ctrl(&mut a, &ctx, vec![]);
    }
    assert_eq!(notes(&a).len(), n0 + 4, "pasted twice, or not at all");
    assert_eq!(
        a.bus.history().len(),
        steps0 + 1,
        "one paste, one undo step"
    );
}

#[test]
fn menu_paste_with_no_text_on_the_clipboard_falls_back_once_and_ignores_a_late_answer() {
    let mut a = app(starter_project());
    let ctx = ctx();
    a.focus_piano_roll();
    let text = copied(&frame(&mut a, &ctx, vec![egui::Event::Copy])).unwrap();
    let n0 = notes(&a).len();

    // The system clipboard holds no text (an image): no answer ever comes. What Cantus
    // copied last is pasted — after a few frames and a tenth of a second, not at once.
    frame_with(&mut a, &ctx, vec![], |a| a.run_action(Action::Paste));
    frame_no_ctrl(&mut a, &ctx, vec![]);
    frame_no_ctrl(&mut a, &ctx, vec![]);
    assert_eq!(notes(&a).len(), n0, "fell back before eframe could answer");
    for _ in 0..10 {
        frame_no_ctrl(&mut a, &ctx, vec![]);
    }
    assert_eq!(notes(&a).len(), n0 + 4, "the fallback did not paste");

    // An answer arriving after the fallback is the same paste, late: not a second one.
    frame_no_ctrl(&mut a, &ctx, vec![egui::Event::Paste(text.clone())]);
    assert_eq!(notes(&a).len(), n0 + 4, "the late answer pasted again");

    // Ctrl+V is the user asking again, and pastes.
    frame(&mut a, &ctx, vec![egui::Event::Paste(text)]);
    assert_eq!(
        notes(&a).len(),
        n0 + 8,
        "Ctrl+V after a menu paste did nothing"
    );
}

/// A clip's own right-click menu acts on that clip, even when a time range drawn earlier
/// was the last selection touched: Cut must not cut the range across every track.
#[test]
fn the_clip_menu_cuts_the_clicked_clip_not_a_leftover_time_range() {
    let mut a = app(range_project());
    let (t1, c1) = (a.project().tracks[1].id, a.project().tracks[1].clips[0].id);
    a.focus_arrangement();
    a.select_clip(t1, c1);
    a.set_time_selection(Ticks::beats(4), Ticks::beats(8));
    let before = all_clips(&a);
    cantus_ui::views::arrangement::clip_menu_opened(&mut a, t1, c1);
    a.run_action(Action::Cut);
    assert_eq!(a.bus.undo_label_key(), Some("cmd.cut_clips"));
    let after = all_clips(&a);
    assert_eq!(after.len(), before.len() - 1, "more than the clip was cut");
    assert!(after.iter().all(|(_, c)| c.id != c1));
    for (t, c) in before.iter().filter(|(_, c)| c.id != c1) {
        assert!(
            after.iter().any(|(u, d)| u == t && d == c),
            "{} on another track was changed",
            c.name
        );
    }
}

/// Importing a DAWproject replaces the project: ids copied before it name other things
/// after it, so a clipboard from before must not count as "this project".
#[test]
fn a_dawproject_import_and_its_undo_make_an_earlier_copy_foreign() {
    use cantus_project::dawproject;
    use cantus_ui::views::interop;
    let mut a = app(rich_project());
    a.focus_arrangement();
    a.select_all_clips();
    a.run_action(Action::Copy);
    let copied_from = board(&a).origin;
    let file = dawproject::export(&rich_project(), "0.1.0");
    assert!(interop::import_dawproject(
        &mut a,
        &file.bytes,
        "x.dawproject"
    ));
    assert_ne!(
        a.clipboard_origin(),
        copied_from,
        "the import kept the origin"
    );
    let autos = |a: &CantusApp| {
        all_clips(a)
            .iter()
            .filter(|(_, c)| c.automation.is_some())
            .count()
    };
    let n = autos(&a);
    a.focus_arrangement();
    a.seek(Ticks::beats(64));
    a.run_action(Action::Paste);
    assert_eq!(
        autos(&a),
        n,
        "an automation clip from before the import was pasted onto an id it never meant"
    );
    a.undo();
    let imported = a.clipboard_origin();
    a.undo();
    assert_ne!(
        a.clipboard_origin(),
        imported,
        "undoing the import kept the origin"
    );
    let restored = a.clipboard_origin();
    a.redo();
    assert_ne!(
        a.clipboard_origin(),
        restored,
        "redoing the import kept the origin"
    );
}

#[test]
fn the_copied_text_carries_the_project_schema() {
    let mut a = app(starter_project());
    let ctx = ctx();
    a.focus_piano_roll();
    let text = copied(&frame(&mut a, &ctx, vec![egui::Event::Copy])).unwrap();
    let schema = format!("\"schema\":{}", cantus_project::SCHEMA_VERSION);
    assert!(
        text.contains(&schema),
        "no {schema} in {}",
        &text[..80.min(text.len())]
    );
}

#[test]
fn a_paste_is_one_undo_step_named_for_what_it_pasted() {
    let mut a = app(starter_project());
    let (t0, c0) = (a.project().tracks[0].id, a.project().tracks[0].clips[0].id);
    a.focus_arrangement();
    a.select_clip(t0, c0);
    a.run_action(Action::Copy);
    a.seek(Ticks::beats(32));
    a.run_action(Action::Paste);
    assert_eq!(a.bus.undo_label_key(), Some("cmd.paste_clips"));
    let pasted = a.clip_selection();
    assert_eq!(pasted.len(), 1);
    assert_ne!(pasted[0].1, c0, "the pasted clip is not the one selected");
    assert_eq!(clip_of(&a, pasted[0]).start, Ticks::beats(32));
    a.run_action(Action::Duplicate);
    assert_eq!(a.bus.undo_label_key(), Some("cmd.duplicate_clips"));
}
