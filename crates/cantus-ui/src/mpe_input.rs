//! The MPE input (ADR 0080): which MIDI ports are MPE controllers and their zone layout,
//! kept in the settings, published to the MIDI callbacks (`inst::mpe::set_mpe_ports`) and
//! to the engine (`EngineMessage::SetMpe`) wherever ports are opened — at start-up
//! (`CantusApp::assemble`), after an audio restart and on a rescan — and drawn as the MPE
//! band of Settings ▸ MIDI.

use crate::app::CantusApp;
use crate::i18n::t;
use crate::kit::knob::KnobSize;
use crate::kit::text::Src;
use crate::kit::widgets::{segmented, Param};
use crate::settings::Settings;
use cantus_core::model::note_expr::zones::{MpeZones, Zone};
use cantus_engine::EngineMessage;
use egui::Ui;
use std::cell::Cell;
use std::fmt::Write as _;

thread_local! {
    /// What this thread last sent the engine (`None` never sent): what the boot test reads.
    static LAST: Cell<Option<Option<MpeZones>>> = const { Cell::new(None) };
}

/// The layout the engine plays MPE by: `None` when no port is marked MPE.
pub fn zones_for(s: &Settings) -> Option<MpeZones> {
    (!s.mpe_ports.is_empty()).then(|| s.mpe_zones.repaired())
}

/// Tell the MIDI callbacks which open ports are MPE and the engine their layout.
pub fn publish(app: &mut CantusApp) {
    let word =
        cantus_engine::inst::mpe::set_mpe_ports(&app.midi_port_names, &app.settings.mpe_ports);
    // A port marked MPE but not open (the controller unplugged) is no MPE input: every
    // open port is then ordinary, and the engine must not read members (review fix 1).
    let zones = zones_for(&app.settings).filter(|_| word != 0);
    app.engine.send(EngineMessage::SetMpe(zones));
    LAST.with(|l| l.set(Some(zones)));
}

/// What [`publish`] last sent from this thread (`None`: it never ran here).
pub fn last_published() -> Option<Option<MpeZones>> {
    LAST.with(Cell::get)
}

/// Mark a port MPE or not.
pub fn set_port(app: &mut CantusApp, name: &str, mpe: bool) {
    let s = &mut app.settings.mpe_ports;
    let had = s.iter().any(|n| n == name);
    if mpe && !had {
        s.push(name.to_string());
    } else if !mpe && had {
        s.retain(|n| n != name);
    } else {
        return;
    }
    app.store_settings();
    publish(app);
}

/// A new zone layout.
pub fn set_zones(app: &mut CantusApp, zones: MpeZones) {
    let zones = zones.repaired();
    if zones == app.settings.mpe_zones {
        return;
    }
    app.settings.mpe_zones = zones;
    app.store_settings();
    publish(app);
}

/// The MPE cell of one port's row: *Off* or *MPE* (the per-port table's column).
pub fn port_cell(app: &mut CantusApp, ui: &mut Ui, name: &str) {
    let on = app.settings.mpe_ports.iter().any(|n| n == name);
    let mut sel = usize::from(on);
    let labels = [Src::Key("mpe.port.off"), Src::Key("mpe.port.on")];
    let tip = format!("{} — {name}", t("mpe.port"));
    if segmented(ui, &mut sel, &labels, &tip, 120.0).changed() {
        set_port(app, name, sel == 1);
    }
}

/// Settings ▸ MIDI's MPE band: the zone layout and the bend ranges.
pub fn settings(app: &mut CantusApp, ui: &mut Ui, label_w: f32) {
    use crate::views::workspace_widgets as w;
    // Each port's mark is the MPE column of Settings ▸ MIDI's input table (`port_cell`).
    w::band(ui, Src::Key("mpe.band"));
    let z = app.settings.mpe_zones;
    // The layout: lower, upper or both (a split controller).
    let mut layout = match (z.lower.is_some(), z.upper.is_some()) {
        (true, true) => 2,
        (false, true) => 1,
        _ => 0,
    };
    let labels = [
        Src::Key("mpe.zone.lower"),
        Src::Key("mpe.zone.upper"),
        Src::Key("mpe.zone.both"),
    ];
    let mut next = z;
    ui.horizontal(|ui| {
        w::row_label(ui, "mpe.zones", label_w, 24.0);
        if segmented(ui, &mut layout, &labels, &t("mpe.zones"), 300.0).changed() {
            next = match layout {
                1 => MpeZones {
                    lower: None,
                    upper: Some(z.upper.or(z.lower).unwrap_or(Zone::new(15))),
                },
                2 => MpeZones {
                    lower: Some(Zone::new(7)),
                    upper: Some(Zone::new(7)),
                },
                _ => MpeZones {
                    lower: Some(z.lower.or(z.upper).unwrap_or(Zone::new(15))),
                    upper: None,
                },
            };
        }
    });
    for (upper, key) in [(false, "mpe.zone.lower"), (true, "mpe.zone.upper")] {
        let Some(zone) = next.zone(upper) else {
            continue;
        };
        let mut zone2 = zone;
        ui.horizontal(|ui| {
            w::row_label(ui, key, label_w, 70.0);
            let mut members = f32::from(zone.members);
            knob(ui, &mut members, 1.0..=15.0, 15.0, "mpe.members", |v, b| {
                let _ = write!(b, "{}", v.round() as u8);
            });
            let mut bend = f32::from(zone.member_bend);
            knob(
                ui,
                &mut bend,
                1.0..=96.0,
                48.0,
                "mpe.member_bend",
                |v, b| {
                    let _ = write!(b, "±{} st", v.round() as u8);
                },
            );
            zone2.members = members.round().clamp(1.0, 15.0) as u8;
            zone2.member_bend = bend.round().clamp(1.0, 96.0) as u8;
        });
        if upper {
            next.upper = Some(zone2);
        } else {
            next.lower = Some(zone2);
        }
    }
    ui.horizontal(|ui| {
        w::row_label(ui, "", label_w, 18.0);
        dim(ui, &t("mpe.zones_hint"));
    });
    if next != app.settings.mpe_zones {
        set_zones(app, next);
    }
}

/// A line of dim text, wrapped to the row.
fn dim(ui: &mut Ui, s: &str) {
    let st = crate::kit::Style::current();
    let width = ui.available_width();
    crate::views::workspace_settings::wrapped(
        ui,
        s,
        crate::kit::text::Role::Label,
        st.t.text_dim,
        width,
    );
}

/// One small knob with its cell and name.
fn knob(
    ui: &mut Ui,
    v: &mut f32,
    range: std::ops::RangeInclusive<f32>,
    default: f32,
    key: &'static str,
    format: impl Fn(f32, &mut String),
) {
    let name = t(key);
    Param::new(v, range, default)
        .format(format)
        // L: the cell's caption ("Channels", "Bend range") reads whole in both languages.
        .size(KnobSize::L)
        .cell(Src::Key(key))
        .name(&name)
        .show(ui);
}
