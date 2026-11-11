//! The recording mode on screen (round 36, ADR 0092): the bar across the window that cannot
//! be missed while the mode is on — its name, the buffer and the round trip, the CPU
//! before and after, how many strips are baked, **Leave** — the question before it starts
//! (what will be rendered, how big, what stays live), the question when leaving (keep or
//! discard the renders), and the rows of the record popover.

use super::interop::form;
use crate::app::CantusApp;
use crate::i18n::{t, tf};
use crate::kit::color::{shade, with_alpha};
use crate::kit::text::{self, Elide, Policy, Role, Src, Text};
use crate::kit::{with_brush, Fill, Style};
use crate::record_mode::{self as rm, Phase};
use cantus_core::freeze::FreezeTarget;
use cantus_core::record_mode::Why;
use egui::{pos2, vec2, Align2, Id, Rect, Ui};

/// The bar's height at 100 %.
pub const HEIGHT: f32 = 30.0;

/// Percent as the chip prints it.
fn pct(load: f32) -> String {
    format!("{:.0} %", (load * 100.0).clamp(0.0, 999.0))
}

/// "64 (1.3 ms)": the buffer the engine runs with.
pub fn buffer_text(app: &CantusApp) -> String {
    let st = app.engine.stats();
    if st.block_frames == 0 || st.sample_rate <= 0.0 {
        return t("record.bar.unknown");
    }
    let ms = st.block_frames as f64 * 1000.0 / st.sample_rate;
    tf(
        "record.bar.buffer_value",
        &[("frames", &st.block_frames), ("ms", &format!("{ms:.1}"))],
    )
}

/// The round trip, labelled for what it is: measured (a ping out and back) or what the
/// driver reports (`record_mode::round_trip`, the seam for a measured figure).
pub fn round_trip_text(app: &CantusApp) -> String {
    let rate = app.engine.sample_rate;
    let Some((frames, measured)) = rm::round_trip(app).filter(|_| rate > 0.0) else {
        return t("record.bar.trip_unknown");
    };
    let ms = format!("{:.1}", frames as f64 * 1000.0 / rate);
    tf(
        if measured {
            "record.bar.trip_measured"
        } else {
            "record.bar.trip_reported"
        },
        &[("ms", &ms)],
    )
}

/// "31 % → 6 %": the load before and since, whichever is known.
pub fn cpu_text(app: &CantusApp) -> String {
    let s = &app.freeze.record;
    match (s.cpu_before(), s.cpu_now()) {
        (Some(a), Some(b)) => tf(
            "record.bar.cpu_both",
            &[("before", &pct(a)), ("now", &pct(b))],
        ),
        (Some(a), None) => tf("record.bar.cpu_before", &[("before", &pct(a))]),
        (None, Some(b)) => tf("record.bar.cpu_now", &[("now", &pct(b))]),
        (None, None) => t("record.bar.cpu_unknown"),
    }
}

/// The words after the chip, as the bar reads them.
pub fn bar_text(app: &CantusApp) -> String {
    let s = &app.freeze.record;
    match &s.phase {
        Phase::Idle => String::new(),
        Phase::Preparing(p) => {
            let done = p
                .total
                .saturating_sub(app.freeze.queued() + usize::from(app.freeze.job.is_some()));
            let now = app
                .freeze
                .progress()
                .map(|(name, f)| tf("record.bar.rendering", &[("name", &name), ("pct", &pct(f))]))
                .unwrap_or_default();
            tf(
                "record.bar.preparing",
                &[("done", &done), ("total", &p.total), ("now", &now)],
            )
        }
        Phase::Active(a) => {
            let live = if a.live.is_empty() {
                String::new()
            } else {
                crate::i18n::tf_count("record.bar.live", a.live.len(), &[])
            };
            let mut text = tf(
                "record.bar.text",
                &[
                    ("baked", &a.baked.len()),
                    ("buffer", &buffer_text(app)),
                    ("trip", &round_trip_text(app)),
                    ("cpu", &cpu_text(app)),
                    ("live", &live),
                ],
            );
            if a.master.is_some() {
                text = format!("{} {text}", t("record.bar.master_baked"));
            }
            // What low-latency monitoring leaves out for the armed strips, by name.
            let off = super::freeze_monitor::bypassed(app);
            if !off.is_empty() {
                text.push(' ');
                text.push_str(&tf("record.bar.without", &[("names", &off)]));
            }
            text
        }
    }
}

/// The bar, under the toolbar, while the mode is on or preparing.
pub fn banner(app: &mut CantusApp, root: &mut Ui) {
    if !app.freeze.record.on() {
        return;
    }
    let s = Style::current();
    let h = (HEIGHT * s.t.density).round();
    egui::Panel::top(Id::new("record-bar"))
        .frame(egui::Frame::NONE)
        .exact_size(h)
        .resizable(false)
        .show(root, |ui| paint(app, ui, &s));
}

fn paint(app: &mut CantusApp, ui: &mut Ui, s: &Style) {
    let rect = ui.max_rect();
    let k = s.k;
    let tk = s.tk();
    let preparing = app.freeze.record.is_preparing();
    let text_words = bar_text(app);
    let chip_words = t(if preparing {
        "record.bar.chip_preparing"
    } else {
        "record.bar.chip"
    });
    let pulse = ui.ctx().input(|i| i.time) as f32;
    let danger = s.t.danger;
    // The button first (the right edge), so the words know their room.
    let button_w = 110.0 * k;
    let btn = Rect::from_min_max(
        pos2(rect.right() - button_w - 8.0 * k, rect.top() + 2.0 * k),
        pos2(rect.right() - 8.0 * k, rect.bottom() - 2.0 * k),
    );
    let mut pressed = false;
    ui.scope_builder(
        egui::UiBuilder::new()
            .max_rect(btn)
            .layout(egui::Layout::right_to_left(egui::Align::Center)),
        |ui| {
            let label = t(if preparing {
                "record.bar.cancel"
            } else {
                "record.bar.leave"
            });
            pressed = form::button(ui, &label, true, true);
        },
    );
    with_brush(ui.painter(), |b| {
        let r = b.snap_rect(rect);
        b.rect(
            r,
            Fill::V(
                shade(with_alpha(danger, 0.28), 0.02),
                shade(with_alpha(danger, 0.18), -0.02),
            ),
        );
        b.hline(
            r.left(),
            r.right(),
            r.bottom() - b.px() * 0.5,
            b.px() * 2.0,
            danger,
        );
        // The lamp.
        let c = pos2(rect.left() + 14.0 * k, rect.center().y);
        let a = 0.6 + 0.4 * (pulse * 3.0).sin().abs();
        b.circle(c, 5.0 * k, Fill::Solid(with_alpha(danger, a)));
        // The chip: the mode's name, outlined in the record colour.
        let chip_t = Text::new(&chip_words, Role::Control, danger);
        let cw = text::measure(b.ctx(), &chip_t, tk).x + 16.0 * k;
        let chip = Rect::from_min_size(
            pos2(rect.left() + 28.0 * k, rect.top() + 5.0 * k),
            vec2(cw, rect.height() - 10.0 * k),
        );
        b.rrect(chip, s.r(3.0), Fill::Solid(with_alpha(danger, 0.12)));
        b.rrect_stroke(chip, s.r(3.0), 1.0, danger);
        let _ = text::paint(b, chip, &chip_t.align(Align2::CENTER_CENTER), tk);
        // What it says.
        let words = Rect::from_min_max(
            pos2(chip.right() + 12.0 * k, rect.top()),
            pos2(btn.left() - 10.0 * k, rect.bottom()),
        );
        let tt = Text::new(&text_words, Role::Label, s.t.text)
            .align(Align2::LEFT_CENTER)
            .policy(Policy::Elide(Elide::End));
        let _ = text::paint(b, words, &tt, tk);
    });
    let resp = ui.interact(rect, Id::new("record-bar-tip"), egui::Sense::hover());
    if resp.hovered() {
        let _ = resp.on_hover_text(t("record.bar.tip"));
    }
    if pressed {
        if preparing {
            app.record_mode_cancel();
        } else {
            app.freeze.record.leaving = true;
        }
    }
}

/// The questions: before it starts, when leaving, and the offer to bring a strip live.
pub fn frame(app: &mut CantusApp, ctx: &egui::Context) {
    ask(app, ctx);
    leave(app, ctx);
    offer(app, ctx);
}

/// A paragraph in the kit's prose (ADR 0024 §3): broken at the dialog's width, measured
/// before it is painted.
fn para(ui: &mut Ui, text: &str) {
    let s = Style::current();
    let w = ui.available_width().max(s.s(300.0));
    let lines = crate::kit::prose::wrap(ui.ctx(), text, Role::Label, s.tk(), w);
    let (r, _) = ui.allocate_exact_size(vec2(w, lines.height()), egui::Sense::hover());
    let dim = s.t.text_dim;
    with_brush(ui.painter(), |b| {
        let _ = crate::kit::prose::paint(b, r, &lines, Role::Label, s.tk(), dim, Align2::LEFT_TOP);
    });
}

fn why_text(app: &CantusApp, target: FreezeTarget, why: Why) -> String {
    let name = crate::freeze::name_of(app.project(), target);
    let key = match why {
        Why::Armed(_) => "record.live.armed",
        Why::Monitoring(_) => "record.live.monitoring",
        Why::Soloed => "record.live.soloed",
        Why::Dancer => "record.live.dancer",
        Why::External(_) => "record.live.external",
        Why::CueSolo(_) => "record.live.cue_solo",
        // r37:drums — split, or only chains on its pads.
        Why::SplitKit(t) if cantus_core::padout::is_split(app.project(), t) => "padout.record_live",
        Why::SplitKit(_) => "padout.record_live_chains",
    };
    tf(key, &[("name", &name)])
}

fn ask(app: &mut CantusApp, ctx: &egui::Context) {
    let Some((plan, _)) = app.freeze.record.ask.clone() else {
        return;
    };
    let mut opts = app.freeze.record.options;
    // The estimate follows the choices (a baked master is one render more).
    let est = rm::estimate(app, &plan, opts);
    let mut go = false;
    let mut cancel = false;
    crate::kit::dialog::window(t("record.ask.title")) // r39:el-controls
        .id(Id::new("cantus-record-ask"))
        .collapsible(false)
        .resizable(false)
        .anchor(Align2::CENTER_CENTER, vec2(0.0, 0.0))
        .show(ctx, |ui| {
            if crate::kit::dialog::title(ui, &t("record.ask.title")) {
                cancel = true; // r39:el-controls
            }
            para(ui, &t("record.ask.what"));
            para(
                ui,
                &tf(
                    "record.ask.plan",
                    &[
                        ("n", &est.targets),
                        ("reused", &est.reused),
                        ("seconds", &format!("{:.0}", est.seconds)),
                    ],
                ),
            );
            para(
                ui,
                &tf(
                    "record.ask.size",
                    &[
                        ("new", &rm::size_text(est.bytes_new)),
                        ("all", &rm::size_text(est.bytes_all)),
                    ],
                ),
            );
            for (target, why) in &plan.live {
                para(ui, &why_text(app, *target, *why));
            }
            form::band(ui, "record.ask.master_band");
            if form::choice(ui, opts.master_live, true, &t("record.ask.master_live")) {
                opts.master_live = true;
            }
            if form::choice(ui, !opts.master_live, true, &t("record.ask.master_baked")) {
                opts.master_live = false;
            }
            form::band(ui, "record.ask.mix_band");
            // A baked master holds the whole mix: its controls are locked with it.
            let free = opts.master_live;
            if form::choice(ui, opts.mix_live && free, free, &t("record.ask.mix_live")) {
                opts.mix_live = true;
            }
            if form::choice(
                ui,
                !opts.mix_live || !free,
                free,
                &t("record.ask.mix_locked"),
            ) {
                opts.mix_live = false;
            }
            if est.locked_mix > 0 && free {
                para(
                    ui,
                    &crate::i18n::tf_count("record.ask.locked_mix", est.locked_mix, &[]),
                );
            }
            form::band(ui, "record.ask.fail_band");
            if form::choice(ui, !opts.stop_on_failure, true, &t("record.ask.fail_live")) {
                opts.stop_on_failure = false;
            }
            if form::choice(ui, opts.stop_on_failure, true, &t("record.ask.fail_stop")) {
                opts.stop_on_failure = true;
            }
            para(ui, &t("record.ask.honest"));
            form::buttons(ui, |ui| {
                if form::button(ui, &t("record.ask.prepare"), true, true) {
                    go = true;
                }
                if form::button(ui, &t("record.ask.cancel"), false, true) {
                    cancel = true;
                }
            });
            if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                cancel = true;
            }
        });
    app.freeze.record.options = opts;
    if go {
        if let Err(e) = app.record_mode_begin(opts) {
            app.set_error(e);
            app.freeze.record.ask = None;
        }
    } else if cancel {
        app.freeze.record.ask = None;
    }
}

fn leave(app: &mut CantusApp, ctx: &egui::Context) {
    if !app.freeze.record.leaving || !app.freeze.record.is_active() {
        app.freeze.record.leaving = false;
        return;
    }
    let mut answer: Option<Option<bool>> = None;
    crate::kit::dialog::window(t("record.leave.title")) // r39:el-controls
        .id(Id::new("cantus-record-leave"))
        .collapsible(false)
        .resizable(false)
        .anchor(Align2::CENTER_CENTER, vec2(0.0, 0.0))
        .show(ctx, |ui| {
            if crate::kit::dialog::title(ui, &t("record.leave.title")) {
                answer = Some(None); // r39:el-controls
            }
            para(ui, &t("record.leave.text"));
            form::buttons(ui, |ui| {
                if form::button(ui, &t("record.leave.keep"), true, true) {
                    answer = Some(Some(false));
                }
                if form::button(ui, &t("record.leave.discard"), false, true) {
                    answer = Some(Some(true));
                }
                if form::button(ui, &t("record.leave.stay"), false, true) {
                    answer = Some(None);
                }
            });
            if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                answer = Some(None);
            }
        });
    match answer {
        Some(Some(discard)) => {
            if let Err(e) = app.record_mode_leave(discard) {
                app.set_error(e);
            }
        }
        Some(None) => app.freeze.record.leaving = false,
        None => {}
    }
}

/// Arming a track whose strip is baked: bring that strip live (no render made again) and
/// arm it, or not.
fn offer(app: &mut CantusApp, ctx: &egui::Context) {
    let Some(track) = app.freeze.record.offer else {
        return;
    };
    if !app.freeze.record.is_active() {
        app.freeze.record.offer = None;
        return;
    }
    let name = app
        .project()
        .track(track)
        .map(|t| t.name.clone())
        .unwrap_or_default();
    let holding: Vec<String> = app
        .baked_holding(track)
        .into_iter()
        .map(|t| crate::freeze::name_of(app.project(), t))
        .collect();
    let mut answer = None;
    crate::kit::dialog::window(t("record.offer.title")) // r39:el-controls
        .id(Id::new("cantus-record-offer"))
        .collapsible(false)
        .resizable(false)
        .anchor(Align2::CENTER_CENTER, vec2(0.0, 0.0))
        .show(ctx, |ui| {
            if crate::kit::dialog::title(ui, &t("record.offer.title")) {
                answer = Some(false); // r39:el-controls
            }
            para(
                ui,
                &tf(
                    "record.offer.text",
                    &[("name", &name), ("strips", &holding.join(", "))],
                ),
            );
            if app.freeze.record.master_baked() {
                para(ui, &t("record.offer.master"));
            }
            form::buttons(ui, |ui| {
                if form::button(ui, &t("record.offer.yes"), true, true) {
                    answer = Some(true);
                }
                if form::button(ui, &t("record.offer.no"), false, true) {
                    answer = Some(false);
                }
            });
            if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                answer = Some(false);
            }
        });
    match answer {
        Some(true) => {
            if let Err(e) = app.record_mode_bring_live(track) {
                app.set_error(e);
            }
        }
        Some(false) => app.freeze.record.offer = None,
        None => {}
    }
}

/// The record popover's rows: the mode's button, and what it will do.
pub fn popover_rows(app: &mut CantusApp, ui: &mut Ui, control_w: f32) {
    let s = Style::current();
    super::record_options::band_row(ui, "record.row.band", &s);
    ui.horizontal(|ui| {
        super::record_options::label(ui, "record.row.mode", &s);
        let on = app.freeze.record.on();
        let mut lit = on;
        let name = t("record.row.tip");
        let label = if on {
            Src::Key("record.row.leave")
        } else {
            Src::Key("record.row.prepare")
        };
        if crate::kit::widgets::led_button(ui, &mut lit, label, &name, control_w.min(150.0))
            .changed()
        {
            if on {
                app.freeze.record.leaving = app.freeze.record.is_active();
                if app.freeze.record.is_preparing() {
                    app.record_mode_cancel();
                }
            } else {
                app.record_mode_ask();
            }
        }
    });
}
