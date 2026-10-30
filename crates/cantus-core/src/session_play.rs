//! Playing the session (ADR 0076 §7): from what the performer pressed, and when, to what
//! is heard — a [`Performance`] — and the project that plays it.
//!
//! **The conductor is a pure function.** [`plan`] takes the project, the presses (each
//! with the earliest tick it may take effect, [`Timed`]) and a horizon, and returns every
//! clip heard, every track the session held, every scene tempo and metre, and the changes
//! the view shows ([`Change`]) — launch boundaries resolved on the bar lines, launch modes,
//! legato, follow actions drawn from the session's seed by **where** they happen (a hash
//! of the seed, the track or scene and the tick: no state, so a longer horizon only adds,
//! never changes what came before), stops and *back to arrangement*. The same presses give
//! the same performance, live or offline.
//!
//! **What is heard is a projection.** [`project_play`] is the project with the performance
//! written into its arrangement exactly as arrangement record writes it (`edit::perform`,
//! keyed ids): the engine plays it as it plays any snapshot — sample-accurate by
//! construction, every feature of an arrangement clip (warp, fades, lanes, grooves, note
//! effects, automation) with no engine code of its own — and committing the performance
//! writes the very clips that were heard.

use super::edit::{Performance, Played};
use super::{
    boundary_at_or_after, FollowAction, FollowActions, FollowAfter, LaunchMode, QuantGrid,
    Quantize, SceneId, SessionClip, SlotRef,
};
use crate::model::{Project, TrackId};
use crate::tempo::{MetreChange, TempoMap};
use crate::time::{Ticks, TimeSignature};
use std::cmp::Reverse;
use std::collections::BinaryHeap;

/// A press of the session's controls (the engine's `SessionMsg`, less recording).
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum Input {
    Launch {
        slot: SlotRef,
        velocity: f32,
    },
    Release {
        slot: SlotRef,
    },
    LaunchScene {
        scene: SceneId,
        velocity: f32,
    },
    ReleaseScene {
        scene: SceneId,
    },
    StopTrack {
        track: TrackId,
    },
    StopAll,
    /// One track (or every track) back to its arrangement, at once.
    Back {
        track: Option<TrackId>,
    },
    /// Start a slot's clip as if it had been launched at `since` (earlier): where it would
    /// be by now — a clip just recorded, playing on from where its recording ended.
    Resume {
        slot: SlotRef,
        since: Ticks,
    },
}

/// A press, and the earliest tick it may take effect: a quantised one at the first
/// boundary at or after it; with `now` (the transport was stopped) exactly there.
/// `pressed` is what was heard when it was pressed: a launch pressed within
/// [`LAUNCH_TOLERANCE`] after a boundary plays as launched on it (Bitwig's launch
/// tolerance), caught up from `at` — not a whole grid later.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Timed {
    pub at: Ticks,
    pub now: bool,
    pub input: Input,
    pub pressed: Ticks,
}

impl Timed {
    /// A press taking effect no sooner than `at`, heard there too (a script, a test).
    pub fn at(at: Ticks, input: Input) -> Self {
        Timed {
            at,
            now: false,
            input,
            pressed: at,
        }
    }
}

/// How late after a boundary a launch may be pressed and still play on it: a 32nd note —
/// at most a quarter of the launch grid ([`tolerance`]).
pub const LAUNCH_TOLERANCE: Ticks = Ticks(crate::time::PPQ / 8);

/// The launch tolerance on `grid`: [`LAUNCH_TOLERANCE`], at most a quarter of a note grid
/// (on a grid of a 32nd or finer a press is not caught up by a whole step).
pub fn tolerance(grid: QuantGrid) -> Ticks {
    match grid {
        QuantGrid::None => Ticks::ZERO,
        QuantGrid::Bars(_) => LAUNCH_TOLERANCE,
        QuantGrid::Ticks(g) => LAUNCH_TOLERANCE.min(Ticks(g.max(1) / 4)),
    }
}

/// A change the view shows, at the tick it takes effect.
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum Change {
    /// A slot's clip starts playing, from content tick `from`.
    Play { slot: SlotRef, from: Ticks },
    /// A slot's clip stops (stopped, replaced, ended, or followed by another).
    Stop { slot: SlotRef },
    /// A track plays its arrangement (`true`) or the session holds it (`false`).
    Arrangement { track: TrackId, on: bool },
    /// A scene's row was launched (and plays until another is).
    Scene { scene: SceneId },
    /// A follow action handed `from`'s clip over to `to` (or stopped it).
    Followed { from: SlotRef, to: Option<SlotRef> },
    /// A scene's tempo (and metre) took effect.
    Tempo {
        bpm: Option<f64>,
        metre: Option<TimeSignature>,
    },
}

/// What [`plan`] works out: the performance up to the horizon, its changes in order, and
/// for each press the tick it took (or will take) effect at — `None` for one that did
/// nothing.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Plan {
    pub perf: Performance,
    pub changes: Vec<(Ticks, Change)>,
    pub effect: Vec<Option<Ticks>>,
}

/// A draw 0…99 from the seed, the thing that follows (a track or a scene, by id) and the
/// tick it follows at: the same everywhere, whatever was drawn before.
fn roll(seed: u64, id: u64, tick: Ticks, salt: u64) -> u64 {
    let mix = |mut z: u64| {
        z = z.wrapping_add(0x9E37_79B9_7F4A_7C15);
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    };
    mix(seed ^ mix(id ^ mix(tick.0 as u64 ^ mix(salt))))
}

/// The first of `due`, `due + period`, `due + 2 period`… at or after `cut`.
fn next_due(due: Ticks, period: Ticks, cut: Ticks) -> Ticks {
    if due >= cut {
        return due;
    }
    let p = period.0.max(1);
    let k = (cut - due).0.div_euclid(p) + i64::from((cut - due).0.rem_euclid(p) != 0);
    due + Ticks(k * p)
}

/// How long after a launch from `from` its follow action is taken.
fn follow_time(c: &SessionClip, from: Ticks, f: &FollowActions) -> Ticks {
    let n = i64::from(f.count.max(1));
    match f.after {
        FollowAfter::Time(t) => Ticks(t.0 * n),
        FollowAfter::Linked => (c.end() - from).max(Ticks::ZERO) + Ticks(c.pass().0 * (n - 1)),
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Item {
    Input(usize),
    Follow { track: usize, gen: u32 },
    Retrigger { track: usize, gen: u32 },
    SceneFollow { gen: u32 },
    End { track: usize, gen: u32 },
    Stop { track: usize, gen: u32 },
}

impl Item {
    /// At one tick: presses first (they replace what would have followed), then follow
    /// actions, then a clip's natural end.
    fn class(&self) -> u8 {
        match self {
            Item::Input(_) => 0,
            Item::Follow { .. } | Item::Retrigger { .. } | Item::SceneFollow { .. } => 1,
            Item::Stop { .. } => 2,
            Item::End { .. } => 3,
        }
    }
}

#[derive(Copy, Clone, Debug)]
struct Seg {
    slot: SlotRef,
    at: Ticks,
    from: Ticks,
    velocity: f32,
    /// The launch it keeps in step with (its own, or a legato's or catch-up's).
    origin: Ticks,
}

#[derive(Clone, Debug, Default)]
struct TrackState {
    seg: Option<Seg>,
    held: Option<Ticks>,
    gen: u32,
    /// A gate or repeat clip held down: its slot (released ones stop).
    holding: Option<SlotRef>,
}

struct Planner<'a> {
    project: &'a Project,
    map: TempoMap,
    metres: Vec<MetreChange>,
    tracks: Vec<TrackState>,
    agenda: BinaryHeap<Reverse<(Ticks, u8, u64, Item)>>,
    seq: u64,
    out: Plan,
    scene: Option<(SceneId, u32)>,
    /// Where the scene playing was launched.
    scene_at: Ticks,
    scene_gen: u32,
    global: Quantize,
    /// The next start's phase (`Input::Resume`).
    phase: Option<Ticks>,
    /// The press being played was caught up: the boundary it plays as launched on.
    catch: Option<Ticks>,
    /// Each press's catch-up boundary, once resolved.
    origins: Vec<Option<Ticks>>,
}

impl<'a> Planner<'a> {
    fn push(&mut self, at: Ticks, item: Item) {
        self.seq += 1;
        self.agenda
            .push(Reverse((at, item.class(), self.seq, item)));
    }

    fn change(&mut self, at: Ticks, c: Change) {
        self.out.changes.push((at, c));
    }

    fn track_index(&self, id: TrackId) -> Option<usize> {
        self.project.track_index(id)
    }

    fn clip(&self, slot: SlotRef) -> Option<&'a SessionClip> {
        self.project.slot_clip(slot)
    }

    fn boundary(&self, at: Ticks, grid: QuantGrid) -> Ticks {
        boundary_at_or_after(&self.map, at, grid)
    }

    /// End the track's clip at `at`, if one plays.
    fn end(&mut self, ti: usize, at: Ticks) {
        if let Some(s) = self.tracks[ti].seg.take() {
            if at > s.at {
                let gain = self.clip(s.slot).map_or(1.0, |c| c.launch.gain(s.velocity));
                self.out.perf.played.push(Played {
                    slot: s.slot,
                    at: s.at,
                    from: s.from,
                    until: at,
                    gain,
                    velocity: s.velocity,
                    origin: s.origin,
                });
            }
            self.change(at, Change::Stop { slot: s.slot });
        }
        self.tracks[ti].gen += 1;
    }

    fn hold(&mut self, ti: usize, at: Ticks) {
        if self.tracks[ti].held.is_none() {
            self.tracks[ti].held = Some(at);
            let track = self.project.tracks[ti].id;
            self.change(at, Change::Arrangement { track, on: false });
        }
    }

    /// Start the clip of `slot` at `at`: from its start marker, or — legato, or `from` given
    /// — where the clip it replaces had got to.
    fn start(&mut self, slot: SlotRef, at: Ticks, velocity: f32, legato_ok: bool, pressed: bool) {
        let Some(ti) = self.track_index(slot.track) else {
            return;
        };
        let Some(c) = self.clip(slot) else {
            return;
        };
        // A frozen track plays its render: nothing of its session launches.
        if self.project.tracks[ti].frozen.is_some() {
            return;
        }
        let prev = self.tracks[ti].seg;
        // In step with a launch earlier than `at`: a resume, a caught-up press, or — legato —
        // the launch the clip it replaces kept in step with (a chain of legato launches
        // keeps the first's phase).
        let phase = self
            .phase
            .take()
            .or(if pressed { self.catch } else { None });
        let origin = match (prev, phase) {
            (_, Some(since)) => since,
            (Some(p), _) if legato_ok && c.launch.legato => p.origin,
            _ => at,
        };
        let from = c
            .content_from(c.play_from(), at - origin)
            .unwrap_or(c.play_from());
        self.end(ti, at);
        self.hold(ti, at);
        self.tracks[ti].seg = Some(Seg {
            slot,
            at,
            from,
            velocity,
            origin,
        });
        let gen = self.tracks[ti].gen;
        self.change(at, Change::Play { slot, from });
        if c.follow.is_active() {
            self.push(
                at + follow_time(c, from, &c.follow).max(Ticks(1)),
                Item::Follow { track: ti, gen },
            );
        }
        if !c.looping() {
            self.push(
                at + (c.end() - from).max(Ticks(1)),
                Item::End { track: ti, gen },
            );
        }
        if pressed {
            self.tracks[ti].holding = match c.launch.mode {
                LaunchMode::Gate | LaunchMode::Repeat => Some(slot),
                _ => None,
            };
        }
        if c.launch.mode == LaunchMode::Repeat && self.tracks[ti].holding == Some(slot) {
            let grid = c.launch.quantize.grid(self.global);
            if grid != QuantGrid::None {
                let next = self.boundary(at + Ticks(1), grid);
                self.push(next, Item::Retrigger { track: ti, gen });
            }
        }
    }

    /// A slot's launcher, by its clip's mode (`pressed`: by a hand, not a follow action).
    fn launch(&mut self, slot: SlotRef, at: Ticks, velocity: f32, scene: bool) {
        let Some(ti) = self.track_index(slot.track) else {
            return;
        };
        match self.clip(slot) {
            Some(c) => {
                let playing = self.tracks[ti].seg.is_some_and(|s| s.slot == slot);
                if c.launch.mode == LaunchMode::Toggle && playing && !scene {
                    self.end(ti, at);
                } else {
                    self.start(slot, at, velocity, true, true);
                }
            }
            None => {
                let stop = self
                    .project
                    .track(slot.track)
                    .and_then(|t| t.slot(slot.scene))
                    .is_none_or(|s| s.stop);
                if stop && self.project.tracks[ti].frozen.is_none() {
                    self.end(ti, at);
                    self.hold(ti, at);
                }
            }
        }
    }

    /// The column's slots that hold clips, top to bottom.
    fn column(&self, ti: usize) -> Vec<SlotRef> {
        let t = &self.project.tracks[ti];
        self.project
            .session
            .scenes
            .iter()
            .filter(|s| t.slot_clip(s.id).is_some())
            .map(|s| SlotRef::new(t.id, s.id))
            .collect()
    }

    fn follow(&mut self, ti: usize, at: Ticks) {
        let Some(seg) = self.tracks[ti].seg else {
            return;
        };
        let Some(c) = self.clip(seg.slot) else {
            return;
        };
        let seed = self.project.session.settings.seed;
        let id = self.project.tracks[ti].id.0;
        let action = c.follow.pick((roll(seed, id, at, 0) % 100) as u8);
        let col = self.column(ti);
        let here = col.iter().position(|s| *s == seg.slot);
        let n = col.len();
        let pick = |k: usize| col.get(k).copied();
        let target = match (action, here) {
            (FollowAction::None, _) => return,
            (FollowAction::Stop, _) => None,
            (FollowAction::Again, _) => Some(seg.slot),
            (_, None) => return,
            (FollowAction::Previous, Some(i)) => pick((i + n - 1) % n),
            (FollowAction::Next, Some(i)) => pick((i + 1) % n),
            (FollowAction::First, _) => pick(0),
            (FollowAction::Last, _) => pick(n - 1),
            (FollowAction::Any, _) => pick((roll(seed, id, at, 1) % n as u64) as usize),
            (FollowAction::Other, Some(i)) => {
                if n < 2 {
                    Some(seg.slot)
                } else {
                    let k = (roll(seed, id, at, 1) % (n as u64 - 1)) as usize;
                    pick(if k >= i { k + 1 } else { k })
                }
            }
            (FollowAction::Jump(scene), _) => {
                let s = SlotRef::new(seg.slot.track, scene);
                if self.clip(s).is_none() {
                    return;
                }
                Some(s)
            }
        };
        self.change(
            at,
            Change::Followed {
                from: seg.slot,
                to: target,
            },
        );
        match target {
            None => self.end(ti, at),
            Some(s) => self.start(s, at, seg.velocity, s != seg.slot, false),
        }
    }

    fn launch_scene(&mut self, scene: SceneId, at: Ticks, velocity: f32) {
        let Some(sc) = self.project.session.scene(scene) else {
            return;
        };
        for ti in 0..self.project.tracks.len() {
            let slot = SlotRef::new(self.project.tracks[ti].id, scene);
            self.launch(slot, at, velocity, true);
        }
        if sc.tempo.is_some() || sc.metre.is_some() {
            if let Some(bpm) = sc.tempo {
                self.out.perf.tempos.push((at, bpm));
            }
            if let Some(m) = sc.metre {
                let bar = if self.map.bar_start(at) == at {
                    at
                } else {
                    self.map.bar_end(at)
                };
                self.out.perf.metres.push((bar, m));
                self.metres.retain(|x| x.tick != bar);
                self.metres.push(MetreChange {
                    tick: bar,
                    signature: m,
                });
                self.metres.sort_by_key(|x| x.tick);
                self.map = TempoMap::new(
                    self.project.tempo_bpm,
                    self.project.time_signature,
                    &self.project.tempo_points,
                    &self.metres,
                );
            }
            self.change(
                at,
                Change::Tempo {
                    bpm: sc.tempo,
                    metre: sc.metre,
                },
            );
        }
        self.scene_gen += 1;
        self.scene = Some((scene, self.scene_gen));
        self.scene_at = at;
        self.change(at, Change::Scene { scene });
        if sc.follow.is_active() {
            let row: Vec<&SessionClip> = self
                .project
                .tracks
                .iter()
                .filter_map(|t| t.slot_clip(scene))
                .collect();
            let n = i64::from(sc.follow.count.max(1));
            let wait = match sc.follow.after {
                FollowAfter::Time(t) => Ticks(t.0 * n),
                FollowAfter::Linked => {
                    let longest = row
                        .iter()
                        .map(|c| c.first_pass())
                        .max()
                        .unwrap_or(Ticks::ZERO);
                    Ticks(longest.0 * n)
                }
            };
            if wait > Ticks::ZERO {
                let gen = self.scene_gen;
                self.push(at + wait, Item::SceneFollow { gen });
            }
        }
    }

    fn scene_follow(&mut self, at: Ticks) {
        let Some((scene, _)) = self.scene else {
            return;
        };
        let Some(sc) = self.project.session.scene(scene) else {
            return;
        };
        let seed = self.project.session.settings.seed;
        let action = sc.follow.pick((roll(seed, scene.0, at, 2) % 100) as u8);
        let scenes = &self.project.session.scenes;
        let n = scenes.len();
        let Some(i) = self.project.session.scene_index(scene) else {
            return;
        };
        let target = match action {
            FollowAction::None => return,
            FollowAction::Stop => {
                for ti in 0..self.tracks.len() {
                    self.end(ti, at);
                }
                return;
            }
            FollowAction::Again => Some(i),
            FollowAction::Previous => Some((i + n - 1) % n),
            FollowAction::Next => Some((i + 1) % n),
            FollowAction::First => Some(0),
            FollowAction::Last => Some(n - 1),
            FollowAction::Any => Some((roll(seed, scene.0, at, 3) % n as u64) as usize),
            FollowAction::Other => Some(if n < 2 {
                i
            } else {
                let k = (roll(seed, scene.0, at, 3) % (n as u64 - 1)) as usize;
                if k >= i {
                    k + 1
                } else {
                    k
                }
            }),
            FollowAction::Jump(s) => self.project.session.scene_index(s),
        };
        if let Some(k) = target {
            let id = scenes[k].id;
            self.launch_scene(id, at, 1.0);
        }
    }

    /// When a press takes effect, and the boundary it plays as launched on when it was
    /// pressed within the launch tolerance after one (caught up from where it takes
    /// effect).
    fn resolve(&self, t: &Timed) -> (Ticks, Option<Ticks>) {
        let global = self.global.grid(self.global);
        let grid = match t.input {
            Input::Launch { slot, .. } | Input::Release { slot } => match self.clip(slot) {
                Some(c) => c.launch.quantize.grid(self.global),
                None => global,
            },
            Input::LaunchScene { .. }
            | Input::ReleaseScene { .. }
            | Input::StopTrack { .. }
            | Input::StopAll => global,
            Input::Back { .. } | Input::Resume { .. } => QuantGrid::None,
        };
        if t.now || grid == QuantGrid::None {
            return (t.at, None);
        }
        if t.pressed < t.at {
            // A launch pressed just after a boundary, or before one it can no longer reach
            // (the lead, or a compile that overran, took `at` past it): on that boundary,
            // caught up from `at` — never a whole grid later.
            let catches = matches!(t.input, Input::Launch { .. } | Input::LaunchScene { .. });
            let back = if catches {
                tolerance(grid)
            } else {
                Ticks::ZERO
            };
            let near = self.boundary((t.pressed - back).max(Ticks::ZERO), grid);
            // A stop or a release pressed before a boundary it can no longer reach takes
            // effect as soon as it can, not a grid later (one pressed on a boundary, or
            // after it, waits for the next).
            if near < t.at && (catches || near > t.pressed) {
                return (t.at, catches.then_some(near));
            }
        }
        (self.boundary(t.at, grid), None)
    }

    fn release(&mut self, slot: SlotRef, at: Ticks) {
        let Some(ti) = self.track_index(slot.track) else {
            return;
        };
        if self.tracks[ti].holding != Some(slot) {
            return;
        }
        self.tracks[ti].holding = None;
        if self.tracks[ti].seg.is_some_and(|s| s.slot == slot) {
            let gen = self.tracks[ti].gen;
            self.push(at, Item::Stop { track: ti, gen });
        }
    }

    fn input(&mut self, i: usize, inputs: &[Timed], at: Ticks) {
        self.catch = self.origins.get(i).copied().flatten();
        self.input_inner(i, inputs, at);
        self.catch = None;
    }

    fn input_inner(&mut self, i: usize, inputs: &[Timed], at: Ticks) {
        match inputs[i].input {
            Input::Launch { slot, velocity } => self.launch(slot, at, velocity, false),
            Input::Resume { slot, since } => {
                self.phase = Some(since);
                self.start(slot, at, 1.0, false, false);
                self.phase = None;
            }
            Input::Release { slot } => self.release(slot, at),
            Input::LaunchScene { scene, velocity } => self.launch_scene(scene, at, velocity),
            Input::ReleaseScene { scene } => {
                for t in 0..self.project.tracks.len() {
                    let slot = SlotRef::new(self.project.tracks[t].id, scene);
                    self.release(slot, at);
                }
            }
            Input::StopTrack { track } => {
                if let Some(ti) = self.track_index(track) {
                    self.end(ti, at);
                    self.hold(ti, at);
                }
            }
            Input::StopAll => {
                for ti in 0..self.tracks.len() {
                    if self.tracks[ti].seg.is_some() {
                        self.end(ti, at);
                    }
                }
            }
            Input::Back { track } => {
                for ti in 0..self.tracks.len() {
                    let id = self.project.tracks[ti].id;
                    if track.is_some_and(|t| t != id) {
                        continue;
                    }
                    self.end(ti, at);
                    if let Some(from) = self.tracks[ti].held.take() {
                        if at > from {
                            self.out.perf.held.push((id, from, at));
                        }
                        self.change(
                            at,
                            Change::Arrangement {
                                track: id,
                                on: true,
                            },
                        );
                    }
                }
            }
        }
    }
}

/// The performance the presses `inputs` (in the order pressed; each is taken at its tick)
/// make on `project`, worked out up to `horizon`: what still plays there is cut at it.
pub fn plan(project: &Project, inputs: &[Timed], horizon: Ticks) -> Plan {
    plan_from(project, inputs, horizon, None)
}

/// [`plan`] carried on from `prior` (planned from the same presses, fewer of them) as it
/// stands before tick `cut`: what was planned before it is kept as it was — what was heard
/// does not change when a follow chance, a loop or a scene's tempo is edited, or a press is
/// added (ADR 0076 §7) — and the project as it is now plays from there. The clips playing
/// at `cut` play on, their follow actions and ends taken again from the clips as they now
/// are (not before `cut`).
pub fn plan_from(
    project: &Project,
    inputs: &[Timed],
    horizon: Ticks,
    prior: Option<(&Plan, Ticks)>,
) -> Plan {
    let mut p = Planner {
        project,
        map: project.tempo_map(),
        metres: project.metre_changes.clone(),
        tracks: vec![TrackState::default(); project.tracks.len()],
        agenda: BinaryHeap::new(),
        seq: 0,
        out: Plan {
            effect: vec![None; inputs.len()],
            ..Plan::default()
        },
        scene: None,
        scene_at: Ticks::ZERO,
        scene_gen: 0,
        global: project.session.settings.launch_quantize,
        phase: None,
        catch: None,
        origins: vec![None; inputs.len()],
    };
    let mut applied = vec![false; inputs.len()];
    let cut = prior.map(|(_, c)| c);
    if let Some((old, cut)) = prior {
        p.resume(old, cut, inputs, &mut applied);
    }
    // The presses by the tick they may take effect from (a recording's launch laid a bar
    // ahead comes after a later press for sooner): each in its place, ties in the order
    // pressed. A press keeps its index (`effect`, `Item::Input`).
    let mut order: Vec<usize> = (0..inputs.len()).filter(|&i| !applied[i]).collect();
    order.sort_by_key(|&i| inputs[i].at);
    let mut k = 0;
    loop {
        let agenda_at = p.agenda.peek().map(|Reverse((t, ..))| *t);
        // A press is resolved once everything before it has happened (a scene's metre
        // before it moves the bar lines it waits for).
        if let Some(&next) = order.get(k) {
            if agenda_at.is_none_or(|t| inputs[next].at <= t) {
                if inputs[next].at >= horizon {
                    break;
                }
                let (at, origin) = p.resolve(&inputs[next]);
                // Never before what is kept.
                let at = cut.map_or(at, |c| at.max(c));
                p.out.effect[next] = Some(at);
                p.origins[next] = origin;
                p.push(at, Item::Input(next));
                k += 1;
                continue;
            }
        }
        let Some(Reverse((at, _, _, item))) = p.agenda.pop() else {
            break;
        };
        if at >= horizon {
            break;
        }
        match item {
            Item::Input(i) => p.input(i, inputs, at),
            Item::Follow { track, gen } if p.tracks[track].gen == gen => p.follow(track, at),
            Item::Retrigger { track, gen } if p.tracks[track].gen == gen => {
                // Only while it is held: a release cancels the next retrigger.
                if let Some(s) = p.tracks[track].seg {
                    if p.tracks[track].holding == Some(s.slot) {
                        p.start(s.slot, at, s.velocity, false, false);
                    }
                }
            }
            Item::SceneFollow { gen } if p.scene.is_some_and(|(_, g)| g == gen) => {
                p.scene_follow(at)
            }
            Item::End { track, gen } | Item::Stop { track, gen } if p.tracks[track].gen == gen => {
                p.end(track, at)
            }
            _ => {}
        }
    }
    // What still plays, and the tracks still held, run to the horizon.
    for ti in 0..p.tracks.len() {
        if let Some(s) = p.tracks[ti].seg.take() {
            let gain = p.clip(s.slot).map_or(1.0, |c| c.launch.gain(s.velocity));
            p.out.perf.played.push(Played {
                slot: s.slot,
                at: s.at,
                from: s.from,
                until: horizon.max(s.at),
                gain,
                velocity: s.velocity,
                origin: s.origin,
            });
        }
        if let Some(from) = p.tracks[ti].held.take() {
            if horizon > from {
                p.out.perf.held.push((project.tracks[ti].id, from, horizon));
            }
        }
    }
    p.out.perf.played.retain(|x| x.until > x.at);
    p.out
        .perf
        .played
        .sort_by_key(|x| (x.at, x.slot.track, x.slot.scene));
    p.out.perf.held.sort_by_key(|&(t, a, _)| (a, t));
    p.out.changes.sort_by_key(|(t, _)| *t);
    p.out
}

impl Planner<'_> {
    /// Take on `old`'s plan as it stands before `cut`: what ended before it kept, what
    /// plays across it playing on (its follow action and end planned again from the clip
    /// as it now is), the presses that took effect before it marked `applied`.
    fn resume(&mut self, old: &Plan, cut: Ticks, inputs: &[Timed], applied: &mut [bool]) {
        for (i, e) in old.effect.iter().enumerate().take(inputs.len()) {
            if e.is_some_and(|e| e < cut) {
                applied[i] = true;
                self.out.effect[i] = *e;
            }
        }
        for &(t, c) in old.changes.iter().filter(|(t, _)| *t < cut) {
            self.out.changes.push((t, c));
            if let Change::Scene { scene } = c {
                self.scene_gen += 1;
                self.scene = Some((scene, self.scene_gen));
                self.scene_at = t;
            }
        }
        self.out.perf.tempos = old
            .perf
            .tempos
            .iter()
            .copied()
            .filter(|(t, _)| *t < cut)
            .collect();
        self.out.perf.metres = old
            .perf
            .metres
            .iter()
            .copied()
            .filter(|(t, _)| *t < cut)
            .collect();
        for &(tick, signature) in &self.out.perf.metres {
            self.metres.retain(|x| x.tick != tick);
            self.metres.push(MetreChange { tick, signature });
        }
        if !self.out.perf.metres.is_empty() {
            self.metres.sort_by_key(|x| x.tick);
            self.map = TempoMap::new(
                self.project.tempo_bpm,
                self.project.time_signature,
                &self.project.tempo_points,
                &self.metres,
            );
        }
        for &(t, a, b) in &old.perf.held {
            if b <= cut {
                self.out.perf.held.push((t, a, b));
            } else if a < cut {
                if let Some(ti) = self.track_index(t) {
                    self.tracks[ti].held = Some(a);
                }
            }
        }
        // Which gate and repeat clips are held down.
        for (i, t) in inputs.iter().enumerate().filter(|(i, _)| applied[*i]) {
            let _ = i;
            match t.input {
                Input::Launch { slot, .. } => self.hold_down(slot, true),
                Input::Release { slot } => self.hold_down(slot, false),
                Input::LaunchScene { scene, .. } | Input::ReleaseScene { scene } => {
                    let down = matches!(t.input, Input::LaunchScene { .. });
                    for k in 0..self.project.tracks.len() {
                        let slot = SlotRef::new(self.project.tracks[k].id, scene);
                        self.hold_down(slot, down);
                    }
                }
                _ => {}
            }
        }
        for x in &old.perf.played {
            if x.until <= cut {
                self.out.perf.played.push(*x);
                continue;
            }
            if x.at >= cut {
                continue;
            }
            let Some(ti) = self.track_index(x.slot.track) else {
                continue;
            };
            self.tracks[ti].seg = Some(Seg {
                slot: x.slot,
                at: x.at,
                from: x.from,
                velocity: x.velocity,
                origin: x.origin,
            });
            let gen = self.tracks[ti].gen;
            let Some(c) = self.clip(x.slot) else {
                // Its clip is gone: it stops where the kept plan ends.
                self.push(cut, Item::End { track: ti, gen });
                continue;
            };
            if c.follow.is_active() {
                // A follow time now shorter than what already played: the next time it
                // comes round (a pass, for a linked one), not at once mid-bar.
                let due = x.at + follow_time(c, x.from, &c.follow).max(Ticks(1));
                let period = match c.follow.after {
                    FollowAfter::Time(t) => Ticks(t.0 * i64::from(c.follow.count.max(1))),
                    FollowAfter::Linked => c.pass(),
                };
                self.push(next_due(due, period, cut), Item::Follow { track: ti, gen });
            }
            if !c.looping() {
                let end = x.at + (c.end() - x.from).max(Ticks(1));
                self.push(end.max(cut), Item::End { track: ti, gen });
            }
            if c.launch.mode == LaunchMode::Repeat && self.tracks[ti].holding == Some(x.slot) {
                let grid = c.launch.quantize.grid(self.global);
                if grid != QuantGrid::None {
                    let next = self.boundary(cut, grid);
                    self.push(next, Item::Retrigger { track: ti, gen });
                }
            }
        }
        if let Some((scene, gen)) = self.scene {
            if let Some(wait) = self.scene_wait(scene) {
                let due = next_due(self.scene_at + wait, wait, cut);
                self.push(due, Item::SceneFollow { gen });
            }
        }
    }

    fn hold_down(&mut self, slot: SlotRef, down: bool) {
        let Some(ti) = self.track_index(slot.track) else {
            return;
        };
        let held = self
            .clip(slot)
            .is_some_and(|c| matches!(c.launch.mode, LaunchMode::Gate | LaunchMode::Repeat));
        if down && held {
            self.tracks[ti].holding = Some(slot);
        } else if !down && self.tracks[ti].holding == Some(slot) {
            self.tracks[ti].holding = None;
        }
    }

    /// How long a launched scene waits for its follow action (none: no follow).
    fn scene_wait(&self, scene: SceneId) -> Option<Ticks> {
        let sc = self.project.session.scene(scene)?;
        if !sc.follow.is_active() {
            return None;
        }
        let n = i64::from(sc.follow.count.max(1));
        let wait = match sc.follow.after {
            FollowAfter::Time(t) => Ticks(t.0 * n),
            FollowAfter::Linked => {
                let longest = self
                    .project
                    .tracks
                    .iter()
                    .filter_map(|t| t.slot_clip(scene))
                    .map(|c| c.first_pass())
                    .max()
                    .unwrap_or(Ticks::ZERO);
                Ticks(longest.0 * n)
            }
        };
        (wait > Ticks::ZERO).then_some(wait)
    }
}

/// The performance as the projection plays it: every looping clip still playing at the
/// horizon, and every track still held there, carried `bars` bars further (ADR 0076 §7,
/// the stall policy) — an interface that stalls past its horizon leaves them looping, and
/// the conductor, when it is back, plans on from what was heard. A follow chain does not
/// go on past the horizon while it stalls: its last clip loops.
pub fn open_ended(project: &Project, perf: &Performance, horizon: Ticks, bars: i64) -> Performance {
    let map = project.tempo_map();
    let far = map.add_bars(horizon, bars);
    let mut out = perf.clone();
    for x in &mut out.played {
        if x.until == horizon && project.slot_clip(x.slot).is_some_and(|c| c.looping()) {
            x.until = far;
        }
    }
    for h in &mut out.held {
        if h.2 == horizon {
            h.2 = far;
        }
    }
    out
}

/// The project that plays `perf`: `project` with it written into the arrangement (as a
/// commit writes it) and its loop switched off while the session holds a track — the
/// session plays on through the timeline, as Live's does.
pub fn project_play(project: &Project, perf: &Performance) -> Project {
    let (mut p, _) = super::edit::perform_as(project, perf, false);
    if !perf.held.is_empty() {
        p.loop_enabled = false;
    }
    p
}
