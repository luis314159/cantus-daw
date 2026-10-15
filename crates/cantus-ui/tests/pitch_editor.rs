//! The pitch editor through the whole window (round 28, ADR 0054): opened from an audio
//! clip (a double-click, *Edit pitch*), the take analysed off the window's thread, its
//! notes drawn as blobs; a drag moves a note's pitch (one undo step, undone exactly),
//! the Split tool cuts one, Ctrl+J joins, the macros correct the take, *Original* compares,
//! MIDI out and *Double* make their clips — and the engine plays the render. (The boot
//! path's test, which points `CANTUS_DATA_DIR` elsewhere, has its own binary:
//! `pitch_boot.rs`.)

use cantus_core::pitch::PitchOp;
use cantus_core::*;
use cantus_dsp::pitch::midi_to_hz;
use cantus_dsp::pitch::testkit::{self, Vowel};
use cantus_engine::EngineHandle;
use cantus_ui::app::CentralView;
use cantus_ui::clips::ClipEditor;
use cantus_ui::views::workspace::Pane;
use cantus_ui::CantusApp;
use egui::{pos2, vec2, Event, Key, Modifiers, Pos2, Rect};
use std::time::{Duration, Instant};

const SR: u32 = 48_000;

fn data_dir() {
    static DIR: std::sync::OnceLock<std::path::PathBuf> = std::sync::OnceLock::new();
    let d = DIR.get_or_init(|| {
        let d = std::env::temp_dir().join(format!("cantus-pitch-ui-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&d);
        d
    });
    std::env::set_var("CANTUS_DATA_DIR", d);
}

/// Four notes sung out of tune (A3 +35, C4 −28, E4 +22, D4 −40 cents), with gaps.
fn voice() -> Vec<f32> {
    let notes = [
        (0.10, 0.70, 57.35f32),
        (0.85, 1.45, 59.72),
        (1.60, 2.30, 64.22),
        (2.45, 3.05, 61.60),
    ];
    testkit::voice(SR as f32, 3.2, Vowel::A, 0.5, move |t| {
        notes
            .iter()
            .find(|(a, b, _)| t >= *a && t < *b)
            .map_or(0.0, |(_, _, m)| midi_to_hz(*m))
    })
}

fn write_wav(path: &std::path::Path, x: &[f32]) {
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: SR,
        bits_per_sample: 32,
        sample_format: hound::SampleFormat::Float,
    };
    let mut w = hound::WavWriter::create(path, spec).unwrap();
    for s in x {
        w.write_sample(*s).unwrap();
    }
    w.finalize().unwrap();
}

/// A project with a Keys track and a Voice track holding the take from bar 1.
fn project(path: &std::path::Path, frames: u64) -> Project {
    let mut p = Project::new("pitch");
    p.tempo_bpm = 120.0;
    let keys = p.make_track("Keys");
    p.tracks.push(keys);
    let mut v = p.make_track("Voice");
    v.input = Some(TrackInput::mono(0));
    let id = AudioSourceId(p.alloc_id());
    p.audio_pool.push(AudioSource {
        id,
        name: "take".into(),
        path: path.to_string_lossy().into_owned(),
        sample_rate: SR,
        channels: 1,
        frames,
        sha256: None,
        recorded: true,
    });
    let c = p.make_audio_clip("Take 1", id, Ticks::ZERO, Ticks::beats(8));
    v.clips.push(c);
    p.tracks.push(v);
    p
}

struct Rig {
    ctx: egui::Context,
    app: CantusApp,
    size: egui::Vec2,
    frame: u64,
    pointer: Option<Pos2>,
    texts: Vec<(String, Rect)>,
    _dir: tempfile::TempDir,
}

impl Rig {
    fn new() -> Rig {
        data_dir();
        cantus_ui::i18n::set_locale(cantus_ui::i18n::Locale::En);
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("take.wav");
        let x = voice();
        write_wav(&path, &x);
        let ctx = egui::Context::default();
        cantus_ui::design::install_fonts(&ctx);
        cantus_ui::design::install_style(&ctx);
        cantus_ui::theme::apply(&cantus_ui::theme_family::cantus_night(), &ctx);
        let mut app = CantusApp::from_parts(
            Some(project(&path, x.len() as u64)),
            EngineHandle::dummy(),
            false,
        );
        app.settings_path = dir.path().join("settings.json");
        app.layouts_path = dir.path().join("layouts.json");
        let mut r = Rig {
            ctx,
            app,
            size: vec2(1600.0, 900.0),
            frame: 0,
            pointer: None,
            texts: Vec::new(),
            _dir: dir,
        };
        r.idle(3);
        r
    }

    fn frame(&mut self, events: Vec<Event>) {
        self.frame += 1;
        let mut ev = Vec::new();
        if let Some(p) = self.pointer {
            ev.push(Event::PointerMoved(p));
        }
        ev.extend(events);
        let input = egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, self.size)),
            time: Some(self.frame as f64 / 60.0),
            predicted_dt: 1.0 / 60.0,
            events: ev,
            ..Default::default()
        };
        let app = &mut self.app;
        let mut out = self.ctx.run_ui(input, |ui| app.frame(ui));
        self.texts.clear();
        for c in &out.shapes {
            if let egui::epaint::Shape::Text(t) = &c.shape {
                self.texts
                    .push((t.galley.text().to_string(), t.visual_bounding_rect()));
            }
        }
        out.textures_delta.clear();
    }

    fn idle(&mut self, n: usize) {
        for _ in 0..n {
            self.frame(Vec::new());
        }
    }

    fn button(at: Pos2, pressed: bool, modifiers: Modifiers) -> Event {
        Event::PointerButton {
            pos: at,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers,
        }
    }

    fn click(&mut self, at: Pos2) {
        self.idle(30);
        self.pointer = Some(at);
        self.idle(2);
        self.frame(vec![Rig::button(at, true, Modifiers::NONE)]);
        self.frame(vec![Rig::button(at, false, Modifiers::NONE)]);
        self.idle(2);
    }

    fn drag(&mut self, from: Pos2, to: Pos2) {
        self.idle(30);
        self.pointer = Some(from);
        self.idle(2);
        self.frame(vec![Rig::button(from, true, Modifiers::NONE)]);
        for i in 1..=8 {
            let p = from + (to - from) * (i as f32 / 8.0);
            self.pointer = Some(p);
            self.frame(Vec::new());
        }
        self.frame(vec![Rig::button(to, false, Modifiers::NONE)]);
        self.idle(2);
    }

    fn key(&mut self, key: Key, modifiers: Modifiers) {
        self.frame(vec![
            Event::ModifiersChanged(modifiers),
            Event::Key {
                key,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers,
            },
        ]);
        self.frame(vec![
            Event::Key {
                key,
                physical_key: None,
                pressed: false,
                repeat: false,
                modifiers,
            },
            Event::ModifiersChanged(Modifiers::NONE),
        ]);
        self.idle(2);
    }

    fn voice(&self) -> (TrackId, ClipId) {
        let t = &self.app.project().tracks[1];
        (t.id, t.clips[0].id)
    }

    /// Open the editor and wait for the take's analysis.
    fn open(&mut self) {
        let (t, c) = self.voice();
        assert_eq!(self.app.open_clip_editor(t, c), Some(ClipEditor::Pitch));
        let t0 = Instant::now();
        while self.app.engine.samples.pitch().busy() && t0.elapsed() < Duration::from_secs(30) {
            std::thread::sleep(Duration::from_millis(5));
        }
        self.idle(4);
    }

    fn blob(&self, i: usize) -> Rect {
        self.app.pitch.view.blobs[i].1
    }

    /// Where a word was drawn last frame.
    fn word(&self, w: &str) -> Pos2 {
        self.texts
            .iter()
            .find(|(t, _)| t == w)
            .map(|(_, r)| r.center())
            .unwrap_or_else(|| panic!("{w:?} not drawn: {:?}", self.texts))
    }

    /// Where a word starting with `w` was drawn last frame.
    fn word_starting(&self, w: &str) -> Pos2 {
        self.texts
            .iter()
            .find(|(t, _)| t.starts_with(w))
            .map(|(_, r)| r.center())
            .unwrap_or_else(|| panic!("{w:?} not drawn: {:?}", self.texts))
    }

    fn stored(&self) -> Option<cantus_core::pitch::PitchEdit> {
        let (t, c) = self.voice();
        self.app.stored_pitch(t, c).cloned()
    }
}

#[test]
fn a_double_click_on_an_audio_clip_opens_its_notes() {
    let mut r = Rig::new();
    r.open();
    assert_eq!(r.app.central, CentralView::PianoRoll);
    assert!(r.app.workspace.is_visible(Pane::PianoRoll));
    assert_eq!(r.app.pitch.view.blobs.len(), 4, "four notes drawn");
    let g = r.app.pitch.view.geo.expect("drawn");
    for (_, b) in &r.app.pitch.view.blobs {
        assert!(g.grid.intersects(*b), "every note in view after the fit");
    }
    // The bar says what the take is.
    assert!(
        r.texts.iter().any(|(t, _)| t.starts_with("4 notes")),
        "{:?}",
        r.texts
    );
    assert!(r.stored().is_none(), "looking stores nothing");
}

#[test]
fn a_drag_moves_a_notes_pitch_in_one_step_and_undo_gives_it_back() {
    let mut r = Rig::new();
    r.open();
    let before = r.app.project().clone();
    r.app.pitch.view.snap = cantus_ui::views::pitch::Snap::Semitone;
    let b = r.blob(1);
    let row = r.app.pitch.view.geo.unwrap().row_h;
    r.drag(b.center(), b.center() - vec2(0.0, 2.0 * row));
    let e = r.stored().expect("the first edit stores the notes");
    let n = &e.notes[1];
    assert_eq!(n.pitch - n.home(), 2.0, "two semitones up");
    assert_eq!(r.app.bus.undo_label_key(), Some(PitchOp::Pitch.label_key()));
    assert_eq!(
        r.app.bus.history().len(),
        1,
        "one undo step for the whole drag"
    );
    r.app.undo();
    assert_eq!(r.app.project(), &before, "undone exactly");
    r.app.redo();
    assert_eq!(r.stored(), Some(e));
}

#[test]
fn split_join_macros_and_the_original() {
    let mut r = Rig::new();
    r.open();
    // Split: the tool, a click inside the third note.
    r.app.pitch.view.tool = cantus_ui::views::pitch::Tool::Split;
    let b = r.blob(2);
    r.click(b.center());
    assert_eq!(r.stored().unwrap().notes.len(), 5);
    assert_eq!(r.app.bus.undo_label_key(), Some(PitchOp::Split.label_key()));
    // Join them again: select both halves, Ctrl+J.
    r.app.pitch.view.tool = cantus_ui::views::pitch::Tool::Edit;
    let e = r.stored().unwrap();
    r.app.pitch.view.selected = vec![e.notes[2].id, e.notes[3].id];
    r.pointer = Some(r.blob(0).center());
    r.key(Key::J, Modifiers::COMMAND);
    assert_eq!(r.stored().unwrap().notes.len(), 4);
    // Correct pitch on every note (none selected): the key on the macro card.
    r.app.pitch.view.selected.clear();
    r.idle(2);
    let at = r.word("Correct pitch");
    r.click(at);
    let e = r.stored().unwrap();
    assert!(e.notes.iter().all(|n| n.centre == 1.0), "{e:#?}");
    // The clip now plays a render, made on the pitch service's worker.
    let t0 = Instant::now();
    r.app.sync_engine();
    while r.app.engine.samples.pitch_pending() > 0 && t0.elapsed() < Duration::from_secs(30) {
        std::thread::sleep(Duration::from_millis(5));
        r.app.sync_engine();
    }
    assert!(r.app.engine.samples.pitch_held() > 0);
    // Original: A/B.
    let at = r.word_starting("ORIGINAL");
    r.click(at);
    assert!(r.stored().unwrap().bypass);
    assert_eq!(
        r.app.bus.undo_label_key(),
        Some(PitchOp::Compare.label_key())
    );
}

#[test]
fn midi_out_and_the_double_make_their_clips() {
    let mut r = Rig::new();
    r.open();
    let (t, c) = r.voice();
    let tracks = r.app.project().tracks.len();
    let (mt, mc) = r.app.pitch_to_midi(t, c, None).expect("a MIDI clip");
    let clip = r.app.project().track(mt).unwrap().clip(mc).unwrap().clone();
    let pitches: Vec<u8> = clip.notes.iter().map(|n| n.pitch).collect();
    assert_eq!(
        pitches,
        vec![57, 60, 64, 62],
        "the notes as they sound, nearest"
    );
    assert_eq!(r.app.project().tracks.len(), tracks + 1);
    // Onto the Keys track instead.
    let keys = r.app.project().tracks[0].id;
    let (on, _) = r.app.pitch_to_midi(t, c, Some(keys)).unwrap();
    assert_eq!(on, keys);
    // Double a third above, in A minor / C major.
    let (dt, dc) = r
        .app
        .pitch_double(t, c, cantus_core::pitch::midi::Interval::Third)
        .unwrap();
    let d = r.app.stored_pitch(dt, dc).unwrap().clone();
    let outs: Vec<f32> = d.notes.iter().map(|n| n.out_centre().round()).collect();
    assert_eq!(
        outs,
        vec![60.0, 64.0, 67.0, 65.0],
        "a diatonic third above each"
    );
    assert_eq!(r.app.bus.undo_label_key(), Some("cmd.pitch.double"));
    r.app.undo();
    assert!(r.app.project().track(dt).is_none(), "one step");
}

#[test]
fn every_pitch_string_is_in_both_languages() {
    for key in [
        "pitch.analysing",
        "pitch.original",
        "pitch.menu.edit",
        "pitch.macro.apply_pitch",
    ] {
        for l in cantus_ui::i18n::Locale::ALL {
            cantus_ui::i18n::set_locale(l);
            assert_ne!(cantus_ui::i18n::t(key), key, "{l:?} {key}");
        }
    }
    for op in PitchOp::ALL {
        cantus_ui::i18n::set_locale(cantus_ui::i18n::Locale::Es);
        assert_ne!(cantus_ui::i18n::t(op.label_key()), op.label_key());
    }
    cantus_ui::i18n::set_locale(cantus_ui::i18n::Locale::En);
    let _ = pos2(0.0, 0.0);
}

/// The demo project for the owner's ears opens, its corrected voice plays a render (not
/// the file), the take as sung plays the file, and the double plays a render of its own.
#[test]
fn the_demo_project_opens_and_plays_its_edits() {
    let dir = tempfile::tempdir().unwrap();
    let path = cantus_ui::pitch::demo::write_demo(dir.path()).unwrap();
    let p = cantus_project::load(&path, true).unwrap().project;
    let names: Vec<&str> = p.tracks.iter().map(|t| t.name.as_str()).collect();
    assert_eq!(names.len(), 5, "{names:?}");
    let e = p.tracks[0].clips[0]
        .audio
        .as_ref()
        .unwrap()
        .pitch
        .as_deref()
        .unwrap();
    assert!(e.audible() && e.notes.len() >= 7, "{} notes", e.notes.len());
    assert!(!p.tracks[3].clips[0].notes.is_empty(), "the melody");
    let mut app = CantusApp::from_parts(Some(p), EngineHandle::dummy(), false);
    let proj = app.project().clone();
    let snap = app.engine.compile_for_render(&proj, 1);
    let file: Vec<f32> = cantus_ui::views::fx::tonus::demo::voice_line();
    let data = |i: usize| snap.tracks[i].audio[0].data.clone().expect("attached");
    let differs = |d: &cantus_core::SampleData| {
        d.frames
            .iter()
            .zip(&file)
            .any(|(a, b)| a.to_bits() != b.to_bits())
    };
    // Muted tracks are not compiled: unmute to look at the double and the take as sung.
    assert!(differs(&data(0)), "the corrected voice plays its render");
    for i in [1usize, 2] {
        app.exec(Command::SetTrackMute {
            id: app.project().tracks[i].id,
            muted: false,
        });
    }
    let proj = app.project().clone();
    let snap = app.engine.compile_for_render(&proj, 2);
    let double = snap.tracks[2].audio[0].data.clone().unwrap();
    assert!(differs(&double), "the double plays a render");
    let corrected = snap.tracks[0].audio[0].data.clone().unwrap();
    assert!(
        !std::sync::Arc::ptr_eq(&double, &corrected)
            && double
                .frames
                .iter()
                .zip(corrected.frames.iter())
                .any(|(a, b)| a != b),
        "a render of its own"
    );
    let sung = snap.tracks[1].audio[0].data.clone().unwrap();
    assert!(!differs(&sung), "the take as sung plays the file");
}

/// MIDI out by hand: the chip dragged from the editor's bar onto the Keys track in the
/// arrangement leaves the melody there, at the voice's place.
#[test]
fn the_midi_chip_dragged_onto_a_track_leaves_the_melody_there() {
    let mut r = Rig::new();
    r.open();
    let keys = r.app.project().tracks[0].id;
    let before = r.app.project().track(keys).unwrap().clips.len();
    let chip = r.word("MIDI");
    let geo = r.app.arrangement.geometry().expect("the arrangement drawn");
    let onto = pos2(geo.x(6.0), geo.row_y(0));
    r.drag(chip, onto);
    let clips = &r.app.project().track(keys).unwrap().clips;
    assert_eq!(clips.len(), before + 1, "a clip on Keys");
    let pitches: Vec<u8> = clips
        .last()
        .unwrap()
        .notes
        .iter()
        .map(|n| n.pitch)
        .collect();
    assert_eq!(pitches, vec![57, 60, 64, 62]);
    assert_eq!(r.app.bus.undo_label_key(), Some("cmd.pitch.midi"));
}

/// A clip played backwards: MIDI out reads it as it sounds, last note first.
#[test]
fn a_reversed_clip_gives_its_notes_as_they_sound() {
    let mut r = Rig::new();
    r.open();
    let (t, c) = r.voice();
    let mut a = r
        .app
        .project()
        .track(t)
        .unwrap()
        .clip(c)
        .unwrap()
        .audio
        .clone()
        .unwrap();
    a.reverse = true;
    assert!(r.app.exec(Command::SetClipAudio {
        track: t,
        clip: c,
        audio: Some(a),
    }));
    let (mt, mc) = r.app.pitch_to_midi(t, c, None).unwrap();
    let clip = r.app.project().track(mt).unwrap().clip(mc).unwrap();
    let pitches: Vec<u8> = clip.notes.iter().map(|n| n.pitch).collect();
    assert_eq!(pitches, vec![62, 64, 60, 57]);
}

/// The edit keys act on the notes while the editor has the focus: Ctrl+A, ↑ a step of the
/// key, Shift+↑ an octave, Alt+↑ ten cents, Esc lets the notes go (never the clip).
#[test]
fn the_edit_keys_act_on_the_notes() {
    let mut r = Rig::new();
    r.open();
    r.pointer = Some(r.blob(0).center());
    r.key(Key::A, Modifiers::COMMAND);
    assert_eq!(r.app.pitch.view.selected.len(), 4, "Ctrl+A");
    r.app.pitch.view.selected = vec![r.app.pitch.view.blobs[0].0];
    r.key(Key::ArrowUp, Modifiers::NONE);
    let n = r.stored().unwrap().notes[0].clone();
    // A3 in A minor: a step up is B3.
    assert_eq!(n.pitch, 59.0, "a step of the key");
    r.key(Key::ArrowUp, Modifiers::SHIFT);
    assert_eq!(r.stored().unwrap().notes[0].pitch, 71.0, "an octave");
    r.key(Key::ArrowDown, Modifiers::ALT);
    assert!(
        (r.stored().unwrap().notes[0].pitch - 70.9).abs() < 1e-4,
        "ten cents"
    );
    let clip = r.app.selected_clip;
    r.key(Key::Escape, Modifiers::NONE);
    assert!(r.app.pitch.view.selected.is_empty());
    assert_eq!(r.app.selected_clip, clip, "the clip stays in the editor");
}

// ---- review 1 ------------------------------------------------------------------------

/// Store `e` on the voice clip (as a commit would).
fn store(r: &mut Rig, e: cantus_core::pitch::PitchEdit) {
    let (t, c) = r.voice();
    assert!(r.app.commit_pitch(t, c, Some(e), PitchOp::Notes, false));
    r.idle(2);
}

/// ↑/↓ from a note moved off the key land on the key's next note (scale snap).
#[test]
fn a_key_step_from_an_off_key_note_lands_on_the_key() {
    let mut r = Rig::new();
    r.open();
    let (t, c) = r.voice();
    let take = r.app.pitch_take(t, c);
    let mut e = r.app.pitch_edit(t, c, &take).unwrap();
    // The take's key has no C# (A minor or C major): the first note moved onto C#4.
    let id = e.notes[0].id;
    cantus_core::pitch::edit::set_pitch(&mut e, id, 61.0);
    store(&mut r, e);
    r.app.pitch.view.selected = vec![id];
    r.pointer = Some(r.blob(0).center());
    r.key(Key::ArrowUp, Modifiers::NONE);
    assert_eq!(r.stored().unwrap().notes[0].pitch, 62.0, "C# up: D");
    let mut e = r.stored().unwrap();
    cantus_core::pitch::edit::set_pitch(&mut e, id, 61.0);
    store(&mut r, e);
    r.key(Key::ArrowDown, Modifiers::NONE);
    assert_eq!(r.stored().unwrap().notes[0].pitch, 60.0, "C# down: C");
}

/// Alt+↑/↓ (10 cents) works whenever the editor has the focus, the pointer anywhere.
#[test]
fn ten_cents_with_the_pointer_away_from_the_notes() {
    let mut r = Rig::new();
    r.open();
    r.app.pitch.view.selected = vec![r.app.pitch.view.blobs[0].0];
    let before = r.stored().map(|e| e.notes[0].pitch);
    r.pointer = Some(pos2(800.0, 20.0));
    r.key(Key::ArrowDown, Modifiers::ALT);
    let after = r.stored().expect("moved").notes[0].pitch;
    assert!(
        (after - (before.unwrap_or(57.0) - 0.1)).abs() < 1e-4,
        "{after}"
    );
}

/// A reversed clip over a slice of the take opens fitted to its slice, its ruler counts
/// bars, and play from the selection starts a beat before the note's first edge.
#[test]
fn a_reversed_slice_opens_on_its_slice_with_its_bars() {
    let mut r = Rig::new();
    let (t, c) = r.voice();
    // The window: one second from 0.8 s (beats 1.6 … 3.6), played backwards.
    let mut a = r
        .app
        .project()
        .track(t)
        .unwrap()
        .clip(c)
        .unwrap()
        .audio
        .clone()
        .unwrap();
    a.reverse = true;
    assert!(r.app.exec(Command::SetClipAudio {
        track: t,
        clip: c,
        audio: Some(a),
    }));
    assert!(r.app.exec(Command::SetClipBounds {
        track: t,
        clip: c,
        start: Ticks::beats(4),
        length: Ticks::beats(2),
    }));
    assert!(r.app.exec(Command::SetClipOffset {
        track: t,
        clip: c,
        offset: Ticks::beats(1) + Ticks::beats(1) / 2 + Ticks(48),
    }));
    r.open();
    let g = r.app.pitch.view.geo.unwrap();
    let shown = g.grid.width() as f64 / g.px_per_frame / SR as f64;
    assert!(shown < 1.5, "fitted to its second, not the take: {shown} s");
    let ruler: Vec<&String> = r
        .texts
        .iter()
        .filter(|(_, rc)| g.ruler.intersects(*rc))
        .map(|(t, _)| t)
        .collect();
    assert!(
        ruler.iter().any(|t| t.parse::<u32>().is_ok()),
        "bar numbers: {ruler:?}"
    );
    let e = r.app.pitch_edit(t, c, &r.app.pitch_take(t, c)).unwrap();
    let tm = r.app.clip_time(t, c).unwrap();
    let n = e
        .notes
        .iter()
        .find(|n| {
            let (x, y) = (tm.tick(n.start as f64), tm.tick(n.end as f64));
            x.min(y) >= Ticks::beats(4) && x.max(y) <= Ticks::beats(6)
        })
        .expect("a note inside the window")
        .clone();
    r.app.pitch.view.selected = vec![n.id];
    r.app.pitch_play_from_selection(t, c);
    let first = tm.tick(n.start as f64).min(tm.tick(n.end as f64));
    let want = (first - Ticks::beats(1)).max(Ticks::ZERO);
    assert_eq!(r.app.engine.position(), want);
}

/// MIDI out gives the notes the clip plays: a window over the middle of the take gives
/// its two notes, at their places.
#[test]
fn midi_out_gives_the_notes_the_clip_plays() {
    let mut r = Rig::new();
    let (t, c) = r.voice();
    // The take's second and third notes are sung 0.85 … 2.30 s: beats 1.7 … 4.6.
    assert!(r.app.exec(Command::SetClipBounds {
        track: t,
        clip: c,
        start: Ticks::beats(8),
        length: Ticks::beats(3),
    }));
    assert!(r.app.exec(Command::SetClipOffset {
        track: t,
        clip: c,
        offset: Ticks::beats(1) + Ticks::beats(1) / 2,
    }));
    r.open();
    let (mt, mc) = r.app.pitch_to_midi(t, c, None).unwrap();
    let clip = r.app.project().track(mt).unwrap().clip(mc).unwrap().clone();
    let pitches: Vec<u8> = clip.notes.iter().map(|n| n.pitch).collect();
    assert_eq!(pitches, vec![60, 64], "only the notes in the window");
    assert!(clip.start >= Ticks::beats(8) - Ticks::beats(4));
    for n in &clip.notes {
        let at = clip.start + n.start;
        assert!(
            at >= Ticks::beats(8) && at + n.length <= Ticks::beats(11),
            "{at:?}"
        );
    }
}

/// Glue refuses two halves of a take whose pitch edits differ (one half's corrections
/// would be lost); halves with the same edit still glue.
#[test]
fn glue_keeps_pitch_edits_or_refuses() {
    let mut r = Rig::new();
    r.open();
    let (t, c) = r.voice();
    let whole = r.app.project().track(t).unwrap().clip(c).unwrap().clone();
    let (mut left, mut right) = (whole.clone(), whole.clone());
    left.length = Ticks::beats(4);
    right.id = ClipId(9_999);
    right.start = Ticks::beats(4);
    right.offset = Ticks::beats(4);
    right.length = Ticks::beats(4);
    assert!(
        cantus_ui::audio_clips::glue(t, &[&left, &right]).is_some(),
        "the same edit"
    );
    let take = r.app.pitch_take(t, c);
    let mut e = r.app.pitch_edit(t, c, &take).unwrap();
    let id = e.notes[3].id;
    cantus_core::pitch::edit::move_pitch(&mut e, &[id], 2.0);
    right.audio.as_mut().unwrap().pitch = Some(Box::new(e));
    assert!(
        cantus_ui::audio_clips::glue(t, &[&left, &right]).is_none(),
        "edits differ"
    );
}

/// The key in words: one notation with the notes (letters), the mode in lower case.
#[test]
fn the_key_reads_as_the_notes_do() {
    use cantus_core::pitch::PitchKey;
    use cantus_ui::i18n::{set_locale, Locale};
    let a_minor = PitchKey { tonic: 9, scale: 2 };
    let g_dorian = PitchKey { tonic: 7, scale: 5 };
    set_locale(Locale::Es);
    assert_eq!(cantus_ui::views::pitch::key_words(a_minor), "A menor");
    assert_eq!(cantus_ui::views::pitch::key_words(g_dorian), "G dórico");
    set_locale(Locale::En);
    assert_eq!(cantus_ui::views::pitch::key_words(a_minor), "A minor");
    assert_eq!(cantus_ui::i18n::t("pitch.macro.pitch"), "Pitch");
    set_locale(Locale::Es);
    assert_eq!(cantus_ui::i18n::t("pitch.macro.pitch"), "Afinación");
    set_locale(Locale::En);
}

/// The note panel never spills into the macros, at any width: narrower than its cards, it
/// scrolls sideways (its knobs keep their words whole).
#[test]
fn the_note_panel_fits_its_width() {
    for (w, scrolled) in [(1600.0f32, false), (1000.0, false), (640.0, true)] {
        let mut r = Rig::new();
        r.size = vec2(w, 900.0);
        r.app.workspace.hide(Pane::Inspector);
        r.idle(3);
        r.open();
        r.idle(3);
        let p = r.app.pitch.view.panel.expect("drawn");
        assert!(p.note_used.right() <= p.note.right() + 1.0, "{w}: {p:?}");
        assert!(
            p.macros_used.right() <= p.macros.right() + 1.0,
            "{w}: {p:?}"
        );
        assert!(p.note.right() < p.macros.left(), "{w}: {p:?}");
        assert_eq!(p.scrolled, scrolled, "{w}: {p:?}");
    }
}

/// At a width the bar cannot hold, the groups it has no room for fold into ⋯, whose
/// menu offers them: nothing on the bar is out of reach.
#[test]
fn a_narrow_bar_folds_into_its_more_menu() {
    cantus_ui::i18n::set_locale(cantus_ui::i18n::Locale::Es);
    let mut r = Rig::new();
    cantus_ui::i18n::set_locale(cantus_ui::i18n::Locale::Es);
    r.size = vec2(900.0, 900.0);
    r.idle(3);
    r.open();
    r.idle(3);
    let folded = r.app.pitch.view.bar_folded;
    assert!(folded.iter().any(|f| *f), "{folded:?}");
    let more = r.word("\u{2026}");
    r.click(more);
    for (i, words) in [
        (0usize, &["Unir", "Restablecer"][..]),
        (
            1,
            &[
                "Reproducir desde la selección",
                "Melodía a una pista MIDI nueva",
            ][..],
        ),
        (2, &["1/16", "1/8T"][..]),
    ] {
        if folded[i] {
            for w in words {
                assert!(r.texts.iter().any(|(t, _)| t == w), "{w}: {:?}", r.texts);
            }
        }
    }
    cantus_ui::i18n::set_locale(cantus_ui::i18n::Locale::En);
    let mut wide = Rig::new();
    wide.open();
    assert_eq!(
        wide.app.pitch.view.bar_folded, [false; 3],
        "room for all at 1 600"
    );
}

/// Quantise to another track: each note's start onto the nearest of the Keys track's
/// notes within an eighth; a note with none near stays.
#[test]
fn quantise_onto_another_tracks_notes() {
    let mut r = Rig::new();
    let keys = r.app.project().tracks[0].id;
    let mut c = r
        .app
        .bus
        .new_clip("hits".to_string(), Ticks::ZERO, Ticks::beats(8));
    for b in [0.25f64, 1.75, 3.0] {
        let n = r
            .app
            .bus
            .new_note(Ticks((b * 960.0) as i64), Ticks(120), 60, 100);
        c.notes.push(n);
    }
    assert!(r.app.exec(Command::InsertClip {
        track: keys,
        clip: Box::new(c),
    }));
    r.open();
    let (t, cl) = r.voice();
    assert!(r.app.pitch_onset_tracks(t).iter().any(|(k, _)| *k == keys));
    r.app.pitch.view.quantise_to = Some(keys);
    let take = r.app.pitch_take(t, cl);
    let e = r.app.pitch_edit(t, cl, &take).unwrap();
    let ids: Vec<u32> = e.notes.iter().map(|n| n.id).collect();
    cantus_ui::views::pitch::quantise(&mut r.app, t, cl, &e, &ids);
    let q = r.stored().expect("quantised");
    let want = [0.125f64, 0.875, 1.5];
    for (n, w) in q.notes.iter().zip(want) {
        assert!(
            (n.start as f64 - w * SR as f64).abs() <= 1.0,
            "{} vs {w}",
            n.start
        );
    }
    assert_eq!(q.notes[3].start, e.notes[3].start, "no note near: it stays");
}

/// The clip's menu makes MIDI and a double without opening the editor; before the take is
/// analysed it says so instead of doing nothing.
#[test]
fn the_clip_menu_makes_midi_and_a_double() {
    let mut r = Rig::new();
    let (t, c) = r.voice();
    let tracks = r.app.project().tracks.len();
    r.app.pitch_make(t, c, None);
    let analysed = r.app.project().tracks.len() > tracks;
    if !analysed {
        assert!(
            r.app.status.text.contains("analys"),
            "{}",
            r.app.status.text
        );
    }
    r.open();
    let n = r.app.project().tracks.len();
    r.app.pitch_make(t, c, None);
    r.app
        .pitch_make(t, c, Some(cantus_core::pitch::midi::Interval::Fifth));
    assert_eq!(r.app.project().tracks.len(), n + 2);
}

/// The inspector's *Edit pitch* says what it does on the help line.
#[test]
fn the_inspectors_pitch_button_explains_itself() {
    let mut r = Rig::new();
    let (t, c) = r.voice();
    r.app.select_clip(t, c);
    r.app.show_pane(Pane::Inspector);
    r.idle(4);
    let at = r.word("Edit pitch");
    r.pointer = Some(at);
    r.idle(3);
    let tip = cantus_ui::i18n::t("pitch.inspector.edit_tip");
    assert!(
        r.texts.iter().any(|(w, _)| w.contains(&tip[..20])),
        "{:?}",
        r.texts
    );
}

/// An idle editor rebuilds nothing: the edit, its curves and its time map are built once
/// per edit, not once per frame.
#[test]
fn an_idle_editor_builds_its_curves_once() {
    let mut r = Rig::new();
    r.open();
    let (t, c) = r.voice();
    let take = r.app.pitch_take(t, c);
    let mut e = r.app.pitch_edit(t, c, &take).unwrap();
    let ids: Vec<u32> = e.notes.iter().map(|n| n.id).collect();
    cantus_core::pitch::edit::correct_drift(&mut e, &ids, 0.5);
    store(&mut r, e);
    r.idle(2);
    let before = r.app.pitch.view.builds;
    r.idle(12);
    assert_eq!(
        r.app.pitch.view.builds, before,
        "nothing rebuilt while idle"
    );
    let mut e = r.stored().unwrap();
    cantus_core::pitch::edit::move_pitch(&mut e, &[ids[0]], 1.0);
    store(&mut r, e);
    r.idle(3);
    assert_eq!(r.app.pitch.view.builds, before + 1, "once for the edit");
}

/// The pitch lane under an audio track (§3c): shown from the track (one undo step), its
/// notes laid out where they play; a drag moves a note's pitch (one step, the editor's
/// command), a double-click pulls its centre onto its note.
#[test]
fn the_pitch_lane_edits_notes_in_the_arrangement() {
    let mut r = Rig::new();
    let (t, c) = r.voice();
    let _ = r.app.pitch_take(t, c);
    let t0 = Instant::now();
    while r.app.engine.samples.pitch().busy() && t0.elapsed() < Duration::from_secs(30) {
        std::thread::sleep(Duration::from_millis(5));
    }
    r.app.toggle_pitch_lane(t);
    assert_eq!(r.app.bus.undo_label_key(), Some("cmd.pitch.lane"));
    r.idle(4);
    let geo = r.app.arrangement.geometry().expect("drawn");
    let row = geo.rows[1];
    assert!(row.pitch > 0.0, "the lane takes its height");
    let lanes = Rect::from_min_max(
        pos2(geo.left, geo.body_top),
        pos2(geo.left + geo.width, 2000.0),
    );
    let laid = cantus_ui::views::pitch_lane::layout(&r.app, &geo, lanes);
    let lane = laid
        .iter()
        .find(|l| l.track == Some(t))
        .expect("the voice's lane");
    assert_eq!(lane.notes.len(), 4, "{:?}", lane.notes);
    r.app.pitch.view.snap = cantus_ui::views::pitch::Snap::Semitone;
    let n = lane.notes[1].clone();
    let steps = r.app.bus.history().len();
    r.drag(
        n.rect.center(),
        n.rect.center() - vec2(0.0, 2.0 * n.px_per_st),
    );
    let e = r.stored().expect("edited from the lane");
    let moved = e.note(n.id).unwrap();
    assert_eq!(moved.pitch - moved.home(), 2.0, "two semitones up");
    assert_eq!(r.app.bus.history().len(), steps + 1, "one step");
    assert_eq!(r.app.bus.undo_label_key(), Some(PitchOp::Pitch.label_key()));
    // A double-click on it: its centre onto its note.
    let geo = r.app.arrangement.geometry().unwrap();
    let laid = cantus_ui::views::pitch_lane::layout(&r.app, &geo, lanes);
    let n = laid.iter().find(|l| l.track == Some(t)).unwrap().notes[1].clone();
    r.idle(30);
    r.pointer = Some(n.rect.center());
    r.idle(2);
    for _ in 0..2 {
        r.frame(vec![Rig::button(n.rect.center(), true, Modifiers::NONE)]);
        r.frame(vec![Rig::button(n.rect.center(), false, Modifiers::NONE)]);
    }
    r.idle(2);
    assert_eq!(r.stored().unwrap().note(n.id).unwrap().centre, 1.0);
    r.app.undo();
    r.app.undo();
    r.app.undo();
    assert!(!r.app.project().track(t).unwrap().pitch_open, "undone");
}
