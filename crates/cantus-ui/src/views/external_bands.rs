//! Settings ▸ MIDI, the external gear (round 36, ADR 0094 and 0095): the project's external
//! units — each with its routing, its state, its measured latency and a *Ping* — and the
//! studio's control surfaces — each with its kind and ports, a log of what it said, and a
//! virtual unit to try the setup with nothing plugged in.
//!
//! Every field is the kit's own (a stepper field, a key, a band); a unit's change is one
//! command (`SetExternalUnit`), a ping's result one more (`SetExternalLatency`).

use super::{note, LABEL_W};
use crate::app::CantusApp;
use crate::external_service::{self as svc, Outcome};
use crate::i18n::{t, tf};
use crate::kit::text::Src;
use crate::kit::widgets::{led_button, menu_field};
use crate::surface::SurfaceChoice;
use crate::views::workspace_widgets as w;
use cantus_core::external_unit::{Availability, ExternalUnit, MidiRoute, Pair, UnitKind};
use cantus_core::Command;
use egui::Ui;

pub fn page(app: &mut CantusApp, ui: &mut Ui) {
    units_band(app, ui);
    surfaces_band(app, ui);
}

/// A stepper field: its words, and which way it was clicked (−1, 0, 1).
fn stepper(ui: &mut Ui, value: &str, name: &str, width: f32) -> i32 {
    let (_, c) = menu_field(ui, value, true, false, name, width);
    i32::from(c.next) - i32::from(c.prev)
}

/// The word of a pair: `1–2` (stereo) or `3` (mono), channels from 1.
pub fn pair_text(p: Pair) -> String {
    if p.stereo {
        format!("{}-{}", p.first + 1, p.first + 2)
    } else {
        format!("{}", p.first + 1)
    }
}

/// A pair stepped by one stereo pair (two channels), within `0..max`.
fn step_pair(p: Pair, by: i32, max: u16) -> Pair {
    let w = i32::from(p.width());
    let last = (i32::from(max) - w).max(0);
    Pair {
        first: (i32::from(p.first) + by * w).clamp(0, last) as u16,
        stereo: p.stereo,
    }
}

fn units_band(app: &mut CantusApp, ui: &mut Ui) {
    w::band(ui, Src::Key("external.band.units"));
    let sel = app.selected_track;
    ui.horizontal(|ui| {
        w::row_label(ui, "", LABEL_W, 24.0);
        for (kind, key) in [
            (UnitKind::Instrument, "external.add.instrument"),
            (UnitKind::Effect, "external.add.effect"),
        ] {
            let can = sel.is_some_and(|t| app.project().external_unit_of(t, kind).is_none());
            if w::button(ui, Src::Key(key), 0.0, can).clicked() {
                if let Some(track) = sel {
                    svc::add_unit(app, track, kind);
                }
            }
        }
    });
    if app.project().external_units.is_empty() {
        note(ui, &t("external.none"), false);
        return;
    }
    let units = app.project().external_units.clone();
    for u in units {
        unit_block(app, ui, &u);
    }
}

fn unit_block(app: &mut CantusApp, ui: &mut Ui, u: &ExternalUnit) {
    let present = svc::present(app);
    let state = svc::state(app, u);
    let track = app
        .project()
        .track(u.track)
        .map_or_else(|| t("external.state.track"), |t| t.name.clone());
    let kind = match u.kind {
        UnitKind::Instrument => t("external.kind.instrument"),
        UnitKind::Effect => t("external.kind.effect"),
    };
    let mut next = u.clone();
    let mut remove = false;
    let mut ping = false;
    ui.horizontal(|ui| {
        w::row_label(ui, "", LABEL_W, 24.0);
        note(
            ui,
            &tf(
                "external.unit.title",
                &[
                    ("track", &track),
                    ("kind", &kind),
                    ("state", &t(state.key())),
                ],
            ),
            !matches!(state, Availability::Ready),
        );
    });
    // Routing.
    ui.horizontal(|ui| {
        w::row_label(ui, "external.row.routing", LABEL_W, 24.0);
        if let Some(m) = &u.midi {
            let ports = &present.midi_outputs;
            let at = ports.iter().position(|p| *p == m.port);
            let shown = if m.port.is_empty() {
                t("external.no_port")
            } else {
                m.port.clone()
            };
            let by = stepper(ui, &shown, &t("external.port"), 200.0);
            if by != 0 && !ports.is_empty() {
                let i = at.map_or(0, |i| {
                    (i as i32 + by).rem_euclid(ports.len() as i32) as usize
                });
                next.midi = Some(MidiRoute {
                    port: ports[i].clone(),
                    ordinal: 0,
                    channel: m.channel,
                });
            }
            let by = stepper(
                ui,
                &tf("external.channel", &[("n", &(m.channel + 1))]),
                &t("external.channel_name"),
                84.0,
            );
            if by != 0 {
                next.midi = Some(MidiRoute {
                    channel: (i32::from(m.channel) + by).rem_euclid(16) as u8,
                    ..next.midi.clone().unwrap_or(m.clone())
                });
            }
        }
        if let Some(s) = u.send {
            let by = stepper(
                ui,
                &tf("external.send_pair", &[("pair", &pair_text(s))]),
                &t("external.send_name"),
                110.0,
            );
            if by != 0 {
                next.send = Some(step_pair(s, by, present.out_channels.max(2)));
            }
        }
        let by = stepper(
            ui,
            &tf("external.return_pair", &[("pair", &pair_text(u.ret))]),
            &t("external.return_name"),
            110.0,
        );
        if by != 0 {
            next.ret = step_pair(u.ret, by, present.in_channels.max(2));
        }
    });
    // Latency, trim, level, switches.
    ui.horizontal(|ui| {
        w::row_label(ui, "external.row.latency", LABEL_W, 24.0);
        let said = match (&u.latency, app.external.outcomes.get(&u.id)) {
            (_, Some(Outcome::Measuring)) => t("external.ping.running"),
            (_, Some(Outcome::Failed(e))) => t(e.key()),
            (Some(m), _) => tf(
                "external.latency",
                &[
                    ("frames", &m.frames),
                    ("ms", &format!("{:.1}", m.ms())),
                    (
                        "jitter",
                        &match app.external.outcomes.get(&u.id) {
                            Some(Outcome::Done(r)) => r.jitter,
                            _ => 0,
                        },
                    ),
                ],
            ),
            (None, _) => t("external.not_measured"),
        };
        // Measured on another interface (or before the interface was recorded): used as it
        // is, and a new measurement suggested; another buffer on the same one: re-based.
        let said = match svc::staleness(app, u) {
            cantus_core::external_unit::Staleness::OtherDevice { measured_buffer }
                if !matches!(app.external.outcomes.get(&u.id), Some(Outcome::Measuring)) =>
            {
                format!(
                    "{said} - {}",
                    if measured_buffer == 0 {
                        t("external.stale_unknown")
                    } else {
                        tf("external.stale_device", &[("buffer", &measured_buffer)])
                    }
                )
            }
            cantus_core::external_unit::Staleness::Rebased {
                measured_buffer,
                now_buffer,
            } => format!(
                "{said} - {}",
                tf(
                    "external.rebased",
                    &[("from", &measured_buffer), ("to", &now_buffer)]
                )
            ),
            _ => said,
        };
        let failed = matches!(app.external.outcomes.get(&u.id), Some(Outcome::Failed(_)));
        let ready = svc::cannot_ping(app, u.id).is_none();
        let label = if u.is_measured() {
            Src::Key("external.remeasure")
        } else {
            Src::Key("external.ping")
        };
        if w::button(ui, label, 0.0, ready).clicked() {
            ping = true;
        }
        note(ui, &said, failed);
    });
    ui.horizontal(|ui| {
        w::row_label(ui, "external.row.adjust", LABEL_W, 24.0);
        let by = stepper(
            ui,
            &tf("external.trim", &[("ms", &format!("{:+.1}", u.trim_ms))]),
            &t("external.trim_name"),
            110.0,
        );
        if by != 0 {
            next.trim_ms = (u.trim_ms + by as f32 * 0.5).clamp(-500.0, 500.0);
        }
        let by = stepper(
            ui,
            &tf("external.level", &[("db", &format!("{:+.0}", u.return_db))]),
            &t("external.level_name"),
            90.0,
        );
        if by != 0 {
            next.return_db = (u.return_db + by as f32).clamp(-60.0, 24.0);
        }
        let mut on = u.enabled;
        if led_button(
            ui,
            &mut on,
            Src::Key("external.on"),
            &t("external.on"),
            70.0,
        )
        .changed()
        {
            next.enabled = on;
        }
        if w::button(ui, Src::Key("external.remove"), 0.0, true).clicked() {
            remove = true;
        }
    });
    if remove {
        svc::remove_unit(app, u.id);
    } else if next != *u {
        app.exec(Command::SetExternalUnit {
            unit: Box::new(next),
        });
    }
    if ping {
        if let Err(why) = svc::start_ping(app, u.id) {
            app.set_error(t(why));
        }
    }
}

fn surfaces_band(app: &mut CantusApp, ui: &mut Ui) {
    w::band(ui, Src::Key("external.band.surfaces"));
    let inputs = app.midi_port_names.clone();
    let outputs = cantus_engine::midi::list_outputs();
    let kinds = crate::surface::service::kinds();
    let mut list = app.settings.surfaces.clone();
    let mut changed = false;
    ui.horizontal(|ui| {
        w::row_label(ui, "", LABEL_W, 24.0);
        if w::button(ui, Src::Key("external.surface.add"), 0.0, true).clicked() {
            list.push(SurfaceChoice {
                input: inputs.first().cloned().unwrap_or_default(),
                ..Default::default()
            });
            changed = true;
        }
        if w::button(ui, Src::Key("external.surface.add_virtual"), 0.0, true).clicked() {
            list.push(SurfaceChoice {
                virtual_unit: true,
                ..Default::default()
            });
            changed = true;
        }
    });
    if list.is_empty() {
        note(ui, &t("external.surface.none"), false);
    }
    let mut drop_at = None;
    for (i, c) in list.iter_mut().enumerate() {
        ui.horizontal(|ui| {
            w::row_label(ui, "external.row.surface", LABEL_W, 24.0);
            let k = kinds.iter().position(|(id, _)| *id == c.kind).unwrap_or(0);
            let by = stepper(ui, &t(kinds[k].1), &t("external.surface.kind"), 150.0);
            if by != 0 {
                c.kind = kinds[(k as i32 + by).rem_euclid(kinds.len() as i32) as usize]
                    .0
                    .clone();
                changed = true;
            }
            if c.virtual_unit {
                note(ui, &t("external.surface.virtual"), false);
            } else {
                let at = inputs.iter().position(|p| *p == c.input);
                let shown = if c.input.is_empty() {
                    t("external.no_port")
                } else {
                    c.input.clone()
                };
                let by = stepper(ui, &shown, &t("external.surface.input"), 190.0);
                if by != 0 && !inputs.is_empty() {
                    let j = at.map_or(0, |j| {
                        (j as i32 + by).rem_euclid(inputs.len() as i32) as usize
                    });
                    c.input = inputs[j].clone();
                    changed = true;
                }
                let at = outputs.iter().position(|p| *p == c.output);
                let shown = if c.output.is_empty() {
                    t("external.surface.output_auto")
                } else {
                    c.output.clone()
                };
                let by = stepper(ui, &shown, &t("external.surface.output"), 190.0);
                if by != 0 {
                    // Auto, then each output.
                    let n = outputs.len() as i32 + 1;
                    let j = at.map_or(0, |j| j as i32 + 1);
                    let j = (j + by).rem_euclid(n);
                    c.output = if j == 0 {
                        String::new()
                    } else {
                        outputs[j as usize - 1].clone()
                    };
                    changed = true;
                }
            }
            let mut on = c.enabled;
            if led_button(
                ui,
                &mut on,
                Src::Key("external.on"),
                &t("external.on"),
                70.0,
            )
            .changed()
            {
                c.enabled = on;
                changed = true;
            }
            if w::button(ui, Src::Key("external.remove"), 0.0, true).clicked() {
                drop_at = Some(i);
            }
        });
        // Faders without touch sensing: the motors hold after a move instead of fighting the
        // hand (some MCU-mode controllers have none).
        ui.horizontal(|ui| {
            w::row_label(ui, "", LABEL_W, 24.0);
            let mut touch = c.touch_sensing;
            if led_button(
                ui,
                &mut touch,
                Src::Key("external.surface.touch"),
                &t("external.surface.touch"),
                170.0,
            )
            .changed()
            {
                c.touch_sensing = touch;
                changed = true;
            }
            note(
                ui,
                &t(if c.touch_sensing {
                    "external.surface.touch_on"
                } else {
                    "external.surface.touch_off"
                }),
                false,
            );
        });
    }
    if let Some(i) = drop_at {
        list.remove(i);
        changed = true;
    }
    if changed {
        app.settings.surfaces = list;
        app.store_settings();
    }
    // What the surfaces said and were told, newest last.
    let tail: Vec<String> = app
        .surfaces
        .log
        .iter()
        .rev()
        .take(4)
        .rev()
        .cloned()
        .collect();
    if !tail.is_empty() {
        ui.horizontal(|ui| {
            w::row_label(ui, "external.row.log", LABEL_W, 24.0);
            note(ui, &tail.join("   "), false);
        });
    }
}
