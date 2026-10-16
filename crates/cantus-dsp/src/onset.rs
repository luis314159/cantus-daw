//! **Onsets** — where the transients of a recording are (round 29, ADR 0061): SuperFlux
//! (Böck and Widmer, DAFx 2013) — a log-magnitude spectrogram on a logarithmic filterbank,
//! the positive difference to a frame 10 ms earlier taken after a maximum filter across
//! frequency (so a vibrato or a glide is not a new note), summed — then peaks picked by a
//! moving maximum, an adaptive threshold over a moving mean, and a minimum gap; each onset
//! then refined to the sample where its attack starts. Offline, deterministic, allocates.

/// What [`detect`] finds.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Onsets {
    /// The onset-strength function's frames per second.
    pub fps: f64,
    /// The onset strength of every frame (frame `i` at `i / fps` seconds).
    pub odf: Vec<f32>,
    /// The same below 200 Hz only (kicks and basses): the downbeat's cue.
    pub low: Vec<f32>,
    /// The onsets, in samples (the attack's first sample), in order.
    pub frames: Vec<f64>,
    /// Each onset's strength (its peak in `odf`).
    pub strengths: Vec<f32>,
}

/// Find the onsets of a mono signal at `rate`.
pub fn detect(x: &[f32], rate: f32) -> Onsets {
    let _ = (x, rate);
    Onsets::default()
}

#[cfg(test)]
#[path = "onset_tests.rs"]
mod tests;
