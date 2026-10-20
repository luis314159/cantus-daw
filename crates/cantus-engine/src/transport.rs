//! Sample-accurate transport, through the tempo map (ADR 0060).
//!
//! Position is an integer sample counter since an *origin tick*, so it is exactly
//! reproducible regardless of block size (the key to the offline render being bit-identical
//! to real-time playback). Ticks are derived, never accumulated: the tick at a sample is
//! the map's tick `samples / rate` seconds after the origin, so a tempo point or a ramp is
//! **not** a jump — only a seek, a loop's wrap or an edit of the tempo map re-anchors.
//!
//! While the origin lies in a stretch of held tempo the old constant-tempo arithmetic is
//! used there unchanged (`origin + samples · ticks-per-sample`, [`sample_at`]), so a
//! project without a tempo track plays exactly as before, to the bit.

use crate::fx::Wraps;
use cantus_core::{LoopRegion, TempoMap, Ticks, TimeSignature, PPQ};
use std::sync::{Arc, LazyLock};

/// The sample (counted from `origin`) at which a transport playing at `bpm` reaches
/// `tick` — the transport's own arithmetic, so an offline render that must stop exactly
/// where an event would fire computes the same sample, to the last bit (round 23).
#[inline]
pub fn sample_at(origin: f64, tick: f64, bpm: f64, sample_rate: f64) -> u64 {
    let x = (tick - origin) / (bpm * PPQ as f64 / (60.0 * sample_rate));
    ceil_sample(x)
}

/// The first whole sample at or after `x` samples (a tiny epsilon keeps an event that
/// lands exactly on a sample from slipping one late through floating-point noise).
#[inline]
fn ceil_sample(x: f64) -> u64 {
    if x <= 0.0 {
        0
    } else {
        (x - 1e-6).ceil().max(0.0) as u64
    }
}

/// The sample (counted from `origin`) at which a transport playing through `map` from
/// `origin` reaches `tick` — what [`Transport::samples_until`] says after a seek to
/// `origin`; the offline renders' stop sample.
pub fn sample_at_map(map: &Arc<TempoMap>, origin: f64, tick: f64, sample_rate: f64) -> u64 {
    let mut t = Transport::new(sample_rate);
    t.map = map.clone();
    t.anchor(origin);
    t.target_sample(tick)
}

/// The map a transport plays before any snapshot: 120 BPM in 4/4. A static, so the
/// transport's first swap to a snapshot's map never frees it on the audio thread.
static DEFAULT_MAP: LazyLock<Arc<TempoMap>> =
    LazyLock::new(|| Arc::new(TempoMap::constant(120.0, TimeSignature::default())));

/// A stretch of held tempo around the origin: its ticks per sample, the ticks it holds
/// over and the tempo.
#[derive(Copy, Clone, Debug, PartialEq)]
struct Held {
    tps: f64,
    from: f64,
    to: f64,
    bpm: f64,
}

#[derive(Clone, Debug)]
pub struct Transport {
    sample_rate: f64,
    map: Arc<TempoMap>,
    playing: bool,
    /// Tick at which `samples` was zero.
    origin_tick: f64,
    /// The origin's seconds from tick 0, through the map.
    origin_sec: f64,
    /// The held tempo around the origin, if it lies outside a ramp.
    held: Option<Held>,
    samples: u64,
    loop_region: Option<LoopRegion>,
    /// The loop's wraps since a start or a seek — the samples since the last, how many,
    /// the last passes' lengths; `None` before the first (review of fix3: the audio an
    /// insert hears late is from before the loop only on its first entry; review of fix4:
    /// and never from more passes than were played).
    wraps: Option<Wraps>,
    /// The loop the last wrap came in — its length and its end, in ticks: what the passes
    /// older than the kept ones are counted by, whatever the loop is now (review of fix6).
    wrapped_loop: (f64, f64),
    /// The sample count when the snapshot playing now was installed (a piece whose data it
    /// swapped crossfades from there — warp, round 29's second review).
    swapped: u64,
}

impl Transport {
    pub fn new(sample_rate: f64) -> Self {
        let mut t = Transport {
            sample_rate,
            map: DEFAULT_MAP.clone(),
            playing: false,
            origin_tick: 0.0,
            origin_sec: 0.0,
            held: None,
            samples: 0,
            loop_region: None,
            wraps: None,
            wrapped_loop: (0.0, 0.0),
            swapped: 0,
        };
        t.anchor(0.0);
        t
    }

    /// A snapshot is installed now: a piece whose data it swapped fades from here.
    pub fn mark_swap(&mut self) {
        self.swapped = self.samples;
    }

    /// Where sample `s` lies in a swap's crossfade (`cantus_core::SWAP_FADE_S` from the
    /// snapshot's install): `(u, n)` with `u` in [0, 1) — `None` outside it (before the
    /// install, after a seek, or once it is done).
    #[inline]
    pub fn swap_fade(&self, s: i64) -> Option<f32> {
        let n = (cantus_core::snapshot::SWAP_FADE_S * self.sample_rate)
            .round()
            .max(1.0) as i64;
        let d = s - self.swapped as i64;
        (0..n).contains(&d).then(|| (d as f32 + 0.5) / n as f32)
    }

    /// Count from `tick`: the origin moves there and the sample counter starts again.
    fn anchor(&mut self, tick: f64) {
        self.origin_tick = tick;
        self.samples = 0;
        self.origin_sec = self.map.seconds_at(tick);
        self.held = self.map.held_at(tick).map(|(bpm, from, to)| Held {
            tps: bpm * PPQ as f64 / (60.0 * self.sample_rate),
            from,
            to,
            bpm,
        });
    }

    /// The held stretch around the origin when it covers every tick in `ticks`.
    #[inline]
    fn held_over(&self, ticks: &[f64]) -> Option<Held> {
        self.held
            .filter(|h| ticks.iter().all(|t| *t >= h.from && *t <= h.to))
    }

    pub fn sample_rate(&self) -> f64 {
        self.sample_rate
    }

    /// The tempo map the transport plays.
    pub fn map(&self) -> &Arc<TempoMap> {
        &self.map
    }

    /// The tempo now (inside a ramp, the ramp's value at the playhead).
    pub fn bpm(&self) -> f64 {
        let pos = self.position_f64();
        match self.held {
            Some(h) if pos < h.to => h.bpm,
            _ => self.map.bpm_at(pos),
        }
    }

    pub fn is_playing(&self) -> bool {
        self.playing
    }

    pub fn loop_region(&self) -> Option<&LoopRegion> {
        self.loop_region.as_ref()
    }

    /// Ticks advanced per sample at the tempo now.
    #[inline]
    pub fn ticks_per_sample(&self) -> f64 {
        let pos = self.position_f64();
        match self.held {
            Some(h) if pos < h.to => h.tps,
            _ => self.map.bpm_at(pos) * PPQ as f64 / (60.0 * self.sample_rate),
        }
    }

    /// Exact current position in ticks (fractional).
    #[inline]
    pub fn position_f64(&self) -> f64 {
        self.position_at(self.samples)
    }

    /// The position, in ticks, at a sample counted from the origin — the same arithmetic
    /// as [`Self::position_f64`], so a control point computed ahead lands on the tick the
    /// playhead will report there.
    #[inline]
    pub fn position_at(&self, samples: u64) -> f64 {
        if let Some(h) = self.held {
            let p = self.origin_tick + samples as f64 * h.tps;
            if p < h.to {
                return p;
            }
        }
        self.map
            .tick_at_seconds(self.origin_sec + samples as f64 / self.sample_rate)
    }

    pub fn position(&self) -> Ticks {
        Ticks(self.position_f64().floor() as i64)
    }

    /// The tick the sample counter counts from: the last start, seek, loop wrap or edit of
    /// the tempo map. With [`Self::samples`], what an audio clip's frame is computed from,
    /// so playback is the same for every block size.
    #[inline]
    pub fn origin_tick(&self) -> f64 {
        self.origin_tick
    }

    /// Samples since [`Self::origin_tick`].
    #[inline]
    pub fn samples(&self) -> u64 {
        self.samples
    }

    /// Where the playhead was `back` samples ago, exactly as [`Self::position_f64`] said
    /// then — or `None` if it has jumped since (a seek, a loop wrap, a tempo-map edit).
    #[inline]
    pub fn position_back(&self, back: u64) -> Option<f64> {
        (self.samples >= back).then(|| self.position_at(self.samples - back))
    }

    /// The tick `samples` samples before `tick` (counted through the map; held, the
    /// constant arithmetic): where the audio a moment before a wrap's end was.
    pub fn tick_before(&self, tick: f64, samples: u64) -> f64 {
        match self.held_over(&[tick]) {
            Some(h) if tick - samples as f64 * h.tps >= h.from => tick - samples as f64 * h.tps,
            _ => self
                .map
                .tick_after_seconds(tick, -(samples as f64) / self.sample_rate),
        }
    }

    pub fn play(&mut self) {
        if !self.playing {
            self.wraps = None;
        }
        self.playing = true;
    }

    /// Samples played since the loop last wrapped — `None` before the first wrap since the
    /// transport started or was moved.
    #[inline]
    pub fn since_wrap(&self) -> Option<u64> {
        self.wraps.as_ref().map(|w| w.since)
    }

    /// The loop's wraps since the transport started or was moved, as long as a loop lies
    /// ahead to wrap again — `None` before the first, or with no loop ahead.
    /// [`Self::wraps_played`] for what the audio still in flight lies behind.
    pub fn wraps(&self) -> Option<Wraps> {
        self.samples_until_loop_end()?;
        self.wraps_played()
    }

    /// The loop's wraps played since the transport started or was moved, whatever the loop
    /// is now — turned off or moved ahead of the playhead just after a wrap, the audio of
    /// the passes played is still in flight (review of fix6). The pass playing, to the
    /// loop's end ahead (`u64::MAX` with none); the passes older than the kept ones at the
    /// length of the loop the last wrap came in, through the map (ADR 0060: exact across a
    /// tempo change inside the loop). `None` before the first wrap; a start or a seek
    /// forgets them.
    pub fn wraps_played(&self) -> Option<Wraps> {
        let mut w = self.wraps?;
        let (len, end) = self.wrapped_loop;
        w.pass = self
            .samples_until_loop_end()
            .map_or(u64::MAX, |r| w.since + r);
        w.loop_samples = match self.held_over(&[end - len, end]) {
            Some(h) => len / h.tps,
            None => self.map.seconds_between_f64(end - len, end) * self.sample_rate,
        };
        w.loop_end = end / PPQ as f64;
        Some(w)
    }

    pub fn stop(&mut self) {
        self.playing = false;
    }

    /// Move the playhead. A tick before zero is a pre-roll (the count-in before bar 1),
    /// which plays nothing but the click until the timeline begins; the application
    /// clamps the seeks the user makes.
    pub fn seek(&mut self, tick: Ticks) {
        self.anchor(tick.0 as f64);
        self.wraps = None;
    }

    /// Move the playhead to the loop's start as a wrap (the loop's end due at a pass's first
    /// sample: played from at or past the loop's end, the loop moved behind the playhead):
    /// counted with the others, as `advance` counts its own (review of fix4), the stretch
    /// before it ending where the playhead was (review of fix5).
    pub fn wrap_to(&mut self, start: Ticks) {
        let at = self.position_f64();
        self.anchor(start.0 as f64);
        self.count_wrap(at);
    }

    /// A wrap now, the playhead at `at` (ticks) when it came.
    fn count_wrap(&mut self, at: f64) {
        if let Some(l) = &self.loop_region {
            self.wrapped_loop = ((l.end.0 - l.start.0) as f64, l.end.0 as f64);
        }
        let at = at / PPQ as f64;
        match self.wraps.as_mut() {
            Some(w) => w.wrap(at),
            None => self.wraps = Some(Wraps::first(0, 0, at, 0.0, 0.0)),
        }
    }

    /// Play `map` from now on without moving the playhead. An edit of the tempo (a
    /// different tempo function) re-anchors at the playhead; the same tempo with another
    /// metre, or the same map again, only takes the new `Arc` (the old one is the previous
    /// snapshot's, freed with it off the audio thread).
    pub fn set_map(&mut self, map: Arc<TempoMap>) {
        if map.points() != self.map.points() {
            let pos = self.position_f64();
            self.map = map;
            self.anchor(pos);
        } else {
            self.map = map;
        }
    }

    /// One tempo for the whole song, without moving the playhead (tests and tools: it
    /// allocates a map).
    pub fn set_bpm(&mut self, bpm: f64) {
        self.set_map(Arc::new(TempoMap::constant(bpm, TimeSignature::default())));
    }

    pub fn set_loop(&mut self, region: Option<LoopRegion>) {
        self.loop_region = region;
    }

    /// Absolute sample index (since the origin) at which the playhead reaches `tick`.
    /// Computed from the origin, never from the running position, so it is identical
    /// for every block size. A tiny epsilon keeps events that land exactly on a sample
    /// from slipping one sample late through floating-point noise.
    #[inline]
    fn target_sample(&self, tick: f64) -> u64 {
        match self.held_over(&[tick]) {
            Some(h) => sample_at(self.origin_tick, tick, h.bpm, self.sample_rate),
            None => ceil_sample((self.map.seconds_at(tick) - self.origin_sec) * self.sample_rate),
        }
    }

    /// The transport sample (counted from the origin, fractional) a timeline tick falls on.
    #[inline]
    pub fn sample_of_f64(&self, tick: f64) -> f64 {
        match self.held_over(&[tick]) {
            Some(h) => (tick - self.origin_tick) / h.tps,
            None => (self.map.seconds_at(tick) - self.origin_sec) * self.sample_rate,
        }
    }

    /// The transport sample (from the origin, rounded) an unwarped audio clip's frame 0
    /// lands on: its content origin (`start − offset`, `cantus_core::audio_time`), through
    /// the map, moved by `slip` seconds. Where the tempo holds there, the old arithmetic to
    /// the bit.
    pub fn clip_zero(&self, start: Ticks, offset: Ticks, slip: f64) -> i64 {
        let (s, origin) = (start.0 as f64, (start.0 - offset.0) as f64);
        // The constant arithmetic where one held stretch has the origin and the start;
        // else from the start, back by the source's seconds before it — the same sample,
        // and exactly the start's for a render that sets `slip` to cancel them (a warp).
        // (Both inside it: a negative offset puts the origin after the start, maybe past the
        // held stretch — warp, round 29's second review.)
        match self
            .held
            .filter(|h| origin.min(s) >= h.from && origin.max(s) < h.to)
        {
            Some(h) => {
                ((origin - self.origin_tick) / h.tps - slip * self.sample_rate).round() as i64
            }
            None => (self.sample_of_f64(s)
                - (cantus_core::audio_time::offset_seconds(&self.map, start, offset) + slip)
                    * self.sample_rate)
                .round() as i64,
        }
    }

    /// Samples between two ticks (fades, crossfades): the constant arithmetic where the
    /// tempo holds over both, through the map otherwise.
    #[inline]
    pub fn samples_between(&self, from: f64, to: f64) -> f64 {
        match self.held_over(&[from, to]) {
            Some(h) => (to - from) / h.tps,
            None => self.map.seconds_between_f64(from, to) * self.sample_rate,
        }
    }

    /// Number of samples from now until the playhead reaches `tick` (0 if already past).
    #[inline]
    pub fn samples_until(&self, tick: f64) -> u64 {
        self.target_sample(tick).saturating_sub(self.samples)
    }

    /// The start of the tempo step the playhead is in (ADR 0060 §4): the tick whose tempo
    /// units are told for this pass. A step reached by the engine's own rounding counts as
    /// reached, so a pass that starts on a step boundary is in the new step at every block
    /// size.
    pub fn step_tick(&self) -> f64 {
        let pos = self.position_f64();
        if let Some(n) = self.map.next_step_after(pos) {
            if self.target_sample(n) <= self.samples {
                return n;
            }
        }
        if let Some(h) = self.held {
            if pos < h.to {
                return pos;
            }
        }
        self.map.step_start(pos)
    }

    /// Samples until the next tempo step (a point, or inside a ramp the next
    /// `RAMP_STEP`): where a pass that tells units one tempo ends. `None` while the tempo
    /// holds to the end of the song.
    pub fn samples_until_step(&self) -> Option<u64> {
        let next = self.map.next_step_after(self.step_tick())?;
        Some(self.samples_until(next))
    }

    /// [`Self::samples_until_step`] for a unit whose audio is `late` samples behind the
    /// transport: samples until the position it hears reaches its next step.
    ///
    /// After a loop's wrap the unit may still hear the pass before (review 2, findings 4
    /// and 9): its steps there are counted to that pass's end, as the position it is told
    /// folds (`fx::heard_tick`, shared); a step at or past the end is no cut of its own —
    /// the late wrap's cut (`Wraps::before_wrap`) is.
    pub fn samples_until_step_late(&self, late: u64) -> Option<u64> {
        let sr = self.sample_rate;
        let wraps = self.wraps_played();
        let (heard, folded) =
            crate::fx::heard_tick(&self.map, self.position_f64(), wraps.as_ref(), late, sr);
        let next = self
            .map
            .next_step_after(crate::fx::step_of(&self.map, heard, sr))?;
        let x = match folded {
            Some((r, end)) => {
                if next >= end {
                    return None;
                }
                r as f64 - self.map.seconds_between_f64(next, end) * sr
            }
            None => self.sample_of_f64(next) + late as f64 - self.samples as f64,
        };
        Some(ceil_sample(x))
    }

    /// Samples until the loop end, if looping and the playhead is inside the loop.
    pub fn samples_until_loop_end(&self) -> Option<u64> {
        let l = self.loop_region.as_ref()?;
        let pos = self.position_f64();
        if pos < l.start.0 as f64 {
            return None;
        }
        let end_s = self.target_sample(l.end.0 as f64);
        if self.samples >= end_s {
            return Some(0);
        }
        Some(end_s - self.samples)
    }

    /// Advance by `n` samples while playing. Returns `true` if the loop wrapped.
    pub fn advance(&mut self, n: u64) -> bool {
        if !self.playing {
            return false;
        }
        let before = self.samples;
        self.samples += n;
        if let Some(w) = self.wraps.as_mut() {
            w.since += n;
        }
        if let Some(l) = &self.loop_region {
            let start = l.start.0 as f64;
            let end = l.end.0 as f64;
            let end_s = self.target_sample(end);
            if self.samples >= end_s && before < end_s && self.position_f64() >= start {
                // Carry the sub-sample overshoot so loops never drift: as much time past
                // the start as the playhead went past the end.
                let at = self.position_f64();
                let overshoot = at - end;
                let origin = match self.held_over(&[end, at]) {
                    Some(h) if self.map.held_at(start).is_some_and(|(b, ..)| b == h.bpm) => {
                        start + overshoot.max(0.0)
                    }
                    _ => {
                        let late = self.map.seconds_between_f64(end, at).max(0.0);
                        self.map.tick_after_seconds(start, late)
                    }
                };
                self.anchor(origin);
                self.count_wrap(at);
                return true;
            }
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use cantus_core::tempo::{MetreChange, TempoPoint};

    /// 90 → 140 BPM over 32 beats from beat 4, a step to 100 at beat 48.
    fn ramp_map() -> Arc<TempoMap> {
        let pt = |beats: i64, bpm: f64, ramp: bool| TempoPoint {
            tick: Ticks::beats(beats),
            bpm,
            ramp,
        };
        Arc::new(TempoMap::new(
            90.0,
            TimeSignature::default(),
            &[
                pt(0, 90.0, false),
                pt(4, 90.0, true),
                pt(36, 140.0, false),
                pt(48, 100.0, false),
            ],
            &[MetreChange {
                tick: Ticks::ZERO,
                signature: TimeSignature::default(),
            }],
        ))
    }

    fn on_map(map: &Arc<TempoMap>, sr: f64) -> Transport {
        let mut t = Transport::new(sr);
        t.set_map(map.clone());
        t.seek(Ticks::ZERO);
        t.play();
        t
    }

    /// ADR 0060: the position is the map's tick at the sample — across a held stretch, a
    /// ramp and a step — and the same to the bit at every block size.
    #[test]
    fn the_position_follows_the_map_at_every_block_size() {
        let map = ramp_map();
        let sr = 48_000.0;
        let mut ends = Vec::new();
        for block in [1u64, 37, 64, 512] {
            let mut t = on_map(&map, sr);
            let mut n = 0;
            while n < 2_000_000 {
                let k = block.min(2_000_000 - n);
                t.advance(k);
                n += k;
                let want = map.tick_at_sample(n as f64, sr);
                assert!((t.position_f64() - want).abs() < 1e-6, "{block}: {n}");
            }
            ends.push(t.position_f64().to_bits());
        }
        assert!(ends.windows(2).all(|w| w[0] == w[1]), "{ends:?}");
    }

    /// A note after the ramp fires at the analytic sample (the map's seconds × rate,
    /// rounded up), and a tempo point is no jump: the position a moment ago is known.
    #[test]
    fn an_event_after_a_ramp_is_due_at_the_analytic_sample() {
        let map = ramp_map();
        let sr = 44_100.0;
        let t = on_map(&map, sr);
        for beat in [1, 4, 20, 36, 40, 48, 60] {
            let tick = Ticks::beats(beat).0 as f64;
            let want = (map.sample_at(tick, sr) - 1e-6).ceil() as u64;
            assert_eq!(t.samples_until(tick), want, "beat {beat}");
        }
        let mut t = on_map(&map, sr);
        t.advance(3_000_000);
        let back = t
            .position_back(2_999_000)
            .expect("no jump at a point or a ramp");
        assert!((back - map.tick_at_sample(1_000.0, sr)).abs() < 1e-6);
    }

    /// A loop over a tempo change wraps exactly: every pass is the map's length of the
    /// loop in samples (within the sub-sample carried), however many passes.
    #[test]
    fn a_loop_spanning_a_tempo_change_wraps_exactly() {
        let map = ramp_map();
        let sr = 48_000.0;
        let mut t = on_map(&map, sr);
        let (ls, le) = (Ticks::beats(30), Ticks::beats(50));
        t.set_loop(Some(LoopRegion { start: ls, end: le }));
        t.seek(ls);
        let len = map.seconds_between(ls, le) * sr;
        let mut wraps = Vec::new();
        let mut clock = 0u64;
        while wraps.len() < 40 {
            clock += 64;
            if t.advance(64) {
                // The wrap is due within the block: where exactly is the carried overshoot.
                wraps.push(clock as f64 - t.samples_between(ls.0 as f64, t.position_f64()));
            }
        }
        for (i, w) in wraps.iter().enumerate() {
            let want = len * (i + 1) as f64;
            assert!((w - want).abs() < 1e-3, "pass {i}: {w} vs {want}");
        }
        let w = t.wraps().unwrap();
        assert!((w.loop_samples - len).abs() < 1e-6);
    }

    /// Review 1 (finding 26): at one tempo a host's real sample position — the song's
    /// seconds × the rate, what VST3's `projectTimeSamples` is now — is the sample position
    /// it was told before round 29 (`samples`), to the sample, everywhere.
    #[test]
    fn one_tempo_tells_hosts_the_sample_position_they_had() {
        for (bpm, sr) in [(120.0, 48_000.0), (133.0, 44_100.0), (97.3, 96_000.0)] {
            let mut t = Transport::new(sr);
            t.set_bpm(bpm);
            t.seek(Ticks(-3840));
            t.play();
            for _ in 0..2_000 {
                let p = crate::processor::song_position_of(&t, &None, sr);
                // CLAP's seconds are round 28's expression to the bit (review 2, finding
                // 13); VST3's sample position the one it had.
                assert_eq!(
                    p.seconds.to_bits(),
                    (p.beats * (60.0 / bpm)).to_bits(),
                    "{bpm} at {}",
                    p.beats
                );
                assert_eq!(p.project_samples, p.samples, "{bpm} at {}", p.beats);
                t.advance(997);
            }
        }
    }

    /// A late unit under a ramp hears the song through the map: its beats are the map's
    /// tick the latency's seconds back (review 1, finding 22).
    #[test]
    fn a_late_unit_hears_the_ramp_through_the_map() {
        let map = ramp_map();
        let sr = 48_000.0;
        let mut t = on_map(&map, sr);
        t.advance(400_000);
        let p = crate::processor::song_position_of(&t, &None, sr);
        let e = p.earlier_in(5_000, sr, &map);
        let want = map.tick_after_seconds(t.position_f64(), -5_000.0 / sr) / PPQ as f64;
        assert!((e.beats - want).abs() < 1e-9);
        assert_eq!(e.tempo_bpm, map.bpm_at(map.step_start(want * PPQ as f64)));
        assert!((e.seconds - map.seconds_at(want * PPQ as f64)).abs() < 1e-12);
    }

    /// Review 1 (finding 0), review 2: a clip starting exactly on a tempo point plays from
    /// its content origin — the same source frame at its start from a seek before the
    /// point and from a seek at it, as `cantus_core::audio_time` says.
    #[test]
    fn a_clip_on_a_tempo_point_plays_from_its_content_origin() {
        let pt = |tick: i64, bpm: f64| TempoPoint {
            tick: Ticks(tick),
            bpm,
            ramp: false,
        };
        let map = Arc::new(TempoMap::new(
            120.0,
            TimeSignature::default(),
            &[pt(0, 120.0), pt(7680, 60.0)],
            &[],
        ));
        let sr = 48_000.0;
        let (start, offset) = (Ticks(7680), Ticks(960));
        // Its content origin a beat before the point, at 120 BPM: half a second into the
        // file at the clip's start (review 2: the origin rule, ADR 0060 §5).
        let want = cantus_core::audio_time::frame_at(&map, start, offset, 0.0, sr, 7680.0);
        assert_eq!(want, 24_000.0);
        for from in [0, 7680] {
            let mut t = on_map(&map, sr);
            t.seek(Ticks(from));
            let at_start = t.sample_of_f64(start.0 as f64).round() as i64;
            let frame = at_start - t.clip_zero(start, offset, 0.0);
            assert_eq!(frame, 24_000, "played from {from}");
        }
    }

    /// The tempo units are told is held for a step of the ramp, and a pass that starts on
    /// a step boundary is already in the new step.
    #[test]
    fn units_hear_the_ramp_in_steps_on_tick_boundaries() {
        let map = ramp_map();
        let sr = 48_000.0;
        let mut t = on_map(&map, sr);
        t.seek(Ticks::beats(10));
        let first = t.step_tick();
        assert_eq!(first, map.step_start(Ticks::beats(10).0 as f64));
        let until = t.samples_until_step().unwrap();
        t.advance(until);
        let knot = first + cantus_core::tempo::RAMP_STEP as f64;
        assert_eq!(t.step_tick(), knot, "on the boundary, in the new step");
        t.advance(1);
        assert_eq!(t.step_tick(), knot);
        // In a held stretch the step is the playhead itself, and ends at the next point.
        t.seek(Ticks::beats(40));
        assert_eq!(t.bpm(), 140.0);
        let to_point = t.samples_until_step().unwrap();
        assert_eq!(to_point, t.samples_until(Ticks::beats(48).0 as f64));
    }

    #[test]
    fn position_is_exact_and_block_size_independent() {
        let mut a = Transport::new(48_000.0);
        let mut b = Transport::new(48_000.0);
        a.set_bpm(133.0);
        b.set_bpm(133.0);
        a.play();
        b.play();
        for _ in 0..1000 {
            a.advance(64);
        }
        for _ in 0..125 {
            b.advance(512);
        }
        assert_eq!(a.position_f64(), b.position_f64());
        assert_eq!(a.position(), b.position());
        // 64000 samples at 133 bpm = 1.3333 s = 2.9556 beats
        let beats = a.position_f64() / PPQ as f64;
        assert!((beats - 64_000.0 / 48_000.0 * 133.0 / 60.0).abs() < 1e-9);
    }

    #[test]
    fn loop_wraps_without_drift() {
        let mut t = Transport::new(44_100.0);
        t.set_loop(Some(LoopRegion {
            start: Ticks::beats(1),
            end: Ticks::beats(3),
        }));
        t.seek(Ticks::beats(1));
        t.play();
        let mut wraps = 0;
        for _ in 0..10_000 {
            if t.advance(256) {
                wraps += 1;
            }
        }
        // 2 beats at 120 bpm = 1 s = 44100 samples; 2 560 000 samples ≈ 58 loops.
        assert_eq!(wraps, 58);
        let pos = t.position_f64();
        assert!(pos >= Ticks::beats(1).0 as f64 && pos < Ticks::beats(3).0 as f64);
        let expected =
            Ticks::beats(1).0 as f64 + (2_560_000.0 - 58.0 * 44_100.0) * t.ticks_per_sample();
        assert!(
            (pos - expected).abs() < 1e-6,
            "pos {pos} expected {expected}"
        );
    }

    #[test]
    fn samples_until_rounds_up() {
        let mut t = Transport::new(48_000.0);
        t.play();
        // 1 tick = 48000*60/(120*960) = 25 samples exactly.
        assert_eq!(t.samples_until(1.0), 25);
        assert_eq!(t.samples_until(0.5), 13);
        assert_eq!(t.samples_until(-4.0), 0);
        t.advance(10);
        assert_eq!(t.samples_until(1.0), 15);
    }

    /// Review of fix5: a wrap the engine forces — played from past the loop's end, or the
    /// loop's end moved behind the playhead — counts like `advance`'s own, and the stretch
    /// it ends ends where the playhead was (the song straight, or the old loop's pass), so
    /// a late insert folds back to where its audio was played. Going back to `seek` (the
    /// wraps forgotten) fails it.
    #[test]
    fn a_forced_wrap_counts_and_its_stretch_ends_where_the_playhead_was() {
        let beats = |n: f64| Ticks((n * PPQ as f64) as i64);
        let mut t = Transport::new(48_000.0);
        t.set_bpm(120.0);
        t.set_loop(Some(LoopRegion {
            start: beats(8.0),
            end: beats(9.0),
        }));
        // Played from beat 12, past the loop: the end is due at once.
        t.seek(beats(12.0));
        t.play();
        assert_eq!(t.samples_until_loop_end(), Some(0));
        t.wrap_to(beats(8.0));
        let w = t.wraps().expect("the forced wrap counts");
        assert_eq!((w.count, w.since), (1, 0));
        t.advance(6_000);
        // 30 000 samples back: 24 000 before the wrap, the song at beat 12 played straight.
        assert_eq!(t.wraps().unwrap().locate(30_000), Some((24_000, 12.0)));
        // A natural wrap at beat 9, then halfway through the next pass the loop's end
        // moved to 8¼, behind the playhead: forced again, ending a stretch at 8½.
        assert!(t.advance(18_000));
        t.advance(12_000);
        t.set_loop(Some(LoopRegion {
            start: beats(8.0),
            end: beats(8.25),
        }));
        assert_eq!(t.samples_until_loop_end(), Some(0));
        t.wrap_to(beats(8.0));
        t.advance(1_000);
        let w = t.wraps().unwrap();
        assert_eq!(w.count, 3);
        assert_eq!(w.locate(1_000), None, "the pass playing");
        assert_eq!(
            w.locate(7_000),
            Some((6_000, 8.5)),
            "the stretch the edit cut"
        );
        assert_eq!(
            w.locate(13_001),
            Some((1, 9.0)),
            "the pass before, at its end"
        );
        assert_eq!(
            w.locate(37_001),
            Some((1, 12.0)),
            "the song before the loop"
        );
    }

    /// Review of fix6: a late insert folds back through the wraps played whatever the loop
    /// does after them. A loop 8–9 turned off, or moved ahead of the playhead (12–13),
    /// 1 000 samples after a wrap: the song position an insert 12 000 samples late is
    /// given is beat 9 less 11 000 samples, not the song straight back (≈ 7.54). Played
    /// past the wraps' reach, straight; a seek, or a stop and a start, forgets them.
    #[test]
    fn the_wraps_played_outlive_the_loop() {
        let sr = 48_000.0;
        let late = |t: &Transport| {
            crate::processor::song_position_of(t, &None, sr)
                .earlier(12_000, sr)
                .beats
        };
        for ahead in [None, Some((12, 13))] {
            let mut t = Transport::new(sr);
            t.set_bpm(120.0);
            t.set_loop(Some(LoopRegion {
                start: Ticks::beats(8),
                end: Ticks::beats(9),
            }));
            t.seek(Ticks::beats(8));
            t.play();
            assert!(t.advance(24_000));
            t.advance(1_000);
            t.set_loop(ahead.map(|(s, e)| LoopRegion {
                start: Ticks::beats(s),
                end: Ticks::beats(e),
            }));
            assert_eq!(t.samples_until_loop_end(), None, "no loop ahead");
            let want = 9.0 - 11_000.0 / 24_000.0;
            let got = late(&t);
            assert!((got - want).abs() < 1e-9, "{ahead:?}: {got}, want {want}");
            assert_eq!(t.wraps_played().map(|w| w.count), Some(1));
            t.advance(11_000);
            let got = late(&t);
            assert!((got - 8.0).abs() < 1e-9, "{ahead:?}, at the wrap: {got}");
            let mut stopped = t.clone();
            stopped.stop();
            stopped.play();
            assert_eq!(stopped.wraps_played(), None, "a stop and a start");
            t.seek(Ticks::beats(10));
            assert_eq!(t.wraps_played(), None, "a seek");
        }
    }

    /// Play `samples` one at a time from `from` with the loop `[ls, le)` (in ticks), the
    /// tempo changed to `bpm_at(pass)` at each wrap: the clock at each wrap.
    fn run(
        bpm: f64,
        ls: i64,
        le: i64,
        from: i64,
        samples: u64,
        bpm_at: impl Fn(usize) -> f64,
    ) -> (Transport, Vec<u64>) {
        let mut t = Transport::new(48_000.0);
        t.set_bpm(bpm);
        t.set_loop(Some(LoopRegion {
            start: Ticks(ls),
            end: Ticks(le),
        }));
        t.seek(Ticks(from));
        t.play();
        let mut wraps = Vec::new();
        for clock in 1..=samples {
            if t.advance(1) {
                wraps.push(clock);
                t.set_bpm(bpm_at(wraps.len()));
            }
        }
        (t, wraps)
    }

    /// The truth `Wraps::before_wrap` stands for: how many samples before the wrap that
    /// ends its stretch the audio `back` samples ago lies.
    fn truth(wraps: &[u64], now: u64, back: u64) -> Option<u64> {
        let at = now.checked_sub(back)?;
        if wraps.last().is_none_or(|w| at >= *w) {
            return None;
        }
        wraps.iter().find(|w| **w > at).map(|w| w - at)
    }

    /// Review of fix4: the wraps a late insert's audio lies behind are the ones played —
    /// none before the first, as many as there were after — at their real samples: exact
    /// for the kept passes even when the tempo changed between them (the passes' lengths
    /// differ), and within a sample of the truth further back while the tempo holds (a loop
    /// whose length is no whole number of samples: the measured worst case, here over 200
    /// passes of a 1/16-beat loop at 133 BPM).
    #[test]
    fn the_wraps_are_the_ones_played_at_their_samples() {
        // 1/16 beat at 133 BPM: 1 353.38… samples a pass.
        let (t, wraps) = run(133.0, 960, 1020, 0, 300_000, |_| 133.0);
        let w = t.wraps().expect("wrapped");
        assert_eq!(w.count as usize, wraps.len());
        let now = 300_000;
        let mut worst = 0u64;
        for back in (1..=now).step_by(7) {
            let (got, want) = (w.before_wrap(back), truth(&wraps, now, back));
            match (got, want) {
                (Some(g), Some(x)) => {
                    let kept_span: u64 = now - wraps[wraps.len() - 1 - crate::fx::PASSES_KEPT];
                    if back <= kept_span {
                        assert_eq!(g, x, "kept passes are exact, {back} back");
                    }
                    worst = worst.max(g.abs_diff(x));
                }
                (None, None) => {}
                // Before the clock's start: the song before the play, straight.
                (Some(_), None) if back > now => {}
                (g, x) => panic!("{back} back: {g:?}, the truth {x:?}"),
            }
        }
        assert!(worst <= 1, "the measured worst case: {worst} samples");
        let placed = wrap_error(&w, &wraps, now);
        eprintln!(
            "steady tempo, {} wraps back: off by {placed} samples at most",
            wraps.len()
        );
        assert!(placed <= 1, "{placed}");
        // Only one wrap played: the audio before it is the lead-in, however far back.
        let (t, wraps) = run(120.0, 7680, 8640, 0, 230_000, |_| 120.0);
        let w = t.wraps().expect("wrapped");
        assert_eq!(wraps.len(), 1);
        let since = 230_000 - wraps[0];
        assert_eq!(w.before_wrap(since + 60_000), Some(60_000));
        // A tempo that changes at every wrap, the passes 24 000 · 120/bpm samples long:
        // exact over the kept passes.
        let (t, wraps) = run(120.0, 960, 1920, 960, 400_000, |n| {
            100.0 + (n % 5) as f64 * 9.0
        });
        let w = t.wraps().expect("wrapped");
        let now = 400_000;
        let kept_span = now - wraps[wraps.len() - 1 - crate::fx::PASSES_KEPT.min(wraps.len() - 1)];
        for back in (1..kept_span).step_by(3) {
            assert_eq!(w.before_wrap(back), truth(&wraps, now, back), "{back} back");
        }
        // The documented limit (ADR 0057, fix5): a tempo change more than the kept passes
        // back. A 1/16-beat loop (1 500 samples at 120 BPM, 1 487.6 at 121) whose tempo
        // went from 120 to 121 at its 10th wrap, heard 60 passes back: the passes before
        // the change are counted at 121's length, 12.4 samples short each.
        let (t, wraps) = run(120.0, 960, 1020, 960, 1500 * 70, |n| {
            if n >= 10 {
                121.0
            } else {
                120.0
            }
        });
        let w = t.wraps().expect("wrapped");
        let worst = wrap_error(&w, &wraps, 1500 * 70);
        eprintln!(
            "a tempo change {} passes back: the wraps before it off by {worst} samples at most",
            wraps.len() - 10
        );
        assert!(worst > 100 && worst < 140, "{worst}");
    }

    /// How far the wraps `w` places differ from the real ones, at most: the model's wrap
    /// is where the audio one sample earlier lies one sample before it.
    fn wrap_error(w: &Wraps, wraps: &[u64], now: u64) -> u64 {
        let model: Vec<u64> = (1..now)
            .filter(|b| w.before_wrap(*b) == Some(1))
            .map(|b| b - 1)
            .collect();
        let real: Vec<u64> = wraps.iter().rev().map(|t| now - t).collect();
        assert_eq!(model.len(), real.len(), "as many wraps as were played");
        model
            .iter()
            .zip(&real)
            .map(|(m, r)| m.abs_diff(*r))
            .max()
            .unwrap_or(0)
    }
}
