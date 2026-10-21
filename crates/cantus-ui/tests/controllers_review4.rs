//! Review 4 of ADR 0062 (round 29): each confirmed finding about the interface's
//! controllers after the third fix round, held by a test — names before Off/On, a hosted
//! plug-in's own steps, the instruments' choices without an editor, 14-bit pairs at speed,
//! recording's passes and latch, learning from one value of a choice, the inspector's
//! controls, names of swapped-out plug-ins, the pane's rows.

mod support;
use cantus_core::controller::*;
use cantus_core::*;
use cantus_engine::midi::Heard;
use cantus_engine::MidiEvent;
use cantus_ui::controllers::Scope;
use cantus_ui::views::controllers::{parse_value, value_words};
use cantus_ui::CantusApp;
use std::time::{Duration, Instant};
use support::*;

fn msg_on(port: u8, status: u8, data1: u8, data2: u8, at: Instant) -> Heard {
    Heard {
        port,
        event: MidiEvent {
            status,
            data1,
            data2,
        },
        at,
    }
}

fn msg_at(status: u8, data1: u8, data2: u8, at: Instant) -> Heard {
    msg_on(0, status, data1, data2, at)
}

fn cc(n: u8, v: u8) -> Heard {
    msg_at(0xB0, n, v, Instant::now())
}

fn keys_app() -> CantusApp {
    keys_app_with(cantus_ui::starter_project())
}

fn keys_app_with(p: Project) -> CantusApp {
    let mut a = app(p);
    a.midi_port_names = vec!["Keys".into(), "Pads".into()];
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

fn add(a: &mut CantusApp, id: u64, control: Control, target: MappingTarget, mode: MapMode) {
    let mut m = Mapping::new(MappingId(id), source(control), target, mode);
    m.takeover = Takeover::Jump;
    assert!(a.add_mapping(Scope::Project, m));
}

/// An effect of `kind` on the first strip: its id.
fn insert(a: &mut CantusApp, kind: EffectKind) -> EffectId {
    let s = strip(a);
    a.add_effect(s, kind);
    a.project().strip(s).unwrap().effects.last().unwrap().id
}

fn effect(fx: EffectId, key: &str) -> MappingTarget {
    MappingTarget::Param(AutoTarget::Effect {
        effect: fx,
        key: key.into(),
    })
}

fn instrument(track: TrackId, key: &str) -> MappingTarget {
    MappingTarget::Param(AutoTarget::Instrument {
        track,
        key: key.into(),
    })
}

fn in_both<T>(f: impl Fn() -> T) -> (T, T) {
    use cantus_ui::i18n::{set_locale, Locale};
    set_locale(Locale::Es);
    let es = f();
    set_locale(Locale::En);
    (f(), es)
}

// ---- names before Off and On --------------------------------------------------------------

/// #0, #44, #59: two named choices read their names, in either language, and are typed by
/// them; a wordless 0…1 reads Off and On; a two-step number (1…2 oct) keeps its number and
/// unit, and a typed 2 is its top.
#[test]
fn two_named_choices_read_their_names() {
    use cantus_ui::i18n::t_in;
    use cantus_ui::i18n::Locale::{En, Es};
    let mut a = keys_app();
    let ds = insert(&mut a, EffectKind::DeEsser);
    let sat = insert(&mut a, EffectKind::Saturator);
    let comp = insert(&mut a, EffectKind::Compressor);
    for (target, k0, k1) in [
        (effect(ds, "mode"), "ds.mode.0", "ds.mode.1"),
        (effect(sat, "phase"), "sat.phase.0", "sat.phase.1"),
    ] {
        let (en, es) = in_both(|| (value_words(&a, &target, 0.0), value_words(&a, &target, 1.0)));
        assert_eq!(en, (t_in(En, k0), t_in(En, k1)), "{target:?}");
        assert_eq!(es, (t_in(Es, k0), t_in(Es, k1)), "{target:?}");
        assert_eq!(parse_value(&a, &target, &t_in(En, k1)), Some(1.0));
        assert_eq!(parse_value(&a, &target, &t_in(Es, k0)), Some(0.0));
    }
    // A wordless switch: Off and On.
    let key_eq = effect(comp, "key_eq");
    assert_eq!(value_words(&a, &key_eq, 1.0), cantus_ui::i18n::t("ctl.value.on"));
    assert_eq!(parse_value(&a, &key_eq, "off"), Some(0.0));
    // A two-step number: its number and unit.
    let track = a.project().tracks[0].id;
    a.set_instrument(
        track,
        InstrumentSpec::Native(NativeSpec::new(InstrumentKind::Bass)),
    );
    let sub = instrument(track, "sub.octave");
    assert_eq!(value_words(&a, &sub, 0.0), "1 oct");
    assert_eq!(value_words(&a, &sub, 1.0), "2 oct");
    assert_eq!(parse_value(&a, &sub, "2"), Some(1.0));
    assert_eq!(parse_value(&a, &sub, "1"), Some(0.0));
}

// ---- a hosted plug-in's own steps ----------------------------------------------------------

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

/// An app with the test plug-in inserted and its editor open: the insert's id.
fn hosted_app() -> (CantusApp, EffectId) {
    let mut a = keys_app();
    cantus_ui::views::plugins::demo_setup(&mut a, &test_plugin(), true);
    let effect = a.fx.open.map(|(_, e, _)| e).expect("the editor");
    (a, effect)
}

/// #1, #58: a hosted parameter whose plain values are 0…1 with three steps (a JUCE VST3's
/// four-way choice) is no switch: an encoder moves one step a detent, a knob reaches the
/// middle choices (on their own steps, not 0 or 1 only), and its words are asked at its
/// steps.
#[test]
fn a_hosted_choice_moves_on_its_own_steps() {
    let (mut a, effect) = hosted_app();
    let id = 2; // the test plug-in's key switch, made a four-way choice of 0…1
    let mut info = a
        .hosted_param_info(&AutoTarget::Plugin { effect, id })
        .cloned()
        .expect("the parameter");
    (info.min, info.max, info.stepped, info.steps) = (0.0, 1.0, true, 3);
    a.plugins.set_param_info_for_tests(effect, info.clone());
    let target = MappingTarget::Param(AutoTarget::Plugin { effect, id });
    let shape = a.target_shape(&target);
    assert!(!shape.switch && shape.steps == Some(3), "{shape:?}");
    let Some(Command::SetPluginParam {
        value: Some(value), ..
    }) = a.target_command(&target, 0.4)
    else {
        panic!("a command");
    };
    assert!((value - 1.0 / 3.0).abs() < 1e-9, "the second of four: {value}");
    // An encoder: one choice a detent, down from where the demo left it (its top).
    add(
        &mut a,
        1,
        Control::Cc(20),
        target.clone(),
        MapMode::Relative(Encoding::TwosComplement),
    );
    let held = |a: &CantusApp| {
        a.project()
            .effect(strip(a), effect)
            .and_then(|e| e.spec.external.as_ref().and_then(|x| x.params.get(&id).copied()))
    };
    let before = held(&a).unwrap_or(0.0);
    // (The session reads the plug-in's own description again as the project changes.)
    a.plugins.set_param_info_for_tests(effect, info);
    let down = if before > 0.5 { 127 } else { 1 };
    a.controller_heard(&[cc(20, down)]);
    let after = held(&a).expect("set");
    assert!(
        ((after - before).abs() - 1.0 / 3.0).abs() < 1e-9,
        "one step: {before} to {after}"
    );
}

// ---- the instruments' choices, whatever was opened -----------------------------------------

/// #7, #22, #33, #45: an instrument's choices read their names in a fresh session with no
/// editor ever drawn — Calor's filter model, Gravis's play mode at its third value
/// (Strings, which its editor draws with one engine only), Modulus's operator wave, a
/// modulation slot's curve, Firmus's wave — and a name typed is read back.
#[test]
fn instrument_choices_read_their_names_with_no_editor_drawn() {
    use cantus_ui::i18n::t;
    let mut a = keys_app();
    let track = a.project().tracks[0].id;
    let cases = [
        (InstrumentKind::Analog, "vcf.model", 3.0 / 4.0, "calor.model.svf"),
        (InstrumentKind::Bass, "mode", 1.0, "bass.mode.strings"),
        (InstrumentKind::Fm, "op.c.wave", 2.0 / 3.0, "fm.wave.saw"),
        (InstrumentKind::Wavetable, "flt2.type", 0.0, "unda.flt.svf_lp12"),
        (InstrumentKind::Organ, "mod3.curve", 1.0, "mod.curve.steps"),
    ];
    for (kind, key, pos, word) in cases {
        a.set_instrument(track, InstrumentSpec::Native(NativeSpec::new(kind)));
        let target = instrument(track, key);
        assert_eq!(value_words(&a, &target, pos), t(word), "{kind:?} {key}");
        let back = parse_value(&a, &target, &t(word)).expect("typed back");
        assert!((back - pos).abs() < 1e-4, "{kind:?} {key}: {back}");
    }
    // Firmus's waves.
    a.set_instrument(track, InstrumentSpec::Firmus(FirmusParams::default()));
    let wave = MappingTarget::Param(AutoTarget::Firmus {
        track,
        param: FirmusParam::Osc1Wave,
    });
    assert_eq!(value_words(&a, &wave, 1.0), t("wave.square"));
}

/// #33: every instrument's every stepped choice (an automatable stepped parameter of 64
/// steps or fewer that the table names) reads a name at each step and reads it back —
/// nothing painted first.
#[test]
fn every_named_instrument_choice_reads_and_types_its_names() {
    let mut a = keys_app();
    let track = a.project().tracks[0].id;
    let mut checked = 0;
    for kind in InstrumentKind::ALL {
        a.set_instrument(track, InstrumentSpec::Native(NativeSpec::new(kind)));
        let mut keys: Vec<String> = kind
            .params()
            .iter()
            .filter(|i| i.stepped && kind.automatable(i.key))
            .map(|i| i.key.to_string())
            .collect();
        keys.extend(["mod1.curve".to_string()]);
        for key in keys {
            let Some(words) = cantus_ui::inst_choices::words(kind, &key) else {
                continue;
            };
            let target = instrument(track, &key);
            let r = cantus_core::automation::range::range_of(a.project(), match &target {
                MappingTarget::Param(p) => p,
                _ => unreachable!(),
            })
            .expect("a range");
            assert_eq!(
                words.len() as f32,
                r.max - r.min + 1.0,
                "{kind:?} {key}: a word for each value"
            );
            for i in 0..words.len() {
                let pos = r.pos(r.min + i as f32);
                let shown = value_words(&a, &target, pos);
                assert_eq!(shown, words[i].show(), "{kind:?} {key}: its name");
                let back = parse_value(&a, &target, &shown).expect("typed back");
                assert!((back - pos).abs() < 1e-4, "{kind:?} {key} {shown}: {back}");
            }
            checked += 1;
        }
    }
    assert!(checked > 60, "{checked} choices");
}

// ---- 14-bit pairs at speed ------------------------------------------------------------------

fn wide_fader() -> CantusApp {
    let mut a = keys_app();
    add(
        &mut a,
        1,
        Control::Cc14(7),
        MappingTarget::Param(AutoTarget::MasterWidth),
        MapMode::Absolute,
    );
    a
}

fn width_of(v: u16) -> f32 {
    f32::from(v) / 16_383.0 * 2.0
}

/// #4, #11: a fader that sends its fine half first, pairs 2 ms apart (a quick move), each
/// in its own frame, across coarse steps: after its first pair every value is a whole pair
/// (no dip of a coarse step at a crossing), and it comes to rest on its last pair.
#[test]
fn a_fast_fine_first_fader_reads_whole_pairs() {
    let mut a = wide_fader();
    a.controller_heard(&[cc(7, 9), cc(39, 100)]);
    let mut t = Instant::now() + Duration::from_millis(50);
    let pairs: Vec<u16> = (0..40u16).map(|i| (10 << 7 | 120) + i * 3).collect();
    let mut seen = Vec::new();
    for (i, v) in pairs.iter().enumerate() {
        let (m, l) = ((v >> 7) as u8, (v & 0x7F) as u8);
        a.controller_heard(&[msg_at(0xB0, 39, l, t)]);
        a.controller_heard(&[msg_at(0xB0, 7, m, t + Duration::from_micros(300))]);
        if i > 1 {
            seen.push(a.project().master.width);
        }
        t += Duration::from_millis(2);
    }
    // No value below the pair two before it: never a coarse step back.
    for w in seen.windows(2) {
        assert!(w[1] >= w[0] - width_of(3), "a dip: {w:?}");
    }
    // At rest: the last pair, whole.
    let old = Instant::now() + Duration::from_secs(1);
    a.controller_heard(&[msg_at(0xB0, 100, 0, old)]);
    let last = *pairs.last().unwrap();
    assert!(
        (a.project().master.width - width_of(last)).abs() < 1e-4,
        "{} against {}",
        a.project().master.width,
        width_of(last)
    );
}

/// #37: a controller known to send its fine half first stays known at speed, and its last
/// pair is read whole.
#[test]
fn a_known_fine_first_fader_stays_known_at_speed() {
    let mut a = wide_fader();
    a.controller_heard(&[cc(7, 1), cc(39, 1)]);
    let mut t = Instant::now() + Duration::from_millis(50);
    // Known, slowly.
    for m in 2..6u8 {
        a.controller_heard(&[
            msg_at(0xB0, 39, 64, t),
            msg_at(0xB0, 7, m, t + Duration::from_micros(300)),
        ]);
        t += Duration::from_millis(20);
    }
    // Fast: 2 ms apart, a burst each.
    for v in (6u16 << 7..20 << 7).step_by(37) {
        a.controller_heard(&[
            msg_at(0xB0, 39, (v & 0x7F) as u8, t),
            msg_at(0xB0, 7, (v >> 7) as u8, t + Duration::from_micros(300)),
        ]);
        assert!(
            (a.project().master.width - width_of(v)).abs() < 1e-4,
            "{v}: {}",
            a.project().master.width
        );
        t += Duration::from_millis(2);
    }
}

/// #12: a coarse half left waiting on one port is judged against the burst's earliest
/// message, not the first port's: a later message on port 0 does not make it old.
#[test]
fn a_split_pair_on_another_port_waits_for_its_fine_half() {
    let mut a = keys_app();
    let pads = MidiSource {
        port: PortRef {
            name: "Pads".into(),
            ordinal: 0,
        },
        channel: Some(0),
        control: Control::Cc14(7),
    };
    let mut m = Mapping::new(
        MappingId(1),
        pads,
        MappingTarget::Param(AutoTarget::MasterWidth),
        MapMode::Absolute,
    );
    m.takeover = Takeover::Jump;
    assert!(a.add_mapping(Scope::Project, m));
    add(
        &mut a,
        2,
        Control::Cc(10),
        MappingTarget::Param(AutoTarget::MasterPan),
        MapMode::Absolute,
    );
    let t = Instant::now() + Duration::from_millis(50);
    a.controller_heard(&[msg_on(1, 0xB0, 7, 10, t), msg_on(1, 0xB0, 39, 5, t)]);
    let before = a.project().master.width;
    let t = t + Duration::from_millis(30);
    a.controller_heard(&[msg_on(1, 0xB0, 7, 11, t)]);
    assert_eq!(a.project().master.width, before, "the coarse half waits");
    // The next burst: port 0's knob 8 ms later, then port 1's fine half 0.3 ms later.
    a.controller_heard(&[
        msg_on(0, 0xB0, 10, 64, t + Duration::from_millis(8)),
        msg_on(1, 0xB0, 39, 3, t + Duration::from_micros(300)),
    ]);
    assert!(
        (a.project().master.width - width_of(11 << 7 | 3)).abs() < 1e-4,
        "the pair, whole: {}",
        a.project().master.width
    );
}

// ---- learning ----------------------------------------------------------------------------

/// #8, #24: an NRPN knob that sends its data entry alone (CC 6/38) is passed over, said,
/// and the learn stays armed: the next control moved is learnt.
#[test]
fn an_nrpn_burst_is_passed_over_and_the_learn_stays_armed() {
    let mut a = keys_app();
    let s = strip(&a);
    let pan = MappingTarget::Param(AutoTarget::StripPan(s));
    a.learn_target(pan.clone());
    a.controller_heard(&[cc(6, 64), cc(38, 0), cc(6, 65)]);
    assert!(a.status.error, "said");
    assert!(
        a.controllers.learning.as_ref().is_some_and(|l| l.target == pan),
        "still armed"
    );
    assert!(a.project().controllers.is_empty());
    a.controller_heard(&(0..8).map(|v| cc(21, 30 + v)).collect::<Vec<_>>());
    let m = &a.project().controllers.mappings[0];
    assert_eq!(m.source.control, Control::Cc(21));
}

/// #14: a mapping on an action this build does not know takes nothing: dormant, its
/// control free.
#[test]
fn an_unknown_actions_mapping_takes_nothing() {
    let mut a = keys_app();
    let m = Mapping::new(
        MappingId(3),
        source(Control::Note(36)),
        MappingTarget::Action("from-a-later-cantus".into()),
        MapMode::Trigger,
    );
    a.controllers.global.mappings.push(m.clone());
    assert!(!a.mapping_alive(Scope::Global, &m));
}

/// #6: a strip's record arm with no track to arm reads *No input* and takes nothing; a
/// track with an input brings it back.
#[test]
fn a_record_arm_with_nothing_to_arm_says_so() {
    use cantus_ui::controllers::Idle;
    let mut a = keys_app();
    let s = strip(&a);
    let m = Mapping::new(
        MappingId(4),
        source(Control::Note(41)),
        MappingTarget::StripArm(s),
        MapMode::Toggle,
    );
    a.add_mapping(Scope::Project, m.clone());
    assert_eq!(a.mapping_state(Scope::Project, &m), Some(Idle::NoInput));
    assert!(!a.mapping_alive(Scope::Project, &m));
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
    assert_eq!(a.mapping_state(Scope::Project, &m), None);
}

/// #29: opening another project while a learn is armed lets it go without a word: the
/// open's own status stays.
#[test]
fn a_new_project_lets_the_learn_go_silently() {
    let mut a = keys_app();
    let s = strip(&a);
    a.learn_target(MappingTarget::Param(AutoTarget::StripPan(s)));
    a.replace_project(cantus_ui::starter_project());
    a.set_status("links repaired".into());
    a.controller_heard(&[]);
    assert!(a.controllers.learning.is_none());
    assert_eq!(a.status.text, "links repaired");
}

// ---- recording: the pass that ran, the latch, the wraps ------------------------------------

/// Bar `n` (counting from 0) of the starter's 4/4.
fn bar(n: i64) -> Ticks {
    Ticks::beats(4 * n)
}

fn pan_knob(p: Project) -> (CantusApp, StripId) {
    let mut a = keys_app_with(p);
    let s = strip(&a);
    add(
        &mut a,
        1,
        Control::Cc(10),
        MappingTarget::Param(AutoTarget::StripPan(s)),
        MapMode::Absolute,
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

fn looped(p: &mut Project, from: i64, to: i64) {
    p.loop_region = Some(LoopRegion {
        start: bar(from),
        end: bar(to),
    });
    p.loop_enabled = true;
}

/// #15, #20: a loop from bar 4 to 8, recorded from bar 6; in the second lap the knob rides
/// from 4.5 to 5.5 and the take stops at 5.75 — before the record start, after a real
/// wrap: the automation is kept, from 4.5.
#[test]
fn a_later_lap_stopped_before_the_record_start_keeps_its_automation() {
    let mut p = cantus_ui::starter_project();
    looped(&mut p, 4, 8);
    let (mut a, s) = pan_knob(p);
    a.engine.seek(bar(6));
    a.start_recording();
    a.engine.set_heard_for_tests(bar(8) - Ticks(10), true);
    a.controller_heard(&[]);
    a.engine.set_heard_for_tests(bar(4) + Ticks::beats(2), true);
    a.controller_heard(&[cc(10, 40)]);
    a.engine.set_heard_for_tests(bar(5) + Ticks::beats(2), true);
    a.controller_heard(&[cc(10, 90)]);
    a.engine.set_heard_for_tests(bar(5) + Ticks::beats(3), false);
    a.stop_recording();
    let clips = pan_clips(&a, s);
    assert_eq!(clips.len(), 1, "kept");
    assert_eq!(clips[0].start, bar(4) + Ticks::beats(2));
}

/// Whether the lane the engine was last sent for `target` plays its clip (not latched).
fn plays_its_clip(a: &CantusApp, target: &AutoTarget) -> bool {
    a.engine
        .sent_snapshot()
        .and_then(|s| {
            s.automation
                .iter()
                .find(|l| &l.target == target)
                .map(|l| !l.segs.is_empty())
        })
        .expect("the lane")
}

/// #16, #25, #31: past the punch-out the parameter follows its clip again — the engine is
/// sent the lane anew when the playhead crosses it, with nothing else edited.
#[test]
fn the_latch_lets_go_in_the_engine_at_the_punch_out() {
    let mut p = cantus_ui::starter_project();
    p.recording.punch_enabled = true;
    p.recording.punch = Some(LoopRegion {
        start: bar(4),
        end: bar(8),
    });
    let (mut a, s) = pan_knob(p);
    let target = AutoTarget::StripPan(s);
    assert!(a.create_automation_clip(target.clone()));
    a.sync_engine();
    assert!(plays_its_clip(&a, &target), "the clip plays");
    a.engine.seek(bar(2));
    a.start_recording();
    a.engine.set_heard_for_tests(bar(6), true);
    a.controller_heard(&[cc(10, 100)]);
    a.sync_engine();
    assert!(!plays_its_clip(&a, &target), "latched in the punch");
    // Past the punch-out, nothing else done: the clip again.
    a.engine.set_heard_for_tests(bar(9), true);
    a.controller_heard(&[]);
    a.sync_engine();
    assert!(plays_its_clip(&a, &target), "the latch let go");
    a.stop_recording();
}

/// #17, #38: after a count-in's seek, a stale position from before it (the transport not
/// yet playing) is no wrap: a count-in move inside the loop lands at the record start.
#[test]
fn a_stale_position_after_the_count_in_is_no_wrap() {
    let mut p = cantus_ui::starter_project();
    looped(&mut p, 4, 8);
    p.recording.count_in_bars = 1;
    let (mut a, s) = pan_knob(p);
    a.engine.seek(bar(6));
    a.start_recording();
    a.engine.set_heard_for_tests(bar(6), false);
    a.controller_heard(&[]);
    a.engine.set_heard_for_tests(bar(5) + Ticks::beats(2), true);
    a.controller_heard(&[cc(10, 100)]);
    let points = a.modulators.rec.points.get(&AutoTarget::StripPan(s)).cloned().unwrap();
    assert!(points.iter().all(|(t, _)| *t >= bar(6).0), "{points:?}");
}

/// #17 (its second case): in the frame a loop wraps, a move recorded before the
/// controllers' turn (a plug-in window's) already belongs to the new lap.
#[test]
fn a_move_in_the_wrap_frame_belongs_to_the_new_lap() {
    let mut p = cantus_ui::starter_project();
    looped(&mut p, 4, 8);
    let (mut a, s) = pan_knob(p);
    a.engine.seek(bar(6));
    a.start_recording();
    a.engine.set_heard_for_tests(bar(8) - Ticks(10), true);
    a.controller_heard(&[]);
    a.engine.set_heard_for_tests(bar(4) + Ticks(40), true);
    a.controller_heard(&[]);
    let at = bar(4) + Ticks(20);
    a.record_point_at(AutoTarget::StripPan(s), 0.25, at.0);
    let points = a.modulators.rec.points.get(&AutoTarget::StripPan(s)).cloned().unwrap();
    assert!(points.iter().any(|(t, _)| *t == at.0), "{points:?}");
}

// ---- a strip's arm in a gesture -------------------------------------------------------------

/// #18: a strip's arm and its track's arm pressed in one gesture: the newest press wins.
#[test]
fn a_strips_arm_and_its_tracks_arm_newest_press_wins() {
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
    add(&mut a, 1, Control::Note(40), MappingTarget::StripArm(s), MapMode::Toggle);
    add(&mut a, 2, Control::Note(41), MappingTarget::TrackArm(t), MapMode::Toggle);
    let press = |a: &mut CantusApp, n: u8| {
        a.controller_heard(&[msg_at(0x90, n, 100, Instant::now())]);
        a.controller_heard(&[msg_at(0x80, n, 0, Instant::now())]);
    };
    press(&mut a, 40); // the strip: armed
    assert!(a.project().track(t).unwrap().armed);
    press(&mut a, 41); // the track: off
    assert!(!a.project().track(t).unwrap().armed);
    press(&mut a, 40); // the strip again: armed, whatever the gesture held
    assert!(a.project().track(t).unwrap().armed, "the newest press");
}

// ---- names of swapped-out plug-ins, and the pane's rows ------------------------------------

/// The pane painted at `size` (three frames): its painted text.
fn paint_pane(a: &mut CantusApp, size: egui::Vec2) -> String {
    paint(&ctx(), size, 3, |ui| cantus_ui::views::controllers::show(a, ui))
}

fn tagged(a: &CantusApp, id: MappingId, what: &str) -> bool {
    let name = format!("1#{}#{what}", id.0);
    a.controllers.view.geo.iter().any(|(n, _)| *n == name)
}

/// #47, #49, #61, #48, #41: a parameter learnt on a hosted insert keeps the plug-in's and
/// the parameter's names; the insert swapped for one of Cantus's own, its row reads *Other
/// plug-in* (not *Missing*), named by them; the pane's filter finds it by that name; an
/// undo of the swap brings it back to life.
#[test]
fn a_hosted_mapping_swapped_for_a_native_insert_is_another_plug_ins() {
    use cantus_ui::controllers::Idle;
    let (mut a, effect) = hosted_app();
    let s = a.project().effect_host(effect).expect("its strip");
    let target = MappingTarget::Param(AutoTarget::Plugin { effect, id: 0 });
    let info = a
        .hosted_param_info(&AutoTarget::Plugin { effect, id: 0 })
        .cloned()
        .expect("the parameter");
    a.learn_target(target);
    a.controller_heard(&(0..8).map(|v| cc(23, 40 + v)).collect::<Vec<_>>());
    let m = a.project().controllers.mappings.last().cloned().expect("learnt");
    let learnt = m.plugin.clone().expect("its plug-in");
    assert_eq!(learnt.param, info.name, "the parameter's name");
    assert!(!learnt.name.is_empty(), "the plug-in's name");
    assert!(a.replace_effect_with(
        s,
        effect,
        &cantus_ui::picker::PickChoice::Effect(EffectKind::Compressor)
    ));
    let now = a.project().controllers.get(m.id).cloned().expect("kept");
    assert_eq!(a.mapping_state(Scope::Project, &now), Some(Idle::OtherPlugin));
    let name = a.mapping_name(&now);
    assert!(
        name.contains(&learnt.name) && name.contains(&info.name),
        "{name}"
    );
    // The filter finds the row by the name it shows.
    a.controllers.view.filter = learnt.name.to_lowercase();
    let text = paint_pane(&mut a, egui::vec2(1400.0, 600.0));
    assert!(tagged(&a, m.id, "row"), "found by {}: {text}", learnt.name);
    a.undo();
    let back = a.project().controllers.get(m.id).cloned().unwrap();
    assert_eq!(a.mapping_state(Scope::Project, &back), None, "alive again");
}

/// #41: an insert swapped for another native effect that has the same parameter: its
/// mapping follows the new slot; one it lacks is left on the old (and undo restores both).
#[test]
fn swapping_an_insert_moves_the_parameters_the_new_one_has() {
    let mut a = keys_app();
    let s = strip(&a);
    let fx = insert(&mut a, EffectKind::Compressor);
    add(&mut a, 1, Control::Cc(21), effect(fx, "thr"), MapMode::Absolute);
    add(&mut a, 2, Control::Cc(22), effect(fx, "key_eq"), MapMode::Absolute);
    assert!(a.replace_effect_with(
        s,
        fx,
        &cantus_ui::picker::PickChoice::Effect(EffectKind::Gate)
    ));
    let new = a.project().strip(s).unwrap().effects.last().unwrap().id;
    let target_of = |a: &CantusApp, id: u64| {
        a.project()
            .controllers
            .get(MappingId(id))
            .map(|m| m.target.clone())
            .unwrap()
    };
    assert_eq!(target_of(&a, 1), effect(new, "thr"), "the gate has a threshold");
    assert_eq!(target_of(&a, 2), effect(fx, "key_eq"), "left on the compressor");
    a.undo();
    assert_eq!(target_of(&a, 1), effect(fx, "thr"));
}

/// #34: learning a control on the plug-in there now, while a mapping learnt on a
/// swapped-out one has it too: the status names the new mapping alone (no *also*), the
/// project holds both, and neither row wears *Shared*.
#[test]
fn learning_beside_a_swapped_out_mapping_shares_nothing() {
    use cantus_core::external::PluginFormat;
    let vital = ExternalPlugin::new(PluginFormat::Clap, "audio.vital", "Vital");
    let surge = ExternalPlugin::new(PluginFormat::Clap, "org.surge", "Surge");
    let mut p = cantus_ui::starter_project();
    let t = p.tracks[0].id;
    p.track_mut(t).unwrap().instrument = InstrumentSpec::External(vital);
    let mut a = keys_app_with(p);
    let target = MappingTarget::Param(AutoTarget::PluginInstrument { track: t, id: 7 });
    let mut m = Mapping::new(
        MappingId(1),
        source(Control::Cc(71)),
        target.clone(),
        MapMode::Absolute,
    );
    m.plugin = target.plugin_of(a.project());
    a.add_mapping(Scope::Project, m);
    a.exec(Command::SetInstrument {
        track: t,
        spec: Box::new(InstrumentSpec::External(surge)),
    });
    a.learn_target(target.clone());
    a.controller_heard(&(0..8).map(|v| cc(71, 60 + v)).collect::<Vec<_>>());
    assert_eq!(a.project().controllers.mappings.len(), 2, "both kept");
    let new = a.project().controllers.mappings.last().cloned().unwrap();
    let want = cantus_ui::i18n::tf(
        "ctl.status.learnt",
        &[
            ("source", &cantus_ui::controllers::source_text(&new.source)),
            ("name", &a.mapping_target_name(&new.target)),
        ],
    );
    assert_eq!(a.status.text, want);
    paint_pane(&mut a, egui::vec2(1400.0, 600.0));
    assert!(tagged(&a, new.id, "row"));
    assert!(!tagged(&a, new.id, "shared"), "no Shared");
    assert!(!tagged(&a, MappingId(1), "shared"), "nor on the old row");
}

/// #60: a row whose plug-in was swapped out shows its range in percent, not the loaded
/// plug-in's words for its own parameter of that id.
#[test]
fn a_swapped_out_row_reads_its_range_in_percent() {
    let (mut a, effect) = hosted_app();
    let target = MappingTarget::Param(AutoTarget::Plugin { effect, id: 2 });
    let mut m = Mapping::new(
        MappingId(9),
        source(Control::Cc(24)),
        target,
        MapMode::Absolute,
    );
    m.plugin = Some(PluginRef::new(
        cantus_core::external::PluginFormat::Clap,
        "org.other",
    ));
    a.add_mapping(Scope::Project, m);
    let text = paint_pane(&mut a, egui::vec2(1400.0, 600.0)).replace(' ', "");
    assert!(text.contains("|100%|"), "{text}");
    assert!(!text.contains("|On|"), "not the loaded plug-in's word: {text}");
}

/// #51: the master's effects switch is named as the pane names the master everywhere, in
/// Spanish too.
#[test]
fn the_masters_switch_is_named_as_the_master() {
    let a = keys_app();
    let (fx, vol) = in_both(|| {
        (
            a.mapping_target_name(&MappingTarget::StripFx(StripId::MASTER)),
            a.mapping_target_name(&MappingTarget::Param(AutoTarget::MasterGain)),
        )
    })
    .1;
    let master = vol.split(" \u{b7} ").next().unwrap().to_string();
    assert!(fx.starts_with(&master), "{fx} / {vol}");
}

/// #52: an absolute mapping on a switch shows no takeover (it takes none).
#[test]
fn a_switch_shows_no_takeover() {
    let mut a = keys_app();
    let s = strip(&a);
    let fx = insert(&mut a, EffectKind::Compressor);
    add(
        &mut a,
        1,
        Control::Cc(30),
        MappingTarget::StripMute(s),
        MapMode::Absolute,
    );
    add(&mut a, 2, Control::Cc(31), effect(fx, "key_eq"), MapMode::Absolute);
    add(&mut a, 3, Control::Cc(32), effect(fx, "threshold"), MapMode::Absolute);
    paint_pane(&mut a, egui::vec2(1400.0, 600.0));
    assert!(!tagged(&a, MappingId(1), "takeover"));
    assert!(!tagged(&a, MappingId(2), "takeover"));
    assert!(tagged(&a, MappingId(3), "takeover"), "a range takes one");
}

/// #26: the clips list and the device keyboard feed their state back.
#[test]
fn the_clips_list_and_the_keyboard_light_a_button() {
    let mut a = keys_app();
    let before = a.action_state("toggle-clips");
    assert!(before.is_some());
    a.run_action(cantus_ui::keymap::Action::ToggleClips);
    assert_ne!(a.action_state("toggle-clips"), before, "it follows the list");
    let track = a.project().tracks[0].id;
    a.set_instrument(
        track,
        InstrumentSpec::Native(NativeSpec::new(InstrumentKind::Analog)),
    );
    a.open_inst_editor(track);
    assert!(a.action_state("toggle-device-keyboard").is_some());
}

// ---- learning by clicking: one value of a choice, the inspector, the header ---------------

/// Paint `show` in learn mode, then click every learn spot it showed, one at a time: what
/// each click armed, with the range it was armed over.
fn armed_by_clicks(
    a: &mut CantusApp,
    size: egui::Vec2,
    show: &mut dyn FnMut(&mut CantusApp, &mut egui::Ui),
) -> Vec<(MappingTarget, Option<(f32, f32)>)> {
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
            armed.push((l.target.clone(), l.range));
        }
        a.cancel_learning();
    }
    armed
}

/// #46: a band's solo LED is learnt over none ↔ that band (never the whole 0…24 choice),
/// and a controller's mark shows on the LED whose band it reaches.
#[test]
fn a_bands_solo_led_is_learnt_for_that_band() {
    let mut a = keys_app();
    let fx = insert(&mut a, EffectKind::Eq);
    let s = strip(&a);
    cantus_ui::views::fx::demo_setup(&mut a, s, fx, EffectKind::Eq);
    a.open_effect_editor(s, fx);
    let armed = armed_by_clicks(&mut a, egui::vec2(1800.0, 1100.0), &mut |a, ui| {
        let ctx = ui.ctx().clone();
        cantus_ui::views::eq_editor::window(a, &ctx);
    });
    let solo: Vec<Option<(f32, f32)>> = armed
        .iter()
        .filter(|(t, _)| *t == effect(fx, "solo"))
        .map(|(_, r)| *r)
        .collect();
    assert!(!solo.is_empty(), "a solo LED learnt: {armed:?}");
    for r in &solo {
        let (from, to) = r.expect("over its band");
        assert_eq!(from, 0.0, "none");
        let band = to * 24.0;
        assert!((band - band.round()).abs() < 1e-4 && band >= 1.0, "a band: {band}");
    }
    let mut m = Mapping::new(
        MappingId(5),
        source(Control::Note(50)),
        effect(fx, "solo"),
        MapMode::Toggle,
    );
    (m.min, m.max) = (0.0, 3.0 / 24.0);
    a.add_mapping(Scope::Project, m);
    assert!(a.mapping_reaches(&effect(fx, "solo"), 3.0 / 24.0));
    assert!(!a.mapping_reaches(&effect(fx, "solo"), 5.0 / 24.0));
}

/// #46: a scale key is learnt over as the scale ↔ its click (removed for a note of the
/// scale, a target for one outside it).
#[test]
fn a_scale_key_is_learnt_over_its_click() {
    let mut a = keys_app();
    let fx = insert(&mut a, EffectKind::AutoTune);
    let s = strip(&a);
    a.open_effect_editor(s, fx);
    let armed = armed_by_clicks(&mut a, egui::vec2(1400.0, 900.0), &mut |a, ui| {
        let ctx = ui.ctx().clone();
        cantus_ui::views::fx::window(a, &ctx);
    });
    let keys: Vec<_> = armed
        .iter()
        .filter(|(t, _)| {
            matches!(t, MappingTarget::Param(AutoTarget::Effect { key, .. }) if key.starts_with('n'))
        })
        .collect();
    assert!(keys.len() >= 12, "{armed:?}");
    for (t, r) in keys {
        let (from, to) = r.expect("over its click");
        assert_eq!(from, 0.0, "{t:?}: as the scale");
        assert!(
            (to - 2.0 / 3.0).abs() < 1e-4 || (to - 1.0 / 3.0).abs() < 1e-4,
            "{t:?}: removed or a target, {to}"
        );
    }
}

/// #53: Organum's rotary lamp (lit while its output is 0) is learnt the lamp's way: *To*
/// lights it.
#[test]
fn a_lamp_lit_at_zero_is_learnt_the_lamps_way() {
    let mut a = keys_app();
    let track = a.project().tracks[0].id;
    a.set_instrument(
        track,
        InstrumentSpec::Native(NativeSpec::new(InstrumentKind::Organ)),
    );
    cantus_ui::views::inst::demo_setup(&mut a, track, InstrumentKind::Organ);
    a.open_inst_editor(track);
    a.inst.view = cantus_ui::views::inst::View::Play;
    let armed = armed_by_clicks(&mut a, egui::vec2(1800.0, 1100.0), &mut |a, ui| {
        cantus_ui::views::inst::window_in(a, ui)
    });
    let rot = armed
        .iter()
        .find(|(t, _)| *t == instrument(track, "rot.out"))
        .unwrap_or_else(|| panic!("the rotary lamp: {armed:?}"));
    assert_eq!(rot.1, Some((1.0, 0.0)));
}

/// #21, #36: the inspector's insert tile (its lamp, its ring), the inserts' switch, the
/// arm LED, and the device header's bypass lamp arm their targets; so does a strip's
/// record dot when a track records into it.
#[test]
fn the_inspector_header_and_record_dot_learn() {
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
    let fx = insert(&mut a, EffectKind::Compressor);
    a.select_strip(s);
    a.selected_track = Some(t);
    let inspector = armed_by_clicks(&mut a, egui::vec2(420.0, 1600.0), &mut |a, ui| {
        cantus_ui::views::inspector::show(a, ui)
    });
    let inspector: Vec<MappingTarget> = inspector.into_iter().map(|(t, _)| t).collect();
    for want in [
        MappingTarget::EffectBypass(fx),
        MappingTarget::EffectMix(fx),
        MappingTarget::StripFx(s),
    ] {
        assert!(inspector.contains(&want), "{want:?}: {inspector:?}");
    }
    a.open_effect_editor(s, fx);
    let header = armed_by_clicks(&mut a, egui::vec2(1400.0, 900.0), &mut |a, ui| {
        let ctx = ui.ctx().clone();
        cantus_ui::views::comp_editor::window(a, &ctx);
        cantus_ui::views::fx::window(a, &ctx);
    });
    assert!(
        header.iter().any(|(t, _)| *t == MappingTarget::EffectBypass(fx)),
        "the header's lamp: {header:?}"
    );
    let mixer = armed_by_clicks(&mut a, egui::vec2(1600.0, 700.0), &mut |a, ui| {
        cantus_ui::views::mixer::show(a, ui)
    });
    assert!(
        mixer.iter().any(|(tg, _)| *tg == MappingTarget::StripArm(s)),
        "the record dot: {mixer:?}"
    );
}

/// #21: the track's inspector arm LED learns its arm.
#[test]
fn the_inspectors_arm_led_learns_the_arm() {
    let mut a = keys_app();
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
    a.select_track(t);
    let armed = armed_by_clicks(&mut a, egui::vec2(420.0, 1800.0), &mut |a, ui| {
        cantus_ui::views::inspector::show(a, ui)
    });
    assert!(
        armed.iter().any(|(tg, _)| *tg == MappingTarget::TrackArm(t)),
        "{armed:?}"
    );
}

/// #35: clicking learns in a native effect's editor (its style segments, its key EQ LED),
/// an instrument's editor, and the arrangement's headers (mute, solo, arm).
#[test]
fn clicking_learns_in_editors_and_the_arrangement() {
    let mut a = keys_app();
    let fx = insert(&mut a, EffectKind::Compressor);
    let s = strip(&a);
    a.open_effect_editor(s, fx);
    let armed = armed_by_clicks(&mut a, egui::vec2(1400.0, 900.0), &mut |a, ui| {
        let ctx = ui.ctx().clone();
        cantus_ui::views::comp_editor::window(a, &ctx);
        cantus_ui::views::fx::window(a, &ctx);
    });
    for key in ["style", "key_eq", "thr"] {
        assert!(
            armed.iter().any(|(t, _)| *t == effect(fx, key)),
            "{key}: {armed:?}"
        );
    }
    let track = a.project().tracks[0].id;
    a.set_instrument(
        track,
        InstrumentSpec::Native(NativeSpec::new(InstrumentKind::Analog)),
    );
    cantus_ui::views::inst::demo_setup(&mut a, track, InstrumentKind::Analog);
    a.open_inst_editor(track);
    a.inst.view = cantus_ui::views::inst::View::Edit;
    let armed = armed_by_clicks(&mut a, egui::vec2(1800.0, 1400.0), &mut |a, ui| {
        cantus_ui::views::inst::window_in(a, ui)
    });
    assert!(
        armed.iter().any(|(t, _)| *t == instrument(track, "vcf.model")),
        "a choice: {armed:?}"
    );
    let armed = armed_by_clicks(&mut a, egui::vec2(1400.0, 600.0), &mut |a, ui| {
        cantus_ui::views::arrangement::show(a, ui)
    });
    for want in [
        MappingTarget::TrackMute(track),
        MappingTarget::TrackSolo(track),
    ] {
        assert!(armed.iter().any(|(t, _)| *t == want), "{want:?}: {armed:?}");
    }
}

// ---- the tempo gesture over audio windows --------------------------------------------------

/// #32: a gesture holding the tempo and a gain, over a song with trimmed audio windows
/// (whose tempo edits add a fix to the step on some frames, none on others), the gain ridden
/// alone for a long while: the step stays small, and one undo puts every window back.
#[test]
fn a_tempo_gesture_over_audio_windows_stays_small() {
    let mut p = cantus_ui::starter_project();
    let sid = AudioSourceId(p.alloc_id());
    p.audio_pool.push(AudioSource {
        id: sid,
        name: "take".into(),
        path: "take.wav".into(),
        sample_rate: 48_000,
        channels: 1,
        frames: 48_000_000,
        sha256: None,
        recorded: true,
    });
    let mut t = p.make_track("Vocal");
    for (start, offset) in [(16, 19), (24, 240), (32, 960)] {
        let mut c = p.make_audio_clip("w", sid, Ticks::beats(start), Ticks::beats(4));
        c.offset = Ticks(offset);
        c.audio.as_mut().unwrap().slip_frames = 37;
        t.clips.push(c);
    }
    let vocal = t.id;
    p.tracks.push(t);
    let mut a = keys_app_with(p);
    let s = strip(&a);
    add(
        &mut a,
        1,
        Control::Cc(16),
        MappingTarget::Tempo,
        MapMode::Relative(Encoding::TwosComplement),
    );
    add(
        &mut a,
        2,
        Control::Cc(10),
        MappingTarget::Param(AutoTarget::StripGain(s)),
        MapMode::Absolute,
    );
    let windows = |a: &CantusApp| {
        a.project()
            .track(vocal)
            .unwrap()
            .clips
            .iter()
            .map(|c| (c.start, c.offset, c.audio.as_ref().unwrap().slip_frames))
            .collect::<Vec<_>>()
    };
    let before = (a.project().tempo_bpm, windows(&a));
    a.controller_heard(&[cc(16, 3), cc(10, 50)]);
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
    assert_eq!((a.project().tempo_bpm, windows(&a)), before);
}

// ---- words in both languages ---------------------------------------------------------------

/// #39: every choice key the pane may show — an effect's and an instrument's — is in both
/// languages' own tables.
#[test]
fn every_choice_key_is_in_both_languages() {
    use cantus_ui::i18n::{has, Locale};
    let keys: Vec<String> = cantus_ui::controllers_words::effect_choice_keys()
        .into_iter()
        .chain(cantus_ui::inst_choices::all_keys())
        .collect();
    assert!(keys.len() > 200, "{}", keys.len());
    let missing: Vec<&String> = keys
        .iter()
        .filter(|k| !has(Locale::En, k) || !has(Locale::Es, k))
        .collect();
    assert!(missing.is_empty(), "{missing:?}");
}

/// #63: an in-process plug-in's "no words" for a value is its answer: kept, never asked
/// again every frame (an isolated one's is "not yet", asked again: review 3's test).
#[test]
fn an_in_process_plug_ins_no_words_is_kept() {
    use cantus_ui::views::plugins::ValueWords;
    let mut w = ValueWords::default();
    let bits = 0.42f64.to_bits();
    assert_eq!(w.get(EffectId(1), 0, bits, false, || None), None);
    assert_eq!(
        w.get(EffectId(1), 0, bits, false, || panic!("asked again")),
        None
    );
}
