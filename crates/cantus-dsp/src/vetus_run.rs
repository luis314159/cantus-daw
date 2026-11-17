//! Vetus's unit at work (ADR 0133, provisional): the grid, the samples, the landing of a
//! switch, the rest predicate and the telemetry. A child of `vetus_unit.rs`.

use super::*;

impl Vetus {
    /// The start of grid cell `cell`: the glides of the coefficients step, the filters are
    /// designed where they moved, the age is read at both ends of the cell, and decayed
    /// filter states settle to +0.0.
    fn grid(&mut self, cell: i64) {
        self.cell = cell;
        let s0 = cell * GRID;
        let rate_moving = !self.rate_l.still();
        self.rate_l.next();
        if rate_moving || self.dirty {
            self.rate.design(math::exp2(self.rate_l.value));
        }
        let tone_moving = !(self.lo_l.still() && self.hi_l.still() && self.res.still());
        self.lo_l.next();
        self.hi_l.next();
        self.res.next();
        if tone_moving || self.dirty {
            self.design_tone();
        }
        self.dirty = false;
        if self.on[AGE].value > 0.0 || self.on[AGE].target > 0.0 {
            let (g0, b0) = self.age.at(&self.eff.age, s0);
            let (g1, _) = self.age.at(&self.eff.age, s0 + GRID);
            self.age_g = (g0, g1);
            if b0 != self.age_b {
                self.age_b = b0;
                self.age_c = SvfCoef::low(
                    age::cutoff(b0).min(0.45 * self.sr),
                    std::f32::consts::FRAC_1_SQRT_2,
                    self.sr,
                );
            }
        }
        for f in self
            .dc
            .iter_mut()
            .chain(self.age_lp.iter_mut())
            .chain(self.tone.iter_mut().flatten())
        {
            f.settle();
        }
        self.rate.settle();
        // Under −90 dBFS the noise it lets through is 60 dB under its level: none.
        if self.env < 1.0 / (1_000.0 * FOLLOW_FULL) {
            self.env = 0.0;
        }
    }

    fn design_tone(&mut self) {
        let sr = self.sr;
        let (lo, hi) = (math::exp2(self.lo_l.value), math::exp2(self.hi_l.value));
        let q = std::f32::consts::FRAC_1_SQRT_2 + 2.3 * self.res.value;
        let (bells, steep) = self.eff.medium.colour();
        self.tone_c[0] = SvfCoef::high(lo, q, sr);
        self.tone_c[1] = SvfCoef::low(hi, q, sr);
        self.tone_c[2] = if steep {
            SvfCoef::low(hi, std::f32::consts::FRAC_1_SQRT_2, sr)
        } else {
            SvfCoef::default()
        };
        for (c, (hz, bq, db)) in self.tone_c[3..].iter_mut().zip(bells) {
            *c = if db == 0.0 {
                SvfCoef::default()
            } else {
                SvfCoef::bell(hz, bq, db, sr)
            };
        }
    }

    pub fn process(&mut self, l: &mut [f32], r: &mut [f32]) {
        let n = l.len().min(r.len());
        let in_peak = peak_of(&l[..n], &r[..n]);
        let truncate = self.eff.bits_mode == BitsMode::Truncate && !self.bits.is_wire();
        let noisy = self.eff.noise_sounds();
        let follow = self.eff.follow.clamp(0.0, 1.0);
        let rate_filter = self.eff.rate_filter;
        let drift = self.eff.drift.clamp(0.0, 1.0);
        let (mut mw, mut mf, mut noise_peak) = (0.0f32, 0.0f32, 0.0f32);
        self.noise.clicks = 0;
        for i in 0..n {
            let song = self.clock.song(i);
            let cell = song.div_euclid(GRID);
            if cell != self.cell {
                self.grid(cell);
            }
            let dry = [l[i], r[i]];
            let mut v = dry;
            // 1. Bits.
            let g = self.on[BITS].next();
            if g > 0.0 {
                let mut q = [self.bits.run(v[0], 0, song), self.bits.run(v[1], 1, song)];
                if truncate {
                    for (x, f) in q.iter_mut().zip(self.dc.iter_mut()) {
                        *x = f.run(*x, &self.dc_c);
                    }
                }
                v = blend(v, q, g);
            }
            // 2. Rate.
            let g = self.on[RATE].next();
            if g > 0.0 {
                let rate = math::exp2(self.rate_l.value);
                let h = self.rate.run(v, song, rate, rate_filter);
                v = blend(v, h, g);
            }
            // 3. Wobble.
            let aw = self.amp_w.next();
            let af = self.amp_f.next();
            let b = self.base.next().max(aw + af);
            if self.wob_live || b != 0.0 {
                self.wob_live = true;
                self.lines[0].push(v[0]);
                self.lines[1].push(v[1]);
                if b != 0.0 || aw != 0.0 || af != 0.0 {
                    mw = self.wow.value(&self.clock, i) * (1.0 - drift)
                        + self.drift.value(&self.clock, i) * drift;
                    mf = self.flut.value(&self.clock, i);
                    let d = b + aw * mw + af * mf;
                    v = [self.lines[0].read(d), self.lines[1].read(d)];
                }
            }
            // 4. Age.
            let g = self.on[AGE].next();
            if g > 0.0 {
                let (g0, g1) = self.age_g;
                let x = (song - self.cell * GRID) as f32 / GRID as f32;
                let gain = g0 + (g1 - g0) * x;
                let y = [
                    self.age_lp[0].run(v[0] * gain, &self.age_c),
                    self.age_lp[1].run(v[1] * gain, &self.age_c),
                ];
                v = blend(v, y, g);
            }
            // 5. Noise, after its follower of the music.
            let g = self.on[NOISE].next();
            if g > 0.0 && noisy {
                let a = dry[0].abs().max(dry[1].abs());
                let k = if a > self.env {
                    self.env_att
                } else {
                    self.env_rel
                };
                self.env += (a - self.env) * k;
                let level = g * ((1.0 - follow) + follow * (self.env * FOLLOW_FULL).min(1.0));
                if level > 0.0 {
                    let (nl, nr) = self.noise.sample(&self.eff.noise, song, level);
                    noise_peak = noise_peak.max(nl.abs()).max(nr.abs());
                    v = [v[0] + nl, v[1] + nr];
                }
            }
            // 6. Tone.
            let g = self.on[TONE].next();
            if g > 0.0 {
                let mut y = v;
                for (ch, s) in y.iter_mut().enumerate() {
                    for (f, c) in self.tone[ch].iter_mut().zip(&self.tone_c) {
                        *s = f.run(*s, c);
                    }
                }
                v = blend(v, y, g);
            }
            let mix = self.mix.next();
            let out = self.out.next();
            let mut o = blend(dry, v, mix);
            if out != 1.0 {
                o[0] *= out;
                o[1] *= out;
            }
            l[i] = o[0];
            r[i] = o[1];
        }
        self.clock.advance(n);
        self.land();
        self.blocks = (self.blocks + 1) % (1 << 24);
        self.telemetry_block(in_peak, peak_of(&l[..n], &r[..n]), mw, mf, noise_peak);
    }

    /// A module whose switch has landed off is made new, so a module turned on again
    /// starts as a new unit's would.
    fn land(&mut self) {
        let off = |g: &Glide| g.value == 0.0 && g.target == 0.0;
        if off(&self.on[BITS]) {
            for f in &mut self.dc {
                f.reset();
            }
        }
        if off(&self.on[RATE]) && !self.rate.quiet() {
            self.rate.reset();
        }
        if off(&self.on[AGE]) {
            for f in &mut self.age_lp {
                f.reset();
            }
        }
        if off(&self.on[NOISE]) {
            self.noise.reset();
            self.env = 0.0;
        }
        if off(&self.on[TONE]) {
            for f in self.tone.iter_mut().flatten() {
                f.reset();
            }
        }
        let still = self.amp_w.value == 0.0 && self.amp_f.value == 0.0 && self.base.value == 0.0;
        if self.wob_live && still && self.base.target == 0.0 {
            for l in &mut self.lines {
                if !l.is_silent() {
                    l.reset();
                }
            }
            self.wob_live = false;
        }
    }

    fn telemetry_block(&mut self, in_peak: f32, out_peak: f32, mw: f32, mf: f32, noise: f32) {
        let mut on = 0u32;
        for (k, g) in self.on.iter().enumerate() {
            let live = if k == WOB {
                self.base.value > 0.0
            } else {
                g.value > 0.0
            };
            if live {
                on |= 1 << k;
            }
        }
        let song_s = self.clock.song(0) as f64 / f64::from(self.sr);
        let (g0, g1) = self.age_g;
        let gain = 0.5 * (g0 + g1);
        // The pitch now from the delay's slope: a delay that grows reads slower.
        let slope = std::f32::consts::TAU / self.sr
            * (self.amp_w.value * self.wow_hz() as f32 + self.amp_f.value * self.eff.flut_hz);
        let speed = if slope > 0.0 {
            -1200.0 * math::log2(1.0 + slope * mw.abs().max(mf.abs()).min(1.0))
        } else {
            0.0
        };
        let tl = &mut self.tel;
        tl[tel::SAMPLE_RATE] = self.sr;
        tl[tel::TEMPO] = self.clock.tempo() as f32;
        tl[tel::PLAYING] = if self.clock.playing() { 1.0 } else { 0.0 };
        tl[tel::MOD_L] = mw;
        tl[tel::MOD_R] = mf;
        tl[tel::IN_PEAK] = peak_db(in_peak);
        tl[tel::OUT_PEAK] = peak_db(out_peak);
        tl[tel::BLOCKS] = self.blocks as f32;
        tl[t::ON] = on as f32;
        tl[t::BITS] = self.eff.bits;
        tl[t::RATE] = math::exp2(self.rate_l.value);
        tl[t::WOW] = mw;
        tl[t::FLUT] = mf;
        tl[t::SPEED_CT] = speed;
        tl[t::DROP_DB] = if self.on[AGE].value > 0.0 {
            20.0 * math::log10(gain.max(1e-6))
        } else {
            0.0
        };
        tl[t::BRIGHT] = self.age_b;
        tl[t::CLICKS] = self.noise.clicks as f32;
        tl[t::NOISE_DB] = peak_db(noise);
        tl[t::LO] = math::exp2(self.lo_l.value);
        tl[t::HI] = math::exp2(self.hi_l.value);
        tl[t::CHARACTER] = self.eff.character;
        tl[t::TURN] = (song_s * 100.0 / 3.0 / 60.0).rem_euclid(1.0) as f32;
        tl[t::ENV] = self.env;
    }

    pub fn telemetry(&self, out: &mut [f32; 64]) {
        out.copy_from_slice(&self.tel);
    }

    pub fn at_rest(&self) -> bool {
        self.rest_blocker().is_none()
    }

    pub fn rest_blocker(&self) -> Option<&'static str> {
        let moving = self
            .on
            .iter()
            .chain([
                &self.rate_l,
                &self.amp_w,
                &self.amp_f,
                &self.base,
                &self.lo_l,
                &self.hi_l,
                &self.res,
                &self.mix,
                &self.out,
            ])
            .any(|g| !g.still());
        if moving {
            return Some("a glide is moving");
        }
        if self.on[NOISE].value > 0.0 && self.eff.noise_sounds() {
            if self.eff.follow < 1.0 {
                return Some("the noise plays in silence");
            }
            if self.env != 0.0 {
                return Some("the noise follows a sound");
            }
        }
        if self.wob_live && !self.lines.iter().all(ModLine::is_silent) {
            return Some("the wobble's line holds sound");
        }
        if !self.rate.quiet() {
            return Some("the hold holds sound");
        }
        let rings = !self
            .dc
            .iter()
            .chain(&self.age_lp)
            .chain(self.tone.iter().flatten())
            .all(Svf::is_zero);
        if rings {
            return Some("a filter rings");
        }
        None
    }

    /// `frames` of silence skipped at rest: the clock and the lines count on, the hold's
    /// phase is where a new unit's would be, and the grid is on the cell of the last sample
    /// skipped (what processing the silence would have left: a glide moved after the rest
    /// then steps on the same samples).
    pub fn skip(&mut self, frames: usize) {
        if frames == 0 {
            return;
        }
        self.clock.skip(frames);
        if self.wob_live {
            for l in &mut self.lines {
                l.skip(frames);
            }
        }
        let rate = math::exp2(self.rate_l.value);
        self.rate.rest_at(self.clock.song(0), rate);
        let cell = (self.clock.song(0) - 1).div_euclid(GRID);
        if cell != self.cell {
            self.grid(cell);
        }
    }
}
