//! The first-run setup (round 20): what a musician who has never opened Cantus sees
//! first, so that a guitar is recording through a new interface in under a minute,
//! without Settings.
//!
//! One page, four numbered steps, top to bottom: **choose** the interface from pictures
//! of what is connected (a click opens it, with the suggested driver and buffer);
//! **listen** to a test tone on its outputs; **play or speak** and watch its inputs'
//! meters — the one that moves is found and named; **latency**: what the setup chose and
//! what it gives, and the loopback measurement offered. *Record on input N* finishes by
//! adding an armed audio track on the input that moved.

use super::audio_parts::{self as parts};
use crate::app::CantusApp;
use crate::audio_setup::{self as setup, interfaces, latency, Hit, Measure};
use crate::i18n::{t, tf};
use crate::kit::color::{shade, with_alpha};
use crate::kit::glyph::{self, Glyph};
use crate::kit::meter::MIN_DB;
use crate::kit::text::{self, Elide, Policy, Role, Text};
use crate::kit::{band, with_brush, Fill, State, Style, Tok};
use egui::{Align2, Id, Pos2, Rect, Sense, Ui, Vec2};

/// The window's size at 100 %.
pub const SIZE: Vec2 = Vec2::new(780.0, 600.0);
/// An interface card.
const CARD: Vec2 = Vec2::new(172.0, 118.0);

/// The level an input must reach to count as "moving", in dB.
const HEARD_DB: f32 = -42.0;

/// The setup, when open.
pub fn show(app: &mut CantusApp, ctx: &egui::Context) {
    if app.audio_setup.first_run.is_none() {
        return;
    }
    let s = Style::current();
    let k = s.k;
    let screen = ctx.content_rect();
    let live = setup::live(app, true);
    poll_measure(app);
    // Held peaks fall 20 dB a second; the loudest input lately is the one being played.
    if let Some(fr) = app.audio_setup.first_run.as_mut() {
        fr.hold.resize(live.in_channels, MIN_DB);
        for (c, h) in fr.hold.iter_mut().enumerate() {
            let l = parts::db(live.peaks.get(c).copied().unwrap_or(0.0));
            *h = (*h - 20.0 / 60.0).max(l).max(MIN_DB);
        }
        if let Some((c, h)) = fr.hold.iter().enumerate().max_by(|a, b| a.1.total_cmp(b.1)) {
            if *h > HEARD_DB {
                fr.heard = Some(c);
            }
        }
    }
    if live.live {
        ctx.request_repaint();
    }
    // The room behind dims, and takes no clicks; the setup is moved above it each frame.
    egui::Area::new(Id::new("cantus-audio-first-run-veil"))
        .order(egui::Order::Foreground)
        .fixed_pos(screen.min)
        .show(ctx, |ui| {
            let (r, _) = ui.allocate_exact_size(screen.size(), Sense::click());
            with_brush(ui.painter(), |b| {
                b.rect(r, Fill::Solid(with_alpha(s.t.shadow, 0.55)));
            });
        });
    let size = (SIZE * k).min(screen.size() - Vec2::splat(24.0));
    let rect = Rect::from_center_size(screen.center(), size);
    egui::Area::new(Id::new("cantus-audio-first-run"))
        .order(egui::Order::Foreground)
        .fixed_pos(rect.min)
        .show(ctx, |ui| {
            ui.set_min_size(size);
            parts::panel(ui, rect, &s);
            let head = Rect::from_min_size(rect.min, Vec2::new(rect.width(), 34.0 * k));
            if super::workspace_settings::title(ui, head, &s, Glyph::Device, "devices.first.title")
            {
                setup::close_first_run(app);
                return;
            }
            let body = Rect::from_min_max(
                Pos2::new(rect.left() + 22.0 * k, head.bottom() + 10.0 * k),
                Pos2::new(rect.right() - 22.0 * k, rect.bottom() - 54.0 * k),
            );
            let mut child = ui.new_child(egui::UiBuilder::new().max_rect(body));
            // A short window (a laptop at 150 %) scrolls; the footer stays put.
            egui::ScrollArea::vertical()
                .id_salt("cantus-audio-first-run-body")
                .auto_shrink([false, false])
                .max_height(body.height())
                .show(&mut child, |ui| {
                    ui.set_width(body.width() - 12.0 * k);
                    ui.spacing_mut().item_spacing = Vec2::new(8.0 * k, 8.0 * k);
                    parts::paragraph(ui, &t("devices.first.intro"), Role::Body, s.t.text_dim);
                    step(ui, 1, "devices.first.choose");
                    cards(app, ui);
                    step(ui, 2, "devices.first.listen");
                    listen(app, ui);
                    step(ui, 3, "devices.first.play");
                    meters(app, ui, &live);
                    step(ui, 4, "devices.first.latency");
                    latency_step(app, ui, &live);
                });
            let foot = Rect::from_min_max(
                Pos2::new(rect.left() + 22.0 * k, rect.bottom() - 46.0 * k),
                Pos2::new(rect.right() - 22.0 * k, rect.bottom() - 14.0 * k),
            );
            footer(app, ui, foot);
        });
    // Always above its veil, whichever was clicked last.
    ctx.move_to_top(egui::LayerId::new(
        egui::Order::Foreground,
        Id::new("cantus-audio-first-run"),
    ));
    if ctx.input(|i| i.key_pressed(egui::Key::Escape)) && app.audio_setup.first_run.is_some() {
        setup::close_first_run(app);
    }
}

/// A step's band: its number in a lit disc, its title in the band's caps.
fn step(ui: &mut Ui, n: u32, key: &str) {
    let s = Style::current();
    let k = s.k;
    let (r, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 22.0 * k), Sense::hover());
    with_brush(ui.painter(), |b| {
        let title = Rect::from_min_max(Pos2::new(r.left() + 22.0 * k, r.top()), r.max);
        band::section(b, title, &s, State::REST, text::Src::Key(key), None, false);
        let c = Pos2::new(r.left() + 10.0 * k, r.center().y);
        b.circle(
            c,
            9.0 * k,
            Fill::V(shade(s.t.selection, 0.08), shade(s.t.selection, -0.08)),
        );
        let digit = n.to_string();
        let _ = text::paint(
            b,
            Rect::from_center_size(c, Vec2::splat(18.0 * k)),
            &Text::new(
                &digit,
                Role::Label,
                crate::kit::color::on_fill(s.t.selection),
            )
            .align(Align2::CENTER_CENTER),
            s.tk(),
        );
    });
}

/// One card per interface connected.
fn cards(app: &mut CantusApp, ui: &mut Ui) {
    let s = Style::current();
    let k = s.k;
    let list = app
        .audio_setup
        .scan
        .as_ref()
        .map(|s| s.interfaces.clone())
        .unwrap_or_default();
    if list.is_empty() {
        let key = if app.audio_setup.is_scanning() || app.audio_setup.scan.is_none() {
            "devices.scanning"
        } else {
            "devices.first.none"
        };
        super::workspace_settings::note(ui, &t(key), false);
        return;
    }
    let current = setup::current_key(app);
    let gap = 10.0 * k;
    let per_row = ((ui.available_width() + gap) / (CARD.x * k + gap))
        .floor()
        .max(1.0) as usize;
    let shown = list.len().min(per_row * 2);
    for row in 0..shown.div_ceil(per_row) {
        let (rr, _) =
            ui.allocate_exact_size(Vec2::new(ui.available_width(), CARD.y * k), Sense::hover());
        for c in 0..per_row {
            let i = row * per_row + c;
            let Some(iface) = list.get(i) else { break };
            let r = Rect::from_min_size(
                Pos2::new(rr.left() + c as f32 * (CARD.x * k + gap), rr.top()),
                CARD * k,
            );
            let resp = ui.interact(r, Id::new(("audio-card", i)), Sense::click());
            app.audio_setup.hit(ui.ctx(), Hit::Card(i), r);
            let lit = current.as_deref() == Some(iface.key.as_str());
            card(ui, r, iface, State::of(&resp, lit));
            if resp.clicked() && !lit {
                setup::choose_interface(app, &iface.key);
            }
        }
    }
}

fn card(ui: &Ui, r: Rect, iface: &interfaces::Interface, st: State) {
    let s = Style::current();
    let k = s.k;
    let kind = match iface.kind {
        interfaces::Kind::Interface => "devices.kind.interface",
        interfaces::Kind::BuiltIn => "devices.kind.builtin",
        interfaces::Kind::Headphones => "devices.kind.headphones",
        interfaces::Kind::Display => "devices.kind.display",
        interfaces::Kind::Virtual => "devices.kind.virtual",
    };
    let mut sub = t(kind);
    if iface.has_asio() && cantus_engine::devices::asio_built() {
        sub.push_str(" · ASIO");
    }
    with_brush(ui.painter(), |b| {
        b.register(
            r,
            "audio-card",
            &[
                Tok::PanelRaised,
                Tok::Well,
                Tok::Selection,
                Tok::Text,
                Tok::TextDim,
            ],
            &[],
        );
        let base = if st.lit {
            shade(s.t.panel_raised, 0.05)
        } else if st.hovered {
            shade(s.t.panel_raised, 0.03)
        } else {
            s.t.panel_raised
        };
        crate::kit::light::raised(
            b,
            r,
            s.r(6.0),
            base,
            crate::kit::light::RaisedOpts {
                k,
                depth: s.t.depth,
                drop: !st.lit,
                ..Default::default()
            },
        );
        let pic = Rect::from_min_size(
            r.min + Vec2::splat(6.0 * k),
            Vec2::new(r.width() - 12.0 * k, 66.0 * k),
        );
        b.rrect(pic, s.r(4.0), Fill::V(shade(s.t.well, 0.03), s.t.well));
        super::audio_art::paint(b, pic.shrink(7.0 * k), &s, iface.kind, st.lit);
        let name = Rect::from_min_max(
            Pos2::new(r.left() + 10.0 * k, pic.bottom() + 4.0 * k),
            Pos2::new(r.right() - 10.0 * k, pic.bottom() + 24.0 * k),
        );
        let _ = text::paint(
            b,
            name,
            &Text::new(&iface.key, Role::Body, s.t.text)
                .align(Align2::LEFT_CENTER)
                .policy(Policy::Elide(Elide::Middle)),
            s.tk(),
        );
        let line = Rect::from_min_max(
            Pos2::new(r.left() + 10.0 * k, name.bottom()),
            Pos2::new(r.right() - 10.0 * k, r.bottom() - 6.0 * k),
        );
        let _ = text::paint(
            b,
            line,
            &Text::new(&sub, Role::Label, s.t.text_dim)
                .align(Align2::LEFT_CENTER)
                .policy(Policy::Elide(Elide::End)),
            s.tk(),
        );
        if st.lit {
            b.rrect_stroke(r, s.r(6.0), 2.0 * k, s.t.selection);
            let c = Pos2::new(r.right() - 14.0 * k, r.top() + 14.0 * k);
            b.circle(c, 8.0 * k, Fill::Solid(s.t.selection));
            glyph::paint(
                b,
                Rect::from_center_size(c, Vec2::splat(10.0 * k)),
                Glyph::Check,
                crate::kit::color::on_fill(s.t.selection),
            );
        }
    });
}

/// The test tone, and what to expect.
fn listen(app: &mut CantusApp, ui: &mut Ui) {
    let s = Style::current();
    let on = app.audio_setup.tone.is_some();
    ui.horizontal(|ui| {
        let label = if on {
            t("devices.tone.stop")
        } else {
            t("devices.tone.play")
        };
        let r = parts::button(ui, &label, on, true, 140.0);
        app.audio_setup.hit(ui.ctx(), Hit::FirstTone, r.rect);
        if r.clicked() {
            setup::set_tone(app, if on { None } else { Some(0) });
            if let Some(fr) = app.audio_setup.first_run.as_mut() {
                fr.tone = !on;
            }
        }
        let hint = if on {
            t("devices.first.tone_on")
        } else {
            t("devices.first.tone_hint")
        };
        parts::paragraph(ui, &hint, Role::Body, s.t.text_dim);
    });
}

/// A meter per input, its number and name; the one that moved is said aloud.
fn meters(app: &mut CantusApp, ui: &mut Ui, live: &setup::Live) {
    let s = Style::current();
    let k = s.k;
    let Some(fr) = app.audio_setup.first_run.clone() else {
        return;
    };
    if live.in_channels == 0 {
        super::workspace_settings::note(ui, &t("devices.first.no_inputs"), true);
        return;
    }
    let n = live.in_channels.min(8);
    let cols = if n > 4 { 2 } else { 1 };
    let per_col = n.div_ceil(cols);
    let wdt = ui.available_width();
    let col_w = (wdt - 16.0 * k * (cols as f32 - 1.0)) / cols as f32;
    let row_h = 18.0 * k;
    let (rect, _) = ui.allocate_exact_size(
        Vec2::new(wdt, per_col as f32 * (row_h + 4.0 * k)),
        Sense::hover(),
    );
    with_brush(ui.painter(), |b| {
        for c in 0..n {
            let (col, row) = (c / per_col, c % per_col);
            let x = rect.left() + col as f32 * (col_w + 16.0 * k);
            let y = rect.top() + row as f32 * (row_h + 4.0 * k);
            let label = Rect::from_min_size(Pos2::new(x, y), Vec2::new(64.0 * k, row_h));
            let mut name = String::new();
            if !crate::audio_setup::names::write_input(c, false, &mut name) {
                name = tf("devices.first.input_n", &[("n", &(c + 1))]);
            }
            let heard = fr.heard == Some(c);
            let ink = if heard { s.t.text } else { s.t.text_dim };
            let _ = text::paint(
                b,
                label,
                &Text::new(&name, Role::Label, ink)
                    .align(Align2::LEFT_CENTER)
                    .policy(Policy::Elide(Elide::End)),
                s.tk(),
            );
            let bar = Rect::from_min_max(
                Pos2::new(label.right() + 6.0 * k, y + 3.0 * k),
                Pos2::new(x + col_w, y + row_h - 3.0 * k),
            );
            let lvl = parts::db(live.peaks.get(c).copied().unwrap_or(0.0));
            parts::level_bar(b, bar, &s, lvl, fr.hold.get(c).copied().unwrap_or(MIN_DB));
        }
    });
    let line = match fr.heard {
        Some(c) => {
            let mut name = String::new();
            if !crate::audio_setup::names::write_input(c, false, &mut name) {
                name = tf("devices.first.input_n", &[("n", &(c + 1))]);
            }
            tf("devices.first.heard", &[("name", &name)])
        }
        None => t("devices.first.play_hint"),
    };
    super::workspace_settings::note(ui, &line, false);
}

/// What was chosen and what it gives; the loopback measurement offered.
fn latency_step(app: &mut CantusApp, ui: &mut Ui, live: &setup::Live) {
    let s = Style::current();
    let rate = if live.rate > 0.0 { live.rate } else { 48_000.0 };
    let block = app
        .settings
        .audio
        .buffer_frames
        .unwrap_or(live.block)
        .max(1);
    let mut ms = String::new();
    latency::fmt_ms(setup::estimate_ms(live, block), &mut ms);
    let feel = match latency::feel(setup::estimate_ms(live, block)) {
        latency::Feel::Tight => t("devices.feel.tight"),
        latency::Feel::Fine => t("devices.feel.fine"),
        latency::Feel::Loose => t("devices.feel.loose"),
    };
    let line = if live.live {
        tf(
            "devices.first.chosen",
            &[
                ("driver", &live.driver),
                ("buffer", &block),
                ("rate", &latency::short_rate(rate as u32)),
                ("ms", &ms),
                ("feel", &feel),
            ],
        )
    } else {
        t("devices.chip.none")
    };
    parts::paragraph(ui, &line, Role::Body, s.t.text);
    // No ASIO driver yet (or this build cannot use one): the way to lower latency.
    let no_asio = app
        .audio_setup
        .scan
        .as_ref()
        .is_some_and(|sc| sc.asio.is_empty())
        || !cantus_engine::devices::asio_built();
    if no_asio || !live.driver.eq_ignore_ascii_case("ASIO") {
        ui.horizontal(|ui| {
            let r = parts::button(ui, &t("devices.drivers.get"), false, true, 0.0);
            app.audio_setup.hit(ui.ctx(), Hit::GetDriver, r.rect);
            if r.clicked() {
                app.audio_setup.drivers = true;
                app.audio_setup.request_scan();
            }
            let hint = if no_asio {
                t("devices.drivers.first_hint")
            } else {
                t("devices.drivers.first_hint_some")
            };
            parts::paragraph(ui, &hint, Role::Label, s.t.text_dim);
        });
    }
    let measure = app
        .audio_setup
        .first_run
        .as_ref()
        .map(|f| f.measure.clone())
        .unwrap_or_default();
    ui.horizontal(|ui| {
        let running = measure == Measure::Running;
        let label = if running {
            t("devices.measuring")
        } else {
            t("devices.first.measure")
        };
        let r = parts::button(ui, &label, false, live.live && !running, 150.0);
        app.audio_setup.hit(ui.ctx(), Hit::FirstMeasure, r.rect);
        if r.clicked() {
            start_measure(app);
        }
        let (text, warn) = match &measure {
            Measure::Done(r) => (
                tf(
                    "devices.first.measured",
                    &[
                        ("offset", &format!("{:+}", r.offset)),
                        ("ms", &format!("{:.1}", r.round_trip as f64 / rate * 1000.0)),
                    ],
                ),
                false,
            ),
            Measure::Failed(e) => (t(e.key()), true),
            _ => (t("devices.first.measure_hint"), false),
        };
        let ink = if warn { s.t.danger } else { s.t.text_dim };
        parts::paragraph(ui, &text, Role::Label, ink);
    });
}

fn start_measure(app: &mut CantusApp) {
    if app.engine.is_playing() {
        app.stop();
    }
    setup::set_tone(app, None);
    let m = match app.engine.begin_loopback() {
        Ok(()) => Measure::Running,
        Err(e) => Measure::Failed(e),
    };
    if let Some(fr) = app.audio_setup.first_run.as_mut() {
        fr.measure = m;
        fr.tone = false;
    }
}

fn poll_measure(app: &mut CantusApp) {
    if app.audio_setup.first_run.as_ref().map(|f| &f.measure) != Some(&Measure::Running) {
        return;
    }
    let m = match app.engine.poll_loopback() {
        Some(Ok(r)) => {
            app.settings.audio.record_offset_frames = r.offset;
            app.engine.set_record_offset(r.offset);
            if let Some(k) = setup::current_key(app) {
                let a = app.settings.audio.clone();
                app.settings.audio_setup.remember(&k, &a);
            }
            app.store_settings();
            Measure::Done(r)
        }
        Some(Err(e)) => Measure::Failed(e),
        None if !app.engine.is_measuring() => Measure::Idle,
        None => return,
    };
    if let Some(fr) = app.audio_setup.first_run.as_mut() {
        fr.measure = m;
    }
}

/// Skip, or finish — with a track to record on when an input moved.
fn footer(app: &mut CantusApp, ui: &mut Ui, r: Rect) {
    let s = Style::current();
    let k = s.k;
    with_brush(ui.painter(), |b| {
        b.hline(
            r.left(),
            r.right(),
            r.top() - 6.0 * k,
            b.px(),
            shade(s.t.panel, -0.14),
        );
    });
    let mut child = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(r)
            .layout(egui::Layout::left_to_right(egui::Align::Center)),
    );
    let skip = parts::button(&mut child, &t("devices.first.skip"), false, true, 90.0);
    app.audio_setup.hit(child.ctx(), Hit::FirstSkip, skip.rect);
    if skip.clicked() {
        setup::close_first_run(app);
        return;
    }
    let heard = app.audio_setup.first_run.as_ref().and_then(|f| f.heard);
    let mut right = child.new_child(
        egui::UiBuilder::new()
            .max_rect(r)
            .layout(egui::Layout::right_to_left(egui::Align::Center)),
    );
    let label = match heard {
        Some(c) => {
            let mut name = String::new();
            if !crate::audio_setup::names::write_input(c, false, &mut name) {
                name = tf("devices.first.input_n", &[("n", &(c + 1))]);
            }
            tf("devices.first.record_on", &[("name", &name)])
        }
        None => t("devices.first.done"),
    };
    let done = parts::button(&mut right, &label, true, true, 140.0);
    app.audio_setup.hit(right.ctx(), Hit::FirstDone, done.rect);
    if done.clicked() {
        setup::finish_first_run(app, heard);
    }
}
