//! Fides's input as a sidechain from a strip, the way a user reaches it (round 41, ADR
//! 0154): a song with a source track, Fides added through `CantusApp::assemble` and the
//! picker, its exciter set to *Input* — silent, and its window says so — then the source's
//! strip chosen in its Input card with the pointer (the kit's field and its list, as an
//! insert's key is chosen), and the song played through the real engine wiring: the
//! source's audio excites Fides, heard. The choice is one step of undo, offers no strip
//! that would close a loop, and survives a save and a load.

use cantus_core::{InstrumentKind, Project, Ticks};
use cantus_engine::EngineHandle;
use cantus_ui::app::Files;
use cantus_ui::i18n::{set_thread_locale, Locale};
use cantus_ui::picker::{PickChoice, PickPurpose};
use cantus_ui::views::inst::View;
use cantus_ui::CantusApp;
use egui::{vec2, Event, Pos2, Rect};

struct Rig {
    app: CantusApp,
    proc_: cantus_engine::Processor,
    ctx: egui::Context,
    t: f64,
}

impl Rig {
    fn new(data: &std::path::Path, song: Project) -> Rig {
        set_thread_locale(Locale::En);
        std::env::set_var("CANTUS_DATA_DIR", data);
        std::fs::write(
            data.join("settings.json"),
            r#"{"audio_setup":{"setup_done":true}}"#,
        )
        .unwrap();
        let files = Files::read(data, Some(Locale::En));
        let ctx = egui::Context::default();
        cantus_ui::design::install_fonts(&ctx);
        cantus_ui::design::install_style(&ctx);
        let (engine, proc_) = EngineHandle::manual(48_000.0);
        let app = CantusApp::assemble(&ctx, files, Some(song), engine, None, None, None, false);
        let mut r = Rig {
            app,
            proc_,
            ctx,
            t: 0.0,
        };
        r.frames(3);
        r
    }

    /// One frame with `events`: the texts painted, with where they lie.
    fn frame(&mut self, events: Vec<Event>) -> Vec<(String, Rect)> {
        self.t += 1.0 / 60.0;
        let input = egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(1920.0, 1080.0))),
            time: Some(self.t),
            predicted_dt: 1.0 / 60.0,
            events,
            ..Default::default()
        };
        let app = &mut self.app;
        let mut out = self.ctx.run_ui(input, |ui| app.frame(ui));
        out.textures_delta.clear();
        fn walk(shape: &egui::Shape, out: &mut Vec<(String, Rect)>) {
            match shape {
                egui::Shape::Text(t) => out.push((
                    t.galley.text().to_string(),
                    t.galley.rect.translate(t.pos.to_vec2()),
                )),
                egui::Shape::Vec(v) => v.iter().for_each(|s| walk(s, out)),
                _ => {}
            }
        }
        let mut texts = Vec::new();
        for c in &out.shapes {
            walk(&c.shape, &mut texts);
        }
        texts
    }

    fn frames(&mut self, n: usize) -> Vec<(String, Rect)> {
        let mut t = Vec::new();
        for _ in 0..n {
            t = self.frame(Vec::new());
        }
        t
    }

    fn click(&mut self, p: Pos2) {
        self.frame(vec![Event::PointerMoved(p)]);
        for pressed in [true, false] {
            self.frame(vec![Event::PointerButton {
                pos: p,
                button: egui::PointerButton::Primary,
                pressed,
                modifiers: egui::Modifiers::NONE,
            }]);
        }
        self.frames(2);
    }

    /// Where `text` is painted now (the last of them: a list over the window).
    fn find(&mut self, text: &str) -> Option<Pos2> {
        self.frames(2)
            .iter()
            .rev()
            .find(|(t, _)| t == text)
            .map(|(_, r)| r.center())
    }

    /// Play the song from the start for `seconds` through the processor: the output.
    fn play(&mut self, seconds: usize) -> Vec<f32> {
        let app = &mut self.app;
        app.sync_engine();
        app.seek(Ticks::ZERO);
        app.toggle_play();
        app.sync_engine();
        let frames = 48_000 * seconds;
        let mut out = Vec::with_capacity(frames * 2);
        while out.len() < frames * 2 {
            let mut buf = vec![0f32; 512 * 2];
            self.proc_.process(&mut buf, 2);
            out.extend_from_slice(&buf);
        }
        app.toggle_play();
        app.sync_engine();
        // What rings after the stop dies before the next play.
        for _ in 0..200 {
            let mut buf = vec![0f32; 512 * 2];
            self.proc_.process(&mut buf, 2);
        }
        out
    }
}

/// A song whose one track plays short notes from beat 1 on its own strip.
fn song() -> Project {
    let mut p = Project::new("fides input");
    p.tempo_bpm = 120.0;
    p.loop_enabled = false;
    let mut src = p.make_track("Source");
    let mut clip = p.make_clip("Notes", Ticks::ZERO, Ticks::beats(4));
    let q = Ticks::QUARTER;
    for (at, pitch) in [(2i64, 60u8), (3, 64), (4, 67), (6, 72)] {
        clip.notes.push(p.make_note(q * at / 2, q / 2, pitch, 110));
    }
    src.clips.push(clip);
    p.tracks.push(src);
    p
}

/// The tests point `CANTUS_DATA_DIR` at a folder of their own: they take turns.
fn turn() -> std::sync::MutexGuard<'static, ()> {
    static TURN: std::sync::Mutex<()> = std::sync::Mutex::new(());
    TURN.lock().unwrap_or_else(|e| e.into_inner())
}

fn rms_db(x: &[f32]) -> f64 {
    let e = x.iter().map(|v| (*v as f64) * (*v as f64)).sum::<f64>() / x.len().max(1) as f64;
    10.0 * cantus_dsp::math::log10(e.max(1e-30))
}

#[test]
fn a_track_s_audio_chosen_in_fides_s_window_excites_it_audibly() {
    let _turn = turn();
    let data = tempfile::tempdir().unwrap();
    let mut r = Rig::new(data.path(), song());
    let source_strip = r.app.project().tracks[0].strip.expect("the source's strip");
    // Fides through the picker, its exciter set to Input, a fifth held for four beats.
    let purpose = PickPurpose::AddInstrument { index: None };
    r.app.open_picker(purpose.clone());
    r.app
        .apply_pick(purpose, PickChoice::Instrument(InstrumentKind::Resonator));
    r.app.picker = None;
    let track = r.app.project().tracks.last().unwrap().id;
    r.app.set_inst_value(track, "exc.type", 5.0, false);
    let mut p = r.app.project().clone();
    let mut held = p.make_clip("Held", Ticks::ZERO, Ticks::beats(4));
    for pitch in [48u8, 55] {
        held.notes
            .push(p.make_note(Ticks::ZERO, Ticks::beats(4), pitch, 100));
    }
    assert!(r.app.exec(cantus_core::Command::InsertClip {
        track,
        clip: Box::new(held),
    }));
    let unkeyed = r.play(3);

    // Its window, Play (the start page put away): the Input card says nothing is chosen,
    // and that it is silent.
    r.app.workflow.start.close();
    r.app.audio_setup.first_run = None;
    r.app.open_inst_editor(track);
    r.app.inst.view = View::Play;
    let texts = r.frames(4);
    assert!(
        texts
            .iter()
            .any(|(t, _)| t == "Choose a strip: silent until then"),
        "the Input card does not say it is silent"
    );
    let field = r.find("No strip").expect("the Input card's source field");
    r.click(field);
    // The list offers the source's strip and not Fides's own (a loop).
    let want = r
        .app
        .capture_label(cantus_engine::CaptureTarget::Strip(source_strip));
    let own = r.app.project().track(track).and_then(|t| t.strip).unwrap();
    let own_label = r
        .app
        .capture_label(cantus_engine::CaptureTarget::Strip(own));
    let listed = r.frames(2);
    assert!(
        !listed.iter().any(|(t, _)| *t == own_label),
        "its own strip is offered: {own_label}"
    );
    let item = listed
        .iter()
        .rev()
        .find(|(t, _)| *t == want)
        .map(|(_, rect)| rect.center())
        .unwrap_or_else(|| {
            let seen: Vec<&String> = listed
                .iter()
                .map(|(t, _)| t)
                .filter(|t| t.contains("Strip") || t.contains("strip") || t.contains('·'))
                .collect();
            panic!("the list does not offer {want}: {seen:?}")
        });
    r.click(item);
    assert_eq!(
        r.app.project().track(track).and_then(|t| t.sidechain),
        Some(source_strip),
        "the source's strip is not chosen"
    );
    // The card now names it.
    assert!(r.find(&want).is_some(), "the field does not name {want}");

    // Played through the engine: the source excites Fides.
    let keyed = r.play(3);
    assert!(keyed.iter().all(|v| v.is_finite()));
    let part: Vec<f32> = keyed
        .chunks_exact(2)
        .zip(unkeyed.chunks_exact(2))
        .map(|(a, b)| a[0] - b[0])
        .collect();
    let heard = rms_db(&part[24_000..72_000]);
    println!("Fides excited by the source: {heard:.1} dBFS RMS over beats 1–3");
    assert!(heard > -45.0, "Fides is not heard: {heard:.1} dBFS");
    let peak = keyed.iter().fold(0f32, |m, v| m.max(v.abs()));
    assert!(peak < 1.0, "it clips: {peak}");

    // One step of undo, named so.
    r.app.undo();
    assert_eq!(r.app.project().track(track).unwrap().sidechain, None);
    r.app.redo();
    assert_eq!(
        r.app.project().track(track).unwrap().sidechain,
        Some(source_strip)
    );

    // Saved and loaded, the key is kept.
    let path = data.path().join("fides-input.cantus");
    cantus_project::save(r.app.project(), &path, "test").expect("save");
    let back = cantus_project::load(&path, true).expect("load").project;
    assert_eq!(
        back.tracks
            .iter()
            .find(|t| t.id == track)
            .and_then(|t| t.sidechain),
        Some(source_strip)
    );
}

/// r41:key-wiring — an agent keys Fides's input through the MCP server the assembled app
/// serves (`set_track`'s `sidechain_strip_id`), the way `set_effect` keys an insert: a
/// source that would close a loop is refused with the reason, an instrument that takes no
/// input too; the source's strip is set, heard through the real engine wiring, and taken
/// back by one step of the user's undo.
#[test]
fn an_agent_keys_fides_s_input_through_the_server() {
    use cantus_mcp::Answer;
    use serde_json::json;
    let _turn = turn();
    let data = tempfile::tempdir().unwrap();
    let mut r = Rig::new(data.path(), song());
    assert!(r.app.ai.is_wired(), "assemble makes the MCP server");
    let source_track = r.app.project().tracks[0].id;
    let source_strip = r.app.project().tracks[0].strip.expect("the source's strip");
    let purpose = PickPurpose::AddInstrument { index: None };
    r.app.open_picker(purpose.clone());
    r.app
        .apply_pick(purpose, PickChoice::Instrument(InstrumentKind::Resonator));
    r.app.picker = None;
    let track = r.app.project().tracks.last().unwrap().id;
    let own = r.app.project().track(track).and_then(|t| t.strip).unwrap();
    r.app.set_inst_value(track, "exc.type", 5.0, false);
    let mut p = r.app.project().clone();
    let mut held = p.make_clip("Held", Ticks::ZERO, Ticks::beats(4));
    for pitch in [48u8, 55] {
        held.notes
            .push(p.make_note(Ticks::ZERO, Ticks::beats(4), pitch, 100));
    }
    assert!(r.app.exec(cantus_core::Command::InsertClip {
        track,
        clip: Box::new(held),
    }));
    let unkeyed = r.play(3);

    let server = r.app.ai.server().unwrap().clone();
    let mut client = cantus_mcp::client::TestClient::legacy(server, "2025-11-25");
    let mut call = |app: &mut CantusApp, args: serde_json::Value| {
        client.call_tool("set_track", args, &mut || {
            cantus_ui::ai_mcp::pump(app);
            if let Some(agents) = app.ai.agents_mut() {
                let ids: Vec<u64> = agents.asks().map(|x| x.id).collect();
                for id in ids {
                    agents.answer(id, Answer::Allow);
                }
            }
        })
    };
    // Refused: its own strip (a loop), and the source track's instrument (no input).
    let depth = r.app.bus.history().len();
    let e = call(
        &mut r.app,
        json!({"track_id": track.0, "sidechain_strip_id": own.0}),
    );
    assert!(e.is_error && e.text.contains("a loop"), "{}", e.text);
    let e = call(
        &mut r.app,
        json!({"track_id": source_track.0, "sidechain_strip_id": own.0}),
    );
    assert!(
        e.is_error && e.text.contains("takes no input"),
        "{}",
        e.text
    );
    assert_eq!(r.app.bus.history().len(), depth, "a refusal is no step");
    // Set: one step, answered with the source.
    let ok = call(
        &mut r.app,
        json!({"track_id": track.0, "sidechain_strip_id": source_strip.0}),
    );
    assert!(!ok.is_error, "{}", ok.text);
    assert_eq!(
        ok.structured.unwrap()["sidechain_strip_id"],
        json!(source_strip.0)
    );
    assert_eq!(r.app.bus.history().len(), depth + 1, "one undo step");
    assert_eq!(
        r.app.project().track(track).unwrap().sidechain,
        Some(source_strip)
    );
    // Heard through the engine: the source excites Fides.
    let keyed = r.play(3);
    let part: Vec<f32> = keyed
        .chunks_exact(2)
        .zip(unkeyed.chunks_exact(2))
        .map(|(a, b)| a[0] - b[0])
        .collect();
    let heard = rms_db(&part[24_000..72_000]);
    println!("Fides keyed by an agent: {heard:.1} dBFS RMS over beats 1–3");
    assert!(heard > -45.0, "Fides is not heard: {heard:.1} dBFS");
    // The user's undo takes the agent's step back.
    r.app.undo();
    assert_eq!(r.app.project().track(track).unwrap().sidechain, None);
}

/// r41:key-wiring — the key is a route like an insert's (ADR 0154), seen where a user looks
/// for routes: on the desk (the mixer, Fides's strip selected, the Cables key on) a key
/// cable from the source's strip into Fides's; a click selects it and Delete takes the key
/// away, one step of undo; Fides's strip's inspector lists it under Receives.
#[test]
fn fides_s_key_is_a_cable_on_the_desk_and_a_row_of_the_inspector() {
    use cantus_core::routing::{Route, RouteKind};
    use cantus_ui::views::mixer::{self, layout::CableView, layout::WidthMode};
    let _turn = turn();
    let data = tempfile::tempdir().unwrap();
    let mut r = Rig::new(data.path(), song());
    let source = r.app.project().tracks[0].strip.expect("the source's strip");
    let purpose = PickPurpose::AddInstrument { index: None };
    r.app.open_picker(purpose.clone());
    r.app
        .apply_pick(purpose, PickChoice::Instrument(InstrumentKind::Resonator));
    r.app.picker = None;
    r.app.workflow.start.close();
    r.app.audio_setup.first_run = None;
    let track = r.app.project().tracks.last().unwrap().id;
    let own = r.app.project().track(track).and_then(|t| t.strip).unwrap();
    assert!(r.app.exec(cantus_core::Command::SetTrackSidechain {
        track,
        source: Some(source),
    }));
    let name = r.app.project().track(track).unwrap().name.clone();
    r.app.show_central(cantus_ui::app::CentralView::Mixer);
    r.app
        .workspace
        .toggle_fold(cantus_ui::views::workspace::Pane::Arrangement);
    mixer::set_width_mode(&mut r.app, WidthMode::Wide);
    r.app.select_strip(own);
    let mut prefs = r.app.mixer.prefs();
    prefs.cables = CableView::Selected;
    r.app.mixer.set_prefs(prefs);
    let texts = r.frames(4);
    // The desk: one key cable, from the source into Fides's strip.
    let route = Route {
        from: source,
        to: own,
        kind: RouteKind::TrackKey(track),
    };
    let cable = r
        .app
        .mixer
        .cables()
        .iter()
        .find(|d| d.route == route)
        .copied()
        .unwrap_or_else(|| panic!("no key cable: {:?}", r.app.mixer.cables()));
    assert_eq!(format!("{:?}", cable.kind), "Key", "drawn as a key");
    // The inspector: Receives names the key.
    let want = format!("{name}, input ← key");
    assert!(
        texts.iter().any(|(t, _)| *t == want),
        "the inspector's Receives does not say {want:?}"
    );
    // A click on the cable selects it; Delete takes the key away, one step of undo.
    let at = r.app.mixer.cable_point(&cable);
    r.click(at);
    assert_eq!(r.app.mixer.selected_cable(), Some(route));
    let depth = r.app.bus.history().len();
    r.frame(vec![Event::Key {
        key: egui::Key::Delete,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: egui::Modifiers::NONE,
    }]);
    r.frames(2);
    assert_eq!(r.app.project().track(track).unwrap().sidechain, None);
    assert_eq!(r.app.bus.history().len(), depth + 1, "one undo step");
    r.app.undo();
    assert_eq!(
        r.app.project().track(track).unwrap().sidechain,
        Some(source)
    );
    r.frames(2);
    assert!(
        r.app.mixer.cables().iter().any(|d| d.route == route),
        "the undone key is drawn again"
    );
}
