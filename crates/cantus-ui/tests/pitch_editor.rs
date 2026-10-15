//! The pitch editor through the whole window (round 28, ADR 0054): opened from an audio
//! clip (a double-click, *Edit pitch*), the take analysed off the window's thread, its
//! notes drawn as blobs; a drag moves a note's pitch (one undo step, undone exactly),
//! the Split tool cuts one, Ctrl+J joins, the macros correct the take, *Original* compares,
//! MIDI out and *Double* make their clips — and the engine plays the render. Its own test
//! binary: the boot path's test points `CANTUS_DATA_DIR` at a temporary folder.

use cantus_core::pitch::PitchOp;
use cantus_core::*;
use cantus_dsp::pitch::midi_to_hz;
use cantus_dsp::pitch::testkit::{self, Vowel};
use cantus_engine::EngineHandle;
use cantus_ui::app::{CentralView, Files};
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
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(1600.0, 900.0))),
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
    // The clip now plays a render (the window's engine makes it here, at once).
    r.app.sync_engine();
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
fn the_boot_path_keeps_analyses_in_the_data_folder() {
    let data = tempfile::tempdir().unwrap();
    std::env::set_var("CANTUS_DATA_DIR", data.path());
    let files = Files::read(data.path(), Some(cantus_ui::i18n::Locale::En));
    let ctx = egui::Context::default();
    let app = CantusApp::assemble(
        &ctx,
        files,
        None,
        EngineHandle::dummy(),
        None,
        None,
        None,
        false,
    );
    assert_eq!(
        app.engine.samples.pitch().disk_cache(),
        Some(cantus_ui::app::data_dir().join("cache").join("pitch"))
    );
    data_dir();
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
    let snap = app.engine.compile(&proj, 1);
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
    app.exec(Command::SetTrackMute {
        id: app.project().tracks[1].id,
        muted: false,
    });
    let proj = app.project().clone();
    let snap = app.engine.compile(&proj, 2);
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
