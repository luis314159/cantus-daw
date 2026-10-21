//! A hosted plug-in that stopped working (round 29, ADR 0063), and where plug-ins run.
//!
//! When an isolated plug-in's helper crashes or hangs, Cantus goes on playing — the slot
//! passes its sound through, an instrument falls silent — and this module tells the user,
//! once, in a card at the corner (in the one card stack it shares with the audio notices,
//! `audio_notice`): *"Glacier stopped working. Cantus kept playing without it."* with
//! **Reload** and **Keep it off**, and *Run individually* for a plug-in that keeps crashing.
//! *Reload* loads it again where it runs, with its last-good state and the project's
//! values: what the user had. Several plug-ins that went together (a *Together* helper)
//! share one card, and *Reload all*.
//!
//! It also owns the choice of where plug-ins run: the machine's default (Settings' plug-in
//! manager, applied at start-up in `CantusApp::assemble`), the rows of an insert's menu
//! (*Runs: Within Cantus / Together / By maker / Individually*), and the words of a hosted
//! plug-in's CPU figure.

use crate::app::CantusApp;
use crate::i18n::{t, tf};
use cantus_core::{Command, EffectId, PluginIsolation, StripId, TrackId};
use cantus_host::{Format, HostNotice};

/// What stopped working.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum Target {
    Insert(EffectId),
    Instrument(TrackId),
}

/// One plug-in on a card.
#[derive(Clone, Debug, PartialEq)]
pub struct Crashed {
    pub target: Target,
    pub name: String,
    pub reason: String,
}

/// The plug-ins waiting for an answer: one card holds them all.
#[derive(Clone, Debug, Default)]
pub struct Crashes {
    pub waiting: Vec<Crashed>,
    /// Plug-ins waiting on a window of their own whose card the user closed: not shown
    /// again until they wait anew.
    pub left_waiting: Vec<Target>,
}

/// The waiting card's buttons (review isolation-4 #54).
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum WaitAnswer {
    /// Its window to the front.
    Show,
    /// Its process ended: Cantus plays on without it, and it can be reloaded.
    Stop,
    /// The card closed.
    Dismiss,
}

/// The card of a plug-in waiting on a window of its own (a dialog inside a call of its,
/// perhaps behind Cantus): its name, what happens, and *Show it* / *Stop it*.
#[derive(Clone, Debug, PartialEq)]
pub struct WaitCard {
    pub target: Target,
    pub name: String,
    pub title: String,
    pub body: String,
    /// The plug-in's whole name and the card's words, as a hint on the card (a long name
    /// is shortened in the title — review isolation-5 #48).
    pub tip: (String, String),
    pub buttons: Vec<(String, WaitAnswer)>,
}

/// The waiting card, while a plug-in waits on a window of its own (review isolation-4
/// #54).
pub fn waiting_card(app: &CantusApp) -> Option<WaitCard> {
    let (e, tr) = app.plugins.waiting();
    let (target, name) = e
        .into_iter()
        .map(|(id, n)| (Target::Insert(id), n))
        .chain(tr.into_iter().map(|(id, n)| (Target::Instrument(id), n)))
        .find(|(target, _)| !app.plugin_crashes.left_waiting.contains(target))?;
    let name = if name.is_empty() {
        name_of(app, target)
    } else {
        name
    };
    // Stopping it ends its process: the plug-ins that share it stop too, and the card says
    // so (review isolation-5 #44).
    let others = match target {
        Target::Insert(e) => app.plugins.neighbours(Some(e), None),
        Target::Instrument(tr) => app.plugins.neighbours(None, Some(tr)),
    };
    let body = match others {
        0 => t("isolation.waiting.body"),
        1 => t("isolation.waiting.body_shared_one"),
        n => tf("isolation.waiting.body_shared", &[("n", &n)]),
    };
    Some(WaitCard {
        target,
        tip: (name.clone(), body.clone()),
        name,
        title: t("isolation.waiting.title"),
        body,
        buttons: vec![
            (t("isolation.waiting.show"), WaitAnswer::Show),
            (t("isolation.waiting.stop"), WaitAnswer::Stop),
        ],
    })
}

/// The user answered the waiting card.
pub fn answer_waiting(app: &mut CantusApp, target: Target, a: WaitAnswer) {
    match (a, target) {
        (WaitAnswer::Show, Target::Insert(e)) => app.plugins.surface(e),
        (WaitAnswer::Show, Target::Instrument(tr)) => app.plugins.surface_instrument(tr),
        (WaitAnswer::Stop, Target::Insert(e)) => app.plugins.stop(e),
        (WaitAnswer::Stop, Target::Instrument(tr)) => app.plugins.stop_instrument(tr),
        (WaitAnswer::Dismiss, _) => app.plugin_crashes.left_waiting.push(target),
    }
}

/// The card's buttons, in order.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Answer {
    Reload,
    KeepOff,
    RunIndividually,
}

/// Crashes this session after which a plug-in is offered its own process.
pub const OFFER_INDIVIDUALLY: u32 = 3;

/// The host said something about isolation.
pub fn on_notice(app: &mut CantusApp, n: HostNotice) {
    match n {
        HostNotice::PluginCrashed {
            effect,
            name,
            reason,
        } => add(app, Target::Insert(effect), name, reason),
        HostNotice::InstrumentCrashed {
            track,
            name,
            reason,
        } => add(app, Target::Instrument(track), name, reason),
        HostNotice::IsolationUnavailable => app.set_error(t("isolation.unavailable")),
        _ => {}
    }
}

fn add(app: &mut CantusApp, target: Target, name: String, reason: String) {
    log::warn!("plug-in {name} stopped working: {reason}");
    let w = &mut app.plugin_crashes.waiting;
    w.retain(|c| c.target != target);
    w.push(Crashed {
        target,
        name,
        reason,
    });
}

/// Keeps only the plug-ins that are still stopped: one removed, swapped, reloaded or moved
/// (or a project replaced) leaves the card, which never speaks of what is gone (#16, #30).
/// Every frame, before the card is drawn.
pub fn prune(app: &mut CantusApp) {
    if !app.plugin_crashes.left_waiting.is_empty() {
        let (e, tr) = app.plugins.waiting();
        let still: Vec<Target> = e
            .into_iter()
            .map(|(id, _)| Target::Insert(id))
            .chain(tr.into_iter().map(|(id, _)| Target::Instrument(id)))
            .collect();
        app.plugin_crashes
            .left_waiting
            .retain(|t| still.contains(t));
    }
    if app.plugin_crashes.waiting.is_empty() {
        return;
    }
    let still: Vec<bool> = app
        .plugin_crashes
        .waiting
        .iter()
        .map(|c| stopped(app, c.target).is_some())
        .collect();
    let mut keep = still.into_iter();
    app.plugin_crashes
        .waiting
        .retain(|_| keep.next().unwrap_or(false));
}

/// Why a hosted insert or instrument is stopped, while it is.
pub fn stopped(app: &CantusApp, target: Target) -> Option<&str> {
    match target {
        Target::Insert(e) => app.plugins.crashed(e),
        Target::Instrument(tr) => app.plugins.instrument_crashed(tr),
    }
}

/// The machine's default, from the settings, into the host (start-up, and a change).
pub fn apply_default(app: &mut CantusApp) {
    let mode = app.settings.plugin_isolation;
    if app.plugins.default_isolation != mode {
        app.plugins.default_isolation = mode;
        // Plug-ins that follow the default move at the next sync.
        app.force_resync();
    }
}

/// Settings' plug-in manager chose a new default.
pub fn set_default(app: &mut CantusApp, mode: PluginIsolation) {
    app.settings.plugin_isolation = mode;
    app.store_settings();
    apply_default(app);
}

/// The card's words.
#[derive(Clone, Debug, PartialEq)]
pub struct Card {
    /// For one plug-in: its name, drawn before `title` and shortened in its middle when
    /// long, so the words that matter are never cut (#36).
    pub name: Option<String>,
    /// For one plug-in, what happened ("stopped working"); for several, the whole title.
    pub title: String,
    /// What Cantus did, and for one plug-in why it stopped.
    pub body: String,
    /// The buttons' labels, with what each does.
    pub buttons: Vec<(String, Answer)>,
    /// Every plug-in and why it stopped: the whole card's hint.
    pub reasons: String,
}

impl Card {
    /// The title as one sentence ("Glacier stopped working").
    pub fn sentence(&self) -> String {
        match &self.name {
            Some(n) => tf("isolation.crashed.one", &[("name", n)]),
            None => self.title.clone(),
        }
    }
}

/// The card's words, or `None` when nothing waits.
pub fn card(app: &CantusApp) -> Option<Card> {
    let w = &app.plugin_crashes.waiting;
    let first = w.first()?;
    let one = w.len() == 1;
    let (name, title) = if one {
        (Some(first.name.clone()), t("isolation.crashed.suffix"))
    } else {
        (None, tf("isolation.crashed.many", &[("n", &w.len())]))
    };
    let body = if one {
        format!(
            "{} {}",
            t("isolation.crashed.body_one"),
            as_sentence(&stopped_words(&first.reason))
        )
    } else {
        t("isolation.crashed.body_many")
    };
    let mut buttons = vec![
        (
            t(if one {
                "isolation.reload"
            } else {
                "isolation.reload_all"
            }),
            Answer::Reload,
        ),
        (
            t(if one {
                "isolation.keep_off"
            } else {
                "isolation.keep_off_many"
            }),
            Answer::KeepOff,
        ),
    ];
    // The one that keeps crashing — the one its helper's end is laid on, even when the
    // card lists its neighbours too — is offered a process of its own (#25, #49).
    if let Some(c) = culprit(app) {
        let label = if one {
            t("isolation.run_individually")
        } else {
            // The name shortened in its middle, so the words that say what the button does
            // always show (review #27).
            tf(
                "isolation.run_name_individually",
                &[("name", &short_name(&c.name, 16))],
            )
        };
        buttons.push((label, Answer::RunIndividually));
    }
    let reasons = w
        .iter()
        .map(|c| {
            tf(
                "isolation.reason",
                &[("name", &c.name), ("reason", &stopped_words(&c.reason))],
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    Some(Card {
        name,
        title,
        body,
        buttons,
        reasons,
    })
}

/// The plug-in on the card offered its own process: one its helper's end is laid on, that
/// crashed `OFFER_INDIVIDUALLY` times this session, and does not run alone already.
fn culprit(app: &CantusApp) -> Option<&Crashed> {
    app.plugin_crashes
        .waiting
        .iter()
        .find(|c| offers_individually(app, c.target))
}

/// What a stopped plug-in is (format and id) and where it runs: the loaded instance's
/// word, or the project's record for one that crashed while it loaded.
fn plugin_of(app: &CantusApp, target: Target) -> Option<(Format, String, PluginIsolation)> {
    let loaded = match target {
        Target::Insert(e) => app
            .plugins
            .hosted(e)
            .map(|h| (h.info.format, h.info.id.clone(), h.isolation())),
        Target::Instrument(tr) => app
            .plugins
            .hosted_instrument(tr)
            .map(|h| (h.info.format, h.info.id.clone(), h.isolation())),
    };
    loaded.or_else(|| {
        let p = app.project();
        let record = match target {
            Target::Insert(e) => p
                .all_effects()
                .find(|(_, x)| x.id == e)
                .and_then(|(_, x)| x.spec.external.as_deref()),
            Target::Instrument(tr) => p.track(tr).and_then(|t| t.instrument.external()),
        }?;
        let runs = app.plugins.effective_isolation(record.isolation);
        Some((
            cantus_host::from_project_format(record.format),
            record.id.clone(),
            runs,
        ))
    })
}

fn offers_individually(app: &CantusApp, target: Target) -> bool {
    let culprit = match target {
        Target::Insert(e) => app.plugins.crash_culprit(e),
        Target::Instrument(tr) => app.plugins.instrument_crash_culprit(tr),
    };
    let Some((format, id, runs)) = plugin_of(app, target) else {
        return false;
    };
    culprit
        && runs != PluginIsolation::Individually
        && app.plugins.crashes_this_session(format, &id) >= OFFER_INDIVIDUALLY
}

/// The user answered the card. *Run individually* moves the plug-in that keeps crashing
/// to a process of its own, and reloads the neighbours it took down with it.
pub fn answer(app: &mut CantusApp, a: Answer) {
    let alone = match a {
        Answer::RunIndividually => culprit(app).map(|c| c.target),
        _ => None,
    };
    let waiting = std::mem::take(&mut app.plugin_crashes.waiting);
    match a {
        Answer::KeepOff => {}
        Answer::Reload => {
            for c in &waiting {
                reload(app, c.target, &c.name);
            }
        }
        Answer::RunIndividually => {
            for c in &waiting {
                if Some(c.target) == alone {
                    set_isolation(app, c.target, Some(PluginIsolation::Individually));
                } else {
                    reload(app, c.target, &c.name);
                }
            }
        }
    }
}

/// Loads a crashed plug-in again where it runs, with what the user had.
pub fn reload(app: &mut CantusApp, target: Target, name: &str) {
    let r = match target {
        Target::Insert(e) => app.plugins.reload(app.bus.project(), &mut app.engine, e),
        Target::Instrument(tr) => {
            app.plugins
                .reload_instrument(app.bus.project(), &mut app.engine, tr)
        }
    };
    app.plugin_crashes.waiting.retain(|c| c.target != target);
    match r {
        Ok(()) => app.set_status(tf("isolation.reloaded", &[("name", &name)])),
        Err(error) => {
            // In the interface's words — a failure that is no crash too, never the host's
            // English (review #31; isolation-4 #25, #40); and its card comes back with why
            // it is off now — counted when it crashed, so one that keeps crashing is offered
            // its own process (review #15; isolation-4 #64).
            app.set_error(tf(
                "isolation.reload_failed",
                &[("name", &name), ("error", &stopped_words(&error))],
            ));
            if let Some(why) = stopped(app, target).map(str::to_string) {
                add(app, target, name.to_string(), why);
            }
        }
    }
}

/// Where one plug-in runs (`None`: as this computer's default), as one undo step. A change
/// moves it at the next sync, with its state; a crashed one comes back running there.
pub fn set_isolation(app: &mut CantusApp, target: Target, isolation: Option<PluginIsolation>) {
    let cmd = match target {
        Target::Insert(e) => {
            let Some(strip) = strip_of(app, e) else {
                return;
            };
            Command::SetPluginIsolation {
                strip,
                effect: e,
                isolation,
            }
        }
        Target::Instrument(track) => Command::SetInstrumentIsolation { track, isolation },
    };
    app.plugin_crashes.waiting.retain(|c| c.target != target);
    app.exec(cmd);
}

fn strip_of(app: &CantusApp, e: EffectId) -> Option<StripId> {
    app.project()
        .all_effects()
        .find(|(_, x)| x.id == e)
        .map(|(s, _)| s)
}

/// What the project says about where a hosted insert or instrument runs.
pub fn chosen(app: &CantusApp, target: Target) -> Option<Option<PluginIsolation>> {
    let p = app.project();
    match target {
        Target::Insert(e) => p
            .all_effects()
            .find(|(_, x)| x.id == e)
            .and_then(|(_, x)| x.spec.external.as_deref())
            .map(|x| x.isolation),
        Target::Instrument(tr) => p
            .track(tr)
            .and_then(|t| t.instrument.external())
            .map(|x| x.isolation),
    }
}

/// The rows a hosted plug-in's menu adds: where it runs (a radio list, the default first)
/// and, while it is stopped, *Reload*. Each row: its words, whether it is the one chosen,
/// and what it does.
pub fn menu_rows(app: &CantusApp, target: Target) -> Vec<(String, Option<bool>, MenuAct)> {
    let Some(chosen) = chosen(app, target) else {
        return Vec::new();
    };
    let mut rows = Vec::new();
    // Stopped, or never loaded (refused, or not loaded in time): it can be loaded again
    // (review isolation-4 #57).
    let reloadable = match target {
        Target::Insert(e) => {
            app.plugins.crashed(e).is_some() || app.plugins.load_error(e).is_some()
        }
        Target::Instrument(tr) => {
            app.plugins.instrument_crashed(tr).is_some()
                || app.plugins.instrument_load_error(tr).is_some()
        }
    };
    if reloadable {
        rows.push((t("isolation.slot.reload"), None, MenuAct::Reload));
    }
    let default = t(app.plugins.default_isolation.label_key());
    rows.push((
        tf("isolation.slot.default", &[("mode", &default)]),
        Some(chosen.is_none()),
        MenuAct::Run(None),
    ));
    for m in PluginIsolation::ALL {
        rows.push((
            t(m.label_key()),
            Some(chosen == Some(m)),
            MenuAct::Run(Some(m)),
        ));
    }
    rows
}

/// What a row of [`menu_rows`] does.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum MenuAct {
    Reload,
    Run(Option<PluginIsolation>),
}

/// Runs a row of [`menu_rows`].
pub fn run_menu(app: &mut CantusApp, target: Target, act: MenuAct) {
    match act {
        MenuAct::Reload => {
            let name = name_of(app, target);
            reload(app, target, &name);
        }
        MenuAct::Run(m) => set_isolation(app, target, m),
    }
}

/// A hosted insert's plug-in's name.
pub fn effect_name(app: &CantusApp, effect: EffectId) -> String {
    name_of(app, Target::Insert(effect))
}

fn name_of(app: &CantusApp, target: Target) -> String {
    match target {
        Target::Insert(e) => app.plugins.hosted(e).map(|h| h.info.name.clone()),
        Target::Instrument(tr) => app
            .plugins
            .hosted_instrument(tr)
            .map(|h| h.info.name.clone()),
    }
    .unwrap_or_else(|| plugin_name(app, target))
}

/// A percentage as the CPU figure shows it: "0.42 %", "3.1 %", "12 %".
pub fn percent(share: f32) -> String {
    let p = share * 100.0;
    if p < 1.0 {
        format!("{p:.2}\u{2009}%")
    } else if p < 10.0 {
        format!("{p:.1}\u{2009}%")
    } else {
        format!("{p:.0}\u{2009}%")
    }
}

/// The lines a hosted plug-in's tip adds: where it runs, its CPU (and its own part when
/// isolated), blocks it was late for, and why it stopped when it has.
pub fn tip(app: &CantusApp, target: Target) -> String {
    let crashed = stopped(app, target);
    let loaded = match target {
        Target::Insert(e) => app.plugins.hosted(e).map(|h| (h.isolation(), h.cpu())),
        Target::Instrument(tr) => app
            .plugins
            .hosted_instrument(tr)
            .map(|h| (h.isolation(), h.cpu())),
    };
    // One that crashed while it loaded has no instance: where it was tried, and why.
    let (runs, cpu) = match (loaded, plugin_of(app, target)) {
        (Some(l), _) => l,
        (None, Some((_, _, runs))) if crashed.is_some() => (runs, None),
        _ => return String::new(),
    };
    let mut lines = vec![tf("isolation.tip.runs", &[("mode", &t(runs.label_key()))])];
    if let Some(why) = crashed {
        lines.push(tf(
            "isolation.tip.stopped",
            &[("reason", &stopped_words(why))],
        ));
    } else if let Some(c) = cpu {
        lines.push(tf("isolation.tip.cpu", &[("share", &percent(c.share))]));
        if let Some(own) = c.own {
            lines.push(tf("isolation.tip.own", &[("share", &percent(own))]));
        }
        if c.late > 0 {
            lines.push(tf("isolation.tip.late", &[("n", &c.late)]));
        }
    }
    lines.join("\n")
}

/// A track whose hosted instrument stopped working: its sign over the header's icon, in the
/// danger colour (the arrangement's track header, round 29).
pub fn paint_header_stopped(
    painter: &egui::Painter,
    icon: egui::Rect,
    app: &CantusApp,
    track: TrackId,
    s: &crate::kit::Style,
) {
    if app.plugins.instrument_crashed(track).is_none() {
        return;
    }
    crate::kit::with_brush(painter, |b| {
        b.circle(
            icon.center(),
            icon.width().min(icon.height()) * 0.5,
            crate::kit::Fill::Solid(s.t.panel),
        );
        crate::kit::glyph::paint(b, icon, crate::kit::glyph::Glyph::PluginStopped, s.t.danger);
    });
}

/// A channel whose hosted instrument stopped working: its button outlined in the danger
/// colour (the channel rack, round 29); its tip says why.
pub fn paint_rack_stopped(
    b: &mut crate::kit::Brush,
    button: egui::Rect,
    app: &CantusApp,
    track: TrackId,
    s: &crate::kit::Style,
) {
    if app.plugins.instrument_crashed(track).is_some() {
        b.rrect_stroke(button.shrink(0.5), s.r(3.0), 1.5 * s.k, s.t.danger);
    }
}

/// `name` in at most `max` letters, shortened in its middle ("Spectras…Edition").
pub fn short_name(name: &str, max: usize) -> String {
    let n = name.chars().count();
    if n <= max {
        return name.to_string();
    }
    let head = (max - 1).div_ceil(2);
    let tail = max - 1 - head;
    let first: String = name.chars().take(head).collect();
    let last: String = name.chars().skip(n - tail).collect();
    format!("{}\u{2026}{}", first.trim_end(), last.trim_start())
}

/// `words` as a sentence of its own: a capital first, a full stop last.
fn as_sentence(words: &str) -> String {
    let mut c = words.chars();
    let mut out: String = match c.next() {
        Some(f) => f.to_uppercase().chain(c).collect(),
        None => return String::new(),
    };
    if !out.ends_with('.') {
        out.push('.');
    }
    out
}

/// The host's account of why a plug-in stopped, in the interface's language: the forms
/// `cantus_host::bridge::sandbox` writes (an exception, a signal, a hang, a timeout), with
/// the system's detail kept — the exception's code, the signal's number (#37); anything else
/// as the host said it.
pub fn reason_words(reason: &str) -> String {
    let (code, signal) = system_detail(reason);
    // The common Unix signals, as the Windows exceptions they are.
    let by_signal = match signal {
        Some(11) | Some(7) | Some(10) => Some("isolation.why.access"),
        Some(6) => Some("isolation.why.abort"),
        Some(4) => Some("isolation.why.illegal"),
        Some(8) => Some("isolation.why.division"),
        _ => None,
    };
    let key = if let Some(k) = by_signal {
        k
    } else if reason.contains("access violation") {
        "isolation.why.access"
    } else if reason.contains("stack buffer overrun or an abort") || reason.contains("an abort") {
        "isolation.why.abort"
    } else if reason.contains("stack overflow") {
        "isolation.why.stack"
    } else if reason.contains("illegal instruction") {
        "isolation.why.illegal"
    } else if reason.contains("division by zero") {
        "isolation.why.division"
    } else if reason.contains("processed audio") || reason.contains("while it rendered") {
        "isolation.why.hung_audio"
    } else if reason.contains("stopped answering") {
        "isolation.why.hung"
    } else if reason.contains("stopped by you") || reason.contains("stopped by the user") {
        "isolation.why.stopped_by_you"
    } else if let Some(name) = reason.split("was ended to stop ").nth(1) {
        return tf("isolation.why.ended_for", &[("name", &name)]);
    } else if reason.contains("did not load within") {
        "isolation.why.load_timeout"
    } else if reason.contains("still answering") {
        "isolation.why.answering"
    } else if reason.contains("was not asked for") {
        "isolation.why.not_asked"
    } else if reason.starts_with("state: ") {
        "isolation.why.no_state"
    } else if reason.contains("did not answer in")
        || reason.contains("did not start in time")
        || reason.contains("did not answer while")
    {
        "isolation.why.timeout"
    } else if reason.contains("is busy") || reason.contains("is not answering") {
        "isolation.why.busy"
    } else if reason.contains("sandbox could not start") {
        "isolation.why.no_helper"
    } else if reason.contains("not installed") {
        "isolation.why.not_installed"
    } else if reason.contains("could not restart after its state") {
        "isolation.why.no_restart"
    } else if reason.contains("broke its protocol") {
        "isolation.why.protocol"
    } else if reason.contains("crashed") {
        "isolation.why.crashed"
    } else if reason.contains("ended") {
        "isolation.why.ended"
    } else {
        return reason.to_string();
    };
    match code {
        Some(c) => format!("{} ({c})", t(key)),
        None => t(key),
    }
}

/// [`reason_words`] for a plug-in that is off, or could not be loaded again: a reason in
/// none of the forms the host writes is a load failure — said in the interface's words
/// ("it could not be loaded again"), the host's own text kept for the log (review
/// isolation-4 #25, #40).
pub fn stopped_words(reason: &str) -> String {
    words_or(reason, "isolation.why.load_failed")
}

/// Why a plug-in could not be loaded, in the interface's words (a form the host writes, or
/// "it could not be loaded"; the host's own text kept for the log) — never its English on
/// the status line (review isolation-4 #40).
pub fn load_words(error: &str) -> String {
    words_or(error, "isolation.why.not_loaded")
}

/// [`reason_words`], with `fallback`'s words for a reason in none of the host's forms (the
/// host logs its own text where it records it — this runs while drawing, and says nothing
/// to the log: review isolation-5 #29).
pub fn words_or(reason: &str, fallback: &str) -> String {
    let words = reason_words(reason);
    if words == reason {
        t(fallback)
    } else {
        words
    }
}

/// Why a plug-in's state could not be had as it is now, in the interface's words — a reason
/// in none of the host's forms said as such, never as a failed load (review isolation-6
/// #10, #21, #31).
pub fn stale_words(reason: &str) -> String {
    words_or(reason, "isolation.why.no_state")
}

/// What a save says of the plug-ins it kept as they were last said (their state as it is
/// now not to be had in time), in the interface's words — `None` when there were none
/// (review isolation-5 rule 2: never silently).
pub fn saved_stale(stale: &[(String, String)]) -> Option<String> {
    match stale {
        [] => None,
        [(name, why)] => Some(tf(
            "isolation.save.stale_one",
            &[("name", name), ("reason", &stale_words(why))],
        )),
        many => {
            let names: Vec<&str> = many.iter().map(|(n, _)| n.as_str()).collect();
            Some(tf(
                "isolation.save.stale_many",
                &[("n", &many.len()), ("names", &names.join(", "))],
            ))
        }
    }
}

/// What a render's failed plug-ins say (review #6, #47; isolation-4 #10, #41), in the words
/// of `area` (`export`, `bake`, `freeze`; `job` names a freeze's or bounce's subject): the
/// losses — one that stopped part way, one never loaded for it — which fail the render, and
/// the warnings — one that began from the state it last saved — which do not.
pub fn render_words(
    lost: &[cantus_host::RenderFailure],
    area: &str,
    job: &str,
) -> (Option<String>, Option<String>) {
    use cantus_host::RenderLoss;
    let say = |kind: &str, f: &cantus_host::RenderFailure, reason: String| {
        tf(
            &format!("isolation.{area}.{kind}"),
            &[("name", &f.name), ("reason", &reason), ("job", &job)],
        )
    };
    let mut losses = Vec::new();
    let mut warnings = Vec::new();
    for f in lost {
        match f.kind {
            RenderLoss::Stopped => losses.push(say("stopped", f, reason_words(&f.reason))),
            RenderLoss::NeverLoaded => losses.push(say(
                "never",
                f,
                words_or(&f.reason, "isolation.why.not_loaded"),
            )),
            RenderLoss::Stale => warnings.push(say("stale", f, String::new())),
        }
    }
    let join = |v: Vec<String>| (!v.is_empty()).then(|| v.join(" "));
    (join(losses), join(warnings))
}

/// A plug-in's window could not be opened: its helper busy (its own dialog open, a slow
/// request ahead) is said as such — the window opens by itself once it is free — anything
/// else as the failure it was (review isolation-4 #39).
pub fn window_failed(app: &mut CantusApp, name: &str, e: &cantus_host::HostError) {
    use cantus_host::HostError;
    match e {
        HostError::Busy(_) => app.set_status(tf("isolation.busy", &[("name", &name)])),
        // It stopped as its window opened: said as a stop, in the interface's words
        // (review isolation-5 #49).
        HostError::Crashed(why) | HostError::Ended { why, .. } => app.set_error(tf(
            "isolation.window.stopped",
            &[("name", &name), ("reason", &stopped_words(why))],
        )),
        e => app.set_error(tf(
            "host.editor.window_failed",
            &[(
                "error",
                &words_or(&e.to_string(), "isolation.why.no_window"),
            )],
        )),
    }
}

/// The system's detail a reason ends with, in the interface's words, and the signal's
/// number: only a code — `(0x…)`, `(signal N)`, `(exit code …)`, `(exit status: N)` —
/// closing the text counts; a plug-in name's own parentheses ("Kontakt 7 (16 out)") never
/// do (review #26, #32).
fn system_detail(reason: &str) -> (Option<String>, Option<i32>) {
    let Some(inner) = reason
        .strip_suffix(')')
        .and_then(|r| r.rfind('(').map(|i| &r[i + 1..]))
    else {
        return (None, None);
    };
    let hex = |c: &str| {
        c.strip_prefix("0x")
            .is_some_and(|h| !h.is_empty() && h.chars().all(|ch| ch.is_ascii_hexdigit()))
    };
    if hex(inner) {
        return (Some(inner.to_string()), None);
    }
    if let Some(n) = inner
        .strip_prefix("signal ")
        .and_then(|n| n.trim().parse::<i32>().ok())
    {
        return (Some(tf("isolation.why.signal", &[("n", &n)])), Some(n));
    }
    for prefix in ["exit code ", "exit code: ", "exit status: "] {
        if let Some(c) = inner.strip_prefix(prefix) {
            if hex(c) {
                return (Some(c.to_string()), None);
            }
            if c.trim().parse::<i64>().is_ok() {
                return (Some(tf("isolation.why.exit", &[("code", &c.trim())])), None);
            }
        }
    }
    (None, None)
}

impl CantusApp {
    /// A plug-in that stopped working has no window: asked for one, the status line says
    /// why in the interface's words, and its card comes back with *Reload* one click away
    /// (review #31). `true` when it is stopped.
    pub fn window_of_stopped(&mut self, target: Target) -> bool {
        let Some(why) = stopped(self, target).map(str::to_string) else {
            return false;
        };
        let name = name_of(self, target);
        let name = if name.is_empty() {
            plugin_name(self, target)
        } else {
            name
        };
        self.set_error(tf(
            "isolation.window.stopped",
            &[("name", &name), ("reason", &stopped_words(&why))],
        ));
        add(self, target, name, why);
        true
    }
}

/// The plug-in's name as the project keeps it.
fn plugin_name(app: &CantusApp, target: Target) -> String {
    let p = app.project();
    match target {
        Target::Insert(e) => p
            .all_effects()
            .find(|(_, x)| x.id == e)
            .and_then(|(_, x)| x.spec.external.as_deref())
            .map(|x| x.name.clone()),
        Target::Instrument(tr) => p
            .track(tr)
            .and_then(|t| t.instrument.external())
            .map(|x| x.name.clone()),
    }
    .unwrap_or_default()
}
