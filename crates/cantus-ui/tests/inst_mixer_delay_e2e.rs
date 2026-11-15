//! An output's delay on the instrument mixer's strip, end to end (round 40 phase 3, ADR
//! 0123): an app made by `CantusApp::assemble` with the real engine handle, Tympanum's Mixer
//! view drawn, the snare mic's *Delay* cell typed and dragged as a user does.
//!
//! - Typed "1" (ms): `o{n}.delay` is 1, and the master — that mic soloed — is the same
//!   sound 48 samples later at 48 kHz, to the bit; typed back to 0 it is the master before,
//!   to the bit (no line at 0).
//! - Dragged sideways: the delay grows, one undo step takes it back.
//!
//! Its own test binary: it points `CANTUS_DATA_DIR` at a temporary folder.

use cantus_core::inst_out::{self, OutputField};
use cantus_core::*;
use cantus_engine::{render, EngineHandle, Processor, RenderOptions, SampleCache};
use cantus_ui::app::Files;
use cantus_ui::views::inst::inst_mixer::strip::delay_rect;
use cantus_ui::views::inst::tympanum;
use cantus_ui::CantusApp;
use std::path::Path;
use std::sync::Arc;

fn data() -> &'static Path {
    static DIR: std::sync::OnceLock<tempfile::TempDir> = std::sync::OnceLock::new();
    DIR.get_or_init(|| {
        let d = tempfile::tempdir().unwrap();
        std::env::set_var("CANTUS_DATA_DIR", d.path());
        std::fs::write(
            d.path().join("settings.json"),
            r#"{"audio_setup":{"setup_done":true},"start_page_at_start":false}"#,
        )
        .unwrap();
        d
    })
    .path()
}

struct Rig {
    app: CantusApp,
    proc_: Processor,
    ctx: egui::Context,
    t: f64,
}

impl Rig {
    fn new(project: Project) -> Rig {
        cantus_ui::i18n::set_thread_locale(cantus_ui::i18n::Locale::En);
        let files = Files::read(data(), Some(cantus_ui::i18n::Locale::En));
        let ctx = egui::Context::default();
        cantus_ui::design::install_fonts(&ctx);
        cantus_ui::design::install_style(&ctx);
        let (engine, proc_) = EngineHandle::manual(48_000.0);
        let app = CantusApp::assemble(&ctx, files, Some(project), engine, None, None, None, false);
        Rig {
            app,
            proc_,
            ctx,
            t: 0.0,
        }
    }

    fn frame(&mut self, events: Vec<egui::Event>) {
        let mut buf = vec![0f32; 800 * 2];
        self.proc_.process(&mut buf, 2);
        self.t += 1.0 / 60.0;
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(1600.0, 900.0),
            )),
            time: Some(self.t),
            predicted_dt: 1.0 / 60.0,
            events,
            ..Default::default()
        };
        let app = &mut self.app;
        self.ctx
            .run_ui(input, |ui| app.frame(ui))
            .textures_delta
            .clear();
    }

    fn idle(&mut self, n: usize) {
        for _ in 0..n {
            self.frame(Vec::new());
        }
    }

    fn button(&mut self, at: egui::Pos2, pressed: bool) {
        self.frame(vec![egui::Event::PointerButton {
            pos: at,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: Default::default(),
        }]);
    }

    fn key(&mut self, key: egui::Key) {
        for pressed in [true, false] {
            self.frame(vec![egui::Event::Key {
                key,
                physical_key: None,
                pressed,
                repeat: false,
                modifiers: Default::default(),
            }]);
        }
    }

    /// Double-click the cell, type `words`, Enter.
    fn type_into(&mut self, cell: egui::Rect, words: &str) {
        // Half a second on: an earlier click is no longer half of this double-click.
        self.idle(30);
        let at = cell.center();
        self.frame(vec![egui::Event::PointerMoved(at)]);
        for _ in 0..2 {
            self.button(at, true);
            self.button(at, false);
        }
        self.idle(2);
        self.frame(vec![egui::Event::Text(words.into())]);
        self.key(egui::Key::Enter);
        self.idle(1);
    }
}

/// Drums: a Tympanum playing four snare hits.
fn kit_song() -> (Project, TrackId, u8) {
    let mut p = Project::new("Delay");
    let mut t = p.make_track("Drums");
    t.strip = Some(p.strips[0].id);
    let spec = NativeSpec::new(InstrumentKind::AcousticDrums);
    let def = tympanum::def::def_of(&spec);
    let snare = def.pieces[1].articulations[0].key;
    t.instrument = InstrumentSpec::Native(spec);
    let mut clip = p.make_clip("Groove", Ticks::ZERO, Ticks(1920));
    for i in 0..4 {
        let n = p.make_note(Ticks(i * 480), Ticks(240), snare, 100);
        clip.notes.push(n);
    }
    t.clips.push(clip);
    let id = t.id;
    p.tracks.push(t);
    (p, id, snare)
}

/// The project's master, rendered offline as the export renders it.
fn master(project: &Project) -> Vec<f32> {
    let mut snap = Snapshot::compile(project, 1);
    let errors = SampleCache::new().attach(&mut snap);
    assert!(errors.is_empty(), "{errors:?}");
    render(
        Arc::new(snap),
        &RenderOptions {
            end: Some(Ticks(1920)),
            tail_seconds: 0.5,
            block_frames: 333,
            ..Default::default()
        },
    )
    .unwrap()
    .samples
}

fn bits(x: &[f32]) -> Vec<u32> {
    x.iter().map(|v| (v + 0.0).to_bits()).collect()
}

/// The first mic of the snare's family: the one the test listens to, soloed.
fn snare_mic(app: &CantusApp, drums: TrackId) -> OutputId {
    let spec = app.native_of(drums).unwrap();
    let def = tympanum::def::def_of(&spec);
    let m = def
        .mics
        .iter()
        .position(|m| m.family == OutputFamily::Snare)
        .expect("a snare mic");
    OutputId(m as u16)
}

#[test]
fn a_typed_delay_moves_that_mic_by_whole_samples_and_zero_is_the_master_again() {
    let (p, drums, _) = kit_song();
    let mut r = Rig::new(p);
    let mic = snare_mic(&r.app, drums);
    let delay = inst_out::key(mic, OutputField::Delay);
    // That mic alone on the master.
    r.app
        .set_inst_value(drums, &inst_out::key(mic, OutputField::Solo), 1.0, false);
    let dry = master(r.app.project());
    assert!(dry.iter().any(|v| v.abs() > 1e-4), "the snare mic sounds");
    r.app.open_inst_editor(drums);
    r.app.inst.view = cantus_ui::views::inst::View::Mixer;
    r.idle(4);
    let cell = delay_rect(&r.ctx, mic).expect("the snare mic's Delay cell is drawn");
    r.type_into(cell, "1");
    let spec = r.app.native_of(drums).unwrap();
    assert_eq!(spec.get(&delay), 1.0, "typed: 1 ms");
    let late = master(r.app.project());
    // 1 ms at 48 kHz: 48 frames, stereo interleaved.
    let n = 48 * 2;
    assert_eq!(late.len(), dry.len());
    assert!(
        late[..n].iter().all(|v| *v == 0.0),
        "nothing before the delay"
    );
    assert_eq!(
        bits(&late[n..]),
        bits(&dry[..dry.len() - n]),
        "the same sound, 48 samples later"
    );
    // Typed back to 0: the master before, to the bit.
    let cell = delay_rect(&r.ctx, mic).unwrap();
    r.type_into(cell, "0");
    assert_eq!(r.app.native_of(drums).unwrap().get(&delay), 0.0);
    assert_eq!(bits(&master(r.app.project())), bits(&dry), "0: as recorded");
    // Each typed value was one undo step.
    let steps: Vec<String> = r
        .app
        .bus
        .history()
        .iter()
        .map(|h| format!("{} {:?}", h.label_key, h.command))
        .collect();
    r.app.undo();
    assert_eq!(r.app.native_of(drums).unwrap().get(&delay), 1.0);
    r.app.undo();
    assert_eq!(
        r.app.native_of(drums).unwrap().get(&delay),
        0.0,
        "{steps:#?}"
    );
}

#[test]
fn a_dragged_delay_grows_and_one_undo_takes_it_back() {
    let (p, drums, _) = kit_song();
    let mut r = Rig::new(p);
    let mic = snare_mic(&r.app, drums);
    let delay = inst_out::key(mic, OutputField::Delay);
    r.app.open_inst_editor(drums);
    r.app.inst.view = cantus_ui::views::inst::View::Mixer;
    r.idle(4);
    let cell = delay_rect(&r.ctx, mic).expect("drawn");
    let a = cell.center();
    r.frame(vec![egui::Event::PointerMoved(a)]);
    r.button(a, true);
    for k in 1..=8 {
        r.frame(vec![egui::Event::PointerMoved(
            a + egui::vec2(4.0 * k as f32, 0.0),
        )]);
    }
    let b = a + egui::vec2(32.0, 0.0);
    r.button(b, false);
    r.idle(1);
    let v = r.app.native_of(drums).unwrap().get(&delay);
    assert!(
        v > 0.5 && v < 2.5,
        "dragged 32 pt right: about 1.6 ms, got {v}"
    );
    r.app.undo();
    assert_eq!(
        r.app.native_of(drums).unwrap().get(&delay),
        0.0,
        "the drag was one undo step"
    );
}
