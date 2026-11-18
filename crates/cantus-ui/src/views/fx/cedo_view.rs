//! Cedo's memory between frames (design sheet v1): one block's numbers as the display
//! reads them from the engine's telemetry (`cantus_engine::fx::cedo::t`), and the last four
//! seconds of them — the reduction, the amount, the key's level, the hits and the shape's
//! phase — in a ring that never allocates after it is made.

use cantus_engine::fx::cedo::t;

/// The floor every level is clamped to, dBFS.
pub const FLOOR_DB: f32 = -96.0;

/// One block's numbers.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Reading {
    pub seen: bool,
    /// The deepest reduction in the block, dB (≥ 0).
    pub gr: f32,
    /// The amount, 0 … 1.
    pub amount: f32,
    /// The filtered key's peak, dBFS; whether it reached the threshold.
    pub key_db: f32,
    pub triggered: bool,
    /// Where the shape's cycle is (0 … 1), negative when none runs.
    pub phase: f32,
    pub in_db: f32,
    pub out_db: f32,
    /// The latency, samples, and the rate it is counted at.
    pub latency: f32,
    pub sample_rate: f32,
    pub tempo: f32,
    pub playing: bool,
    pub keyed: bool,
    /// The trigger in force, 0 … 2.
    pub trigger: usize,
}

impl Default for Reading {
    fn default() -> Self {
        Reading {
            seen: false,
            gr: 0.0,
            amount: 0.0,
            key_db: FLOOR_DB,
            triggered: false,
            phase: -1.0,
            in_db: FLOOR_DB,
            out_db: FLOOR_DB,
            latency: 0.0,
            sample_rate: 48_000.0,
            tempo: 120.0,
            playing: false,
            keyed: false,
            trigger: 1,
        }
    }
}

impl Reading {
    pub fn of(tel: &[f32; 64]) -> Reading {
        if tel[t::SAMPLE_RATE] <= 0.0 {
            return Reading::default();
        }
        let level = |x: f32| {
            if x.is_finite() {
                x.max(FLOOR_DB)
            } else {
                FLOOR_DB
            }
        };
        Reading {
            seen: true,
            gr: tel[t::GR].max(0.0),
            amount: tel[t::AMOUNT].clamp(0.0, 1.0),
            key_db: level(tel[t::KEY_DB]),
            triggered: tel[t::TRIGGERED] >= 0.5,
            phase: tel[t::PHASE],
            in_db: level(tel[t::IN_DB]),
            out_db: level(tel[t::OUT_DB]),
            latency: tel[t::LATENCY].max(0.0),
            sample_rate: tel[t::SAMPLE_RATE],
            tempo: tel[t::TEMPO],
            playing: tel[t::PLAYING] >= 0.5,
            keyed: tel[t::KEYED] >= 0.5,
            trigger: tel[t::TRIGGER].round().clamp(0.0, 2.0) as usize,
        }
    }
}

/// What starts a point: nothing, the key over its threshold, a cycle's start (the beat)
/// or a note's.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Default)]
pub enum Hit {
    #[default]
    None,
    Key,
    Cycle,
}

/// One point of the history.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Point {
    /// The deepest reduction since the point before, dB.
    pub gr: f32,
    /// The amount at the point, 0 … 1.
    pub amount: f32,
    /// The key's loudest peak since the point before, dBFS.
    pub key_db: f32,
    pub hit: Hit,
    /// The shape's phase at the point (negative: none).
    pub phase: f32,
}

impl Point {
    pub const QUIET: Point = Point {
        gr: 0.0,
        amount: 0.0,
        key_db: FLOOR_DB,
        hit: Hit::None,
        phase: -1.0,
    };
}

/// Points a second.
pub const RATE: f32 = 60.0;
/// The history's length: four seconds.
pub const HISTORY: usize = 240;

/// The display's memory: the history (a ring filled at [`RATE`]) and the last phase seen,
/// to tell a cycle's start.
#[derive(Debug)]
pub struct View {
    ring: [Point; HISTORY],
    head: usize,
    len: usize,
    acc: f32,
    pending: Point,
    last_phase: f32,
}

impl Default for View {
    fn default() -> Self {
        View {
            ring: [Point::QUIET; HISTORY],
            head: 0,
            len: 0,
            acc: 0.0,
            pending: Point::QUIET,
            last_phase: -1.0,
        }
    }
}

/// Whether the phase moving from `a` to `b` began a cycle: a wrap, a note's restart, or a
/// start from none.
pub fn began(a: f32, b: f32) -> bool {
    b >= 0.0 && (a < 0.0 || b + 0.02 < a)
}

impl View {
    /// A frame's reading: kept as the deepest and loudest until the next point is due.
    pub fn update(&mut self, r: &Reading, dt: f32) {
        if !r.seen {
            return;
        }
        let p = &mut self.pending;
        p.gr = p.gr.max(r.gr);
        p.amount = r.amount;
        p.key_db = p.key_db.max(r.key_db);
        p.phase = r.phase;
        if began(self.last_phase, r.phase) {
            p.hit = Hit::Cycle;
        } else if r.triggered && p.hit == Hit::None {
            p.hit = Hit::Key;
        }
        self.last_phase = r.phase;
        self.acc += dt;
        if self.acc >= 1.0 / RATE {
            let p = std::mem::replace(&mut self.pending, Point::QUIET);
            while self.acc >= 1.0 / RATE {
                self.acc -= 1.0 / RATE;
                self.push(p);
            }
        }
    }

    fn push(&mut self, p: Point) {
        self.ring[self.head] = p;
        self.head = (self.head + 1) % HISTORY;
        self.len = (self.len + 1).min(HISTORY);
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// The history, oldest first.
    pub fn points(&self) -> impl Iterator<Item = &Point> + '_ {
        let start = (self.head + HISTORY - self.len) % HISTORY;
        (0..self.len).map(move |i| &self.ring[(start + i) % HISTORY])
    }
}
