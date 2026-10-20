//! The plug-in contract on the engine side: one [`FxUnit`] per plug-in, in a module of its
//! own, so a plug-in is owned by its files and the processor never learns its name.
//!
//! `EffectUnit` (`crate::effects`) holds a `Box<dyn FxUnit>` for every kind made here. The
//! box is built on the UI thread and handed to the audio thread, which only calls these
//! methods; none of them may allocate, lock or block (ADR 0005).

use cantus_core::{EffectKind, EffectSpec};

pub mod anulus;
pub mod concentus;
pub mod deesser;
pub mod discrimen;
pub mod flexus;
pub mod fluctus;
pub mod gate;
pub mod impetus;
pub mod limiter;
pub mod maximizer;
pub mod modfx;
pub mod multiband;
pub mod phasis;
pub mod saturator;
pub mod space;
pub mod speculum;
pub mod stereo;
pub mod tonsor;
pub mod tonus;
pub mod translatio;
pub mod tremor;
pub mod vibrans;

/// Numbers a plug-in reports to its editor each block: meters, gain reduction per band,
/// loudness — whatever its display needs. Their meaning is the plug-in's own; they travel
/// in `Meters::fx` for the effect whose editor is open.
pub const TELEMETRY_LEN: usize = 64;
pub type Telemetry = [f32; TELEMETRY_LEN];

/// A note an effect hears from the track its slot listens to
/// (`cantus_core::EffectSlot::midi_from`, ADR 0022).
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum FxNote {
    On {
        pitch: u8,
        velocity: u8,
    },
    Off {
        pitch: u8,
    },
    /// Every note stops: the transport stopped, sought or wrapped a loop.
    AllOff,
}

pub trait FxUnit: Send + std::fmt::Debug {
    /// New parameter values. Called on the audio thread with each snapshot: read the spec,
    /// set the DSP, never allocate. Changes should glide where a jump would click.
    fn configure(&mut self, spec: &EffectSpec);
    /// Process a stereo block in place; `key` is the sidechain when one is routed.
    fn process(&mut self, left: &mut [f32], right: &mut [f32], key: Option<(&[f32], &[f32])>);
    fn reset(&mut self);
    /// True when the unit is **at rest** (ADR 0031 §5.1). Exactly:
    ///
    /// 1. silent input (silent key, no note) gives exact zeros out and leaves it at rest;
    /// 2. **suspension is exact**: skipping those calls — telling the unit with
    ///    [`skip`](Self::skip) — changes no sample of what follows, whatever input and
    ///    whatever parameter changes come next;
    /// 3. **it is as good as new**: it renders exactly what a freshly reset unit with the same
    ///    configuration renders (that unit [`skip`](Self::skip)ped as far as this one has run,
    ///    so their counters agree) for any input, as long as the parameters stay as they
    ///    are. A unit whose memory outlives silence (a loudness match that holds its level)
    ///    may differ from a new one only after a parameter moves.
    ///
    /// **Exact, never a guess**: a unit that cannot prove it — a free-running LFO, a
    /// smoother still gliding, noise that advances on silence — says `false`. Cheap: the
    /// engine may ask once per block, so a unit keeps counters rather than scanning long
    /// lines. Default `false`: nothing is ever skipped until a unit opts in. Checked for
    /// every native kind by the twin tests (`tests/fx_contract.rs`).
    fn at_rest(&self) -> bool {
        false
    }
    /// Why the unit is not at rest, when it can say: the first condition that fails, by
    /// name — for the tests and a profiler that shows why a node keeps running. `None`
    /// exactly when [`Self::at_rest`] is true.
    fn rest_blocker(&self) -> Option<&'static str> {
        (!self.at_rest()).then_some("not at rest")
    }
    /// The engine did not call `process` for `frames` samples of silence (silent key, no
    /// note) because the unit was [at rest](Self::at_rest): advance whatever counts samples
    /// — a ring's write position, the grid a unit updates its coefficients on, a meter's
    /// columns — exactly as processing silence would have. Cheap (no audio is rendered), no
    /// allocation. Called only while at rest (a new unit is, when its kind can rest); it
    /// stays at rest. Default: nothing counts.
    fn skip(&mut self, frames: usize) {
        let _ = frames;
    }
    /// Samples of delay the unit adds, for delay compensation.
    fn latency(&self) -> usize {
        0
    }
    /// What the editor draws, refreshed each block.
    fn telemetry(&self) -> Telemetry {
        [0.0; TELEMETRY_LEN]
    }
    /// The gain reduction a dynamics unit applies now, dB (positive): what the master's
    /// reduction meter (`Meters::master_gr`, ADR 0026) shows. Default: none.
    fn gain_reduction_db(&self) -> Option<f32> {
        None
    }

    // ---- For units the engine did not build: hosted plug-ins (ADR 0019). Every one has
    // ---- a default, so a native unit ignores them.

    /// Where the song is, before each `process`: for a unit that follows tempo.
    fn set_position(&mut self, position: &SongPosition) {
        let _ = position;
    }
    /// What the unit's own editor did in the last `process` calls (a hosted plug-in's
    /// window moving a parameter), oldest first. The processor forwards each as
    /// `EngineEvent::FxParam` and then calls [`Self::clear_param_events`].
    fn param_events(&self) -> &[FxParamEvent] {
        &[]
    }
    fn clear_param_events(&mut self) {}
    /// Called on the audio thread just before the unit is handed back to be dropped:
    /// `process` is not called again. A hosted plug-in stops processing here.
    fn stop(&mut self) {}

    /// A note from the slot's MIDI source, `offset` samples into the next `process` call
    /// (always inside it): sample-accurate whatever the block size, already delayed by
    /// the host to line up with the audio arriving at this slot. Called on the audio
    /// thread before that `process`; must not allocate. Default: ignored.
    fn note(&mut self, offset: usize, note: FxNote) {
        let _ = (offset, note);
    }

    /// A hosted plug-in's parameter `id` driven by automation or modulators (ADR 0057),
    /// `offset` samples into the next `process` call (always inside it), in time order.
    /// Called on the audio thread before that `process`; must not allocate. A native unit
    /// is configured through its parameter set instead. Default: ignored.
    fn automate(&mut self, offset: usize, id: u32, value: HostedValue) {
        let _ = (offset, id, value);
    }

    /// The engine's automation was planned again (a new snapshot): from the next control
    /// point on, a parameter this unit is not sent [`Self::automate`] for is the project's
    /// own again. Default: ignored.
    fn automation_replanned(&mut self) {}

    /// Changes for the plug-in that found no room on the way (its event lists full) since
    /// the last time this was asked: the engine counts them in
    /// `EngineStats::dropped_events`. Default: none.
    fn take_dropped(&mut self) -> u64 {
        0
    }
}

/// What automation and modulation ask of a hosted parameter (ADR 0057), in the plug-in's
/// **normalised** domain (0…1 of its range): the automated position (`NaN`: nothing
/// automates it here, the project's own value is the base) and the modulators' sum, which
/// the unit adds to the base and clamps.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct HostedValue {
    pub pos: f32,
    pub modulation: f32,
}

impl HostedValue {
    /// Nothing drives the parameter here: the project's own value.
    pub const RELEASED: HostedValue = HostedValue {
        pos: f32::NAN,
        modulation: 0.0,
    };

    /// Whether it leaves the parameter to the project.
    pub fn is_released(&self) -> bool {
        !self.pos.is_finite() && self.modulation == 0.0
    }
}

/// A parameter change made by a unit's own editor, by the unit's parameter id: a gesture's
/// start, a value, a gesture's end. The host records them as commands, one undo step per
/// gesture.
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum FxParamEvent {
    Begin(u32),
    Value(u32, f64),
    End(u32),
}

/// The song position at the start of a block, for units that follow the host's tempo.
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub struct SongPosition {
    pub playing: bool,
    /// The tempo of this block: the tempo at the playhead, held for one step of a ramp
    /// (ADR 0060 §4: a ramp is told in steps of `RAMP_STEP` ticks, each starting a pass).
    pub tempo_bpm: f64,
    /// How fast the tempo moves inside a ramp, in BPM per sample (CLAP's `tempo_inc`);
    /// 0 while it holds.
    pub tempo_inc: f64,
    /// The tempo exactly at the block's first sample (inside a ramp, the ramp's value
    /// there, which `tempo_inc` moves on from): what a hosted plug-in is told (review 1).
    pub tempo_now: f64,
    /// Quarter notes from the song start.
    pub beats: f64,
    /// Samples from the song start at the current tempo: the beats as samples at
    /// `tempo_bpm` — what a synced phase counts (the modulation family's clock). Inside a
    /// ramp, counted on from the step's start, so it moves one per sample within a step.
    pub samples: i64,
    /// Seconds from the song start through the tempo map: the real time a host's
    /// `song_pos_seconds` and `projectTimeSamples` report.
    pub seconds: f64,
    /// Start of the current bar, in quarter notes.
    pub bar_start_beats: f64,
    /// The current bar's index from bar 1 (0-based), counted through every metre change.
    pub bar_number: i32,
    pub time_sig_num: u16,
    pub time_sig_den: u16,
    /// The loop, in quarter notes, when looping.
    pub loop_beats: Option<(f64, f64)>,
    /// The loop in song seconds (through the tempo map), when looping.
    pub loop_seconds: Option<(f64, f64)>,
    /// After a loop wrap (none since the start or a seek): the wraps the audio a late
    /// insert hears may lie behind. What [`Self::earlier`] folds by — exactly where the
    /// engine cuts its passes.
    pub wrap: Option<Wraps>,
}

/// How many of a loop's last passes [`Wraps`] keeps the exact lengths of: an insert later
/// than that many passes counts the ones before at the loop's length at the current tempo
/// (ADR 0057, as fix5 amended it).
pub const PASSES_KEPT: usize = 32;

/// A loop's wraps since the transport started or was moved, as the audio reaching a late
/// insert lies behind them (review of fix3: folded only after a wrap; review of fix4: no
/// further back than the wraps played, by the passes' real lengths; review of fix5: each
/// stretch ends where the playhead really was when it wrapped — the loop's end, or where a
/// forced wrap found it).
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Wraps {
    /// Samples played since the last wrap.
    pub since: u64,
    /// The pass playing, in samples: those played and those left to the loop's end
    /// (`u64::MAX` with no loop ahead: turned off or moved past the playhead).
    pub pass: u64,
    /// Wraps since the start or the seek (saturating): the stretch before the first is
    /// the song before the loop, played straight through.
    pub count: u32,
    /// The loop's length in samples at the current tempo, for passes older than the
    /// kept ones.
    pub loop_samples: f64,
    /// The loop's end, in quarter notes: where the passes older than the kept ones end.
    pub loop_end: f64,
    /// Where the stretch before the first wrap ended, in quarter notes.
    lead_end: f64,
    /// The last full passes, a ring with the newest at `head`: their lengths in samples
    /// and where each ended, in quarter notes.
    past: [u32; PASSES_KEPT],
    ends: [f64; PASSES_KEPT],
    head: u8,
}

impl Wraps {
    /// The first wrap, `since` samples ago, the playhead at `end` (quarter notes) when it
    /// wrapped; the pass playing is `pass` samples of a loop `loop_samples` long, ending
    /// at `loop_end`.
    pub fn first(since: u64, pass: u64, end: f64, loop_samples: f64, loop_end: f64) -> Wraps {
        Wraps {
            since,
            pass,
            count: 1,
            loop_samples,
            loop_end,
            lead_end: end,
            past: [0; PASSES_KEPT],
            ends: [0.0; PASSES_KEPT],
            head: 0,
        }
    }

    /// Another wrap now, the playhead at `end` (quarter notes), ending a pass `since`
    /// samples long.
    pub fn wrap(&mut self, end: f64) {
        self.head = ((self.head as usize + 1) % PASSES_KEPT) as u8;
        self.past[self.head as usize] = u32::try_from(self.since).unwrap_or(u32::MAX);
        self.ends[self.head as usize] = end;
        self.count = self.count.saturating_add(1);
        self.since = 0;
    }

    /// Full passes kept.
    fn kept(&self) -> u32 {
        (self.count - 1).min(PASSES_KEPT as u32)
    }

    /// The ring's index of the full pass `j` before the one playing (1: the last).
    fn at(&self, j: u32) -> usize {
        (self.head as usize + PASSES_KEPT - (j as usize - 1)) % PASSES_KEPT
    }

    /// Samples from the start of the passes older than the kept ones to `m` passes before
    /// it, at the loop's length now (the transport carries its sub-sample remainder, so
    /// the real wraps are within a sample of these while the tempo holds).
    fn older(&self, m: u64) -> u64 {
        (m as f64 * self.loop_samples.max(1.0)).round() as u64
    }

    /// Where the audio `back` samples before now lies: `None` in the pass playing (from
    /// the last wrap on), otherwise how many samples before the wrap that ends its stretch
    /// (1 … that stretch's length; before the first wrap, the song before the loop) and
    /// where the playhead was at that wrap, in quarter notes.
    pub fn locate(&self, back: u64) -> Option<(u64, f64)> {
        if back <= self.since {
            return None;
        }
        let mut r = back - self.since;
        let full = self.count.saturating_sub(1);
        let kept = self.kept();
        let mut j = 1;
        while j <= kept {
            let i = self.at(j);
            let p = u64::from(self.past[i]);
            if r <= p {
                return Some((r, self.ends[i]));
            }
            r -= p;
            j += 1;
        }
        // Older than the kept passes: whole passes at the loop's length now, no further
        // back than the wraps played.
        let left = u64::from(full - kept);
        if left == 0 {
            return Some((r, self.lead_end));
        }
        let mut m = ((r as f64 / self.loop_samples.max(1.0)) as u64).min(left);
        while m > 0 && self.older(m) >= r {
            m -= 1;
        }
        while m < left && self.older(m + 1) < r {
            m += 1;
        }
        let end = if m == left {
            self.lead_end
        } else {
            self.loop_end
        };
        Some((r - self.older(m), end))
    }

    /// [`Self::locate`]'s samples before the wrap alone.
    pub fn before_wrap(&self, back: u64) -> Option<u64> {
        self.locate(back).map(|(r, _)| r)
    }
}

impl SongPosition {
    /// Where the song was `frames` samples earlier at this tempo: what an insert hears
    /// when the audio reaching it is that late against the transport (delay compensation
    /// and the inserts before it; integration review — a synced LFO led the audio it
    /// shapes by the slot's latency). Inside a loop, a moment before a wrap is the end of
    /// the stretch that wrap ended — the pass before, or before the first wrap the song
    /// leading into the loop, played straight (review of fix3: no fold on the first
    /// entry; review of fix4: never back through more wraps than were played), the wraps
    /// placed at the samples the engine cut its passes ([`Wraps::before_wrap`]).
    /// Unchanged at 0 and while stopped (nothing plays: the parked playhead is where it
    /// was), to the bit.
    pub fn earlier_in(
        &self,
        frames: usize,
        sample_rate: f64,
        map: &cantus_core::TempoMap,
    ) -> SongPosition {
        if map.is_constant() && map.is_one_metre() {
            return self.earlier(frames, sample_rate);
        }
        if frames == 0 || !self.playing {
            return *self;
        }
        let ppq = cantus_core::PPQ as f64;
        let back = frames as f64 / sample_rate.max(1.0);
        let mut tick = map.tick_after_seconds(self.beats * ppq, -back);
        if let Some((r, end)) = self.wrap.and_then(|w| w.locate(frames as u64)) {
            tick = map.tick_after_seconds(end * ppq, -(r as f64) / sample_rate.max(1.0));
        }
        SongPosition {
            loop_beats: self.loop_beats,
            loop_seconds: self.loop_seconds,
            wrap: self.wrap,
            ..position_at_tick(map, tick, true, sample_rate)
        }
    }

    /// [`Self::earlier_in`] at one tempo and one metre: the constant arithmetic.
    pub fn earlier(&self, frames: usize, sample_rate: f64) -> SongPosition {
        if frames == 0 || !self.playing {
            return *self;
        }
        let per_sample = self.tempo_bpm / 60.0 / sample_rate.max(1.0);
        let mut beats = self.beats - frames as f64 * per_sample;
        if let Some((r, end)) = self.wrap.and_then(|w| w.locate(frames as u64)) {
            beats = end - r as f64 * per_sample;
        }
        let bar = f64::from(self.time_sig_num.max(1)) * 4.0 / f64::from(self.time_sig_den.max(1));
        SongPosition {
            beats,
            samples: (beats * 60.0 / self.tempo_bpm.max(1e-9) * sample_rate).round() as i64,
            seconds: beats * 60.0 / self.tempo_bpm.max(1e-9),
            bar_start_beats: (beats / bar).floor() * bar,
            bar_number: (beats / bar).floor() as i32,
            ..*self
        }
    }
}

/// Where the song is at `tick` through `map` (ADR 0060), for a unit that follows it: the
/// tempo of the step that holds `tick`, the beats, the beats as samples at that tempo
/// counted on from the step's start, the real seconds, the bar and the metre in force.
/// The loop and its wraps are the caller's.
pub fn position_at_tick(
    map: &cantus_core::TempoMap,
    tick: f64,
    playing: bool,
    sample_rate: f64,
) -> SongPosition {
    position_at_step(map, tick, step_of(map, tick), playing, sample_rate)
}

/// The start of the tempo step that holds `tick` — a step whose boundary `tick` is a
/// hair short of (the rounding of a position reached by the engine's own samples) counts
/// as reached, so a pass cut there is in the new step at every block size (review 1).
pub fn step_of(map: &cantus_core::TempoMap, tick: f64) -> f64 {
    match map.next_step_after(tick) {
        Some(n) if n - tick < 1e-6 => n,
        _ => map.step_start(tick),
    }
}

/// [`position_at_tick`] with the step's start given (the transport knows it to the
/// sample).
pub fn position_at_step(
    map: &cantus_core::TempoMap,
    tick: f64,
    step: f64,
    playing: bool,
    sample_rate: f64,
) -> SongPosition {
    let ppq = cantus_core::PPQ as f64;
    let beats = tick / ppq;
    let tempo = map.bpm_at(step);
    let sig = map.signature_at(cantus_core::Ticks(tick.floor() as i64));
    // Inside a ramp: the step's beats as samples at its tempo, then one per sample.
    let samples = if map.in_ramp(step) {
        step / ppq * 60.0 / tempo * sample_rate + map.seconds_between_f64(step, tick) * sample_rate
    } else {
        beats * 60.0 / tempo.max(1e-9) * sample_rate
    };
    SongPosition {
        playing,
        tempo_bpm: tempo,
        tempo_inc: map.slope_at(tick) * map.bpm_at(tick) * ppq / (60.0 * sample_rate.max(1.0)),
        tempo_now: map.bpm_at(tick),
        beats,
        samples: samples.round() as i64,
        seconds: map.seconds_at(tick),
        bar_start_beats: map.bar_start(cantus_core::Ticks(tick.floor() as i64)).0 as f64 / ppq,
        bar_number: (map.bbt(cantus_core::Ticks(tick.floor() as i64)).0 - 1) as i32,
        time_sig_num: u16::from(sig.numerator),
        time_sig_den: u16::from(sig.denominator),
        loop_beats: None,
        loop_seconds: None,
        wrap: None,
    }
}

/// A unit that passes audio through untouched: a hosted plug-in that is missing on this
/// machine, or one the engine was given no unit for.
#[derive(Debug, Default)]
pub struct Wire;

impl FxUnit for Wire {
    fn configure(&mut self, _spec: &EffectSpec) {}
    fn process(&mut self, _l: &mut [f32], _r: &mut [f32], _key: Option<(&[f32], &[f32])>) {}
    fn reset(&mut self) {}
}

/// The unit for a kind whose DSP lives behind this contract; `None` for the kinds the
/// engine handles directly (delay, reverb, EQ, compressor).
pub fn make(kind: EffectKind, sample_rate: f32) -> Option<Box<dyn FxUnit>> {
    Some(match kind {
        EffectKind::Limiter => Box::new(limiter::Unit::new(sample_rate)),
        EffectKind::Space => Box::new(space::Unit::new(sample_rate)),
        EffectKind::Multiband => Box::new(multiband::Unit::new(sample_rate)),
        EffectKind::DeEsser => Box::new(deesser::Unit::new(sample_rate)),
        EffectKind::Gate => Box::new(gate::Unit::new(sample_rate)),
        EffectKind::Saturator => Box::new(saturator::Unit::new(sample_rate)),
        EffectKind::Stereo => Box::new(stereo::Unit::new(sample_rate)),
        EffectKind::Maximizer => Box::new(maximizer::Unit::new(sample_rate)),
        // Hosted: the host delivers the real unit (`EngineHandle::add_unit`); without one
        // the slot is a wire.
        EffectKind::Chorus => Box::new(concentus::Unit::new(sample_rate)),
        EffectKind::Flanger => Box::new(flexus::Unit::new(sample_rate)),
        EffectKind::Phaser => Box::new(phasis::Unit::new(sample_rate)),
        EffectKind::Tremolo => Box::new(tremor::Unit::new(sample_rate)),
        EffectKind::Vibrato => Box::new(vibrans::Unit::new(sample_rate)),
        EffectKind::RingMod => Box::new(anulus::Unit::new(sample_rate)),
        EffectKind::FreqShift => Box::new(translatio::Unit::new(sample_rate)),
        EffectKind::VolumeShaper => Box::new(fluctus::Unit::new(sample_rate)),
        EffectKind::Clipper => Box::new(tonsor::Unit::new(sample_rate)),
        EffectKind::Transient => Box::new(impetus::Unit::new(sample_rate)),
        EffectKind::TransientSplit => Box::new(discrimen::Unit::new(sample_rate)),
        EffectKind::Analyzer => Box::new(speculum::Unit::new(sample_rate)),
        EffectKind::AutoTune => Box::new(tonus::Unit::new(sample_rate)),
        EffectKind::External => Box::new(Wire),
        _ => return None,
    })
}

#[cfg(test)]
mod position_tests {
    use super::SongPosition;

    #[test]
    fn earlier_moves_back_at_the_tempo_and_folds_into_the_loop() {
        let p = SongPosition {
            playing: true,
            tempo_bpm: 120.0,
            beats: 8.25,
            samples: 198_000,
            tempo_inc: 0.0,
            tempo_now: 120.0,
            seconds: 4.125,
            bar_start_beats: 8.0,
            bar_number: 2,
            loop_seconds: Some((4.0, 8.0)),
            time_sig_num: 4,
            time_sig_den: 4,
            loop_beats: Some((8.0, 16.0)),
            // A quarter beat (6 000 samples) after a wrap, the pass 8 beats long.
            wrap: Some(super::Wraps::first(6_000, 192_000, 16.0, 192_000.0, 16.0)),
        };
        assert_eq!(p.earlier(0, 48_000.0), p);
        // 12 000 samples at 120 BPM are half a beat: before the loop's start, so the end
        // of the pass before.
        let e = p.earlier(12_000, 48_000.0);
        assert!((e.beats - 15.75).abs() < 1e-12, "{}", e.beats);
        assert_eq!(e.bar_start_beats, 12.0);
        assert_eq!(e.samples, 378_000);
        let free = SongPosition {
            loop_beats: None,
            ..p
        };
        let e = free.earlier(6_000, 48_000.0);
        assert!((e.beats - 8.0).abs() < 1e-12);
        assert_eq!(e.bar_start_beats, 8.0);
    }

    /// Review of fix3: on the first entry into the loop (no wrap yet) the audio a late
    /// insert hears is from before the loop — no fold; an insert later than the loop is
    /// long folds as many passes back as it reaches; a stopped transport is not moved
    /// (parked on a bar line, or at zero: never negative). Review of fix4: no further
    /// back than the wraps played — after one wrap, audio from before it is the song
    /// leading into the loop, played straight.
    #[test]
    fn earlier_folds_only_after_a_wrap_and_never_moves_a_stopped_playhead() {
        let p = SongPosition {
            playing: true,
            tempo_bpm: 120.0,
            beats: 8.25,
            samples: 198_000,
            tempo_inc: 0.0,
            tempo_now: 120.0,
            seconds: 4.125,
            bar_start_beats: 8.0,
            bar_number: 2,
            loop_seconds: Some((4.0, 8.0)),
            time_sig_num: 4,
            time_sig_den: 4,
            loop_beats: Some((8.0, 16.0)),
            wrap: None,
        };
        let e = p.earlier(12_000, 48_000.0);
        assert!((e.beats - 7.75).abs() < 1e-12, "first entry: {}", e.beats);
        // A loop of one beat (24 000 samples), 6 000 samples after a wrap, an insert
        // 60 000 samples late: 54 000 before the wrap.
        let after = |passes: &[u64]| {
            let mut w = super::Wraps::first(0, 24_000, 9.0, 24_000.0, 9.0);
            for &len in passes {
                w.since = len;
                w.wrap(9.0);
            }
            w.since = 6_000;
            SongPosition {
                loop_beats: Some((8.0, 9.0)),
                wrap: Some(w),
                ..p
            }
            .earlier(60_000, 48_000.0)
            .beats
        };
        // One wrap only: the audio is from the song before the loop, 2¼ beats before the
        // loop's end where the lead-in reached it (it played 8 → 9 straight): 6.75.
        let e = after(&[]);
        assert!((e - 6.75).abs() < 1e-9, "one wrap: {e}");
        // Two: one pass back, then the lead-in.
        let e = after(&[24_000]);
        assert!((e - 7.75).abs() < 1e-9, "two wraps: {e}");
        // Three or more: three passes back, inside the loop.
        for n in [2, 3, 40] {
            let e = after(&vec![24_000; n]);
            assert!((e - 8.75).abs() < 1e-9, "{} wraps: {e}", n + 1);
        }
        // Passes of their real lengths, not the one playing's: a pass a sample longer
        // moves the wrap the audio crosses by that sample.
        let mut w = super::Wraps::first(0, 24_000, 9.0, 24_000.0, 9.0);
        for len in [24_001, 24_001] {
            w.since = len;
            w.wrap(9.0);
        }
        w.since = 6_000;
        assert_eq!(
            w.before_wrap(6_000),
            None,
            "the wrap itself is the pass playing's"
        );
        assert_eq!(w.before_wrap(6_001), Some(1));
        assert_eq!(w.before_wrap(6_000 + 24_001), Some(24_001));
        assert_eq!(w.before_wrap(6_000 + 24_002), Some(1), "the pass before");
        assert_eq!(
            w.before_wrap(6_000 + 48_002 + 5),
            Some(5),
            "the lead-in: as far back as it goes"
        );
        assert_eq!(w.before_wrap(6_000 + 48_002 + 500_000), Some(500_000));
        for beats in [16.0, 0.0] {
            let parked = SongPosition {
                playing: false,
                beats,
                samples: (beats * 24_000.0) as i64,
                bar_start_beats: beats,
                ..p
            };
            assert_eq!(parked.earlier(240, 48_000.0), parked);
        }
    }
}
