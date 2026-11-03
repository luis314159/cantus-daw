//! The audio editor (round 36, ADR 0096): a take opened from its clip, edits that make new
//! content-addressed files and one undo step each, noise reduction by profile, remove
//! silence, the agents' tools, and the window painted headless.

mod support;

use cantus_core::audio_edit::{silence, AudioBuf, Edit, Level, Sel};
use cantus_core::{ClipId, Command, Project, Ticks, TrackId};
use cantus_dsp::denoise::Params;
use cantus_ui::editor::{self, Act};
use cantus_ui::CantusApp;
use std::path::{Path, PathBuf};
use support::{app, ctx};

const RATE: u32 = 48_000;

fn noise(seed: u32, n: usize, amp: f32) -> Vec<f32> {
    let mut s = seed;
    (0..n)
        .map(|_| {
            s = s.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            ((s >> 8) as f32 / (1u32 << 23) as f32 - 1.0) * amp
        })
        .collect()
}

/// One second of hiss, one of a voice-like tone in hiss, one of hiss; mono.
fn take() -> Vec<f32> {
    let n = RATE as usize;
    let mut x = noise(7, 3 * n, 0.02);
    for (i, v) in x[n..2 * n].iter_mut().enumerate() {
        let t = i as f32 / RATE as f32;
        let env = 0.5 + 0.5 * cantus_dsp::math::sin(std::f32::consts::TAU * 3.0 * t);
        for h in 1..=3 {
            *v += env * 0.25 / h as f32
                * cantus_dsp::math::sin(std::f32::consts::TAU * 180.0 * h as f32 * t);
        }
    }
    x
}

fn write_wav(path: &Path, x: &[f32]) {
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: RATE,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut w = hound::WavWriter::create(path, spec).unwrap();
    for v in x {
        w.write_sample((v * 32767.0).round() as i16).unwrap();
    }
    w.finalize().unwrap();
}

struct Rig {
    app: CantusApp,
    ctx: egui::Context,
    dir: tempfile::TempDir,
    track: TrackId,
    clip: ClipId,
}

fn rig(x: &[f32]) -> Rig {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("take.wav");
    write_wav(&file, x);
    let mut p = Project::empty("Edit");
    let t = p.make_track("Voice");
    p.tracks.push(t);
    let mut a = app(p);
    a.path = Some(dir.path().join("song.cantus"));
    assert_eq!(a.import_audio(&[file], Ticks::ZERO, None), 1);
    let (track, clip) = {
        let t = a
            .project()
            .tracks
            .iter()
            .find(|t| !t.clips.is_empty())
            .unwrap();
        (t.id, t.clips[0].id)
    };
    Rig {
        app: a,
        ctx: ctx(),
        dir,
        track,
        clip,
    }
}

impl Rig {
    /// Frames until the editor's thread has nothing left to do.
    fn settle(&mut self) {
        for _ in 0..4000 {
            editor::frame(&mut self.app, &self.ctx);
            let s = self.app.editor.session.as_ref().expect("a session");
            if s.busy.is_none() && s.buf.is_some() && s.spec.is_some() {
                return;
            }
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        panic!("the editor never settled");
    }

    fn open(&mut self) {
        editor::open(&mut self.app, self.track, self.clip);
        self.settle();
    }

    fn do_edit(&mut self, e: Edit) {
        editor::act(&mut self.app, Act::Edit(e));
        self.settle();
    }

    fn source(&self) -> cantus_core::AudioSourceId {
        self.app
            .project()
            .track(self.track)
            .unwrap()
            .clip(self.clip)
            .unwrap()
            .audio
            .as_ref()
            .unwrap()
            .source
    }

    fn status(&self) -> String {
        self.app
            .editor
            .session
            .as_ref()
            .and_then(|s| s.status.clone())
            .map(|s| s.0)
            .unwrap_or_default()
    }

    fn audio_files(&self) -> Vec<PathBuf> {
        // The project is song.cantus: its edits are in Audio/song (ADR 0096 §2).
        let d = self.dir.path().join("Audio").join("song");
        let mut v: Vec<PathBuf> = std::fs::read_dir(d)
            .map(|r| r.filter_map(|e| e.ok().map(|e| e.path())).collect())
            .unwrap_or_default();
        v.sort();
        v
    }

    fn edited_buf(&self) -> AudioBuf {
        let s = self
            .app
            .project()
            .audio_source(self.source())
            .unwrap()
            .clone();
        let path = self.app.audio_file(&s);
        cantus_ui::editor::jobs::load(&path)
            .unwrap()
            .buf
            .as_ref()
            .clone()
    }
}

#[test]
fn a_take_opens_from_its_clip_and_is_read_whole() {
    let x = take();
    let mut r = rig(&x);
    r.open();
    let s = r.app.editor.session.as_ref().unwrap();
    let b = s.buf.as_ref().unwrap();
    assert_eq!((b.rate, b.channels, b.frames()), (RATE, 1, x.len()));
    assert!(s.peaks.is_some() && s.spec.is_some());
    editor::close(&mut r.app);
    assert!(r.app.editor.session.is_none());
}

#[test]
fn an_edit_is_a_new_content_addressed_file_and_one_undo_step() {
    let x = take();
    let mut r = rig(&x);
    r.open();
    let before = r.source();
    let history = r.app.project().clone();
    r.do_edit(Edit::Gain { db: -6.0 });
    let after = r.source();
    assert_ne!(before, after, "the clip plays a new file");
    // The original is still in the pool and on disk; the new one is named by its bytes.
    let pool = &r.app.project().audio_pool;
    assert_eq!(pool.len(), 2);
    let new = pool.iter().find(|s| s.id == after).unwrap();
    let sha = new.sha256.clone().expect("hash");
    assert!(
        new.path.contains(&format!("edit-{}", &sha[..16])),
        "{}",
        new.path
    );
    let files = r.audio_files();
    assert_eq!(files.len(), 1, "{files:?}");
    let bytes = std::fs::read(&files[0]).unwrap();
    assert_eq!(cantus_core::audio_edit::wav::sha256_hex(&bytes), sha);
    assert!(r.dir.path().join("take.wav").exists());
    // The audio is the original less 6 dB, to the float.
    let b = r.edited_buf();
    let g = cantus_dsp::math::db_to_gain(-6.0);
    let orig = cantus_ui::editor::jobs::load(&r.dir.path().join("take.wav"))
        .unwrap()
        .buf;
    assert!(b.data.iter().zip(&orig.data).all(|(a, o)| *a == *o * g));
    // One undo step puts the clip back; the editor follows it to the old file.
    r.app.undo();
    assert_eq!(r.app.project().audio_pool, history.audio_pool);
    assert_eq!(r.source(), before);
    r.settle();
    assert_eq!(
        r.app
            .editor
            .session
            .as_ref()
            .unwrap()
            .buf
            .as_ref()
            .unwrap()
            .data,
        orig.data
    );
    // Redo and the same edit again are the same bytes: one file, not two.
    r.do_edit(Edit::Gain { db: -6.0 });
    assert_eq!(r.audio_files().len(), 1);
}

#[test]
fn an_edit_that_changes_nothing_writes_nothing() {
    let mut r = rig(&take());
    r.open();
    let before = r.source();
    r.do_edit(Edit::Gain { db: 0.0 });
    assert_eq!(r.source(), before);
    assert!(r.audio_files().is_empty());
    assert!(!r.status().is_empty());
    assert_eq!(r.app.project().audio_pool.len(), 1);
}

#[test]
fn delete_needs_a_selection_and_shortens_a_clip_that_showed_the_whole_take() {
    let x = take();
    let mut r = rig(&x);
    r.open();
    let len = r
        .app
        .project()
        .track(r.track)
        .unwrap()
        .clip(r.clip)
        .unwrap()
        .length;
    r.do_edit(Edit::Delete);
    assert_eq!(
        r.source(),
        r.app.project().audio_pool[0].id,
        "refused: no selection"
    );
    r.app.editor.session.as_mut().unwrap().sel = Some(Sel::new(RATE as usize, 2 * RATE as usize));
    r.do_edit(Edit::Delete);
    // The edges moved to zero crossings, a few milliseconds at most.
    let got = r.edited_buf().frames() as i64;
    assert!((got - 2 * RATE as i64).abs() <= 480, "{got}");
    let now = r
        .app
        .project()
        .track(r.track)
        .unwrap()
        .clip(r.clip)
        .unwrap()
        .length;
    assert!(now < len, "{now:?} < {len:?}");
    r.app.undo();
    let back = r
        .app
        .project()
        .track(r.track)
        .unwrap()
        .clip(r.clip)
        .unwrap()
        .length;
    assert_eq!(back, len, "one undo step puts the length back too");
}

#[test]
fn noise_reduction_learns_a_stretch_and_cleans_the_take() {
    let x = take();
    let mut r = rig(&x);
    r.open();
    // Nothing to reduce with before the noise is learned.
    r.do_edit(Edit::Denoise {
        profile: cantus_dsp::denoise::NoiseProfile::learn(&[vec![0.0; 9000]], RATE).unwrap(),
        params: Params::default(),
    });
    assert!(r.audio_files().is_empty());
    r.app.editor.session.as_mut().unwrap().sel = Some(Sel::new(0, RATE as usize / 2));
    editor::act(&mut r.app, Act::Learn);
    r.settle();
    assert!(r.app.editor.session.as_ref().unwrap().profile.is_some());
    // Amount 0 is the take as it was.
    let none = Params {
        amount: 0.0,
        ..Params::default()
    };
    r.app.editor.session.as_mut().unwrap().sel = None;
    r.do_edit(Edit::Denoise {
        profile: r
            .app
            .editor
            .session
            .as_ref()
            .unwrap()
            .profile
            .clone()
            .unwrap(),
        params: none,
    });
    assert!(r.audio_files().is_empty(), "amount 0 touches nothing");
    // Full amount: the hiss is down, the voice is there.
    r.do_edit(Edit::Denoise {
        profile: r
            .app
            .editor
            .session
            .as_ref()
            .unwrap()
            .profile
            .clone()
            .unwrap(),
        params: Params::default(),
    });
    assert_eq!(r.audio_files().len(), 1);
    let b = r.edited_buf();
    let rms =
        |s: &[f32]| (s.iter().map(|v| (*v as f64).powi(2)).sum::<f64>() / s.len() as f64).sqrt();
    let n = RATE as usize;
    let hiss_before = rms(&x[2_000..n - 2_000]);
    let hiss_after = rms(&b.data[2_000..n - 2_000]);
    assert!(
        hiss_after < hiss_before * 0.25,
        "{hiss_before} -> {hiss_after}"
    );
    let voice_before = rms(&x[n + 6_000..2 * n - 6_000]);
    let voice_after = rms(&b.data[n + 6_000..2 * n - 6_000]);
    assert!(
        (voice_after / voice_before - 1.0).abs() < 0.12,
        "{voice_before} -> {voice_after}"
    );
}

#[test]
fn remove_silence_cuts_the_quiet_and_keeps_the_phrases() {
    let n = RATE as usize;
    let tone = |k: usize| -> Vec<f32> {
        (0..k)
            .map(|i| 0.4 * cantus_dsp::math::sin(i as f32 * 0.2))
            .collect()
    };
    let mut x = tone(n / 2);
    x.extend(vec![0.0; 2 * n]);
    x.extend(tone(n / 2));
    let mut r = rig(&x);
    r.open();
    r.do_edit(Edit::RemoveSilence(silence::Settings {
        threshold_db: -60.0,
        min_ms: 300.0,
        keep_ms: 100.0,
    }));
    let b = r.edited_buf();
    let expect = x.len() - (2 * n - n / 10);
    assert!(
        (b.frames() as i64 - expect as i64).abs() < 480,
        "{} vs {expect}",
        b.frames()
    );
    let s = r.app.editor.session.as_ref().unwrap();
    assert!(s.marks.markers.is_empty());
}

#[test]
fn normalize_and_fades_and_reverse_are_each_a_step() {
    let mut r = rig(&take());
    r.open();
    let steps = |r: &Rig| r.app.project().audio_pool.len();
    r.do_edit(Edit::Normalize {
        level: Level::Peak,
        target_db: -3.0,
        ceiling_db: None,
    });
    let peak = r.edited_buf().peak(Sel::whole(3 * RATE as usize));
    assert!((20.0 * peak.log10() + 3.0).abs() < 0.05);
    r.do_edit(Edit::Reverse);
    r.do_edit(Edit::Invert);
    assert_eq!(steps(&r), 4);
    r.app.undo();
    r.app.undo();
    r.app.undo();
    assert_eq!(steps(&r), 1);
}

#[test]
fn the_editor_keeps_cut_copy_paste_delete_while_open_and_lets_go_when_closed() {
    use cantus_ui::keymap::Action;
    let mut r = rig(&take());
    assert!(
        !editor::action(&mut r.app, Action::Delete),
        "closed: not its keys"
    );
    r.open();
    for a in [
        Action::Delete,
        Action::Copy,
        Action::Cut,
        Action::Paste,
        Action::SelectAll,
        Action::PlayPause,
    ] {
        assert!(editor::action(&mut r.app, a), "{a:?}");
    }
    editor::close(&mut r.app);
    assert!(!editor::action(&mut r.app, Action::Copy));
}

#[test]
fn undo_redo_while_open_makes_the_window_read_the_clips_file_again() {
    let mut r = rig(&take());
    r.open();
    r.do_edit(Edit::Silence); // refused (no selection) — nothing changes
    r.app.editor.session.as_mut().unwrap().sel = Some(Sel::new(100, 4000));
    r.do_edit(Edit::Silence);
    let edited = r.source();
    r.app.undo();
    r.settle();
    r.app.redo();
    r.settle();
    assert_eq!(r.source(), edited);
    let b = r.app.editor.session.as_ref().unwrap().buf.clone().unwrap();
    // (Its edges moved to zero crossings and crossfade in and out: the inside is silent.)
    assert!(b.data[500..3600].iter().all(|v| *v == 0.0));
}

#[test]
fn the_window_paints_headless_in_both_themes_with_its_words() {
    let mut r = rig(&take());
    r.open();
    r.app.editor.session.as_mut().unwrap().sel = Some(Sel::new(10_000, 40_000));
    for dark in [true, false] {
        let theme = if dark {
            cantus_ui::theme::cantus_night()
        } else {
            cantus_ui::theme::cantus_paper()
        };
        cantus_ui::theme::apply(&theme, &r.ctx);
        let text = support::paint(&r.ctx, egui::vec2(1500.0, 900.0), 3, |ui| {
            let ctx = ui.ctx().clone();
            editor::view::show(&mut r.app, &ctx);
        });
        for word in [
            "Audio editor",
            "Normalize",
            "Reduce noise",
            "Remove silence",
            "Play",
        ] {
            assert!(text.contains(word), "{dark}: no {word:?} in {text}");
        }
    }
    // At 200 % the spectrogram is wider than a texture may be (2160 px at 1500 points):
    // its picture stays within the side egui allows.
    r.ctx.set_zoom_factor(2.0);
    let text = support::paint(&r.ctx, egui::vec2(3000.0, 1800.0), 3, |ui| {
        let ctx = ui.ctx().clone();
        editor::view::show(&mut r.app, &ctx);
    });
    assert!(text.contains("Audio editor"));
}

// --- the agents' tools -----------------------------------------------------------------

/// An agent's call as the server runs it: a job's work on its own thread (here, in line)
/// and the edit it ends in planned back on the interface thread.
fn call(
    r: &mut Rig,
    name: &str,
    args: serde_json::Value,
) -> Result<cantus_ai::EditPlan, cantus_ai::ToolError> {
    let def = cantus_ui::ai_tools::find(name).expect("the tool");
    match &def.run {
        cantus_ai::ToolRun::Edit(f) => {
            let mut cx = cantus_ai::ToolCx { host: &mut r.app };
            f(&mut cx, &args)
        }
        cantus_ai::ToolRun::Job(f) => {
            let job = {
                let mut cx = cantus_ai::ToolCx { host: &mut r.app };
                f(&mut cx, &args)?
            };
            let progress = |_: f32| {};
            let cancelled = || false;
            let jcx = cantus_ai::JobCx {
                progress: &progress,
                cancelled: &cancelled,
            };
            match (job.work)(&jcx)? {
                cantus_ai::JobDone::Then(_, then) => {
                    let mut cx = cantus_ai::ToolCx { host: &mut r.app };
                    then(&mut cx)
                }
                cantus_ai::JobDone::Answer(_) => panic!("an edit job"),
            }
        }
        _ => panic!("an edit or a job"),
    }
}

#[test]
fn the_agents_edit_a_take_in_one_undo_step() {
    let mut r = rig(&take());
    let id = r.clip.0;
    let plan = call(
        &mut r,
        "edit_audio",
        serde_json::json!({"clip": id, "operation": "normalize", "level": "peak", "target_db": -2.0}),
    )
    .expect("planned");
    let before = r.source();
    assert!(r.app.exec(plan.command));
    assert_ne!(r.source(), before);
    let peak = r.edited_buf().peak(Sel::whole(3 * RATE as usize));
    assert!((20.0 * peak.log10() + 2.0).abs() < 0.05);
    r.app.undo();
    assert_eq!(r.source(), before);
    // A range operation without a range, and a bad operation, are refused in words.
    let e = call(
        &mut r,
        "edit_audio",
        serde_json::json!({"clip": id, "operation": "delete"}),
    );
    assert!(e.is_err());
    let e = call(
        &mut r,
        "edit_audio",
        serde_json::json!({"clip": id, "operation": "explode"}),
    );
    assert!(e.is_err());
}

#[test]
fn the_agents_denoise_by_a_noise_stretch_and_amount_zero_changes_nothing() {
    let mut r = rig(&take());
    let id = r.clip.0;
    let args = |amount: f64| serde_json::json!({"clip": id, "noise_start_seconds": 0.0, "noise_end_seconds": 0.5, "amount": amount});
    let e = call(&mut r, "denoise_clip", args(0.0));
    assert!(e.is_err(), "amount 0 would change nothing");
    assert!(r.audio_files().is_empty());
    let plan = call(&mut r, "denoise_clip", args(1.0)).expect("planned");
    assert!(matches!(plan.command, Command::Batch { .. }));
    assert!(r.app.exec(plan.command));
    assert_eq!(r.audio_files().len(), 1);
    let short = call(
        &mut r,
        "denoise_clip",
        serde_json::json!({"clip": id, "noise_start_seconds": 0.0, "noise_end_seconds": 0.01}),
    );
    assert!(short.is_err(), "too short a stretch to learn from");
}

// --- the integrator's review of round 36 -------------------------------------------------

fn clip_of(r: &Rig) -> cantus_core::Clip {
    r.app
        .project()
        .track(r.track)
        .unwrap()
        .clip(r.clip)
        .unwrap()
        .clone()
}

#[test]
fn a_cut_or_paste_while_an_edit_is_on_its_way_is_held_and_said() {
    let mut r = rig(&take());
    r.open();
    r.app.editor.session.as_mut().unwrap().sel = Some(Sel::new(1_000, 9_000));
    editor::act(&mut r.app, Act::Edit(Edit::Gain { db: -3.0 }));
    assert!(r.app.editor.busy());
    let before = r.app.editor.clipboard.clone();
    editor::act(&mut r.app, Act::Cut);
    assert_eq!(
        r.app.editor.clipboard, before,
        "the clipboard is not touched"
    );
    editor::act(&mut r.app, Act::Edit(Edit::Invert));
    assert!(r.status().contains("Wait"), "{}", r.status());
    r.settle();
    // The step is the gain's, named as the gain — not the edit refused meanwhile.
    assert_eq!(r.app.bus.undo_label_key(), Some("cmd.editor.gain"));
}

#[test]
fn gain_like_edits_keep_the_warp_and_a_reversal_drops_it_and_says_so() {
    let mut r = rig(&take());
    let mut region = clip_of(&r).audio.unwrap();
    region.warp = Some(Box::new(cantus_core::warp::Warp::new(RATE, 120.0)));
    assert!(r.app.exec(Command::SetClipAudio {
        track: r.track,
        clip: r.clip,
        audio: Some(region),
    }));
    r.open();
    r.do_edit(Edit::Gain { db: -2.0 });
    assert!(clip_of(&r).audio.unwrap().warp.is_some(), "a gain keeps it");
    r.do_edit(Edit::Reverse);
    assert!(
        clip_of(&r).audio.unwrap().warp.is_none(),
        "a reversal drops it"
    );
    assert!(r.status().contains("dropped"), "{}", r.status());
}

#[test]
fn another_project_closes_the_window() {
    let mut r = rig(&take());
    r.open();
    r.app.new_project();
    assert!(r.app.editor.session.is_none());
    // And an undoable replacement too, on the next frame.
    let mut r = rig(&take());
    r.open();
    let p = cantus_core::Project::empty("Other");
    assert!(r.app.exec(Command::ReplaceProject {
        project: Box::new(p)
    }));
    editor::frame(&mut r.app, &r.ctx);
    assert!(r.app.editor.session.is_none());
}

#[test]
fn a_save_as_while_an_edit_is_made_puts_its_file_in_the_new_folder() {
    let mut r = rig(&take());
    r.open();
    editor::act(&mut r.app, Act::Edit(Edit::Gain { db: -4.0 }));
    let moved = tempfile::tempdir().unwrap();
    r.app.path = Some(moved.path().join("moved.cantus"));
    r.settle();
    let s = r.app.project().audio_source(r.source()).unwrap().clone();
    let file = r.app.audio_file(&s);
    assert!(
        file.starts_with(moved.path().join("Audio")),
        "{}",
        file.display()
    );
    assert!(file.exists());
}

#[test]
fn a_result_for_a_closed_window_leaves_no_file() {
    let mut r = rig(&take());
    r.open();
    editor::act(&mut r.app, Act::Edit(Edit::Gain { db: -5.0 }));
    editor::close(&mut r.app);
    // The thread finishes the edit; its result comes back for no one.
    std::thread::sleep(std::time::Duration::from_millis(500));
    for _ in 0..400 {
        editor::frame(&mut r.app, &r.ctx);
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    assert!(r.audio_files().is_empty(), "{:?}", r.audio_files());
}

#[test]
fn a_declined_agent_edit_leaves_no_file() {
    let mut r = rig(&take());
    let id = r.clip.0;
    let plan = call(
        &mut r,
        "edit_audio",
        serde_json::json!({"clip": id, "operation": "gain", "gain_db": -3.0}),
    )
    .expect("planned");
    assert_eq!(r.audio_files().len(), 1, "written by the job");
    drop(plan); // the card declined: never executed
    editor::frame(&mut r.app, &r.ctx);
    assert!(r.audio_files().is_empty());
}

#[test]
fn the_agents_bad_arguments_are_bad_arguments() {
    let mut r = rig(&take());
    let id = r.clip.0;
    for args in [
        serde_json::json!({"clip": id, "operation": "insert_silence", "seconds": 0}),
        serde_json::json!({"clip": id, "operation": "attenuate", "start_seconds": 0.5, "end_seconds": 1.0, "lo_hz": 4000.0, "hi_hz": 1000.0, "gain_db": -6.0}),
    ] {
        match call(&mut r, "edit_audio", args.clone()) {
            Err(cantus_ai::ToolError::BadArgument { .. }) => {}
            Err(e) => panic!("{args}: {e:?}"),
            Ok(_) => panic!("{args}: planned"),
        }
    }
}

#[test]
fn marks_are_saved_with_the_file_and_move_with_a_cut() {
    let mut r = rig(&take());
    r.open();
    let n = RATE as usize;
    r.app.editor.session.as_mut().unwrap().sel = Some(Sel::new(2 * n, 2 * n));
    editor::act(&mut r.app, Act::AddMarker);
    r.app.editor.session.as_mut().unwrap().sel = Some(Sel::new(n / 2, n));
    editor::act(&mut r.app, Act::SetLoop);
    let m = r.app.project().source_marks[&r.source()].clone();
    assert_eq!(m.markers[0].frame, 2 * n as u64);
    assert_eq!(m.loop_points, Some((n as u64 / 2, n as u64)));
    // Cut the first half second: the marker moves up with the audio, the new file has it.
    r.app.editor.session.as_mut().unwrap().sel = Some(Sel::new(0, n / 2));
    r.do_edit(Edit::Delete);
    let moved = r.app.project().source_marks[&r.source()].clone();
    let cut = (3 * n - r.edited_buf().frames()) as u64;
    assert_eq!(moved.markers[0].frame, 2 * n as u64 - cut);
    // One undo returns the old file with its marks.
    r.app.undo();
    assert_eq!(r.app.project().source_marks[&r.source()], m);
}

#[test]
fn a_paste_from_another_rate_and_channel_count_is_conformed() {
    let mut r = rig(&take());
    r.open();
    // Half a second of stereo at 44.1 kHz.
    let clip = AudioBuf::new(44_100, 2, noise(4, 2 * 22_050, 0.2));
    r.app.editor.clipboard = Some(clip);
    r.app.editor.session.as_mut().unwrap().sel = Some(Sel::new(RATE as usize, RATE as usize));
    editor::act(&mut r.app, Act::Paste);
    r.settle();
    let b = r.edited_buf();
    assert_eq!((b.rate, b.channels), (RATE, 1));
    let added = b.frames() as i64 - 3 * RATE as i64;
    assert!((added - RATE as i64 / 2).abs() < 64, "{added}");
}

#[test]
fn a_file_pastes_and_opens_from_disk() {
    let mut r = rig(&take());
    r.open();
    let other = r.dir.path().join("other.wav");
    write_wav(&other, &noise(8, 12_000, 0.3));
    editor::paste_file(&mut r.app, &other);
    r.settle();
    let got = r.edited_buf().frames() as i64;
    assert!((got - (3 * RATE as i64 + 12_000)).abs() <= 1, "{got}");
    let tracks = r.app.project().tracks.len();
    editor::open_file(&mut r.app, &other);
    assert_eq!(r.app.project().tracks.len(), tracks + 1);
    let s = r.app.editor.session.as_ref().expect("open on the new clip");
    assert_ne!(s.clip, r.clip);
}

#[test]
fn a_selection_becomes_a_clip_of_its_own_in_one_step() {
    let mut r = rig(&take());
    r.open();
    let tracks = r.app.project().tracks.len();
    r.app.editor.session.as_mut().unwrap().sel = Some(Sel::new(RATE as usize, 2 * RATE as usize));
    // Dragged out of the window (the view sets it when the drag ends outside).
    r.app.editor.session.as_mut().unwrap().dragged_out = true;
    editor::frame(&mut r.app, &r.ctx);
    r.settle();
    let p = r.app.project();
    assert_eq!(p.tracks.len(), tracks + 1);
    let t = p
        .tracks
        .iter()
        .find(|t| t.id != r.track && !t.clips.is_empty())
        .unwrap();
    assert!(t.clips[0].name.contains("(part)"), "{}", t.clips[0].name);
    r.app.undo();
    assert_eq!(r.app.project().tracks.len(), tracks, "one undo step");
}

#[test]
fn unused_audio_is_listed_with_its_size_and_removed_in_one_step() {
    let mut r = rig(&take());
    r.open();
    r.do_edit(Edit::Gain { db: -6.0 });
    let first = r.audio_files();
    // Undone and replaced by another edit: the first edit's file is no one's now.
    r.app.undo();
    r.settle();
    r.do_edit(Edit::Gain { db: -9.0 });
    assert_eq!(r.audio_files().len(), 2);
    let plan = cantus_ui::editor::commit::plan_cleanup(&r.app);
    // The original take (no clip plays it now) and the orphaned edit file.
    assert_eq!(plan.sources.len(), 1);
    assert!(plan.sources[0].2 > 0);
    assert_eq!(plan.files.len(), 1);
    assert_eq!(plan.files[0].0, first[0]);
    let (entries, files) = cantus_ui::editor::commit::run_cleanup(&mut r.app);
    assert_eq!((entries, files), (1, 1));
    assert!(!first[0].exists());
    assert_eq!(r.app.project().audio_pool.len(), 1);
    // One undo brings the entry back (its file was never deleted: the history held it).
    r.app.undo();
    assert_eq!(r.app.project().audio_pool.len(), 2);
    assert!(r.dir.path().join("take.wav").exists());
}

#[test]
fn a_click_on_a_five_minute_take_is_found_in_detail_and_repaired() {
    let n = RATE as usize * 300;
    let clean: Vec<f32> = (0..n)
        .map(|i| 0.3 * cantus_dsp::math::sin(i as f32 * 0.031))
        .collect();
    let mut x = clean.clone();
    let at = RATE as usize * 180;
    x[at] += 0.6;
    x[at + 1] -= 0.4;
    let mut r = rig(&x);
    r.open();
    // Zoomed in on the click: the window asks for the span in detail.
    {
        let s = r.app.editor.session.as_mut().unwrap();
        s.view = editor::View {
            start: (at - 2_000) as f64,
            per: 4.0,
        };
        s.fit = false;
    }
    let _ = support::paint(&r.ctx, egui::vec2(1500.0, 900.0), 2, |ui| {
        let ctx = ui.ctx().clone();
        editor::view::show(&mut r.app, &ctx);
    });
    for _ in 0..2000 {
        editor::frame(&mut r.app, &r.ctx);
        if r.app.editor.session.as_ref().unwrap().detail.is_some() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    let s = r.app.editor.session.as_ref().unwrap();
    let (off, d) = s.detail.clone().expect("the span in detail");
    assert!(
        d.hop <= 512 && d.hop < s.spec.as_ref().unwrap().hop,
        "{}",
        d.hop
    );
    assert!(off <= at && off + d.frames * d.hop >= at);
    // Select the click and repair it (16-bit file: compare at its resolution).
    r.app.editor.session.as_mut().unwrap().sel = Some(Sel::new(at - 4, at + 8));
    r.do_edit(Edit::Repair);
    let b = r.edited_buf();
    let worst = (at - 10..at + 10)
        .map(|i| (b.data[i] - clean[i]).abs())
        .fold(0.0, f32::max);
    assert!(worst < 0.01, "{worst}");
}

// --- the fix round's review ---------------------------------------------------------------

/// A second project beside the first, in the same folder, with its own clip on the same take.
fn beside(r: &Rig, name: &str) -> Rig {
    let mut p = Project::empty(name);
    let t = p.make_track("Voice");
    p.tracks.push(t);
    let mut a = app(p);
    a.path = Some(r.dir.path().join(format!("{name}.cantus")));
    assert_eq!(
        a.import_audio(&[r.dir.path().join("take.wav")], Ticks::ZERO, None),
        1
    );
    let (track, clip) = {
        let t = a
            .project()
            .tracks
            .iter()
            .find(|t| !t.clips.is_empty())
            .unwrap();
        (t.id, t.clips[0].id)
    };
    Rig {
        app: a,
        ctx: ctx(),
        dir: tempfile::tempdir().unwrap(),
        track,
        clip,
    }
}

fn edit_files_under(dir: &Path) -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = std::fs::read_dir(dir)
        .map(|r| r.filter_map(|e| e.ok().map(|e| e.path())).collect())
        .unwrap_or_default();
    v.retain(|p| p.is_file());
    v.sort();
    v
}

#[test]
fn each_project_keeps_its_edits_in_its_own_folder_and_never_cleans_anothers() {
    let mut a = rig(&take());
    let mut b = beside(&a, "B");
    // B edits (and is not even saved): its file is in Audio/B, nothing of A's.
    b.open();
    b.do_edit(Edit::Gain { db: -3.0 });
    let b_files = edit_files_under(&a.dir.path().join("Audio").join("B"));
    assert_eq!(b_files.len(), 1, "B's edit in Audio/B");
    // A's cleanup lists nothing of B's — however unreferenced by A it is.
    a.open();
    let plan = cantus_ui::editor::commit::plan_cleanup(&a.app);
    assert!(
        plan.files.iter().all(|(p, _)| !b_files.contains(p)),
        "{:?}",
        plan.files
    );
    let _ = cantus_ui::editor::commit::run_cleanup(&mut a.app);
    assert!(b_files[0].exists(), "B's edit survives A's cleanup");
    // A's own edits are in Audio/song (its file is song.cantus).
    a.do_edit(Edit::Gain { db: -2.0 });
    assert_eq!(
        edit_files_under(&a.dir.path().join("Audio").join("song")).len(),
        1
    );
}

#[test]
fn a_file_a_sibling_project_refers_to_is_never_deleted() {
    let mut a = rig(&take());
    a.open();
    a.do_edit(Edit::Gain { db: -6.0 });
    let file = edit_files_under(&a.dir.path().join("Audio").join("song"))[0].clone();
    // Saved as a copy beside it: the copy refers to A's edit file.
    cantus_project::save(a.app.project(), &a.dir.path().join("copy.cantus"), "test").unwrap();
    // A undoes and edits again: the first file is no longer A's.
    a.app.undo();
    a.settle();
    a.do_edit(Edit::Gain { db: -9.0 });
    let plan = cantus_ui::editor::commit::plan_cleanup(&a.app);
    assert!(
        plan.files.iter().all(|(p, _)| *p != file),
        "copy.cantus still plays it: {:?}",
        plan.files
    );
    let _ = cantus_ui::editor::commit::run_cleanup(&mut a.app);
    assert!(file.exists());
}

#[test]
fn a_pending_agent_edit_and_a_running_one_are_never_swept() {
    let mut r = rig(&take());
    let id = r.clip.0;
    // An agent's edit, planned and not yet made (its card waits): pending, referenced.
    let _plan = call(
        &mut r,
        "edit_audio",
        serde_json::json!({"clip": id, "operation": "gain", "gain_db": -3.0}),
    )
    .expect("planned");
    let pending = r.audio_files();
    assert_eq!(pending.len(), 1);
    let plan = cantus_ui::editor::commit::plan_cleanup(&r.app);
    assert!(plan.files.is_empty(), "{:?}", plan.files);
    assert_eq!(cantus_ui::editor::commit::sweep(&r.app), 0);
    assert!(pending[0].exists());
    // While an agent's edit is being made, nothing is swept at all.
    r.app.editor.pending.clear();
    r.app
        .editor
        .agent_jobs
        .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    assert_eq!(cantus_ui::editor::commit::sweep(&r.app), 0);
    assert!(pending[0].exists());
    r.app
        .editor
        .agent_jobs
        .fetch_sub(1, std::sync::atomic::Ordering::SeqCst);
    assert_eq!(
        cantus_ui::editor::commit::sweep(&r.app),
        1,
        "now it is no one's"
    );
}

#[test]
fn a_half_written_file_is_cleaned_only_when_a_cut_session_left_it() {
    let r = rig(&take());
    let dir = r.dir.path().join("Audio").join("song");
    std::fs::create_dir_all(&dir).unwrap();
    let fresh = dir.join("edit-0123456789abcdef.wav.part");
    let old = dir.join("edit-fedcba9876543210.wav.part");
    std::fs::write(&fresh, b"x").unwrap();
    std::fs::write(&old, b"x").unwrap();
    let long_ago = std::time::SystemTime::now() - std::time::Duration::from_secs(24 * 3600);
    std::fs::File::options()
        .write(true)
        .open(&old)
        .unwrap()
        .set_modified(long_ago)
        .unwrap();
    let plan = cantus_ui::editor::commit::plan_cleanup(&r.app);
    let listed: Vec<_> = plan.files.iter().map(|f| f.0.clone()).collect();
    assert!(listed.contains(&old), "{listed:?}");
    assert!(
        !listed.contains(&fresh),
        "one being written now is never listed"
    );
    cantus_ui::editor::commit::tidy(&r.app);
    assert!(!old.exists() && fresh.exists());
}

#[test]
fn remove_unused_audio_is_always_asked_first() {
    let def = cantus_ui::ai_tools::find("remove_unused_audio").expect("the tool");
    assert_eq!(def.class, cantus_ai::ToolClass::Destructive);
    // A host run the pump asks about before it runs — files only or not.
    assert!(matches!(def.run, cantus_ai::ToolRun::HostSaid { .. }));
}

#[test]
fn a_clip_an_ara_plugin_edits_is_not_opened_and_never_handed_its_binding() {
    let mut r = rig(&take());
    let mut region = clip_of(&r).audio.unwrap();
    region.ara = Some(Box::new(cantus_core::ara::AraClip {
        plugin: cantus_core::ara::AraPlugin {
            id: "0".repeat(32),
            name: "Melody tool".into(),
            vendor: String::new(),
            path: String::new(),
            factory_id: String::new(),
        },
        binding: 1,
        archive: None,
    }));
    assert!(r.app.exec(Command::SetClipAudio {
        track: r.track,
        clip: r.clip,
        audio: Some(region),
    }));
    editor::open(&mut r.app, r.track, r.clip);
    assert!(r.app.editor.session.is_none());
    assert!(r.app.status.text.contains("ARA"), "{}", r.app.status.text);
    // And an agent is refused too.
    let id = r.clip.0;
    let e = call(
        &mut r,
        "edit_audio",
        serde_json::json!({"clip": id, "operation": "invert"}),
    );
    assert!(matches!(e, Err(cantus_ai::ToolError::Refused(_))));
    assert!(r.audio_files().is_empty());
}

#[test]
fn in_the_recording_mode_an_edit_is_refused_before_any_work_in_the_modes_words() {
    let mut r = rig(&take());
    r.open();
    r.app
        .bus
        .set_record_lock(Some(cantus_core::record_mode::EditLock::default()));
    editor::act(&mut r.app, Act::Edit(Edit::Gain { db: -3.0 }));
    assert!(!r.app.editor.busy(), "no job started");
    assert_eq!(r.status(), cantus_ui::i18n::t("record.notice"));
    assert!(r.audio_files().is_empty());
}
