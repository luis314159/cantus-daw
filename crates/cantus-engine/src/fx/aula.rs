//! Aula, the convolution reverb, behind the engine's plug-in contract (ADR 0112). Owned by
//! Aula.
//!
//! The parameters the audio thread applies itself (pre-delay, width, mix, level) are set
//! in `configure`. Those that change the response (which one, stretch, length, decay, the
//! gate, damping, the cuts, reverse, attack) make a **need** — what to build, by value — the
//! engine asks the interface's thread for (`EngineEvent::EffectNeed`, ADR 0022); the kernel
//! comes back built and partitioned ([`super::aula_store::build`]) and is swapped in whole.
//! Meanwhile the response playing goes on playing. An offline render builds it at once
//! (`EffectUnit::new`), so a render hears exactly the response the project names.

use super::{FxUnit, Telemetry, TELEMETRY_LEN};
use crate::effects::{EffectMemory, MemoryNeed};
use cantus_core::EffectSpec;
use cantus_dsp::aula::{capacity_for, Aula, AulaSettings};
use cantus_dsp::aula_conv::State;
use cantus_dsp::aula_kernel::{Kernel, Layout};
use cantus_dsp::aula_shape::Shape;
use std::sync::Arc;

/// Telemetry: 1 when the response playing is the one the parameters ask for.
pub const T_READY: usize = 5;

/// What a response is built from, by value (so a need can be compared and copied on the
/// audio thread): the source's key (Cantus's own index, or a file's
/// [`super::aula_store::file_key`]), the shape's bits, and the room the unit has.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct AulaNeed {
    pub source: u64,
    pub shape: [u32; 9],
    pub cap: Layout,
}

impl AulaNeed {
    pub fn shape(&self) -> Shape {
        let f = f32::from_bits;
        let b = self.shape;
        Shape {
            stretch: f(b[0]),
            length: f(b[1]),
            reverse: b[2] != 0,
            attack_ms: f(b[3]),
            decay: f(b[4]),
            gate_db: f(b[5]),
            low_hz: f(b[6]),
            high_hz: f(b[7]),
            damp: f(b[8]),
        }
    }
}

/// What answers an [`AulaNeed`]: the kernel, and a new state when it needs more room. It
/// travels back (empty, or with the state it replaced) to be freed off the audio thread.
#[derive(Debug)]
pub struct Memory {
    pub need: AulaNeed,
    pub kernel: Option<Arc<Kernel>>,
    pub state: Option<Box<State>>,
}

/// The response's source and shape a spec asks for. Allocation-free.
pub fn want(spec: &EffectSpec) -> (u64, [u32; 9]) {
    let source = match &spec.ir {
        Some(f) => super::aula_store::file_key(&f.path),
        None => spec.get("ir").round().max(0.0) as u64,
    };
    (source, super::aula_store::shape_bits(&shape(spec)))
}

/// The shape a spec asks for. Allocation-free.
pub fn shape(spec: &EffectSpec) -> Shape {
    Shape {
        stretch: spec.get("stretch"),
        length: spec.get("length"),
        reverse: spec.get("reverse") >= 0.5,
        attack_ms: spec.get("attack"),
        decay: spec.get("decay"),
        gate_db: spec.get("gate"),
        low_hz: spec.get("low"),
        high_hz: spec.get("high"),
        damp: spec.get("damp"),
    }
}

/// The settings the audio thread applies; also what the editor reads.
pub fn settings(spec: &EffectSpec) -> AulaSettings {
    AulaSettings {
        predelay_ms: spec.get("predelay"),
        width: spec.get("width"),
        mix: spec.get("mix"),
        level_db: spec.get("level"),
    }
}

#[derive(Clone, Debug)]
pub struct Unit {
    dsp: Aula,
    want: (u64, [u32; 9]),
    /// What the kernel playing was built from (`None`: the silence it starts with).
    have: Option<(u64, [u32; 9])>,
}

impl Unit {
    /// Allocates (the pre-delay, a first state): on the interface's thread.
    pub fn new(sample_rate: f32) -> Self {
        let cap = capacity_for(0);
        Unit {
            dsp: Aula::with_kernel(sample_rate, super::aula_store::silent(cap)),
            want: (u64::MAX, [0; 9]),
            have: None,
        }
    }
}

impl FxUnit for Unit {
    fn fork(&self) -> Option<Box<dyn FxUnit>> {
        Some(Box::new(self.clone()))
    }

    fn configure(&mut self, spec: &EffectSpec) {
        self.want = want(spec);
        self.dsp.set(&settings(spec));
    }

    fn process(&mut self, left: &mut [f32], right: &mut [f32], _key: Option<(&[f32], &[f32])>) {
        self.dsp.process(left, right);
    }

    fn reset(&mut self) {
        self.dsp.reset();
    }

    fn at_rest(&self) -> bool {
        self.dsp.at_rest()
    }

    fn rest_blocker(&self) -> Option<&'static str> {
        self.dsp.rest_blocker()
    }

    fn skip(&mut self, frames: usize) {
        self.dsp.skip(frames);
    }

    fn latency(&self) -> usize {
        self.dsp.latency()
    }

    fn telemetry(&self) -> Telemetry {
        let mut t = [0.0; TELEMETRY_LEN];
        self.dsp.telemetry(&mut t);
        t[T_READY] = if self.have == Some(self.want) {
            1.0
        } else {
            0.0
        };
        t
    }

    fn memory_need(&self) -> Option<MemoryNeed> {
        (self.have != Some(self.want)).then(|| {
            MemoryNeed::Aula(AulaNeed {
                source: self.want.0,
                shape: self.want.1,
                cap: self.dsp.convolver().cap(),
            })
        })
    }

    fn supply(&mut self, memory: EffectMemory) -> Option<EffectMemory> {
        let EffectMemory::Aula(mut m) = memory else {
            return Some(memory);
        };
        let Some(kernel) = m.kernel.take() else {
            return Some(EffectMemory::Aula(m));
        };
        match self.dsp.supply(kernel, m.state.take()) {
            Ok(old) => {
                // Even when the parameters moved on meanwhile: closer than before, and the
                // next ask brings the rest.
                self.have = Some((m.need.source, m.need.shape));
                m.state = old;
            }
            Err((k, s)) => {
                m.kernel = Some(k);
                m.state = s;
            }
        }
        // The box itself is freed off the audio thread too.
        Some(EffectMemory::Aula(m))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cantus_core::EffectKind;

    #[test]
    fn the_tables_defaults_are_the_dsps_defaults() {
        let spec = EffectSpec::new(EffectKind::Aula);
        assert_eq!(settings(&spec), AulaSettings::default());
        assert_eq!(shape(&spec), Shape::default());
    }

    #[test]
    fn a_new_unit_asks_for_its_response_and_takes_it() {
        let mut u = Unit::new(48_000.0);
        let mut spec = EffectSpec::new(EffectKind::Aula);
        spec.set("ir", 3.0);
        u.configure(&spec);
        let Some(MemoryNeed::Aula(need)) = u.memory_need() else {
            panic!("no need");
        };
        assert_eq!(need.source, 3);
        let back = u.supply(EffectMemory::build(MemoryNeed::Aula(need), 48_000.0));
        assert!(
            matches!(back, Some(EffectMemory::Aula(_))),
            "the box goes back"
        );
        assert!(u.memory_need().is_none());
        assert_eq!(u.telemetry()[T_READY], 1.0);
        // Pre-delay and mix are the audio thread's: no new kernel.
        spec.set("predelay", 40.0);
        spec.set("mix", 0.5);
        u.configure(&spec);
        assert!(u.memory_need().is_none());
        spec.set("decay", 0.5);
        u.configure(&spec);
        assert!(u.memory_need().is_some());
    }
}
