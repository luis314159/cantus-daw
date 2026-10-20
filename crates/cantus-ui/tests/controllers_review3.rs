//! Review 3 of ADR 0062 (round 29): each confirmed finding about the interface's
//! controllers after the second fix round, held by a test — two-state and stepped targets
//! and their words, 14-bit pairs across frames, feedback's echo, frozen targets,
//! recording's bounds, a new project, the global file, names, ports, the mixer's switches,
//! and learning everywhere checked by clicking.

mod support;
use cantus_core::controller::*;
use cantus_core::*;
use cantus_engine::midi::Heard;
use cantus_engine::MidiEvent;
use cantus_ui::controllers::Scope;
use cantus_ui::CantusApp;
use std::time::{Duration, Instant};
use support::*;

fn msg_at(status: u8, data1: u8, data2: u8, at: Instant) -> Heard {
    Heard {
        port: 0,
        event: MidiEvent {
            status,
            data1,
            data2,
        },
        at,
    }
}

fn cc(n: u8, v: u8) -> Heard {
    msg_at(0xB0, n, v, Instant::now())
}

fn keys_app() -> CantusApp {
    keys_app_with(cantus_ui::starter_project())
}

fn keys_app_with(p: Project) -> CantusApp {
    let mut a = app(p);
    a.midi_port_names = vec!["Keys".into()];
    a
}

fn strip(a: &CantusApp) -> StripId {
    a.project().tracks[0].strip.expect("a strip")
}

fn source(control: Control) -> MidiSource {
    MidiSource {
        port: PortRef {
            name: "Keys".into(),
            ordinal: 0,
        },
        channel: Some(0),
        control,
    }
}

fn add(
    a: &mut CantusApp,
    scope: Scope,
    id: u64,
    control: Control,
    target: MappingTarget,
    mode: MapMode,
    takeover: Takeover,
) {
    let mut m = Mapping::new(MappingId(id), source(control), target, mode);
    m.takeover = takeover;
    assert!(a.add_mapping(scope, m));
}

/// A compressor on the first strip: its id.
fn compressor(a: &mut CantusApp) -> EffectId {
    let s = strip(a);
    a.add_effect(s, EffectKind::Compressor);
    a.project().strip(s).unwrap().effects.last().unwrap().id
}

fn param(a: &CantusApp, fx: EffectId, key: &str) -> f32 {
    let s = a.project().effect_host(fx).unwrap();
    a.project().effect(s, fx).unwrap().spec.get(key)
}

fn effect(fx: EffectId, key: &str) -> MappingTarget {
    MappingTarget::Param(AutoTarget::Effect {
        effect: fx,
        key: key.into(),
    })
}

// ---- two states and steps -----------------------------------------------------------------

/// #0: a two-state parameter (the compressor's key EQ switch) is a switch: a latching
/// button's first press switches it under pick-up; a pad learnt on it toggles.
#[test]
fn a_two_state_parameter_is_a_switch() {
    let mut a = keys_app();
    let fx = compressor(&mut a);
    let on = param(&a, fx, "key_eq");
    add(
        &mut a,
        Scope::Project,
        1,
        Control::Cc(20),
        effect(fx, "key_eq"),
        MapMode::Absolute,
        Takeover::Pickup,
    );
    let want = if on >= 0.5 { 0 } else { 127 };
    a.controller_heard(&[cc(20, want)]);
    assert_ne!(param(&a, fx, "key_eq") >= 0.5, on >= 0.5, "the first press");
    a.learn_target(effect(fx, "key_eq"));
    a.controller_heard(&[msg_at(0x99, 36, 90, Instant::now())]);
    let m = a
        .project()
        .controllers
        .mappings
        .iter()
        .find(|m| m.source.control == Control::Note(36))
        .unwrap()
        .clone();
    assert_eq!(m.mode, MapMode::Toggle);
}

/// #1: an encoder on a stepped choice (nine styles) moves one style per detent.
#[test]
fn an_encoder_moves_one_style_per_detent() {
    let mut a = keys_app();
    let fx = compressor(&mut a);
    a.exec(Command::SetEffectParam {
        strip: strip(&a),
        effect: fx,
        key: "style".into(),
        value: 0.0,
    });
    add(
        &mut a,
        Scope::Project,
        1,
        Control::Cc(16),
        effect(fx, "style"),
        MapMode::Relative(Encoding::TwosComplement),
        Takeover::Jump,
    );
    for want in 1..=3 {
        a.controller_heard(&[cc(16, 1)]);
        assert_eq!(param(&a, fx, "style"), want as f32);
    }
}

/// #2, #15, #23, #24, #37, #42, #53: a stepped choice reads its names in the pane, and a
/// name is taken back typed, in either language; a two-state parameter reads Off and On.
#[test]
fn a_stepped_choice_reads_its_names() {
    use cantus_ui::views::controllers::{parse_value, value_words};
    let mut a = keys_app();
    let fx = compressor(&mut a);
    let style = effect(fx, "style");
    assert_eq!(value_words(&a, &style, 0.0), "Clean");
    assert_eq!(value_words(&a, &style, 1.0), "Classic");
    assert_eq!(value_words(&a, &effect(fx, "key_eq"), 1.0), "On");
    let opto = parse_value(&a, &style, "opto").expect("a name");
    assert!((opto - 2.0 / 8.0).abs() < 1e-4, "{opto}");
    // In Spanish, and its English name still understood.
    cantus_ui::i18n::set_locale(cantus_ui::i18n::Locale::Es);
    assert_eq!(value_words(&a, &style, 0.0), "Limpio");
    assert!(parse_value(&a, &style, "Clean").is_some());
    cantus_ui::i18n::set_locale(cantus_ui::i18n::Locale::En);
}

/// #2: an instrument's choice reads the words its editor draws.
#[test]
fn an_instruments_choice_reads_its_editors_words() {
    use cantus_ui::views::controllers::value_words;
    let mut a = keys_app();
    let track = a.project().tracks[0].id;
    a.set_instrument(
        track,
        InstrumentSpec::Native(NativeSpec::new(InstrumentKind::Analog)),
    );
    cantus_ui::views::inst::demo_setup(&mut a, track, InstrumentKind::Analog);
    a.open_inst_editor(track);
    use cantus_ui::views::inst::View;
    for view in [View::Play, View::Edit, View::Matrix] {
        a.inst.view = view;
        paint(&ctx(), egui::vec2(1800.0, 1200.0), 3, |ui| {
            cantus_ui::views::inst::window_in(&mut a, ui)
        });
    }
    let target = MappingTarget::Param(AutoTarget::Instrument {
        track,
        key: "vcf.model".into(),
    });
    let words = value_words(&a, &target, 0.0);
    assert!(
        words.chars().any(|c| c.is_alphabetic()),
        "a name, not a number: {words}"
    );
}

// ---- 14-bit pairs across frames ------------------------------------------------------------

fn wide_fader() -> CantusApp {
    let mut a = keys_app();
    add(
        &mut a,
        Scope::Global,
        1,
        Control::Cc14(7),
        MappingTarget::Param(AutoTarget::MasterWidth),
        MapMode::Absolute,
        Takeover::Jump,
    );
    a
}

fn width_of(v: u16) -> f32 {
    f32::from(v) / 16_383.0 * 2.0
}

/// #5, #9, #27, #38, #56: a pair split between two frames is never driven half-made; a
/// coarse half alone is read once [`PAIR_GAP`] has passed.
#[test]
fn a_pair_split_between_frames_is_never_half_made() {
    let mut a = wide_fader();
    a.controller_heard(&[cc(7, 10), cc(39, 5)]);
    let before = a.project().master.width;
    let t = Instant::now();
    a.controller_heard(&[msg_at(0xB0, 7, 11, t)]);
    assert_eq!(a.project().master.width, before, "the coarse half waits");
    a.controller_heard(&[msg_at(0xB0, 39, 3, t + Duration::from_micros(600))]);
    assert!((a.project().master.width - width_of(11 << 7 | 3)).abs() < 1e-4);
    // Alone: read with a fine 0 once the gap has passed.
    let old = Instant::now() - Duration::from_millis(20);
    a.controller_heard(&[msg_at(0xB0, 7, 20, old)]);
    assert!((a.project().master.width - width_of(20 << 7)).abs() < 1e-4);
}

/// #9 (its third case), #56: a 14-bit fader moved while a pass records, each pair split
/// between two frames: every recorded point is a whole pair, never a coarse half with a
/// fine 0.
#[test]
fn a_split_pair_records_no_half_made_point() {
    let mut a = keys_app();
    let s = strip(&a);
    add(
        &mut a,
        Scope::Project,
        1,
        Control::Cc14(7),
        MappingTarget::Param(AutoTarget::StripPan(s)),
        MapMode::Absolute,
        Takeover::Jump,
    );
    a.controller_heard(&[cc(7, 30), cc(39, 3)]);
    a.engine.seek(bar(1));
    a.start_recording();
    let mut t = Instant::now() + Duration::from_millis(50);
    for (i, coarse) in (31..41u8).enumerate() {
        // The fine half in the next frame, the playhead a little on.
        let at = Ticks::beats(4 + i as i64);
        a.engine.set_heard_for_tests(at, true);
        a.controller_heard(&[msg_at(0xB0, 7, coarse, t)]);
        a.engine.set_heard_for_tests(at + Ticks(4), true);
        a.controller_heard(&[msg_at(0xB0, 39, 3, t + Duration::from_micros(600))]);
        t += Duration::from_millis(30);
    }
    a.engine.set_heard_for_tests(bar(4), false);
    a.stop_recording();
    let clips: Vec<Clip> = a
        .project()
        .tracks
        .iter()
        .flat_map(|t| t.clips.iter())
        .filter(|c| {
            c.automation
                .as_ref()
                .is_some_and(|x| x.target == AutoTarget::StripPan(s))
        })
        .cloned()
        .collect();
    assert_eq!(clips.len(), 1, "one pass, one clip");
    let points = &clips[0].automation.as_ref().unwrap().points;
    assert!(points.len() >= 2, "{points:?}");
    for p in points {
        let v = (p.value * 16_383.0).round() as u16;
        assert_eq!(v & 0x7F, 3, "a half-made point: {} ({v})", p.value);
    }
}

/// #4, #9, #22, #27: a controller that sends a fine half alone (its coarse half the same)
/// and then a normal pair coarse half first never becomes "fine half first".
#[test]
fn a_lone_fine_half_proves_nothing() {
    let mut a = wide_fader();
    a.controller_heard(&[cc(7, 10), cc(39, 5)]);
    let mut t = Instant::now() + Duration::from_millis(50);
    for coarse in 11..20u8 {
        // A fine-only update, then a pair: coarse, fine, a moment later.
        a.controller_heard(&[msg_at(0xB0, 39, 120, t)]);
        t += Duration::from_millis(2);
        // The pair split between two frames: the coarse half waits for its fine half,
        // never driven with the lone fine half before it.
        let w = a.project().master.width;
        a.controller_heard(&[msg_at(0xB0, 7, coarse, t)]);
        assert_eq!(
            a.project().master.width,
            w,
            "{coarse}: the coarse half waits"
        );
        a.controller_heard(&[msg_at(0xB0, 39, 3, t + Duration::from_micros(300))]);
        assert!(
            (a.project().master.width - width_of(u16::from(coarse) << 7 | 3)).abs() < 1e-4,
            "{coarse}: {}",
            a.project().master.width
        );
        t += Duration::from_millis(20);
    }
}

/// #4, #56: a controller that does send its fine half first, again and again, is read as
/// such — its fine half waits for its coarse half, never driven with the old one.
#[test]
fn a_fine_first_controller_is_read_whole() {
    let mut a = wide_fader();
    a.controller_heard(&[cc(7, 1), cc(39, 1)]);
    let mut t = Instant::now() + Duration::from_millis(50);
    for coarse in 2..10u8 {
        a.controller_heard(&[
            msg_at(0xB0, 39, 64, t),
            msg_at(0xB0, 7, coarse, t + Duration::from_micros(300)),
        ]);
        t += Duration::from_millis(20);
    }
    // Known now: a fine half alone in one frame drives nothing until its coarse half.
    let w = a.project().master.width;
    a.controller_heard(&[msg_at(0xB0, 39, 9, t)]);
    assert_eq!(a.project().master.width, w, "the fine half waits");
    a.controller_heard(&[msg_at(0xB0, 7, 30, t + Duration::from_micros(400))]);
    assert!((a.project().master.width - width_of(30 << 7 | 9)).abs() < 1e-4);
}

// ---- feedback's echo -----------------------------------------------------------------------

/// #14, #31, #33, #55: an echo stamped a moment before the send was recorded (a loopback's
/// thread ran first) is still the echo.
#[test]
fn an_echo_stamped_before_its_send_is_an_echo() {
    let mut a = keys_app();
    let s = strip(&a);
    let mut m = Mapping::new(
        MappingId(1),
        source(Control::Cc(10)),
        MappingTarget::Param(AutoTarget::StripPan(s)),
        MapMode::Absolute,
    );
    m.takeover = Takeover::Jump;
    m.feedback = true;
    a.add_mapping(Scope::Project, m);
    a.controllers.feedback_log = Some(Vec::new());
    a.controller_input();
    let (_, bytes, sent_at) = *a.controllers.echoes.last().expect("sent");
    a.exec(Command::SetStripPan { id: s, pan: 0.9 });
    a.controller_heard(&[msg_at(
        bytes[0],
        bytes[1],
        bytes[2],
        sent_at - Duration::from_micros(500),
    )]);
    assert_eq!(
        a.project().strip(s).unwrap().pan,
        0.9,
        "the echo moved nothing"
    );
}

// ---- frozen --------------------------------------------------------------------------------

fn frozen_strip() -> (CantusApp, StripId, EffectId) {
    let mut p = cantus_ui::starter_project();
    let s = p.tracks[0].strip.unwrap();
    let delay = p.make_effect(EffectKind::Delay);
    let fx = delay.id;
    p.strip_mut(s).unwrap().effects.push(delay);
    let content = cantus_core::freeze::content_key(&p, cantus_core::freeze::FreezeTarget::Strip(s))
        .expect("a key");
    p.strip_mut(s).unwrap().frozen = Some(cantus_core::freeze::Frozen { content });
    (keys_app_with(p), s, fx)
}

/// #8, #29, #39, #47: a native target on a frozen sound reads *Frozen* in the pane.
#[test]
fn a_native_target_on_a_frozen_sound_is_frozen() {
    use cantus_ui::controllers::Idle;
    let (mut a, _s, fx) = frozen_strip();
    let key = EffectKind::Delay.params()[0].key;
    add(
        &mut a,
        Scope::Project,
        1,
        Control::Cc(10),
        effect(fx, key),
        MapMode::Absolute,
        Takeover::Jump,
    );
    let m = a.project().controllers.mappings[0].clone();
    assert_eq!(a.mapping_state(Scope::Project, &m), Some(Idle::Frozen));
}

/// #20, #28: a knob that only moves a frozen target says so again on a later turn.
#[test]
fn a_frozen_refusal_is_said_again_after_a_pause() {
    let (mut a, _s, fx) = frozen_strip();
    let key = EffectKind::Delay.params()[0].key;
    add(
        &mut a,
        Scope::Project,
        1,
        Control::Cc(10),
        effect(fx, key),
        MapMode::Absolute,
        Takeover::Jump,
    );
    let t0 = Instant::now();
    a.controller_heard(&[msg_at(0xB0, 10, 40, t0)]);
    assert!(a.status.error);
    a.set_status("something else".into());
    let later = t0 + Duration::from_secs(2);
    a.controller_heard(&[msg_at(0xB0, 10, 60, later)]);
    assert!(a.status.error, "said again: {}", a.status.text);
}

// ---- recording's bounds ---------------------------------------------------------------------

/// Bar `n` (counting from 0) of the starter's 4/4.
fn bar(n: i64) -> Ticks {
    Ticks::beats(4 * n)
}

fn pan_knob(p: Project) -> (CantusApp, StripId) {
    let mut a = keys_app_with(p);
    let s = strip(&a);
    add(
        &mut a,
        Scope::Project,
        1,
        Control::Cc(10),
        MappingTarget::Param(AutoTarget::StripPan(s)),
        MapMode::Absolute,
        Takeover::Jump,
    );
    (a, s)
}

fn pan_clips(a: &CantusApp, s: StripId) -> Vec<Clip> {
    a.project()
        .tracks
        .iter()
        .flat_map(|t| t.clips.iter())
        .filter(|c| {
            c.automation
                .as_ref()
                .is_some_and(|x| x.target == AutoTarget::StripPan(s))
        })
        .cloned()
        .collect()
}

/// #16: punched over bars 5–8 and stopped at bar 12, the clip ends at the punch-out.
#[test]
fn a_punched_pass_ends_its_clip_at_the_punch_out() {
    let mut p = cantus_ui::starter_project();
    p.recording.punch_enabled = true;
    p.recording.punch = Some(LoopRegion {
        start: bar(4),
        end: bar(8),
    });
    let (mut a, s) = pan_knob(p);
    a.engine.seek(bar(2));
    a.start_recording();
    a.engine.set_heard_for_tests(bar(5), true);
    a.controller_heard(&[cc(10, 100)]);
    a.engine.set_heard_for_tests(bar(6), true);
    a.controller_heard(&[cc(10, 20)]);
    assert!(
        a.latched(&AutoTarget::StripPan(s)),
        "latched inside the punch"
    );
    a.engine.set_heard_for_tests(bar(9), true);
    assert!(
        !a.latched(&AutoTarget::StripPan(s)),
        "the latch lets go past the punch-out"
    );
    a.engine.set_heard_for_tests(bar(11), false);
    a.stop_recording();
    let clips = pan_clips(&a, s);
    assert_eq!(clips.len(), 1);
    assert!(
        clips[0].start + clips[0].length <= bar(8),
        "ends at the punch-out: {:?} + {:?}",
        clips[0].start,
        clips[0].length
    );
}

/// #18: a false start — a knob turned in the count-in, stopped before the pass began —
/// writes nothing.
#[test]
fn a_false_start_writes_no_automation() {
    let mut p = cantus_ui::starter_project();
    p.recording.count_in_bars = 2;
    let (mut a, s) = pan_knob(p);
    a.engine.seek(bar(8));
    let steps = a.bus.history().len();
    a.start_recording();
    a.engine.set_heard_for_tests(bar(7), true);
    a.controller_heard(&[cc(10, 100)]);
    a.stop_recording();
    assert!(pan_clips(&a, s).is_empty());
    assert!(
        a.bus.history().len() <= steps + 1,
        "no automation step: {}",
        a.bus.history().len() - steps
    );
}

/// #19, #34, #58: a count-in inside the loop is not the pass: a move there lands at the
/// record start.
#[test]
fn a_count_in_inside_the_loop_lands_at_the_record_start() {
    let mut p = cantus_ui::starter_project();
    p.loop_region = Some(LoopRegion {
        start: bar(4),
        end: bar(8),
    });
    p.loop_enabled = true;
    p.recording.count_in_bars = 1;
    let (mut a, s) = pan_knob(p);
    a.engine.seek(bar(6));
    a.start_recording();
    let mid5 = bar(5) + Ticks::beats(2);
    a.engine.set_heard_for_tests(mid5, true);
    a.controller_heard(&[cc(10, 100)]);
    let points = a
        .modulators
        .rec
        .points
        .get(&AutoTarget::StripPan(s))
        .cloned()
        .unwrap();
    assert!(points.iter().all(|(t, _)| *t >= bar(6).0), "{points:?}");
}

/// #34: a real wrap — the playhead going back to the loop's start — puts a message heard
/// before it at the loop's end.
#[test]
fn a_message_before_a_real_wrap_lands_at_the_end() {
    let mut p = cantus_ui::starter_project();
    p.loop_region = Some(LoopRegion {
        start: bar(4),
        end: bar(8),
    });
    p.loop_enabled = true;
    let (mut a, s) = pan_knob(p);
    a.engine.seek(bar(4));
    a.start_recording();
    a.engine.set_heard_for_tests(bar(8) - Ticks(10), true);
    a.controller_heard(&[]);
    // Wrapped: the playhead is just past the loop's start; a message 200 ms old.
    a.engine.set_heard_for_tests(bar(4) + Ticks(20), true);
    a.controller_heard(&[msg_at(
        0xB0,
        10,
        90,
        Instant::now() - Duration::from_millis(200),
    )]);
    let points = a
        .modulators
        .rec
        .points
        .get(&AutoTarget::StripPan(s))
        .cloned()
        .unwrap();
    assert!(
        points.iter().all(|(t, _)| *t > bar(7).0),
        "at the loop's end: {points:?}"
    );
}

/// #34: the punch bounds: a move before the punch-in lands there, one after the punch-out
/// is not written.
#[test]
fn the_punch_bounds_hold() {
    let mut p = cantus_ui::starter_project();
    p.recording.punch_enabled = true;
    p.recording.punch = Some(LoopRegion {
        start: bar(4),
        end: bar(6),
    });
    let (mut a, s) = pan_knob(p);
    a.engine.seek(bar(2));
    a.start_recording();
    for (b, v) in [(3, 10u8), (5, 60), (7, 120)] {
        a.engine.set_heard_for_tests(bar(b), true);
        a.controller_heard(&[cc(10, v)]);
    }
    let points = a
        .modulators
        .rec
        .points
        .get(&AutoTarget::StripPan(s))
        .cloned()
        .unwrap();
    let ticks: Vec<i64> = points.iter().map(|(t, _)| *t).collect();
    assert_eq!(
        ticks,
        vec![bar(4).0, bar(5).0],
        "clamped to the punch-in, dropped after the punch-out"
    );
}

// ---- another project ------------------------------------------------------------------------

/// #25: a learn armed in one project lets go when another is opened: the first key played
/// there is no mapping.
#[test]
fn a_learn_lets_go_when_another_project_comes() {
    let mut a = keys_app();
    let s = strip(&a);
    a.learn_target(MappingTarget::Param(AutoTarget::StripGain(s)));
    a.replace_project(cantus_ui::starter_project());
    a.controller_heard(&[msg_at(0x90, 60, 100, Instant::now())]);
    assert!(a.controllers.learning.is_none());
    assert!(a.project().controllers.is_empty());
    assert!(!a.controllers.tap.filter().learning());
}

// ---- the global file ------------------------------------------------------------------------

/// #30, #40: a file that loses a mapping is kept beside once, however many starts read it;
/// one that cannot even be kept is never saved over.
#[test]
fn a_lossy_global_file_is_kept_once() {
    use cantus_ui::controllers::{global_path, load_global_report, GlobalLoad};
    let dir = tempfile::tempdir().unwrap();
    let path = global_path(dir.path());
    let good = serde_json::to_value(Mapping::new(
        MappingId(1),
        source(Control::Cc(1)),
        MappingTarget::Tempo,
        MapMode::Absolute,
    ))
    .unwrap();
    let text = serde_json::json!({
        "version": 1,
        "mappings": [good, {"id": 2, "target": {"FromTheFuture": 3}}],
    })
    .to_string();
    std::fs::write(&path, &text).unwrap();
    let first = load_global_report(&path).1;
    let second = load_global_report(&path).1;
    assert!(matches!(first, GlobalLoad::Kept(_)));
    assert_eq!(first, second, "the same file named");
    let bad = std::fs::read_dir(dir.path())
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_name().to_string_lossy().ends_with(".bad"))
        .count();
    assert_eq!(bad, 1, "kept once");
    // A path that is a folder: unreadable, and nothing can be kept.
    let folder = dir.path().join("as-folder");
    std::fs::create_dir_all(global_path(&folder)).unwrap();
    assert_eq!(
        load_global_report(&global_path(&folder)).1,
        GlobalLoad::Unsafe
    );
}

// ---- names ----------------------------------------------------------------------------------

/// #32, #44: a mapping learnt on another plug-in is named after it, and learning its
/// control on the plug-in there now says nothing is shared.
#[test]
fn a_swapped_out_plug_ins_mapping_keeps_its_name_and_shares_nothing() {
    use cantus_core::external::PluginFormat;
    let vital = ExternalPlugin::new(PluginFormat::Clap, "audio.vital", "Vital");
    let surge = ExternalPlugin::new(PluginFormat::Clap, "org.surge", "Surge");
    let mut p = cantus_ui::starter_project();
    let t = p.tracks[0].id;
    p.track_mut(t).unwrap().instrument = InstrumentSpec::External(vital.clone());
    let mut a = keys_app_with(p);
    let target = MappingTarget::Param(AutoTarget::PluginInstrument { track: t, id: 7 });
    let mut m = Mapping::new(
        MappingId(1),
        source(Control::Cc(71)),
        target.clone(),
        MapMode::Absolute,
    );
    m.plugin = target.plugin_of(a.project());
    a.add_mapping(Scope::Project, m.clone());
    a.exec(Command::SetInstrument {
        track: t,
        spec: Box::new(InstrumentSpec::External(surge)),
    });
    let name = a.mapping_name(&m);
    assert!(
        name.contains("audio.vital") || name.contains("Vital"),
        "{name}"
    );
    assert!(!name.contains("Surge"), "{name}");
    a.learn_target(target);
    a.controller_heard(&(0..8).map(|v| cc(71, 60 + v)).collect::<Vec<_>>());
    let also = cantus_ui::i18n::t("ctl.status.learnt_shared");
    let head = also.split('{').next().unwrap_or_default().to_string();
    assert!(
        head.is_empty() || !a.status.text.starts_with(&head) || !a.status.text.contains(','),
        "{}",
        a.status.text
    );
    let shared = a
        .project()
        .controllers
        .sharing_in(
            a.project().controllers.mappings.last().unwrap(),
            a.project(),
        )
        .count();
    assert_eq!(shared, 0);
}

/// #50: the Spanish pill keeps the locale's word, *plug-in*.
#[test]
fn the_spanish_pill_says_plug_in() {
    use cantus_ui::i18n::{t_in, Locale};
    let es = t_in(Locale::Es, "ctl.view.other_plugin");
    assert!(es.contains("plug-in"), "{es}");
}

/// #49: an action this build does not know is no transport word, and its row is missing.
#[test]
fn an_unknown_action_is_missing() {
    use cantus_ui::controllers::Idle;
    let a = keys_app();
    let m = Mapping::new(
        MappingId(9),
        source(Control::Cc(50)),
        MappingTarget::Action("open-mixer-snapshot".into()),
        MapMode::Trigger,
    );
    let name = a.mapping_target_name(&m.target);
    assert!(
        !name.starts_with(&cantus_ui::i18n::t("ctl.target.action")),
        "{name}"
    );
    assert_eq!(a.mapping_state(Scope::Global, &m), Some(Idle::Missing));
}

// ---- ports and NRPN -------------------------------------------------------------------------

/// #45: a mapping on a port that is not open reads *Not connected*; a Linux port's name
/// shows as the device's.
#[test]
fn a_port_not_open_is_not_connected() {
    use cantus_ui::controllers::Idle;
    let mut a = keys_app();
    a.midi_port_names = vec!["nanoKONTROL2:nanoKONTROL2 _ CTRL 28:0".into()];
    let mut m = Mapping::new(
        MappingId(1),
        MidiSource {
            port: PortRef {
                name: "Keys".into(),
                ordinal: 0,
            },
            channel: Some(0),
            control: Control::Cc(1),
        },
        MappingTarget::Tempo,
        MapMode::Absolute,
    );
    assert_eq!(a.mapping_state(Scope::Global, &m), Some(Idle::NotConnected));
    m.source.port = PortRef::of(&["nanoKONTROL2:nanoKONTROL2 _ CTRL 24:0".to_string()], 0);
    assert_eq!(a.mapping_state(Scope::Global, &m), None, "the same device");
    assert_eq!(
        cantus_ui::controllers::port_text(&m.source.port),
        "nanoKONTROL2 _ CTRL"
    );
}

/// #6: an NRPN knob is not learnt as a CC 99 mapping: the learn says so and lets go.
#[test]
fn an_nrpn_knob_is_not_learnt() {
    let mut a = keys_app();
    let s = strip(&a);
    a.learn_target(MappingTarget::Param(AutoTarget::StripPan(s)));
    a.controller_heard(&[cc(99, 1), cc(98, 2), cc(6, 64), cc(38, 0)]);
    assert!(a.controllers.learning.is_none());
    assert!(a.project().controllers.is_empty());
    assert!(a.status.error);
}

// ---- the selected strip, the mixer's switches, action lights -------------------------------

/// #7: "the selected strip" with the master selected moves the master.
#[test]
fn the_selected_strip_can_be_the_master() {
    let mut a = keys_app();
    a.controllers.global.mappings.push({
        let mut m = Mapping::new(
            MappingId(1),
            source(Control::Cc(7)),
            MappingTarget::SelectedStrip(DeskParam::Volume),
            MapMode::Absolute,
        );
        m.takeover = Takeover::Jump;
        m
    });
    a.select_master();
    a.controller_heard(&[cc(7, 0)]);
    assert!(
        a.project().master.gain_db < -60.0,
        "{}",
        a.project().master.gain_db
    );
}

/// #46: the strip's effects switch and record arm are mapped from its menu's rows and move
/// what the bulb and the dot move.
#[test]
fn a_strips_effects_switch_and_record_arm_are_learnable() {
    let mut a = keys_app();
    let s = strip(&a);
    let t = a.project().tracks[0].id;
    a.exec(Command::SetTrackInput {
        id: t,
        input: Some(cantus_core::audio::TrackInput {
            first: 0,
            stereo: false,
            monitor: Default::default(),
            trim_db: 0.0,
        }),
    });
    add(
        &mut a,
        Scope::Project,
        1,
        Control::Note(40),
        MappingTarget::StripFx(s),
        MapMode::Toggle,
        Takeover::Jump,
    );
    add(
        &mut a,
        Scope::Project,
        2,
        Control::Note(41),
        MappingTarget::StripArm(s),
        MapMode::Toggle,
        Takeover::Jump,
    );
    let fx = a.project().strip(s).unwrap().fx_enabled;
    a.controller_heard(&[msg_at(0x90, 40, 100, Instant::now())]);
    assert_ne!(a.project().strip(s).unwrap().fx_enabled, fx);
    a.controller_heard(&[msg_at(0x90, 41, 100, Instant::now())]);
    assert!(a.project().track(t).unwrap().armed);
    // The strip's menu offers both rows (the arm with a track recording into it); the
    // master's the effects switch only.
    use cantus_ui::controllers::strip_learn_target;
    assert_eq!(strip_learn_target(s, 5), Some(MappingTarget::StripFx(s)));
    assert_eq!(strip_learn_target(s, 6), Some(MappingTarget::StripArm(s)));
    assert_eq!(
        strip_learn_target(StripId::MASTER, 5),
        Some(MappingTarget::StripFx(StripId::MASTER))
    );
    assert_eq!(strip_learn_target(StripId::MASTER, 6), None);
    cantus_ui::views::mixer::open_menu(&mut a, Some(s), egui::pos2(200.0, 200.0));
    let text = paint(&ctx(), egui::vec2(1400.0, 800.0), 3, |ui| {
        cantus_ui::views::mixer::show(&mut a, ui)
    });
    for key in ["ctl.menu.fx", "ctl.menu.arm"] {
        let row = cantus_ui::i18n::t(key);
        assert!(text.contains(&row), "{row:?} in the strip's menu: {text}");
    }
}

/// #48: the learn mode and the View menu's areas light a button.
#[test]
fn the_learn_mode_and_the_areas_light_a_button() {
    use cantus_ui::keymap::Action;
    let mut a = keys_app();
    assert_eq!(a.action_state(Action::MidiLearnMode.id()), Some(0.0));
    a.toggle_learn_mode();
    assert_eq!(a.action_state(Action::MidiLearnMode.id()), Some(1.0));
    for action in [
        Action::ViewMixer,
        Action::ToggleBrowser,
        Action::ToggleInspector,
    ] {
        let before = a.action_state(action.id()).expect("a state");
        a.run_action(action);
        assert_ne!(a.action_state(action.id()), Some(before), "{action:?}");
    }
}

// ---- an insert swapped ----------------------------------------------------------------------

/// #54: swapping an insert keeps its bypass and mix controllers on the new one.
#[test]
fn swapping_an_insert_keeps_its_controllers() {
    let mut a = keys_app();
    let s = strip(&a);
    let fx = compressor(&mut a);
    add(
        &mut a,
        Scope::Project,
        1,
        Control::Cc(21),
        MappingTarget::EffectBypass(fx),
        MapMode::Toggle,
        Takeover::Jump,
    );
    add(
        &mut a,
        Scope::Project,
        2,
        Control::Cc(22),
        MappingTarget::EffectMix(fx),
        MapMode::Absolute,
        Takeover::Jump,
    );
    assert!(a.replace_effect_with(
        s,
        fx,
        &cantus_ui::picker::PickChoice::Effect(EffectKind::Delay)
    ));
    let new = a.project().strip(s).unwrap().effects.last().unwrap().id;
    let targets: Vec<MappingTarget> = a
        .project()
        .controllers
        .mappings
        .iter()
        .map(|m| m.target.clone())
        .collect();
    assert!(
        targets.contains(&MappingTarget::EffectBypass(new)),
        "{targets:?}"
    );
    assert!(
        targets.contains(&MappingTarget::EffectMix(new)),
        "{targets:?}"
    );
    a.undo();
    assert!(a
        .project()
        .controllers
        .mappings
        .iter()
        .any(|m| m.target == MappingTarget::EffectBypass(fx)));
}

// ---- learning everywhere, by clicking -------------------------------------------------------

/// Paint `show` in learn mode, then click every learn spot it showed, one at a time: what
/// each click armed (review 3, #36: a control drawn without its learn is missing here).
fn armed_by_clicks(
    a: &mut CantusApp,
    size: egui::Vec2,
    show: &mut dyn FnMut(&mut CantusApp, &mut egui::Ui),
) -> Vec<MappingTarget> {
    use cantus_ui::kit::automate::probe;
    let c = ctx();
    if !a.controllers.learn_mode {
        a.toggle_learn_mode();
    }
    let mut frame = 0u64;
    let mut run = |a: &mut CantusApp, events: Vec<egui::Event>| {
        frame += 1;
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, size)),
            time: Some(frame as f64 * 0.05),
            events,
            ..Default::default()
        };
        let mut out = c.run_ui(input, |ui| show(a, ui));
        out.textures_delta.clear();
    };
    probe::start();
    for _ in 0..3 {
        run(a, Vec::new());
    }
    let spots = probe::take_spots();
    let mut armed = Vec::new();
    for r in spots {
        let at = r.center();
        a.cancel_learning();
        run(a, vec![egui::Event::PointerMoved(at)]);
        for pressed in [true, false] {
            run(
                a,
                vec![egui::Event::PointerButton {
                    pos: at,
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: Default::default(),
                }],
            );
        }
        if let Some(l) = a.controllers.learning.as_ref() {
            armed.push(l.target.clone());
        }
        a.cancel_learning();
    }
    armed
}

/// #36: every channel rack row's pan, volume and lamp arms its own target when clicked in
/// the learn mode.
#[test]
fn clicking_the_racks_controls_arms_them() {
    let mut a = keys_app();
    let t0 = a.project().tracks[0].id;
    a.selected_track = Some(t0);
    a.selected_clip = a.project().tracks[0].clips.first().map(|c| c.id);
    let armed = armed_by_clicks(&mut a, egui::vec2(1400.0, 600.0), &mut |a, ui| {
        cantus_ui::views::channel_rack::show(a, ui)
    });
    for t in a.project().tracks.iter().map(|t| t.id) {
        for want in [
            MappingTarget::Param(AutoTarget::TrackPan(t)),
            MappingTarget::Param(AutoTarget::TrackGain(t)),
            MappingTarget::TrackMute(t),
        ] {
            assert!(armed.contains(&want), "{want:?}: {armed:?}");
        }
    }
}

/// #36, #46: the mixer: every strip's fader, pan, width, mute, solo and effects switch, a
/// bus strip's, and the master's volume, pan, width and effects switch.
#[test]
fn clicking_the_mixers_controls_arms_them() {
    let mut a = keys_app();
    a.add_strip();
    let armed = armed_by_clicks(&mut a, egui::vec2(1600.0, 700.0), &mut |a, ui| {
        cantus_ui::views::mixer::show(a, ui)
    });
    let mut strips: Vec<StripId> = a.project().tracks.iter().filter_map(|t| t.strip).collect();
    strips.push(a.project().strips.last().unwrap().id);
    for s in strips {
        for want in [
            MappingTarget::Param(AutoTarget::StripGain(s)),
            MappingTarget::Param(AutoTarget::StripPan(s)),
            MappingTarget::Param(AutoTarget::StripWidth(s)),
            MappingTarget::StripMute(s),
            MappingTarget::StripSolo(s),
            MappingTarget::StripFx(s),
        ] {
            assert!(armed.contains(&want), "{want:?}: {armed:?}");
        }
    }
    for want in [
        MappingTarget::Param(AutoTarget::MasterGain),
        MappingTarget::Param(AutoTarget::MasterPan),
        MappingTarget::Param(AutoTarget::MasterWidth),
        MappingTarget::StripFx(StripId::MASTER),
    ] {
        assert!(armed.contains(&want), "{want:?}: {armed:?}");
    }
}

/// The test CLAP plug-in, built from source where the host's tests build it.
fn test_plugin() -> std::path::PathBuf {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..");
    let target = root.join("target").join("test-plugins");
    let status =
        std::process::Command::new(std::env::var("CARGO").unwrap_or_else(|_| "cargo".into()))
            .args(["build", "-p", "cantus-test-clap", "--target-dir"])
            .arg(&target)
            .current_dir(&root)
            .status()
            .expect("cargo");
    assert!(status.success());
    target.join("debug").join(if cfg!(windows) {
        "cantus_test_clap.dll"
    } else if cfg!(target_os = "macos") {
        "libcantus_test_clap.dylib"
    } else {
        "libcantus_test_clap.so"
    })
}

/// #36: a hosted plug-in's editor: its parameters' cells arm them when clicked.
#[test]
fn clicking_a_hosted_editors_cells_arms_them() {
    let mut a = keys_app();
    cantus_ui::views::plugins::demo_setup(&mut a, &test_plugin(), true);
    let effect = a.fx.open.map(|(_, e, _)| e).expect("the editor");
    let armed = armed_by_clicks(&mut a, egui::vec2(1600.0, 1000.0), &mut |a, ui| {
        let ctx = ui.ctx().clone();
        cantus_ui::views::fx::window(a, &ctx);
    });
    assert!(
        armed.contains(&MappingTarget::Param(AutoTarget::Plugin { effect, id: 0 })),
        "{armed:?}"
    );
}

/// #59: a hosted editor's words for a value are asked again while the plug-in has not
/// answered (its number shows meanwhile, never kept as its words), kept once answered, and
/// another effect's editor never shows the first one's words.
#[test]
fn a_hosted_values_words_are_asked_until_answered() {
    use cantus_ui::views::plugins::ValueWords;
    let mut w = ValueWords::default();
    let (a, b) = (EffectId(1), EffectId(2));
    let bits = 0.42f64.to_bits();
    assert_eq!(w.get(a, 0, bits, true, || None), None, "not answered yet");
    assert_eq!(
        w.get(a, 0, bits, true, || Some("1.2 kHz".into())),
        Some("1.2 kHz"),
        "asked again"
    );
    assert_eq!(
        w.get(a, 0, bits, true, || panic!("asked again once answered")),
        Some("1.2 kHz")
    );
    assert_eq!(
        w.get(b, 0, bits, true, || Some("3 dB".into())),
        Some("3 dB"),
        "another effect's own words"
    );
}

/// #53: a hosted plug-in's stepped parameter reads the plug-in's own words for its steps,
/// never a percentage: its two-state key switch *Off* and *On*, its fault selector the
/// plug-in's number for each step.
#[test]
fn a_hosted_stepped_parameter_reads_the_plug_ins_words() {
    use cantus_ui::views::controllers::value_words;
    let mut a = keys_app();
    cantus_ui::views::plugins::demo_setup(&mut a, &test_plugin(), true);
    let effect = a.fx.open.map(|(_, e, _)| e).expect("the editor");
    let key = MappingTarget::Param(AutoTarget::Plugin { effect, id: 2 });
    assert_eq!(value_words(&a, &key, 0.0), "Off");
    assert_eq!(value_words(&a, &key, 1.0), "On");
    let fault = MappingTarget::Param(AutoTarget::Plugin { effect, id: 4 });
    assert_eq!(value_words(&a, &fault, 3.0 / 17.0), "3");
}

/// #57: a loaded plug-in that crashed: its mappings read *Not loaded* (they stay the map's).
#[test]
fn a_crashed_plug_ins_mapping_is_not_loaded() {
    use cantus_ui::controllers::Idle;
    let mut a = keys_app();
    cantus_ui::views::plugins::demo_setup(&mut a, &test_plugin(), true);
    let effect = a.fx.open.map(|(_, e, _)| e).expect("the editor");
    let target = MappingTarget::Param(AutoTarget::Plugin { effect, id: 0 });
    let mut m = Mapping::new(
        MappingId(1),
        source(Control::Cc(20)),
        target.clone(),
        MapMode::Absolute,
    );
    m.plugin = target.plugin_of(a.project());
    a.add_mapping(Scope::Project, m.clone());
    assert_eq!(a.mapping_state(Scope::Project, &m), None);
    a.plugins.mark_loaded_crashed_for_tests(effect, "test");
    assert_eq!(a.mapping_state(Scope::Project, &m), Some(Idle::NotLoaded));
    assert!(a.mapping_alive(Scope::Project, &m), "still the map's");
}

// ---- the tempo gesture ----------------------------------------------------------------------

/// #17: a gesture holding the tempo and a volume, the volume ridden alone for a long while,
/// keeps a step of bounded size, and undo restores both.
#[test]
fn a_tempo_gesture_stays_small() {
    let mut a = keys_app();
    let s = strip(&a);
    add(
        &mut a,
        Scope::Global,
        1,
        Control::Cc(16),
        MappingTarget::Tempo,
        MapMode::Relative(Encoding::TwosComplement),
        Takeover::Jump,
    );
    add(
        &mut a,
        Scope::Project,
        2,
        Control::Cc(10),
        MappingTarget::Param(AutoTarget::StripGain(s)),
        MapMode::Absolute,
        Takeover::Jump,
    );
    let before = a.project().clone();
    a.controller_heard(&[cc(16, 1), cc(10, 50)]);
    let size = |a: &CantusApp| {
        let e = a.bus.history().last().unwrap();
        format!("{:?}{:?}", e.command, e.inverse).len()
    };
    let first = size(&a);
    for v in 0..200u32 {
        a.controller_heard(&[cc(10, (v % 120) as u8)]);
    }
    assert!(size(&a) < first * 3, "{} against {first}", size(&a));
    a.undo();
    assert_eq!(a.project().tempo_bpm, before.tempo_bpm);
    assert_eq!(
        a.project().strip(s).unwrap().gain_db,
        before.strip(s).unwrap().gain_db
    );
}

// ---- typing into the pane, clicking the Tonus keyboard ---------------------------------------

/// Frames of `show` on one context, the clock moving as a hand's would.
struct Frames {
    ctx: egui::Context,
    t: f64,
    size: egui::Vec2,
}

impl Frames {
    fn new(size: egui::Vec2) -> Frames {
        Frames {
            ctx: ctx(),
            t: 0.0,
            size,
        }
    }

    fn run(
        &mut self,
        a: &mut CantusApp,
        events: Vec<egui::Event>,
        show: &mut dyn FnMut(&mut CantusApp, &mut egui::Ui),
    ) {
        self.t += 0.05;
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, self.size)),
            time: Some(self.t),
            events,
            ..Default::default()
        };
        let mut out = self.ctx.run_ui(input, |ui| show(a, ui));
        out.textures_delta.clear();
    }

    fn click(
        &mut self,
        a: &mut CantusApp,
        at: egui::Pos2,
        show: &mut dyn FnMut(&mut CantusApp, &mut egui::Ui),
    ) {
        self.run(a, vec![egui::Event::PointerMoved(at)], show);
        for pressed in [true, false] {
            self.run(
                a,
                vec![egui::Event::PointerButton {
                    pos: at,
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: Default::default(),
                }],
                show,
            );
        }
    }
}

/// #41: the curve is typed into its cell in the pane, read in percent, and kept as typed —
/// a bend under 2 % is not snapped straight the way a drag near the middle is.
#[test]
fn a_curve_is_typed_into_its_cell_as_typed() {
    let mut a = keys_app();
    let s = strip(&a);
    let id = MappingId(1);
    add(
        &mut a,
        Scope::Project,
        1,
        Control::Cc(20),
        MappingTarget::Param(AutoTarget::StripGain(s)),
        MapMode::Absolute,
        Takeover::Jump,
    );
    let mut show =
        |a: &mut CantusApp, ui: &mut egui::Ui| cantus_ui::views::controllers::show(a, ui);
    let mut f = Frames::new(egui::vec2(1400.0, 600.0));
    let curve = |a: &CantusApp| a.project().controllers.get(id).unwrap().curve;
    for (typed, want) in [("1", 0.01f32), ("-35 %", -0.35), ("+50", 0.5)] {
        for _ in 0..3 {
            f.run(&mut a, Vec::new(), &mut show);
        }
        let name = format!("1#{}#curve", id.0);
        let cell = a
            .controllers
            .view
            .geo
            .iter()
            .find(|(n, _)| *n == name)
            .map(|(_, r)| *r)
            .expect("the curve cell");
        // A double-click opens the field (well after any earlier click).
        for _ in 0..10 {
            f.run(&mut a, Vec::new(), &mut show);
        }
        f.click(&mut a, cell.center(), &mut show);
        f.click(&mut a, cell.center(), &mut show);
        f.run(&mut a, Vec::new(), &mut show);
        // Select what is there, type over it, Enter.
        f.run(
            &mut a,
            vec![egui::Event::Key {
                key: egui::Key::A,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: egui::Modifiers::COMMAND,
            }],
            &mut show,
        );
        f.run(&mut a, vec![egui::Event::Text(typed.into())], &mut show);
        f.run(
            &mut a,
            vec![egui::Event::Key {
                key: egui::Key::Enter,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: egui::Modifiers::NONE,
            }],
            &mut show,
        );
        f.run(&mut a, Vec::new(), &mut show);
        assert!(
            (curve(&a) - want).abs() < 1e-4,
            "{typed:?} typed: the curve is {}, not {want}",
            curve(&a)
        );
    }
}

/// #43: in the learn mode a click at three quarters of a black key's width learns that
/// black key — never the white key beside it — and a white key is learnt from its part
/// between the black keys too.
#[test]
fn a_black_keys_right_half_learns_the_black_key() {
    use cantus_core::fx::tonus::NOTE_KEYS;
    let mut a = keys_app();
    let s = strip(&a);
    a.add_effect(s, EffectKind::AutoTune);
    let fx = a
        .project()
        .strip(s)
        .unwrap()
        .effects
        .iter()
        .find(|e| e.spec.kind == EffectKind::AutoTune)
        .unwrap()
        .id;
    a.open_effect_editor(s, fx);
    a.toggle_learn_mode();
    let mut show = |a: &mut CantusApp, ui: &mut egui::Ui| {
        let ctx = ui.ctx().clone();
        cantus_ui::views::fx::window(a, &ctx);
    };
    let mut f = Frames::new(egui::vec2(1400.0, 900.0));
    for _ in 0..3 {
        f.run(&mut a, Vec::new(), &mut show);
    }
    let keys = a.fx.tonus.geometry.expect("the keyboard drawn").keys;
    let mut armed_at = |a: &mut CantusApp, at: egui::Pos2| {
        a.cancel_learning();
        for _ in 0..10 {
            f.run(a, Vec::new(), &mut show);
        }
        f.click(a, at, &mut show);
        let got = a.controllers.learning.as_ref().map(|l| l.target.clone());
        a.cancel_learning();
        got
    };
    for pc in [1usize, 3, 6, 8, 10] {
        let r = keys[pc];
        let at = egui::pos2(r.left() + r.width() * 0.75, r.center().y);
        assert_eq!(
            armed_at(&mut a, at),
            Some(effect(fx, NOTE_KEYS[pc])),
            "the black key {pc} clicked right of its middle"
        );
    }
    // D between C# and D#, above the black keys' bottom.
    let at = egui::pos2(keys[2].center().x, keys[1].center().y);
    assert_eq!(armed_at(&mut a, at), Some(effect(fx, NOTE_KEYS[2])));
    // E below the black keys.
    let at = egui::pos2(keys[4].center().x, keys[4].bottom() - 4.0);
    assert_eq!(armed_at(&mut a, at), Some(effect(fx, NOTE_KEYS[4])));
}

/// The coordinator's note on review 4 of the tempo: while a take records the tempo stays,
/// a knob's included. A gesture that turned the tempo just before the take began and goes
/// on with a pan knob during it sends no tempo command in the take — the pan moves alone.
/// (Tempo review 5: the hold is an *audio* take's — notes are recorded in ticks — so the
/// take here records audio: a hand-driven engine with one input, an armed audio track.)
#[test]
fn a_gesture_holding_the_tempo_sends_none_while_a_take_records() {
    let dir = tempfile::tempdir().unwrap();
    let (h, mut proc) = cantus_engine::EngineHandle::manual(48_000.0);
    let (_ring, consumer) = rtrb::RingBuffer::<f32>::new(96_000);
    proc.attach_input(consumer, 1);
    let mut a = keys_app();
    a.engine = h;
    a.rec.demo_inputs = Some(vec![0.0]);
    a.path = Some(dir.path().join("song.cantus"));
    a.add_audio_track();
    a.sync_engine();
    let s = strip(&a);
    add(
        &mut a,
        Scope::Project,
        1,
        Control::Cc(12),
        MappingTarget::Tempo,
        MapMode::Absolute,
        Takeover::Jump,
    );
    add(
        &mut a,
        Scope::Project,
        2,
        Control::Cc(10),
        MappingTarget::Param(AutoTarget::StripPan(s)),
        MapMode::Absolute,
        Takeover::Jump,
    );
    a.controller_heard(&[cc(12, 90)]);
    let bpm = a.project().tempo_bpm;
    a.start_recording();
    assert!(a.audio_take_running(), "an audio take");
    let pan = a.project().strip(s).unwrap().pan;
    a.controller_heard(&[cc(10, 20)]);
    assert_ne!(a.project().strip(s).unwrap().pan, pan, "the pan moves");
    assert_eq!(a.project().tempo_bpm, bpm);
    let last = a.bus.history().last().unwrap();
    assert!(
        !cantus_core::tempo::keep::changes_tempo(&last.command),
        "a tempo command sent during the take: {:?}",
        last.command
    );
    a.stop_recording();
}
