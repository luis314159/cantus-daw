//! Automation in the application (ADR 0027): the lanes as the knobs read them, the *last
//! tweaked* parameter, making a clip for a parameter, and naming a target in words.
//!
//! Views never compile anything themselves: [`CantusApp::auto_lanes`] is compiled once per
//! project revision and shared, so a hundred knobs asking "am I automated?" in one frame
//! cost a hundred comparisons.

use crate::app::CantusApp;
use crate::i18n::{t, tf};
use cantus_core::automation::compile::compile;
use cantus_core::automation::range::{base_value, label_key, range_of};
use cantus_core::{AutoLane, AutoPoint, AutoTarget, Command, Project, StripId, Ticks, TrackId};
use std::cell::RefCell;
use std::sync::Arc;

/// The application's automation state.
#[derive(Default)]
pub struct AutoUi {
    /// The parameter the user last moved, from the command that moved it: FL's *Tools ▸
    /// Last tweaked*.
    pub last_tweaked: Option<AutoTarget>,
    /// The lanes, compiled at a revision.
    cache: RefCell<Option<(u64, Arc<Vec<AutoLane>>)>>,
    /// The large editor's own state (`views::auto_editor`).
    pub editor: crate::views::auto_editor::EditorState,
}

impl std::fmt::Debug for AutoUi {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AutoUi")
            .field("last_tweaked", &self.last_tweaked)
            .finish()
    }
}

impl CantusApp {
    /// Every automated target's lane, as of the project now.
    pub fn auto_lanes(&self) -> Arc<Vec<AutoLane>> {
        let rev = self.bus.revision();
        let mut cache = self.automation.cache.borrow_mut();
        match cache.as_ref() {
            Some((r, lanes)) if *r == rev => lanes.clone(),
            _ => {
                let lanes = Arc::new(compile(self.project()));
                *cache = Some((rev, lanes.clone()));
                lanes
            }
        }
    }

    /// Whether a lane drives `target`.
    pub fn is_automated(&self, target: &AutoTarget) -> bool {
        self.auto_lanes().iter().any(|l| &l.target == target)
    }

    /// The value automation gives `target` where the song is heard now, in the
    /// parameter's units; `None` when nothing automates it. What an automated knob shows
    /// (FL: the automation moves the knob, and wins over a hand while it plays).
    pub fn automated_value(&self, target: &AutoTarget) -> Option<f32> {
        let lanes = self.auto_lanes();
        let lane = lanes.iter().find(|l| &l.target == target)?;
        Some(lane.value_at(self.engine.position().0 as f64))
    }

    /// The automated value of the target `f` picks, without building one: `None` when
    /// nothing automates it, `Some(value)` where the song is heard (a knob asks this every
    /// frame, so it allocates nothing).
    pub fn automated_value_where(&self, f: impl Fn(&AutoTarget) -> bool) -> Option<Option<f32>> {
        let lanes = self.auto_lanes();
        let lane = lanes.iter().find(|l| f(&l.target))?;
        Some(Some(lane.value_at(self.engine.position().0 as f64)))
    }

    /// A parameter's menu chose *automate*: open its clip when it has one (the first, in
    /// time), or create one.
    pub fn automate(&mut self, target: AutoTarget) {
        let found = self.project().tracks.iter().find_map(|t| {
            t.clips
                .iter()
                .filter(|c| c.automation.as_ref().is_some_and(|a| a.target == target))
                .min_by_key(|c| c.start)
                .map(|c| (t.id, c.id))
        });
        match found {
            Some((t, c)) => self.open_automation_editor(t, c),
            None => {
                self.create_automation_clip(target);
            }
        }
    }

    /// Remember the parameter a command moved, if it moved one.
    pub fn note_tweak(&mut self, cmd: &Command) {
        if let Some(t) = AutoTarget::of_command(cmd) {
            self.automation.last_tweaked = Some(t);
        }
        // Written into the pass's automation while one records it (ADR 0057).
        self.record_tweak(cmd);
    }

    /// FL's *Tools ▸ Last tweaked ▸ Create automation clip*.
    pub fn automate_last_tweaked(&mut self) -> bool {
        match self.automation.last_tweaked.clone() {
            Some(t) => self.create_automation_clip(t),
            None => {
                self.set_status(t("auto.status.nothing_tweaked"));
                false
            }
        }
    }

    /// Make an automation clip for `target`, starting flat at the parameter's value now:
    /// over the time selection, else the loop, else four bars from the playhead's bar.
    /// It goes under the track the parameter belongs to (for a strip or the master, the
    /// selected track), selected, with the track's lanes shown. One undo step.
    pub fn create_automation_clip(&mut self, target: AutoTarget) -> bool {
        let project = self.project();
        // A key that is set, never automated (Chorda's tuning, ADR 0059).
        if let AutoTarget::Instrument { track, key } = &target {
            let kind = project
                .track(*track)
                .and_then(|t| t.instrument.native())
                .map(|n| n.kind);
            if kind.is_some_and(|k| !k.automatable(key)) {
                self.set_error(t("chorda.not_automatable"));
                return false;
            }
        }
        let (Some(range), Some(value)) = (range_of(project, &target), base_value(project, &target))
        else {
            self.set_error(t("auto.status.no_target"));
            return false;
        };
        let Some(track) = host_track(project, &target, self.selected_track) else {
            self.set_error(t("auto.status.no_track"));
            return false;
        };
        let (start, end) = self.automation_span();
        // A hosted parameter's curve is its normalised value (ADR 0057): where it is now.
        let pos = if target.is_plugin() {
            self.plugin_pos(&target).unwrap_or(0.0)
        } else {
            range.pos(value)
        };
        let name = self.target_name(&target);
        let points = vec![
            AutoPoint::new(Ticks::ZERO, pos),
            AutoPoint::new(end - start, pos),
        ];
        let mut clip = self.bus.new_clip(name.clone(), start, end - start);
        clip.automation = Some(cantus_core::Automation::new(target, points));
        let id = clip.id;
        let mut commands = vec![Command::InsertClip {
            track,
            clip: Box::new(clip),
        }];
        if !self
            .project()
            .track(track)
            .is_some_and(|t| t.automation_open)
        {
            commands.push(Command::SetTrackAutomationOpen {
                id: track,
                open: true,
            });
        }
        let ok = self.exec(Command::Batch {
            label_key: "auto.cmd.create".into(),
            commands,
        });
        if ok {
            self.select_clip(track, id);
            self.set_status(tf("auto.status.created", &[("name", &name)]));
        }
        ok
    }

    /// Open an automation clip in the large editor (the piano roll's area).
    pub fn open_automation_editor(&mut self, track: TrackId, clip: cantus_core::ClipId) {
        self.select_clip(track, clip);
        self.show_central(crate::app::CentralView::PianoRoll);
    }

    /// Where a new automation clip goes in time.
    fn automation_span(&self) -> (Ticks, Ticks) {
        if let Some((a, b)) = self.time_selection.filter(|(a, b)| b > a) {
            return (a, b);
        }
        let p = self.project();
        if let Some(l) = p
            .loop_region
            .as_ref()
            .filter(|l| p.loop_enabled && l.end > l.start)
        {
            return (l.start, l.end);
        }
        // Four bars from the playhead's bar, through the metre changes (ADR 0060).
        let at = crate::views::bars::bar_start(p, self.engine.position().max(Ticks::ZERO));
        (at, crate::views::bars::bars_from(p, at, 4))
    }
}

/// The track an automation clip for `target` lives under: the track the parameter is
/// on; for a strip, the first track feeding it; else the selected track, else the first.
pub fn host_track(
    project: &Project,
    target: &AutoTarget,
    selected: Option<TrackId>,
) -> Option<TrackId> {
    let feeding = |s: StripId| project.tracks_on_strip(s).next().map(|t| t.id);
    let own = match target {
        AutoTarget::TrackGain(t)
        | AutoTarget::TrackPan(t)
        | AutoTarget::Instrument { track: t, .. }
        | AutoTarget::Firmus { track: t, .. } => Some(*t),
        AutoTarget::StripGain(s)
        | AutoTarget::StripPan(s)
        | AutoTarget::StripWidth(s)
        | AutoTarget::Send { strip: s, .. } => feeding(*s),
        AutoTarget::Effect { effect, .. } | AutoTarget::Plugin { effect, .. } => project
            .effect_host(*effect)
            .filter(|h| !h.is_master())
            .and_then(feeding),
        AutoTarget::PluginInstrument { track: t, .. } => Some(*t),
        // A modulator's amount: under the track it lives on (or that feeds its strip).
        AutoTarget::ModAmount(m) => match project.modulator(*m).map(|(h, _, _)| h) {
            Some(cantus_core::ModHost::Track(t)) => Some(t),
            Some(cantus_core::ModHost::Strip(s)) => feeding(s),
            _ => None,
        },
        AutoTarget::MasterGain | AutoTarget::MasterPan | AutoTarget::MasterWidth => None,
    };
    own.filter(|t| project.track(*t).is_some())
        .or(selected.filter(|t| project.track(*t).is_some()))
        .or_else(|| project.tracks.first().map(|t| t.id))
}

/// A strip's name as the desk shows it: its own, else what feeds it, else its number.
fn strip_name(project: &Project, s: StripId) -> String {
    if s.is_master() {
        return t("kit.mixer.master");
    }
    match project.strip(s) {
        Some(st) if !st.name.is_empty() => st.name.clone(),
        _ => match project.tracks_on_strip(s).next() {
            Some(tr) => tr.name.clone(),
            None => tf(
                "auto.target.insert",
                &[("n", &project.strip_number(s).unwrap_or(0).to_string())],
            ),
        },
    }
}

/// What a target is, in words: where, then what — "Keys · Volume", "Fervor · Drive".
pub fn target_name(project: &Project, target: &AutoTarget) -> String {
    let (place, what) = target_parts(project, target);
    format!("{place} \u{b7} {what}")
}

/// [`target_name`] in its two parts: the device or channel, and the parameter.
pub fn target_parts(project: &Project, target: &AutoTarget) -> (String, String) {
    let track = |id: TrackId| {
        project
            .track(id)
            .map(|t| t.name.clone())
            .unwrap_or_else(|| t("auto.target.missing"))
    };
    let param = || {
        label_key(project, target)
            .map(t)
            .unwrap_or_else(|| t("auto.target.missing"))
    };
    match target {
        AutoTarget::TrackGain(id) => (track(*id), t("auto.target.level")),
        AutoTarget::TrackPan(id) => (track(*id), t("auto.target.balance")),
        AutoTarget::StripGain(s) => (strip_name(project, *s), t("auto.target.volume")),
        AutoTarget::StripPan(s) => (strip_name(project, *s), t("auto.target.pan")),
        AutoTarget::StripWidth(s) => (strip_name(project, *s), t("auto.target.width")),
        AutoTarget::Send { strip, to } => (
            format!(
                "{} \u{2192} {}",
                strip_name(project, *strip),
                strip_name(project, *to)
            ),
            t("auto.target.send"),
        ),
        AutoTarget::MasterGain => (t("kit.mixer.master"), t("auto.target.volume")),
        AutoTarget::MasterPan => (t("kit.mixer.master"), t("auto.target.pan")),
        AutoTarget::MasterWidth => (t("kit.mixer.master"), t("auto.target.width")),
        AutoTarget::Effect { effect, .. } => {
            let device = project
                .effect_host(*effect)
                .and_then(|h| project.effect(h, *effect))
                .map(|e| t(e.spec.kind.name_key()))
                .unwrap_or_else(|| t("auto.target.missing"));
            (device, param())
        }
        AutoTarget::Instrument { track: id, .. } => {
            let device = project
                .track(*id)
                .map(|tr| t(tr.instrument.name_key()))
                .unwrap_or_else(|| t("auto.target.missing"));
            (device, param())
        }
        AutoTarget::Firmus { track: id, .. } => {
            let device = project
                .track(*id)
                .map(|tr| t(tr.instrument.name_key()))
                .unwrap_or_else(|| t("auto.target.missing"));
            (device, param())
        }
        // A hosted plug-in's parameter: the plug-in's name, and the parameter by its id
        // (the application names it from the plug-in's own list: `CantusApp::target_name`).
        AutoTarget::Plugin { effect, id } => {
            let device = project
                .effect_host(*effect)
                .and_then(|h| project.effect(h, *effect))
                .and_then(|e| e.spec.external.as_deref())
                .map(|x| x.name.clone())
                .unwrap_or_else(|| t("auto.target.missing"));
            (
                device,
                tf("mods.target.param_id", &[("id", &id.to_string())]),
            )
        }
        AutoTarget::PluginInstrument { track: tr, id } => {
            let device = project
                .track(*tr)
                .and_then(|x| x.instrument.external())
                .map(|x| x.name.clone())
                .unwrap_or_else(|| t("auto.target.missing"));
            (
                device,
                tf("mods.target.param_id", &[("id", &id.to_string())]),
            )
        }
        AutoTarget::ModAmount(m) => {
            let name = crate::views::modulator::mod_name(project, *m)
                .unwrap_or_else(|| t("auto.target.missing"));
            (name, t("mods.target.amount"))
        }
    }
}

/// The parameter's unit, for its readout.
pub fn unit_of(project: &Project, target: &AutoTarget) -> &'static str {
    match target {
        AutoTarget::TrackGain(_) | AutoTarget::StripGain(_) | AutoTarget::MasterGain => "dB",
        AutoTarget::Effect { effect, key } => project
            .effect_host(*effect)
            .and_then(|h| project.effect(h, *effect))
            .and_then(|e| e.spec.kind.param(key))
            .map_or("", |i| i.unit),
        AutoTarget::Instrument { track, key } => project
            .track(*track)
            .and_then(|t| t.instrument.native())
            .and_then(|n| n.kind.param(key))
            .map_or("", |i| i.unit),
        AutoTarget::Firmus { param, .. } => param.info().unit,
        _ => "",
    }
}

/// A target's value as its control reads: "−6.0 dB", "L 30", "120 %", "1.20 kHz".
pub fn write_value(project: &Project, target: &AutoTarget, v: f32, out: &mut String) {
    use std::fmt::Write as _;
    match target {
        AutoTarget::TrackPan(_) | AutoTarget::StripPan(_) | AutoTarget::MasterPan => {
            let p = (v * 100.0).round() as i32;
            let _ = match p {
                0 => write!(out, "C"),
                p if p < 0 => write!(out, "L {}", -p),
                p => write!(out, "R {p}"),
            };
        }
        AutoTarget::StripWidth(_)
        | AutoTarget::MasterWidth
        | AutoTarget::Send { .. }
        | AutoTarget::Plugin { .. }
        | AutoTarget::PluginInstrument { .. }
        | AutoTarget::ModAmount(_) => {
            let _ = write!(out, "{:.0} %", v * 100.0);
        }
        AutoTarget::TrackGain(_) | AutoTarget::StripGain(_) | AutoTarget::MasterGain => {
            if v <= cantus_core::MIN_GAIN_DB {
                let _ = write!(out, "-\u{221e} dB");
            } else {
                let _ = write!(out, "{v:+.1} dB");
            }
        }
        _ => crate::views::fx::controls::write_readout(v, unit_of(project, target), out),
    }
}
