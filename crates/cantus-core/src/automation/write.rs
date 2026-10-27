//! Writing automation while the song plays (ADR 0068): Pro Tools', Logic's, Cubase's and
//! Studio One's **Read / Write / Touch / Latch**, FL's *record automation* and Ableton's
//! *automation arm* as the simple case.
//!
//! A [`Writer`] is one **pass** — from the transport's start to its stop — as pure data: the
//! application tells it what the hand did (a value at a tick, which controls are held) and
//! where the playhead is each frame, and when the pass ends [`commit`](super::write_commit)
//! turns what it wrote into automation clips, one undo step. Nothing here reads a clock,
//! the engine or the interface, so every rule is a unit test:
//!
//! - **Read** plays the curves; a hand on a control changes the project's own value (FL's
//!   override, ADR 0027 §6) and writes nothing.
//! - **Write** overwrites every *armed* target of the track — every target it already
//!   automates, and every one the hand moves — from the pass's start to its stop, holding
//!   each where it was when the pass began until the hand moves it.
//! - **Touch** writes while a control is held; let go, the value glides back to the curve
//!   already there over the touch glide time, and the old curve plays on.
//! - **Latch** writes from the first touch until the stop, holding the last value.
//!
//! **Loops.** Each loop pass writes over what the previous one wrote where it writes: the
//! last pass wins. A jump of the playhead (a wrap, a seek) ends every run; a held, latched
//! or written target begins a new one where the playhead lands.
//!
//! **Punch.** With punch on, nothing is written outside the punch range: a run begins at
//! the punch-in at the earliest and ends at the punch-out.

use super::AutoTarget;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// How a track's (or the master's) automation behaves while the song plays.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AutoMode {
    /// Plays the curves; writes nothing (every older project).
    #[default]
    Read,
    /// Overwrites every armed target from the start of the pass to its stop.
    Write,
    /// Writes while a control is held, then glides back to the curve already there.
    Touch,
    /// Writes from the first touch until the stop, holding the last value.
    Latch,
}

impl AutoMode {
    pub const ALL: [AutoMode; 4] = [
        AutoMode::Read,
        AutoMode::Write,
        AutoMode::Touch,
        AutoMode::Latch,
    ];

    /// Whether the mode writes anything.
    pub fn writes(self) -> bool {
        self != AutoMode::Read
    }

    pub fn is_read(&self) -> bool {
        *self == AutoMode::Read
    }

    /// The locale key of its name ("Read", "Touch", …).
    pub fn label_key(self) -> &'static str {
        match self {
            AutoMode::Read => "automodes.mode.read",
            AutoMode::Write => "automodes.mode.write",
            AutoMode::Touch => "automodes.mode.touch",
            AutoMode::Latch => "automodes.mode.latch",
        }
    }

    /// The locale key of its one-letter badge (R, W, T, L — the track header's chip).
    pub fn badge_key(self) -> &'static str {
        match self {
            AutoMode::Read => "automodes.badge.read",
            AutoMode::Write => "automodes.badge.write",
            AutoMode::Touch => "automodes.badge.touch",
            AutoMode::Latch => "automodes.badge.latch",
        }
    }

    /// The locale key of what it does, for its tooltip.
    pub fn hint_key(self) -> &'static str {
        match self {
            AutoMode::Read => "automodes.hint.read",
            AutoMode::Write => "automodes.hint.write",
            AutoMode::Touch => "automodes.hint.touch",
            AutoMode::Latch => "automodes.hint.latch",
        }
    }
}

/// One stretch written without a jump: `(tick, position 0…1)`, in time order.
pub type Run = Vec<(i64, f32)>;

/// What one target is doing in the pass.
#[derive(Clone, Debug, PartialEq)]
pub struct TargetWrite {
    pub mode: AutoMode,
    /// The runs written so far, oldest first (a loop's later passes come after).
    pub runs: Vec<Run>,
    /// Whether the newest run is still being written.
    pub open: bool,
    /// A hand holds the control (a drag, a plug-in's gesture, a controller still moving).
    pub held: bool,
    /// Touched once in Latch (or armed in Write): it writes until the stop.
    pub latched: bool,
    /// The last value written.
    pub last: f32,
    /// A touch let go: the glide back, `(from tick, to tick, from value, to value)`.
    pub glide: Option<(i64, i64, f32, f32)>,
    /// The run the last jump of the playhead closed (a loop's lap): where a value heard
    /// before the jump but told after it belongs.
    pub before_jump: Option<usize>,
}

impl TargetWrite {
    fn new(mode: AutoMode, pos: f32) -> Self {
        TargetWrite {
            mode,
            runs: Vec::new(),
            open: false,
            held: false,
            latched: false,
            last: pos,
            glide: None,
            before_jump: None,
        }
    }

    fn push(&mut self, tick: i64, pos: f32) {
        if !self.open {
            self.runs.push(Vec::new());
            self.open = true;
        }
        let run = self.runs.last_mut().expect("opened");
        insert(run, tick, pos);
        self.last = pos;
    }

    /// A value told late (a controller's message, at its own time): ahead of the playhead
    /// after a jump, it belongs to the lap before the jump; behind the open run's last
    /// sample, it goes in order into the run covering it — never dropped.
    fn push_late(&mut self, tick: i64, pos: f32, playhead: Option<i64>) {
        let ahead = playhead.is_some_and(|p| tick > p);
        if let (true, Some(i)) = (ahead, self.before_jump) {
            if let Some(run) = self.runs.get_mut(i) {
                insert(run, tick, pos);
                self.last = pos;
                return;
            }
        }
        if self.open {
            if let Some(run) = self.runs.last_mut() {
                if run.first().is_some_and(|f| f.0 > tick) {
                    // Before the open run began: the run the jump closed, if any.
                    if let Some(prev) = self.before_jump.and_then(|i| self.runs.get_mut(i)) {
                        insert(prev, tick, pos);
                        return;
                    }
                }
            }
        }
        self.push(tick, pos);
    }

    /// Whether it writes on its own now (held, latched, or Write's hold).
    fn continues(&self) -> bool {
        self.held || self.latched
    }
}

/// One pass of writing.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Writer {
    pub targets: BTreeMap<AutoTarget, TargetWrite>,
    /// The punch range, when punch is on: nothing is written outside it.
    pub window: Option<(i64, i64)>,
    /// Where the pass began (the playhead at its start, or the punch-in).
    pub began: i64,
    /// The playhead at the last frame.
    pub at: Option<i64>,
    /// The playhead now (set at the top of a frame, before its values): a value told
    /// ahead of it was heard before a jump.
    pub now: Option<i64>,
}

impl Writer {
    /// A pass beginning at `tick`, writing only inside `window` when punch is on.
    pub fn new(tick: i64, window: Option<(i64, i64)>) -> Writer {
        Writer {
            targets: BTreeMap::new(),
            window,
            began: window.map_or(tick, |(a, _)| a.max(tick)),
            at: Some(tick),
            now: Some(tick),
        }
    }

    /// Whether `tick` is where the pass writes.
    fn inside(&self, tick: i64) -> bool {
        self.window.is_none_or(|(a, b)| tick >= a && tick <= b)
    }

    /// Nothing written yet.
    pub fn is_empty(&self) -> bool {
        self.targets.values().all(|t| t.runs.is_empty())
    }

    /// A Write-mode target armed at the start of the pass: it holds `pos` (where it was
    /// heard when the pass began) until the hand moves it, and writes until the stop.
    pub fn arm(&mut self, target: AutoTarget, tick: i64, pos: f32) {
        let w = self
            .targets
            .entry(target)
            .or_insert_with(|| TargetWrite::new(AutoMode::Write, pos));
        w.mode = AutoMode::Write;
        w.latched = true;
        w.last = pos;
        if self.window.is_none_or(|(a, b)| tick >= a && tick <= b) {
            w.push(tick, pos);
        }
    }

    /// The hand put `target` at `pos` (0…1) at `tick`, under `mode` (the mode of the track
    /// it belongs to). Read writes nothing.
    pub fn value(&mut self, target: AutoTarget, mode: AutoMode, tick: i64, pos: f32) {
        if !mode.writes() {
            return;
        }
        // Past the punch-out nothing is written; before the window begins (a count-in, the
        // pre-roll, before the punch-in) a move is where it begins.
        let (lo, hi) = self.window.unwrap_or((i64::MIN, i64::MAX));
        if tick > hi {
            return;
        }
        let tick = tick.max(lo);
        let w = self
            .targets
            .entry(target)
            .or_insert_with(|| TargetWrite::new(mode, pos));
        w.mode = mode;
        w.glide = None;
        // A value is a touch: held until a frame says the hand let go (a click, a typed
        // value: let go at once).
        w.held = true;
        if mode == AutoMode::Latch || mode == AutoMode::Write {
            w.latched = true;
        }
        let playhead = self.now.or(self.at);
        w.push_late(tick, pos, playhead);
    }

    /// The playhead jumped to `to` (a loop's wrap, a seek) rather than playing there: every
    /// run ends where the playhead was, and what goes on writing — held, latched or written —
    /// begins again where it lands. Call it before the frame's values.
    pub fn jump(&mut self, to: i64) {
        let prev = self.at;
        let window = self.window;
        let inside = |t: i64| window.is_none_or(|(a, b)| t >= a && t <= b);
        for w in self.targets.values_mut() {
            if let (true, Some(p)) = (w.open, prev) {
                if inside(p) && w.continues() {
                    let v = w.last;
                    w.push(p, v);
                }
            }
            if w.open {
                w.before_jump = Some(w.runs.len() - 1);
            }
            w.open = false;
            w.glide = None;
            if w.continues() && w.mode.writes() && inside(to) {
                let v = w.last;
                w.push(to, v);
            }
        }
        self.at = Some(to);
    }

    /// A frame: the playhead is at `tick`, `held` says which controls a hand holds.
    /// Releases begin their glide (`glide_end` is where a glide that starts at a tick
    /// ends; `old` the curve already there, 0…1); latched and written targets hold their
    /// value up to here.
    pub fn frame(
        &mut self,
        tick: i64,
        held: &dyn Fn(&AutoTarget) -> bool,
        glide_end: &dyn Fn(i64) -> i64,
        old: &dyn Fn(&AutoTarget, i64) -> f32,
    ) {
        let prev = self.at;
        let window = self.window;
        let inside = |t: i64| window.is_none_or(|(a, b)| t >= a && t <= b);
        for (target, w) in self.targets.iter_mut() {
            let now_held = held(target);
            if w.held && !now_held {
                // Let go.
                w.held = false;
                if w.mode == AutoMode::Touch && w.open {
                    let to = glide_end(tick).max(tick);
                    let back = old(target, to);
                    let from = w.last;
                    if inside(tick) {
                        w.push(tick, from);
                    }
                    if to > tick && inside(to) {
                        w.push(to, back);
                    } else if let (true, Some((_, b))) = (to > tick && inside(tick), window) {
                        // The glide runs past the punch-out: it ends there, where it has got to.
                        let u = (b - tick) as f32 / (to - tick) as f32;
                        w.push(b, from + (back - from) * u);
                    } else if inside(tick) {
                        w.push(tick, back);
                    }
                    w.open = false;
                    w.glide = Some((tick, to, from, back));
                    continue;
                }
            }
            if now_held {
                w.held = true;
            }
            // Holding: a latched or held target writes its value up to the playhead — from
            // the punch-in exactly, when the playhead just crossed it.
            if w.continues() && inside(tick) && w.mode.writes() {
                let v = w.last;
                if let (Some((a, _)), Some(p)) = (window, prev) {
                    if p < a && tick > a {
                        w.push(a, v);
                    }
                }
                w.push(tick, v);
            } else if w.open && !inside(tick) && prev.is_some_and(&inside) {
                // Past the punch-out: the run ends there.
                if let Some((_, b)) = window {
                    let v = w.last;
                    w.push(b, v);
                }
                w.open = false;
            }
        }
        self.at = Some(tick);
    }

    /// The pass stopped at `tick`: every open run ends there, holding its value.
    pub fn finish(&mut self, tick: i64) {
        let window = self.window;
        // Stopped before the window began (a false start in a count-in or a pre-roll): what
        // was clamped to its start was never heard there — nothing is written.
        if window.is_some_and(|(a, _)| tick < a) {
            for w in self.targets.values_mut() {
                w.runs.clear();
                w.open = false;
                w.held = false;
            }
            self.at = Some(tick);
            return;
        }
        for w in self.targets.values_mut() {
            if w.open && w.continues() {
                let end = match window {
                    Some((a, b)) => tick.clamp(a, b),
                    None => tick,
                };
                let v = w.last;
                w.push(end, v);
            }
            w.open = false;
            w.held = false;
        }
        self.at = Some(tick);
    }

    /// What the engine should hear for `target` now (see [`Live`]).
    pub fn live(&self, target: &AutoTarget) -> Live {
        self.live_at(target, self.at.unwrap_or(self.began))
    }

    /// [`Self::live`] with the playhead at `tick`: outside the pass's window (past the
    /// punch-out) every target plays its curve again.
    pub fn live_at(&self, target: &AutoTarget, tick: i64) -> Live {
        if !self.inside(tick) {
            return Live::Curve;
        }
        match self.targets.get(target) {
            None => Live::Curve,
            Some(w) if w.held => Live::Hand,
            Some(w) if w.latched && w.mode.writes() => Live::Hold(w.last),
            Some(w) => match w.glide {
                Some((a, b, from, to)) => Live::Glide { a, b, from, to },
                None => Live::Curve,
            },
        }
    }

    /// Every target the pass has written, with its runs (oldest first).
    pub fn written(&self) -> impl Iterator<Item = (&AutoTarget, &[Run])> {
        self.targets
            .iter()
            .filter(|(_, w)| !w.runs.is_empty())
            .map(|(t, w)| (t, w.runs.as_slice()))
    }
}

/// What the engine hears for a target while a pass writes it.
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum Live {
    /// Its curve, as written before the pass.
    Curve,
    /// The hand: the project's value, which the hand moves (no lane drives it).
    Hand,
    /// One value, held (Latch after the hand let go, Write before it moved).
    Hold(f32),
    /// The curve, but for the glide back from a released touch: `from` at tick `a` to
    /// `to` at tick `b`.
    Glide { a: i64, b: i64, from: f32, to: f32 },
}

/// The runs of a target laid in pass order, each later one written over the stretch it
/// covers: disjoint runs in time order, each sorted (the last pass wins inside a loop). An
/// earlier run cut by a later one joins it where it was cut — the writing never stopped
/// there — so no sliver of the old curve shows between them.
pub fn overlay(runs: &[Run]) -> Vec<Run> {
    let mut out: Vec<Run> = Vec::new();
    for run in runs.iter().filter(|r| !r.is_empty()) {
        let (a, b) = (run[0].0, run[run.len() - 1].0);
        let mut next: Vec<Run> = Vec::with_capacity(out.len() + 2);
        let mut joined: Run = Vec::new();
        let mut tail: Run = Vec::new();
        for r in out.drain(..) {
            let (ra, rb) = (r[0].0, r[r.len() - 1].0);
            let left: Run = r.iter().copied().filter(|s| s.0 < a).collect();
            let right: Run = r.iter().copied().filter(|s| s.0 > b).collect();
            // Cut at `a` (it ran on past it): its left part leads into the new run.
            if ra < a && rb >= a {
                joined.extend(left);
            } else if !left.is_empty() {
                next.push(left);
            }
            // Cut at `b`: its right part follows the new run.
            if ra <= b && rb > b {
                tail.extend(right);
            } else if !right.is_empty() {
                next.push(right);
            }
        }
        joined.extend(run.iter().copied());
        joined.extend(tail);
        next.push(joined);
        next.sort_by_key(|r| r[0].0);
        out = next;
    }
    out
}

/// `(tick, pos)` into a run in time order: at a tick already there it replaces the value.
fn insert(run: &mut Run, tick: i64, pos: f32) {
    match run.last_mut() {
        Some(l) if l.0 == tick => l.1 = pos,
        Some(l) if l.0 > tick => {
            let k = run.partition_point(|s| s.0 < tick);
            if run[k].0 == tick {
                run[k].1 = pos;
            } else {
                run.insert(k, (tick, pos));
            }
        }
        _ => run.push((tick, pos)),
    }
}

#[cfg(test)]
#[path = "write_tests.rs"]
mod tests;
