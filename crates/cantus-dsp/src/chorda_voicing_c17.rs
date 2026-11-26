//! Chorda's candidate 17 (`DSP_VERSION` 16, round 42 `chorda-c17`, ADR 0098's sixth
//! amendment, `docs/round42/chorda-c17.md`): what ships over the unison pass of
//! `DSP_VERSION` 11 after the owner's blind test of candidate 16 (2026-11-25: told from the
//! Salamander 16/16). [`SPEC`] (the voicing lab's format, applied after the unison pass's)
//! gives the action's knock the case's ring a recorded grand's has — its noise in the band
//! the two recorded grands measure, a second, slower stage, the keybed's thud shorter — and
//! voices middle C's third partial (785 Hz, the nasal mid) and its neighbours' as both grands
//! keep them, and six top keys whose second or third partial stood 10–30 dB over both grands
//! (G6, A♯6, B6, C♯7, D♯7, E7), as a technician voices a hammer. Fitted by
//! `tools/chorda-voicing/probes/c17/ksearch.py` against the Salamander and the bitKlavier
//! Grand, scored on held-out keys; the analysis that followed (`probes/c17/report17.py`) set
//! the knock's low ring shorter and higher, the dampers' hold after the pedal (the pedalled
//! phrases' tails 5–10 dB short) and the soft strikes' knock (the run's 60 Hz).

/// The pass's spec, over the unison pass's (`DSP_VERSION` 11's).
pub const SPEC: &str = concat!(
    "partial3.59=-8.3,partial3.60=-19.5,partial3.61=-16.8,partial6.60=-10,key_return=1,",
    "knock.noise=300.42,knock.noise_treble=31.623,knock.tail=0.63,knock.tail_tau=0.16,",
    "knock.tau=0.0020751,knock.lo=90,knock.hi=1659,knock.noise_vel=-0.375,",
    "knock_treble=0.0258,knock.t60=1.2,partial2.99=-25,partial2.97=-25,partial2.95=-20,",
    "partial2.91=-15,partial2.90=-10,partial2.100=-15,partial3.97=-30,",
    "damper_t60=1.6,knock_vel=0.8,landing.hi=500",
);
