//! Round 19 fixes on the recording path, in the application: a pass through a real
//! engine that warns of lost frames while it runs, the count-in before bar 1, every armed
//! instrument track heard, the project folder told to the audio cache, autosaves that
//! find their audio, the piano roll refusing audio clips, and an audio track's inspector.

mod support;
use cantus_core::{AudioSource, AudioSourceId, Ticks, TrackId};
use cantus_engine::{EngineHandle, Processor};
use cantus_ui::views::inspector;
use cantus_ui::{starter_project, CantusApp};
use support::*;

const SR: usize = 48_000;

/// An app on a hand-driven engine with one input channel, its project saved in `dir`, an
/// armed audio track; the processor and the input ring to drive it with.
fn rig(dir: &std::path::Path) -> (CantusApp, Processor, rtrb::Producer<f32>, TrackId) {
    cantus_ui::i18n::set_thread_locale(cantus_ui::i18n::Locale::En);
    let (h, mut proc) = EngineHandle::manual(SR as f64);
    let (ring, consumer) = rtrb::RingBuffer::<f32>::new(SR * 2);
    proc.attach_input(consumer, 1);
    let mut a = CantusApp::from_parts(Some(starter_project()), h, false);
    a.rec.demo_inputs = Some(vec![0.0]);
    a.path = Some(dir.join("song.cantus"));
    a.add_audio_track();
    let id = a.selected_track.unwrap();
    assert!(
        a.project().track(id).unwrap().armed,
        "a new audio track comes armed"
    );
    a.sync_engine();
    (a, proc, ring, id)
}

/// Push and render `frames` of input in blocks of 480, as the callback would, with a UI
/// frame after each block.
fn drive(a: &mut CantusApp, proc: &mut Processor, ring: &mut rtrb::Producer<f32>, frames: usize) {
    let mut out = vec![0f32; 480 * 2];
    for _ in 0..frames / 480 {
        for i in 0..480 {
            let _ = ring.push(((i % 7) as f32 - 3.0) * 0.1);
        }
        proc.process(&mut out, 2);
        a.engine.poll();
        a.poll_recording();
    }
}

#[test]
fn lost_frames_are_warned_of_while_the_pass_runs() {
    let dir = tempfile::tempdir().unwrap();
    let (mut a, mut proc, mut ring, track) = rig(dir.path());
    a.start_recording();
    assert!(
        a.rec.pass.as_ref().is_some_and(|p| p.audio),
        "an audio pass"
    );
    // The disk stalls: the 8 s ring fills, and then a second is lost.
    a.engine.hold_record_writer(true);
    drive(&mut a, &mut proc, &mut ring, SR * 9 + 960);
    assert!(a.is_recording(), "still recording");
    assert!(
        a.rec.dropped >= SR as u64,
        "{} frames known lost",
        a.rec.dropped
    );
    assert!(a.status.error, "the status line says so at once");
    // The disk comes back: the writer catches up, the recording goes on, and the gap is
    // filled with silence in the file.
    a.engine.hold_record_writer(false);
    let since = std::time::Instant::now();
    while a.rec.pass.as_ref().unwrap().takes[0].frames < (SR * 8) as u64 {
        assert!(since.elapsed().as_secs() < 20, "the writer never caught up");
        a.engine.poll();
        a.poll_recording();
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    drive(&mut a, &mut proc, &mut ring, SR);
    a.stop_recording();
    // The take keeps the pass's length: silence where the frames were lost.
    let clip = a
        .project()
        .track(track)
        .unwrap()
        .clips
        .iter()
        .find(|c| c.audio.is_some())
        .cloned()
        .unwrap();
    let src = a
        .project()
        .audio_source(clip.audio.as_ref().unwrap().source)
        .unwrap();
    assert!(src.frames >= (SR * 10) as u64, "{} frames", src.frames);
    assert!(a.rec.dropped >= SR as u64);
}

#[test]
fn a_count_in_at_bar_1_pre_rolls_and_every_armed_instrument_track_sounds() {
    let mut a = app(starter_project());
    let keys = a.project().tracks[0].id;
    let mut second = a.bus.new_track("Bass");
    second.armed = true;
    let bass = second.id;
    a.exec(cantus_core::Command::InsertTrack {
        index: 1,
        track: Box::new(second),
    });
    a.toggle_arm(keys, false);
    let mut s = a.project().recording.clone();
    s.count_in_bars = 1;
    a.set_record_settings(s);
    a.seek(Ticks::ZERO);
    a.start_recording();
    let bar = a.project().time_signature.ticks_per_bar();
    let live = a.rec.pass.as_ref().unwrap();
    assert_eq!(live.transport_start, Ticks::ZERO - bar, "a bar before zero");
    assert_eq!(
        live.plan.record_start,
        Ticks::ZERO,
        "the take starts at bar 1"
    );
    assert_eq!(a.engine.position(), Ticks::ZERO - bar);
    let mut echo = a.rec.echo.clone();
    echo.sort_by_key(|t| t.0);
    let mut want = vec![keys, bass];
    want.sort_by_key(|t| t.0);
    assert_eq!(echo, want, "both armed instrument tracks are heard");
    a.stop_recording();
    assert!(a.rec.echo.is_empty());
}

fn with_source(a: &mut CantusApp, path: &str) -> (TrackId, cantus_core::ClipId) {
    let id = AudioSourceId(a.bus.project().next_id);
    a.bus.reserve_ids(id.0 + 1);
    a.exec(cantus_core::Command::InsertAudioSource {
        source: AudioSource {
            id,
            name: "Gtr".into(),
            path: path.into(),
            sample_rate: 48_000,
            channels: 1,
            frames: 48_000,
            sha256: None,
            recorded: true,
        },
    });
    a.add_audio_track();
    let t = a.selected_track.unwrap();
    let mut scratch = a.bus.project().clone();
    let clip = scratch.make_audio_clip("Gtr", id, Ticks::ZERO, Ticks::beats(2));
    a.bus.reserve_ids(scratch.next_id);
    let c = clip.id;
    a.exec(cantus_core::Command::InsertClip {
        track: t,
        clip: Box::new(clip),
    });
    (t, c)
}

#[test]
fn the_audio_cache_follows_the_project_folder_and_autosaves_find_their_audio() {
    let dir = tempfile::tempdir().unwrap();
    let mut a = app(starter_project());
    a.sync_engine();
    assert_eq!(a.engine.samples.project_dir(), None, "unsaved");
    let song = dir.path().join("one").join("song.cantus");
    std::fs::create_dir_all(song.parent().unwrap()).unwrap();
    a.save_to_path(&song);
    assert_eq!(a.engine.samples.project_dir(), song.parent(), "saved");
    a.open(&song);
    assert_eq!(a.engine.samples.project_dir(), song.parent(), "opened");
    let other = dir.path().join("two").join("song.cantus");
    std::fs::create_dir_all(other.parent().unwrap()).unwrap();
    a.save_to_path(&other);
    assert_eq!(a.engine.samples.project_dir(), other.parent(), "saved as");
    // An autosave writes every relative path absolute: it is found from anywhere.
    with_source(&mut a, "Audio/Gtr.wav");
    let copy = a.autosave_copy();
    let p = std::path::Path::new(&copy.audio_pool[0].path);
    assert!(p.is_absolute(), "{}", p.display());
    assert_eq!(p, other.parent().unwrap().join("Audio/Gtr.wav"));
    assert_eq!(
        a.project().audio_pool[0].path,
        "Audio/Gtr.wav",
        "the project keeps its own"
    );
    a.new_project();
    assert_eq!(a.engine.samples.project_dir(), None, "new");
}

#[test]
fn the_piano_roll_refuses_an_audio_clip() {
    let mut a = app(starter_project());
    let (t, c) = with_source(&mut a, "C:/nowhere/Gtr.wav");
    a.select_clip(t, c);
    let ctx = ctx();
    let steps = a.bus.history().len();
    let run = |a: &mut CantusApp, events: Vec<egui::Event>| {
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(900.0, 500.0),
            )),
            events,
            ..Default::default()
        };
        let mut out = ctx.run_ui(input, |ui| {
            egui::CentralPanel::default()
                .frame(egui::Frame::NONE)
                .show(ui, |ui| cantus_ui::views::piano_roll::show(a, ui));
        });
        out.textures_delta.clear();
        painted_text(&out.shapes)
    };
    // Round 28 (ADR 0054): the piano roll's place shows the pitch editor, which says
    // the take is being analysed, then that its file cannot be read — and draws no note.
    let text = run(&mut a, vec![]);
    let failed = cantus_ui::i18n::t("pitch.failed");
    let failed = failed.split('{').next().unwrap_or_default().to_string();
    assert!(
        text.contains(&cantus_ui::i18n::t("pitch.analysing")) || text.contains(&failed),
        "{text}"
    );
    let t0 = std::time::Instant::now();
    while a.engine.samples.pitch().busy() && t0.elapsed() < std::time::Duration::from_secs(10) {
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    let text = run(&mut a, vec![]);
    assert!(text.contains(&failed), "{text}");
    // A click where the grid would be draws no note.
    let at = egui::pos2(500.0, 250.0);
    let press = |pressed| egui::Event::PointerButton {
        pos: at,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: egui::Modifiers::NONE,
    };
    run(&mut a, vec![egui::Event::PointerMoved(at), press(true)]);
    run(&mut a, vec![press(false)]);
    assert_eq!(a.bus.history().len(), steps);
    assert!(a
        .project()
        .track(t)
        .unwrap()
        .clip(c)
        .unwrap()
        .notes
        .is_empty());
}

#[test]
fn an_audio_tracks_inspector_shows_its_input_and_audio_not_an_instrument() {
    let mut a = app(starter_project());
    let (t, _) = with_source(&mut a, "C:/nowhere/Gtr.wav");
    a.select_track(t);
    assert_eq!(inspector::subject(&a), inspector::Subject::Track(t));
    let ctx = ctx();
    let mut text = String::new();
    for i in 0..2 {
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(300.0, 1600.0),
            )),
            time: Some(i as f64 * 0.016),
            ..Default::default()
        };
        let mut out = ctx.run_ui(input, |ui| {
            egui::CentralPanel::default()
                .frame(egui::Frame::NONE)
                .show(ui, |ui| inspector::show(&mut a, ui));
        });
        out.textures_delta.clear();
        text = painted_text(&out.shapes).to_lowercase();
    }
    let has = |k: &str| text.contains(&cantus_ui::i18n::t(k).to_lowercase());
    assert!(!has("inspector.title"), "no instrument: {text}");
    let firmus = cantus_ui::i18n::t(a.project().track(t).unwrap().instrument.name_key());
    assert!(!text.contains(&firmus.to_lowercase()), "no Firmus: {text}");
    assert!(has("audioclips.inspector.band"), "its audio: {text}");
    assert!(has("rec.input"), "its input: {text}");
}

#[test]
fn split_trim_and_glue_keep_a_takes_fine_offset() {
    use cantus_ui::{audio_clips, clips};
    let mut a = app(starter_project());
    let (t, c) = with_source(&mut a, "C:/nowhere/Gtr.wav");
    let clip = a.project().track(t).unwrap().clip(c).unwrap().clone();
    let mut region = clip.audio.clone().unwrap();
    region.slip_frames = -9;
    a.exec(audio_clips::set(t, &clip, region));
    let clip = a.project().track(t).unwrap().clip(c).unwrap().clone();
    // The frames the window shows move with it (the waveform and normalise read these).
    let (from, _) = audio_clips::window_frames(a.project(), &clip, 48_000);
    assert_eq!(from, -9.0);
    // A split: both halves keep it, so the content runs on across the cut.
    let mut next = a.project().next_id;
    let mut ids = || {
        next += 1;
        next - 1
    };
    let cmds = clips::split(a.project(), t, &clip, Ticks::beats(1), &mut ids).unwrap();
    a.bus.reserve_ids(next);
    a.exec(cantus_core::Command::Batch {
        label_key: "cmd.split".into(),
        commands: cmds,
    });
    let halves: Vec<_> = a.project().track(t).unwrap().clips.clone();
    assert_eq!(halves.len(), 2);
    assert!(halves
        .iter()
        .all(|h| h.audio.as_ref().unwrap().slip_frames == -9));
    // Glued back: one clip, the same fine offset.
    let refs: Vec<&cantus_core::Clip> = halves.iter().collect();
    assert!(audio_clips::glue(t, &refs).is_some());
    // Two windows whose content does not continue (another fine offset) do not glue.
    let mut other = halves[1].clone();
    other.audio.as_mut().unwrap().slip_frames = 4;
    assert!(audio_clips::glue(t, &[&halves[0], &other]).is_none());
}
