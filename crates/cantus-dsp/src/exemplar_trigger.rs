//! Which zones a note starts, starting a layer, releasing a note, release samples, chokes.

use super::*;
use crate::exemplar::voice::CONTROL;
use crate::math;

/// Gain of a zone's key and velocity crossfades for a note (1 outside the fades). Power
/// curves keep the sum of squares at 1 across a fade, gain curves the sum of amplitudes.
pub fn crossfade(z: &ZoneParams, key: u8, v7: f32) -> f32 {
    let f = |x: f32| {
        let x = x.clamp(0.0, 1.0);
        if z.fade_gain {
            x
        } else {
            x.sqrt()
        }
    };
    let mut g = 1.0;
    let k = key as f32;
    if z.kfin > 0 && k < z.lo as f32 + z.kfin as f32 {
        g *= f((k - z.lo as f32) / z.kfin as f32);
    }
    if z.kfout > 0 && k > z.hi as f32 - z.kfout as f32 {
        g *= f((z.hi as f32 - k) / z.kfout as f32);
    }
    if z.vfin > 0 && v7 < z.vlo as f32 + z.vfin as f32 {
        g *= f((v7 - z.vlo as f32) / z.vfin as f32);
    }
    if z.vfout > 0 && v7 > z.vhi as f32 - z.vfout as f32 {
        g *= f((z.vhi as f32 - v7) / z.vfout as f32);
    }
    g
}

impl Exemplar {
    /// Whether zone `zi` answers the note in `s` for trigger `cond`.
    fn matches(&self, zi: usize, s: &Slot, cond: Trig) -> bool {
        let z = &self.zones[zi];
        let trig_ok = match z.trig {
            Trig::Attack => matches!(cond, Trig::First | Trig::Legato),
            t => t == cond,
        };
        if !trig_ok {
            return false;
        }
        let v7 = (s.vel * 127.0).round().clamp(1.0, 127.0) as u8;
        if s.key < z.lo || s.key > z.hi || v7 < z.vlo || v7 > z.vhi {
            return false;
        }
        self.trigger_ok(zi, s)
    }

    /// What a zone asks of any trigger: a sound, its group not muted, its keyswitch in
    /// force, its round-robin turn and its random range.
    fn trigger_ok(&self, zi: usize, s: &Slot) -> bool {
        let z = &self.zones[zi];
        if self.sources[zi].is_empty() || self.groups[z.group as usize].mute {
            return false;
        }
        let sw = if self.keyswitch >= 0 {
            self.keyswitch
        } else {
            self.globals.sw_default
        };
        if z.sw >= 0 && z.sw != sw {
            return false;
        }
        if z.seqlen > 1 && (s.rr % z.seqlen as u32) + 1 != z.seq as u32 {
            return false;
        }
        s.rnd >= z.rlo && (s.rnd < z.rhi || z.rhi >= 1.0)
    }

    /// Start the attack zones of a new note (`cond`: first or legato).
    pub(super) fn start_zones(&mut self, voice: usize, cond: Trig, gain: f32) {
        let s = self.slots[voice];
        // Chokes first, so this note's own layers survive their own class.
        let mut classes = [0u8; 8];
        let mut nc = 0;
        for oi in 0..self.order_len {
            let zi = self.order[oi] as usize;
            let c = self.zones[zi].excl;
            if c > 0
                && nc < classes.len()
                && !classes[..nc].contains(&c)
                && self.matches(zi, &s, cond)
            {
                classes[nc] = c;
                nc += 1;
            }
        }
        for &c in &classes[..nc] {
            self.choke(c);
        }
        for oi in 0..self.order_len {
            let zi = self.order[oi] as usize;
            if self.matches(zi, &s, cond) {
                self.start_layer(voice, zi, gain, false);
            }
        }
    }

    /// Start the release (or key-up) zones of a note, quieter by `rt_decay` per second held.
    pub(super) fn start_zones_of(&mut self, voice: usize, cond: Trig, relvel: f32) {
        let s = self.slots[voice];
        let held = (self.clock - s.start) as f32 / self.fs;
        for oi in 0..self.order_len {
            let zi = self.order[oi] as usize;
            if !self.matches(zi, &s, cond) {
                continue;
            }
            let z = &self.zones[zi];
            let mut g = db(-z.rt_decay * held);
            if z.rt_follow {
                g *= s.reached;
            }
            if g > 1e-5 {
                self.start_layer(voice, zi, g, true);
            }
        }
        let _ = relvel;
        if self.slots[voice].layers == 0
            && !self.slots[voice].key_down
            && self.slots[voice].released
        {
            self.alloc.voice_finished(voice);
        }
    }

    /// The pitch of a layer before modulation, in semitones from its zone's root.
    fn base_semis(&self, zi: usize, key: u8) -> f64 {
        let z = &self.zones[zi];
        let g = &self.globals;
        let grp = &self.groups[z.group as usize];
        (key as f64 - z.root as f64) * z.keytrack as f64 / 100.0
            + (z.tune_ct + grp.tune_ct + g.tune_ct - self.sources[zi].tune_ct) as f64 / 100.0
            + g.transpose as f64
    }

    /// The layers controller `n` started whose zone loops and whose range `v` has left:
    /// released (a sample that does not loop plays on, as SFZ players play pedal noises).
    pub(super) fn cc_left(&mut self, n: u8, v: u8) {
        let Exemplar { layers, zones, .. } = self;
        for l in layers.iter_mut() {
            if !(l.active && l.by_cc && !l.released) {
                continue;
            }
            let z = &zones[l.zone as usize];
            if z.cc != n || (z.cclo..=z.cchi).contains(&v) || !z.region.looping {
                continue;
            }
            // As a key's release does (`release_slot`): a one-shot plays on, a sustain loop
            // lets go, a continuous loop loops on through the release.
            l.released = true;
            if !l.one_shot {
                l.env.release();
                l.env2.release();
                if l.sustain_loop {
                    l.play.release_loop();
                }
            }
        }
    }

    /// Start the zones controller `n` crossed into, going from `prev` to `v`: one trigger,
    /// with its own round-robin count and random number, chokes first.
    pub(super) fn cc_entered(&mut self, n: u8, prev: u8, v: u8) {
        let entered = |z: &ZoneParams| {
            z.trig == Trig::Controller
                && z.cc == n
                && (z.cclo..=z.cchi).contains(&v)
                && !(z.cclo..=z.cchi).contains(&prev)
        };
        if !(0..self.order_len).any(|oi| entered(&self.zones[self.order[oi] as usize])) {
            return;
        }
        let count = self.cc_count;
        self.cc_count = count.wrapping_add(1);
        let id = CC_IDS.wrapping_add(count as u64);
        let s = Slot {
            key: 60,
            id,
            vel: 1.0,
            start: self.clock,
            rr: count,
            rnd: note_random(id, n),
            ..Slot::default()
        };
        let ok = |e: &Exemplar, zi: usize| {
            let z = &e.zones[zi];
            entered(z) && e.trigger_ok(zi, &s)
        };
        let mut classes = [0u8; 8];
        let mut nc = 0;
        for oi in 0..self.order_len {
            let zi = self.order[oi] as usize;
            let c = self.zones[zi].excl;
            if c > 0 && nc < classes.len() && !classes[..nc].contains(&c) && ok(self, zi) {
                classes[nc] = c;
                nc += 1;
            }
        }
        for &c in &classes[..nc] {
            self.choke(c);
        }
        for oi in 0..self.order_len {
            let zi = self.order[oi] as usize;
            if ok(self, zi) {
                let mut at_root = s;
                at_root.key = self.zones[zi].root;
                self.start_layer_for(ORPHAN, at_root, zi, 1.0, false);
            }
        }
    }

    pub(super) fn start_layer(&mut self, voice: usize, zi: usize, gain: f32, release: bool) {
        let s = self.slots[voice];
        self.start_layer_for(voice, s, zi, gain, release);
    }

    /// Start zone `zi` as a layer of `voice` (or of no note: [`ORPHAN`]) for the note `s`.
    pub(super) fn start_layer_for(
        &mut self,
        voice: usize,
        s: Slot,
        zi: usize,
        gain: f32,
        release: bool,
    ) {
        let z = self.zones[zi];
        let grp = self.groups[z.group as usize].clone();
        if grp.poly > 0 {
            let (mut count, mut oldest) = (0usize, None::<usize>);
            for (i, l) in self.layers.iter().enumerate() {
                if l.active && l.group == z.group && l.fade_step == 0.0 {
                    count += 1;
                    if oldest.is_none_or(|o| self.layers[o].serial > l.serial) {
                        oldest = Some(i);
                    }
                }
            }
            if count >= grp.poly as usize {
                if let Some(o) = oldest {
                    self.fade_layer(o);
                }
            }
        }
        let li = match self.layers.iter().position(|l| !l.active) {
            Some(i) => i,
            None => {
                let i = (0..MAX_LAYERS)
                    .min_by_key(|i| self.layers[*i].serial)
                    .unwrap_or(0);
                self.drop_layer(i);
                i
            }
        };
        let g = &self.globals;
        let fs = self.fs;
        let ms = |x: f32| x * fs / 1000.0;
        let a = |i: usize| grp.amp[i].unwrap_or(g.amp[i]);
        let shape = EnvShape {
            delay: ms(z.delay_ms) as u32,
            attack: ms(a(0)),
            hold: if a(1) > 0.1 { ms(a(1)) as u32 } else { 0 },
            decay: ms(a(2)),
            sustain: a(3),
            release: ms(a(4)),
        };
        let shape2 = EnvShape {
            delay: shape.delay,
            attack: ms(g.env2[0]),
            hold: 0,
            decay: ms(g.env2[1]),
            sustain: g.env2[2],
            release: ms(g.env2[3]),
        };
        let t = grp.vel_amp.unwrap_or(g.vel_amp);
        let vg = (1.0 + t * (s.vel * s.vel - 1.0)).max(0.0);
        // Key and velocity crossfades are the keys' business: a layer a controller starts
        // plays on its root at full velocity, whatever fades the zone has.
        let xf = if voice == ORPHAN {
            1.0
        } else {
            crossfade(&z, s.key, s.vel * 127.0)
        };
        let amp = db(z.gain_db + grp.gain_db + g.gain_db) * vg * xf * gain;
        let semis = self.base_semis(zi, s.key);
        let rate_ratio = self.sources[zi].rate as f64 / fs as f64;
        let bend = (self.bend * g.bend_range / 2.0) as f64;
        let ratio = rate_ratio * math::powf(2f64, (semis + bend) / 12.0);
        let offset = (g.start_vel_ms * (1.0 - s.vel) * self.sources[zi].rate / 1000.0) as i64;
        let mut region = z.region;
        if release {
            region.looping = false;
        }
        let quality = g.quality as usize;
        let filter = g.filter;
        self.serial += 1;
        let serial = self.serial;
        let Exemplar {
            layers,
            sources,
            readers,
            ..
        } = self;
        let l = &mut layers[li];
        l.active = true;
        l.slot = voice;
        l.id = s.id;
        l.key = s.key;
        l.zone = zi as u16;
        l.group = z.group;
        l.offby = z.offby;
        l.off_normal = z.off_normal;
        l.one_shot = z.one_shot || release;
        l.by_cc = voice == ORPHAN;
        l.sustain_loop = z.sustain_loop;
        l.released = false;
        l.semis = semis;
        l.rate_ratio = rate_ratio;
        l.ratio = ratio;
        l.glide = 0.0;
        l.glide_step = 0.0;
        l.amp = amp;
        l.pan = z.pan + grp.pan + g.pan;
        l.gmod = 1.0;
        l.gmod_to = 1.0;
        l.first = true;
        l.env.start(&shape);
        l.env2.start(&shape2);
        l.filter.reset();
        l.filter.mode = filter;
        l.lfo = [0.0; 2];
        l.ctl = 0;
        l.age = 0;
        l.serial = serial;
        l.fade = 1.0;
        l.fade_step = 0.0;
        l.vel = s.vel;
        l.relvel = 0.0;
        l.rnd = s.rnd * 2.0 - 1.0;
        l.play.start(
            sources[zi].clone(),
            region,
            ratio,
            &readers[quality],
            offset,
        );
        if let Some(slot) = self.slots.get_mut(voice) {
            slot.layers += 1;
        }
    }

    /// Release a note: its layers enter their release (sustain loops let go), and its
    /// release samples start. `fire_key`: the key went up now (key-up samples too).
    pub(super) fn release_slot(&mut self, voice: usize, relvel: f32, fire_key: bool) {
        if self.slots[voice].released {
            return;
        }
        let mut reached = 0.0f32;
        for l in self.layers.iter_mut() {
            if !(l.active && l.slot == voice && !l.released) {
                continue;
            }
            reached = reached.max(l.env.level);
            l.released = true;
            l.relvel = relvel;
            if !l.one_shot {
                l.env.release();
                l.env2.release();
                if l.sustain_loop {
                    l.play.release_loop();
                }
            }
        }
        let s = &mut self.slots[voice];
        s.released = true;
        s.key_down = false;
        s.reached = reached;
        self.start_zones_of(voice, Trig::Release, relvel);
        if fire_key {
            self.start_zones_of(voice, Trig::ReleaseKey, relvel);
        }
        if self.slots[voice].layers == 0 {
            self.alloc.voice_finished(voice);
        }
    }

    /// A stolen note: its layers fade out on their own, no longer tied to the slot.
    pub(super) fn detach_slot(&mut self, voice: usize, _fade: bool) {
        let n = STEAL_MS * self.fs / 1000.0;
        for l in self.layers.iter_mut() {
            if l.active && l.slot == voice {
                l.fade_out(n);
                l.slot = ORPHAN;
            }
        }
        self.slots[voice].layers = 0;
    }

    fn fade_layer(&mut self, i: usize) {
        let n = STEAL_MS * self.fs / 1000.0;
        self.layers[i].fade_out(n);
    }

    /// Stop a layer at once (the pool is full), keeping the slot's count right.
    fn drop_layer(&mut self, i: usize) {
        let slot = self.layers[i].slot;
        self.layers[i].active = false;
        self.layer_ended(slot);
    }

    /// Bookkeeping when a layer of `slot` has ended.
    pub(super) fn layer_ended(&mut self, slot: usize) {
        if slot == ORPHAN || slot >= MAX_VOICES {
            return;
        }
        let s = &mut self.slots[slot];
        s.layers = s.layers.saturating_sub(1);
        if s.layers == 0 && !s.key_down && s.released {
            self.alloc.voice_finished(slot);
        }
    }

    /// Silence every layer the class `c` turns off (SFZ `off_by`, Decent `silencedByTags`).
    fn choke(&mut self, c: u8) {
        let n = STEAL_MS * self.fs / 1000.0;
        for l in self.layers.iter_mut() {
            if l.active && l.offby == c && l.fade_step == 0.0 {
                if l.off_normal {
                    l.released = true;
                    l.env.release();
                    l.env2.release();
                } else {
                    l.fade_out(n);
                }
            }
        }
    }

    /// Mono and legato: the sounding note moves to `to` without a new attack.
    pub(super) fn glide_slot(&mut self, voice: usize, to: u8, to_id: u64) {
        let from = self.slots[voice].key;
        let s = &mut self.slots[voice];
        s.key = to;
        s.id = to_id;
        s.key_down = true;
        let steps = (self.globals.glide_ms * self.fs / 1000.0 / CONTROL as f32).floor() as f64;
        for i in 0..MAX_LAYERS {
            if !(self.layers[i].active && self.layers[i].slot == voice) {
                continue;
            }
            let kt = self.zones[self.layers[i].zone as usize].keytrack as f64 / 100.0;
            let d = (to as f64 - from as f64) * kt;
            let l = &mut self.layers[i];
            l.semis += d;
            l.key = to;
            if steps >= 1.0 {
                l.glide -= d;
                l.glide_step = -l.glide / steps;
            } else {
                l.glide = 0.0;
                l.glide_step = 0.0;
            }
        }
    }
}
