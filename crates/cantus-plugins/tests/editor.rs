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

/// Review 1: the exported Speculum's tap carries its own frames, left and right, every one
/// in order, and the clock it gives is the newest frame's (the block's last, not its
/// start); another kind's editor has no such stream.
#[test]
fn the_exported_speculum_reads_its_own_stereo_frames_and_the_newest_frames_beat() {
    use cantus_plugins::instance::HostTransport;
    let shared = Shared::new(desc::of(EffectKind::Analyzer).unwrap());
    shared.editor_open.store(true, Ordering::Release);
    let mut proc_ = Processor::new(shared.clone(), 48_000.0);
    let mut ph = PluginHost::new(shared.clone(), Box::new(Collect(Default::default())));
    let n = 480;
    let l: Vec<f32> = (0..n)
        .map(|i| cantus_dsp::math::sin(i as f32 * 0.05) * 0.5)
        .collect();
    let r: Vec<f32> = (0..n)
        .map(|i| cantus_dsp::math::cos(i as f32 * 0.11) * 0.3)
        .collect();
    proc_.set_transport(Some(HostTransport {
        playing: true,
        tempo: Some(120.0),
        beats: Some(2.0),
        bar_start: Some(0.0),
        time_sig: Some((4, 4)),
        loop_beats: None,
        seconds: None,
    }));
    let (mut a, mut b) = (l.clone(), r.clone());
    proc_.process(&mut a, &mut b, None, &[]);
    ph.refresh();
    let mut out = Vec::new();
    let take = ph.take_stereo(&mut out);
    assert!(take.stream);
    assert_eq!(out.len(), n);
    for (i, (x, y)) in out.iter().enumerate() {
        assert_eq!(
            (x.to_bits(), y.to_bits()),
            (l[i].to_bits(), r[i].to_bits()),
            "frame {i}"
        );
    }
    let (beat, tempo) = take.clock.expect("playing");
    assert_eq!(tempo, 120.0);
    let want = 2.0 + (n - 1) as f64 * 120.0 / 60.0 / 48_000.0;
    assert!((beat - want).abs() < 1e-9, "{beat} vs {want}");
    // A gate's editor has no stereo stream.
    let (_, mut gate_host, _) = host();
    let mut none = Vec::new();
    assert_eq!(gate_host.take_stereo(&mut none), Default::default());
    assert!(none.is_empty());
}

/// A Speculum processor and its editor's host, the editor open.
fn speculum() -> (Processor, PluginHost) {
    let shared = Shared::new(desc::of(EffectKind::Analyzer).unwrap());
    shared.editor_open.store(true, Ordering::Release);
    let proc_ = Processor::new(shared.clone(), 48_000.0);
    let ph = PluginHost::new(shared, Box::new(Collect(Default::default())));
    (proc_, ph)
}

/// One playing block of `n` frames from `beats` at 120 BPM.
fn block(p: &mut Processor, n: usize, beats: f64) {
    use cantus_plugins::instance::HostTransport;
    p.set_transport(Some(HostTransport {
        playing: true,
        tempo: Some(120.0),
        beats: Some(beats),
        bar_start: Some(0.0),
        time_sig: Some((4, 4)),
        loop_beats: None,
        seconds: None,
    }));
    let (mut a, mut b) = (vec![0.25f32; n], vec![-0.25f32; n]);
    p.process(&mut a, &mut b, None, &[]);
}

/// Review 2: the clock dates the newest frame the editor drained, not the newest block
/// the host ran — a block played between the drain and the drawing does not move it.
#[test]
fn the_exported_speculums_clock_travels_with_its_frames() {
    let (mut p, mut ph) = speculum();
    let pf = 120.0 / 60.0 / 48_000.0;
    block(&mut p, 480, 2.0);
    ph.refresh();
    // The host runs another block before the editor draws.
    block(&mut p, 480, 2.0 + 480.0 * pf);
    let mut out = Vec::new();
    let take = ph.take_stereo(&mut out);
    assert_eq!(out.len(), 480);
    let (beat, _) = take.clock.expect("playing");
    let want = 2.0 + 479.0 * pf;
    assert!((beat - want).abs() < 1e-9, "{beat} vs {want}");
    // The next frame drains that block, and the clock its last frame.
    ph.refresh();
    let take = ph.take_stereo(&mut out);
    let (beat, _) = take.clock.unwrap();
    let want = 2.0 + 959.0 * pf;
    assert!((beat - want).abs() < 1e-9, "{beat} vs {want}");
}

/// Review 2: what the tap could not take (the editor drained too late) and what its window
/// could not keep are counted, once, as the gaps Speculum reports.
#[test]
fn the_exported_speculum_counts_what_it_lost() {
    use cantus_plugins::instance::PAIRS;
    // The tap full: 20 000 frames before the editor drains.
    let (mut p, mut ph) = speculum();
    for k in 0..20 {
        block(&mut p, 1_000, k as f64);
    }
    ph.refresh();
    let mut out = Vec::new();
    let take = ph.take_stereo(&mut out);
    assert_eq!(out.len(), PAIRS);
    assert_eq!(take.lost, (20_000 - PAIRS) as u64, "the tap's");
    block(&mut p, 1_000, 30.0);
    ph.refresh();
    out.clear();
    assert_eq!(ph.take_stereo(&mut out).lost, 0, "counted once");
    // The window full: drained twice before Speculum takes it.
    let (mut p, mut ph) = speculum();
    for k in 0..2 {
        for j in 0..10 {
            block(&mut p, 1_000, (10 * k + j) as f64);
        }
        ph.refresh();
    }
    out.clear();
    let take = ph.take_stereo(&mut out);
    assert_eq!(out.len(), PAIRS);
    assert_eq!(take.lost, (20_000 - PAIRS) as u64, "the window's");
}
