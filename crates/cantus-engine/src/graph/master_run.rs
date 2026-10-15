//! The master node's pass (`master.rs`): gather, chain, strip, take, click — on the
//! callback, after the pool's part of the pass, reading what the other nodes left in
//! their cells.

use super::strip_run::{analyse, read_meters, SILENCE};
use super::*;
use crate::graph::gather::add_interleaved;
use crate::profile::Node;

impl Processor {
    /// Run the master over the pass `[p0, p0 + n)`: `out` is the pass's stretch of the
    /// device's interleaved buffer, zeroed at the top of the block.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn run_master_node(
        &mut self,
        snap: Option<&Snapshot>,
        out: &mut [f32],
        channels: usize,
        p0: usize,
        n: usize,
        position: &crate::fx::SongPosition,
        strip: MasterStrip,
        peaks: &mut [f32; 2],
    ) {
        self.prof.enter(Node::Engine);
        // ---- gather, in canonical order: the tracks routed here, then each strip's
        // output edge in the order --------------------------------------------------------
        {
            let nodes = &*self.nodes;
            let master = self.plan.master();
            if let Some(frame) = nodes.read(&nodes.frame) {
                for i in frame.map.inputs(master) {
                    strip::with_input(nodes, *i, n, &mut |c| add_interleaved(out, channels, c));
                }
            }
        }
        // The master's insert chain, on the summed mix, before its fader (ADR 0026).
        self.run_master(out, channels, n, p0, position);
        // Master strip: width, then pan and gain, then polarity last of all. Automation
        // (ADR 0027) overrides the snapshot's values while it drives them.
        let master_width = self.auto.master[2].unwrap_or(strip.width);
        if channels > 1 && master_width != 1.0 {
            for frame in out.chunks_exact_mut(channels) {
                let (mut l, mut r) = (frame[0], frame[1]);
                let (mid, side) = ((l + r) * 0.5, (l - r) * 0.5 * master_width.clamp(0.0, 2.0));
                l = mid + side;
                r = mid - side;
                frame[0] = l;
                frame[1] = r;
            }
        }
        let master_ramp = self
            .auto
            .k0(self.transport.samples())
            .filter(|_| self.auto.master_gain.is_some() || self.auto.master_pan.is_some());
        let master_gain = self.auto.master[0].unwrap_or(strip.gain);
        let (master_l, master_r) = match self.auto.master[1] {
            Some(p) => balance_gains(p),
            None => (strip.l, strip.r),
        };
        let mut pass_peak = 0f32;
        for (i, frame) in out.chunks_exact_mut(channels).enumerate() {
            let (g, ml, mr) = match master_ramp {
                Some(k0) => {
                    let k = k0 + i as u64;
                    let g = self.auto.master_gain.map_or(master_gain, |r| r.at(k));
                    let (l, r) = match self.auto.master_pan {
                        Some(p) => balance_gains(p.at(k)),
                        None => (master_l, master_r),
                    };
                    (g, l, r)
                }
                None => (master_gain, master_l, master_r),
            };
            for (ch, s) in frame.iter_mut().enumerate() {
                let pan = match ch {
                    0 => ml,
                    1 => mr,
                    _ => 1.0,
                };
                *s *= g * pan * strip.sign;
                if ch < 2 {
                    peaks[ch] = peaks[ch].max(s.abs());
                    pass_peak = pass_peak.max(s.abs());
                }
            }
        }
        // What a modulator following the master hears (ADR 0057).
        self.auto.follow_master = self.auto.follow_master.max(pass_peak);
        // The master's loudness (ADR 0052): the mix as it leaves the master strip, read into
        // the scratch lanes the click uses next (it writes them before it reads them).
        {
            let (l, r) = (&mut self.edge_l[..n], &mut self.edge_r[..n]);
            for (i, frame) in out.chunks_exact(channels).enumerate() {
                l[i] = frame[0];
                r[i] = if channels > 1 { frame[1] } else { 0.0 };
            }
            let playing = self.transport.is_playing();
            // A mono output is one channel to BS.1770.
            self.master_loud
                .feed(l, (channels > 1).then_some(r), playing);
        }
        // The recorder, on the master: the mix as it leaves the master strip.
        if self.capture_target == CaptureTarget::Master {
            match self.nodes.capture.try_borrow_mut() {
                Some(mut cap) => {
                    for frame in out.chunks_exact(channels) {
                        let (l, r) = (frame[0], if channels > 1 { frame[1] } else { frame[0] });
                        cap.frame(l, r);
                    }
                }
                None => self.nodes.conflict(),
            }
        }
        self.run_click(snap, out, channels, n);
    }

    /// The click, after the master strip: it is for the player, so the master fader does
    /// not turn it down and the recorder never hears it. It splits the pass at its own
    /// beats only: a beat due at a stretch's first sample sounds there, and the stretch
    /// ends at the next one.
    fn run_click(&mut self, snap: Option<&Snapshot>, out: &mut [f32], channels: usize, n: usize) {
        let beats = match snap {
            Some(s) if self.click_on() && self.transport.is_playing() => Some((
                s.time_signature.ticks_per_beat().0 as f64,
                s.time_signature.ticks_per_bar().0 as f64,
            )),
            _ => None,
        };
        let mut c0 = 0usize;
        while c0 < n {
            let mut c1 = n;
            if let Some((beat_len, bar_len)) = beats {
                let until = |p: &Self| {
                    (p.transport.samples_until(p.next_beat_tick) as usize).saturating_sub(c0)
                };
                if until(self) == 0 {
                    let in_bar = self.next_beat_tick.rem_euclid(bar_len);
                    if self.click_allowed_at(self.transport.samples() + c0 as u64) {
                        self.click.trigger(in_bar.abs() < 0.5);
                    }
                    self.next_beat_tick += beat_len;
                }
                c1 = n.min(c0 + until(self).max(1));
            }
            self.click_stretch(&mut out[c0 * channels..c1 * channels], channels, c1 - c0);
            c0 = c1;
        }
    }

    /// One stretch of the click into the output. With latency it goes through a line of
    /// the output's latency, so it comes out with the beat it marks (ADR 0022).
    fn click_stretch(&mut self, out: &mut [f32], channels: usize, n: usize) {
        let click_line = self
            .click_line
            .filter(|li| self.click.is_active() || self.nodes.line_ringing(Some(*li)));
        if let Some(li) = click_line {
            let active = self.click.is_active();
            for i in 0..n {
                let c = if active { self.click.next() } else { 0.0 };
                self.edge_l[i] = c;
                self.edge_r[i] = c;
            }
            if let Some(mut line) = self.nodes.line(li) {
                line.run(&mut self.edge_l[..n], &mut self.edge_r[..n]);
            }
            for (i, frame) in out.chunks_exact_mut(channels).enumerate() {
                frame[0] += self.edge_l[i];
                if channels > 1 {
                    frame[1] += self.edge_r[i];
                }
            }
        } else if self.click.is_active() && self.click_line.is_none() {
            for frame in out.chunks_exact_mut(channels) {
                let c = self.click.next();
                frame[0] += c;
                if channels > 1 {
                    frame[1] += c;
                }
            }
        }
    }

    /// Run the master chain on the summed mix in `out` (interleaved, `n` frames, the pass
    /// starting `p0` frames into the block), in place. Called after the master's gather
    /// and before its width, gain, balance and polarity.
    ///
    /// A mono output carries the mix as `l + r`; the chain hears half of it on each side
    /// and gives back their sum, which at unity is the input to the bit.
    pub(super) fn run_master(
        &mut self,
        out: &mut [f32],
        channels: usize,
        n: usize,
        p0: usize,
        position: &crate::fx::SongPosition,
    ) {
        if self.master.effects.is_empty() {
            self.master.gr = 0.0;
            return;
        }
        // The deepest reduction of this block: the first pass starts it afresh.
        if p0 == 0 {
            self.master.gr = 0.0;
        }
        if !self.master.runs() {
            return;
        }
        let start = self.clock + p0 as u64;
        let pass = self.pass;
        let nodes = self.nodes.clone();
        let m = &mut self.master;
        let l = &mut m.buf_l[..n];
        let r = &mut m.buf_r[..n];
        for i in 0..n {
            let frame = &out[i * channels..(i + 1) * channels];
            if channels > 1 {
                l[i] = frame[0];
                r[i] = frame[1];
            } else {
                l[i] = frame[0] * 0.5;
                r[i] = frame[0] * 0.5;
            }
        }
        let dry_l = &mut m.dry_l[..n];
        let dry_r = &mut m.dry_r[..n];
        for (i, e) in m.effects.iter_mut().enumerate() {
            self.prof.enter(Node::MasterInsert(i));
            let looked_at = self.analysed == Some(e.id);
            let mut ana = if looked_at {
                let a = nodes.analysis.try_borrow_mut();
                if a.is_none() {
                    nodes.conflict();
                }
                a
            } else {
                None
            };
            if let Some(a) = ana.as_mut() {
                for k in 0..n {
                    a.pre[k] = (l[k] + r[k]) * 0.5;
                }
            }
            // Notes from the track it listens to, due in this pass (ADR 0022).
            if let Some(li) = m.listener[i] {
                if let Some(mut lis) = nodes.listener(li as usize) {
                    if let Some(outbox) = nodes.read(&nodes.tracks[lis.track].outbox) {
                        lis.deliver(&outbox, start, start + n as u64, |at, note| {
                            e.note(at, note)
                        });
                    }
                }
            }
            // Bypassed or switched off: a wire when it has no latency, a soft bypass (its
            // dry line, the effect still running) when it has.
            let off = m.bypass[i] || !m.fx_enabled;
            let latent = e.latency() > 0;
            if off && !latent {
                if let Some(a) = ana.as_mut() {
                    analyse(a, l, r);
                }
                continue;
            }
            let mix = m.mix[i];
            // The master runs after every strip: a key written this pass is heard.
            let kbuf = m.key[i].and_then(|k| nodes.read(&nodes.strips[k].key));
            let key = m.key[i].map(|_| match kbuf.as_deref() {
                Some(b) if b.pass == pass => (&b.l[..n], &b.r[..n]),
                _ => (&SILENCE[..n], &SILENCE[..n]),
            });
            e.set_position(position);
            if mix >= 1.0 && !latent {
                e.process_keyed(l, r, key);
            } else {
                dry_l.copy_from_slice(l);
                dry_r.copy_from_slice(r);
                if let Some(li) = m.dry_line[i] {
                    if let Some(mut line) = nodes.line(li) {
                        line.run(dry_l, dry_r);
                    }
                }
                e.process_keyed(l, r, key);
                if off {
                    l.copy_from_slice(dry_l);
                    r.copy_from_slice(dry_r);
                } else if mix < 1.0 {
                    let dry = 1.0 - mix;
                    for k in 0..n {
                        l[k] = l[k] * mix + dry_l[k] * dry;
                        r[k] = r[k] * mix + dry_r[k] * dry;
                    }
                }
            }
            drop(kbuf);
            let effect = e.id;
            e.drain_param_events(|event| {
                if self
                    .tx
                    .push(EngineEvent::FxParam { effect, event })
                    .is_err()
                {
                    self.stats.dropped_events += 1;
                }
            });
            if let Some(a) = ana.as_mut() {
                analyse(a, l, r);
            }
            if off {
                continue;
            }
            if let Some(gr) = e.gain_reduction_db() {
                m.gr = m.gr.max(gr);
            }
            if let Some(a) = ana.as_mut() {
                read_meters(a, e);
            }
        }
        self.prof.enter(Node::Engine);
        for i in 0..n {
            let frame = &mut out[i * channels..(i + 1) * channels];
            if channels > 1 {
                frame[0] = l[i];
                frame[1] = r[i];
            } else {
                frame[0] = l[i] + r[i];
            }
        }
    }
}

/// The master's loudness meter (ADR 0052): BS.1770 K-weighting, 10 ms columns, momentary,
/// short-term and integrated LUFS and a 4× true peak (`cantus_dsp::terminus::loudness`),
/// fed with the mix as it leaves the master strip — after the recorder's tap point, before
/// the click. It only reads (its lanes are a scratch copy): no sample of the output depends
/// on it (the goldens hold it).
///
/// - Built with the processor, pre-faulted with it ([`Self::prefault`]); feeding it never
///   allocates.
/// - A mono output is weighed as one channel.
/// - A sample that is not finite is metered as silence, so the filters never hold a NaN.
/// - The integrated reading is the pass's: it starts again when playback starts, is worked
///   out only when a 100 ms gating block was added while playing, and holds after Stop.
#[derive(Clone, Debug)]
pub(crate) struct MasterLoud {
    meter: cantus_dsp::terminus::loudness::LoudnessMeter,
    playing: bool,
    /// The gating blocks the integrated reading was worked out from, and the reading.
    blocks_seen: u64,
    integrated: f32,
    /// How many times the integrated reading was worked out (tests).
    pub(crate) scans: u64,
}

impl MasterLoud {
    pub(crate) fn new(sample_rate: f32) -> Self {
        MasterLoud {
            meter: cantus_dsp::terminus::loudness::LoudnessMeter::new(sample_rate),
            playing: false,
            blocks_seen: 0,
            integrated: f32::NEG_INFINITY,
            scans: 0,
        }
    }

    /// Touch the meter's histograms on the building thread, so their pages are never first
    /// written by the audio thread (ADR 0031 R11).
    pub(crate) fn prefault(&mut self) {
        let c = &mut self.meter.columns;
        c.blocks.clear();
        c.shorts.clear();
        std::hint::black_box(&c.blocks);
        std::hint::black_box(&c.shorts);
    }

    /// One stretch of the master's output, in scratch lanes: `l`, and `r` — `None` for a
    /// mono output.
    pub(crate) fn feed(&mut self, l: &mut [f32], r: Option<&mut [f32]>, playing: bool) {
        if playing && !self.playing {
            let c = &mut self.meter.columns;
            // Only what was written needs clearing: a histogram never filled stays clear.
            if !c.blocks.is_empty() {
                c.blocks.clear();
            }
            if !c.shorts.is_empty() {
                c.shorts.clear();
            }
            self.blocks_seen = 0;
            self.integrated = f32::NEG_INFINITY;
        }
        self.playing = playing;
        let finite = |x: &mut f32| {
            if !x.is_finite() {
                *x = 0.0;
            }
        };
        l.iter_mut().for_each(finite);
        match r {
            Some(r) => {
                r.iter_mut().for_each(finite);
                self.meter.process(l, r);
            }
            None => self.meter.process_mono(l),
        }
        // The pass's integrated loudness: worked out when a gating block was added while
        // playing; after Stop it holds.
        let blocks = self.meter.columns.blocks.len();
        if playing && blocks != self.blocks_seen {
            self.blocks_seen = blocks;
            self.scans += 1;
            self.integrated = self
                .meter
                .integrated()
                .map_or(f32::NEG_INFINITY, |v| v as f32);
        }
    }

    /// The readings, and the block's true peak cleared for the next block.
    pub(crate) fn take(&mut self) -> crate::messages::MasterLoudness {
        let m = &mut self.meter;
        let out = crate::messages::MasterLoudness {
            momentary: m.momentary() as f32,
            short_term: m.short_term() as f32,
            integrated: self.integrated,
            true_peak: m.max_true_peak,
        };
        m.max_true_peak = 0.0;
        out
    }
}

#[cfg(test)]
mod loud_tests {
    use super::MasterLoud;

    const SR: f32 = 48_000.0;

    /// `blocks` blocks of `n` frames of a 1 kHz sine at `db` dBFS peak, fed stereo (or mono);
    /// the last reading.
    fn sine(
        m: &mut MasterLoud,
        db: f32,
        blocks: usize,
        n: usize,
        mono: bool,
        playing: bool,
        t: &mut usize,
    ) -> crate::messages::MasterLoudness {
        let a = cantus_dsp::math::powf(10f32, db / 20.0);
        let mut buf = vec![0f32; n];
        let mut r = m.take();
        for _ in 0..blocks {
            for x in buf.iter_mut() {
                let ph = (*t as f64 * 1_000.0 / SR as f64).fract() as f32;
                *x = a * cantus_dsp::math::sin(ph * std::f32::consts::TAU);
                *t += 1;
            }
            let mut r2 = buf.clone();
            m.feed(&mut buf.clone(), (!mono).then_some(&mut r2[..]), playing);
            r = m.take();
        }
        r
    }

    /// What the meter costs the audio thread at 32, 64 and 256-frame callbacks, measured:
    /// `cargo test --release -p cantus-engine --lib loud_cost -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn loud_cost() {
        for n in [32usize, 64, 256] {
            let mut m = MasterLoud::new(SR);
            let buf: Vec<f32> = (0..n)
                .map(|i| ((i * 37 % 101) as f32 / 101.0) - 0.5)
                .collect();
            let blocks = 48_000 * 20 / n;
            let (mut l, mut r) = (buf.clone(), buf.clone());
            let t = std::time::Instant::now();
            for _ in 0..blocks {
                l.copy_from_slice(&buf);
                r.copy_from_slice(&buf);
                m.feed(&mut l, Some(&mut r), true);
                std::hint::black_box(m.take());
            }
            let secs = t.elapsed().as_secs_f64();
            let audio = (blocks * n) as f64 / SR as f64;
            eprintln!(
                "master loudness at {n}-frame callbacks: {:.0} ns a callback, {:.1} ns a \
                 stereo frame, {:.3} % of one core at 48 kHz",
                secs * 1e9 / blocks as f64,
                secs * 1e9 / (blocks * n) as f64,
                100.0 * secs / audio
            );
        }
    }

    /// A 1 kHz sine at −20 dBFS peak on both channels reads −20 LUFS (BS.1770's
    /// calibration: a full-scale stereo 1 kHz sine is 0 LUFS) within 0.1 LU, momentary,
    /// short-term and integrated; its true peak is its peak.
    #[test]
    fn a_stereo_sine_reads_as_bs1770_says() {
        let a = cantus_dsp::math::powf(10f32, -20.0 / 20.0);
        let mut m = MasterLoud::new(SR);
        let r = sine(&mut m, -20.0, 400, 480, false, true, &mut 0);
        for v in [r.momentary, r.short_term, r.integrated] {
            assert!((v + 20.0).abs() < 0.1, "{r:?}");
        }
        assert!((r.true_peak - a).abs() < 0.01 * a, "{r:?}");
        // The block's true peak is the block's: once the oversampler's short history has
        // passed, a silent block reads 0.
        for _ in 0..2 {
            m.feed(&mut [0.0; 480], Some(&mut [0.0; 480]), true);
        }
        let _ = m.take();
        m.feed(&mut [0.0; 480], Some(&mut [0.0; 480]), true);
        assert_eq!(m.take().true_peak, 0.0);
    }

    /// A mono output is one channel: the same sine reads 3 dB less than on two (review 1).
    #[test]
    fn a_mono_output_is_weighed_as_one_channel() {
        let mut m = MasterLoud::new(SR);
        let r = sine(&mut m, -20.0, 400, 480, true, true, &mut 0);
        assert!((r.momentary + 23.01).abs() < 0.1, "{r:?}");
    }

    /// A sample that is not a number reaches the meter as silence: the readings come back
    /// with the music (review 1: one NaN left them at −∞ for good).
    #[test]
    fn a_nan_does_not_poison_the_meter() {
        let mut m = MasterLoud::new(SR);
        let mut t = 0;
        let _ = sine(&mut m, -20.0, 100, 480, false, true, &mut t);
        m.feed(&mut [f32::NAN; 480], Some(&mut [f32::INFINITY; 480]), true);
        let r = sine(&mut m, -20.0, 400, 480, false, true, &mut t);
        assert!((r.momentary + 20.0).abs() < 0.1, "{r:?}");
        assert!((r.short_term + 20.0).abs() < 0.1, "{r:?}");
    }

    /// The integrated reading is the played pass's: what sounds after Stop leaves it as it
    /// was; Play starts it again (review 1).
    #[test]
    fn the_integrated_reading_holds_while_stopped() {
        let mut m = MasterLoud::new(SR);
        let mut t = 0;
        let played = sine(&mut m, -20.0, 400, 480, false, true, &mut t).integrated;
        let after = sine(&mut m, -8.0, 400, 480, false, false, &mut t);
        assert_eq!(after.integrated, played);
        assert!(after.momentary > -9.0, "momentary stays live: {after:?}");
        let again = sine(&mut m, -8.0, 400, 480, false, true, &mut t);
        assert!((again.integrated + 8.0).abs() < 0.2, "{again:?}");
    }

    /// The integrated reading is worked out when a gating block is added (every 100 ms),
    /// not on every callback (review 1).
    #[test]
    fn the_integrated_reading_is_worked_out_only_when_it_can_change() {
        let mut m = MasterLoud::new(SR);
        let _ = sine(&mut m, -20.0, 200, 32, false, true, &mut 0);
        let scans = m.scans;
        // 200 callbacks of 32 frames = 133 ms: at most two gating blocks were added.
        assert!(scans <= 2, "{scans} scans");
        for _ in 0..50 {
            let _ = m.take();
        }
        assert_eq!(m.scans, scans, "no new block, no scan");
    }
}
