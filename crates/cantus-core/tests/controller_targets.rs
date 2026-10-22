//! `AutoTarget::command` (round 29, ADR 0062): the command that sets a target to a value is
//! the exact inverse of `AutoTarget::value_of_command`, over every kind of target — so a
//! controller mapping, which holds a target and a value, sets exactly what a mouse would.

use cantus_core::automation::range::{base_value, range_of};
use cantus_core::external::{ExternalPlugin, PluginFormat};
use cantus_core::modulator::{ModHost, ModKind, Modulator};
use cantus_core::*;

/// A project with one of everything a target can name.
fn rich() -> (Project, Vec<AutoTarget>) {
    let mut p = Project::default();
    let firmus = p.make_track("Firmus");
    let mut native = p.make_track("Native");
    native.instrument = InstrumentSpec::Native(NativeSpec::new(InstrumentKind::Analog));
    let mut hosted = p.make_track("Hosted");
    hosted.instrument = InstrumentSpec::External(ExternalPlugin::new(
        PluginFormat::Clap,
        "org.test.synth",
        "Synth",
    ));
    let (tf, tn, th) = (firmus.id, native.id, hosted.id);
    p.tracks.extend([firmus, native, hosted]);
    let (s0, s1) = (p.strips[0].id, p.strips[1].id);
    p.strips[0].sends.push(Send::new(s1));
    let fx = p.make_effect(EffectKind::Compressor);
    let fx_id = fx.id;
    p.strips[0].effects.push(fx);
    let mut plug = p.make_effect(EffectKind::External);
    plug.spec = EffectSpec::external(ExternalPlugin::new(PluginFormat::Vst3, "ABCD0001", "Gain"));
    let plug_id = plug.id;
    p.strips[1].effects.push(plug);
    let master_fx = p.make_effect(EffectKind::Eq);
    let master_fx_id = master_fx.id;
    p.master.effects.push(master_fx);
    let m = Modulator::new(ModulatorId(p.alloc_id()), ModKind::Lfo);
    let m_id = m.id;
    p.modulators_mut(ModHost::Track(tf)).unwrap().push(m);
    let key = EffectKind::Compressor.params()[0].key.to_string();
    let master_key = EffectKind::Eq.params()[0].key.to_string();
    let inst_key = InstrumentKind::Analog
        .params()
        .iter()
        .find(|i| InstrumentKind::Analog.automatable(i.key))
        .unwrap()
        .key
        .to_string();
    let targets = vec![
        AutoTarget::TrackGain(tf),
        AutoTarget::TrackPan(tf),
        AutoTarget::StripGain(s0),
        AutoTarget::StripPan(s0),
        AutoTarget::StripWidth(s0),
        AutoTarget::Send { strip: s0, to: s1 },
        AutoTarget::MasterGain,
        AutoTarget::MasterPan,
        AutoTarget::MasterWidth,
        AutoTarget::Effect { effect: fx_id, key },
        AutoTarget::Effect {
            effect: master_fx_id,
            key: master_key,
        },
        AutoTarget::Instrument {
            track: tn,
            key: inst_key,
        },
        AutoTarget::Firmus {
            track: tf,
            param: FirmusParam::ALL[3],
        },
        AutoTarget::Plugin {
            effect: plug_id,
            id: 7,
        },
        AutoTarget::PluginInstrument { track: th, id: 3 },
        AutoTarget::ModAmount(m_id),
    ];
    (p, targets)
}

/// How many kinds of target there are.
const KINDS: usize = 15;

/// Each kind of target's number: an exhaustive match, no wildcard (review 1, #34).
fn kind(t: &AutoTarget) -> usize {
    match t {
        AutoTarget::TrackGain(_) => 0,
        AutoTarget::TrackPan(_) => 1,
        AutoTarget::StripGain(_) => 2,
        AutoTarget::StripPan(_) => 3,
        AutoTarget::StripWidth(_) => 4,
        AutoTarget::Send { .. } => 5,
        AutoTarget::MasterGain => 6,
        AutoTarget::MasterPan => 7,
        AutoTarget::MasterWidth => 8,
        AutoTarget::Effect { .. } => 9,
        AutoTarget::Instrument { .. } => 10,
        AutoTarget::Firmus { .. } => 11,
        AutoTarget::Plugin { .. } => 12,
        AutoTarget::PluginInstrument { .. } => 13,
        AutoTarget::ModAmount(_) => 14,
    }
}

/// A value inside the target's range (a hosted parameter's own range is the plug-in's:
/// any number goes to the project as it is).
fn a_value(p: &Project, t: &AutoTarget) -> f64 {
    let r = range_of(p, t).expect("the target exists");
    if t.is_plugin() {
        0.375
    } else {
        f64::from(r.value(0.3))
    }
}

#[test]
fn every_target_has_the_command_that_sets_it() {
    let (p, targets) = rich();
    // Every kind of target is here: `kind` matches every variant with no wildcard, so a
    // new one fails to compile until it has a number, and then this until rich() has it.
    let kinds: std::collections::BTreeSet<usize> = targets.iter().map(kind).collect();
    assert_eq!(
        kinds,
        (0..KINDS).collect(),
        "a kind of target is missing from rich()"
    );
    for t in &targets {
        let v = a_value(&p, t);
        let cmd = t
            .command(&p, v)
            .unwrap_or_else(|| panic!("{t:?}: no command"));
        // value_of_command reads back the same target and value…
        let (back, bv) = AutoTarget::value_of_command(&cmd).expect("read back");
        assert_eq!(&back, t);
        assert!((bv - v).abs() < 1e-6, "{t:?}: {v} → {bv}");
        // …and applied, the project holds that value.
        let mut q = p.clone();
        cmd.apply(&mut q).unwrap_or_else(|e| panic!("{t:?}: {e}"));
        if !t.is_plugin() {
            let now = base_value(&q, t).unwrap();
            assert!(
                (f64::from(now) - v).abs() < 1e-5,
                "{t:?}: set {v}, holds {now}"
            );
        }
    }
}

#[test]
fn command_of_value_of_command_is_the_command() {
    let (p, targets) = rich();
    for t in &targets {
        let c = t.command(&p, a_value(&p, t)).unwrap();
        let (target, value) = AutoTarget::value_of_command(&c).unwrap();
        assert_eq!(target.command(&p, value), Some(c), "{t:?}");
    }
}

#[test]
fn a_target_that_names_nothing_has_no_command() {
    let (p, _) = rich();
    for t in [
        AutoTarget::TrackGain(TrackId(9_999)),
        AutoTarget::StripPan(StripId(9_999)),
        AutoTarget::Effect {
            effect: EffectId(9_999),
            key: "x".into(),
        },
        AutoTarget::Plugin {
            effect: EffectId(9_999),
            id: 1,
        },
        AutoTarget::ModAmount(ModulatorId(9_999)),
    ] {
        assert_eq!(t.command(&p, 0.5), None, "{t:?}");
    }
    assert_eq!(AutoTarget::MasterGain.command(&p, f64::NAN), None);
}
