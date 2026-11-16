//! Arca, amp and cabinet (ADR 0137, provisional), behind the engine's plug-in contract.
//! Owned by the arca builder.
//!
//! [`settings`] reads the table (`cantus_core::fx::arca`) into the DSP's settings
//! (`cantus_dsp::arca`). **The cabinet** is an impulse response the unit asks for by value
//! ([`ArcaNeed`]: which cabinet, the microphone, its place — or the loaded file's key) and
//! the interface's thread makes ([`build`]: one of Arca's own generated responses, or the
//! user's file from Aula's store, cut to half a second); it is swapped in whole, the one
//! playing goes on meanwhile. An offline render builds it at once. **The latency** is the
//! oversampler's while an amp is chosen (4×: 55 samples at 48 kHz), none for the cabinet
//! alone, reported to delay compensation.
//!
//! ## Telemetry
//!
//! | Index | What |
//! |---|---|
//! | 0 | sample rate, Hz |
//! | 1 | latency, samples |
//! | 2 | the amp (0 none, 1 … 5) |
//! | 3 | the supply's sag below idle, V |
//! | 4 | the output's peak, linear |
//! | 5 | 1 when the cabinet playing is the one the parameters ask for |
//! | 6 | the oversampling factor while an amp is chosen (2, 4, 8), else 0 |
//! | 7 | the preamp's circuits (a valve each), 0 … 5 |
//! | 8 … 12 | each circuit's swing over a third of its supply (≈ 1: it clips hard) |
//! | 13 | the phase inverter's input over its swing (1: where its limit bends) |
//! | 14 | the power valves' drive (0 idle, 1 where their grids conduct) |
//! | 15, 16 | the supply now and at idle, V |

use super::{FxUnit, Telemetry, TELEMETRY_LEN};
use crate::effects::{EffectMemory, MemoryNeed};
use cantus_core::EffectSpec;
use cantus_dsp::arca::cab::{impulse, CabSettings, Cabinet, Mic};
use cantus_dsp::arca::{AmpModel, Arca, ArcaSettings};
use cantus_dsp::aula_kernel::{Format, Kernel, Layout};
use std::sync::Arc;

pub const T_SAMPLE_RATE: usize = 0;
pub const T_LATENCY: usize = 1;
pub const T_MODEL: usize = 2;
pub const T_SAG: usize = 3;
pub const T_PEAK: usize = 4;
pub const T_READY: usize = 5;
pub const T_OS: usize = 6;
pub const T_STAGES: usize = 7;
pub const T_PRE: usize = 8;
pub const T_INVERTER: usize = 13;
pub const T_POWER: usize = 14;
pub const T_SUPPLY: usize = 15;
pub const T_IDLE: usize = 16;

/// The cabinet choice that plays the loaded file.
pub const CAB_FILE: u8 = 6;

/// A cabinet's response, by value (small: it travels in the need event as it is).
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct ArcaNeed {
    /// The loaded file's key ([`super::aula_store::file_key`]) when `cab` is the file.
    pub user: u64,
    /// The microphone's position (‰ of the way to the cone's edge) and distance (mm).
    pub pos: u16,
    pub dist: u16,
    /// 1 … 5 Arca's cabinets, [`CAB_FILE`] the loaded file.
    pub cab: u8,
    pub mic: u8,
}

/// What answers an [`ArcaNeed`]: the kernel (none when the file is not read), freed off the
/// audio thread when handed back.
#[derive(Debug)]
pub struct Memory {
    pub need: ArcaNeed,
    pub kernel: Option<Arc<Kernel>>,
}

/// The room every Arca has for a response: half a second (a cabinet's is 43 ms; a longer
/// file is cut there).
pub fn capacity(rate: f32) -> Layout {
    Layout::for_taps((0.5 * rate) as usize)
}

/// Build the response `need` asks for at `rate`. Allocates: off the audio thread.
pub fn build(need: ArcaNeed, rate: f32) -> Memory {
    let cap = capacity(rate);
    let kernel = if need.cab == CAB_FILE {
        super::aula_store::file_ir(need.user).map(|src| {
            let keep = (cap.taps() as f64 * src.rate as f64 / rate as f64) as usize;
            let mut ir = (*src).clone();
            for ch in &mut ir.channels {
                ch.truncate(keep);
            }
            let (_, k) = cantus_dsp::aula::prepare(
                &ir,
                rate,
                &cantus_dsp::aula_shape::Shape::default(),
                Some(cap),
            );
            Arc::new(k)
        })
    } else {
        let h = impulse(
            &CabSettings {
                cabinet: Cabinet::from_index(need.cab.saturating_sub(1) as usize),
                mic: Mic::from_index(need.mic as usize),
                position: need.pos as f32 / 1_000.0,
                distance_cm: need.dist as f32 / 10.0,
            },
            rate,
        );
        Some(Arc::new(Kernel::new(
            Format::Mono,
            std::slice::from_ref(&h),
            cap,
        )))
    };
    // Kept here too, so the audio thread never drops a response's last reference (a swap
    // lets go of the one before): the store frees those no unit holds, off that thread.
    if let Some(k) = &kernel {
        let mut pool = POOL.lock().unwrap_or_else(|e| e.into_inner());
        pool.retain(|(n, p)| Arc::strong_count(p) > 1 || *n == need);
        pool.push((need, k.clone()));
        if pool.len() > POOL_MAX {
            pool.remove(0);
        }
    }
    Memory { need, kernel }
}

/// The responses made lately ([`build`]), and how many are kept when no unit holds them.
static POOL: std::sync::Mutex<Vec<(ArcaNeed, Arc<Kernel>)>> = std::sync::Mutex::new(Vec::new());
const POOL_MAX: usize = 64;

/// Arca's parameters as the DSP wants them, and the cabinet they ask for (none: no cabinet).
pub fn settings(spec: &EffectSpec) -> (ArcaSettings, Option<ArcaNeed>) {
    let idx = |k: &str| spec.get(k).round().max(0.0) as usize;
    let model = match idx("model") {
        0 => None,
        i => Some(AmpModel::from_index(i - 1)),
    };
    let cab = idx("cab").min(CAB_FILE as usize) as u8;
    let need = match cab {
        0 => None,
        CAB_FILE => spec.ir.as_ref().map(|f| ArcaNeed {
            user: super::aula_store::file_key(&f.path),
            pos: 0,
            dist: 0,
            cab,
            mic: 0,
        }),
        _ => Some(ArcaNeed {
            user: 0,
            pos: (spec.get("micpos").clamp(0.0, 1.0) * 1_000.0).round() as u16,
            dist: (spec.get("micdist").clamp(1.0, 30.0) * 10.0).round() as u16,
            cab,
            mic: idx("mic").min(2) as u8,
        }),
    };
    let s = ArcaSettings {
        model,
        gain: spec.get("gain"),
        bass: spec.get("bass"),
        mid: spec.get("mid"),
        treble: spec.get("treble"),
        presence: spec.get("presence"),
        master: spec.get("master"),
        sag: spec.get("sag"),
        cab: need.is_some(),
        oversampling: [2, 4, 8][idx("os").min(2)],
        mix: spec.get("mix"),
        out_db: spec.get("out"),
    };
    (s, need)
}

pub struct Unit {
    dsp: Arca,
    want: Option<ArcaNeed>,
    have: Option<ArcaNeed>,
    set: ArcaSettings,
}

impl Clone for Unit {
    fn clone(&self) -> Self {
        // A fork plays the same response (the kernel shared, the state its own).
        let mut u = Unit::new(self.dsp.sample_rate());
        u.want = self.want;
        u.have = self.have;
        if let Some(k) = self.dsp.cab_kernel() {
            let _ = u.dsp.supply_cab(k, None);
        }
        u.set = self.set;
        u.apply();
        u
    }
}

impl std::fmt::Debug for Unit {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "arca::Unit({:?})", self.set)
    }
}

impl Unit {
    /// Allocates (off the audio thread): the circuits' room and the cabinet's.
    pub fn new(sample_rate: f32) -> Self {
        let mut dsp = Arca::with_room(sample_rate, capacity(sample_rate));
        // The silence it starts with is the store's (never freed on the audio thread).
        let _ = dsp.supply_cab(super::aula_store::silent(capacity(sample_rate)), None);
        Unit {
            set: *dsp.settings(),
            dsp,
            want: None,
            have: None,
        }
    }

    /// The settings, the cabinet on once a response has arrived (the one asked for, or the
    /// one before it while the new one is made).
    fn apply(&mut self) {
        let mut s = self.set;
        s.cab = self.want.is_some() && self.have.is_some() && self.dsp.cab_kernel().is_some();
        self.dsp.set(&s);
    }
}

impl FxUnit for Unit {
    fn fork(&self) -> Option<Box<dyn FxUnit>> {
        Some(Box::new(self.clone()))
    }

    fn configure(&mut self, spec: &EffectSpec) {
        let (s, need) = settings(spec);
        self.set = s;
        self.want = need;
        self.apply();
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
        let (sag, peak) = self.dsp.meters();
        t[T_SAMPLE_RATE] = self.dsp.sample_rate();
        t[T_LATENCY] = self.dsp.latency() as f32;
        t[T_MODEL] = self.set.model.map_or(0.0, |m| (m.index() + 1) as f32);
        t[T_SAG] = sag;
        t[T_PEAK] = peak;
        t[T_READY] = if self.want.is_none() || self.have == self.want {
            1.0
        } else {
            0.0
        };
        let w = self.dsp.work();
        if self.set.model.is_some() {
            t[T_OS] = cantus_dsp::arca::factor(self.set.oversampling) as f32;
        }
        t[T_STAGES] = w.stages as f32;
        t[T_PRE..T_PRE + w.pre.len()].copy_from_slice(&w.pre);
        t[T_INVERTER] = w.inverter;
        t[T_POWER] = w.power;
        t[T_SUPPLY] = w.supply;
        t[T_IDLE] = w.idle;
        t
    }

    fn memory_need(&self) -> Option<MemoryNeed> {
        match self.want {
            Some(w) if self.have != Some(w) => Some(MemoryNeed::Arca(w)),
            _ => None,
        }
    }

    fn supply(&mut self, memory: EffectMemory) -> Option<EffectMemory> {
        let EffectMemory::Arca(mut m) = memory else {
            return Some(memory);
        };
        let Some(kernel) = m.kernel.take() else {
            // The file is not read: nothing to play; asked again with the next snapshot.
            return Some(EffectMemory::Arca(m));
        };
        match self.dsp.supply_cab(kernel, None) {
            Ok(_) => self.have = Some(m.need),
            Err((k, _)) => m.kernel = Some(k),
        }
        self.apply();
        Some(EffectMemory::Arca(m))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cantus_core::EffectKind;

    /// At its defaults (no amp, no cabinet) the unit touches no sample, to the bit, and
    /// asks for nothing.
    #[test]
    fn the_defaults_touch_no_sample() {
        let mut u = Unit::new(48_000.0);
        u.configure(&EffectSpec::new(EffectKind::Amp));
        assert!(u.memory_need().is_none());
        let x: Vec<f32> = (0..256).map(|i| (i as f32 * 0.37).fract() - 0.5).collect();
        let (mut l, mut r) = (x.clone(), x.clone());
        u.process(&mut l, &mut r, None);
        assert!(l
            .iter()
            .chain(&r)
            .zip(x.iter().chain(&x))
            .all(|(a, b)| a.to_bits() == b.to_bits()));
        assert_eq!(u.latency(), 0);
    }

    /// A cabinet is asked for, built, supplied, and then plays.
    #[test]
    fn a_cabinet_is_asked_for_and_supplied() {
        let mut spec = EffectSpec::new(EffectKind::Amp);
        spec.set("cab", 3.0);
        let mut u = Unit::new(48_000.0);
        u.configure(&spec);
        let Some(MemoryNeed::Arca(n)) = u.memory_need() else {
            panic!("no need");
        };
        assert_eq!(n.cab, 3);
        let back = u.supply(EffectMemory::Arca(Box::new(build(n, 48_000.0))));
        assert!(matches!(back, Some(EffectMemory::Arca(_))));
        assert!(u.memory_need().is_none());
        let mut l = vec![0.0f32; 512];
        l[0] = 1.0;
        let mut r = l.clone();
        u.process(&mut l, &mut r, None);
        assert!(l[0] != 0.0 && l[1..].iter().any(|v| *v != 0.0));
    }

    /// The display's meters: at the defaults all zero; a driven crunch amp shows its valves
    /// working, harder as the gain goes up, its supply sagging below idle.
    #[test]
    fn the_valves_work_harder_as_the_gain_goes_up() {
        let run = |gain: f32| {
            let mut u = Unit::new(48_000.0);
            let mut spec = EffectSpec::new(EffectKind::Amp);
            spec.set("model", 2.0);
            spec.set("gain", gain);
            u.configure(&spec);
            let mut t = [0.0; TELEMETRY_LEN];
            for b in 0..40 {
                let mut l: Vec<f32> = (0..480)
                    .map(|i| 0.3 * cantus_dsp::math::sin((b * 480 + i) as f32 * 0.0367))
                    .collect();
                let mut r = l.clone();
                u.process(&mut l, &mut r, None);
                t = u.telemetry();
            }
            t
        };
        let (lo, hi) = (run(0.2), run(0.9));
        assert_eq!(hi[T_OS], 4.0);
        assert_eq!(hi[T_STAGES], lo[T_STAGES]);
        let n = hi[T_STAGES] as usize;
        assert!((2..=5).contains(&n), "{n}");
        let last = |t: &Telemetry| t[T_PRE + n - 1];
        assert!(
            last(&hi) > last(&lo) && last(&lo) > 0.0,
            "{} {}",
            last(&lo),
            last(&hi)
        );
        assert!(hi[T_POWER] > 0.0 && hi[T_INVERTER] > 0.0);
        assert!(hi[T_IDLE] > 300.0 && hi[T_SUPPLY] < hi[T_IDLE]);
        let mut u = Unit::new(48_000.0);
        u.configure(&EffectSpec::new(EffectKind::Amp));
        let (mut l, mut r) = (vec![0.5f32; 256], vec![0.5f32; 256]);
        u.process(&mut l, &mut r, None);
        assert!(u.telemetry()[T_OS..].iter().all(|v| *v == 0.0));
    }
}
