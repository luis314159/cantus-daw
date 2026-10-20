//! Low-latency monitoring on screen (round 29, ADR 0064 §4): its switch and its limit where
//! a recording musician looks — the record button's popover and the audio chip's — with
//! the inserts it bypasses named ("Monitoring without: Aequalis, Culmen"), and the round
//! trip of what is monitored on the chip.

use crate::app::CantusApp;
use crate::i18n::{t, tf};
use crate::kit::text::Src;
use cantus_core::EffectId;
use egui::Ui;

/// The limits offered, ms: Tonus (2.7 ms) stays at every one of them.
pub const LIMITS: [f32; 4] = [3.0, 5.0, 10.0, 20.0];

/// Switch it on or off: the setting, the engine, the status line.
pub fn set(app: &mut CantusApp, on: bool) {
    app.settings.low_latency = on;
    app.store_settings();
    crate::freeze::apply_low_latency(app);
    app.set_status(t(if on {
        "freeze.ll.now_on"
    } else {
        "freeze.ll.now_off"
    }));
}

/// The limit chosen, ms.
fn set_limit(app: &mut CantusApp, ms: f32) {
    app.settings.low_latency_ms = ms;
    app.store_settings();
    crate::freeze::apply_low_latency(app);
}

/// The name of an insert the engine names by id: its plug-in's or its kind's.
fn effect_name(app: &CantusApp, id: EffectId) -> Option<String> {
    let (_, slot) = app.project().all_effects().find(|(_, e)| e.id == id)?;
    Some(match slot.spec.external.as_deref() {
        Some(p) => p.name.clone(),
        None => t(slot.spec.kind.name_key()),
    })
}

/// "Aequalis, Culmen": the inserts low-latency monitoring bypasses now (empty: none) —
/// "and 3 more" past the names the engine keeps (review 1).
pub fn bypassed(app: &CantusApp) -> String {
    let ll = app.engine.stats().low_latency;
    let names = ll
        .off
        .iter()
        .take(ll.off_count as usize)
        .filter_map(|id| effect_name(app, EffectId(*id)))
        .collect::<Vec<_>>()
        .join(", ");
    let more = (ll.off_total as usize).saturating_sub(ll.off_count as usize);
    if more > 0 {
        tf("freeze.ll.more", &[("names", &names), ("n", &more)])
    } else {
        names
    }
}

/// The round trip of what is monitored, ms — the driver's, the input's wait and the
/// monitored chain's — while the mode is on and something is monitored.
pub fn monitored_ms(app: &CantusApp) -> Option<f64> {
    let stats = app.engine.stats();
    let ll = stats.low_latency;
    if !ll.on || !ll.monitoring || app.engine.sample_rate <= 0.0 {
        return None;
    }
    let samples = app.engine.device_latency().round_trip() as f64
        + stats.input_queue.max(0.0)
        + ll.monitored_samples as f64;
    Some(samples * 1000.0 / app.engine.sample_rate)
}

/// The rows of the record button's popover and of the audio chip's: the switch, the limit,
/// and what it bypasses.
pub fn popover_rows(app: &mut CantusApp, ui: &mut Ui, control_w: f32) {
    let s = crate::kit::Style::current();
    // The value column: what is left beside the labels (92 at 100 %), never past the edge.
    let control_w = control_w.min((ui.available_width() / s.k - 92.0 - 10.0).max(120.0));
    super::record_options::band_row(ui, "freeze.ll.band", &s);
    ui.horizontal(|ui| {
        super::record_options::label(ui, "freeze.ll.row", &s);
        let mut on = app.settings.low_latency;
        let key = app
            .keymap
            .bindings_for(crate::keymap::Action::ToggleLowLatency)
            .first()
            .map(|b| b.display())
            .unwrap_or_default();
        let name = tf("freeze.ll.tip", &[("key", &key)]);
        if crate::kit::widgets::led_button(ui, &mut on, Src::Key("freeze.ll.on"), &name, 90.0)
            .changed()
        {
            set(app, on);
        }
    });
    ui.horizontal(|ui| {
        super::record_options::label(ui, "freeze.ll.limit", &s);
        let mut i = LIMITS
            .iter()
            .position(|l| (l - app.settings.low_latency_ms).abs() < 0.01)
            .unwrap_or(0);
        let words: Vec<String> = LIMITS
            .iter()
            .map(|l| tf("freeze.ll.ms", &[("ms", &l)]))
            .collect();
        let labels: Vec<Src<'_>> = words.iter().map(|w| Src::Str(w)).collect();
        if crate::kit::widgets::segmented(ui, &mut i, &labels, &t("freeze.ll.limit_tip"), control_w)
            .changed()
        {
            set_limit(app, LIMITS[i]);
        }
    });
    if app.settings.low_latency {
        let off = bypassed(app);
        let text = if off.is_empty() {
            t("freeze.ll.none_off")
        } else {
            tf("freeze.ll.without", &[("names", &off)])
        };
        ui.horizontal(|ui| {
            super::record_options::label(ui, "freeze.ll.now", &s);
            let _ = crate::kit::widgets::readout(ui, &text, false, &text, control_w);
        });
        if let Some(ms) = monitored_ms(app) {
            let rt = tf("freeze.ll.round_trip", &[("ms", &format!("{ms:.1}"))]);
            ui.horizontal(|ui| {
                super::record_options::label(ui, "freeze.ll.heard", &s);
                let _ = crate::kit::widgets::readout(ui, &rt, true, &rt, control_w);
            });
        }
    }
}
