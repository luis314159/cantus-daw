//! Settings ▸ MIDI (round 33, ADR 0078–0079): a table of the ports — each input's notes,
//! controllers, clock and time code; each output's clock, time code, controller feedback
//! and offset — and the sync band: what is followed (nothing, Link, a port's clock, a
//! port's time code), Link's quantum and start/stop, the time code's rate and start.
//!
//! Every column is a toggle in the kit's look (an LED key), a label above it; a port's
//! name is elided in its column, its whole name in the hover. A change applies at once
//! (`sync_service::apply`) and is kept.

use super::LABEL_W;
use crate::app::CantusApp;
use crate::i18n::{t, tf};
use crate::kit::hint::{self, Caps};
use crate::kit::text::{self, Elide, Policy, Role, Src, Text};
use crate::kit::widgets::{led_button, menu_field, segmented};
use crate::kit::{with_brush, Style};
use crate::views::transport::sync_service::{self as svc, SyncSource, MAX_OFFSET_MS, QUANTA};
use crate::views::workspace_widgets as w;
use cantus_engine::transport::sync::mtc;
use egui::{Align2, Id, Sense, Ui, Vec2};

/// The port column's width and each toggle's, points.
const PORT_W: f32 = 220.0;
/// The inputs table: its toggle columns (the MPE mark among them), each as wide as its
/// heading reads and an LED key at the least; the port's column takes what is left between
/// a floor and a ceiling, and past the floor the table scrolls sideways. // r33:launcher
const INPUT_TOGGLES: [&str; 5] = [
    // r33:launcher
    "sync.col.notes",       // r33:launcher
    "sync.col.controllers", // r33:launcher
    "sync.col.clock",       // r33:launcher
    "sync.col.mtc",         // r33:launcher
    "mpe.band",             // r33:launcher
]; // r33:launcher
const LED_MIN: f32 = 40.0; // r33:launcher
const PORT_MIN: f32 = 150.0; // r33:launcher
const PORT_MAX: f32 = 260.0; // r33:launcher
const COL_W: f32 = 74.0;
const OFFSET_W: f32 = 110.0;

/// A column heading: a small caps word, centred over its column (the port's, over names
/// read from the left, at the left). // r33:launcher
fn head(ui: &mut Ui, key: &str, width: f32) -> egui::Rect {
    // r33:launcher (returns its rect)
    let at = if key == "sync.col.port" {
        Align2::LEFT_CENTER
    } else {
        Align2::CENTER_CENTER
    }; // r33:launcher
    let s = Style::current();
    let (rect, _) = ui.allocate_exact_size(Vec2::new(width * s.k, 18.0 * s.k), Sense::hover());
    with_brush(ui.painter(), |b| {
        let _ = text::paint(
            b,
            rect,
            &Text::key(key, Role::Micro, s.t.text_dim)
                .caps(true)
                .align(at) // r33:launcher
                .policy(Policy::Elide(Elide::End)),
            s.tk(),
        );
    });
    rect // r33:launcher
}

/// How wide a heading's word reads, points (an LED key at the least). // r33:launcher
fn head_w(ui: &Ui, key: &str) -> f32 {
    // r33:launcher
    let s = Style::current(); // r33:launcher
    let word = Text::key(key, Role::Micro, s.t.text_dim).caps(true); // r33:launcher
    (text::measure(ui.ctx(), &word, s.tk()).x / s.k + 14.0).max(LED_MIN) // r33:launcher
} // r33:launcher

/// Where the inputs table was laid out in the last frame: what its scroll shows, all it
/// holds, each column's heading and its first row's cell (the layout test reads it).
#[derive(Clone, Debug)] // r33:launcher
pub struct InputsLayout {
    // r33:launcher
    pub viewport: egui::Rect,   // r33:launcher
    pub content: egui::Rect,    // r33:launcher
    pub heads: Vec<egui::Rect>, // r33:launcher
    pub cells: Vec<egui::Rect>, // r33:launcher
} // r33:launcher

fn layout_id() -> Id {
    // r33:launcher
    Id::new("settings-midi-inputs-layout") // r33:launcher
} // r33:launcher

/// The inputs table's layout in the last frame (`InputsLayout`). // r33:launcher
pub fn inputs_layout(ctx: &egui::Context) -> Option<InputsLayout> {
    // r33:launcher
    ctx.data(|d| d.get_temp(layout_id())) // r33:launcher
} // r33:launcher

/// A port's name in its column, elided; the whole name in the hover.
fn port_name(ui: &mut Ui, name: &str, width: f32) -> egui::Rect {
    // r33:launcher (its width; returns its rect)
    let s = Style::current();
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(width * s.k, 22.0 * s.k), Sense::hover());
    with_brush(ui.painter(), |b| {
        let _ = text::paint(
            b,
            rect,
            &Text::new(name, Role::Body, s.t.text)
                .align(Align2::LEFT_CENTER)
                .policy(Policy::Elide(Elide::End)),
            s.tk(),
        );
    });
    if resp.hovered() {
        hint::set_text(ui.ctx(), name, "", Caps::NONE);
    }
    rect // r33:launcher
}

/// A toggle cell: an LED key with no word (the column's heading names it).
fn toggle(ui: &mut Ui, on: &mut bool, tip_key: &str, salt: (&str, &str)) -> bool {
    toggle_cell(ui, on, tip_key, salt, COL_W).0 // r33:launcher
}

/// [`toggle`] `width` wide with its gap, and the cell it took. // r33:launcher
fn toggle_cell(
    ui: &mut Ui,
    on: &mut bool,
    tip_key: &str,
    salt: (&str, &str),
    width: f32,
) -> (bool, egui::Rect) {
    // r33:launcher
    let tip = t(tip_key);
    let id = Id::new(("midi-port", salt.0, salt.1));
    let r = ui.push_id(id, |ui| led_button(ui, on, Src::Str(""), &tip, width - 6.0)); // r33:launcher
    let s = Style::current(); // r33:launcher
    let cell = egui::Rect::from_min_size(
        r.inner.rect.min,
        Vec2::new(width * s.k, r.inner.rect.height()),
    ); // r33:launcher
    ui.add_space(6.0 * s.k); // r33:launcher
    (r.inner.changed(), cell) // r33:launcher
}

/// A note under the value column, its lines stacked within the page (in the row's own
/// horizontal layout they would run on past the page's edge).
fn indented(ui: &mut Ui, s: &str, warn: bool) {
    ui.horizontal_top(|ui| {
        w::row_label(ui, "", LABEL_W, 18.0);
        ui.vertical(|ui| super::note(ui, s, warn));
    });
}

pub fn page(app: &mut CantusApp, ui: &mut Ui) {
    ports_band(app, ui);
    inputs_band(app, ui);
    crate::mpe::input::settings(app, ui, LABEL_W); // r33:mpe — zones and bend ranges
    outputs_band(app, ui);
    sync_band(app, ui);
}

fn ports_band(app: &mut CantusApp, ui: &mut Ui) {
    w::band(ui, Src::Key("settings.midi"));
    ui.horizontal(|ui| {
        w::row_label(ui, "", LABEL_W, 24.0);
        if w::button(ui, Src::Key("settings.rescan_midi"), 0.0, true).clicked() {
            app.rescan_midi();
        }
        super::note(
            ui,
            &tf("status.midi", &[("n", &app.midi_port_names.len())]),
            false,
        );
    });
}

// r33:launcher — from here to the end of `inputs_band`, the launcher's layout (round 33's
// review: the merged table ran past the page, its Grid column clipped).
fn inputs_band(app: &mut CantusApp, ui: &mut Ui) {
    w::band(ui, Src::Key("sync.band.inputs"));
    if app.midi_port_names.is_empty() {
        super::note(ui, &t("settings.no_midi"), false);
        return;
    }
    let s = Style::current();
    let k = s.k;
    let gap = ui.spacing().item_spacing.x / k;
    let widths: Vec<f32> = INPUT_TOGGLES.iter().map(|key| head_w(ui, key)).collect();
    let grid_key = crate::controllers::grid::COLUMN_KEY;
    let grid_w = crate::controllers::grid::cell_width(ui).max(head_w(ui, grid_key));
    let fixed: f32 = widths.iter().map(|w| w + gap).sum::<f32>() + grid_w + gap;
    let port_w = (ui.available_width() / k - fixed).clamp(PORT_MIN, PORT_MAX);
    let total = (port_w + fixed) * k;
    let names = app.midi_port_names.clone();
    let mut changed = false;
    let mut heads = Vec::new();
    let mut cells = Vec::new();
    let out = egui::ScrollArea::horizontal()
        .id_salt("settings-midi-inputs")
        .auto_shrink([false, true])
        // Seen whenever the table overflows: a column past the edge is not hidden.
        .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::VisibleWhenNeeded)
        .show(ui, |ui| {
            ui.set_min_width(total);
            ui.horizontal(|ui| {
                heads.push(head(ui, "sync.col.port", port_w));
                for (key, w) in INPUT_TOGGLES.iter().zip(&widths) {
                    heads.push(head(ui, key, *w));
                }
                heads.push(head(ui, grid_key, grid_w));
            });
            for (row, name) in names.iter().enumerate() {
                ui.horizontal(|ui| {
                    let mut at = vec![port_name(ui, name, port_w)];
                    let st = &mut app.settings.sync;
                    let mut p = st.input(name);
                    let mine = st.source_port.as_ref() == Some(name);
                    let mut clock = st.source == SyncSource::Clock && mine;
                    let mut code = st.source == SyncSource::Mtc && mine;
                    let cell = |ui: &mut Ui, on: &mut bool, tip: &str, salt: &str, i: usize| {
                        toggle_cell(ui, on, tip, (name, salt), widths[i])
                    };
                    let (a, r) = cell(ui, &mut p.notes, "sync.tip.notes", "notes", 0);
                    at.push(r);
                    let (b, r) = cell(ui, &mut p.controllers, "sync.tip.controllers", "ctl", 1);
                    at.push(r);
                    let (c, r) = cell(ui, &mut clock, "sync.tip.in_clock", "clock", 2);
                    at.push(r);
                    let (d, r) = cell(ui, &mut code, "sync.tip.in_mtc", "mtc", 3);
                    at.push(r);
                    if a || b {
                        *st.input_mut(name) = p;
                    }
                    if c || d {
                        // One source at a time: this port's clock or code, or nothing.
                        let (on, kind) = if c {
                            (clock, SyncSource::Clock)
                        } else {
                            (code, SyncSource::Mtc)
                        };
                        if on {
                            st.source = kind;
                            st.source_port = Some(name.clone());
                        } else {
                            st.source = SyncSource::None;
                        }
                    }
                    changed |= a || b || c || d;
                    // The MPE mark, an LED key as its neighbours (round 33, ADR 0079).
                    let mut mpe = app.settings.mpe_ports.iter().any(|n| n == name);
                    let (m, r) = cell(ui, &mut mpe, "mpe.port", "mpe", 4);
                    at.push(r);
                    if m {
                        crate::mpe::input::set_port(app, name, mpe);
                    }
                    at.push(crate::controllers::grid::cell(app, ui, name, grid_w));
                    if row == 0 {
                        cells = at;
                    }
                });
            }
        });
    let content =
        egui::Rect::from_min_size(out.inner_rect.min - out.state.offset, out.content_size);
    let layout = InputsLayout {
        viewport: out.inner_rect,
        content,
        heads,
        cells,
    };
    ui.ctx().data_mut(|d| d.insert_temp(layout_id(), layout));
    if changed {
        svc::apply(app);
        app.store_settings();
    }
}
// r33:launcher — end of `inputs_band`.

fn outputs_band(app: &mut CantusApp, ui: &mut Ui) {
    w::band(ui, Src::Key("sync.band.outputs"));
    let outputs = outputs_now(app);
    if outputs.is_empty() {
        super::note(ui, &t("sync.no_outputs"), false);
        return;
    }
    ui.horizontal(|ui| {
        head(ui, "sync.col.port", PORT_W);
        for key in ["sync.col.clock", "sync.col.mtc", "sync.col.feedback"] {
            head(ui, key, COL_W);
        }
        head(ui, "sync.col.offset", OFFSET_W);
    });
    let mut changed = false;
    for name in &outputs {
        ui.horizontal(|ui| {
            port_name(ui, name, PORT_W); // r33:launcher (its width)
            let mut p = app.settings.sync.output(name);
            let mut c = toggle(ui, &mut p.clock, "sync.tip.out_clock", (name, "oclock"));
            c |= toggle(ui, &mut p.mtc, "sync.tip.out_mtc", (name, "omtc"));
            c |= toggle(ui, &mut p.feedback, "sync.tip.feedback", (name, "fb"));
            let value = tf(
                "sync.offset_value",
                &[("ms", &format!("{:+.1}", p.offset_ms))],
            );
            let (_, clicks) = ui
                .push_id(("midi-offset", name.as_str()), |ui| {
                    menu_field(ui, &value, true, false, &t("sync.tip.offset"), OFFSET_W)
                })
                .inner;
            if clicks.prev || clicks.next || clicks.open {
                let step = if clicks.prev { -0.5 } else { 0.5 };
                p.offset_ms = if clicks.open {
                    0.0
                } else {
                    (p.offset_ms + step).clamp(-MAX_OFFSET_MS, MAX_OFFSET_MS)
                };
                c = true;
            }
            if c {
                *app.settings.sync.output_mut(name) = p;
                changed = true;
            }
        });
    }
    super::note(ui, &t("sync.note.offset"), false);
    if changed {
        svc::apply(app);
        app.store_settings();
    }
}

/// The screenshots' ports: these inputs, and these outputs listed for an hour.
pub fn demo_ports(app: &mut CantusApp, inputs: &[&str], outputs: &[&str]) {
    app.midi_port_names = inputs.iter().map(|s| s.to_string()).collect();
    let later = std::time::Instant::now() + std::time::Duration::from_secs(3600);
    app.settings_page.midi_outputs = (Some(later), outputs.iter().map(|s| s.to_string()).collect());
}

/// The outputs present, listed once a second at most (enumerating ports is slow).
fn outputs_now(app: &mut CantusApp) -> Vec<String> {
    let now = std::time::Instant::now();
    let cache = &mut app.settings_page.midi_outputs;
    if cache
        .0
        .is_none_or(|t| now.duration_since(t).as_secs_f32() > 1.0)
    {
        *cache = (Some(now), cantus_engine::midi::list_outputs());
    }
    cache.1.clone()
}

fn sync_band(app: &mut CantusApp, ui: &mut Ui) {
    w::band(ui, Src::Key("sync.band.sync"));
    let k = Style::current().k;
    let value_w = |ui: &Ui| (ui.available_width() / k - 8.0).clamp(160.0, 360.0);
    let mut changed = false;
    // The source.
    ui.horizontal(|ui| {
        w::row_label(ui, "sync.source", LABEL_W, 24.0);
        let order = [
            SyncSource::None,
            SyncSource::Link,
            SyncSource::Clock,
            SyncSource::Mtc,
        ];
        let mut sel = order
            .iter()
            .position(|x| *x == app.settings.sync.source)
            .unwrap_or(0);
        let labels = [
            Src::Key("sync.seg.off"),
            Src::Key("sync.seg.link"),
            Src::Key("sync.seg.clock"),
            Src::Key("sync.seg.mtc"),
        ];
        let width = value_w(ui);
        if segmented(ui, &mut sel, &labels, &t("sync.source"), width).changed() {
            let st = &mut app.settings.sync;
            st.source = order[sel];
            if matches!(st.source, SyncSource::Clock | SyncSource::Mtc) && st.source_port.is_none()
            {
                st.source_port = app.midi_port_names.first().cloned();
            }
            changed = true;
        }
    });
    let source = app.settings.sync.source;
    if matches!(source, SyncSource::Clock | SyncSource::Mtc) {
        ui.horizontal(|ui| {
            w::row_label(ui, "sync.port", LABEL_W, 24.0);
            let names = app.midi_port_names.clone();
            let at = app
                .settings
                .sync
                .source_port
                .as_ref()
                .and_then(|n| names.iter().position(|x| x == n));
            let value = match at {
                Some(i) => names[i].clone(),
                None => t("sync.port.none"),
            };
            let width = value_w(ui);
            let (_, clicks) = menu_field(ui, &value, true, false, &t("sync.port"), width);
            if !names.is_empty() && (clicks.prev || clicks.next || clicks.open) {
                let n = names.len();
                let i = match (at, clicks.prev) {
                    (None, _) => 0,
                    (Some(i), true) => (i + n - 1) % n,
                    (Some(i), false) => (i + 1) % n,
                };
                app.settings.sync.source_port = Some(names[i].clone());
                changed = true;
            }
        });
    }
    if source == SyncSource::Link {
        ui.horizontal(|ui| {
            w::row_label(ui, "sync.quantum", LABEL_W, 24.0);
            let labels: Vec<String> = QUANTA.iter().map(|q| format!("{q}")).collect();
            let srcs: Vec<Src> = labels.iter().map(|l| Src::Str(l.as_str())).collect();
            let mut sel = QUANTA
                .iter()
                .position(|q| *q == app.settings.sync.quantum)
                .unwrap_or(3);
            if segmented(ui, &mut sel, &srcs, &t("sync.quantum"), 200.0).changed() {
                app.settings.sync.quantum = QUANTA[sel];
                changed = true;
            }
        });
        ui.horizontal(|ui| {
            w::row_label(ui, "", LABEL_W, 24.0);
            let mut on = app.settings.sync.start_stop;
            if led_button(
                ui,
                &mut on,
                Src::Key("sync.start_stop"),
                &t("sync.start_stop"),
                160.0,
            )
            .changed()
            {
                app.settings.sync.start_stop = on;
                changed = true;
            }
        });
    }
    // Time code: the rate and the start, for code out and in.
    ui.horizontal(|ui| {
        w::row_label(ui, "sync.mtc_rate", LABEL_W, 24.0);
        let labels: Vec<Src> = mtc::Rate::ALL.iter().map(|r| Src::Str(r.label())).collect();
        let mut sel = usize::from(app.settings.sync.mtc_rate.min(3));
        if segmented(ui, &mut sel, &labels, &t("sync.mtc_rate"), 280.0).changed() {
            app.settings.sync.mtc_rate = sel as u8;
            changed = true;
        }
    });
    ui.horizontal(|ui| {
        w::row_label(ui, "sync.mtc_offset", LABEL_W, 24.0);
        let mut text = app.settings.sync.mtc_offset.clone();
        let r = w::text_field(
            ui,
            Id::new("midi-mtc-offset"),
            &mut text,
            "sync.mtc_offset_hint",
            140.0,
        );
        if text != app.settings.sync.mtc_offset {
            app.settings.sync.mtc_offset = text;
            changed = true;
        }
        let _ = r;
    });
    if app.settings.sync.offset_code().is_none() {
        indented(ui, &t("sync.mtc_offset_bad"), true);
    }
    // What is happening now.
    indented(ui, &status_line(app), false);
    if source == SyncSource::Link {
        indented(ui, &t("sync.note.firewall"), false);
    }
    if changed {
        svc::apply(app);
        app.store_settings();
    }
}

/// The band's status line.
pub fn status_line(app: &CantusApp) -> String {
    use std::sync::atomic::Ordering::Relaxed;
    let f = crate::views::transport::sync::facts(app);
    let st = &app.sync.status;
    match f.source {
        SyncSource::None => t("sync.status.none"),
        SyncSource::Link if f.link_failed => t("sync.status.link_failed"),
        SyncSource::Link => {
            let tempo = format!("{:.1}", f64::from_bits(st.link_tempo.load(Relaxed)));
            crate::i18n::tf_count("sync.status.link", f.peers as usize, &[("tempo", &tempo)])
        }
        SyncSource::Clock | SyncSource::Mtc => {
            let Some(port) = app.settings.sync.source_port.clone() else {
                return t("sync.status.no_port");
            };
            if !st.receiving.load(Relaxed) {
                return tf("sync.status.silent", &[("port", &port)]);
            }
            let state = t(match f.lock {
                cantus_engine::transport::sync::Lock::Locked => "sync.lock.locked",
                cantus_engine::transport::sync::Lock::Seeking => "sync.lock.seeking",
                cantus_engine::transport::sync::Lock::Queued => "sync.lock.queued",
                cantus_engine::transport::sync::Lock::Idle => "sync.lock.idle",
            });
            tf(
                "sync.status.following",
                &[
                    (
                        "source",
                        &crate::views::transport::sync::source_name(f.source),
                    ),
                    ("tempo", &format!("{:.1}", f.tempo)),
                    ("state", &state),
                ],
            )
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Round 2, D6: Link on with no interface open says so (it read "0 other apps").
    #[test]
    fn link_with_no_interface_says_so() {
        let mut app = CantusApp::from_parts(None, cantus_engine::EngineHandle::dummy(), false);
        app.settings.sync.source = SyncSource::Link;
        let st = app.sync_service().status.clone();
        st.link_on.store(true, std::sync::atomic::Ordering::Relaxed);
        st.link_failed
            .store(true, std::sync::atomic::Ordering::Relaxed);
        assert_eq!(status_line(&app), t("sync.status.link_failed"));
    }
}
