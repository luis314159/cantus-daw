//! What a modulator outputs, as functions the engine and *Bake to automation* share
//! (ADR 0057).
//!
//! **Functions of time, not running sums.** An LFO, a step sequence and a random source
//! are pure functions of [`ModTime`]: a synced one of the **song position** (a render from
//! bar 5 hears what a full render hears from bar 5), a free one of the **time since
//! playback started** (deterministic from the start of playback). An envelope opened by
//! the sequenced notes is a closed form over the note gates ([`Gates`]), so it too is a
//! function of the song position. Only what follows *audio* or *live* notes carries state
//! ([`Follower`], [`LiveEnv`]): it is stepped on the engine's control grid, restarted when
//! playback starts.

use super::{EnvSource, LfoShape, ModRate, ModSource, Retrigger};
use crate::snapshot::NoteEvent;
use cantus_dsp::math;
use cantus_dsp::modfx_clock::{division_beats, random_bipolar};
use cantus_dsp::modfx_lfo::{wave, Shape as Wave};

/// When a modulator is asked.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct ModTime<'a> {
    /// The song position, in beats.
    pub beats: f64,
    /// Seconds since playback started.
    pub since_play: f64,
    /// Seconds per beat at the tempo there.
    pub spb: f64,
    /// The sequenced notes are playing (stopped, only live notes open an envelope).
    pub playing: bool,
    /// The tempo map when the tempo changes along the song (ADR 0060): beats become
    /// seconds through it (an envelope's gates, a retriggered LFO in Hz). `None` at one
    /// tempo — `spb` is exact then.
    pub map: Option<&'a crate::tempo::TempoMap>,
}

impl ModTime<'static> {
    /// Where the song is at `beats`, with playback started at `play_beats`, at `bpm`.
    pub fn at_beats(beats: f64, play_beats: f64, bpm: f64) -> ModTime<'static> {
        let spb = 60.0 / bpm.max(1e-9);
        ModTime {
            beats,
            since_play: ((beats - play_beats) * spb).max(0.0),
            spb,
            playing: true,
            map: None,
        }
    }
}

impl<'a> ModTime<'a> {
    /// Where the song is at `beats` through `map`, playback started at `play_beats`: at
    /// one tempo exactly [`ModTime::at_beats`].
    pub fn at_beats_in(beats: f64, play_beats: f64, map: &'a crate::tempo::TempoMap) -> Self {
        if map.is_constant() {
            return ModTime::at_beats(beats, play_beats, map.bpm_at(0.0));
        }
        let ppq = crate::PPQ as f64;
        ModTime {
            beats,
            since_play: map
                .seconds_between_f64(play_beats * ppq, beats * ppq)
                .max(0.0),
            spb: 60.0 / map.bpm_at(beats * ppq).max(1e-9),
            playing: true,
            map: Some(map),
        }
    }

    /// The map when the tempo changes along it.
    #[inline]
    fn moving_map(&self) -> Option<&'a crate::tempo::TempoMap> {
        self.map.filter(|m| !m.is_constant())
    }

    /// Seconds from beat `from` to this time, never negative: through the map when the
    /// tempo changes, at `spb` otherwise.
    #[inline]
    pub fn seconds_since(&self, from: f64) -> f64 {
        match self.moving_map() {
            Some(m) => {
                let ppq = crate::PPQ as f64;
                m.seconds_between_f64(from * ppq, self.beats * ppq).max(0.0)
            }
            None => (self.beats - from).max(0.0) * self.spb,
        }
    }
}

/// The intervals a track's notes hold an envelope open, in beats: every note-on opens a
/// new gate (the envelope retriggers from where it is), and the gate closes when the last
/// held note is released.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Gates {
    pub gates: Vec<Gate>,
    /// The level each gate starts from, for one envelope at one tempo
    /// ([`Gates::with_levels`], done when a modulator is compiled): every lookup is then
    /// exact and O(log n). Without them a lookup walks forward from the first gate.
    pub levels: Option<Box<Levels>>,
}

/// What [`Gates::levels`] was computed for, and the levels.
#[derive(Clone, Debug, PartialEq)]
pub struct Levels {
    pub env: EnvSource,
    pub spb: f64,
    pub from: Vec<f32>,
    /// Under a tempo map that changes (ADR 0060): the gates in song seconds, which the
    /// levels were computed on.
    pub secs: Option<Vec<Gate>>,
}

#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Gate {
    /// The note-on.
    pub on: f64,
    /// Where the gate closes (`f64::INFINITY`: another note-on comes first, or never).
    pub off: f64,
}

impl Gates {
    /// The gates of a track's events (sorted by tick, as a snapshot holds them).
    pub fn of_events(events: &[NoteEvent]) -> Gates {
        let beats = |tick: i64| tick as f64 / crate::PPQ as f64;
        let mut gates: Vec<Gate> = Vec::new();
        let mut held = 0u32;
        for ev in events {
            if ev.is_on() {
                held += 1;
                gates.push(Gate {
                    on: beats(ev.tick.0),
                    off: f64::INFINITY,
                });
            } else {
                held = held.saturating_sub(1);
                if held == 0 {
                    if let Some(g) = gates.last_mut() {
                        if g.off.is_infinite() {
                            g.off = beats(ev.tick.0).max(g.on);
                        }
                    }
                }
            }
        }
        Gates {
            gates,
            levels: None,
        }
    }

    /// These gates with the level each one starts from for `env` at `spb` seconds per
    /// beat, found in one forward pass: what the engine reads on the audio thread.
    pub fn with_levels(&self, env: &EnvSource, spb: f64) -> Gates {
        let mut from = Vec::with_capacity(self.gates.len());
        let mut level = 0f32;
        for i in 0..self.gates.len() {
            from.push(level);
            if let Some(next) = self.gates.get(i + 1) {
                level = self.level_from(env, level, i, next.on, spb);
            }
        }
        Gates {
            gates: self.gates.clone(),
            levels: Some(Box::new(Levels {
                env: env.clone(),
                spb,
                from,
                secs: None,
            })),
        }
    }

    /// [`Gates::with_levels`] through a tempo map (ADR 0060): at one tempo exactly that;
    /// when the tempo changes, every gate in song seconds and the levels computed on them.
    pub fn with_levels_in(&self, env: &EnvSource, map: &crate::tempo::TempoMap) -> Gates {
        if map.is_constant() {
            return self.with_levels(env, 60.0 / map.bpm_at(0.0).max(1e-9));
        }
        let ppq = crate::PPQ as f64;
        let sec = |b: f64| {
            if b.is_finite() {
                map.seconds_at(b * ppq)
            } else {
                b
            }
        };
        let secs: Vec<Gate> = self
            .gates
            .iter()
            .map(|g| Gate {
                on: sec(g.on),
                off: sec(g.off),
            })
            .collect();
        let mut from = Vec::with_capacity(secs.len());
        let mut level = 0f32;
        for i in 0..secs.len() {
            from.push(level);
            if let Some(next) = secs.get(i + 1) {
                level = secs_level(env, level, secs[i], next.on);
            }
        }
        Gates {
            gates: self.gates.clone(),
            levels: Some(Box::new(Levels {
                env: env.clone(),
                spb: 0.0,
                from,
                secs: Some(secs),
            })),
        }
    }

    /// The envelope's level at `t`: through the gates in seconds when they were computed
    /// for this envelope under the moving tempo map `t` carries; otherwise
    /// [`Gates::level`].
    pub fn level_at(&self, env: &EnvSource, t: &ModTime) -> f32 {
        if let (Some(map), Some(l)) = (t.moving_map(), &self.levels) {
            if let Some(secs) = l.secs.as_ref().filter(|s| s.len() == self.gates.len()) {
                if l.env == *env && l.from.len() == self.gates.len() {
                    let Some(i) = self.at(t.beats) else {
                        return 0.0;
                    };
                    let now = map.seconds_at(t.beats * crate::PPQ as f64);
                    return secs_level(env, l.from[i], secs[i], now);
                }
            }
        }
        self.level(env, t.beats, t.spb)
    }

    /// The gate playing at beat `t`: the last one opened at or before it.
    #[inline]
    pub fn at(&self, t: f64) -> Option<usize> {
        self.gates.partition_point(|g| g.on <= t).checked_sub(1)
    }

    /// The beat of the last note-on at or before `t`.
    #[inline]
    pub fn last_on(&self, t: f64) -> Option<f64> {
        self.at(t).map(|i| self.gates[i].on)
    }

    /// The envelope's level at beat `t` (0 before the first note), `spb` seconds per
    /// beat: a closed form, so it is the same whatever came before and wherever a render
    /// starts. Each gate starts from the level the previous one left (a retrigger never
    /// jumps): read from [`Gates::levels`] when they were computed for this envelope and
    /// tempo, else found by walking forward over every earlier gate.
    pub fn level(&self, env: &EnvSource, t: f64, spb: f64) -> f32 {
        let Some(i) = self.at(t) else {
            return 0.0;
        };
        let from = match &self.levels {
            Some(l) if l.spb == spb && l.env == *env && l.from.len() == self.gates.len() => {
                l.from[i]
            }
            _ => {
                let mut level = 0f32;
                for k in 0..i {
                    level = self.level_from(env, level, k, self.gates[k + 1].on, spb);
                }
                level
            }
        };
        self.level_from(env, from, i, t, spb)
    }

    /// Gate `i`'s level at beat `t`, started from `from`.
    fn level_from(&self, env: &EnvSource, from: f32, i: usize, t: f64, spb: f64) -> f32 {
        let g = self.gates[i];
        let secs = Gate {
            on: 0.0,
            off: (g.off - g.on) * spb,
        };
        gate_level(env, from, secs, (t - g.on) * spb)
    }
}

/// A gate's level at song second `now`, the gate `g` in song seconds, started from
/// `from`.
fn secs_level(env: &EnvSource, from: f32, g: Gate, now: f64) -> f32 {
    let secs = Gate {
        on: 0.0,
        off: g.off - g.on,
    };
    gate_level(env, from, secs, now - g.on)
}

/// An ADSR's level `t` seconds into gate `g` (in seconds, `g.on` its start) that started
/// from level `from`: a linear attack to 1 (from where it was), a linear decay to the
/// sustain, a linear release from wherever the gate closed to 0.
pub fn gate_level(env: &EnvSource, from: f32, g: Gate, t: f64) -> f32 {
    if t < g.off {
        held_level(env, from, t - g.on)
    } else {
        let at_off = held_level(env, from, g.off - g.on);
        let r = f64::from(env.release_ms) * 1e-3;
        if r <= 0.0 {
            return 0.0;
        }
        let u = ((t - g.off) / r).clamp(0.0, 1.0);
        (f64::from(at_off) * (1.0 - u)) as f32
    }
}

/// The attack, decay and sustain `x` seconds after a gate opened at level `from`.
fn held_level(env: &EnvSource, from: f32, x: f64) -> f32 {
    let x = x.max(0.0);
    let a = f64::from(env.attack_ms) * 1e-3;
    let from = f64::from(from.clamp(0.0, 1.0));
    let ta = if a > 0.0 { (1.0 - from) * a } else { 0.0 };
    if x < ta {
        return (from + x / a) as f32;
    }
    let y = x - ta;
    let d = f64::from(env.decay_ms) * 1e-3;
    let s = f64::from(env.sustain.clamp(0.0, 1.0));
    if d > 0.0 && y < d {
        (1.0 + (s - 1.0) * y / d) as f32
    } else {
        s as f32
    }
}

fn shape(s: LfoShape) -> Wave {
    match s {
        LfoShape::Sine => Wave::Sine,
        LfoShape::Triangle => Wave::Triangle,
        LfoShape::Square => Wave::Square,
        LfoShape::SawUp => Wave::SawUp,
        LfoShape::SawDown => Wave::SawDown,
    }
}

/// Cycles run at `t` by a rate (synced: beats over the division; free: seconds since
/// playback times Hz), counted from beat `since` (a retrigger) when given.
fn cycles(rate: ModRate, t: &ModTime, since: Option<f64>) -> f64 {
    match rate {
        ModRate::Sync { division } => {
            let beats = match since {
                Some(b) => (t.beats - b).max(0.0),
                None => t.beats,
            };
            beats / division_beats(f32::from(division))
        }
        ModRate::Hz { hz } => {
            let secs = match since {
                Some(b) => t.seconds_since(b),
                None => t.since_play,
            };
            secs * f64::from(hz.clamp(ModRate::MIN_HZ, ModRate::MAX_HZ))
        }
    }
}

/// The output of a source that is a function of time (−1…1 bipolar, 0…1 unipolar), at
/// `t`, with the sequenced notes' gates when it hears notes. `live_on` is the beat of the
/// last live note-on, for an LFO retriggered by notes. A follower or an audio envelope
/// answers 0 here: they are the engine's (stateful).
pub fn output(src: &ModSource, t: &ModTime, gates: Option<&Gates>, live_on: Option<f64>) -> f32 {
    let polar = |v: f32, unipolar: bool| {
        if unipolar {
            0.5 * (v + 1.0)
        } else {
            v
        }
    };
    match src {
        ModSource::Lfo(l) => {
            let since = match l.retrigger {
                Retrigger::Song => None,
                Retrigger::Note => {
                    let seq = gates.filter(|_| t.playing).and_then(|g| g.last_on(t.beats));
                    // A live note-on ahead of the song (left from before a jump) is none.
                    let live_on = live_on.filter(|b| *b <= t.beats);
                    match (seq, live_on) {
                        (Some(a), Some(b)) => Some(a.max(b)),
                        (a, b) => a.or(b),
                    }
                }
            };
            let c = cycles(l.rate, t, since) + f64::from(l.phase);
            let v = wave(shape(l.shape), c - c.floor(), c.floor() as i64, 0.0, 0);
            polar(v, l.unipolar)
        }
        ModSource::Step(s) => {
            let n = s.steps.len().max(1);
            let pos = t.beats / division_beats(f32::from(s.division));
            let i = pos.floor();
            let frac = (pos - i) as f32;
            let k = (i as i64).rem_euclid(n as i64) as usize;
            let a = s.steps.get(k).copied().unwrap_or(0.0);
            let g = s.glide.clamp(0.0, 1.0);
            let v = if g > 0.0 && frac > 1.0 - g {
                let b = s.steps.get((k + 1) % n).copied().unwrap_or(0.0);
                let u = (frac - (1.0 - g)) / g;
                a + (b - a) * smooth(u)
            } else {
                a
            };
            polar(v, s.unipolar)
        }
        ModSource::Random(r) => {
            let c = cycles(r.rate, t, None);
            let n = c.floor() as i64;
            let seed = u64::from(r.seed);
            let a = random_bipolar(seed, n);
            let v = if r.smooth {
                let b = random_bipolar(seed, n + 1);
                a + (b - a) * smooth((c - c.floor()) as f32)
            } else {
                a
            };
            polar(v, r.unipolar)
        }
        ModSource::Envelope(e) => match (&e.trigger, gates) {
            (super::EnvTrigger::Notes { .. }, Some(g)) if t.playing => g.level_at(e, t),
            _ => 0.0,
        },
        ModSource::Follower(_) => 0.0,
    }
}

/// A raised cosine from 0 to 1 over `u` in 0…1.
#[inline]
fn smooth(u: f32) -> f32 {
    0.5 - 0.5 * math::cos(std::f32::consts::PI * u.clamp(0.0, 1.0))
}

/// An envelope follower stepped on the control grid: the peak of each grid window in, a
/// level out (attack and release are the times to cover 63 % of a step).
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub struct Follower {
    pub level: f32,
}

impl Follower {
    /// One grid window of `dt` seconds whose peak was `peak` (linear).
    pub fn step(&mut self, peak: f32, attack_ms: f32, release_ms: f32, dt: f64) {
        let peak = if peak.is_finite() {
            peak.clamp(0.0, 16.0)
        } else {
            0.0
        };
        let ms = if peak > self.level {
            attack_ms
        } else {
            release_ms
        };
        let tau = f64::from(ms.max(0.0)) * 1e-3;
        let k = if tau <= 0.0 {
            1.0
        } else {
            1.0 - math::exp((-dt / tau) as f32)
        };
        self.level += (peak - self.level) * k;
        if !self.level.is_finite() || self.level < 1e-9 {
            self.level = 0.0;
        }
    }

    /// The output (0…1): the level, with the gain, clamped.
    pub fn output(&self, gain_db: f32) -> f32 {
        (self.level * crate::db_to_linear(gain_db)).clamp(0.0, 1.0)
    }
}

/// An ADSR stepped on the control grid, for what cannot be known ahead: live notes and an
/// audio threshold.
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub struct LiveEnv {
    pub level: f32,
    pub gate: bool,
    /// Attack until 1 is reached, then decay.
    attacking: bool,
    /// The release's slope (level per second), set when the gate closes.
    release_rate: f32,
}

impl LiveEnv {
    /// Open (`true`) or close the gate. `retrigger` opens it again while it is open (a new
    /// note): the attack starts again from where the level is.
    pub fn set_gate(&mut self, on: bool, env: &EnvSource, retrigger: bool) {
        if on && (!self.gate || retrigger) {
            self.attacking = true;
        }
        if !on && self.gate {
            let r = env.release_ms.max(0.0) * 1e-3;
            self.release_rate = if r > 0.0 {
                self.level / r
            } else {
                f32::INFINITY
            };
        }
        self.gate = on;
    }

    /// `dt` seconds on.
    pub fn step(&mut self, env: &EnvSource, dt: f64) {
        let dt = dt as f32;
        if self.gate {
            if self.attacking {
                let a = env.attack_ms.max(0.0) * 1e-3;
                self.level = if a > 0.0 { self.level + dt / a } else { 1.0 };
                if self.level >= 1.0 {
                    self.level = 1.0;
                    self.attacking = false;
                }
            } else {
                let s = env.sustain.clamp(0.0, 1.0);
                let d = env.decay_ms.max(0.0) * 1e-3;
                self.level = if d > 0.0 {
                    (self.level - (1.0 - s) * dt / d).max(s)
                } else {
                    s
                };
            }
        } else {
            self.level = (self.level - self.release_rate * dt).max(0.0);
        }
        if !self.level.is_finite() {
            self.level = 0.0;
        }
    }

    /// Back to rest.
    pub fn reset(&mut self) {
        *self = LiveEnv::default();
    }
}
