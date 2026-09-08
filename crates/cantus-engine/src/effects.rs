//! Running instances of native effects. Units are **built on the UI thread** (they own
//! delay lines) and handed to the audio thread inside a `Box`; when a snapshot stops
//! referencing one, the audio thread hands the box back so it is freed off-thread.

use cantus_core::{EffectId, EffectKind, EffectSlot, EffectSpec};
use cantus_dsp::{DelaySettings, Reverb, ReverbSettings, StereoDelay};

#[derive(Debug)]
enum Dsp {
    Delay(StereoDelay),
    Reverb(Reverb),
}

#[derive(Debug)]
pub struct EffectUnit {
    pub id: EffectId,
    pub kind: EffectKind,
    dsp: Dsp,
}

impl EffectUnit {
    pub fn new(slot: &EffectSlot, sample_rate: f32) -> Self {
        let dsp = match slot.spec.kind {
            EffectKind::Delay => Dsp::Delay(StereoDelay::new(sample_rate)),
            EffectKind::Reverb => Dsp::Reverb(Reverb::new(sample_rate)),
        };
        let mut unit = EffectUnit {
            id: slot.id,
            kind: slot.spec.kind,
            dsp,
        };
        unit.configure(&slot.spec);
        unit.reset();
        unit
    }

    /// Apply parameters. A spec of another kind is ignored (cannot happen through the
    /// command layer, but the audio thread must never trust its inputs blindly).
    pub fn configure(&mut self, spec: &EffectSpec) {
        if spec.kind != self.kind {
            return;
        }
        match &mut self.dsp {
            Dsp::Delay(d) => d.set(&DelaySettings {
                time_ms: spec.get("time_ms"),
                feedback: spec.get("feedback"),
                tone_hz: spec.get("tone_hz"),
                ping_pong: spec.get("ping_pong") >= 0.5,
                mix: spec.get("mix"),
            }),
            Dsp::Reverb(r) => r.set(&ReverbSettings {
                size: spec.get("size"),
                damping: spec.get("damping"),
                width: spec.get("width"),
                predelay_ms: spec.get("predelay_ms"),
                mix: spec.get("mix"),
            }),
        }
    }

    pub fn reset(&mut self) {
        match &mut self.dsp {
            Dsp::Delay(d) => d.reset(),
            Dsp::Reverb(r) => r.reset(),
        }
    }

    #[inline]
    pub fn process(&mut self, l: &mut [f32], r: &mut [f32]) {
        match &mut self.dsp {
            Dsp::Delay(d) => d.process(l, r),
            Dsp::Reverb(v) => v.process(l, r),
        }
    }
}
