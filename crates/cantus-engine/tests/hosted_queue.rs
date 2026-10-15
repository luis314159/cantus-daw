//! A hosted plug-in's timed values never lost (ADR 0057, review 1): with more parameters
//! moving than the old queue held (40 here, past 32), every grid point of every pass still
//! reaches the unit, and anything that ever found no room is counted in
//! `EngineStats::dropped_events`, never silent. A fake hosted unit records what it hears.

use cantus_core::modulator::*;
use cantus_core::*;
use cantus_engine::fx::HostedValue;
use cantus_engine::*;
use std::collections::BTreeSet;
use std::sync::{Arc, Mutex};

const SR: f64 = 48_000.0;
const PASSES: u64 = 8;

/// A hosted unit that keeps every timed value it hears, at its sample.
#[derive(Debug)]
struct Ears {
    heard: Arc<Mutex<Vec<(u64, u32)>>>,
    at: u64,
}

impl FxUnit for Ears {
    fn configure(&mut self, _spec: &EffectSpec) {}
    fn process(&mut self, left: &mut [f32], _right: &mut [f32], _key: Option<(&[f32], &[f32])>) {
        self.at += left.len() as u64;
    }
    fn reset(&mut self) {}
    fn at_rest(&self) -> bool {
        false
    }
    fn automate(&mut self, offset: usize, id: u32, _value: HostedValue) {
        self.heard
            .lock()
            .unwrap()
            .push((self.at + offset as u64, id));
    }
}

/// Play 8 passes of `lanes` hosted parameters moving at every grid point: what the unit
/// heard (sample from the first value, parameter) and the engine's dropped count.
fn play(lanes: u32) -> (Vec<(u64, u32)>, u64) {
    let mut p = Project::new("queue");
    p.tempo_bpm = 120.0;
    let mut lead = p.make_track("Lead");
    let mut clip = p.make_clip("held", Ticks::ZERO, Ticks::beats(16));
    let n = p.make_note(Ticks::ZERO, Ticks::beats(16), 60, 100);
    clip.notes.push(n);
    lead.clips.push(clip);
    let strip = lead.strip.unwrap();
    let track = lead.id;
    p.tracks.push(lead);
    let mut plugin = ExternalPlugin::new(PluginFormat::Clap, "test.ears", "Ears");
    for id in 0..lanes {
        plugin.params.insert(id, 0.5);
    }
    let mut slot = p.make_effect(EffectKind::Eq);
    slot.spec = EffectSpec::external(plugin);
    let effect = slot.id;
    p.strip_mut(strip).unwrap().effects.push(slot.clone());
    // Rising saws (a new value at every grid point), up to 32 parameters each.
    for first in (0..lanes).step_by(32) {
        let id = ModulatorId(p.alloc_id());
        let mut m = Modulator::new(id, ModKind::Lfo);
        if let ModSource::Lfo(l) = &mut m.source {
            l.shape = LfoShape::SawUp;
        }
        for k in first..(first + 32).min(lanes) {
            m.routes.push(ModRoute {
                target: AutoTarget::Plugin { effect, id: k },
                depth: 0.3,
            });
        }
        p.track_mut(track).unwrap().modulators.push(m);
    }
    let heard = Arc::new(Mutex::new(Vec::new()));
    let (mut engine, mut processor) = EngineHandle::manual(SR);
    assert!(engine.add_unit(EffectUnit::from_fx(
        &slot,
        Box::new(Ears {
            heard: heard.clone(),
            at: 0,
        }),
    )));
    let snapshot = engine.compile(&p, 1);
    engine.set_snapshot(snapshot);
    let mut buf = vec![0f32; 1024 * 2];
    for _ in 0..16 {
        processor.process(&mut buf[..512], 2);
        engine.poll();
    }
    heard.lock().unwrap().clear();
    engine.seek(Ticks::ZERO);
    engine.play();
    for _ in 0..PASSES {
        processor.process(&mut buf, 2);
        engine.poll();
    }
    let heard = heard.lock().unwrap().clone();
    let first = heard.iter().map(|h| h.0).min().expect("values arrive");
    let heard = heard.into_iter().map(|(at, id)| (at - first, id)).collect();
    (heard, engine.stats().dropped_events)
}

#[test]
fn forty_moving_hosted_parameters_reach_the_unit_at_every_grid_point() {
    let lanes = 40;
    let (heard, dropped) = play(lanes);
    for id in 0..lanes {
        let at: BTreeSet<u64> = heard.iter().filter(|h| h.1 == id).map(|h| h.0).collect();
        for g in (0..PASSES * 1024).step_by(16) {
            assert!(at.contains(&g), "parameter {id} misses sample {g}");
        }
    }
    assert_eq!(dropped, 0);
}

#[test]
fn more_than_a_pass_holds_is_counted_never_silent() {
    // 136 parameters moving at every grid point of a 1 024-sample pass: 8 704 values,
    // past the 8 192 a unit's queue holds.
    let (_, dropped) = play(136);
    assert!(dropped > 0, "an overflow is reported");
}

/// A hosted unit that keeps the last value it heard per parameter.
#[derive(Debug)]
struct Last {
    last: Arc<Mutex<Vec<(u32, HostedValue)>>>,
}

impl FxUnit for Last {
    fn configure(&mut self, _spec: &EffectSpec) {}
    fn process(&mut self, _l: &mut [f32], _r: &mut [f32], _key: Option<(&[f32], &[f32])>) {}
    fn reset(&mut self) {}
    fn at_rest(&self) -> bool {
        false
    }
    fn automate(&mut self, _offset: usize, id: u32, value: HostedValue) {
        let mut last = self.last.lock().unwrap();
        match last.iter_mut().find(|l| l.0 == id) {
            Some(l) => l.1 = value,
            None => last.push((id, value)),
        }
    }
}

/// A project with `lanes` parameters of one hosted insert, each under an automation
/// clip (no modulators: no control point after the first, only the hosted grid): a ramp
/// from 0 to 1 over the first two bars, then 1 held for two. The effect slot and the
/// project.
fn ramp_then_hold(lanes: u32) -> (Project, EffectSlot) {
    let mut p = Project::new("held");
    p.tempo_bpm = 120.0;
    let mut lead = p.make_track("Lead");
    let strip = lead.strip.unwrap();
    let mut plugin = ExternalPlugin::new(PluginFormat::Clap, "test.last", "Last");
    for id in 0..lanes {
        plugin.params.insert(id, 0.5);
    }
    let mut slot = p.make_effect(EffectKind::Eq);
    slot.spec = EffectSpec::external(plugin);
    let effect = slot.id;
    for id in 0..lanes {
        let mut clip = p.make_clip("ramp", Ticks::ZERO, Ticks::beats(16));
        clip.automation = Some(Automation::new(
            AutoTarget::Plugin { effect, id },
            vec![
                AutoPoint::new(Ticks::ZERO, 0.0),
                AutoPoint::new(Ticks::beats(8), 1.0),
            ],
        ));
        lead.clips.push(clip);
    }
    p.tracks.push(lead);
    p.strip_mut(strip).unwrap().effects.push(slot.clone());
    (p, slot)
}

type Heard = Arc<Mutex<Vec<(u32, HostedValue)>>>;

fn last_unit(slot: &EffectSlot) -> (EffectUnit, Heard) {
    let last = Arc::new(Mutex::new(Vec::new()));
    let unit = EffectUnit::from_fx(slot, Box::new(Last { last: last.clone() }));
    (unit, last)
}

/// Engine and processor playing `p` from the start with `unit`, after a first pass of
/// `lead` frames and `passes` passes of 1 024.
fn playing(p: &Project, unit: EffectUnit, lead: usize, passes: usize) -> (EngineHandle, Processor) {
    let (mut engine, mut processor) = EngineHandle::manual(SR);
    assert!(engine.add_unit(unit));
    let snapshot = engine.compile(p, 1);
    engine.set_snapshot(snapshot);
    let mut buf = vec![0f32; 1024 * 2];
    processor.process(&mut buf, 2);
    engine.poll();
    engine.seek(Ticks::ZERO);
    engine.play();
    processor.process(&mut buf[..2 * lead], 2);
    for _ in 0..passes {
        processor.process(&mut buf, 2);
        engine.poll();
    }
    (engine, processor)
}

#[test]
fn a_held_value_whose_change_found_no_room_reaches_the_unit_later() {
    // Review 2: a lane was marked as sent although its change was dropped (the queue
    // full), so once it held still its final value never reached the plug-in. 200
    // parameters ramping at once overflow every pass of the ramp.
    let lanes = 200;
    let (p, slot) = ramp_then_hold(lanes);
    let (unit, last) = last_unit(&slot);
    // The ramp ends at 192 000 samples, 800 into a pass (past the 41 grid points of 200
    // values that fit): its last values find no room. Then 1 is held.
    let (engine, _processor) = playing(&p, unit, 736, 200);
    assert!(engine.stats().dropped_events > 0, "the ramp overflowed");
    let last = last.lock().unwrap().clone();
    assert_eq!(last.len(), lanes as usize);
    for (id, v) in last {
        assert_eq!(v.pos, 1.0, "parameter {id} never heard its held value");
    }
}

#[test]
fn a_unit_swapped_in_place_hears_the_held_value_within_a_grid_step() {
    // Review 2: the engine queues only a change, and that memory outlived a unit swapped
    // in place (a plug-in's restart): the fresh instance never heard the held value.
    let (p, slot) = ramp_then_hold(4);
    let (unit, _) = last_unit(&slot);
    let (mut engine, mut processor) = playing(&p, unit, 0, 200);
    // Held at 1 since 192 000: the restarted plug-in's instance comes in.
    let (fresh, last) = last_unit(&slot);
    assert!(engine.replace_unit(fresh));
    let mut buf = vec![0f32; 16 * 2];
    processor.process(&mut buf, 2);
    engine.poll();
    let last = last.lock().unwrap().clone();
    assert_eq!(
        last.len(),
        4,
        "every parameter within a grid step: {last:?}"
    );
    for (id, v) in last {
        assert_eq!(v.pos, 1.0, "parameter {id}");
    }
}

/// A hosted unit nothing automates whose plug-in lost `lose` changes of its own (a
/// snapshot or preset past a VST3 block's 128 queues): it reports them once.
#[derive(Debug)]
struct Loses {
    lose: u64,
}

impl FxUnit for Loses {
    fn configure(&mut self, _spec: &EffectSpec) {}
    fn process(&mut self, _l: &mut [f32], _r: &mut [f32], _key: Option<(&[f32], &[f32])>) {}
    fn reset(&mut self) {}
    fn at_rest(&self) -> bool {
        false
    }
    fn take_dropped(&mut self) -> u64 {
        std::mem::take(&mut self.lose)
    }
}

impl InstrumentUnit for Loses {
    fn configure(&mut self, _spec: &NativeSpec, _assets: &cantus_engine::inst::Assets) {}
    fn note_on(&mut self, _note: NoteOn) {}
    fn note_off(&mut self, _key: u8, _id: u64, _velocity: f32) {}
    fn all_notes_off(&mut self) {}
    fn reset(&mut self) {}
    fn process(&mut self, _l: &mut [f32], _r: &mut [f32]) -> bool {
        false
    }
    fn active_voices(&self) -> usize {
        0
    }
    fn take_dropped(&mut self) -> u64 {
        std::mem::take(&mut self.lose)
    }
}

#[test]
fn an_unautomated_hosted_units_losses_are_counted() {
    // Review 2: drops were collected only from units the plan automates, so a plug-in
    // nothing automates lost changes silently.
    let mut p = Project::new("losses");
    let mut lead = p.make_track("Lead");
    let plugin = ExternalPlugin::new(PluginFormat::Clap, "test.synth", "Synth");
    let key = hosted_key(&plugin);
    lead.instrument = InstrumentSpec::External(plugin.clone());
    let strip = lead.strip.unwrap();
    let track = lead.id;
    p.tracks.push(lead);
    let mut slot = p.make_effect(EffectKind::Eq);
    slot.spec = EffectSpec::external(plugin);
    p.strip_mut(strip).unwrap().effects.push(slot.clone());
    let mut master = p.make_effect(EffectKind::Eq);
    master.spec = slot.spec.clone();
    p.master.effects.push(master.clone());
    let (mut engine, mut processor) = EngineHandle::manual(SR);
    assert!(engine.add_unit(EffectUnit::from_fx(&slot, Box::new(Loses { lose: 3 }))));
    assert!(engine.add_unit(EffectUnit::from_fx(&master, Box::new(Loses { lose: 5 }))));
    assert!(engine.add_instrument(NativeUnit::hosted(track, key, Box::new(Loses { lose: 7 }))));
    let snapshot = engine.compile(&p, 1);
    engine.set_snapshot(snapshot);
    let mut buf = vec![0f32; 512 * 2];
    for _ in 0..4 {
        processor.process(&mut buf, 2);
        engine.poll();
    }
    assert_eq!(engine.stats().dropped_events, 3 + 5 + 7);
}
