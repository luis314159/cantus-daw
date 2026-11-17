//! The batches every routing gesture issues (round 25, ADR 0041): each a pure function of
//! the project returning **one** [`Command::Batch`] with its own `cmd.*` label — one step
//! of undo — or `None` when there is nothing to do or the route is refused (ask
//! [`Project::route_check`] first to say why). The interface (`cantus-ui`'s
//! `routing_actions`) allocates ids and calls these; the undo fuzz calls them too.
//!
//! Labels: `cmd.add_send`, `cmd.replace_output`, `cmd.key_sidechain`, the caller's label
//! for [`key_with`] (`cmd.comp_duck`, `cmd.gate_key`), `cmd.move_route`,
//! `cmd.remove_route`, `cmd.send_to_output`, `cmd.output_to_send`, `cmd.new_return`,
//! `cmd.route_tracks`, `cmd.set_send_tap`, `cmd.new_bus`.

use super::{End, Route, RouteKind, RouteRefusal};
use crate::automation::AutoTarget;
use crate::command::Command;
use crate::effect::EffectSlot;
use crate::model::{
    Dock, EffectId, MixerStrip, Project, Send, SendTap, StripId, TrackId, MAX_INSERTS,
};

/// Every automation clip riding `old` made to ride `new` instead (a moved send keeps its
/// automation): one `SetClipAutomation` per clip, a link group's first member only — its
/// siblings follow it (`links`), and the inverse names one clip the same way.
fn retarget(project: &Project, old: &AutoTarget, new: &AutoTarget, out: &mut Vec<Command>) {
    let mut groups: Vec<crate::model::LinkId> = Vec::new();
    for t in &project.tracks {
        for c in &t.clips {
            let Some(a) = c.automation.as_ref().filter(|a| a.target == *old) else {
                continue;
            };
            if let Some(l) = c.link {
                if groups.contains(&l) {
                    continue;
                }
                groups.push(l);
            }
            out.push(Command::SetClipAutomation {
                track: t.id,
                clip: c.id,
                automation: Some(crate::automation::Automation {
                    target: new.clone(),
                    points: a.points.clone(),
                }),
            });
        }
    }
}

fn batch(label: &str, commands: Vec<Command>) -> Option<Command> {
    (!commands.is_empty()).then(|| Command::Batch {
        label_key: label.to_string(),
        commands,
    })
}

/// The sources in order, each once.
fn each_once(sources: &[StripId]) -> Vec<StripId> {
    let mut out: Vec<StripId> = Vec::with_capacity(sources.len());
    for s in sources {
        if !out.contains(s) {
            out.push(*s);
        }
    }
    out
}

/// The plain gesture (the owner's answer 2): a unity, post-fader send from every source
/// into `to` (answer 3). Sources that cannot send there — itself, a loop, a send already
/// there, no room — are skipped.
pub fn add_sends(project: &Project, sources: &[StripId], to: StripId) -> Option<Command> {
    let commands = each_once(sources)
        .into_iter()
        .filter(|s| project.route_check(*s, to, RouteKind::Send).is_ok())
        .map(|id| Command::SetStripSend {
            id,
            send: Send::new(to),
        })
        .collect();
    batch("cmd.add_send", commands)
}

/// Ctrl + the gesture (answer 2): `to` becomes each source's only output (the old one is
/// gone — there is one output), its sends untouched. `to` may be [`StripId::MASTER`].
pub fn replace_outputs(project: &Project, sources: &[StripId], to: StripId) -> Option<Command> {
    let output = (!to.is_master()).then_some(to);
    let commands = each_once(sources)
        .into_iter()
        .filter(|s| project.route_check(*s, to, RouteKind::Output).is_ok())
        .map(|id| Command::SetStripOutput { id, output })
        .collect();
    batch("cmd.replace_output", commands)
}

/// Alt + the gesture onto an insert: key `effect` on `host` from `from`.
pub fn key(project: &Project, from: StripId, host: StripId, effect: EffectId) -> Option<Command> {
    project
        .route_check(from, host, RouteKind::Key(Some(effect)))
        .ok()?;
    batch(
        "cmd.key_sidechain",
        vec![Command::SetEffectSidechain {
            strip: host,
            effect,
            source: Some(from),
        }],
    )
}

/// Alt + the gesture onto a strip with nothing to key: `slot` (a ducking compressor, a
/// gate — made by the caller, with a fresh id) goes in at the end of `host`'s chain, keyed
/// from `from`, under the caller's `label`.
pub fn key_with(
    project: &Project,
    from: StripId,
    host: StripId,
    slot: EffectSlot,
    label: &str,
) -> Option<Command> {
    let len = project.effects_of(host)?.len();
    if len >= MAX_INSERTS || !slot.spec.takes_sidechain() {
        return None;
    }
    // With room for the new insert, only an unknown strip, the strip itself or a loop
    // refuses.
    project.route_check(from, host, RouteKind::Key(None)).ok()?;
    let effect = slot.id;
    batch(
        label,
        vec![
            Command::InsertEffect {
                strip: host,
                index: len,
                slot: Box::new(slot),
            },
            Command::SetEffectSidechain {
                strip: host,
                effect,
                source: Some(from),
            },
        ],
    )
}

/// A cable's end dragged onto another strip. A send keeps its place in the list, its
/// level, switch and tap, and the automation clips riding it (retargeted in the same
/// batch); an output keeps its source; a key moves only by its source (it
/// belongs to its insert). `None` when the move is refused or changes nothing.
pub fn move_route(project: &Project, route: Route, end: End, to: StripId) -> Option<Command> {
    let commands = match (route.kind, end) {
        (RouteKind::Output, End::Destination) => {
            let src = project.strip(route.from)?;
            if src.output.unwrap_or(StripId::MASTER) != route.to {
                return None;
            }
            project
                .route_check(route.from, to, RouteKind::Output)
                .ok()?;
            vec![Command::SetStripOutput {
                id: route.from,
                output: (!to.is_master()).then_some(to),
            }]
        }
        (RouteKind::Output, End::Source) => {
            if route.to.is_master() || to == route.from {
                return None;
            }
            if project.strip(route.from)?.output != Some(route.to) {
                return None;
            }
            project.route_check(to, route.to, RouteKind::Output).ok()?;
            vec![
                Command::SetStripOutput {
                    id: route.from,
                    output: None,
                },
                Command::SetStripOutput {
                    id: to,
                    output: Some(route.to),
                },
            ]
        }
        (RouteKind::Send, End::Destination) => {
            let src = project.strip(route.from)?;
            let index = src.sends.iter().position(|x| x.to == route.to)?;
            let send = src.sends[index];
            // One send leaves as the other comes: a full list is no reason to refuse.
            match project.route_check(route.from, to, RouteKind::Send) {
                Ok(()) | Err(RouteRefusal::NoRoom) => {}
                Err(_) => return None,
            }
            let mut c = vec![
                Command::RemoveStripSend {
                    id: route.from,
                    to: route.to,
                },
                Command::InsertStripSend {
                    id: route.from,
                    index,
                    send: Send { to, ..send },
                },
            ];
            retarget(
                project,
                &AutoTarget::Send {
                    strip: route.from,
                    to: route.to,
                },
                &AutoTarget::Send {
                    strip: route.from,
                    to,
                },
                &mut c,
            );
            c
        }
        (RouteKind::Send, End::Source) => {
            let send = *project.strip(route.from)?.send_to(route.to)?;
            project.route_check(to, route.to, RouteKind::Send).ok()?;
            let mut c = vec![
                Command::RemoveStripSend {
                    id: route.from,
                    to: route.to,
                },
                Command::SetStripSend { id: to, send },
            ];
            retarget(
                project,
                &AutoTarget::Send {
                    strip: route.from,
                    to: route.to,
                },
                &AutoTarget::Send {
                    strip: to,
                    to: route.to,
                },
                &mut c,
            );
            c
        }
        (RouteKind::Key(Some(effect)), End::Source) => {
            let slot = project
                .effects_of(route.to)?
                .iter()
                .find(|e| e.id == effect)?;
            if slot.sidechain != Some(route.from) {
                return None;
            }
            project
                .route_check(to, route.to, RouteKind::Key(Some(effect)))
                .ok()?;
            vec![Command::SetEffectSidechain {
                strip: route.to,
                effect,
                source: Some(to),
            }]
        }
        (RouteKind::Key(_), _) => return None,
    };
    batch("cmd.move_route", commands)
}

/// Remove a route (its menu, Delete, an end dropped on nothing). An output goes back to
/// the master; the master route itself cannot be removed.
pub fn remove_route(project: &Project, route: Route) -> Option<Command> {
    let command = match route.kind {
        RouteKind::Output => {
            if route.to.is_master() || project.strip(route.from)?.output != Some(route.to) {
                return None;
            }
            Command::SetStripOutput {
                id: route.from,
                output: None,
            }
        }
        RouteKind::Send => {
            project.strip(route.from)?.send_to(route.to)?;
            Command::RemoveStripSend {
                id: route.from,
                to: route.to,
            }
        }
        RouteKind::Key(Some(effect)) => {
            let slot = project
                .effects_of(route.to)?
                .iter()
                .find(|e| e.id == effect)?;
            if slot.sidechain != Some(route.from) {
                return None;
            }
            Command::SetEffectSidechain {
                strip: route.to,
                effect,
                source: None,
            }
        }
        RouteKind::Key(None) => return None,
    };
    batch("cmd.remove_route", vec![command])
}

/// *Make this the output*: the send to `to` goes and `to` becomes the output (the old
/// output is replaced). Already the output: the doubled send just goes. An automation clip
/// riding the send is left as it is — an output has no level to ride — and rides again if
/// the route is made a send once more ([`output_to_send`]), as after removing a send.
pub fn send_to_output(project: &Project, from: StripId, to: StripId) -> Option<Command> {
    let src = project.strip(from)?;
    src.send_to(to)?;
    let mut commands = vec![Command::RemoveStripSend { id: from, to }];
    if src.output != Some(to) {
        commands.push(Command::SetStripOutput {
            id: from,
            output: Some(to),
        });
    }
    batch("cmd.send_to_output", commands)
}

/// *Make this a send*: the output goes back to the master and a unity post-fader send
/// takes its place, so the destination hears what it heard. A send already there stays as
/// it is.
pub fn output_to_send(project: &Project, from: StripId) -> Option<Command> {
    let src = project.strip(from)?;
    let to = src.output?;
    let mut commands = vec![Command::SetStripOutput {
        id: from,
        output: None,
    }];
    if src.send_to(to).is_none() {
        if src.sends.len() >= crate::model::MAX_SENDS {
            return None;
        }
        commands.push(Command::SetStripSend {
            id: from,
            send: Send::new(to),
        });
    }
    batch("cmd.output_to_send", commands)
}

/// *New return ▸*: `strip` (made by the caller: a fresh id, its name and colour) goes in
/// at the end of the desk in the right dock, `effect` (a reverb or a delay at full wet, or
/// nothing) in its chain, and a unity send into it from every source that can send.
pub fn new_return(
    project: &Project,
    sources: &[StripId],
    mut strip: MixerStrip,
    effect: Option<EffectSlot>,
) -> Option<Command> {
    if project.strip(strip.id).is_some() {
        return None;
    }
    strip.dock = Dock::Right;
    let id = strip.id;
    let mut commands = vec![Command::InsertStrip {
        index: project.strips.len(),
        strip: Box::new(strip),
    }];
    if let Some(slot) = effect {
        commands.push(Command::InsertEffect {
            strip: id,
            index: 0,
            slot: Box::new(slot),
        });
    }
    commands.extend(
        each_once(sources)
            .into_iter()
            .filter(|s| {
                project
                    .strip(*s)
                    .is_some_and(|st| st.sends.len() < crate::model::MAX_SENDS)
            })
            .map(|s| Command::SetStripSend {
                id: s,
                send: Send::new(id),
            }),
    );
    batch("cmd.new_return", commands)
}

/// What [`new_bus`] would route: the strips whose outputs go to the bus (each once, in
/// the order given, a selected track's strip after the strips) and the tracks that feed the
/// master with no strip of their own (they feed the bus). VCAs (no audio), strips not on
/// the desk and unknown ids are left out.
pub fn bus_sources(
    project: &Project,
    strips: &[StripId],
    tracks: &[TrackId],
) -> (Vec<StripId>, Vec<TrackId>) {
    let mut out: Vec<StripId> = Vec::new();
    let mut direct: Vec<TrackId> = Vec::new();
    let take = |s: StripId, out: &mut Vec<StripId>| {
        let ok = project
            .strip(s)
            .is_some_and(|st| st.on_desk() && !project.is_vca(s));
        if ok && !out.contains(&s) {
            out.push(s);
        }
    };
    for s in strips {
        take(*s, &mut out);
    }
    for t in tracks {
        match project.track(*t).map(|tr| tr.strip) {
            Some(Some(s)) => take(s, &mut out),
            Some(None) if !direct.contains(t) => direct.push(*t),
            _ => {}
        }
    }
    (out, direct)
}

/// *Route the selected to a new bus* (ADR 0150): `bus` (made by the caller: a fresh id and
/// its name) goes in right after the last of the sources on the desk, in their dock when
/// they share one and in their group when they share one; its output is the output they
/// all share (a nested bus stays nested), else the master; then every source strip's
/// output goes to it and every track that fed the master with no strip of its own feeds
/// it ([`bus_sources`]). Sends are left as they are. One batch, `cmd.new_bus`; `None` when
/// there is nothing to route or `bus`'s id is taken.
pub fn new_bus(
    project: &Project,
    strips: &[StripId],
    tracks: &[TrackId],
    mut bus: MixerStrip,
) -> Option<Command> {
    if project.strip(bus.id).is_some() {
        return None;
    }
    let (sources, direct) = bus_sources(project, strips, tracks);
    if sources.is_empty() && direct.is_empty() {
        return None;
    }
    let of = |s: &StripId| project.strip(*s);
    let shared = |f: &dyn Fn(&MixerStrip) -> Option<StripId>| {
        let mut it = sources.iter().filter_map(of).map(f);
        let first = it.next()?;
        it.all(|x| x == first).then_some(first).flatten()
    };
    // The output they share (it cannot lead back to one of them: each already feeds it,
    // and the model holds no loop), else the master.
    bus.output = shared(&|s| s.output).filter(|o| !sources.contains(o));
    let docks: Vec<Dock> = sources.iter().filter_map(of).map(|s| s.dock).collect();
    bus.dock = match docks.first() {
        Some(d) if docks.iter().all(|x| x == d) => *d,
        _ => Dock::Middle,
    };
    let groups: Vec<Option<crate::model::GroupId>> =
        sources.iter().filter_map(of).map(|s| s.group).collect();
    bus.group = match groups.first() {
        Some(g) if groups.iter().all(|x| x == g) => *g,
        _ => None,
    };
    let index = sources
        .iter()
        .filter_map(|s| project.strip_index(*s))
        .max()
        .map_or(project.strips.len(), |i| i + 1);
    let id = bus.id;
    let mut commands = vec![Command::InsertStrip {
        index,
        strip: Box::new(bus),
    }];
    commands.extend(sources.iter().map(|s| Command::SetStripOutput {
        id: *s,
        output: Some(id),
    }));
    commands.extend(direct.iter().map(|t| Command::SetTrackStrip {
        id: *t,
        strip: Some(id),
    }));
    batch("cmd.new_bus", commands)
}

/// The first number `n` no strip is named `name(n)` by (*Bus 1*, *Bus 2*, …).
pub fn free_number(project: &Project, name: impl Fn(u32) -> String) -> u32 {
    (1..)
        .find(|n| {
            let want = name(*n);
            !project.strips.iter().any(|s| s.name == want)
        })
        .unwrap_or(1)
}

/// Tracks (or channel-rack channels) dropped on a strip, or picked in the route-here
/// menu: each feeds `to` (`None` = the master). Tracks already there are skipped.
pub fn route_tracks(project: &Project, tracks: &[TrackId], to: Option<StripId>) -> Option<Command> {
    if let Some(s) = to {
        project.strip(s)?;
    }
    let mut seen: Vec<TrackId> = Vec::new();
    let commands = tracks
        .iter()
        .filter(|t| {
            let fresh = !seen.contains(t);
            seen.push(**t);
            fresh
        })
        .filter(|t| project.track(**t).is_some_and(|tr| tr.strip != to))
        .map(|t| Command::SetTrackStrip { id: *t, strip: to })
        .collect();
    batch("cmd.route_tracks", commands)
}

/// The `POST | PRE` switch on a send (answer 4).
pub fn set_send_tap(
    project: &Project,
    from: StripId,
    to: StripId,
    tap: SendTap,
) -> Option<Command> {
    let send = *project.strip(from)?.send_to(to)?;
    if send.tap == tap {
        return None;
    }
    batch(
        "cmd.set_send_tap",
        vec![Command::SetStripSend {
            id: from,
            send: Send { tap, ..send },
        }],
    )
}
