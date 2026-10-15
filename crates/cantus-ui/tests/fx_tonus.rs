//! Tonus's editor: what it paints (the detected note, the target, the shift, the cards),
//! how it takes the telemetry's columns, what a click on the keyboard does (one undo step
//! each), the folded settings, its presets and its preview.

mod support;
use cantus_core::*;
use cantus_ui::views::fx::tonus::{self, State};
use cantus_ui::CantusApp;
use support::*;

/// An app with Tonus on the first strip, its editor open.
fn with_tonus() -> (CantusApp, StripId, EffectId) {
    let mut a = app(cantus_ui::starter_project());
    let s0 = a.project().tracks[0].strip.unwrap();
    a.add_effect(s0, EffectKind::AutoTune);
    let fx = a.project().strip(s0).unwrap().effects[0].id;
    a.open_effect_editor(s0, fx);
    (a, s0, fx)
}

fn spec(a: &CantusApp, s: StripId) -> EffectSpec {
    a.project().strip(s).unwrap().effects[0].spec.clone()
}

fn frame(a: &mut CantusApp, ctx: &egui::Context, events: Vec<egui::Event>) {
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

fn click(a: &mut CantusApp, at: egui::Pos2, button: egui::PointerButton) {
    let ctx = ctx();
    let press = |pressed| egui::Event::PointerButton {
        pos: at,
        button,
        pressed,
        modifiers: egui::Modifiers::NONE,
    };
    frame(a, &ctx, vec![]);
    frame(a, &ctx, vec![]);
    frame(a, &ctx, vec![egui::Event::PointerMoved(at)]);
    frame(a, &ctx, vec![press(true)]);
    frame(a, &ctx, vec![press(false)]);
    frame(a, &ctx, vec![]);
}

fn geometry(a: &mut CantusApp) -> tonus::Geometry {
    let ctx = ctx();
    frame(a, &ctx, vec![]);
    frame(a, &ctx, vec![]);
    a.fx.tonus.geometry.expect("the editor drew its display")
}

#[test]
fn the_editor_paints_the_detected_note_its_target_and_its_cards() {
    let (mut a, s0, fx) = with_tonus();
    cantus_ui::views::fx::demo_setup(&mut a, s0, fx, EffectKind::AutoTune);
    cantus_ui::views::fx::demo_feed(&mut a, EffectKind::AutoTune);
    let text = paint_editors(&mut a, egui::vec2(1400.0, 900.0));
    let lower = text.to_lowercase().replace(' ', "");
    for want in [
        "Tonus",
        "Pitch",
        "Correction",
        "Notes",
        "Speed",
        "Amount",
        "Minor",
        "B3",
        "Target",
        "Shift",
        "Sung",
        "MIDI notes",
        "Reset notes",
        "Latency 2.7 ms",
    ] {
        let want = want.replace(' ', "").to_lowercase();
        assert!(lower.contains(&want), "missing {want:?} in {text}");
    }
    assert!(
        a.fx.tonus.history.len() > 300,
        "the display was fed: {}",
        a.fx.tonus.history.len()
    );
    assert_eq!(
        a.fx.tonus.header.target,
        Some(59),
        "the held B3 is the target"
    );
}

/// The telemetry's columns are taken once each, through the counter's wrap, and the
/// history keeps three seconds.
#[test]
fn the_display_takes_each_column_once() {
    use cantus_engine::fx::tonus as tel;
    let block = |seq: u64| {
        let mut t = [0.0f32; 64];
        t[tel::T_SEQ] = (seq % tel::SEQ_WRAP) as f32;
        t[tel::T_SAMPLE_RATE] = 48_000.0;
        t[tel::T_COLUMN_LEN] = 256.0;
        for k in 0..tel::COLUMNS {
            let at = tel::T_RING + tel::T_COLUMN * k;
            t[at] = 60.0 + k as f32;
            t[at + 1] = 60.0;
        }
        t
    };
    let mut st = State::default();
    st.ingest(&block(5));
    assert_eq!(st.history.len(), 5);
    st.ingest(&block(5));
    assert_eq!(st.history.len(), 5, "the same block brings nothing");
    st.ingest(&block(9));
    assert_eq!(st.history.len(), 9);
    // Column 8's slot is 8 mod 24.
    assert_eq!(st.history.back().unwrap().input, 68.0);
    let mut st = State::default();
    st.ingest(&block(tel::SEQ_WRAP - 2));
    let before = st.history.len();
    st.ingest(&block(3));
    assert_eq!(st.history.len() - before, 5, "through the wrap");
    for i in 0..400u64 {
        st.ingest(&block(10 + i * 20));
    }
    assert_eq!(st.history.len(), tonus::HISTORY);
}

/// A click on a key of the scale removes it, a click again brings it back, a right-click
/// bypasses it — one undo step each; Reset puts every note back.
#[test]
fn the_keyboard_sets_each_note_in_one_step() {
    let (mut a, s0, _) = with_tonus();
    // C major: F# is not in it, G is.
    {
        let fx = a.project().strip(s0).unwrap().effects[0].id;
        a.exec(Command::SetEffectParam {
            strip: s0,
            effect: fx,
            key: "scale".into(),
            value: 1.0,
        });
    }
    let g = geometry(&mut a);
    let g_key = g.keys[7].center() + egui::vec2(0.0, g.keys[7].height() * 0.3);
    click(&mut a, g_key, egui::PointerButton::Primary);
    assert_eq!(spec(&a, s0).get("n7"), 2.0, "G removed");
    let g = geometry(&mut a);
    click(&mut a, g.keys[6].center(), egui::PointerButton::Primary);
    assert_eq!(spec(&a, s0).get("n6"), 1.0, "F# made a target");
    let g = geometry(&mut a);
    let e_key = g.keys[4].center() + egui::vec2(0.0, g.keys[4].height() * 0.3);
    click(&mut a, e_key, egui::PointerButton::Secondary);
    assert_eq!(spec(&a, s0).get("n4"), 3.0, "E bypassed");
    a.undo();
    assert_eq!(spec(&a, s0).get("n4"), 0.0, "one undo, one click");
    // A click on G again: back to what the scale says.
    geometry(&mut a);
    click(&mut a, g_key, egui::PointerButton::Primary);
    assert_eq!(spec(&a, s0).get("n7"), 0.0);
}

/// The folded settings appear on *More* and fold again on *Less*.
#[test]
fn more_shows_the_folded_settings() {
    let (mut a, _, _) = with_tonus();
    let text = paint_editors(&mut a, egui::vec2(1400.0, 900.0)).to_lowercase();
    assert!(!text.contains("humanise"), "folded at first");
    a.fx.tonus.more = true;
    let text = paint_editors(&mut a, egui::vec2(1400.0, 900.0)).to_lowercase();
    for want in [
        "humanise",
        "flex",
        "created vibrato",
        "throat",
        "a4",
        "keep",
    ] {
        assert!(text.contains(want), "missing {want:?} in {text}");
    }
}

#[test]
fn the_factory_presets_are_in_range_and_named() {
    assert!(tonus::FACTORY_PRESETS.len() >= 6);
    for want in [
        "preset.tonus.natural",
        "preset.tonus.tight",
        "preset.tonus.hard",
        "preset.tonus.throat_up",
        "preset.tonus.throat_down",
        "preset.tonus.midi_lead",
    ] {
        assert!(tonus::FACTORY_PRESETS.iter().any(|p| p.0 == want), "{want}");
    }
    for (name, values) in tonus::FACTORY_PRESETS {
        assert!(cantus_ui::i18n::t(name) != *name, "{name} has no words");
        for (k, v) in *values {
            let info = EffectKind::AutoTune
                .param(k)
                .unwrap_or_else(|| panic!("{name}: {k}"));
            assert!(*v >= info.min && *v <= info.max, "{name}: {k} = {v}");
            assert!(*v != info.default, "{name}: {k} is its default");
        }
    }
}

#[test]
fn the_preview_says_the_targets_in_one_line() {
    let mut s = EffectSpec::new(EffectKind::AutoTune);
    s.set("key", 9.0);
    s.set("scale", 2.0);
    s.set("speed", 0.0);
    let ctx = ctx();
    let text = paint(&ctx, egui::vec2(420.0, 120.0), 1, |ui| {
        cantus_ui::views::fx::preview(ui, &s);
    });
    assert!(text.contains("A Minor"), "{text}");
    assert!(text.contains("Hard"), "{text}");
}

/// The demo project writes, opens, and plays: the natural voice is corrected (it differs
/// from the line as written), the MIDI voice hears the melody track, and the track for the
/// owner's voice has an input and Tonus on its strip.
#[test]
fn the_demo_project_opens_and_plays_corrected() {
    let dir = tempfile::tempdir().unwrap();
    let path = tonus::demo::write_demo(dir.path()).unwrap();
    let loaded = cantus_project::load(&path, true).expect("the demo opens");
    let p = loaded.project;
    let names: Vec<&str> = p.tracks.iter().map(|t| t.name.as_str()).collect();
    assert_eq!(names.len(), 5, "{names:?}");
    let tonus_on = |i: usize| {
        let s = p.tracks[i].strip.unwrap();
        p.strip(s).unwrap().effects[0].clone()
    };
    assert_eq!(tonus_on(0).spec.kind, EffectKind::AutoTune);
    assert_eq!(
        tonus_on(2).midi_from,
        Some(p.tracks[3].id),
        "the MIDI voice hears the melody"
    );
    assert!(p.tracks[4].input.is_some(), "your voice has an input");
    assert_eq!(tonus_on(4).spec.kind, EffectKind::AutoTune);
    // Render the first voice alone, with and without Tonus.
    let render = |p: &Project| {
        let mut snap = Snapshot::compile(p, 1);
        let errors = cantus_engine::SampleCache::new().attach_audio(&mut snap, 48_000);
        assert!(errors.is_empty(), "{errors:?}");
        let opts = cantus_engine::RenderOptions {
            tail_seconds: 0.1,
            ..Default::default()
        };
        cantus_engine::render(std::sync::Arc::new(snap), &opts)
            .unwrap()
            .samples
    };
    let corrected = render(&p);
    let mut dry = p.clone();
    let s0 = dry.tracks[0].strip.unwrap();
    dry.strip_mut(s0).unwrap().effects.clear();
    let plain = render(&dry);
    assert!(corrected.iter().all(|v| v.is_finite()));
    let peak = corrected.iter().fold(0.0f32, |m, v| m.max(v.abs()));
    assert!(peak > 0.05, "it sounds: {peak}");
    assert!(corrected != plain, "Tonus changed the voice");
    // And it tuned it: the first note, sung 28 cents sharp of A3, comes out on A3.
    let left: Vec<f32> = corrected.chunks_exact(2).map(|f| f[0]).collect();
    let a = cantus_dsp::pitch::analyse(&left, 48_000.0);
    let sung: Vec<f32> = a
        .frames
        .iter()
        .filter(|f| f.voiced && f.centre > 0.6 * 48_000.0 && f.centre < 0.9 * 48_000.0)
        .map(|f| cantus_dsp::pitch::hz_to_midi(f.f0))
        .collect();
    let mean = sung.iter().sum::<f32>() / sung.len().max(1) as f32;
    assert!(
        sung.len() > 20 && (mean - 57.0).abs() < 0.08,
        "A3 came out at {mean}"
    );
}

/// Review 1: with a transposition (the octave-down preset's −12, the knob's −24), what
/// comes out is drawn where it is — the lanes span both curves — not pinned flat along the
/// graph's edge.
#[test]
fn the_display_keeps_a_transposed_output_in_view() {
    // Before any telemetry (the unit has not run): around A3, not at note 0.
    let (mut a, _, _) = with_tonus();
    let (lo, lanes) = geometry(&mut a).lanes;
    assert!(
        (40..=57).contains(&lo),
        "no telemetry yet: lanes from {lo}, {lanes} of them"
    );
    for trans in [-12.0, -24.0, 24.0] {
        let (mut a, s0, fx) = with_tonus();
        cantus_ui::views::fx::demo_setup(&mut a, s0, fx, EffectKind::AutoTune);
        a.exec(Command::SetEffectParam {
            strip: s0,
            effect: fx,
            key: "trans".into(),
            value: trans,
        });
        cantus_ui::views::fx::demo_feed(&mut a, EffectKind::AutoTune);
        let g = geometry(&mut a);
        let (lo, lanes) = g.lanes;
        let (bottom, top) = (lo as f32 - 0.5, (lo + lanes) as f32 - 0.5);
        let (mut ins, mut outs) = (0, 0);
        for c in a.fx.tonus.history.iter().rev().take(120) {
            if c.input > 0.0 {
                ins += 1;
                let p = c.input;
                assert!(
                    p > bottom && p < top,
                    "{trans}: sung {p} out of {lo}+{lanes}"
                );
            }
            if c.output > 0.0 {
                outs += 1;
                let p = c.output;
                assert!(
                    p > bottom && p < top,
                    "{trans}: out {p} out of {lo}+{lanes}"
                );
            }
        }
        assert!(ins > 20 && outs > 20, "{trans}: {ins} sung, {outs} out");
    }
}

/// The hint a hover at `at` writes.
fn hover(a: &mut CantusApp, at: egui::Pos2) -> cantus_ui::kit::hint::Hint {
    let ctx = ctx();
    frame(a, &ctx, vec![]);
    frame(a, &ctx, vec![]);
    frame(a, &ctx, vec![egui::Event::PointerMoved(at)]);
    frame(a, &ctx, vec![]);
    cantus_ui::kit::hint::current(&ctx).expect("a hint under the pointer")
}

/// Review 1: Speed and Amount explain themselves on hover — what 0 ms means, the natural
/// range, what 0 % leaves — with the value beside it.
#[test]
fn speed_and_amount_explain_themselves_on_hover() {
    let (mut a, s0, fx) = with_tonus();
    a.exec(Command::SetEffectParam {
        strip: s0,
        effect: fx,
        key: "speed".into(),
        value: 0.0,
    });
    let g = geometry(&mut a);
    let h = hover(&mut a, g.speed.center());
    assert_eq!(h.name, cantus_ui::i18n::t("tonus.speed_tip"));
    assert_eq!(h.value, "Hard");
    let g = geometry(&mut a);
    let h = hover(&mut a, g.amount.center());
    assert_eq!(h.name, cantus_ui::i18n::t("tonus.amount_tip"));
    assert_eq!(h.value, "100%");
}

/// Review 1: *Reset notes* is its own step in the undo history, not a preset.
#[test]
fn reset_notes_is_named_in_the_history() {
    let (mut a, s0, fx) = with_tonus();
    a.exec(Command::SetEffectParam {
        strip: s0,
        effect: fx,
        key: "n7".into(),
        value: 2.0,
    });
    let g = geometry(&mut a);
    click(&mut a, g.reset.center(), egui::PointerButton::Primary);
    assert_eq!(spec(&a, s0).get("n7"), 0.0, "reset");
    assert_eq!(a.bus.undo_label_key(), Some("cmd.tonus_reset_notes"));
    assert!(cantus_ui::i18n::t("cmd.tonus_reset_notes") != "cmd.tonus_reset_notes");
}

/// Review 1: the preview words a speed of 0 as *Hard* whatever the source, as the editor
/// does.
#[test]
fn the_preview_says_hard_for_every_source() {
    for src in [0.0, 1.0, 2.0] {
        let mut s = EffectSpec::new(EffectKind::AutoTune);
        s.set("src", src);
        s.set("speed", 0.0);
        let ctx = ctx();
        let text = paint(&ctx, egui::vec2(420.0, 120.0), 1, |ui| {
            cantus_ui::views::fx::preview(ui, &s);
        });
        assert!(text.contains("Hard"), "src {src}: {text}");
        assert!(!text.contains("0 ms"), "src {src}: {text}");
    }
}

/// Review 1: the MIDI source's messages are sentences on a body line — not a caption's
/// abbreviated capitals — and fit the source column (236 pt) in both languages.
#[test]
fn the_midi_messages_are_whole_sentences_in_both_languages() {
    use cantus_ui::i18n::{set_locale, t, Locale};
    use cantus_ui::kit::text::{measure, Role, Text};
    for locale in [Locale::En, Locale::Es] {
        set_locale(locale);
        let ctx = ctx();
        for key in ["tonus.midi_host", "tonus.midi_waiting"] {
            let mut w = 0.0;
            paint(&ctx, egui::vec2(400.0, 100.0), 1, |ui| {
                let s = t(key);
                let words = Text::new(&s, Role::Body, egui::Color32::WHITE);
                w = measure(ui.ctx(), &words, 1.0).x;
            });
            assert!(w <= 236.0, "{locale:?} {key}: {w} pt for {:?}", t(key));
        }
        // In Cantus, the MIDI source with no track chosen says so as written.
        let (mut a, s0, fx) = with_tonus();
        a.exec(Command::SetEffectParam {
            strip: s0,
            effect: fx,
            key: "src".into(),
            value: 1.0,
        });
        let text = paint_editors(&mut a, egui::vec2(1400.0, 900.0));
        let want = t("tonus.midi_waiting");
        assert!(text.contains(&want), "{locale:?}: {want:?} in {text}");
    }
    set_locale(Locale::En);
}
