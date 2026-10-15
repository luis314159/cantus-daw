//! Chorda — the modelled grand piano (ADR 0059, BACKLOG §3e).
//!
//! **The model.** Every key is one, two or three **digital waveguide strings** (Smith,
//! *PASP*) struck by a **nonlinear, hysteretic felt hammer** (Chaigne & Askenfelt 1994;
//! Stulov 1995) solved against the strings with a fixed number of iterations. A string's
//! stiffness is a **dispersion filter** — first-order allpass sections fitted to the
//! stiff-string law `f_n = n F₀ √(1 + B n²)` at its partials — and its losses a
//! **linear-phase loss filter** whose decay follows the measured `σ = b₁ + b₂ (kπ/L)²` (so
//! changing a decay never moves a pitch). The strings of a key meet at a **yielding bridge**
//! (Weinreich 1977): slightly detuned, they beat, and their in-phase motion drains through
//! the bridge faster than their out-of-phase motion — the two-stage decay. The bridge's
//! force goes on to a **post-voice stage**: the keys' own strings ringing in sympathy
//! (freed by the sustain pedal, continuously, or by held keys), the **phantom partials**
//! of the strings' tension modulation, and the **soundboard**. Dampers fall when a key is
//! released (the top keys have none), with the pedal's position setting how firmly — half
//! pedalling is continuous; the soft pedal shifts the hammers off one string and onto
//! softer felt.
//!
//! **What comes from where.** The keyboard's numbers (inharmonicity, tuning, string
//! lengths, decay, hammers) are `tables`: closed-form physics and published measurements,
//! computed off the audio thread, never fitted to a soundboard (ADR 0059 §patents).

#[path = "chorda_tables.rs"]
pub mod tables;

#[cfg(test)]
#[path = "chorda_tests_tables.rs"]
mod tests_tables;

#[path = "chorda_design.rs"]
pub mod design;

#[cfg(test)]
#[path = "chorda_tests_design.rs"]
mod tests_design;

#[path = "chorda_string.rs"]
pub mod string;

#[cfg(test)]
#[path = "chorda_testkit.rs"]
mod testkit;

#[cfg(test)]
#[path = "chorda_tests_string.rs"]
mod tests_string;
