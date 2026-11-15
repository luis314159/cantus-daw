//! Tympanum's **hybrid** voice (round 40, `drums-hyb`; ADR 0124's third amendment): a
//! recorded residual played under a modelled kit's synthesis.
//!
//! For each piece × articulation × velocity layer × take of a recorded kit, an off-line
//! tool (`tools/tympanum-fit/residual_fit.py`) stores what the recording holds that model 5
//! does not — its first 30–200 ms (per piece) in a spectral sense: the recording's own
//! signal under a mask `√(1 − E|model|² / |recording|²)`, so the model's power plus the
//! residual's equals the recording's whatever seed the model's hit drew — deconvolved to
//! the piece's **dry source** by the engine's own measured source → mix response. Played
//! here, it is added to each piece's source before the virtual microphones, so the mics'
//! places, the bleed matrix, the room and the outputs apply to it as to the model.
//!
//! - **One draw per hit**: the take is [`super::tympanum::take_index`] of
//!   [`super::tympanum::hit_draw`] — the recorded kits' own rule (never the take before).
//! - **Velocity**: the two nearest layers, crossfaded with a gain that keeps the level
//!   whatever their correlation (`rho`, measured by the fit); below the softest layer its
//!   level follows the two softest layers' law.
//! - **No allocation** after [`Player::new`]; the data is an `Arc` the engine loaded with
//!   the kit, off the audio thread. A player without data does nothing: the model alone,
//!   to the bit.
//!
//! The pack's format, `CRS1` (little-endian): `"CRS1"`, `u32` version, `u32` rate, `u32`
//! the model's blob hash it was fitted on, `u16` articulations, `u16` 0; per articulation
//! `u8` key, `u8` openness ×100 (255: any), `u8` layers, `u8` 0; per layer `u8` velocity,
//! `u8` takes, `u16` 0, `f32` rho (its correlation with the next layer); per take `u32`
//! start, `u32` length, `f32` scale (sample = `i16` × scale); then `u32` samples and the
//! `i16` samples.

use std::sync::Arc;

/// The format's four bytes and version.
pub const MAGIC: &[u8; 4] = b"CRS1";
pub const VERSION: u32 = 1;
/// Residual voices at once (a busy kit's 200 ms: a few dozen hits).
pub const MAX_VOICES: usize = 48;
/// Openness states one key has at most (a hat's closed … open).
const MAX_STATES: usize = 4;
const NONE: u8 = u8::MAX;

/// One take: where its samples are and their scale.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Take {
    pub start: u32,
    pub len: u32,
    pub scale: f32,
}

/// One velocity layer.
#[derive(Clone, Debug, PartialEq)]
pub struct Layer {
    pub vel: u8,
    /// The correlation of its takes with the next layer's (the crossfade's gain).
    pub rho: f32,
    pub takes: Vec<Take>,
}

/// One articulation (a key, and for a hat its openness).
#[derive(Clone, Debug, PartialEq)]
pub struct Art {
    pub key: u8,
    /// Openness × 100 (a hat's state); 255 any.
    pub open: u8,
    /// Layers, softest first.
    pub layers: Vec<Layer>,
}

/// A kit's residual: every articulation's takes and their samples.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Residual {
    pub rate: u32,
    /// The hash ([`model_hash`]) of the model's tables it was fitted on.
    pub model: u32,
    pub arts: Vec<Art>,
    pub data: Vec<i16>,
    /// Per articulation: how its level falls below its softest layer (the exponent of
    /// velocity between its two softest layers' levels; worked out by [`Residual::finish`],
    /// not stored).
    pub soft: Vec<f32>,
}

/// FNV-1a over bytes: the model's tables' fingerprint.
pub fn fnv1a(bytes: &[u8]) -> u32 {
    let mut h: u32 = 0x811C_9DC5;
    for b in bytes {
        h ^= u32::from(*b);
        h = h.wrapping_mul(0x0100_0193);
    }
    h
}

/// The fingerprint of the model's tables this build plays (`pulsus_measured.bin`): a
/// residual fitted on other tables still plays, but the fit is stale (logged).
pub fn model_hash() -> u32 {
    fnv1a(include_bytes!("pulsus_measured.bin"))
}

struct Reader<'a> {
    b: &'a [u8],
    at: usize,
}

impl Reader<'_> {
    fn take(&mut self, n: usize) -> Result<&[u8], String> {
        let end = self.at.checked_add(n).ok_or("too long")?;
        let s = self.b.get(self.at..end).ok_or("cut short")?;
        self.at = end;
        Ok(s)
    }
    fn u8(&mut self) -> Result<u8, String> {
        Ok(self.take(1)?[0])
    }
    fn u16(&mut self) -> Result<u16, String> {
        let s = self.take(2)?;
        Ok(u16::from_le_bytes([s[0], s[1]]))
    }
    fn u32(&mut self) -> Result<u32, String> {
        let s = self.take(4)?;
        Ok(u32::from_le_bytes([s[0], s[1], s[2], s[3]]))
    }
    fn f32(&mut self) -> Result<f32, String> {
        Ok(f32::from_bits(self.u32()?))
    }
}

impl Residual {
    /// Read a `CRS1` pack. Refuses another version, a short file, a take outside the data,
    /// a non-finite number, layers out of order.
    pub fn decode(bytes: &[u8]) -> Result<Residual, String> {
        let mut r = Reader { b: bytes, at: 0 };
        if r.take(4)? != MAGIC {
            return Err("not a residual (CRS1)".into());
        }
        let version = r.u32()?;
        if version != VERSION {
            return Err(format!(
                "residual version {version}, this build reads {VERSION}"
            ));
        }
        let rate = r.u32()?;
        if !(8_000..=384_000).contains(&rate) {
            return Err(format!("rate {rate}"));
        }
        let model = r.u32()?;
        let n_arts = r.u16()? as usize;
        r.u16()?;
        let mut arts = Vec::with_capacity(n_arts);
        for _ in 0..n_arts {
            let key = r.u8()?;
            let open = r.u8()?;
            let n_layers = r.u8()? as usize;
            r.u8()?;
            let mut layers: Vec<Layer> = Vec::with_capacity(n_layers);
            for _ in 0..n_layers {
                let vel = r.u8()?;
                let n_takes = r.u8()? as usize;
                r.u16()?;
                let rho = r.f32()?;
                if !rho.is_finite() {
                    return Err("rho".into());
                }
                if layers.last().is_some_and(|l| l.vel >= vel) {
                    return Err(format!("key {key}: layers out of order"));
                }
                let mut takes = Vec::with_capacity(n_takes);
                for _ in 0..n_takes {
                    let start = r.u32()?;
                    let len = r.u32()?;
                    let scale = r.f32()?;
                    if !scale.is_finite() {
                        return Err("scale".into());
                    }
                    takes.push(Take { start, len, scale });
                }
                if takes.is_empty() {
                    return Err(format!("key {key} velocity {vel}: no take"));
                }
                layers.push(Layer {
                    vel,
                    rho: rho.clamp(-1.0, 1.0),
                    takes,
                });
            }
            if key > 127 || layers.is_empty() {
                return Err(format!("articulation {key}"));
            }
            arts.push(Art { key, open, layers });
        }
        let n = r.u32()? as usize;
        let raw = r.take(n.checked_mul(2).ok_or("too long")?)?;
        let data: Vec<i16> = raw
            .chunks_exact(2)
            .map(|c| i16::from_le_bytes([c[0], c[1]]))
            .collect();
        for a in &arts {
            for t in a.layers.iter().flat_map(|l| &l.takes) {
                if t.start as usize + t.len as usize > data.len() {
                    return Err(format!("key {}: a take outside the data", a.key));
                }
            }
        }
        if r.at != bytes.len() {
            return Err("bytes after the data".into());
        }
        if arts.len() > 255 {
            return Err("more than 255 articulations".into());
        }
        let mut r = Residual {
            rate,
            model,
            arts,
            data,
            soft: Vec::new(),
        };
        r.finish();
        Ok(r)
    }

    /// Work out what is derived from the takes ([`Residual::soft`]).
    pub fn finish(&mut self) {
        self.soft = self.arts.iter().map(|a| soft_law(a, self)).collect();
    }

    /// Write a `CRS1` pack (the probe the fit measures with; the tests).
    pub fn encode(&self) -> Vec<u8> {
        let mut o = Vec::new();
        o.extend_from_slice(MAGIC);
        o.extend_from_slice(&VERSION.to_le_bytes());
        o.extend_from_slice(&self.rate.to_le_bytes());
        o.extend_from_slice(&self.model.to_le_bytes());
        o.extend_from_slice(&(self.arts.len() as u16).to_le_bytes());
        o.extend_from_slice(&0u16.to_le_bytes());
        for a in &self.arts {
            o.extend_from_slice(&[a.key, a.open, a.layers.len() as u8, 0]);
            for l in &a.layers {
                o.extend_from_slice(&[l.vel, l.takes.len() as u8, 0, 0]);
                o.extend_from_slice(&l.rho.to_le_bytes());
                for t in &l.takes {
                    o.extend_from_slice(&t.start.to_le_bytes());
                    o.extend_from_slice(&t.len.to_le_bytes());
                    o.extend_from_slice(&t.scale.to_le_bytes());
                }
            }
        }
        o.extend_from_slice(&(self.data.len() as u32).to_le_bytes());
        for s in &self.data {
            o.extend_from_slice(&s.to_le_bytes());
        }
        o
    }

    /// Bytes it holds in memory.
    pub fn bytes(&self) -> usize {
        self.data.len() * 2
            + self
                .arts
                .iter()
                .map(|a| {
                    8 + a
                        .layers
                        .iter()
                        .map(|l| 40 + 12 * l.takes.len())
                        .sum::<usize>()
                })
                .sum::<usize>()
    }
}

/// One take playing: where in the data, how far, its gain.
#[derive(Copy, Clone, Debug, Default)]
struct Read {
    start: u32,
    len: u32,
    gain: f32,
}

#[derive(Copy, Clone, Debug, Default)]
struct Voice {
    active: bool,
    piece: u8,
    /// Up to two takes (the two layers either side of the velocity).
    a: Read,
    b: Read,
    /// Frames played (at the data's rate when it is the engine's).
    pos: u32,
    /// The data's position when its rate is not the engine's.
    fpos: f64,
    age: u64,
}

/// The residual's player: a fixed pool of voices reading the kit's residual into each
/// piece's dry source.
#[derive(Clone, Debug)]
pub struct Player {
    fs: f32,
    data: Option<Arc<Residual>>,
    /// Per key: its piece (the model's bus) and its articulations (states), 255 none.
    piece: [u8; 128],
    arts: [[u8; MAX_STATES]; 128],
    seed: u32,
    live: [u32; 128],
    voices: [Voice; MAX_VOICES],
    counter: u64,
    step: f64,
}

impl Player {
    /// Allocates its tables (with the unit, never on the audio thread).
    pub fn new(fs: f32) -> Player {
        Player {
            fs: if fs > 0.0 { fs } else { 48_000.0 },
            data: None,
            piece: [NONE; 128],
            arts: [[NONE; MAX_STATES]; 128],
            seed: 0,
            live: [0; 128],
            voices: [Voice::default(); MAX_VOICES],
            counter: 0,
            step: 1.0,
        }
    }

    /// Whether a residual is loaded (the hybrid plays).
    pub fn on(&self) -> bool {
        self.data.is_some()
    }

    /// The residual this kit plays (`None`: the model alone), the kit's keys and their
    /// pieces, and the kit's seed. Allocation-free (an `Arc` clone). A new residual stops
    /// the voices of the old.
    pub fn set(&mut self, data: Option<Arc<Residual>>, keys: &[(u8, u8)], seed: u32) {
        self.seed = seed;
        let same = match (&self.data, &data) {
            (Some(a), Some(b)) => Arc::ptr_eq(a, b),
            (None, None) => true,
            _ => false,
        };
        if !same {
            for v in &mut self.voices {
                v.active = false;
            }
        }
        self.piece = [NONE; 128];
        self.arts = [[NONE; MAX_STATES]; 128];
        self.data = data;
        let Some(d) = self.data.as_deref() else {
            return;
        };
        self.step = f64::from(d.rate) / f64::from(self.fs);
        for &(key, piece) in keys {
            self.piece[key as usize & 127] = piece;
        }
        for (i, a) in d.arts.iter().enumerate().take(255) {
            let slots = &mut self.arts[a.key as usize & 127];
            if let Some(s) = slots.iter_mut().find(|s| **s == NONE) {
                *s = i as u8;
            }
        }
    }

    /// A hit: its key, velocity, the sequencer's ordinal (`None`: live) and, for a hat,
    /// its openness (0 … 1). Nothing when the key has no residual.
    pub fn strike(&mut self, key: u8, velocity: u8, ordinal: Option<u32>, openness: f32) {
        let k = key as usize & 127;
        let Some(d) = self.data.as_deref() else {
            return;
        };
        if self.piece[k] == NONE || self.arts[k][0] == NONE {
            return;
        }
        // The nearest state by openness (a hat), else its one articulation.
        let mut best = self.arts[k][0] as usize;
        let mut dist = f32::MAX;
        for &s in self.arts[k].iter().take_while(|s| **s != NONE) {
            let a = &d.arts[s as usize];
            let e = if a.open == NONE {
                0.0
            } else {
                (f32::from(a.open) * 0.01 - openness).abs()
            };
            if e < dist {
                dist = e;
                best = s as usize;
            }
        }
        let art = &d.arts[best];
        let ordinal = ordinal.unwrap_or_else(|| {
            self.live[k] = self.live[k].wrapping_add(1);
            u32::MAX / 4 + self.live[k]
        });
        let draw = super::tympanum::hit_draw(self.seed, key, ordinal);
        let pick = |l: &Layer| l.takes[super::tympanum::take_index(draw, l.takes.len(), ordinal)];
        let v = f32::from(velocity.clamp(1, 127));
        let ls = &art.layers;
        let (a, b) = if v <= f32::from(ls[0].vel) {
            let soft = d.soft.get(best).copied().unwrap_or(2.0);
            let g = super::math::powf(v / f32::from(ls[0].vel), soft);
            (read(pick(&ls[0]), g), Read::default())
        } else if v >= f32::from(ls[ls.len() - 1].vel) {
            (read(pick(&ls[ls.len() - 1]), 1.0), Read::default())
        } else {
            let i = ls.iter().rposition(|l| f32::from(l.vel) <= v).unwrap_or(0);
            let (la, lb) = (&ls[i], &ls[i + 1]);
            let w = (v - f32::from(la.vel)) / f32::from(lb.vel - la.vel);
            let (wa, wb) = (1.0 - w, w);
            // Two different takes summed: their level kept, whatever they share.
            let norm = 1.0
                / (wa * wa + wb * wb + 2.0 * wa * wb * la.rho)
                    .max(0.25)
                    .sqrt();
            (read(pick(la), wa * norm), read(pick(lb), wb * norm))
        };
        let piece = self.piece[k];
        self.counter += 1;
        // A free voice, else the oldest.
        let slot = match self.voices.iter().position(|v| !v.active) {
            Some(i) => i,
            None => {
                let mut o = 0;
                for (i, v) in self.voices.iter().enumerate() {
                    if v.age < self.voices[o].age {
                        o = i;
                    }
                }
                o
            }
        };
        self.voices[slot] = Voice {
            active: true,
            piece,
            a,
            b,
            pos: 0,
            fpos: 0.0,
            age: self.counter,
        };
    }

    /// Add `c` frames of every voice into the pieces' sources (`src[piece * lane + t]`),
    /// marking the pieces it touched.
    pub fn render(&mut self, c: usize, src: &mut [f32], lane: usize, touched: &mut [bool]) {
        let Some(d) = self.data.as_deref() else {
            return;
        };
        let exact = self.step == 1.0;
        for v in self.voices.iter_mut().filter(|v| v.active) {
            let p = v.piece as usize;
            if p >= touched.len() || (p + 1) * lane > src.len() {
                v.active = false;
                continue;
            }
            touched[p] = true;
            let out = &mut src[p * lane..p * lane + c.min(lane)];
            if exact {
                for o in out.iter_mut() {
                    let i = v.pos;
                    *o += tap(&d.data, &v.a, i) + tap(&d.data, &v.b, i);
                    v.pos += 1;
                }
                if v.pos >= v.a.len.max(v.b.len) {
                    v.active = false;
                }
            } else {
                for o in out.iter_mut() {
                    let i = v.fpos as u32;
                    let f = (v.fpos - f64::from(i)) as f32;
                    let x0 = tap(&d.data, &v.a, i) + tap(&d.data, &v.b, i);
                    let x1 = tap(&d.data, &v.a, i + 1) + tap(&d.data, &v.b, i + 1);
                    *o += x0 + f * (x1 - x0);
                    v.fpos += self.step;
                }
                if v.fpos as u32 >= v.a.len.max(v.b.len) {
                    v.active = false;
                }
            }
        }
    }

    /// Voices sounding.
    pub fn active(&self) -> usize {
        self.voices.iter().filter(|v| v.active).count()
    }

    pub fn at_rest(&self) -> bool {
        self.data.is_none() || self.voices.iter().all(|v| !v.active)
    }

    pub fn reset(&mut self) {
        for v in &mut self.voices {
            v.active = false;
        }
        self.forget_hits();
    }

    /// Forget the live strikes counted so far (a unit come to rest is as good as new).
    pub fn forget_hits(&mut self) {
        self.live = [0; 128];
        self.counter = 0;
    }
}

#[inline]
fn read(t: Take, gain: f32) -> Read {
    Read {
        start: t.start,
        len: t.len,
        gain: gain * t.scale,
    }
}

#[inline]
fn tap(data: &[i16], r: &Read, i: u32) -> f32 {
    if i < r.len && r.gain != 0.0 {
        f32::from(data[(r.start + i) as usize]) * r.gain
    } else {
        0.0
    }
}

/// How an articulation's level falls below its softest layer: the exponent of
/// velocity between its two softest layers' levels (2 with one layer), within 0.5 … 4.
fn soft_law(a: &Art, d: &Residual) -> f32 {
    if a.layers.len() < 2 {
        return 2.0;
    }
    let level = |l: &Layer| {
        let mut e = 0.0f64;
        let mut n = 0usize;
        for t in &l.takes {
            for s in &d.data[t.start as usize..(t.start + t.len) as usize] {
                let x = f64::from(*s) * f64::from(t.scale);
                e += x * x;
            }
            n += t.len as usize;
        }
        (e / n.max(1) as f64).sqrt() as f32
    };
    let (l0, l1) = (&a.layers[0], &a.layers[1]);
    let (e0, e1) = (level(l0), level(l1));
    if e0 <= 0.0 || e1 <= 0.0 {
        return 2.0;
    }
    let k = super::math::ln(e1 / e0) / super::math::ln(f32::from(l1.vel) / f32::from(l0.vel));
    if k.is_finite() {
        k.clamp(0.5, 4.0)
    } else {
        2.0
    }
}

#[cfg(test)]
#[path = "tympanum_residual_tests.rs"]
mod tests;
