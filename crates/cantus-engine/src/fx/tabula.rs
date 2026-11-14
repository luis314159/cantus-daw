//! Tabula, the pedalboard (ADR 0134, provisional), behind the engine's plug-in contract.
//! Owned by the tabula builder.
//!
//! A board runs its pedals — each an [`EffectUnit`] of a native kind, built off the audio
//! thread ([`Unit::build`], `EngineMessage::AddPedal` → [`FxUnit::adopt`]) — in the spec's
//! order along **Pre → (A ∥ B) → Post**:
//!
//! ```text
//! in ─┬─ × in ─ Pre … ─┬─ A … ──────────┬─ ab ─ Post … ─ mix ─ dip ─ × out
//!     │                └─ B … ─ (align) ┘                 │     │
//!     └─ dry (delayed by the board's latency) ────────────┴─────┘
//! ```
//!
//! - **A board of one pedal is that unit**, to the bit: with every fade settled, the split
//!   absent, `mix` at 100 % and the gains at 0 dB the board only calls the pedal's
//!   `process_keyed` on the buffers it was given — no multiply.
//! - **The footswitch** crossfades over [`FADE_MS`]. A pedal without latency then stops
//!   running (and is reset, so it comes back as new); a latent one keeps running and what
//!   leaves it is its input through a dry line of exactly its latency — **a footswitch never
//!   changes the board's latency**.
//! - **Latency** = Σ Pre + max(A, B) + Σ Post, bypassed pedals included; the shorter lane is
//!   delayed to meet the longer one. The lines (each latent pedal's dry line, the lane's
//!   alignment, the board's own dry line for `mix` and the dip) are one buffer asked for as
//!   `MemoryNeed::Tabula(frames)` and built off the audio thread; a line that has not
//!   arrived yet is no delay.
//! - **Structure.** A pedal added fades in where it stands; one taken out fades out where it
//!   stood and is then handed back (`take_retired` → `GarbageEffect`). A new order (or a
//!   pedal onto another lane) dips the board to its dry signal for [`FADE_MS`] on each side
//!   of the change, as Fragor's chain does. Everything structural happens on a grid of
//!   [`GRID`] samples counted from the stream's start, so the output does not depend on the
//!   block size.
//! - **Fan-out**: the key to every running pedal, each note to every running pedal at its
//!   sample, the song position to each as late as it hears it (`set_position_late`).
//! - **Rest**: when nothing fades or dips, every running pedal rests and every line is
//!   silent; `skip` goes to the running pedals and the lines.

use super::{FxNote, FxUnit, SongPosition, Telemetry, TELEMETRY_LEN};
use crate::effects::{EffectMemory, EffectUnit, MemoryNeed};
use cantus_core::fx::tabula::{self as model, Path, Pedal, PedalId, MAX_PEDALS};
use cantus_core::{EffectId, EffectSlot, EffectSpec};

/// A footswitch's crossfade, a pedal's fade in or out, and each half of a reorder's dip.
pub const FADE_MS: f32 = 10.0;
/// How long the board's own gains, `mix` and `ab` take to glide to a new value.
pub const GLIDE_MS: f32 = 20.0;
/// Structural changes land on this grid, counted from the stream's start.
pub const GRID: usize = 16;
/// The most frames the board's working buffers hold; a longer block is run in pieces.
pub const CHUNK: usize = 1024;
/// Pedals held at once: the board's sixteen and as many fading out.
const ROOM: usize = 2 * MAX_PEDALS;
/// Notes held for the next block.
const NOTES: usize = 256;
/// Lines: the dry, two alignments, one per pedal.
const REGIONS: usize = 3 + ROOM;

/// Where the board's telemetry keeps what (`FxUnit::telemetry`): per pedal in running order
/// its id + 1 (0: none), its input's peak and its output's, then the board's own.
pub mod tel {
    /// How many pedals follow.
    pub const COUNT: usize = 0;
    /// The first pedal's entry: id + 1, in peak, out peak.
    pub const PEDALS: usize = 1;
    pub const STRIDE: usize = 3;
    /// The board's input and output peaks, its latency in samples, and the dip (1: none).
    pub const IN: usize = 49;
    pub const OUT: usize = 50;
    pub const LATENCY: usize = 51;
    pub const DIP: usize = 52;
}

/// The board's own parameters as the DSP wants them.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Settings {
    pub in_db: f32,
    pub out_db: f32,
    pub mix: f32,
    pub ab: f32,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            in_db: 0.0,
            out_db: 0.0,
            mix: 1.0,
            ab: 0.5,
        }
    }
}

/// The board's own parameters, read from its spec.
pub fn settings(spec: &EffectSpec) -> Settings {
    Settings {
        in_db: spec.get("in"),
        out_db: spec.get("out"),
        mix: spec.get("mix"),
        ab: spec.get("ab"),
    }
}

/// Exactly 1 at 0 dB.
fn gain_of(db: f32) -> f32 {
    if db == 0.0 {
        1.0
    } else {
        cantus_dsp::math::db_to_gain(db)
    }
}

/// A pedal's unit, built off the audio thread from its spec: for [`Unit::build`] and for
/// `EngineMessage::AddPedal`. Its id is the board's (`effect`); it is marked `replaced`, so
/// when it comes back as garbage the handle never forgets its board's id.
pub fn pedal_unit(effect: EffectId, pedal: &Pedal, sample_rate: f32) -> Box<EffectUnit> {
    let slot = EffectSlot {
        id: effect,
        bypass: false,
        mix: 1.0,
        sidechain: None,
        midi_from: None,
        spec: pedal.spec.clone(),
    };
    let mut u = Box::new(EffectUnit::new(&slot, sample_rate));
    u.replaced = true;
    u
}

/// The pedals the engine runs, board by board: what the handle sends before a snapshot
/// (`EngineHandle::send_snapshot`). A pedal is known by its board, its id and its kind (an
/// id's kind never changes, but a board preset may reuse an id for another kind).
#[derive(Debug, Default)]
pub struct KnownPedals(std::collections::HashSet<(EffectId, PedalId, cantus_core::EffectKind)>);

impl KnownPedals {
    /// Before a snapshot holding `slots` goes: forget every pedal it no longer holds; a
    /// board sent whole now (`sent_whole`) brings its pedals; for every other board, a unit
    /// for each pedal the engine has not got, to send as `AddPedal` before the snapshot.
    pub fn update<'a>(
        &mut self,
        slots: impl Iterator<Item = &'a EffectSlot> + Clone,
        sent_whole: impl Fn(EffectId) -> bool,
        sample_rate: f32,
    ) -> Vec<(EffectId, PedalId, Box<EffectUnit>)> {
        let held = |e: EffectId, id: PedalId, k: cantus_core::EffectKind| {
            slots.clone().any(|s| {
                s.id == e
                    && s.spec
                        .pedals()
                        .iter()
                        .any(|p| p.id == id && p.spec.kind == k)
            })
        };
        self.0.retain(|&(e, id, k)| held(e, id, k));
        let mut out = Vec::new();
        for s in slots {
            let whole = sent_whole(s.id);
            for p in s.spec.pedals().iter().take(MAX_PEDALS) {
                if !model::pedal_allowed(p.spec.kind) {
                    continue;
                }
                if self.0.insert((s.id, p.id, p.spec.kind)) && !whole {
                    out.push((s.id, p.id, pedal_unit(s.id, p, sample_rate)));
                }
            }
        }
        out
    }

    /// Forget a pedal the engine could not be sent (its message found no room): it goes
    /// again with the next snapshot.
    pub fn forget(&mut self, effect: EffectId, pedal: PedalId) {
        self.0.retain(|&(e, id, _)| !(e == effect && id == pedal));
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

/// The board's lines (`MemoryNeed::Tabula(frames)`): built off the audio thread, `frames`
/// long per channel, carved into the lines the board's structure needs.
#[derive(Clone, Debug)]
pub struct Lines {
    pub left: Box<[f32]>,
    pub right: Box<[f32]>,
}

impl Lines {
    pub fn new(frames: usize) -> Self {
        Lines {
            left: vec![0.0; frames].into_boxed_slice(),
            right: vec![0.0; frames].into_boxed_slice(),
        }
    }
}

/// A value gliding to its target, one sample at a time (so the result does not depend on
/// how the stream is cut); it lands exactly on the target.
#[derive(Copy, Clone, Debug, PartialEq)]
struct Glide {
    v: f32,
    t: f32,
}

impl Glide {
    fn at(v: f32) -> Self {
        Glide { v, t: v }
    }

    fn settled(&self) -> bool {
        self.v == self.t
    }

    /// Settled at exactly `x`.
    fn rests_at(&self, x: f32) -> bool {
        self.v == x && self.t == x
    }

    /// The value for this sample, then one step on.
    #[inline]
    fn tick(&mut self, c: f32) -> f32 {
        let v = self.v;
        if v != self.t {
            let n = self.t + (v - self.t) * c;
            self.v = if (n - self.t).abs() <= 1e-6 {
                self.t
            } else {
                n
            };
        }
        v
    }
}

/// One pedal of a running board: its id and its unit (what `FxUnit::pedals_mut` hands the
/// processor, to ask and supply its memory), and how it runs.
#[derive(Debug)]
pub struct Kid {
    pub pedal: PedalId,
    pub unit: Box<EffectUnit>,
    /// Its own, for its lines: a pedal fading out and its successor with the same id
    /// never share one.
    serial: u32,
    /// The footswitch.
    on: bool,
    /// Taken off the board: fading out, then handed back.
    leaving: bool,
    /// Adopted, and no spec has named it yet.
    fresh: bool,
    /// Named by the spec being configured.
    seen: bool,
    /// 1: it sounds; 0: it is a wire (or its dry line).
    fade: f32,
    /// Off, faded, without latency: not run (and reset, so it starts as new).
    stopped: bool,
    peak_in: f32,
    peak_out: f32,
}

impl Kid {
    fn new(pedal: PedalId, unit: Box<EffectUnit>, serial: u32) -> Self {
        Kid {
            pedal,
            unit,
            serial,
            on: true,
            leaving: false,
            fresh: true,
            seen: false,
            fade: 0.0,
            stopped: false,
            peak_in: 0.0,
            peak_out: 0.0,
        }
    }

    /// Where its fade goes.
    fn target(&self) -> f32 {
        if self.on && !self.leaving {
            1.0
        } else {
            0.0
        }
    }

    fn settled(&self) -> bool {
        self.fade == self.target()
    }

    /// Faded to a wire, with no latency to keep: ready to stop.
    fn idle(&self) -> bool {
        self.fade == 0.0 && self.target() == 0.0 && self.unit.latency() == 0
    }
}

/// A place in the running order: which kid, on which lane.
#[derive(Copy, Clone, Debug, PartialEq)]
struct Slot {
    kid: u8,
    path: Path,
}

const NO_SLOT: Slot = Slot {
    kid: 0,
    path: Path::Pre,
};

/// One line in the board's buffer: whose, where, how long, where it writes, and how many
/// frames of silence it has taken since its last sound (silent when that reaches `len`).
#[derive(Copy, Clone, Debug, Default, PartialEq)]
struct Region {
    owner: u32,
    off: usize,
    len: usize,
    pos: usize,
    quiet: usize,
}

/// The board's own dry line (its latency long).
const DRY: u32 = 0;
/// Lane A's alignment (A shorter than B), lane B's (B shorter).
const ALIGN_A: u32 = 1;
const ALIGN_B: u32 = 2;

/// A pedal's dry line.
fn kid_owner(serial: u32) -> u32 {
    serial.wrapping_add(3)
}

/// The lines a buffer is carved into (or wanted).
#[derive(Copy, Clone, Debug, PartialEq)]
struct Layout {
    r: [Region; REGIONS],
    n: usize,
}

impl Default for Layout {
    fn default() -> Self {
        Layout {
            r: [Region::default(); REGIONS],
            n: 0,
        }
    }
}

impl Layout {
    fn push(&mut self, owner: u32, len: usize) {
        if len > 0 && self.n < REGIONS {
            let off = self.total();
            self.r[self.n] = Region {
                owner,
                off,
                len,
                pos: 0,
                quiet: len,
            };
            self.n += 1;
        }
    }

    fn total(&self) -> usize {
        self.r[..self.n].iter().map(|r| r.len).sum()
    }

    fn find(&self, owner: u32, len: usize) -> Option<usize> {
        self.r[..self.n]
            .iter()
            .position(|r| r.owner == owner && r.len == len)
    }

    fn regions(&self) -> &[Region] {
        &self.r[..self.n]
    }
}

/// A working buffer: two channels of [`CHUNK`] frames.
struct Pair([[f32; CHUNK]; 2]);

impl Pair {
    fn new() -> Box<Self> {
        Box::new(Pair([[0.0; CHUNK]; 2]))
    }

    /// Both channels' first `m` frames.
    fn split(&mut self, m: usize) -> (&mut [f32], &mut [f32]) {
        let [l, r] = &mut self.0;
        (&mut l[..m], &mut r[..m])
    }
}

impl std::fmt::Debug for Pair {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Pair").finish_non_exhaustive()
    }
}

/// Run `x` through a line in place (it comes out `len` frames late), counting its silence.
#[inline]
fn run_line(lines: &mut Lines, r: &mut Region, xl: &mut [f32], xr: &mut [f32]) {
    let (bl, br) = (
        &mut lines.left[r.off..r.off + r.len],
        &mut lines.right[r.off..r.off + r.len],
    );
    for (a, b) in xl.iter_mut().zip(xr.iter_mut()) {
        let p = r.pos;
        let (yl, yr) = (bl[p], br[p]);
        bl[p] = *a;
        br[p] = *b;
        r.quiet = if a.to_bits() == 0 && b.to_bits() == 0 {
            (r.quiet + 1).min(r.len)
        } else {
            0
        };
        *a = yl;
        *b = yr;
        r.pos = if p + 1 == r.len { 0 } else { p + 1 };
    }
}

/// The highest magnitude in two buffers (eight lanes, so it vectorises).
#[inline]
fn peak(l: &[f32], r: &[f32]) -> f32 {
    let mut acc = [0.0f32; 8];
    for x in [l, r] {
        let mut c = x.chunks_exact(8);
        for v in &mut c {
            for j in 0..8 {
                acc[j] = acc[j].max(v[j].abs());
            }
        }
        for (j, v) in c.remainder().iter().enumerate() {
            acc[j] = acc[j].max(v.abs());
        }
    }
    acc.iter().fold(0.0f32, |m, v| m.max(*v))
}

#[derive(Debug)]
pub struct Unit {
    sr: f32,
    /// The pedals: those on the board first (`..active`), then those fading out.
    kids: Vec<Kid>,
    active: usize,
    /// The running order, by lane.
    order: [Slot; ROOM],
    n: usize,
    /// The order a dip is taking the board to.
    next: [Slot; ROOM],
    n_next: usize,
    pending: bool,
    /// The reorder's dip: 1 none, 0 the dry signal; going down (−1), up (1) or still.
    dip: f32,
    dip_dir: i8,
    fade_step: f32,
    glide: f32,
    in_g: Glide,
    out_g: Glide,
    mix: Glide,
    ab: Glide,
    lines: Option<Box<Lines>>,
    layout: Layout,
    retired: Vec<Box<EffectUnit>>,
    notes: [(usize, FxNote); NOTES],
    n_notes: usize,
    /// A pedal's input (its wire, or its dry line's), lane B, and the board's dry signal.
    xs: Option<Box<Pair>>,
    bs: Option<Box<Pair>>,
    ds: Option<Box<Pair>>,
    /// Samples since the stream's start: the grid.
    clock: u64,
    serial: u32,
    peak_in: f32,
    peak_out: f32,
}

/// The latency of each lane: Pre, A, B, Post.
#[derive(Copy, Clone, Debug, Default, PartialEq)]
struct Lats {
    pre: usize,
    a: usize,
    b: usize,
    post: usize,
    split: bool,
}

impl Lats {
    fn total(&self) -> usize {
        self.pre + if self.split { self.a.max(self.b) } else { 0 } + self.post
    }
}

impl Unit {
    /// An empty board.
    pub fn new(sample_rate: f32) -> Self {
        let sr = sample_rate.max(1.0);
        Unit {
            sr,
            kids: Vec::with_capacity(ROOM),
            active: 0,
            order: [NO_SLOT; ROOM],
            n: 0,
            next: [NO_SLOT; ROOM],
            n_next: 0,
            pending: false,
            dip: 1.0,
            dip_dir: 0,
            fade_step: 1.0 / (FADE_MS * 0.001 * sr).max(1.0),
            glide: cantus_dsp::math::exp(-1.0 / (GLIDE_MS * 0.001 * sr).max(1.0)),
            in_g: Glide::at(1.0),
            out_g: Glide::at(1.0),
            mix: Glide::at(1.0),
            ab: Glide::at(0.5),
            lines: None,
            layout: Layout::default(),
            retired: Vec::with_capacity(ROOM),
            notes: [(0, FxNote::AllOff); NOTES],
            n_notes: 0,
            xs: Some(Pair::new()),
            bs: Some(Pair::new()),
            ds: Some(Pair::new()),
            clock: 0,
            serial: 0,
            peak_in: 0.0,
            peak_out: 0.0,
        }
    }

    /// The board for `spec`, every pedal built and settled (the UI thread).
    pub fn build(spec: &EffectSpec, sample_rate: f32) -> Self {
        let mut u = Unit::new(sample_rate);
        for p in spec.pedals().iter().take(MAX_PEDALS) {
            if model::pedal_allowed(p.spec.kind) {
                let k = pedal_unit(EffectId(0), p, sample_rate);
                drop(FxUnit::adopt(&mut u, p.id, k));
            }
        }
        u.configure(spec);
        u.settle();
        u
    }

    /// Swap two kids, their places in both orders following them.
    fn swap_kids(&mut self, a: usize, b: usize) {
        if a == b {
            return;
        }
        self.kids.swap(a, b);
        let (a, b) = (a as u8, b as u8);
        for s in self.order[..self.n]
            .iter_mut()
            .chain(self.next[..self.n_next].iter_mut())
        {
            if s.kid == a {
                s.kid = b;
            } else if s.kid == b {
                s.kid = a;
            }
        }
    }

    /// Kid `i` (on the board) taken off it: it fades out from where it stands.
    fn make_leaving(&mut self, i: usize) {
        let last = self.active - 1;
        self.swap_kids(i, last);
        self.active = last;
        self.kids[last].leaving = true;
        self.kids[last].stopped = self.kids[last].stopped && self.kids[last].idle();
    }

    /// Hand kid `i` back (`take_retired`); `false` when there is no room to yet.
    fn retire(&mut self, i: usize) -> bool {
        if self.retired.len() >= self.retired.capacity() {
            return false;
        }
        let k = self.kids.remove(i);
        if i < self.active {
            self.active -= 1;
        }
        let gone = i as u8;
        let mut m = 0;
        for j in 0..self.n {
            let s = self.order[j];
            if s.kid != gone {
                self.order[m] = Slot {
                    kid: if s.kid > gone { s.kid - 1 } else { s.kid },
                    ..s
                };
                m += 1;
            }
        }
        self.n = m;
        let mut m = 0;
        for j in 0..self.n_next {
            let s = self.next[j];
            if s.kid != gone {
                self.next[m] = Slot {
                    kid: if s.kid > gone { s.kid - 1 } else { s.kid },
                    ..s
                };
                m += 1;
            }
        }
        self.n_next = m;
        let mut unit = k.unit;
        unit.stop();
        self.retired.push(unit);
        true
    }

    /// Hand back every pedal fading out that has faded (`all`: every one, sounding or not —
    /// the dip's bottom, a reset).
    fn retire_faded(&mut self, all: bool) {
        let mut i = self.active;
        while i < self.kids.len() {
            let k = &self.kids[i];
            let in_order = self.order[..self.n].iter().any(|s| s.kid as usize == i);
            if (all || k.fade == 0.0 || !in_order) && self.retire(i) {
                continue;
            }
            i += 1;
        }
    }

    /// Each lane's latency in the running order (bypassed pedals included).
    fn lats(&self) -> Lats {
        let mut l = Lats::default();
        for s in &self.order[..self.n] {
            let k = &self.kids[s.kid as usize];
            let d = if k.stopped { 0 } else { k.unit.latency() };
            match s.path {
                Path::Pre => l.pre += d,
                Path::A => {
                    l.a += d;
                    l.split = true;
                }
                Path::B => {
                    l.b += d;
                    l.split = true;
                }
                Path::Post => l.post += d,
            }
        }
        l
    }

    /// The lines the running order needs.
    fn wanted(&self) -> Layout {
        let lats = self.lats();
        let mut w = Layout::default();
        w.push(DRY, lats.total());
        if lats.split {
            if lats.a < lats.b {
                w.push(ALIGN_A, lats.b - lats.a);
            } else {
                w.push(ALIGN_B, lats.a - lats.b);
            }
        }
        for s in &self.order[..self.n] {
            let k = &self.kids[s.kid as usize];
            if !k.stopped {
                w.push(kid_owner(k.serial), k.unit.latency());
            }
        }
        w
    }

    /// Lines no longer wanted are cleared, so one that is wanted again starts silent.
    fn clear_unwanted(&mut self) {
        let w = self.wanted();
        let Some(lines) = self.lines.as_deref_mut() else {
            return;
        };
        for r in self.layout.r[..self.layout.n].iter_mut() {
            if w.find(r.owner, r.len).is_none() && r.quiet < r.len {
                lines.left[r.off..r.off + r.len].fill(0.0);
                lines.right[r.off..r.off + r.len].fill(0.0);
                r.quiet = r.len;
            }
        }
    }

    /// Whether the new order keeps the running order's pedals in the same order on the same
    /// lanes (pedals added or taken out aside): then it applies at once, without a dip.
    fn same_structure(&self, new: &[Slot]) -> bool {
        let mut j = 0;
        for s in &self.order[..self.n] {
            if self.kids[s.kid as usize].leaving {
                continue;
            }
            match new.iter().position(|x| x.kid == s.kid) {
                Some(q) if q >= j && new[q].path == s.path => j = q + 1,
                Some(_) => return false,
                None => {}
            }
        }
        true
    }

    /// The new order with every pedal fading out kept where it stood.
    fn merge(&mut self, new: &[Slot]) {
        let mut out = [NO_SLOT; ROOM];
        let mut m = 0;
        let mut j = 0;
        for s in self.order[..self.n].iter().copied() {
            match new.iter().position(|x| x.kid == s.kid) {
                Some(q) => {
                    for x in &new[j..=q] {
                        out[m] = *x;
                        m += 1;
                    }
                    j = q + 1;
                }
                None => {
                    out[m] = s;
                    m += 1;
                }
            }
        }
        for x in &new[j..] {
            out[m] = *x;
            m += 1;
        }
        self.order = out;
        self.n = m;
    }

    /// The dip's bottom: the new order, every pedal fading out gone, each pedal's fade where
    /// its footswitch says.
    fn apply_pending(&mut self) {
        self.pending = false;
        self.retire_faded(true);
        self.order = self.next;
        self.n = self.n_next;
        self.n_next = 0;
        for j in 0..self.n {
            let k = &mut self.kids[self.order[j].kid as usize];
            k.fade = k.target();
        }
        self.stop_idle();
        self.clear_unwanted();
    }

    /// Pedals faded to a wire with no latency stop running, made new for when they return.
    fn stop_idle(&mut self) {
        for j in 0..self.n {
            let k = &mut self.kids[self.order[j].kid as usize];
            if !k.stopped && !k.leaving && k.idle() {
                k.stopped = true;
                k.unit.reset();
            }
        }
    }

    /// Whether anything structural is under way (fades, the dip, a pedal leaving or about
    /// to stop): the board then runs on the grid.
    fn moving(&self) -> bool {
        self.dip_dir != 0
            || self.pending
            || self.kids.len() > self.active
            || self.order[..self.n].iter().any(|s| {
                let k = &self.kids[s.kid as usize];
                !k.settled() || (!k.stopped && k.idle())
            })
    }

    /// On a grid point, while something moves: what has finished fading stops or goes, and
    /// the dip turns.
    fn at_grid(&mut self) {
        self.stop_idle();
        let before = self.kids.len();
        self.retire_faded(false);
        if self.dip_dir == -1 && self.dip <= 0.0 {
            self.dip = 0.0;
            if self.pending {
                self.apply_pending();
            }
            self.dip_dir = 1;
        }
        if self.dip_dir == 1 && self.dip >= 1.0 {
            self.dip = 1.0;
            self.dip_dir = 0;
        }
        if self.kids.len() != before {
            self.clear_unwanted();
        }
    }

    /// Everything where it is going, at once: fades, the dip, the glides (a new unit, a
    /// reset).
    fn settle(&mut self) {
        if self.pending {
            self.apply_pending();
        }
        self.retire_faded(true);
        self.dip = 1.0;
        self.dip_dir = 0;
        for j in 0..self.n {
            let k = &mut self.kids[self.order[j].kid as usize];
            k.fade = k.target();
            k.stopped = false;
        }
        self.stop_idle();
        for g in [&mut self.in_g, &mut self.out_g, &mut self.mix, &mut self.ab] {
            g.v = g.t;
        }
    }

    /// Whether the board is only its pedals one after another, each sounding and settled:
    /// then it runs each on the whole block, with no working buffer.
    fn plain(&self) -> bool {
        self.dip_dir == 0
            && !self.pending
            && self.mix.rests_at(1.0)
            && self.kids.len() == self.active
            && self.order[..self.n].iter().all(|s| {
                let k = &self.kids[s.kid as usize];
                matches!(s.path, Path::Pre | Path::Post)
                    && (k.stopped || (k.fade == 1.0 && k.settled() && k.unit.latency() == 0))
            })
    }
}

impl Unit {
    /// Run a line on `x` in place; `false` when it has not arrived (then no delay).
    fn line(&mut self, owner: u32, len: usize, xl: &mut [f32], xr: &mut [f32]) -> bool {
        if len == 0 {
            return true;
        }
        let Some(i) = self.layout.find(owner, len) else {
            return false;
        };
        let Some(lines) = self.lines.as_deref_mut() else {
            return false;
        };
        run_line(lines, &mut self.layout.r[i], xl, xr);
        true
    }

    /// The pedals of one lane on `l`/`r`, in order: `lane` is the peak arriving there;
    /// returns the peak leaving.
    fn chain(
        &mut self,
        path: Path,
        l: &mut [f32],
        r: &mut [f32],
        key: Option<(&[f32], &[f32])>,
        x: &mut Pair,
        mut lane: f32,
    ) -> f32 {
        let m = l.len();
        let st = self.fade_step;
        for j in 0..self.n {
            let s = self.order[j];
            if s.path != path {
                continue;
            }
            let i = s.kid as usize;
            let (serial, lat, tgt, fade, stopped) = {
                let k = &mut self.kids[i];
                k.peak_in = k.peak_in.max(lane);
                (k.serial, k.unit.latency(), k.target(), k.fade, k.stopped)
            };
            if stopped {
                let k = &mut self.kids[i];
                k.peak_out = k.peak_out.max(lane);
                continue;
            }
            // Settled and sounding: the pedal alone, on the very buffers.
            let direct = fade == 1.0 && tgt == 1.0;
            let need = !direct || lat > 0;
            let (xl, xr) = x.split(if need { m } else { 0 });
            if need {
                xl.copy_from_slice(l);
                xr.copy_from_slice(r);
                if lat > 0 {
                    // Its dry line runs whatever the footswitch says: in step when needed.
                    self.line(kid_owner(serial), lat, xl, xr);
                }
            }
            let k = &mut self.kids[i];
            k.unit.process_keyed(l, r, key);
            if !direct {
                let mut f = fade;
                for t in 0..m {
                    l[t] = xl[t] + f * (l[t] - xl[t]);
                    r[t] = xr[t] + f * (r[t] - xr[t]);
                    if f != tgt {
                        f = if tgt > f {
                            (f + st).min(1.0)
                        } else {
                            (f - st).max(0.0)
                        };
                    }
                }
                k.fade = f;
            }
            lane = peak(l, r);
            k.peak_out = k.peak_out.max(lane);
        }
        lane
    }

    /// One piece of a block (at most [`CHUNK`] frames unless the board is [`Self::plain`]).
    fn run(&mut self, l: &mut [f32], r: &mut [f32], key: Option<(&[f32], &[f32])>) {
        let m = l.len();
        let (Some(mut xs), Some(mut bs), Some(mut ds)) =
            (self.xs.take(), self.bs.take(), self.ds.take())
        else {
            return;
        };
        let lats = self.lats();
        let total = lats.total();
        let c = self.glide;
        let dipping = self.dip_dir != 0 || self.dip < 1.0 || self.pending;
        let mixing = !self.mix.rests_at(1.0);
        // The board's dry signal: its input, as late as the board (in step even when no
        // knob asks for it, so a dip or a mix never starts on a stale line).
        let dry = total > 0 || mixing || dipping;
        if dry {
            let (dl, dr) = ds.split(m);
            dl.copy_from_slice(l);
            dr.copy_from_slice(r);
            self.line(DRY, total, dl, dr);
        }
        let pin = peak(l, r);
        self.peak_in = self.peak_in.max(pin);
        let mut lane = pin;
        if !self.in_g.rests_at(1.0) {
            for t in 0..m {
                let g = self.in_g.tick(c);
                l[t] *= g;
                r[t] *= g;
            }
            lane = peak(l, r);
        }
        lane = self.chain(Path::Pre, l, r, key, &mut xs, lane);
        if lats.split {
            let (bl, br) = bs.split(m);
            bl.copy_from_slice(l);
            br.copy_from_slice(r);
            self.chain(Path::A, l, r, key, &mut xs, lane);
            self.chain(Path::B, bl, br, key, &mut xs, lane);
            // The shorter lane waits for the longer.
            if lats.a > lats.b {
                self.line(ALIGN_B, lats.a - lats.b, bl, br);
            } else if lats.b > lats.a {
                self.line(ALIGN_A, lats.b - lats.a, l, r);
            }
            if self.ab.rests_at(1.0) {
                l.copy_from_slice(bl);
                r.copy_from_slice(br);
            } else if !self.ab.rests_at(0.0) {
                // Equal lanes come out as they are, to the bit (an empty split).
                for t in 0..m {
                    let g = self.ab.tick(c);
                    l[t] += g * (bl[t] - l[t]);
                    r[t] += g * (br[t] - r[t]);
                }
            }
            lane = peak(l, r);
        } else {
            self.ab.v = self.ab.t;
        }
        self.chain(Path::Post, l, r, key, &mut xs, lane);
        if dry {
            let (dl, dr) = ds.split(m);
            if mixing {
                for t in 0..m {
                    let g = self.mix.tick(c);
                    l[t] = l[t] * g + dl[t] * (1.0 - g);
                    r[t] = r[t] * g + dr[t] * (1.0 - g);
                }
            }
            if dipping {
                let st = self.fade_step;
                for t in 0..m {
                    let d = self.dip;
                    if d < 1.0 {
                        l[t] = l[t] * d + dl[t] * (1.0 - d);
                        r[t] = r[t] * d + dr[t] * (1.0 - d);
                    }
                    self.dip = match self.dip_dir {
                        -1 => (d - st).max(0.0),
                        1 => (d + st).min(1.0),
                        _ => d,
                    };
                }
            }
        }
        if !self.out_g.rests_at(1.0) {
            for t in 0..m {
                let g = self.out_g.tick(c);
                l[t] *= g;
                r[t] *= g;
            }
        }
        self.peak_out = self.peak_out.max(peak(l, r));
        self.xs = Some(xs);
        self.bs = Some(bs);
        self.ds = Some(ds);
    }

    /// The notes due in `[at, at + m)`, to every running pedal at its sample.
    fn deliver(&mut self, at: usize, m: usize) {
        for q in 0..self.n_notes {
            let (off, note) = self.notes[q];
            if off < at || off >= at + m {
                continue;
            }
            for j in 0..self.n {
                let k = &mut self.kids[self.order[j].kid as usize];
                if !k.stopped {
                    k.unit.note(off - at, note);
                }
            }
        }
    }
}

impl FxUnit for Unit {
    fn fork(&self) -> Option<Box<dyn FxUnit>> {
        // Every pedal must fork for the board to.
        let mut kids = Vec::with_capacity(ROOM);
        for k in &self.kids {
            kids.push(Kid {
                pedal: k.pedal,
                unit: Box::new(k.unit.fork()?),
                serial: k.serial,
                on: k.on,
                leaving: k.leaving,
                fresh: k.fresh,
                seen: k.seen,
                fade: k.fade,
                stopped: k.stopped,
                peak_in: k.peak_in,
                peak_out: k.peak_out,
            });
        }
        Some(Box::new(Unit {
            sr: self.sr,
            kids,
            active: self.active,
            order: self.order,
            n: self.n,
            next: self.next,
            n_next: self.n_next,
            pending: self.pending,
            dip: self.dip,
            dip_dir: self.dip_dir,
            fade_step: self.fade_step,
            glide: self.glide,
            in_g: self.in_g,
            out_g: self.out_g,
            mix: self.mix,
            ab: self.ab,
            lines: self.lines.clone(),
            layout: self.layout,
            retired: Vec::with_capacity(ROOM),
            notes: self.notes,
            n_notes: self.n_notes,
            xs: Some(Pair::new()),
            bs: Some(Pair::new()),
            ds: Some(Pair::new()),
            clock: self.clock,
            serial: self.serial,
            peak_in: self.peak_in,
            peak_out: self.peak_out,
        }))
    }

    fn configure(&mut self, spec: &EffectSpec) {
        let s = settings(spec);
        let unit = |v: f32, d: f32| if v.is_finite() { v.clamp(0.0, 1.0) } else { d };
        self.in_g.t = gain_of(s.in_db);
        self.out_g.t = gain_of(s.out_db);
        self.mix.t = unit(s.mix, 1.0);
        self.ab.t = unit(s.ab, 0.5);
        for k in &mut self.kids[..self.active] {
            k.seen = false;
        }
        // Each pedal the spec names, configured; what it names in its order.
        let mut named = [(PedalId(0), cantus_core::EffectKind::Board, Path::Pre); MAX_PEDALS];
        let mut nn = 0;
        for p in spec.pedals().iter().take(MAX_PEDALS) {
            if !model::pedal_allowed(p.spec.kind) {
                continue;
            }
            let Some(i) = (0..self.active).find(|&i| {
                let k = &self.kids[i];
                !k.seen && k.pedal == p.id && k.unit.kind == p.spec.kind
            }) else {
                continue;
            };
            let k = &mut self.kids[i];
            k.seen = true;
            k.fresh = false;
            k.on = p.on;
            k.unit.configure(&p.spec);
            if k.stopped && (k.on || k.unit.latency() > 0) {
                k.stopped = false;
            }
            named[nn] = (p.id, p.spec.kind, p.path);
            nn += 1;
        }
        // What it no longer names fades out (a pedal adopted and not yet named waits).
        let mut i = 0;
        while i < self.active {
            if !self.kids[i].seen && !self.kids[i].fresh {
                self.make_leaving(i);
            } else {
                i += 1;
            }
        }
        let mut new = [NO_SLOT; ROOM];
        let mut m = 0;
        for &(id, kind, path) in &named[..nn] {
            if let Some(i) = (0..self.active).find(|&i| {
                let k = &self.kids[i];
                k.seen && k.pedal == id && k.unit.kind == kind
            }) {
                new[m] = Slot { kid: i as u8, path };
                m += 1;
            }
        }
        if self.same_structure(&new[..m]) {
            // Added and taken out only (or a reorder taken back before its dip's bottom).
            if self.pending {
                self.pending = false;
                self.n_next = 0;
                if self.dip_dir == -1 {
                    self.dip_dir = 1;
                }
            }
            self.merge(&new[..m]);
        } else {
            self.next[..m].copy_from_slice(&new[..m]);
            self.n_next = m;
            self.pending = true;
            self.dip_dir = -1;
        }
        // Pedals taken out that make no sound go at once.
        let mut i = self.active;
        while i < self.kids.len() {
            let k = &self.kids[i];
            let in_order = self.order[..self.n].iter().any(|s| s.kid as usize == i);
            if (k.stopped || !in_order) && self.retire(i) {
                continue;
            }
            i += 1;
        }
        self.clear_unwanted();
    }

    fn process(&mut self, left: &mut [f32], right: &mut [f32], key: Option<(&[f32], &[f32])>) {
        let n = left.len().min(right.len());
        for k in &mut self.kids {
            k.peak_in = 0.0;
            k.peak_out = 0.0;
        }
        self.peak_in = 0.0;
        self.peak_out = 0.0;
        let mut at = 0;
        while at < n {
            let moving = self.moving();
            let mut m = n - at;
            if moving || !self.plain() {
                m = m.min(CHUNK);
            }
            if moving {
                m = m.min(GRID - (self.clock % GRID as u64) as usize);
            }
            self.deliver(at, m);
            let key = key.map(|(a, b)| (&a[at..at + m], &b[at..at + m]));
            self.run(&mut left[at..at + m], &mut right[at..at + m], key);
            self.clock = self.clock.wrapping_add(m as u64);
            if moving && self.clock.is_multiple_of(GRID as u64) {
                self.at_grid();
            }
            at += m;
        }
        self.n_notes = 0;
    }

    fn reset(&mut self) {
        for k in &mut self.kids {
            k.unit.reset();
        }
        self.settle();
        if let Some(lines) = self.lines.as_deref_mut() {
            lines.left.fill(0.0);
            lines.right.fill(0.0);
        }
        for r in self.layout.r[..self.layout.n].iter_mut() {
            r.pos = 0;
            r.quiet = r.len;
        }
        self.n_notes = 0;
        self.clock = 0;
        self.peak_in = 0.0;
        self.peak_out = 0.0;
    }

    fn at_rest(&self) -> bool {
        self.rest_blocker().is_none()
    }

    fn rest_blocker(&self) -> Option<&'static str> {
        if self.moving() {
            return Some("a pedal fades, or the board dips");
        }
        if !(self.in_g.settled() && self.out_g.settled() && self.mix.settled() && self.ab.settled())
        {
            return Some("a level glides");
        }
        if self.n_notes > 0 {
            return Some("notes wait");
        }
        for s in &self.order[..self.n] {
            let k = &self.kids[s.kid as usize];
            if !k.stopped {
                if let Some(why) = k.unit.rest_blocker() {
                    return Some(why);
                }
            }
        }
        let w = self.wanted();
        if self
            .layout
            .regions()
            .iter()
            .any(|r| w.find(r.owner, r.len).is_some() && r.quiet < r.len)
        {
            return Some("a line still holds sound");
        }
        None
    }

    fn skip(&mut self, frames: usize) {
        for j in 0..self.n {
            let k = &mut self.kids[self.order[j].kid as usize];
            if !k.stopped {
                k.unit.skip(frames);
            }
        }
        for r in self.layout.r[..self.layout.n].iter_mut() {
            r.pos = (r.pos + frames % r.len) % r.len;
            r.quiet = r.len;
        }
        self.clock = self.clock.wrapping_add(frames as u64);
    }

    fn latency(&self) -> usize {
        self.lats().total()
    }

    fn telemetry(&self) -> Telemetry {
        let mut t = [0.0; TELEMETRY_LEN];
        let mut c = 0;
        for s in &self.order[..self.n] {
            let k = &self.kids[s.kid as usize];
            if k.leaving || c >= MAX_PEDALS {
                continue;
            }
            let at = tel::PEDALS + c * tel::STRIDE;
            t[at] = f32::from(k.pedal.0) + 1.0;
            t[at + 1] = k.peak_in;
            t[at + 2] = k.peak_out;
            c += 1;
        }
        t[tel::COUNT] = c as f32;
        t[tel::IN] = self.peak_in;
        t[tel::OUT] = self.peak_out;
        t[tel::LATENCY] = self.latency() as f32;
        t[tel::DIP] = self.dip;
        t
    }

    fn follows_song(&self) -> bool {
        self.order[..self.n].iter().any(|s| {
            let k = &self.kids[s.kid as usize];
            !k.stopped && k.unit.follows_song()
        })
    }

    fn set_position_late(&mut self, late: &dyn Fn(usize) -> SongPosition) {
        let lats = self.lats();
        let lanes = if lats.split { lats.a.max(lats.b) } else { 0 };
        let (mut pre, mut a, mut b, mut post) = (0, 0, 0, 0);
        for j in 0..self.n {
            let s = self.order[j];
            let k = &mut self.kids[s.kid as usize];
            let (before, acc) = match s.path {
                Path::Pre => (pre, &mut pre),
                Path::A => (lats.pre + a, &mut a),
                Path::B => (lats.pre + b, &mut b),
                Path::Post => (lats.pre + lanes + post, &mut post),
            };
            if !k.stopped {
                if k.unit.follows_song() {
                    k.unit.set_position(&late(before));
                }
                *acc += k.unit.latency();
            }
        }
    }

    fn note(&mut self, offset: usize, note: FxNote) {
        if self.n_notes < NOTES {
            self.notes[self.n_notes] = (offset, note);
            self.n_notes += 1;
        }
    }

    fn stop(&mut self) {
        for k in &mut self.kids {
            k.unit.stop();
        }
    }

    fn memory_need(&self) -> Option<MemoryNeed> {
        let w = self.wanted();
        let here = w
            .regions()
            .iter()
            .all(|r| self.layout.find(r.owner, r.len).is_some() && self.lines.is_some());
        (!here).then(|| MemoryNeed::Tabula(w.total()))
    }

    fn supply(&mut self, memory: EffectMemory) -> Option<EffectMemory> {
        let EffectMemory::Tabula(mut new) = memory else {
            return Some(memory);
        };
        let w = self.wanted();
        let total = w.total();
        if new.left.len() < total || new.right.len() < total {
            return Some(EffectMemory::Tabula(new));
        }
        // Every line the new buffer carries over from the old keeps its sound and its place.
        let mut layout = w;
        if let Some(old) = self.lines.as_deref() {
            for r in layout.r[..layout.n].iter_mut() {
                if let Some(o) = self.layout.find(r.owner, r.len) {
                    let o = self.layout.r[o];
                    new.left[r.off..r.off + r.len].copy_from_slice(&old.left[o.off..o.off + o.len]);
                    new.right[r.off..r.off + r.len]
                        .copy_from_slice(&old.right[o.off..o.off + o.len]);
                    r.pos = o.pos;
                    r.quiet = o.quiet;
                }
            }
        }
        self.layout = layout;
        self.lines.replace(new).map(EffectMemory::Tabula)
    }

    fn adopt(&mut self, pedal: PedalId, unit: Box<EffectUnit>) -> Option<Box<EffectUnit>> {
        if self.kids.len() >= self.kids.capacity() || !model::pedal_allowed(unit.kind) {
            return Some(unit);
        }
        let serial = self.serial;
        self.serial = self.serial.wrapping_add(1);
        self.kids.push(Kid::new(pedal, unit, serial));
        let last = self.kids.len() - 1;
        self.swap_kids(self.active, last);
        self.active += 1;
        None
    }

    fn take_retired(&mut self) -> Option<Box<EffectUnit>> {
        self.retired.pop()
    }

    fn pedals_mut(&mut self) -> &mut [Kid] {
        &mut self.kids[..self.active]
    }

    fn pedal_telemetry(&self, pedal: PedalId) -> Option<Telemetry> {
        self.kids[..self.active]
            .iter()
            .find(|k| k.pedal == pedal)
            .and_then(|k| k.unit.telemetry())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cantus_core::EffectKind;

    #[test]
    fn the_tables_defaults_are_the_dsps_defaults() {
        let spec = EffectSpec::new(EffectKind::Board);
        assert_eq!(settings(&spec), Settings::default());
    }

    /// An empty board touches no sample, to the bit.
    #[test]
    fn an_empty_board_is_a_wire() {
        let mut u = Unit::build(&EffectSpec::new(EffectKind::Board), 48_000.0);
        let x: Vec<f32> = (0..3000).map(|i| (i as f32 * 0.37).fract() - 0.5).collect();
        let (mut l, mut r) = (x.clone(), x.clone());
        u.process(&mut l, &mut r, None);
        assert!(l
            .iter()
            .chain(&r)
            .zip(x.iter().chain(&x))
            .all(|(a, b)| a.to_bits() == b.to_bits()));
        assert!(u.at_rest());
        assert_eq!(u.latency(), 0);
    }

    /// The guitar rig is built whole: four pedals, each its own unit.
    #[test]
    fn the_rig_is_built_with_its_pedals() {
        let mut u = Unit::build(&cantus_core::fx::tabula::guitar_rig(), 48_000.0);
        let ids: Vec<u16> = u.pedals_mut().iter().map(|k| k.pedal.0).collect();
        assert_eq!(ids, [0, 1, 2, 3]);
    }
}
