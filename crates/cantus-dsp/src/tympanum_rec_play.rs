//! The recorded engine's strikes, pedal and rendering.

use super::*;

impl RecEngine {
    /// A strike on `key` at MIDI `velocity`, the `ordinal`-th of its key on the track.
    /// `source(file)` gives a file's audio as a voice reads it (`None`: not loaded — the
    /// mic is off, or its file failed: that mic stays silent).
    pub fn note_on(
        &mut self,
        key: u8,
        velocity: u8,
        note: u64,
        ordinal: u32,
        source: &dyn Fn(u32) -> Option<Source>,
    ) {
        let Some(kit) = self.kit.clone() else {
            return;
        };
        let mut hats = [NONE; 16];
        let nh = kit.hats_on(key, &mut hats);
        // The articulation(s) and the second one's weight.
        let (arts, w, open) = if nh >= 2 {
            let o = self
                .pedal
                .unwrap_or_else(|| kit.arts[hats[nh - 1] as usize].openness.unwrap_or(1.0));
            let at = |i: usize| kit.arts[hats[i] as usize].openness.unwrap_or(0.0);
            if o <= at(0) {
                ([hats[0], NONE], 0.0, at(0))
            } else if o >= at(nh - 1) {
                ([hats[nh - 1], NONE], 0.0, at(nh - 1))
            } else {
                let i = (0..nh - 1).find(|i| o < at(i + 1)).unwrap_or(nh - 2);
                let w = (o - at(i)) / (at(i + 1) - at(i)).max(1e-6);
                if w < 0.02 {
                    ([hats[i], NONE], 0.0, at(i))
                } else if w > 0.98 {
                    ([hats[i + 1], NONE], 0.0, at(i + 1))
                } else {
                    ([hats[i], hats[i + 1]], w, o)
                }
            }
        } else {
            match kit.arts.iter().position(|a| a.key == key) {
                Some(i) => {
                    let o = kit.arts[i].openness.unwrap_or(0.0);
                    ([i as u16, NONE], 0.0, o)
                }
                None => return,
            }
        };
        self.strike(&kit, arts, w, open, key, velocity, note, ordinal, source);
    }

    #[allow(clippy::too_many_arguments)]
    fn strike(
        &mut self,
        kit: &RecKit,
        arts: [u16; 2],
        w: f32,
        open: f32,
        key: u8,
        velocity: u8,
        note: u64,
        ordinal: u32,
        source: &dyn Fn(u32) -> Option<Source>,
    ) {
        let first = &kit.arts[arts[0] as usize];
        let hat = first.openness.is_some();
        // Chokes: the articulations this strike silences, and (hats) every hat ringing
        // more open than it.
        for side in arts.iter().filter(|a| **a != NONE) {
            let a = &kit.arts[*side as usize];
            let fade = (a.choke_s * self.fs).max(1.0);
            for v in self.voices.iter_mut() {
                if !v.active || v.fade_step != 0.0 {
                    continue;
                }
                let hit = v.art.iter().any(|x| *x != NONE && a.chokes.contains(x))
                    || (hat && v.hat && v.open > open + 0.1);
                if hit {
                    v.fade_step = -1.0 / fade;
                }
            }
        }
        // Polyphony: the oldest strikes of the articulation beyond its limit fade.
        if first.polyphony > 0 {
            loop {
                let mut count = 0;
                let mut oldest: Option<usize> = None;
                for (i, v) in self.voices.iter().enumerate() {
                    if v.active && v.fade_step == 0.0 && v.art[0] == arts[0] {
                        count += 1;
                        if oldest.is_none_or(|o| self.voices[o].age > v.age) {
                            oldest = Some(i);
                        }
                    }
                }
                match oldest {
                    Some(o) if count >= first.polyphony as usize => {
                        let fade = (first.choke_s * self.fs).max(1.0);
                        self.voices[o].fade_step = -1.0 / fade;
                    }
                    _ => break,
                }
            }
        }
        // The take: one draw for the strike, every mic and both sides read it.
        let draw = hit_draw(self.seed, key, ordinal);
        let vel = velocity.clamp(1, 127);
        let mics_needed: usize = arts
            .iter()
            .filter(|a| **a != NONE)
            .map(|a| {
                kit.arts[*a as usize]
                    .layers
                    .first()
                    .map_or(0, |l| l.takes.first().map_or(0, |t| t.mics.len()))
            })
            .sum();
        let vi = self.free_voice(mics_needed);
        self.counter += 1;
        let piece = first.piece;
        let p = (piece as usize).min(MAX_PIECES - 1);
        self.voices[vi] = Voice {
            active: true,
            art: arts,
            w,
            w_to: w,
            piece,
            note,
            age: self.counter,
            open,
            hat,
            fade: 1.0,
            fade_step: 0.0,
            env: 1.0,
            env_k: self.piece_env_k[p],
            plays: 0,
        };
        for (side, a) in arts.iter().enumerate() {
            if *a == NONE {
                continue;
            }
            let art = &kit.arts[*a as usize];
            let Some(layer) = art
                .layers
                .iter()
                .find(|l| l.lo <= vel && vel <= l.hi)
                .or_else(|| art.layers.last())
            else {
                continue;
            };
            let g = layer.gain[vel as usize] * self.gain;
            let n = layer.takes.len();
            let t = if self.vary {
                take_index(draw, n, ordinal)
            } else {
                ((draw * n as f32) as usize).min(n.max(1) - 1)
            };
            let Some(take) = layer.takes.get(t) else {
                continue;
            };
            for mt in &take.mics {
                let m = (mt.mic as usize).min(MAX_MICS - 1);
                let cell = self.cells[p][m];
                if cell == 0.0 {
                    continue;
                }
                let Some(src) = source(mt.file) else {
                    continue;
                };
                let Some(pi) = self.plays.iter().position(|q| !q.active) else {
                    break;
                };
                let cents = mt.cents + self.piece_cents[p];
                let ratio =
                    src.rate as f64 / self.fs as f64 * crate::math::exp2(cents as f64 / 1200.0);
                let stereo = src.channels >= 2;
                let q = &mut self.plays[pi];
                q.active = true;
                q.orphan = false;
                q.voice = vi as u16;
                q.side = side as u8;
                q.mic = m as u8;
                q.stereo = stereo;
                let (pl, pr) = if stereo {
                    let b = mt.pan.clamp(-1.0, 1.0);
                    (1.0 - b.max(0.0), 1.0 + b.min(0.0))
                } else {
                    super::super::pan_law(mt.pan)
                };
                q.gl = mt.gain * pl * g * cell;
                q.gr = mt.gain * pr * g * cell;
                q.ratio = ratio;
                q.fade = 1.0;
                q.fade_step = 0.0;
                q.hp_on = mt.highpass > 0.0;
                if q.hp_on {
                    q.hp.mode = 3;
                    q.hp.set(mt.highpass, 0.3, self.fs);
                    q.hp.reset();
                }
                let region = Region {
                    start: 0,
                    end: src.frames as i64,
                    ..Region::default()
                };
                q.head
                    .start(src, region, ratio, &self.reader, i64::from(mt.offset));
                self.voices[vi].plays += 1;
            }
        }
        if self.voices[vi].plays == 0 {
            self.voices[vi].active = false;
        }
    }

    /// Stop the strike of note `note` (a cymbal grabbed): it fades over its choke time.
    pub fn choke_note(&mut self, note: u64) {
        let Some(kit) = self.kit.clone() else {
            return;
        };
        for v in self.voices.iter_mut() {
            if v.active && v.note == note && v.fade_step == 0.0 {
                let a = &kit.arts[v.art[0] as usize];
                v.fade_step = -1.0 / (a.choke_s * self.fs).max(1.0);
            }
        }
    }

    /// The piece each sounding strike belongs to (the editor lights it), first `n`.
    pub fn sounding_pieces(&self, out: &mut [u16]) -> usize {
        let mut n = 0;
        for v in self.voices.iter().filter(|v| v.active) {
            if n < out.len() {
                out[n] = v.piece;
                n += 1;
            }
        }
        n
    }

    /// A voice to strike with, and room for `mics` plays (fix-e: never a cut). While the
    /// pool is short, the stealing order's voices — fading ones first, then the oldest —
    /// are let go **over [`STEAL_MS`]**, every mic of a voice together: their plays fade on
    /// their own ([`Self::orphan_voice`]), on [`RESERVE`] plays kept for them, while the
    /// new strike takes the rest. Only a burst that steals more than the reserve within
    /// one fade cuts the fading plays nearest their end.
    fn free_voice(&mut self, mics: usize) -> usize {
        let live = |e: &Self| e.plays.iter().filter(|p| p.active && !p.orphan).count();
        // The live plays leave the strike and the reserve room.
        while live(self) + mics + RESERVE > MAX_PLAYS {
            let Some(v) = self.victim() else { break };
            self.orphan_voice(v);
        }
        let vi = match self.voices.iter().position(|v| !v.active) {
            Some(i) => i,
            None => {
                let v = self.victim().unwrap_or(0);
                self.orphan_voice(v);
                v
            }
        };
        // More stolen at once than the reserve holds: the fades nearest their end go.
        while self.plays.iter().filter(|p| !p.active).count() < mics {
            let last = self
                .plays
                .iter()
                .enumerate()
                .filter(|(_, p)| p.active && p.orphan)
                .min_by(|(_, a), (_, b)| a.fade.total_cmp(&b.fade))
                .map(|(i, _)| i);
            let Some(i) = last else { break };
            self.plays[i].active = false;
            self.plays[i].head.let_go();
        }
        vi
    }

    /// The voice stealing takes next: fading ones first, then the oldest.
    fn victim(&self) -> Option<usize> {
        self.voices
            .iter()
            .enumerate()
            .filter(|(_, v)| v.active)
            .min_by(|(_, a), (_, b)| {
                (a.fade_step == 0.0)
                    .cmp(&(b.fade_step == 0.0))
                    .then(a.age.cmp(&b.age))
            })
            .map(|(i, _)| i)
    }

    /// Let voice `v` go over [`STEAL_MS`]: each of its plays keeps the gain it has now
    /// (the voice's fade, damping and hat weight, frozen into the play) and fades on its
    /// own; the voice is free at once.
    fn orphan_voice(&mut self, v: usize) {
        let voice = self.voices[v];
        let e = voice.fade * voice.env;
        let w = if voice.art[1] == NONE { 0.0 } else { voice.w };
        let steal = (STEAL_MS * self.fs / 1000.0).ceil().max(1.0);
        for p in self.plays.iter_mut() {
            if p.active && !p.orphan && p.voice as usize == v {
                let g = e * if p.side == 0 { 1.0 - w } else { w };
                p.gl *= g;
                p.gr *= g;
                p.orphan = true;
                let step = -p.fade / steal;
                if p.fade_step == 0.0 || step < p.fade_step {
                    p.fade_step = step;
                }
            }
        }
        self.voices[v] = Voice::default();
    }

    /// The openness controller moved, 0 closed … 1 open: crossfading hats follow it, a
    /// pedal falling below a hat's pair chokes it, and closing fast from open plays the
    /// foot's chick.
    pub fn pedal(&mut self, open: f32, source: &dyn Fn(u32) -> Option<Source>) {
        let open = open.clamp(0.0, 1.0);
        let before = self.pedal.replace(open);
        let Some(kit) = self.kit.clone() else {
            return;
        };
        let closing = before.is_some_and(|b| b >= 0.35) && open < 0.1;
        let mut choked = false;
        for v in self.voices.iter_mut() {
            if !v.active || !v.hat || v.fade_step != 0.0 {
                continue;
            }
            let lo = kit.arts[v.art[0] as usize].openness.unwrap_or(0.0);
            if v.art[1] != NONE {
                let hi = kit.arts[v.art[1] as usize].openness.unwrap_or(1.0);
                v.w_to = ((open - lo) / (hi - lo).max(1e-6)).clamp(0.0, 1.0);
                v.open = open.clamp(lo, hi);
            }
            // The pedal below what the strike can play: the cymbals close on it.
            if open < lo - 0.12 && v.open > open + 0.1 {
                let a = &kit.arts[v.art[0] as usize];
                v.fade_step = -1.0 / (a.choke_s * self.fs).max(1.0);
                choked = true;
            }
        }
        if closing && choked && self.chick {
            if let Some(chick) = kit.pedal {
                let speed = before.unwrap_or(1.0) - open;
                let vel = (40.0 + 70.0 * speed).clamp(1.0, 127.0) as u8;
                self.chicks = self.chicks.wrapping_add(1);
                let key = kit.arts[chick as usize].key;
                let ordinal = u32::MAX / 2 + self.chicks;
                self.strike(
                    &kit,
                    [chick, NONE],
                    0.0,
                    0.0,
                    key,
                    vel,
                    u64::MAX - 1,
                    ordinal,
                    source,
                );
            }
        }
    }

    /// Render `c ≤ LANE` frames of every play into its mic's lane (the lanes begun by the
    /// caller). Each play renders sample by sample from its own start, so any chunking
    /// gives the same bits.
    pub fn render(&mut self, c: usize, outs: &mut Outputs) {
        let follow = 1.0 / (FOLLOW_MS * self.fs / 1000.0).max(1.0);
        // Each voice's gains over the chunk, then its state moves on.
        for (vi, v) in self.voices.iter_mut().enumerate() {
            if !v.active {
                continue;
            }
            let g = &mut self.gains[vi * 2 * LANE..(vi + 1) * 2 * LANE];
            for t in 0..c {
                if v.w != v.w_to {
                    v.w = if v.w < v.w_to {
                        (v.w + follow).min(v.w_to)
                    } else {
                        (v.w - follow).max(v.w_to)
                    };
                }
                if v.fade_step != 0.0 {
                    v.fade = (v.fade + v.fade_step).max(0.0);
                }
                let e = v.fade * v.env;
                if v.env_k != 1.0 {
                    v.env *= v.env_k;
                }
                let w = if v.art[1] == NONE { 0.0 } else { v.w };
                g[t] = e * (1.0 - w);
                g[LANE + t] = e * w;
            }
        }
        let mut coefs = [0.0f32; MAX_TAPS];
        for p in self.plays.iter_mut() {
            if !p.active {
                continue;
            }
            let vi = p.voice as usize;
            // A stolen voice's play carries its gain itself (`orphan_voice`).
            let g = if p.orphan {
                &ONES[..c]
            } else {
                &self.gains[vi * 2 * LANE + p.side as usize * LANE..][..c]
            };
            let (l, r) = outs.lane(p.mic as usize, c);
            let mut i = 0;
            let mut ended = false;
            while i < c && !ended {
                let mut run = (c - i).min(CONTROL);
                if p.head.streamed() {
                    let fading = p.fade_step != 0.0;
                    let fade = (STEAL_MS * self.fs / 1000.0).ceil() as usize;
                    match p.head.ensure(p.ratio, &self.reader, run, fading) {
                        Reach::Ready(k) => run = run.min(k.max(1)),
                        Reach::Late(k) => {
                            if !fading {
                                p.late = true;
                                p.fade_step = -1.0 / k.min(fade).max(1) as f32;
                            }
                            run = run.min(k.max(1));
                        }
                        Reach::End(k) => {
                            if !fading {
                                p.cut = true;
                                p.fade_step = -1.0 / k.min(fade).max(1) as f32;
                            }
                            run = run.min(k.max(1));
                        }
                    }
                }
                for t in i..i + run {
                    let x = p.head.next(p.ratio, &self.reader, &mut coefs);
                    let (mut a, mut b) = if p.stereo { (x[0], x[1]) } else { (x[0], x[0]) };
                    if p.hp_on {
                        a = p.hp.process(a, 0);
                        b = if p.stereo { p.hp.process(b, 1) } else { a };
                    }
                    let mut gg = g[t];
                    if p.fade_step != 0.0 {
                        p.fade += p.fade_step;
                        if p.fade <= 0.0 {
                            ended = true;
                            break;
                        }
                        gg *= p.fade;
                    }
                    l[t] += a * gg * p.gl;
                    r[t] += b * gg * p.gr;
                    if p.head.is_done() {
                        ended = true;
                        break;
                    }
                }
                if p.head.take_missed() && p.fade_step == 0.0 {
                    p.late = true;
                    p.fade_step = -1.0 / (STEAL_MS * self.fs / 1000.0).ceil();
                }
                i += run;
            }
            if std::mem::take(&mut p.late) {
                self.late += 1;
            }
            if std::mem::take(&mut p.cut) {
                self.cut += 1;
            }
            self.wakes += p.head.take_wakes();
            if p.orphan {
                // A stolen voice's play: tied to no voice, it ends with its own fade.
                if ended {
                    p.active = false;
                    p.head.let_go();
                }
                continue;
            }
            let voice_done = self.voices[vi].fade <= 0.0;
            if ended || voice_done {
                p.active = false;
                p.head.let_go();
                let v = &mut self.voices[vi];
                v.plays = v.plays.saturating_sub(1);
                if v.plays == 0 {
                    *v = Voice::default();
                }
            }
        }
        // A voice whose fade ended with plays that ended the same chunk is gone above; one
        // with no plays left (all ended) too.
        for v in self.voices.iter_mut() {
            if v.active && (v.plays == 0 || v.fade <= 0.0) {
                *v = Voice::default();
            }
        }
    }
}
