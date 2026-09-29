//! Porta's editor: what it paints, what its gestures change (one undo step each), and the
//! host's advantages — learning the thresholds, and hits turned into a MIDI clip.

mod support;
use cantus_core::*;
use cantus_dsp::porta::{self, telemetry, Column, Phase};
use cantus_ui::views::fx::gate::{self, State};
use cantus_ui::CantusApp;
use support::*;

/// An app with a gate on the first strip, its editor open.
fn with_gate() -> (CantusApp, StripId, EffectId) {
    let mut a = app(cantus_ui::starter_project());
    let s0 = a.project().tracks[0].strip.unwrap();
    a.add_effect(s0, EffectKind::Gate);
    let fx = a.project().strip(s0).unwrap().effects[0].id;
    a.open_effect_editor(s0, fx);
    (a, s0, fx)
}

fn spec(a: &CantusApp, s: StripId) -> EffectSpec {
    a.project().strip(s).unwrap().effects[0].spec.clone()
}

/// Run frames of the editor window with these events.
fn frame(a: &mut CantusApp, ctx: &egui::Context, events: Vec<egui::Event>) {
    frame_with(a, ctx, events, egui::Modifiers::NONE);
}

/// The same, with keys held.
fn frame_with(
    a: &mut CantusApp,
    ctx: &egui::Context,
    mut events: Vec<egui::Event>,
    modifiers: egui::Modifiers,
) {
    events.insert(0, egui::Event::ModifiersChanged(modifiers));
    let input = egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(1400.0, 900.0),
        )),
        events,
        ..Default::default()
    };
    ctx.run_ui(input, |ui| cantus_ui::views::fx::window(a, ui.ctx()))
        .textures_delta
        .clear();
}

/// Press at `from`, move to `to` in steps, release — with these modifiers.
fn drag(a: &mut CantusApp, from: egui::Pos2, to: egui::Pos2, modifiers: egui::Modifiers) {
    let ctx = ctx();
    let button = |pos, pressed| egui::Event::PointerButton {
        pos,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers,
    };
    frame(a, &ctx, vec![]);
    frame(a, &ctx, vec![]);
    let m = modifiers;
    frame_with(a, &ctx, vec![egui::Event::PointerMoved(from)], m);
    frame_with(
        a,
        &ctx,
        vec![egui::Event::PointerMoved(from), button(from, true)],
        m,
    );
    for i in 1..=4 {
        let at = from + (to - from) * (i as f32 / 4.0);
        frame_with(a, &ctx, vec![egui::Event::PointerMoved(at)], m);
    }
    frame_with(
        a,
        &ctx,
        vec![egui::Event::PointerMoved(to), button(to, false)],
        m,
    );
    frame(a, &ctx, vec![]);
}

/// The geometry the last frame published.
fn geometry(a: &mut CantusApp) -> gate::Geometry {
    let ctx = ctx();
    frame(a, &ctx, vec![]);
    frame(a, &ctx, vec![]);
    a.fx.gate.geometry.expect("the editor drew its displays")
}

#[test]
fn the_editor_paints_its_displays_and_says_what_the_style_is() {
    let (mut a, _, _) = with_gate();
    cantus_ui::views::fx::demo_feed(&mut a, EffectKind::Gate);
    let text = paint_editors(&mut a, egui::vec2(1400.0, 900.0));
    // Labels and section names are set in capitals, and a label wider than its cell
    // takes its short form (the full name is on the help line).
    // A value's unit is set a hair apart, not after a space ("-40.0 dB" paints as
    // "-40.0" and "dB"): compare without spaces.
    let lower = text.to_lowercase().replace(' ', "");
    for want in [
        "Classic",
        "Ducking",
        "peak detector, lets go over 10 ms",
        "Open",
        "-40.0dB",
        "Close",
        "-44.0dB",
        "Removed",
        "Learn",
        "Threshold",
        "Hyst",
        "Look",
        "No latency",
        "DETECT",
        "hold",
        "release",
    ] {
        let want = want.replace(' ', "").to_lowercase();
        assert!(lower.contains(&want), "missing {want:?} in {text}");
    }
    assert!(
        a.fx.gate.history.len() > 700,
        "four seconds of history fed: {}",
        a.fx.gate.history.len()
    );
}

#[test]
fn the_preview_says_the_settings_in_one_line() {
    let s = EffectSpec::new(EffectKind::Gate);
    let ctx = ctx();
    let text = paint(&ctx, egui::vec2(400.0, 120.0), 1, |ui| {
        cantus_ui::views::fx::preview(ui, &s);
    });
    assert!(text.contains("Open -40"), "{text}");
    assert!(text.contains("Close -44"), "{text}");
    assert!(text.contains("\u{221e}:1"), "{text}");
}

/// A telemetry block: `seq` columns made so far (column `j` of the block has key −`j` dB),
/// `hits` hits so far, the newest `ago` samples before the end of the block.
fn telemetry_with(seq: u32, hits: u32, ago: f32) -> [f32; 64] {
    let mut t = [0.0f32; 64];
    t[telemetry::SEQ] = (seq % telemetry::WRAP) as f32;
    t[telemetry::COLUMN_MS] = 5.0;
    t[telemetry::HITS] = hits as f32;
    t[telemetry::SAMPLE_RATE] = 48_000.0;
    t[telemetry::EVENTS_AT] = ago;
    t[telemetry::EVENTS_AT + 1] = -6.0;
    for j in 0..porta::HISTORY_COLUMNS {
        let c = Column {
            key_db: -(j as f32),
            phase: Phase::Open,
            ..Column::default()
        };
        let [x, y] = telemetry::pack(&c);
        t[telemetry::COLUMNS_AT + 2 * j] = x;
        t[telemetry::COLUMNS_AT + 2 * j + 1] = y;
    }
    t
}

/// The editor takes exactly the columns that are new: none twice, none lost while it
/// keeps up, and through the counter's wrap.
#[test]
fn the_history_takes_each_column_once() {
    let mut st = State::default();
    st.ingest(&telemetry_with(5, 0, 0.0), None, 1_920.0);
    assert_eq!(st.history.len(), 5, "a young gate: only what it made");
    st.ingest(&telemetry_with(5, 0, 0.0), None, 1_920.0);
    assert_eq!(st.history.len(), 5, "the same block again brings nothing");
    st.ingest(&telemetry_with(8, 0, 0.0), None, 1_920.0);
    assert_eq!(st.history.len(), 8);
    // The three new ones are the newest three of the block (keys −21, −22, −23 dB).
    let keys: Vec<f32> = st
        .history
        .iter()
        .rev()
        .take(3)
        .map(|c| c.c.key_db)
        .collect();
    assert_eq!(keys, vec![-23.0, -22.0, -21.0]);
    // Across the wrap of the counter.
    let mut st = State::default();
    st.ingest(&telemetry_with(telemetry::WRAP - 2, 0, 0.0), None, 1_920.0);
    let before = st.history.len();
    st.ingest(&telemetry_with(3, 0, 0.0), None, 1_920.0);
    assert_eq!(st.history.len() - before, 5);
    // Never more than it keeps.
    for i in 0..100 {
        st.ingest(&telemetry_with(100 + i * 24, 0, 0.0), None, 1_920.0);
    }
    assert_eq!(st.history.len(), gate::HISTORY);
}

/// While capturing and playing, a hit is stamped at the song position where the key
/// crossed: the block's end, less how long ago it was.
#[test]
fn a_captured_hit_lands_where_it_happened() {
    let mut st = State::default();
    st.capturing = true;
    // 120 BPM: 1 920 ticks a second. The block ends at tick 9 600 (5 s).
    st.ingest(&telemetry_with(10, 0, 0.0), Some(9_600), 1_920.0);
    st.ingest(&telemetry_with(11, 1, 24_000.0), Some(9_600), 1_920.0);
    assert_eq!(
        st.hits,
        vec![(9_600 - 960, -6.0)],
        "half a second before the end"
    );
    // Stopped, nothing is captured.
    st.ingest(&telemetry_with(12, 2, 100.0), None, 1_920.0);
    assert_eq!(st.hits.len(), 1);
}

#[test]
fn hits_close_together_merge_and_velocities_keep_the_dynamics() {
    let merged = gate::merge_hits(&[(1_000, -10.0), (1_010, -6.0), (5_000, -20.0)], 1_920.0);
    assert_eq!(merged, vec![(1_000, -6.0), (5_000, -20.0)]);
    assert_eq!(
        gate::velocities(&[-6.0, -26.0, -46.0, -80.0]),
        vec![127, 64, 1, 1]
    );
}

/// Drag the open line down on the history: the threshold follows the pointer and the
/// close line keeps its distance; one undo takes the whole drag back.
#[test]
fn dragging_the_open_line_moves_the_threshold_in_one_undo_step() {
    let (mut a, s0, _) = with_gate();
    let geo = geometry(&mut a);
    let x = geo.history.center().x;
    let from = egui::pos2(x, geo.history_y(-40.0));
    let to = egui::pos2(x, geo.history_y(-25.0));
    drag(&mut a, from, to, egui::Modifiers::NONE);
    let s = spec(&a, s0);
    assert!((s.get("thr") - -25.0).abs() < 0.5, "thr {}", s.get("thr"));
    assert_eq!(s.get("hyst"), 4.0, "the close line keeps its distance");
    a.undo();
    assert_eq!(spec(&a, s0).get("thr"), -40.0, "one undo, the whole drag");
}

/// Drag the dashed line: the close threshold moves, the open one stays — hysteresis set
/// by where it closes. Alt on the open line moves it alone.
#[test]
fn dragging_the_close_line_sets_the_hysteresis() {
    let (mut a, s0, _) = with_gate();
    let geo = geometry(&mut a);
    let x = geo.history.center().x;
    drag(
        &mut a,
        egui::pos2(x, geo.history_y(-44.0)),
        egui::pos2(x, geo.history_y(-52.0)),
        egui::Modifiers::NONE,
    );
    let s = spec(&a, s0);
    assert_eq!(s.get("thr"), -40.0);
    assert!((s.get("hyst") - 12.0).abs() < 0.5, "hyst {}", s.get("hyst"));
    // Alt: the open line alone; the close line stays at −52.
    let geo = geometry(&mut a);
    drag(
        &mut a,
        egui::pos2(x, geo.history_y(-40.0)),
        egui::pos2(x, geo.history_y(-46.0)),
        egui::Modifiers::ALT,
    );
    let s = spec(&a, s0);
    assert!((s.get("thr") - -46.0).abs() < 0.5, "thr {}", s.get("thr"));
    let close = s.get("thr") - s.get("hyst");
    assert!(
        (close - -52.0).abs() < 0.5,
        "the close line stayed: {close}"
    );
}

/// On the transfer curve: the square is the range.
#[test]
fn dragging_the_floor_sets_the_range() {
    let (mut a, s0, fx) = with_gate();
    a.set_fx_param(s0, fx, "range", 20.0, false);
    let geo = geometry(&mut a);
    // The floor handle sits under the threshold, the range below it.
    let from = geo.transfer_pos(-40.0, -60.0);
    let to = geo.transfer_pos(-40.0, -70.0);
    drag(&mut a, from, to, egui::Modifiers::NONE);
    let r = spec(&a, s0).get("range");
    assert!((r - 30.0).abs() < 1.0, "range {r}");
}

/// Learn: a passage of hits over bleed sets both thresholds, as one undo step.
#[test]
fn learning_sets_both_thresholds_in_one_step() {
    let (mut a, s0, fx) = with_gate();
    let mut levels = Vec::new();
    for _ in 0..6 {
        levels.extend((0..40).map(|j| -8.0 - 30.0 * j as f32 / 40.0));
        levels.extend((0..60).map(|j| -46.0 + (j % 7) as f32));
    }
    a.fx.gate.learning = Some(levels);
    let got = a
        .finish_gate_learning(s0, fx)
        .expect("a passage to learn from");
    assert!(got.two_levels);
    let s = spec(&a, s0);
    assert!((s.get("thr") - got.open_db).abs() < 1e-4);
    assert!((s.get("thr") - s.get("hyst") - got.close_db).abs() < 1e-3);
    assert!(
        s.get("thr") - s.get("hyst") > -40.0,
        "closes above the bleed"
    );
    a.undo();
    let s = spec(&a, s0);
    assert_eq!((s.get("thr"), s.get("hyst")), (-40.0, 4.0));
    assert!(a.fx.gate.learning.is_none());
}

/// The captured hits become a drum track with one note per hit, velocity from the peak —
/// and one undo removes it all.
#[test]
fn hits_become_a_midi_clip_on_a_new_drum_track() {
    let (mut a, s0, _) = with_gate();
    let tracks = a.project().tracks.len();
    let bar = a.project().time_signature.ticks_per_bar().0;
    a.fx.gate.hits = vec![
        (bar + 100, -6.0),
        (bar + 2_000, -26.0),
        (bar + 2_005, -30.0), // the same hit, twice (a loop): merged
        (2 * bar + 50, -12.0),
    ];
    a.fx.gate.hit_pitch = Some(38);
    assert!(a.gate_hits_to_clip(s0));
    let p = a.project();
    assert_eq!(p.tracks.len(), tracks + 1);
    let t = p.tracks.last().unwrap();
    assert!(matches!(t.instrument, InstrumentSpec::Pulsus(_)));
    let clip = &t.clips[0];
    assert_eq!(
        clip.start,
        Ticks(bar),
        "the clip starts on the bar of the first hit"
    );
    assert_eq!(
        clip.length,
        Ticks(2 * bar),
        "and ends on the bar after the last"
    );
    let notes: Vec<(i64, u8, u8)> = clip
        .notes
        .iter()
        .map(|n| (n.start.0, n.pitch, n.velocity))
        .collect();
    assert_eq!(
        notes,
        vec![(100, 38, 127), (2_000, 38, 64), (bar + 50, 38, 108)]
    );
    assert!(a.fx.gate.hits.is_empty(), "written, so cleared");
    a.undo();
    assert_eq!(
        a.project().tracks.len(),
        tracks,
        "one undo removes the track"
    );
}

/// Every factory preset names real parameters with values in range, and its name exists
/// in both languages.
#[test]
fn the_presets_are_real() {
    let presets = cantus_ui::views::fx::factory_presets(EffectKind::Gate);
    assert!(presets.len() >= 5);
    for (label, params) in presets {
        for locale in [cantus_ui::i18n::Locale::En, cantus_ui::i18n::Locale::Es] {
            cantus_ui::i18n::set_locale(locale);
            assert_ne!(cantus_ui::i18n::t(label), *label, "{label} in {locale:?}");
        }
        cantus_ui::i18n::set_locale(cantus_ui::i18n::Locale::En);
        for (k, v) in params.iter() {
            let info = EffectKind::Gate
                .param(k)
                .unwrap_or_else(|| panic!("{label}: {k}"));
            assert!(*v >= info.min && *v <= info.max, "{label}: {k} = {v}");
        }
    }
    for want in ["Kick", "Snare", "Toms", "Vocal", "Guitar"] {
        assert!(
            presets.iter().any(|(l, _)| cantus_ui::i18n::t(l) == want),
            "no {want} preset"
        );
    }
}
