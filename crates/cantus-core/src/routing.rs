//! The routing graph read as a whole (round 25, ADR 0041): who feeds a strip, the one
//! answer to "can this route be made?", the doubled-signal mark, and the edge list the
//! desk's cables and the Routing tab's matrix both draw.
//!
//! The graph itself does not change here — one output per strip, up to sixteen sends,
//! sidechain keys on inserts (ADR 0014/0015/0017). This module only *reads* it, the same
//! way for every view, so the desk, the matrix, the menus and the inspector can never
//! disagree about a route. The commands keep their own checks (scripts and the MCP server
//! issue them directly); `tests/routing.rs` holds that [`Project::route_check`] and the
//! commands always give the same answer.
//!
//! [`plan`] builds the batches every routing gesture issues and [`order`] the ones that
//! move strips and groups along the desk: pure functions of the project, so the undo fuzz
//! can exercise exactly what the interface does.

#[path = "routing_order.rs"]
pub mod order;
#[path = "routing_plan.rs"]
pub mod plan;

use crate::effect::EffectSlot;
use crate::model::{EffectId, Project, Send, SendTap, StripId, TrackId, MAX_INSERTS, MAX_SENDS};

/// What kind of route a gesture, a cell or a menu item would make.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum RouteKind {
    /// The strip's one output (to another strip, or to the master as [`StripId::MASTER`]).
    Output,
    /// A send (never to the master: a strip reaches the master by its output).
    Send,
    /// A sidechain key from the source onto an insert of the destination: `Some(effect)`
    /// asks about that insert; `None` asks whether the destination can be keyed at all —
    /// an insert that takes a key and is not keyed from the source yet, or room for a new
    /// keyed insert (the duck and gate offers).
    Key(Option<EffectId>),
}

/// Why a route cannot be made. Every refusal but [`RouteRefusal::AlreadyThere`] means the
/// matching command is refused too; under `AlreadyThere` the command still applies, as a
/// change of the route that exists (a send's level, the same output again).
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum RouteRefusal {
    /// No such strip (a stale id).
    UnknownStrip(StripId),
    /// No such insert on the destination.
    UnknownEffect(EffectId),
    /// The master goes to the device; it is never a source.
    FromMaster,
    /// A send cannot go to the master; every strip already reaches it by its output.
    SendToMaster,
    /// A strip cannot route into itself.
    ToItself,
    /// The route would close a loop: the destination already reaches the source, and
    /// `through` is the strip on that path that feeds the source (the hint names it:
    /// "would make a loop: 9 already feeds 4").
    Loop { through: StripId },
    /// Sixteen sends already, or ten inserts with none left to key.
    NoRoom,
    /// The insert takes no key (a delay, a reverb of the old kind, a plug-in without a
    /// sidechain input).
    NotKeyable,
    /// That route exists already.
    AlreadyThere,
}

impl RouteRefusal {
    /// The i18n key of the reason, for hints and disabled menu rows. A loop's text takes
    /// `{through}` and `{from}` (the strips' names).
    pub fn label_key(&self) -> &'static str {
        match self {
            RouteRefusal::UnknownStrip(_) => "routing.refusal.unknown_strip",
            RouteRefusal::UnknownEffect(_) => "routing.refusal.unknown_effect",
            RouteRefusal::FromMaster => "routing.refusal.from_master",
            RouteRefusal::SendToMaster => "routing.refusal.send_to_master",
            RouteRefusal::ToItself => "routing.refusal.itself",
            RouteRefusal::Loop { .. } => "routing.refusal.loop",
            RouteRefusal::NoRoom => "routing.refusal.no_room",
            RouteRefusal::NotKeyable => "routing.refusal.not_keyable",
            RouteRefusal::AlreadyThere => "routing.refusal.already_there",
        }
    }
}

/// One route, as a cable, a matrix cell or the route menu names it.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct Route {
    pub from: StripId,
    /// The destination; [`StripId::MASTER`] for an output to the master or a key on a
    /// master insert.
    pub to: StripId,
    /// For a key, `Key(Some(effect))`: the insert of `to` that is keyed.
    pub kind: RouteKind,
}

impl Route {
    pub fn output(from: StripId, to: StripId) -> Route {
        Route {
            from,
            to,
            kind: RouteKind::Output,
        }
    }

    pub fn send(from: StripId, to: StripId) -> Route {
        Route {
            from,
            to,
            kind: RouteKind::Send,
        }
    }

    pub fn key(from: StripId, host: StripId, effect: EffectId) -> Route {
        Route {
            from,
            to: host,
            kind: RouteKind::Key(Some(effect)),
        }
    }
}

/// Which end of a route a drag moves.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum End {
    Source,
    Destination,
}

/// Everything that feeds one strip (or the master), in project order within each list.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Feeders {
    /// Tracks playing into it.
    pub tracks: Vec<TrackId>,
    /// Strips whose output is it.
    pub outputs: Vec<StripId>,
    /// Strips that send to it, with the send (level, switch, tap). A switched-off send is
    /// listed: it is still a route, one click from sounding.
    pub sends: Vec<(StripId, Send)>,
    /// Strips that key one of its inserts: (the key source, the keyed insert).
    pub keys: Vec<(StripId, EffectId)>,
    /// Tracks whose notes open one of its inserts (ADR 0022): (the track, the insert).
    pub notes: Vec<(TrackId, EffectId)>,
}

impl Feeders {
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn len(&self) -> usize {
        self.tracks.len()
            + self.outputs.len()
            + self.sends.len()
            + self.keys.len()
            + self.notes.len()
    }
}

/// Who feeds `id` (a strip, or [`StripId::MASTER`]). The one function every reader uses:
/// the name cell's hover, the route-here plate, the inspector's Receives, the matrix's
/// column summary.
pub fn feeders(project: &Project, id: StripId) -> Feeders {
    let target = (!id.is_master()).then_some(id);
    let mut f = Feeders {
        tracks: project
            .tracks
            .iter()
            .filter(|t| t.strip == target)
            .map(|t| t.id)
            .collect(),
        outputs: project
            .strips
            .iter()
            .filter(|s| s.output == target && s.id != id)
            .map(|s| s.id)
            .collect(),
        ..Feeders::default()
    };
    if !id.is_master() {
        for s in &project.strips {
            if let Some(send) = s.send_to(id) {
                f.sends.push((s.id, *send));
            }
        }
    }
    for e in project
        .effects_of(id)
        .map(|v| v.as_slice())
        .unwrap_or_default()
    {
        if let Some(k) = e.sidechain {
            f.keys.push((k, e.id));
        }
        if let Some(t) = e.midi_from {
            f.notes.push((t, e.id));
        }
    }
    f
}

/// Whether anything at all feeds `id`, without building the lists: what decides the
/// route-here plate.
pub fn is_fed(project: &Project, id: StripId) -> bool {
    let target = (!id.is_master()).then_some(id);
    project.tracks.iter().any(|t| t.strip == target)
        || project.strips.iter().any(|s| {
            s.id != id && (s.output == target || (target.is_some() && s.send_to(id).is_some()))
        })
        || project.effects_of(id).is_some_and(|v| {
            v.iter()
                .any(|e| e.sidechain.is_some() || e.midi_from.is_some())
        })
}

/// Whether `strip`'s output already carries it to `to` — directly, or through the strips
/// downstream (their outputs and their sounding sends). Every strip reaches the master.
pub fn output_reaches(project: &Project, strip: StripId, to: StripId) -> bool {
    if to.is_master() {
        return true;
    }
    let Some(start) = project.strip(strip).and_then(|s| s.output) else {
        return false;
    };
    let mut seen = vec![false; project.strips.len()];
    let mut stack = vec![start];
    while let Some(id) = stack.pop() {
        if id == to {
            return true;
        }
        let Some(i) = project.strip_index(id) else {
            continue;
        };
        if std::mem::replace(&mut seen[i], true) {
            continue;
        }
        let s = &project.strips[i];
        stack.extend(s.output);
        stack.extend(
            s.sends
                .iter()
                .filter(|x| x.effective_level() > 0.0)
                .map(|x| x.to),
        );
    }
    false
}

/// A sounding send from `strip` to a strip its output already reaches: the destination
/// hears the strip twice (the owner's answer 5 — allowed, for parallel compression and
/// the like, but never silent: its cable carries a warning mark and its hint says so).
pub fn doubled(project: &Project, strip: StripId, to: StripId) -> bool {
    project
        .strip(strip)
        .and_then(|s| s.send_to(to))
        .is_some_and(|x| x.effective_level() > 0.0)
        && output_reaches(project, strip, to)
}

/// The inserts of `host` that `from` could key now, in chain order: they take a key and
/// are not keyed from `from` already, and the key would close no loop. One entry: the
/// Alt gesture keys it at once; several: a menu; none: the duck and gate offers.
pub fn key_choices(project: &Project, from: StripId, host: StripId) -> Vec<EffectId> {
    project
        .effects_of(host)
        .map(|chain| {
            chain
                .iter()
                .filter(|e| {
                    project
                        .route_check(from, host, RouteKind::Key(Some(e.id)))
                        .is_ok()
                })
                .map(|e| e.id)
                .collect()
        })
        .unwrap_or_default()
}

/// One route of the graph, as the cables and the matrix draw it.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Edge {
    pub from: StripId,
    /// [`StripId::MASTER`] for an output to the master or a key on a master insert.
    pub to: StripId,
    pub kind: EdgeKind,
}

impl Edge {
    /// An output to the master: implied on the desk except for the selection.
    pub fn is_master_output(&self) -> bool {
        self.to.is_master() && self.kind == EdgeKind::Output
    }
}

#[derive(Copy, Clone, Debug, PartialEq)]
pub enum EdgeKind {
    Output,
    Send {
        level: f32,
        enabled: bool,
        tap: SendTap,
    },
    Key {
        effect: EffectId,
    },
}

/// Every route of the project, once each: per strip in project order its output (the
/// master's included) and then its sends in list order; then every key, per keyed host in
/// project order and the master last, in chain order.
pub fn edges(project: &Project) -> Vec<Edge> {
    let mut out = Vec::new();
    edges_into(project, &mut out);
    out
}

/// [`edges`] into a buffer that keeps its capacity (the desk rebuilds it per revision).
pub fn edges_into(project: &Project, out: &mut Vec<Edge>) {
    out.clear();
    for s in &project.strips {
        out.push(Edge {
            from: s.id,
            to: s.output.unwrap_or(StripId::MASTER),
            kind: EdgeKind::Output,
        });
        out.extend(s.sends.iter().map(|x| Edge {
            from: s.id,
            to: x.to,
            kind: EdgeKind::Send {
                level: x.level,
                enabled: x.enabled,
                tap: x.tap,
            },
        }));
    }
    let hosts = project
        .strips
        .iter()
        .map(|s| (s.id, s.effects.as_slice()))
        .chain(std::iter::once((
            StripId::MASTER,
            project.master.effects.as_slice(),
        )));
    for (host, chain) in hosts {
        out.extend(chain.iter().filter_map(|e| {
            e.sidechain.map(|from| Edge {
                from,
                to: host,
                kind: EdgeKind::Key { effect: e.id },
            })
        }));
    }
}

impl Project {
    /// Who feeds `id` — see [`feeders`].
    pub fn feeders(&self, id: StripId) -> Feeders {
        feeders(self, id)
    }

    /// Can a route of `kind` from `from` to `to` be made now? The one answer every view
    /// asks — the desk's hatching, the matrix's cells, the menus' disabled rows, the hint
    /// texts — agreeing with the commands (`tests/routing.rs`): `Ok` means the command
    /// that makes it applies; [`RouteRefusal::AlreadyThere`] means it applies as a change
    /// of the route already there; any other refusal means it is refused.
    pub fn route_check(
        &self,
        from: StripId,
        to: StripId,
        kind: RouteKind,
    ) -> Result<(), RouteRefusal> {
        use RouteRefusal::*;
        if from.is_master() {
            return Err(FromMaster);
        }
        let Some(src) = self.strip(from) else {
            return Err(UnknownStrip(from));
        };
        match kind {
            RouteKind::Output => {
                if to.is_master() {
                    return if src.output.is_none() {
                        Err(AlreadyThere)
                    } else {
                        Ok(())
                    };
                }
                if self.strip(to).is_none() {
                    return Err(UnknownStrip(to));
                }
                if to == from {
                    return Err(ToItself);
                }
                if src.output == Some(to) {
                    return Err(AlreadyThere);
                }
                self.no_loop(from, to)
            }
            RouteKind::Send => {
                if to.is_master() {
                    return Err(SendToMaster);
                }
                if self.strip(to).is_none() {
                    return Err(UnknownStrip(to));
                }
                if to == from {
                    return Err(ToItself);
                }
                if src.send_to(to).is_some() {
                    return Err(AlreadyThere);
                }
                self.no_loop(from, to)?;
                if src.sends.len() >= MAX_SENDS {
                    return Err(NoRoom);
                }
                Ok(())
            }
            RouteKind::Key(effect) => {
                let Some(chain) = self.effects_of(to) else {
                    return Err(UnknownStrip(to));
                };
                if to == from {
                    return Err(ToItself);
                }
                self.no_loop(from, to)?;
                match effect {
                    Some(e) => {
                        let slot = chain.iter().find(|x| x.id == e).ok_or(UnknownEffect(e))?;
                        if !slot.spec.takes_sidechain() {
                            return Err(NotKeyable);
                        }
                        if slot.sidechain == Some(from) {
                            return Err(AlreadyThere);
                        }
                        Ok(())
                    }
                    None => key_anywhere(chain, from),
                }
            }
        }
    }

    /// `Err(Loop)` when `to` already reaches `from` (outputs, sends and keys alike: the
    /// processing order's edges), naming the strip on that path that feeds `from`.
    fn no_loop(&self, from: StripId, to: StripId) -> Result<(), RouteRefusal> {
        match self.loop_through(from, to) {
            Some(through) => Err(RouteRefusal::Loop { through }),
            None => Ok(()),
        }
    }

    fn loop_through(&self, from: StripId, to: StripId) -> Option<StripId> {
        if !self.routing_would_cycle(from, Some(to)) {
            return None;
        }
        // A breadth-first walk from `to`: the first strip seen to feed `from` closes it.
        let mut queue = std::collections::VecDeque::from([to]);
        let mut seen = vec![to];
        while let Some(id) = queue.pop_front() {
            for next in self.downstream(id) {
                if next == from {
                    return Some(id);
                }
                if !seen.contains(&next) {
                    seen.push(next);
                    queue.push_back(next);
                }
            }
        }
        // `routing_would_cycle` found one; the walk above always does too.
        Some(to)
    }
}

/// `Key(None)`: an insert that can be keyed anew, or room for a new keyed one.
fn key_anywhere(chain: &[EffectSlot], from: StripId) -> Result<(), RouteRefusal> {
    if chain
        .iter()
        .any(|e| e.spec.takes_sidechain() && e.sidechain != Some(from))
        || chain.len() < MAX_INSERTS
    {
        Ok(())
    } else if chain.iter().any(|e| e.sidechain == Some(from)) {
        Err(RouteRefusal::AlreadyThere)
    } else {
        Err(RouteRefusal::NoRoom)
    }
}
