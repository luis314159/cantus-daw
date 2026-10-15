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
