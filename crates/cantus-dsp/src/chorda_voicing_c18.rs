//! Chorda's candidate 18 (`DSP_VERSION` 18; round 42
//! `chorda-c18`, ADR 0098's seventh amendment, `docs/round42/chorda-c18.md`): what ships over
//! candidate 17 after the owner's blind test of it (2026-11-26: the top's knock "a knock with
//! reverb", not the Salamander's "hammer and mechanism"; something in the two lowest notes;
//! middle C's "give-away mid"). [`SPEC`] (the voicing lab's format, applied after
//! candidate 17's):
//!
//! - **the top keys click dry**: from C6 to C7 and above the knock's first stage grows to a
//!   15 ms burst rising over its first quarter, its band to 2.7 kHz, and the slow stage that
//!   rang 1.1 s for 60 dB three quarters as loud and long and under 1.5 kHz — the two
//!   recorded grands' top keys burst 5–30 ms after the strike at 0.6–3 kHz and fall
//!   110–140 dB/s, where candidate 17's fell 55–65 (a ring a listener hears as a room);
//! - **the bass's knock and blow under its strings'**: at A0 three tenths of the middle's (log
//!   to C4) — a bass string's tone reaches the bridge 10–15 ms after its hammer, and
//!   candidate 17's knock stood before it as a thump of its own (A0–D♯1 rising in 21–34 ms
//!   against the grands' 5–13);
//! - **middle C's fifth partial** (1.31 kHz) voiced down: in the middle C phrase it stood
//!   5–10 dB over both grands' span in the sustain.
//!
//! Measured with `tools/chorda-voicing/probes/c18/` against the Salamander and the bitKlavier.

/// The pass's spec, over candidate 17's.
pub const SPEC: &str = concat!(
    "knock.tau_treble=7.2,knock.hi_treble=1.6,knock.noise_treble=38,knock.rise_treble=0.25,",
    "knock.tail_treble=0.75,knock.tail_tau_treble=0.85,knock.tail_hi_treble=1500,",
    "knock_bass=0.3,blow_bass=0.3,partial5.60=-10",
);
