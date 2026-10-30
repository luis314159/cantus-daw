//! Beats, tempo, the timeline and the ghost clock, as the SDK reckons them — the same
//! roundings, so two implementations agree to the microbeat (ADR 0078 §3).
//!
//! - `link/Beats.hpp`: beats are a whole number of **microbeats** (`i64`), made from a
//!   float by rounding half away from zero (`llround`).
//! - `link/Tempo.hpp`: a tempo is kept in BPM but travels, and converts, as a whole number
//!   of **microseconds per beat**.
//! - `link/Timeline.hpp`: a timeline is a tempo, a beat origin and the time (µs) it falls
//!   at: `beats(t) = origin + (t − time origin) / µs per beat`.
//! - `link/GhostXForm.hpp`: ghost time — the session's shared clock — is host time through
//!   a line (slope 1, an intercept the measurement finds).
//! - `link/Phase.hpp`: phase within a quantum and the phase-matching arithmetic behind
//!   `requestBeatAtTime` / `forceBeatAtTime` (`Link.ipp`).

/// A count of beats, in microbeats.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Beats(i64);

/// `llround`: to the nearest, halves away from zero.
#[inline]
pub fn llround(x: f64) -> i64 {
    x.round() as i64
}

impl Beats {
    pub const ZERO: Beats = Beats(0);
    pub fn new(beats: f64) -> Beats {
        Beats(llround(beats * 1e6))
    }
    pub fn from_micro(micro: i64) -> Beats {
        Beats(micro)
    }
    pub fn micro(self) -> i64 {
        self.0
    }
    pub fn floating(self) -> f64 {
        self.0 as f64 / 1e6
    }
}

// Saturating: a peer's beats are not to be trusted to stay in range (review r33 #10).
impl std::ops::Add for Beats {
    type Output = Beats;
    fn add(self, o: Beats) -> Beats {
        Beats(self.0.saturating_add(o.0))
    }
}

impl std::ops::Sub for Beats {
    type Output = Beats;
    fn sub(self, o: Beats) -> Beats {
        Beats(self.0.saturating_sub(o.0))
    }
}

impl std::ops::Rem for Beats {
    type Output = Beats;
    /// C++'s `%` (the sign of the dividend), and 0 for a zero divisor, as `Beats::operator%`.
    fn rem(self, o: Beats) -> Beats {
        if o.0 == 0 {
            Beats(0)
        } else {
            Beats(self.0 % o.0)
        }
    }
}

/// A tempo, BPM.
#[derive(Copy, Clone, Debug, Default, PartialEq, PartialOrd)]
pub struct Tempo(f64);

impl Tempo {
    pub fn new(bpm: f64) -> Tempo {
        Tempo(bpm)
    }
    pub fn from_micros_per_beat(mpb: i64) -> Tempo {
        Tempo(60.0 * 1e6 / mpb as f64)
    }
    pub fn bpm(self) -> f64 {
        self.0
    }
    pub fn micros_per_beat(self) -> i64 {
        llround(60.0 * 1e6 / self.0)
    }
    pub fn micros_to_beats(self, micros: i64) -> Beats {
        Beats::new(micros as f64 / self.micros_per_beat() as f64)
    }
    pub fn beats_to_micros(self, beats: Beats) -> i64 {
        llround(beats.floating() * self.micros_per_beat() as f64)
    }
}

/// The slowest and fastest tempo a session takes (`clampTempo`).
pub const MIN_BPM: f64 = 20.0;
pub const MAX_BPM: f64 = 999.0;

/// A timeline: the tempo, and which beat falls at which time (µs; host or ghost time,
/// by context).
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Timeline {
    pub tempo: Tempo,
    pub beat_origin: Beats,
    pub time_origin: i64,
}

impl Default for Timeline {
    fn default() -> Self {
        Timeline {
            tempo: Tempo(120.0),
            beat_origin: Beats::ZERO,
            time_origin: 0,
        }
    }
}

impl Timeline {
    pub fn beats_at(&self, time: i64) -> Beats {
        self.beat_origin
            + self
                .tempo
                .micros_to_beats(time.saturating_sub(self.time_origin))
    }
    pub fn time_of(&self, beats: Beats) -> i64 {
        self.time_origin
            .saturating_add(self.tempo.beats_to_micros(beats - self.beat_origin))
    }
    /// The tempo held within 20–999 BPM.
    pub fn clamped(self) -> Timeline {
        Timeline {
            tempo: Tempo(self.tempo.0.clamp(MIN_BPM, MAX_BPM)),
            ..self
        }
    }
}

/// Ghost time from host time and back (`GhostXForm`).
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub struct GhostXForm {
    pub slope: f64,
    pub intercept: i64,
}

impl GhostXForm {
    /// A founder's: its ghost clock starts at zero now (`initXForm`).
    pub fn founder(host_now: i64) -> GhostXForm {
        GhostXForm {
            slope: 1.0,
            intercept: -host_now,
        }
    }
    pub fn host_to_ghost(&self, host: i64) -> i64 {
        llround(self.slope * host as f64).saturating_add(self.intercept)
    }
    pub fn ghost_to_host(&self, ghost: i64) -> i64 {
        llround(ghost.saturating_sub(self.intercept) as f64 / self.slope)
    }
}

/// Start/stop (Link 3): playing or not, from which beat, decided when (ghost µs).
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub struct StartStop {
    pub playing: bool,
    pub beats: Beats,
    pub timestamp: i64,
}

/// The phase of `beats` within `quantum` (`phase`).
pub fn phase(beats: Beats, quantum: Beats) -> Beats {
    if quantum == Beats::ZERO {
        return Beats::ZERO;
    }
    // The SDK's `(beats + bins·q) % q` with `bins` lifting it past zero is the Euclidean
    // remainder; computed as such, it cannot overflow on a hostile beat (`abs` of
    // `i64::MIN` did — review r33 #10).
    let q = quantum.micro();
    if q < 0 {
        return Beats::ZERO;
    }
    Beats(beats.micro().rem_euclid(q))
}

/// The first beat at or after `x` with the phase of `target` (`nextPhaseMatch`).
pub fn next_phase_match(x: Beats, target: Beats, quantum: Beats) -> Beats {
    let desired = phase(target, quantum);
    let xp = phase(x, quantum);
    let diff = (desired - xp + quantum) % quantum;
    x + diff
}

/// The beat nearest `x` with the phase of `target` (`closestPhaseMatch`).
pub fn closest_phase_match(x: Beats, target: Beats, quantum: Beats) -> Beats {
    next_phase_match(x - Beats::new(0.5 * quantum.floating()), target, quantum)
}

/// The beat a client shows at `time` (`toPhaseEncodedBeats`).
pub fn to_phase_encoded_beats(tl: &Timeline, time: i64, quantum: Beats) -> Beats {
    let beat = tl.beats_at(time);
    closest_phase_match(beat, beat - tl.beat_origin, quantum)
}

/// The time a client's `beat` falls at (`fromPhaseEncodedBeats`).
pub fn from_phase_encoded_beats(tl: &Timeline, beat: Beats, quantum: Beats) -> i64 {
    let from_origin = beat - tl.beat_origin;
    let origin_offset = from_origin - phase(from_origin, quantum);
    let inverse = closest_phase_match(
        quantum - phase(from_origin, quantum),
        quantum - phase(beat, quantum),
        quantum,
    );
    tl.time_of(tl.beat_origin + origin_offset + quantum - inverse)
}

/// Shift a timeline by `shift` beats (`shiftClientTimeline`).
pub fn shift_timeline(mut tl: Timeline, shift: Beats) -> Timeline {
    let delta = tl.time_of(shift).saturating_sub(tl.time_of(Beats::ZERO));
    tl.time_origin = tl.time_origin.saturating_sub(delta);
    tl
}

/// Make `beat` fall at `time` within the quantum's phase (`forceBeatAtTimeImpl`).
pub fn force_beat_at_time(tl: &mut Timeline, beat: Beats, time: i64, quantum: Beats) {
    let cur = to_phase_encoded_beats(tl, time, quantum);
    let closest = closest_phase_match(cur, beat, quantum);
    *tl = shift_timeline(*tl, closest - cur);
    tl.beat_origin = tl.beat_origin + beat - closest;
}

/// The session's timeline after a client's tempo change at `at` (ghost µs): a new beat
/// origin past the old one, so every peer adopts it (`updateSessionTimelineFromClient`,
/// specialised to a client whose timeline is the session's).
pub fn retempo(session: &Timeline, bpm: f64, at_ghost: i64) -> Timeline {
    let tempo = Tempo(bpm.clamp(MIN_BPM, MAX_BPM));
    if tempo == session.tempo {
        return *session;
    }
    let beat_now = session.beats_at(at_ghost);
    let through = Timeline {
        tempo,
        beat_origin: beat_now,
        time_origin: at_ghost,
    };
    let origin = beat_now.max(session.beat_origin + Beats(1));
    Timeline {
        tempo,
        beat_origin: origin,
        time_origin: through.time_of(origin),
    }
}
