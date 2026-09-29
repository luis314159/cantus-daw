//! The plug-in's own editor: Porta's real editor (`cantus_ui::views::fx::gate::editor`)
//! running against the plug-in as its host (`cantus_plugins::editor::PluginHost`).

#![cfg(feature = "editor")]

use cantus_core::EffectKind;
use cantus_plugins::desc;
use cantus_plugins::editor::{Edit, EditSink, PluginHost};
use cantus_plugins::instance::{Processor, Shared};
use cantus_ui::views::fx::gate;
use cantus_ui::views::fx::host::{FxHost, KeySource, KeyTarget};
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};

struct Collect(Arc<Mutex<Vec<Edit>>>);

impl EditSink for Collect {
    fn edit(&mut self, edit: Edit) {
        self.0.lock().unwrap().push(edit);
    }
}

fn host() -> (Arc<Shared>, PluginHost, Arc<Mutex<Vec<Edit>>>) {
    let shared = Shared::new(desc::of(EffectKind::Gate).unwrap());
    let edits = Arc::new(Mutex::new(Vec::new()));
    let host = PluginHost::new(shared.clone(), Box::new(Collect(edits.clone())));
    (shared, host, edits)
}

#[test]
fn the_editors_gestures_reach_the_host_as_begin_values_end() {
    let (shared, mut host, edits) = host();
    let t = shared.table;
    let thr = t.by_key("thr").unwrap().index as u32;
    let style = t.by_key("style").unwrap().index as u32;
    let hyst = t.by_key("hyst").unwrap().index as u32;
    // A drag: one gesture, however many values.
    host.set("thr", -20.0, Some("thr"));
    host.set("thr", -22.0, Some("thr"));
    host.end_gesture();
    // A click: a gesture of its own.
    host.set("style", 2.0, None);
    // A learned pair.
    host.set_many(&[("thr", -30.0), ("hyst", 6.0)], "cmd.gate_learn");
    assert_eq!(
        *edits.lock().unwrap(),
        vec![
            Edit::Begin(thr),
            Edit::Value(thr, -20.0),
            Edit::Value(thr, -22.0),
            Edit::End(thr),
            Edit::Begin(style),
            Edit::Value(style, 2.0),
            Edit::End(style),
            Edit::Begin(thr),
            Edit::Value(thr, -30.0),
            Edit::End(thr),
            Edit::Begin(hyst),
            Edit::Value(hyst, 6.0),
            Edit::End(hyst),
        ]
    );
    // What the editor set reads back at once.
    host.refresh();
    assert_eq!(host.spec().get("thr"), -30.0);
    assert_eq!(host.spec().get("style"), 2.0);
    // The key: the host routes it; the editor switches the detector to it.
    assert_eq!(host.key_source(), Some(KeySource::Switch(false)));
    host.set_key_source(KeyTarget::Switch(true));
    assert_eq!(host.key_source(), Some(KeySource::Switch(true)));
    // A DAW-only action is not offered.
    assert!(!host.makes_clips());
    assert_eq!(host.notes_to_clip(&[(0, 100)], 36), None);
}

#[test]
fn porta_draws_its_real_editor_from_the_plugins_own_audio_thread() {
    let (shared, mut host, _) = host();
    // The plug-in running, with its editor open: telemetry and the tap flow.
    shared.editor_open.store(true, Ordering::Release);
    let mut proc_ = Processor::new(shared.clone(), 48_000.0);
    let x = gate::demo_signal(1.0, 48_000.0);
    for chunk in x.chunks(480) {
        let (mut l, mut r) = (chunk.to_vec(), chunk.to_vec());
        proc_.process(&mut l, &mut r, None, &[]);
    }
    assert!(shared.telemetry_serial.load(Ordering::Acquire) > 0);

    let ctx = egui::Context::default();
    cantus_ui::design::install_fonts(&ctx);
    cantus_ui::design::install_style(&ctx);
    cantus_ui::theme::apply(&cantus_ui::theme::default_theme(), &ctx);
    let mut state = gate::State::default();
    let debug = std::env::var("CANTUS_DEBUG_SHOT")
        .ok()
        .map(std::path::PathBuf::from);
    let canvas = cantus_ui::raster::shoot(
        &ctx,
        egui::vec2(980.0, 660.0),
        1.0,
        3,
        debug.as_deref(),
        |ui| {
            host.refresh();
            gate::editor(&mut host, &mut state, ui);
        },
    );
    assert!(!host.pairs().is_empty(), "the tap reached the editor");
    assert!(
        !state.history.is_empty(),
        "the telemetry reached the history"
    );
    let geo = state.geometry.expect("the editor laid out its displays");
    assert!(geo.history.width() > 100.0 && geo.transfer.width() > 100.0);
    // Something was drawn where the history is: not one flat colour. (Its middle is the
    // default threshold, a crisp line across the whole width: look lower.)
    let y = geo.history_y(-66.0) as usize;
    let row: Vec<[u8; 4]> = (geo.history.left() as usize..geo.history.right() as usize)
        .map(|x| canvas.pixel(x, y))
        .collect();
    assert!(row.iter().any(|p| *p != row[0]));
}

/// The text a frame painted, joined.
fn painted(shapes: &[egui::epaint::ClippedShape]) -> String {
    fn walk(shape: &egui::Shape, out: &mut String) {
        match shape {
            egui::Shape::Text(t) => {
                out.push_str(t.galley.text());
                out.push_str(" | ");
            }
            egui::Shape::Vec(v) => v.iter().for_each(|s| walk(s, out)),
            _ => {}
        }
    }
    let mut out = String::new();
    for s in shapes {
        walk(&s.shape, &mut out);
    }
    out
}

/// Every kind whose editor runs on `FxHost` opens it in the plug-in's window, in the
/// device frame: its name in the header, fed by the plug-in's own audio thread.
#[test]
fn every_editor_on_fx_host_draws_in_the_plugin_window() {
    cantus_ui::i18n::default_thread_locale(cantus_ui::i18n::Locale::En);
    assert!(cantus_plugins::editor::has_editor(EffectKind::Gate));
    for kind in EffectKind::ALL
        .into_iter()
        .filter(|k| cantus_plugins::editor::has_editor(*k))
    {
        let shared = Shared::new(desc::of(kind).unwrap());
        shared.editor_open.store(true, Ordering::Release);
        let mut proc_ = Processor::new(shared.clone(), 48_000.0);
        let x = gate::demo_signal(0.5, 48_000.0);
        for chunk in x.chunks(480) {
            let (mut l, mut r) = (chunk.to_vec(), chunk.to_vec());
            proc_.process(&mut l, &mut r, None, &[]);
        }
        let edits = Arc::new(Mutex::new(Vec::new()));
        let mut host = PluginHost::new(shared.clone(), Box::new(Collect(edits)));
        let mut editor = cantus_ui::views::fx::Editor::of(kind).expect("an editor");
        let ctx = egui::Context::default();
        cantus_ui::design::install_fonts(&ctx);
        cantus_ui::design::install_style(&ctx);
        cantus_ui::theme::apply(&cantus_ui::theme::default_theme(), &ctx);
        let (w, h) = cantus_plugins::editor::size(kind);
        let mut text = String::new();
        for i in 0..3 {
            let input = egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(w as f32, h as f32),
                )),
                time: Some(i as f64 * 0.25),
                ..Default::default()
            };
            let mut out = ctx.run_ui(input, |ui| {
                host.refresh();
                let _ = editor.show(&mut host, ui);
            });
            out.textures_delta.clear();
            text = painted(&out.shapes);
        }
        let name = cantus_ui::i18n::t(kind.name_key()).to_lowercase();
        assert!(
            text.to_lowercase().contains(&name),
            "{kind:?}: the frame's header names the effect: {text}"
        );
    }
}

/// The frame's bypass lamp and A/B reach the host as the standards' gestures.
#[test]
fn the_frames_bypass_and_a_b_are_the_hosts_gestures() {
    let (shared, mut host, edits) = host();
    let t = shared.table;
    let thr = t.by_key("thr").unwrap().index as u32;
    let bypass = t.bypass as u32;
    assert_eq!(host.bypassed(), Some(false));
    host.set_bypassed(true);
    assert_eq!(host.bypassed(), Some(true));
    // A/B: B starts as a copy of A; change it; back to A restores A's threshold.
    assert_eq!(host.ab(), Some(false));
    host.toggle_ab();
    assert_eq!(host.ab(), Some(true));
    host.set("thr", -12.0, None);
    host.toggle_ab();
    assert_eq!(host.ab(), Some(false));
    host.refresh();
    assert_eq!(host.spec().get("thr"), -40.0, "A came back");
    let e = edits.lock().unwrap();
    assert_eq!(
        &e[..3],
        &[
            Edit::Begin(bypass),
            Edit::Value(bypass, 1.0),
            Edit::End(bypass)
        ]
    );
    assert!(
        e.contains(&Edit::Value(thr, -40.0)),
        "switching sides sends the other side's values: {e:?}"
    );
}
