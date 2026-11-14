//! Tabula, the pedalboard (ADR 0134, provisional), behind the engine's plug-in contract.
//! Owned by the tabula builder.
//!
//! **The fx side round's wave-3 contract**: the board's own gains (`in`, `out`) and nothing
//! else — an empty board, a wire. Its pedals are not built yet: [`Unit::build`] makes an
//! empty board whatever the spec holds, so a board renders as a wire (with its gains) until
//! the tabula builder writes it: the children as [`EffectUnit`]s built here off the audio
//! thread, `adopt` of `EngineMessage::AddPedal`, `take_retired` (10 ms fades), the order and
//! the A/B split, the soft bypass that never changes the latency, the alignment lines
//! ([`Lines`], `MemoryNeed::Tabula`), the fan-out of key, notes and song position
//! (`set_position_late`), rest, and the per-pedal telemetry. **A board of one pedal must
//! render that unit alone, to the bit** (`fx_contract::a_board_of_one_is_the_unit`).

use super::FxUnit;
use crate::effects::EffectUnit;
use cantus_core::fx::tabula::PedalId;
use cantus_core::EffectSpec;

/// The board's own parameters as the DSP wants them.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Settings {
    pub in_db: f32,
    pub out_db: f32,
    pub mix: f32,
    pub ab: f32,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            in_db: 0.0,
            out_db: 0.0,
            mix: 1.0,
            ab: 0.5,
        }
    }
}

/// The board's own parameters, read from its spec.
pub fn settings(spec: &EffectSpec) -> Settings {
    Settings {
        in_db: spec.get("in"),
        out_db: spec.get("out"),
        mix: spec.get("mix"),
        ab: spec.get("ab"),
    }
}

/// One pedal of a running board: its id and its unit (what `FxUnit::pedals_mut` hands the
/// processor, to ask and supply its memory). The tabula builder adds what it needs.
#[derive(Debug)]
pub struct Kid {
    pub pedal: PedalId,
    pub unit: EffectUnit,
}

/// The board's alignment and dry lines (`MemoryNeed::Tabula(frames)`): built off the audio
/// thread, `frames` long per channel.
#[derive(Debug)]
pub struct Lines {
    pub left: Box<[f32]>,
    pub right: Box<[f32]>,
}

impl Lines {
    pub fn new(frames: usize) -> Self {
        Lines {
            left: vec![0.0; frames].into_boxed_slice(),
            right: vec![0.0; frames].into_boxed_slice(),
        }
    }
}

#[derive(Debug)]
pub struct Unit {
    gain: f32,
    /// The pedals, in no order of their own (the board's order is the spec's).
    kids: Vec<Kid>,
}

impl Unit {
    /// An empty board.
    pub fn new(sample_rate: f32) -> Self {
        let _ = sample_rate;
        Unit {
            gain: 1.0,
            kids: Vec::with_capacity(cantus_core::fx::tabula::MAX_PEDALS),
        }
    }

    /// The board for `spec`, every pedal built (the UI thread). **The contract**: an empty
    /// board whatever the spec holds.
    pub fn build(spec: &EffectSpec, sample_rate: f32) -> Self {
        let mut u = Unit::new(sample_rate);
        u.configure(spec);
        u
    }
}

impl FxUnit for Unit {
    fn fork(&self) -> Option<Box<dyn FxUnit>> {
        // Every pedal must fork for the board to (none yet).
        self.kids.is_empty().then(|| {
            Box::new(Unit {
                gain: self.gain,
                kids: Vec::with_capacity(cantus_core::fx::tabula::MAX_PEDALS),
            }) as Box<dyn FxUnit>
        })
    }

    fn configure(&mut self, spec: &EffectSpec) {
        let s = settings(spec);
        let db = s.in_db + s.out_db;
        self.gain = if db == 0.0 {
            1.0
        } else {
            cantus_dsp::math::db_to_gain(db)
        };
    }

    fn process(&mut self, left: &mut [f32], right: &mut [f32], _key: Option<(&[f32], &[f32])>) {
        if self.gain != 1.0 {
            for x in left.iter_mut().chain(right.iter_mut()) {
                *x *= self.gain;
            }
        }
    }

    fn reset(&mut self) {}

    /// No pedal, no state: always at rest.
    fn at_rest(&self) -> bool {
        true
    }

    fn skip(&mut self, _frames: usize) {}

    fn latency(&self) -> usize {
        0
    }

    fn pedals_mut(&mut self) -> &mut [Kid] {
        &mut self.kids
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cantus_core::EffectKind;

    #[test]
    fn the_tables_defaults_are_the_dsps_defaults() {
        let spec = EffectSpec::new(EffectKind::Board);
        assert_eq!(settings(&spec), Settings::default());
    }

    /// An empty board touches no sample, to the bit — and so does the contract's board
    /// whatever pedals its spec holds.
    #[test]
    fn an_empty_board_is_a_wire() {
        for spec in [
            EffectSpec::new(EffectKind::Board),
            cantus_core::fx::tabula::guitar_rig(),
        ] {
            let mut u = Unit::build(&spec, 48_000.0);
            let x: Vec<f32> = (0..64).map(|i| (i as f32 * 0.37).fract() - 0.5).collect();
            let (mut l, mut r) = (x.clone(), x.clone());
            u.process(&mut l, &mut r, None);
            assert!(l
                .iter()
                .chain(&r)
                .zip(x.iter().chain(&x))
                .all(|(a, b)| a.to_bits() == b.to_bits()));
            assert!(u.at_rest());
            assert_eq!(u.latency(), 0);
        }
    }
}
