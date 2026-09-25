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
    let canvas = cantus_ui::raster::shoot(&ctx, egui::vec2(980.0, 660.0), 1.0, 3, None, |ui| {
        host.refresh();
        gate::editor(&mut host, &mut state, ui);
    });
    assert!(!host.pairs().is_empty(), "the tap reached the editor");
    assert!(
        !state.history.is_empty(),
        "the telemetry reached the history"
    );
    let geo = state.geometry.expect("the editor laid out its displays");
    assert!(geo.history.width() > 100.0 && geo.transfer.width() > 100.0);
    // Something was drawn where the history is: not one flat colour.
    let y = geo.history.center().y as usize;
    let row: Vec<[u8; 4]> = (geo.history.left() as usize..geo.history.right() as usize)
        .map(|x| canvas.pixel(x, y))
        .collect();
    assert!(row.iter().any(|p| *p != row[0]));
}
