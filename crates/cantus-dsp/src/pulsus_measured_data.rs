//! Model 5's tables as one compact blob (`pulsus_measured.bin`, written by
//! `tools/tympanum-fit/tables.py`), decoded once with the kit (never on the audio thread).
//!
//! **Format `CMT1`**, little-endian throughout:
//!
//! ```text
//! b"CMT1"  u32 version (1 or 2)  u32 NB  u32 NBP  u32 tables
//! per table:  u8 id  u8 hat  u8 drum  u8 0  f32 gain_db  u32 modes  f32×modes hz  u32 states
//!   per state:  f32 openness  u8 foot  u8 0 u8 0 u8 0  u32 layers
//!     per layer:  f32 vel  f32 level  f32×2 stick  f32×2 glide
//!                 i16×(NB·NBP) env (dB × 100)
//!                 f32×modes amp  u16×modes t60 (ms)  u16×modes rise (0.1 ms)  u8×modes direct (/255)
//!                 version 2:  f32 ets (s; 0 none)  and if ets > 0:
//!                   i16×modes phase (rad × 10⁴)  f32×modes early amp  i16×modes early phase
//! ```
//!
//! Version 2 (round 40, drums-gen) carries a drum's **onset**: each mode's phase at the
//! strike and its early part (a component dying in `ets`) — the head's first push, which
//! random phases scattered.
//!
//! The quantisation is the data's own: decoding is exact integer arithmetic and one divide,
//! the same bits on every machine.

use super::{Layer, State, Table, NB, NBP};

/// The blob's version this build writes (it reads 1 too).
pub(crate) const VERSION: u32 = 2;

struct Rd<'a> {
    b: &'a [u8],
    at: usize,
}

impl Rd<'_> {
    fn take(&mut self, n: usize) -> Option<&[u8]> {
        let s = self.b.get(self.at..self.at + n)?;
        self.at += n;
        Some(s)
    }
    fn u8(&mut self) -> Option<u8> {
        self.take(1).map(|s| s[0])
    }
    fn u16(&mut self) -> Option<u16> {
        self.take(2).map(|s| u16::from_le_bytes([s[0], s[1]]))
    }
    fn i16(&mut self) -> Option<i16> {
        self.take(2).map(|s| i16::from_le_bytes([s[0], s[1]]))
    }
    fn u32(&mut self) -> Option<u32> {
        self.take(4)
            .map(|s| u32::from_le_bytes([s[0], s[1], s[2], s[3]]))
    }
    fn f32(&mut self) -> Option<f32> {
        self.take(4)
            .map(|s| f32::from_le_bytes([s[0], s[1], s[2], s[3]]))
    }
    fn f32s(&mut self, n: usize) -> Option<Vec<f32>> {
        (0..n).map(|_| self.f32()).collect()
    }
}

/// Decode a blob; `None` if it is not one this build reads.
pub(crate) fn decode(b: &[u8]) -> Option<Vec<Table>> {
    let mut r = Rd { b, at: 0 };
    if r.take(4)? != b"CMT1" {
        return None;
    }
    let version = r.u32()?;
    if version != 1 && version != VERSION {
        return None;
    }
    if r.u32()? as usize != NB || r.u32()? as usize != NBP {
        return None;
    }
    let nt = r.u32()? as usize;
    let mut tables = Vec::with_capacity(nt);
    for _ in 0..nt {
        let id = r.u8()?;
        let hat = r.u8()? != 0;
        let drum = r.u8()? != 0;
        r.u8()?;
        let gain_db = r.f32()?;
        let n = r.u32()? as usize;
        let hz = r.f32s(n)?;
        let ns = r.u32()? as usize;
        let mut states = Vec::with_capacity(ns);
        for _ in 0..ns {
            let openness = r.f32()?;
            let foot = r.u8()? != 0;
            r.take(3)?;
            let nl = r.u32()? as usize;
            let mut layers = Vec::with_capacity(nl);
            for _ in 0..nl {
                let vel = r.f32()?;
                let level = r.f32()?;
                let stick = [r.f32()?, r.f32()?];
                let glide = [r.f32()?, r.f32()?];
                let env = (0..NB * NBP)
                    .map(|_| r.i16().map(|x| f32::from(x) / 100.0))
                    .collect::<Option<Vec<f32>>>()?;
                let amp = r.f32s(n)?;
                let t60 = (0..n)
                    .map(|_| r.u16().map(|x| f32::from(x) / 1000.0))
                    .collect::<Option<Vec<f32>>>()?;
                let rise = (0..n)
                    .map(|_| r.u16().map(|x| f32::from(x) / 10_000.0))
                    .collect::<Option<Vec<f32>>>()?;
                let direct = (0..n)
                    .map(|_| r.u8().map(|x| f32::from(x) / 255.0))
                    .collect::<Option<Vec<f32>>>()?;
                let ets = if version >= 2 { r.f32()? } else { 0.0 };
                let rad = |r: &mut Rd<'_>| r.i16().map(|x| f32::from(x) / 10_000.0);
                let (ph, ea, ep) = if ets > 0.0 {
                    (
                        (0..n).map(|_| rad(&mut r)).collect::<Option<Vec<f32>>>()?,
                        r.f32s(n)?,
                        (0..n).map(|_| rad(&mut r)).collect::<Option<Vec<f32>>>()?,
                    )
                } else {
                    (Vec::new(), Vec::new(), Vec::new())
                };
                layers.push(Layer {
                    vel,
                    level,
                    stick,
                    glide,
                    env,
                    amp,
                    t60,
                    rise,
                    direct,
                    ets,
                    ph,
                    ea,
                    ep,
                });
            }
            states.push(State {
                openness,
                foot,
                layers,
            });
        }
        let low_hz = low_hz(&hz, &states);
        let lock = if hat { locks(&hz) } else { Vec::new() };
        tables.push(Table {
            id,
            hat,
            drum,
            gain_db,
            hz,
            states,
            low_hz,
            lock,
        });
    }
    (r.at == b.len()).then_some(tables)
}

/// A hat's close partials (r42:tympanum-real, model 6): each mode's phase is drawn by the
/// first mode (in the table's order) within 1 % of its frequency — the two halves of a split
/// pair, which one stroke sets ringing together; drawn apart they beat differently each hit
/// inside one third octave (the jazz closed hat: 6.1 dB from one stroke to the next, the
/// recorded hats 1.4–2.0). Not in the blob: computed with it.
fn locks(hz: &[f32]) -> Vec<u16> {
    let mut lock: Vec<u16> = (0..hz.len() as u16).collect();
    for i in 0..hz.len() {
        if let Some(j) = (0..i).find(|&j| hz[j] > 0.0 && (hz[i] - hz[j]).abs() < 0.01 * hz[i]) {
            lock[i] = lock[j];
        }
    }
    lock
}

/// A piece's fundamental, as the fit names it (drums-gen 3): the lowest mode within 10 dB of
/// its layer's strongest, the lowest over every layer (0: no mode).
fn low_hz(hz: &[f32], states: &[State]) -> f32 {
    let mut low = f32::MAX;
    for l in states.iter().flat_map(|s| &s.layers) {
        let top = l.amp.iter().fold(0.0f32, |m, a| m.max(*a));
        if top <= 0.0 {
            continue;
        }
        for (f, a) in hz.iter().zip(&l.amp) {
            if *a > top * 0.316 && *f > 0.0 {
                low = low.min(*f);
            }
        }
    }
    if low == f32::MAX {
        0.0
    } else {
        low
    }
}

/// The blob of `tables` (the inverse of [`decode`], for the round-trip test).
#[cfg(test)]
pub(crate) fn encode(tables: &[Table]) -> Vec<u8> {
    let mut o = Vec::new();
    o.extend_from_slice(b"CMT1");
    for x in [VERSION, NB as u32, NBP as u32, tables.len() as u32] {
        o.extend_from_slice(&x.to_le_bytes());
    }
    let q = |x: f32, k: f32| (x * k).round();
    for t in tables {
        o.extend_from_slice(&[t.id, t.hat as u8, t.drum as u8, 0]);
        o.extend_from_slice(&t.gain_db.to_le_bytes());
        o.extend_from_slice(&(t.hz.len() as u32).to_le_bytes());
        t.hz.iter()
            .for_each(|x| o.extend_from_slice(&x.to_le_bytes()));
        o.extend_from_slice(&(t.states.len() as u32).to_le_bytes());
        for s in &t.states {
            o.extend_from_slice(&s.openness.to_le_bytes());
            o.extend_from_slice(&[s.foot as u8, 0, 0, 0]);
            o.extend_from_slice(&(s.layers.len() as u32).to_le_bytes());
            for l in &s.layers {
                for x in [
                    l.vel, l.level, l.stick[0], l.stick[1], l.glide[0], l.glide[1],
                ] {
                    o.extend_from_slice(&x.to_le_bytes());
                }
                l.env
                    .iter()
                    .for_each(|x| o.extend_from_slice(&(q(*x, 100.0) as i16).to_le_bytes()));
                l.amp
                    .iter()
                    .for_each(|x| o.extend_from_slice(&x.to_le_bytes()));
                l.t60
                    .iter()
                    .for_each(|x| o.extend_from_slice(&(q(*x, 1000.0) as u16).to_le_bytes()));
                l.rise
                    .iter()
                    .for_each(|x| o.extend_from_slice(&(q(*x, 10_000.0) as u16).to_le_bytes()));
                l.direct.iter().for_each(|x| o.push(q(*x, 255.0) as u8));
                o.extend_from_slice(&l.ets.to_le_bytes());
                if l.ets > 0.0 {
                    l.ph.iter()
                        .for_each(|x| o.extend_from_slice(&(q(*x, 10_000.0) as i16).to_le_bytes()));
                    l.ea.iter()
                        .for_each(|x| o.extend_from_slice(&x.to_le_bytes()));
                    l.ep.iter()
                        .for_each(|x| o.extend_from_slice(&(q(*x, 10_000.0) as i16).to_le_bytes()));
                }
            }
        }
    }
    o
}
