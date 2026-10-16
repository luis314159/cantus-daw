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
use cantus_host::HostNotice;

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

/// The card's title, body, button labels (with what each does), and the reasons (its
/// hint), or `None` when nothing waits.
pub fn card(app: &CantusApp) -> Option<(String, String, Vec<(String, Answer)>, String)> {
    let w = &app.plugin_crashes.waiting;
    let first = w.first()?;
    let one = w.len() == 1;
    let title = if one {
        tf("isolation.crashed.one", &[("name", &first.name)])
    } else {
        tf("isolation.crashed.many", &[("n", &w.len())])
    };
    let body = t(if one {
        "isolation.crashed.body_one"
    } else {
        "isolation.crashed.body_many"
    });
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
    if one && offers_individually(app, first.target) {
        buttons.push((t("isolation.run_individually"), Answer::RunIndividually));
    }
    let reasons = w
        .iter()
        .map(|c| tf("isolation.reason", &[("name", &c.name), ("reason", &c.reason)]))
        .collect::<Vec<_>>()
        .join("\n");
    Some((title, body, buttons, reasons))
}

/// A plug-in that crashed `OFFER_INDIVIDUALLY` times this session and does not run alone.
fn offers_individually(app: &CantusApp, target: Target) -> bool {
    let (info, runs) = match target {
        Target::Insert(e) => match app.plugins.hosted(e) {
            Some(h) => (h.info.clone(), h.isolation()),
            None => return false,
        },
        Target::Instrument(tr) => match app.plugins.hosted_instrument(tr) {
            Some(h) => (h.info.clone(), h.isolation()),
            None => return false,
        },
    };
    runs != PluginIsolation::Individually
        && app.plugins.crashes_this_session(info.format, &info.id) >= OFFER_INDIVIDUALLY
}

/// The user answered the card.
pub fn answer(app: &mut CantusApp, a: Answer) {
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
                set_isolation(app, c.target, Some(PluginIsolation::Individually));
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
        Err(error) => app.set_error(tf(
            "isolation.reload_failed",
            &[("name", &name), ("error", &error)],
        )),
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
        rows.push((t(m.label_key()), Some(chosen == Some(m)), MenuAct::Run(Some(m))));
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
        Target::Instrument(tr) => app.plugins.hosted_instrument(tr).map(|h| h.info.name.clone()),
    }
    .unwrap_or_default()
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
    let (runs, cpu, crashed) = match target {
        Target::Insert(e) => match app.plugins.hosted(e) {
            Some(h) => (h.isolation(), h.cpu(), app.plugins.crashed(e)),
            None => return String::new(),
        },
        Target::Instrument(tr) => match app.plugins.hosted_instrument(tr) {
            Some(h) => (h.isolation(), h.cpu(), app.plugins.instrument_crashed(tr)),
            None => return String::new(),
        },
    };
    let mut lines = vec![tf("isolation.tip.runs", &[("mode", &t(runs.label_key()))])];
    if let Some(why) = crashed {
        lines.push(tf("isolation.tip.stopped", &[("reason", &why)]));
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
        crate::kit::glyph::paint(
            b,
            icon,
            crate::kit::glyph::Glyph::PluginStopped,
            s.t.danger,
        );
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
