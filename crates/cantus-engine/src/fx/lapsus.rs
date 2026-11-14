//! Lapsus, the time shaper (ADR 0132, provisional), behind the engine's plug-in contract.
//! Owned by Lapsus.
//!
//! [`settings`] reads the table (`cantus_core::fx::lapsus`) into the DSP's settings, and
//! [`Unit::configure`] hands the DSP the bank's patterns (`EffectSpec::lapsus`, the factory
//! bank where it has none) — compared slot by slot without allocating, so a configure that
//! changes a knob costs nothing for the slots that stayed. The segments bend with the
//! automation editor's shapes (`fluctus::shape_at`). The history is asked for through the
//! memory path (ADR 0022: `MemoryNeed::Lapsus`) and built off the audio thread. Notes
//! arrive at their sample (`TAKES_NOTES`).

use super::fluctus::shape_at;
use super::modfx::{telemetry_of, transport_of};
use super::{FxNote, FxUnit, SongPosition, Telemetry};
use crate::effects::{EffectMemory, MemoryNeed};
use cantus_core::fx::lapsus as model;
use cantus_core::EffectSpec;
use cantus_dsp::fluctus::CurvePoint;
use cantus_dsp::lapsus::{
    Lane, Lapsus, LapsusSettings, Note, NoteMode, Pattern, MAX_POINTS, SLOTS, TIME_HI, TIME_LO,
};

/// Lapsus's parameters as the DSP wants them; also what its editor draws from.
pub fn settings(spec: &EffectSpec) -> LapsusSettings {
    let slot = |k: &str| spec.get(k).round().clamp(0.0, (SLOTS - 1) as f32) as usize;
    let key = |k: &str| spec.get(k).round().clamp(0.0, 127.0) as u8;
    LapsusSettings {
        tslot: slot("tslot"),
        vslot: slot("vslot"),
        mix: spec.get("mix"),
        xfade_ms: spec.get("xfade"),
        smooth_ms: spec.get("smooth"),
        notes: NoteMode::from_index(spec.get("notes")),
        retrig: spec.get("retrig") >= 0.5,
        tkey: key("tkey"),
        vkey: key("vkey"),
        ahead_ms: spec.get("ahead"),
        out_db: spec.get("out"),
    }
}

/// A curve's points as the DSP takes them, into `out`; how many.
fn points(src: &[model::Point], out: &mut [CurvePoint; MAX_POINTS]) -> usize {
    let n = src.len().min(MAX_POINTS);
    for (d, p) in out.iter_mut().zip(&src[..n]) {
        *d = CurvePoint {
            x: p.x,
            y: p.y,
            tension: p.t,
            shape: p.s,
        };
    }
    n
}

/// The DSP's lane of a model lane.
pub fn lane(l: model::Lane) -> Lane {
    match l {
        model::Lane::Time => Lane::Time,
        model::Lane::Vol => Lane::Vol,
    }
}

/// A model pattern of `l` as the DSP plays it. Allocation-free.
pub fn pattern(l: model::Lane, p: &model::Pattern) -> Pattern {
    let mut pts = [CurvePoint::default(); MAX_POINTS];
    let n = points(&p.points, &mut pts);
    Pattern::new(lane(l), p.beats(), &pts[..n])
}

#[derive(Clone, Debug)]
pub struct Unit {
    dsp: Lapsus,
    /// Which slots of each bank hold the factory's pattern now, so a configure skips them.
    factory: [[bool; SLOTS]; 2],
}

impl Unit {
    pub fn new(sample_rate: f32) -> Self {
        let mut dsp = Lapsus::new(sample_rate, shape_at);
        // The factory banks, built here (off the audio thread) and loaded.
        for l in model::Lane::ALL {
            for (slot, p) in model::factory(l).iter().enumerate() {
                dsp.set_pattern(lane(l), slot, &pattern(l, p));
            }
        }
        dsp.reset();
        Unit {
            dsp,
            factory: [[true; SLOTS]; 2],
        }
    }

    /// The DSP, for the editor's drawing and the tests.
    pub fn dsp(&self) -> &Lapsus {
        &self.dsp
    }

    /// Hand slot `slot` of `l` the bank's pattern if it differs from what the DSP holds.
    fn load(&mut self, l: model::Lane, slot: usize, p: &model::Pattern, is_factory: bool) {
        let mut pts = [CurvePoint::default(); MAX_POINTS];
        let n = points(&p.points, &mut pts);
        let dl = lane(l);
        if self.dsp.pattern_differs(dl, slot, p.beats(), &pts[..n]) {
            self.dsp
                .set_pattern(dl, slot, &Pattern::new(dl, p.beats(), &pts[..n]));
        }
        self.factory[dl as usize][slot] = is_factory;
    }
}

impl FxUnit for Unit {
    fn fork(&self) -> Option<Box<dyn FxUnit>> {
        Some(Box::new(self.clone()))
    }

    fn configure(&mut self, spec: &EffectSpec) {
        self.dsp.set(&settings(spec));
        let bank = spec.lapsus.as_deref();
        for l in model::Lane::ALL {
            let li = lane(l) as usize;
            for slot in 0..SLOTS {
                match bank.and_then(|b| b.lane(l).get(&(slot as u8))) {
                    Some(own) => self.load(l, slot, own, false),
                    None if !self.factory[li][slot] => {
                        self.load(l, slot, &model::factory(l)[slot], true)
                    }
                    None => {}
                }
            }
        }
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
        telemetry_of(|t| self.dsp.telemetry(t))
    }

    fn follows_song(&self) -> bool {
        true
    }

    fn set_position(&mut self, position: &SongPosition) {
        self.dsp.set_transport(&transport_of(position));
    }

    /// The notes of the slot's track (ADR 0022): they choose the pattern, at their sample.
    fn note(&mut self, offset: usize, note: FxNote) {
        self.dsp.note(
            offset,
            match note {
                FxNote::On { pitch, .. } => Note::On(pitch),
                FxNote::Off { pitch } => Note::Off(pitch),
                FxNote::AllOff => Note::AllOff,
            },
        );
    }

    fn memory_need(&self) -> Option<MemoryNeed> {
        self.dsp.memory_need().map(MemoryNeed::Lapsus)
    }

    fn supply(&mut self, memory: EffectMemory) -> Option<EffectMemory> {
        match memory {
            EffectMemory::Lapsus(mut ring) => {
                // The old history goes back in the box the new one came in: nothing is
                // allocated or freed here.
                let new = std::mem::take(&mut *ring);
                *ring = self.dsp.supply(new);
                Some(EffectMemory::Lapsus(ring))
            }
            other => Some(other),
        }
    }
}

/// The DSP's curve range, for the editor's drawing (cycle lengths).
pub const TIME_RANGE: (f32, f32) = (TIME_LO, TIME_HI);

#[cfg(test)]
mod tests {
    use super::*;
    use cantus_core::EffectKind;

    /// The defaults of the table read as the DSP's defaults: the null rule holds through
    /// the mapping.
    #[test]
    fn the_tables_defaults_are_the_dsps_defaults() {
        let spec = EffectSpec::new(EffectKind::TimeShaper);
        assert_eq!(settings(&spec), LapsusSettings::default());
    }

    #[test]
    fn the_settings_read_every_key() {
        let mut spec = EffectSpec::new(EffectKind::TimeShaper);
        for (k, v) in [
            ("tslot", 7.0),
            ("vslot", 9.0),
            ("mix", 0.5),
            ("xfade", 5.0),
            ("smooth", 3.0),
            ("notes", 1.0),
            ("retrig", 1.0),
            ("tkey", 70.0),
            ("vkey", 10.0),
            ("ahead", 20.0),
            ("out", -3.0),
        ] {
            spec.set(k, v);
        }
        let s = settings(&spec);
        assert_eq!(
            (s.tslot, s.vslot, s.mix, s.xfade_ms, s.smooth_ms),
            (7, 9, 0.5, 5.0, 3.0)
        );
        assert_eq!(
            (s.notes, s.retrig, s.tkey, s.vkey),
            (NoteMode::Switch, true, 70, 10)
        );
        assert_eq!((s.ahead_ms, s.out_db), (20.0, -3.0));
    }

    /// At its defaults the unit touches no sample, to the bit.
    #[test]
    fn the_defaults_touch_no_sample() {
        let mut u = Unit::new(48_000.0);
        u.configure(&EffectSpec::new(EffectKind::TimeShaper));
        let x: Vec<f32> = (0..64).map(|i| (i as f32 * 0.37).fract() - 0.5).collect();
        let (mut l, mut r) = (x.clone(), x.clone());
        u.process(&mut l, &mut r, None);
        assert!(l
            .iter()
            .chain(&r)
            .zip(x.iter().chain(&x))
            .all(|(a, b)| a.to_bits() == b.to_bits()));
        assert_eq!(u.latency(), 0);
    }

    #[test]
    fn the_factory_banks_are_loaded_and_a_banks_own_slot_replaces_it() {
        let mut u = Unit::new(48_000.0);
        for l in model::Lane::ALL {
            for (slot, p) in model::factory(l).iter().enumerate() {
                assert_eq!(
                    u.dsp().pattern(lane(l), slot),
                    &pattern(l, p),
                    "{l:?} {slot}"
                );
            }
        }
        let mut spec = EffectSpec::new(EffectKind::TimeShaper);
        let mine = model::Pattern {
            len: 0,
            points: vec![model::Point::new(0.0, 0.0), model::Point::new(1.0, 0.5)],
            name: None,
        };
        let mut bank = model::Bank::default();
        bank.vol.insert(3, mine.clone());
        spec.lapsus = Some(Box::new(bank));
        u.configure(&spec);
        let (t, v) = (model::Lane::Time, model::Lane::Vol);
        assert_eq!(u.dsp().pattern(Lane::Vol, 3), &pattern(v, &mine));
        assert_eq!(
            u.dsp().pattern(Lane::Time, 3),
            &pattern(t, &model::factory(t)[3]),
            "the other bank's slot 4 stays"
        );
        // Back to the factory's when the bank lets the slot go.
        u.configure(&EffectSpec::new(EffectKind::TimeShaper));
        assert_eq!(
            u.dsp().pattern(Lane::Vol, 3),
            &pattern(v, &model::factory(v)[3])
        );
    }

    #[test]
    fn its_history_comes_by_the_memory_path() {
        let mut u = Unit::new(48_000.0);
        u.configure(&EffectSpec::new(EffectKind::TimeShaper));
        let Some(MemoryNeed::Lapsus(n)) = u.memory_need() else {
            panic!("a new unit asks for its history");
        };
        assert!(n >= 4 * 48_000, "a bar at 60 BPM");
        let back = u.supply(EffectMemory::build(MemoryNeed::Lapsus(n), 48_000.0));
        assert!(matches!(back, Some(EffectMemory::Lapsus(_))));
        assert!(u.memory_need().is_none());
    }
}
