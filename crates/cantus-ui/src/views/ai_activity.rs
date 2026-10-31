//! The AI activity (ADR 0081 §10): a floating panel opened from the top bar's agent chip
//! or Settings ▸ AI — the agents connected (each with *Undo all of its changes* and
//! *Disconnect*, and the tools it runs unasked), the calls waiting on the card, the jobs
//! and their progress, and every call and what became of it, refusals included (the audit
//! by origin). *Undo all* stops at the user's own step and says so.

use crate::app::CantusApp;
use crate::i18n::{t, tf};
use crate::kit::glyph::Glyph;
use crate::kit::text::{Role, Src};
use crate::kit::{with_brush, Fill, Style};
use crate::views::workspace_settings::{title, wrapped};
use crate::views::workspace_widgets as w;
use cantus_mcp::jobs::JobState;
use cantus_mcp::Outcome;
use egui::{Id, Pos2, Rect, Sense, Ui, Vec2};
use std::time::Instant;

/// The panel's own state.
#[derive(Default)]
pub struct ActivityView {
    /// What the last *Undo all* or *Disconnect* did.
    pub message: Option<(String, bool)>,
}

/// The panel's size at 100 %.
const SIZE: Vec2 = Vec2::new(460.0, 620.0);

/// Where the panel goes: the window's right side, under the top bar.
pub fn rect(ctx: &egui::Context, s: &Style) -> Rect {
    let screen = ctx.content_rect();
    let top = screen.top() + (crate::views::transport::HEIGHT * s.t.density).round() + 8.0;
    let size = (SIZE * s.k).min(Vec2::new(
        screen.width() - 32.0,
        (screen.bottom() - top - 40.0).max(200.0),
    ));
    Rect::from_min_size(Pos2::new(screen.right() - size.x - 12.0, top), size)
}

fn ago(at: Instant) -> String {
    let secs = Instant::now().saturating_duration_since(at).as_secs();
    if secs < 60 {
        tf("ai.activity.secs", &[("n", &secs)])
    } else if secs < 3600 {
        tf("ai.activity.mins", &[("n", &(secs / 60))])
    } else {
        tf("ai.activity.hours", &[("n", &(secs / 3600))])
    }
}

fn outcome_words(o: &Outcome) -> (String, bool) {
    match o {
        Outcome::Done => (t("ai.outcome.done"), false),
        // What the agent was told is in its own words (English, for the model); the user
        // reads the outcome in theirs.
        Outcome::Refused(_) => (t("ai.outcome.refused"), true),
        Outcome::Failed(_) => (t("ai.outcome.failed"), true),
        Outcome::Asked => (t("ai.outcome.asked"), false),
        Outcome::DeniedByUser => (t("ai.outcome.denied"), true),
        Outcome::NoAnswer => (t("ai.outcome.no_answer"), true),
        Outcome::JobStarted(_) => (t("ai.outcome.job"), false),
        Outcome::Dropped => (t("ai.outcome.dropped"), true),
    }
}

/// Draw the panel (called each frame while it is open).
pub fn show(app: &mut CantusApp, ctx: &egui::Context) {
    let s = Style::current();
    let k = s.k;
    let rect = rect(ctx, &s);
    let mut close = false;
    egui::Area::new(Id::new("cantus-ai-activity"))
        .order(egui::Order::Foreground)
        .fixed_pos(rect.min)
        .show(ctx, |ui| {
            ui.set_min_size(rect.size());
            crate::views::audio_parts::panel(ui, rect, &s);
            let head = Rect::from_min_size(rect.min, Vec2::new(rect.width(), 32.0 * k));
            if title(ui, head, &s, Glyph::Agent, "ai.activity.title") {
                close = true;
            }
            let body = Rect::from_min_max(
                Pos2::new(rect.left() + 14.0 * k, head.bottom() + 6.0 * k),
                Pos2::new(rect.right() - 14.0 * k, rect.bottom() - 10.0 * k),
            );
            let mut child = ui.new_child(egui::UiBuilder::new().max_rect(body));
            child.set_clip_rect(body);
            egui::ScrollArea::vertical()
                .id_salt("ai-activity-scroll")
                .auto_shrink([false, false])
                .max_height(body.height())
                .show(&mut child, |ui| {
                    ui.spacing_mut().item_spacing = Vec2::new(8.0, 6.0);
                    contents(app, ui, &s);
                });
        });
    if close || ctx.input(|i| i.key_pressed(egui::Key::Escape)) && !crate::kit::field::active(ctx) {
        app.ai.activity_open = false;
    }
}

fn line(ui: &mut Ui, text: &str, warn: bool) {
    let s = Style::current();
    let color = if warn { s.t.danger } else { s.t.text_dim };
    let width = ui.available_width();
    wrapped(ui, text, Role::Label, color, width);
}

fn contents(app: &mut CantusApp, ui: &mut Ui, s: &Style) {
    let agents = super::connected(app);
    w::band(ui, Src::Key("ai.activity.agents"));
    if agents.is_empty() {
        line(ui, &t("ai.activity.no_agent"), false);
    }
    for caller in &agents {
        let width = ui.available_width();
        let last = app
            .ai
            .agents()
            .and_then(|a| a.activity().rev().find(|x| x.caller.key == caller.key))
            .map(|x| ago(x.at))
            .unwrap_or_default();
        let head = tf(
            "ai.activity.agent",
            &[
                ("name", &caller.name),
                ("version", &caller.version),
                ("ago", &last),
            ],
        );
        wrapped(ui, &head, Role::Body, s.t.text, width);
        let grants = app
            .ai
            .agents()
            .map(|a| a.grants_of(&caller.key))
            .unwrap_or_default();
        if !grants.is_empty() {
            line(
                ui,
                &tf("ai.activity.unasked", &[("tools", &grants.join(", "))]),
                false,
            );
        }
        let (mut undo, mut cut) = (false, false);
        ui.horizontal(|ui| {
            undo = w::button(ui, Src::Key("ai.activity.undo_all"), 0.0, true).clicked();
            cut = w::button(ui, Src::Key("ai.activity.disconnect"), 0.0, true).clicked();
        });
        if undo {
            app.ai.activity.message = super::undo_all(app, &caller.key).map(|done| {
                let n = tf("ai.activity.undone", &[("n", &done.undone)]);
                match done.stopped_at {
                    Some(at) => (
                        format!("{n} {}", tf("ai.activity.stopped", &[("step", &at)])),
                        true,
                    ),
                    None => (n, false),
                }
            });
        }
        if cut {
            super::disconnect(app, &caller.key);
            app.ai.activity.message = Some((
                tf("ai.activity.disconnected", &[("name", &caller.name)]),
                false,
            ));
        }
    }
    if let Some((m, warn)) = app.ai.activity.message.clone() {
        line(ui, &m, warn);
    }
    let waiting = app.ai.agents().map(|a| a.asks().count()).unwrap_or(0);
    if waiting > 0 {
        w::band(ui, Src::Key("ai.activity.waiting"));
        let words = if waiting == 1 {
            t("ai.activity.waiting_one")
        } else {
            tf("ai.activity.waiting_n", &[("n", &waiting)])
        };
        line(ui, &words, false);
    }
    let Some(pump) = app.ai.agents() else {
        return;
    };
    let jobs: Vec<(String, String, f32)> = pump
        .jobs()
        .iter()
        .map(|j| {
            let state = match &j.state {
                JobState::Running if j.cancel_requested() => t("ai.job.cancelling"),
                JobState::Running => t("ai.job.running"),
                JobState::Done(_) => t("ai.job.done"),
                JobState::Failed(_) => t("ai.job.failed"),
                JobState::Cancelled => t("ai.job.cancelled"),
            };
            (
                format!("{} · {}", j.owner.name, j.said),
                state,
                j.progress(),
            )
        })
        .collect();
    if !jobs.is_empty() {
        w::band(ui, Src::Key("ai.activity.jobs"));
        for (what, state, p) in jobs {
            let width = ui.available_width();
            wrapped(
                ui,
                &format!("{what} — {state}"),
                Role::Label,
                s.t.text,
                width,
            );
            let (bar, _) = ui.allocate_exact_size(Vec2::new(width, 6.0 * s.k), Sense::hover());
            with_brush(ui.painter(), |b| {
                b.rrect(bar, 3.0 * s.k, Fill::Solid(s.t.well));
                let mut done = bar;
                done.set_width(bar.width() * p.clamp(0.0, 1.0));
                b.rrect(done, 3.0 * s.k, Fill::Solid(s.t.highlight));
            });
        }
    }
    w::band(ui, Src::Key("ai.activity.calls"));
    let calls: Vec<(String, bool)> = pump
        .activity()
        .rev()
        .take(200)
        .map(|a| {
            let (o, warn) = outcome_words(&a.outcome);
            (
                format!("{} · {} · {} — {o}", ago(a.at), a.caller.name, a.tool),
                warn,
            )
        })
        .collect();
    if calls.is_empty() {
        line(ui, &t("ai.activity.no_call"), false);
    }
    for (c, warn) in calls {
        line(ui, &c, warn);
    }
}
