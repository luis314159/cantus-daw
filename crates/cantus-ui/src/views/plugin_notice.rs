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
            as_sentence(&reason_words(&first.reason))
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
                &[("name", &c.name), ("reason", &reason_words(&c.reason))],
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
            // In the interface's words (review #31); and when it crashed again, its card
            // comes back — counted, so one that keeps crashing is offered its own process
            // (review #15).
            app.set_error(tf(
                "isolation.reload_failed",
                &[("name", &name), ("error", &reason_words(&error))],
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
    let crashed = match target {
        Target::Insert(e) => app.plugins.crashed(e).is_some(),
        Target::Instrument(tr) => app.plugins.instrument_crashed(tr).is_some(),
    };
    if crashed {
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
            &[("reason", &reason_words(why))],
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
    } else if reason.contains("did not answer in") {
        "isolation.why.timeout"
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
            &[("name", &name), ("reason", &reason_words(&why))],
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
