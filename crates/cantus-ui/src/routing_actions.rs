//! The routing actions every view calls (round 25, ADR 0041): the desk's cables and send
//! row, the Routing tab's matrix, the route menu, the route-here plate and the inspector
//! all make, move and remove routes — and move strips and groups along the desk — through
//! these methods, so one gesture means the same thing everywhere.
//!
//! Each action is **one** `Command::Batch` with its own `cmd.*` label (one step of undo),
//! built by `cantus_core::routing::plan` / `::order` from the project (the undo fuzz runs
//! the same builders). Ids for new strips and inserts are allocated here. Before a gesture
//! ends, a view asks `Project::route_check` whether the route can be made, and why not
//! ([`refusal_text`]); a refused gesture issues nothing.
//!
//! The owner's answers (2026-10-10) these follow: a plain gesture from a source to a
//! destination makes a **send** at 0 dB, post-fader (2, 3); **Ctrl** makes the destination
//! the strip's **only output**, sends untouched (2); **Alt** keys a sidechain; a send to
//! where the output already goes is allowed but flagged ([`doubled_text`], 5); a track or
//! a channel-rack channel dropped on a strip routes it (11); strips and groups move by
//! dragging and by Alt+←/→, one undo step each, docks respected (12).

use crate::app::CantusApp;
use crate::i18n::{t, tf};
use cantus_core::routing::order::{self, Spot};
use cantus_core::routing::{self, plan, End, Route, RouteKind, RouteRefusal};
use cantus_core::{
    linear_to_db, EffectId, EffectKind, EffectSlot, GroupId, Project, SendTap, StripId, TrackId,
};
use std::fmt::Write as _;

/// What the modifiers held when a routing gesture ends make (the owner's answer 2).
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum Gesture {
    /// Nothing held: a send.
    Send,
    /// Ctrl (⌘): the destination becomes the only output.
    Output,
    /// Alt: a sidechain key.
    Key,
}

impl Gesture {
    pub fn from_modifiers(m: egui::Modifiers) -> Gesture {
        if m.alt {
            Gesture::Key
        } else if m.command || m.ctrl {
            Gesture::Output
        } else {
            Gesture::Send
        }
    }

    /// The kind of route the gesture asks `route_check` about.
    pub fn kind(self) -> RouteKind {
        match self {
            Gesture::Send => RouteKind::Send,
            Gesture::Output => RouteKind::Output,
            Gesture::Key => RouteKind::Key(None),
        }
    }
}

/// The payload a track (an arrangement header) or a channel-rack channel carries while it
/// is dragged onto the desk (`egui::DragAndDrop`); a strip that receives it calls
/// [`CantusApp::route_tracks_here`]. Answer 11.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct TrackDrag(pub TrackId);

/// What a routing gesture did.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Outcome {
    /// One batch was executed.
    Done,
    /// Nothing was issued; the reason for the hint bar.
    Refused(RouteRefusal),
    /// Alt onto a strip with several inserts that take a key: the view shows a menu of
    /// them and calls [`CantusApp::key_sidechain`] with the one picked.
    ChooseKey(Vec<EffectId>),
    /// Alt onto a strip with nothing to key: the view offers *Add a ducking compressor
    /// keyed from …* ([`CantusApp::duck_strip_from`]) and *Add a gate keyed from …*
    /// ([`CantusApp::gate_strip_from`]).
    NothingToKey,
}

/// What a new return carries (*New return ▸ Empty · Reverb · Delay*).
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum ReturnKind {
    Empty,
    /// Spatium at full wet.
    Reverb,
    /// The delay at full wet.
    Delay,
}

impl CantusApp {
    /// The strips a gesture from `from` acts on: the whole selection when `from` is in
    /// it, else `from` alone.
    pub fn gesture_sources(&self, from: StripId) -> Vec<StripId> {
        let sel = self.strip_selection();
        if sel.contains(&from) {
            sel
        } else {
            vec![from]
        }
    }

    /// A routing gesture from `from` (its port, its ▼ chip, a matrix row) to `to` (a
    /// strip, or the master): a send, the only output, or a key, as `gesture` says. A drop
    /// on the master always routes the output there. From a multi-selection, every
    /// selected strip sends or routes (a key is the dragged strip's alone: an insert has
    /// one key).
    pub fn route_gesture(&mut self, from: StripId, to: StripId, gesture: Gesture) -> Outcome {
        let gesture = if to.is_master() {
            Gesture::Output
        } else {
            gesture
        };
        let p = self.project();
        let cmd = match gesture {
            Gesture::Send => plan::add_sends(p, &self.gesture_sources(from), to),
            Gesture::Output => plan::replace_outputs(p, &self.gesture_sources(from), to),
            Gesture::Key => {
                let choices = routing::key_choices(p, from, to);
                match choices.len() {
                    1 => plan::key(p, from, to, choices[0]),
                    0 => {
                        return match p.route_check(from, to, RouteKind::Key(None)) {
                            Err(
                                r @ (RouteRefusal::ToItself
                                | RouteRefusal::Loop { .. }
                                | RouteRefusal::FromMaster
                                | RouteRefusal::UnknownStrip(_)),
                            ) => Outcome::Refused(r),
                            _ => Outcome::NothingToKey,
                        }
                    }
                    _ => return Outcome::ChooseKey(choices),
                }
            }
        };
        match cmd {
            Some(c) => {
                if self.exec(c) {
                    Outcome::Done
                } else {
                    Outcome::Refused(RouteRefusal::NoRoom)
                }
            }
            None => Outcome::Refused(
                self.project()
                    .route_check(from, to, gesture.kind())
                    .err()
                    .unwrap_or(RouteRefusal::AlreadyThere),
            ),
        }
    }

    /// A unity, post-fader send from every one of `sources` into `to`. One undo step.
    pub fn add_sends(&mut self, sources: &[StripId], to: StripId) -> bool {
        self.run(plan::add_sends(self.project(), sources, to))
    }

    /// `to` (a strip or [`StripId::MASTER`]) becomes the only output of every one of
    /// `sources`, their sends untouched. One undo step.
    pub fn replace_output(&mut self, sources: &[StripId], to: StripId) -> bool {
        self.run(plan::replace_outputs(self.project(), sources, to))
    }

    /// Key `effect` on `host` from `from`.
    pub fn key_sidechain(&mut self, from: StripId, host: StripId, effect: EffectId) -> bool {
        self.run(plan::key(self.project(), from, host, effect))
    }

    /// A gate at the end of `strip`'s chain, keyed from `key` — Alt onto a strip with
    /// nothing to key. Returns the new insert, or `None` when the key is refused.
    pub fn gate_strip_from(&mut self, strip: StripId, key: StripId) -> Option<EffectId> {
        let slot = self.bus.new_effect(EffectKind::Gate);
        let effect = slot.id;
        let cmd = plan::key_with(self.project(), key, strip, slot, "cmd.gate_key")?;
        self.exec(cmd).then_some(effect)
    }

    /// A cable's end dragged onto `to`: the route keeps its level, switch, tap and place.
    pub fn move_route(&mut self, route: Route, end: End, to: StripId) -> bool {
        self.run(plan::move_route(self.project(), route, end, to))
    }

    /// Remove a route (its menu, Delete, an end dropped on nothing).
    pub fn remove_route(&mut self, route: Route) -> bool {
        self.run(plan::remove_route(self.project(), route))
    }

    /// *Make this the output*.
    pub fn send_to_output(&mut self, from: StripId, to: StripId) -> bool {
        self.run(plan::send_to_output(self.project(), from, to))
    }

    /// *Make this a send*.
    pub fn output_to_send(&mut self, from: StripId) -> bool {
        self.run(plan::output_to_send(self.project(), from))
    }

    /// *New return ▸*: a strip at the end of the right dock, named "Return n" (or after
    /// its effect), in the returns' colour, with a reverb or a delay at full wet, and a
    /// unity send into it from every one of `sources`. Selected afterwards, so the next
    /// thing typed renames it. One undo step.
    pub fn new_return(&mut self, sources: &[StripId], kind: ReturnKind) -> Option<StripId> {
        let effect = match kind {
            ReturnKind::Empty => None,
            ReturnKind::Reverb => Some(self.full_wet(EffectKind::Space)),
            ReturnKind::Delay => Some(self.full_wet(EffectKind::Delay)),
        };
        let name = match &effect {
            Some(e) => t(e.spec.kind.name_key()),
            None => {
                let n = self
                    .project()
                    .strips
                    .iter()
                    .filter(|s| s.dock == cantus_core::Dock::Right)
                    .count();
                tf("routing.return_name", &[("n", &(n + 1))])
            }
        };
        let mut strip = self.bus.new_strip(name);
        strip.color = Some(crate::demos::song::colors::RETURN);
        let id = strip.id;
        let cmd = plan::new_return(self.project(), sources, strip, effect)?;
        if self.exec(cmd) {
            self.select_strip(id);
            Some(id)
        } else {
            None
        }
    }

    fn full_wet(&mut self, kind: EffectKind) -> EffectSlot {
        let mut slot = self.new_insert(kind);
        slot.spec.set("mix", 1.0);
        slot
    }

    /// Tracks (or channel-rack channels) routed onto a strip (`None`: the master): a drop
    /// on the desk, or the route-here menu. One undo step; the strip is selected.
    pub fn route_tracks_here(&mut self, tracks: &[TrackId], to: Option<StripId>) -> bool {
        let ok = self.run(plan::route_tracks(self.project(), tracks, to));
        if ok {
            if let Some(s) = to {
                self.selected_strip = Some(s);
            }
        }
        ok
    }

    /// The `POST | PRE` switch on a send.
    pub fn set_send_tap(&mut self, from: StripId, to: StripId, tap: SendTap) -> bool {
        self.run(plan::set_send_tap(self.project(), from, to, tap))
    }

    /// A strip or the selection dragged along the desk to `spot`. One undo step.
    pub fn move_strips_to(&mut self, strips: &[StripId], spot: Spot) -> bool {
        self.run(order::move_strips(self.project(), strips, spot))
    }

    /// A group dragged by its separator to `spot`. One undo step.
    pub fn move_group_to(&mut self, group: GroupId, spot: Spot) -> bool {
        self.run(order::move_group(self.project(), group, spot))
    }

    /// Alt+←/→: the selection one place along its dock — the group, when the selection
    /// is a whole group. One undo step.
    pub fn nudge_strips(&mut self, delta: i32) -> bool {
        let sel = self.strip_selection();
        self.run(order::nudge(self.project(), &sel, delta))
    }

    /// Alt+←/→ on a group (its separator focused): the whole group one place.
    pub fn nudge_group(&mut self, group: GroupId, delta: i32) -> bool {
        self.run(order::nudge_group(self.project(), group, delta))
    }

    fn run(&mut self, cmd: Option<cantus_core::Command>) -> bool {
        cmd.is_some_and(|c| self.exec(c))
    }
}

/// How a strip is named in routing texts: its name, its first track's, or its number;
/// the master by its word.
pub fn strip_name(project: &Project, id: StripId) -> String {
    if id.is_master() {
        return t("mixer.master");
    }
    let Some(s) = project.strip(id) else {
        return String::new();
    };
    if !s.name.is_empty() {
        return s.name.clone();
    }
    match project.tracks_on_strip(id).next() {
        Some(tr) => tr.name.clone(),
        None => project
            .strip_number(id)
            .map(|n| n.to_string())
            .unwrap_or_default(),
    }
}

/// A send's level as the hints say it: `−6.0 dB`, `−∞ dB` at zero.
pub fn level_text(level: f32) -> String {
    if level <= 0.0 {
        "−∞ dB".to_string()
    } else {
        format!("{:.1} dB", linear_to_db(level)).replace('-', "−")
    }
}

/// Who feeds a strip, one line per kind ("Tracks: Kick, Snare", "Sends from: Keys
/// −6.0 dB, Pad −12.0 dB PRE (off)"), or the plain "nothing feeds this strip yet" — the
/// name cell's hover. Counts sends and keys as well as tracks (the owner's bug: a return
/// fed by sends said nothing fed it).
pub fn feeders_text(project: &Project, id: StripId) -> String {
    let f = project.feeders(id);
    if f.is_empty() {
        return t("routing.feeders.none");
    }
    let mut out = String::new();
    let mut line = |key: &str, names: String| {
        if !names.is_empty() {
            if !out.is_empty() {
                out.push('\n');
            }
            out.push_str(&tf(key, &[("names", &names)]));
        }
    };
    let join = |v: Vec<String>| v.join(", ");
    line(
        "routing.feeders.tracks",
        join(
            f.tracks
                .iter()
                .filter_map(|t| project.track(*t).map(|t| t.name.clone()))
                .collect(),
        ),
    );
    line(
        "routing.feeders.outputs",
        join(f.outputs.iter().map(|s| strip_name(project, *s)).collect()),
    );
    line(
        "routing.feeders.sends",
        join(
            f.sends
                .iter()
                .map(|(s, send)| {
                    let mut x = format!("{} {}", strip_name(project, *s), level_text(send.level));
                    if send.tap != SendTap::Post {
                        let _ = write!(x, " {}", t(send.tap.label_key()));
                    }
                    if !send.enabled {
                        let _ = write!(x, " {}", t("routing.feeders.off"));
                    }
                    x
                })
                .collect(),
        ),
    );
    line(
        "routing.feeders.keys",
        join(
            f.keys
                .iter()
                .map(|(s, _)| strip_name(project, *s))
                .collect(),
        ),
    );
    line(
        "routing.feeders.notes",
        join(
            f.notes
                .iter()
                .filter_map(|(tr, _)| project.track(*tr).map(|t| t.name.clone()))
                .collect(),
        ),
    );
    out
}

/// Why a route from `from` cannot be made, in words, for the hint bar and a disabled
/// menu row ("Would make a loop: Bus already feeds Keys").
pub fn refusal_text(project: &Project, from: StripId, r: RouteRefusal) -> String {
    match r {
        RouteRefusal::Loop { through } => tf(
            r.label_key(),
            &[
                ("through", &strip_name(project, through)),
                ("from", &strip_name(project, from)),
            ],
        ),
        _ => t(r.label_key()),
    }
}

/// The doubled-signal warning for a send from `from` to `to` (the owner's answer 5), or
/// `None` when the send is not doubled.
pub fn doubled_text(project: &Project, from: StripId, to: StripId) -> Option<String> {
    routing::doubled(project, from, to)
        .then(|| tf("routing.doubled", &[("dest", &strip_name(project, to))]))
}

#[cfg(test)]
mod tests {
    use super::*;
    use cantus_core::{Command, Dock, Send};
    use cantus_engine::EngineHandle;

    fn app() -> (CantusApp, Vec<StripId>) {
        crate::i18n::default_thread_locale(crate::i18n::Locale::En);
        let a = CantusApp::from_parts(None, EngineHandle::dummy(), false);
        let ids = a.project().strips.iter().map(|s| s.id).collect();
        (a, ids)
    }

    /// One undo step restores the project (ids handed out stay handed out).
    fn undo_restores(a: &mut CantusApp, before: &Project) {
        a.undo();
        let mut now = a.project().clone();
        now.next_id = before.next_id;
        assert!(&now == before, "one undo step restores the project");
    }

    #[test]
    fn a_plain_gesture_from_a_selection_sends_from_every_selected_strip() {
        let (mut a, s) = app();
        a.select_strip(s[0]);
        a.toggle_strip_selected(s[1]);
        let before = a.project().clone();
        assert_eq!(a.route_gesture(s[0], s[5], Gesture::Send), Outcome::Done);
        for from in [s[0], s[1]] {
            assert_eq!(
                a.project().strip(from).unwrap().send_to(s[5]),
                Some(&Send::new(s[5]))
            );
            assert_eq!(
                a.project().strip(from).unwrap().output,
                None,
                "the output is unchanged"
            );
        }
        assert_eq!(a.bus.undo_label_key(), Some("cmd.add_send"));
        undo_restores(&mut a, &before);
        // From a strip outside the selection: that strip alone.
        assert_eq!(a.route_gesture(s[3], s[5], Gesture::Send), Outcome::Done);
        assert!(a.project().strip(s[0]).unwrap().sends.is_empty());
    }

    #[test]
    fn ctrl_replaces_the_output_and_alt_keys() {
        let (mut a, s) = app();
        a.exec(Command::SetStripOutput {
            id: s[0],
            output: Some(s[2]),
        });
        a.set_strip_send(s[0], Send::new(s[3]));
        a.end_gesture();
        assert_eq!(a.route_gesture(s[0], s[4], Gesture::Output), Outcome::Done);
        let st = a.project().strip(s[0]).unwrap();
        assert_eq!(
            (st.output, st.sends.len()),
            (Some(s[4]), 1),
            "sends untouched"
        );
        assert_eq!(
            a.route_gesture(s[0], StripId::MASTER, Gesture::Send),
            Outcome::Done
        );
        assert_eq!(
            a.project().strip(s[0]).unwrap().output,
            None,
            "a drop on the master routes"
        );
        // Alt: nothing to key, then one insert (keyed at once), then two (a menu).
        assert_eq!(
            a.route_gesture(s[1], s[6], Gesture::Key),
            Outcome::NothingToKey
        );
        let comp = a.duck_strip_from(s[6], s[7]).expect("a ducking compressor");
        assert_eq!(a.route_gesture(s[1], s[6], Gesture::Key), Outcome::Done);
        assert_eq!(
            a.project()
                .strip(s[6])
                .unwrap()
                .effect(comp)
                .unwrap()
                .sidechain,
            Some(s[1])
        );
        let gate = a.gate_strip_from(s[6], s[7]).expect("a keyed gate");
        let both = a.route_gesture(s[2], s[6], Gesture::Key);
        assert_eq!(both, Outcome::ChooseKey(vec![comp, gate]));
        assert!(a.key_sidechain(s[2], s[6], gate));
    }

    #[test]
    fn a_refused_gesture_issues_nothing() {
        let (mut a, s) = app();
        a.exec(Command::SetStripOutput {
            id: s[0],
            output: Some(s[1]),
        });
        let before = a.project().clone();
        let r = a.route_gesture(s[1], s[0], Gesture::Send);
        assert_eq!(r, Outcome::Refused(RouteRefusal::Loop { through: s[0] }));
        assert!(a.project() == &before);
        assert_eq!(a.bus.undo_label_key(), Some("cmd.set_strip_output"));
        let text = refusal_text(a.project(), s[1], RouteRefusal::Loop { through: s[0] });
        let (one, two) = (strip_name(a.project(), s[0]), strip_name(a.project(), s[1]));
        assert!(
            text.contains(&format!("{one} already feeds {two}")),
            "names both: {text}"
        );
        assert_eq!(
            a.route_gesture(s[0], s[0], Gesture::Key),
            Outcome::Refused(RouteRefusal::ToItself)
        );
    }

    #[test]
    fn a_new_return_is_one_step_selected_on_the_right() {
        let (mut a, s) = app();
        let before = a.project().clone();
        let id = a
            .new_return(&[s[0], s[1]], ReturnKind::Reverb)
            .expect("made");
        let r = a.project().strip(id).unwrap();
        assert_eq!(r.dock, Dock::Right);
        assert_eq!(r.effects[0].spec.kind, EffectKind::Space);
        assert_eq!(r.effects[0].spec.get("mix"), 1.0, "full wet");
        assert_eq!(a.selected_strip, Some(id));
        assert!(a.project().strip(s[1]).unwrap().send_to(id).is_some());
        assert_eq!(a.bus.undo_label_key(), Some("cmd.new_return"));
        undo_restores(&mut a, &before);
        let empty = a.new_return(&[], ReturnKind::Empty).unwrap();
        assert_eq!(a.project().strip(empty).unwrap().name, "Return 4");
    }

    #[test]
    fn tracks_drop_onto_a_strip_and_the_tap_switches() {
        let (mut a, s) = app();
        a.add_track();
        let t = a.project().tracks[0].id;
        assert!(a.route_tracks_here(&[t], Some(s[7])));
        assert_eq!(a.project().tracks[0].strip, Some(s[7]));
        assert_eq!(a.selected_strip, Some(s[7]));
        a.set_strip_send(s[0], Send::new(s[2]));
        a.end_gesture();
        assert!(a.set_send_tap(s[0], s[2], SendTap::Pre));
        assert_eq!(a.project().strip(s[0]).unwrap().sends[0].tap, SendTap::Pre);
        assert!(
            !a.set_send_tap(s[0], s[2], SendTap::Pre),
            "no change, no step"
        );
    }

    #[test]
    fn alt_arrows_move_the_selection_one_undo_step() {
        let (mut a, s) = app();
        a.select_strip(s[1]);
        a.toggle_strip_selected(s[2]);
        let before = a.project().clone();
        a.run_action(crate::keymap::Action::MoveStripRight);
        let order: Vec<StripId> = a.project().strips.iter().take(4).map(|x| x.id).collect();
        assert_eq!(order, vec![s[0], s[3], s[1], s[2]]);
        assert_eq!(a.bus.undo_label_key(), Some("cmd.move_strips"));
        undo_restores(&mut a, &before);
        a.run_action(crate::keymap::Action::MoveStripLeft);
        let order: Vec<StripId> = a.project().strips.iter().take(3).map(|x| x.id).collect();
        assert_eq!(order, vec![s[1], s[2], s[0]]);
    }

    #[test]
    fn the_hover_says_who_feeds_a_return() {
        let (mut a, s) = app();
        let mut send = Send::new(s[5]);
        send.level = 0.5;
        send.tap = SendTap::Pre;
        a.set_strip_send(s[0], send);
        a.end_gesture();
        let text = feeders_text(a.project(), s[5]);
        assert!(text.contains("−6.0 dB") && text.contains("PRE"), "{text}");
        assert_eq!(feeders_text(a.project(), s[6]), t("routing.feeders.none"));
        a.exec(Command::SetStripOutput {
            id: s[0],
            output: Some(s[5]),
        });
        assert!(doubled_text(a.project(), s[0], s[5]).is_some());
        assert!(doubled_text(a.project(), s[1], s[5]).is_none());
    }
}
