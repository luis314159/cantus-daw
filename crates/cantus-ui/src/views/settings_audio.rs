//! The Audio page of Settings (ADR 0025): the driver, the output and input devices, the
//! rate and the block size (applied together, with *Apply*), the latencies the drivers
//! report, the record offset with a loopback *Measure*, and the open input's channels with
//! a meter on each pair — what a musician checks before pressing record.
//!
//! Enumerating devices is slow (tens of milliseconds per driver on WASAPI), so the lists
//! are scanned when the page first shows, when the driver changes and on *Rescan*, never
//! every frame.

use super::workspace_settings::note;
use super::workspace_widgets as w;
use crate::app::CantusApp;
use crate::i18n::{t, tf};
use crate::kit::knob::KnobSize;
use crate::kit::meter::{self, Levels, MIN_DB};
use crate::kit::text::{self, Elide, Policy, Role, Src, Text};
use crate::kit::widgets::{menu_field, readout, Param};
use crate::kit::{with_brush, Style};
use crate::settings::{AudioSettings, BUFFER_SIZES, SAMPLE_RATES};
use cantus_engine::{
    DeviceInfo, DeviceLatency, DriverInfo, InputDeviceInfo, LoopbackError, LoopbackResult,
};
use egui::{Align2, Pos2, Rect, Sense, Ui, Vec2};

/// The label column's width, in points (the settings window's).
const LABEL_W: f32 = super::workspace_settings::LABEL_W;
/// The record offset's range, in samples either way (100 ms at 48 kHz).
const OFFSET_RANGE: f32 = 4_800.0;

/// Which of the audio lists is open.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum List {
    Driver,
    Device,
    Input,
    Rate,
    Buffer,
}

/// The devices as last scanned, for one driver.
#[derive(Clone, Debug, Default)]
pub struct Scan {
    /// The driver the lists belong to (`None`: the default).
    pub driver: Option<String>,
    pub drivers: Vec<DriverInfo>,
    pub outputs: Vec<DeviceInfo>,
    pub inputs: Vec<InputDeviceInfo>,
}

impl Scan {
    pub fn of(driver: Option<&str>) -> Scan {
        Scan {
            driver: driver.map(str::to_string),
            drivers: cantus_engine::devices::drivers(),
            outputs: cantus_engine::devices::output_devices_on(driver),
            inputs: cantus_engine::devices::input_devices(driver),
        }
    }
}

/// What the running engine says, read once a frame (or set by a demo).
#[derive(Clone, Debug, Default)]
pub struct Facts {
    pub driver: String,
    pub live: bool,
    pub device: String,
    pub rate: f64,
    pub input: Option<String>,
    pub channels: usize,
    /// Each input channel's peak since the last frame, linear.
    pub peaks: Vec<f32>,
    pub latency: DeviceLatency,
    pub dropped: u64,
    pub input_error: Option<String>,
    /// The output stopped calling back.
    pub stalled: bool,
}

/// Where the loopback measurement is.
#[derive(Clone, Debug, Default, PartialEq)]
pub enum Measure {
    #[default]
    Idle,
    Running,
    Done(LoopbackResult),
    Failed(LoopbackError),
}

/// The page's own state.
#[derive(Default)]
pub struct State {
    pub scan: Option<Scan>,
    pub measure: Measure,
    /// The meters' held peaks, in dB, per input channel.
    hold: Vec<f32>,
    /// Facts to show instead of the engine's (the screenshot example).
    pub demo: Option<Facts>,
}

fn facts(app: &mut CantusApp) -> Facts {
    if let Some(d) = &app.settings_page.audio.demo {
        return d.clone();
    }
    let e = &mut app.engine;
    Facts {
        driver: e.driver_name().to_string(),
        live: e.is_live(),
        device: e.device_name.clone(),
        rate: e.sample_rate,
        input: e.input_device_name().map(str::to_string),
        channels: e.input_channel_count(),
        peaks: e.input_peaks().to_vec(),
        latency: e.device_latency(),
        dropped: e.input_dropped(),
        input_error: e.input_error().map(str::to_string),
        stalled: e.output_stalled(),
    }
}

/// A frame count as milliseconds at `rate`.
fn ms(frames: f64, rate: f64) -> String {
    if rate > 0.0 {
        format!("{:.1}", frames / rate * 1000.0)
    } else {
        "–".into()
    }
}

fn pending(app: &CantusApp) -> AudioSettings {
    app.settings_page
        .pending
        .clone()
        .unwrap_or_else(|| app.settings.audio.clone())
}

/// The lists for the pending driver, scanned if they are not yet.
fn scan(app: &mut CantusApp) -> Scan {
    let driver = pending(app).driver;
    let st = &mut app.settings_page.audio;
    if st.scan.as_ref().is_none_or(|s| s.driver != driver) {
        st.scan = Some(Scan::of(driver.as_deref()));
    }
    st.scan.clone().unwrap_or_default()
}

/// The text a field shows for each list.
fn value(list: List, p: &AudioSettings, scan: &Scan, rate: f64) -> String {
    let default = t("settings.default");
    match list {
        List::Driver => p.driver.clone().unwrap_or_else(|| {
            scan.drivers
                .iter()
                .find(|d| d.is_default)
                .map_or(default.clone(), |d| format!("{} ({default})", d.name))
        }),
        List::Device => p.device.clone().unwrap_or(default),
        List::Input => match p.input_device.as_deref() {
            Some("") => t("devices.input.none"),
            // ASIO drivers are listed without being loaded: their channels are unknown (0).
            Some(name) => match scan
                .inputs
                .iter()
                .find(|i| i.name == name && i.channels > 0)
            {
                Some(i) => format!("{name} · {}", tf("devices.channels", &[("n", &i.channels)])),
                None => name.to_string(),
            },
            None => default,
        },
        List::Rate => p.sample_rate.map_or(default, |r| format!("{r} Hz")),
        List::Buffer => match (p.buffer_frames, p.block_latency_ms(rate)) {
            (Some(b), Some(ms)) => format!("{b} · {ms:.1} ms"),
            (Some(b), None) => b.to_string(),
            (None, _) => default,
        },
    }
}

/// The page: three bands in the settings window's row grammar.
pub fn page(app: &mut CantusApp, ui: &mut Ui) {
    let f = facts(app);
    poll_measure(app, ui.ctx());
    device_band(app, ui, &f);
    latency_band(app, ui, &f);
    inputs_band(app, ui, &f);
}

/// A menu field in a label | value row; a click opens its list under it.
fn field_row(app: &mut CantusApp, ui: &mut Ui, list: List, key: &str, value: &str) {
    let k = Style::current().k;
    ui.horizontal(|ui| {
        w::row_label(ui, key, LABEL_W, 24.0);
        let width = (ui.available_width() / k - 8.0).max(120.0);
        let open = app.settings_page.list.is_some_and(|(l, _)| l == list);
        let (resp, clicks) = menu_field(ui, value, false, open, &t(key), width);
        if clicks.open {
            app.settings_page.list = if open {
                None
            } else {
                Some((list, resp.rect.left_bottom()))
            };
        }
    });
}

/// A dim (or warning) line under the value column.
fn indented_note(ui: &mut Ui, s: &str, warn: bool) {
    ui.horizontal(|ui| {
        w::row_label(ui, "", LABEL_W, 18.0);
        note(ui, s, warn);
    });
}

fn device_band(app: &mut CantusApp, ui: &mut Ui, f: &Facts) {
    w::band(ui, Src::Key("devices.band.device"));
    let p = pending(app);
    let scan = scan(app);
    for (list, key) in [
        (List::Driver, "devices.driver"),
        (List::Device, "devices.output"),
        (List::Input, "devices.input"),
        (List::Rate, "settings.sample_rate"),
        (List::Buffer, "settings.buffer"),
    ] {
        let v = value(list, &p, &scan, f.rate);
        field_row(app, ui, list, key, &v);
        // On Windows, say why ASIO is missing when this build cannot offer it.
        if list == List::Driver
            && cfg!(windows)
            && !cantus_engine::devices::asio_built()
            && !scan
                .drivers
                .iter()
                .any(|d| d.name.eq_ignore_ascii_case("ASIO"))
        {
            indented_note(ui, &t("devices.driver.no_asio"), false);
        }
    }
    ui.horizontal(|ui| {
        w::row_label(ui, "", LABEL_W, 24.0);
        let changed = p != app.settings.audio;
        if w::button(ui, Src::Key("settings.apply"), 90.0, changed).clicked() {
            app.settings_page.pending = None;
            // Applied as the popover applies: remembered for its interface (round 20).
            crate::audio_setup::apply(app, p.clone());
            app.settings_page.audio.measure = Measure::Idle;
        }
        if w::button(ui, Src::Key("devices.rescan"), 0.0, true).clicked() {
            app.settings_page.audio.scan = Some(Scan::of(p.driver.as_deref()));
        }
    });
    // Round 20: the guided setup and the channels page, one click from here too.
    ui.horizontal(|ui| {
        w::row_label(ui, "", LABEL_W, 24.0);
        if w::button(ui, Src::Key("devices.pop.assistant"), 0.0, true).clicked() {
            app.show_settings = false;
            crate::audio_setup::open_first_run(app);
        }
        if w::button(ui, Src::Key("devices.pop.routing"), 0.0, true).clicked() {
            app.audio_setup.routing = true;
        }
    });
    let running = if f.live {
        format!(
            "{} · {}",
            f.driver,
            tf(
                "settings.audio_now",
                &[("device", &f.device), ("rate", &(f.rate as u32))],
            )
        )
    } else {
        t("status.no_audio")
    };
    indented_note(ui, &running, false);
    if let Some(e) = app.audio_error().map(str::to_string) {
        indented_note(ui, &e, true);
    }
    if f.stalled {
        indented_note(ui, &t("devices.stalled"), true);
    }
    if let Some(e) = &f.input_error {
        indented_note(ui, &tf("devices.input_error", &[("error", e)]), true);
    }
    if f.dropped > 0 {
        indented_note(ui, &tf("devices.dropped", &[("n", &f.dropped)]), true);
    }
}

fn latency_band(app: &mut CantusApp, ui: &mut Ui, f: &Facts) {
    let s = Style::current();
    let k = s.k;
    w::band(ui, Src::Key("devices.band.latency"));
    let lat = |frames: u32| {
        if f.live {
            tf(
                "devices.latency.value",
                &[("ms", &ms(frames as f64, f.rate)), ("frames", &frames)],
            )
        } else {
            t("devices.latency.none")
        }
    };
    let input = if f.channels > 0 {
        lat(f.latency.input)
    } else {
        t("devices.inputs.none")
    };
    for (key, v) in [
        ("devices.latency.input", input),
        ("devices.latency.output", lat(f.latency.output)),
        ("devices.latency.round_trip", lat(f.latency.round_trip())),
    ] {
        ui.horizontal(|ui| {
            w::row_label(ui, key, LABEL_W, 22.0);
            let width = (ui.available_width() / k - 8.0).clamp(120.0, 260.0);
            readout(ui, &v, false, &t(key), width);
        });
    }
    // The record offset: a knob, Measure beside it, and what the last measure found.
    let before = app.settings.audio.record_offset_frames;
    let mut v = before as f32;
    ui.horizontal(|ui| {
        w::row_label(ui, "devices.offset", LABEL_W, 70.0);
        let name = t("devices.offset");
        Param::new(&mut v, -OFFSET_RANGE..=OFFSET_RANGE, 0.0)
            .format(|v, b| {
                use std::fmt::Write as _;
                let _ = write!(b, "{:+}", v.round() as i64);
            })
            .size(KnobSize::S)
            .cell(Src::Key("devices.offset.cell"))
            .name(&name)
            .show(ui);
        ui.vertical(|ui| {
            ui.add_space(6.0 * k);
            let running = app.settings_page.audio.measure == Measure::Running;
            let label = if running {
                "devices.measuring"
            } else {
                "devices.measure"
            };
            if w::button(ui, Src::Key(label), 110.0, f.live && !running).clicked() {
                start_measure(app);
            }
            let frames = app.settings.audio.record_offset_frames;
            note(
                ui,
                &tf(
                    "devices.offset.value",
                    &[
                        ("frames", &format!("{frames:+}")),
                        ("ms", &ms(frames as f64, f.rate)),
                    ],
                ),
                false,
            );
        });
    });
    let now = v.round() as i64;
    if now != before {
        set_offset(app, now);
    }
    let (line, warn) = match &app.settings_page.audio.measure {
        Measure::Done(r) => (
            tf(
                "devices.measured",
                &[
                    ("ms", &ms(r.round_trip as f64, f.rate)),
                    ("channel", &(r.channel + 1)),
                    ("reported", &r.reported),
                    ("offset", &format!("{:+}", r.offset)),
                ],
            ),
            false,
        ),
        Measure::Failed(e) => (t(e.key()), true),
        _ => (t("devices.measure.hint"), false),
    };
    indented_note(ui, &line, warn);
    indented_note(ui, &t("devices.offset.hint"), false);
}

/// Store the offset and hand it to the engine at once (no restart needed).
fn set_offset(app: &mut CantusApp, frames: i64) {
    app.settings.audio.record_offset_frames = frames;
    if let Some(p) = app.settings_page.pending.as_mut() {
        p.record_offset_frames = frames;
    }
    app.engine.set_record_offset(frames);
    app.store_settings();
}

fn start_measure(app: &mut CantusApp) {
    if app.engine.is_playing() {
        app.stop();
    }
    app.settings_page.audio.measure = match app.engine.begin_loopback() {
        Ok(()) => Measure::Running,
        Err(e) => Measure::Failed(e),
    };
}

/// Fold a finished measurement in: its offset becomes the record offset.
fn poll_measure(app: &mut CantusApp, ctx: &egui::Context) {
    if app.settings_page.audio.measure != Measure::Running {
        return;
    }
    ctx.request_repaint();
    match app.engine.poll_loopback() {
        Some(Ok(r)) => {
            set_offset(app, r.offset);
            app.settings_page.audio.measure = Measure::Done(r);
        }
        Some(Err(e)) => app.settings_page.audio.measure = Measure::Failed(e),
        None if !app.engine.is_measuring() => app.settings_page.audio.measure = Measure::Idle,
        None => {}
    }
}

/// Meter width and height, in points, and the column each pair takes.
const METER: Vec2 = Vec2::new(12.0, 64.0);
const METER_COL: f32 = 40.0;

/// The open input's channels: a meter on each pair (FL's input list, 1–2, 3–4 …), the
/// numbers under them, the levels held a moment so a peak can be read.
fn inputs_band(app: &mut CantusApp, ui: &mut Ui, f: &Facts) {
    let s = Style::current();
    let k = s.k;
    w::band(ui, Src::Key("devices.band.inputs"));
    let Some(name) = f.input.as_deref().filter(|_| f.channels > 0) else {
        indented_note(ui, &t("devices.inputs.none"), false);
        return;
    };
    indented_note(
        ui,
        &tf(
            "devices.inputs.device",
            &[("name", &name), ("n", &f.channels)],
        ),
        false,
    );
    // Held peaks fall 20 dB a second (a frame is about 1/60 s).
    let hold = &mut app.settings_page.audio.hold;
    hold.resize(f.channels, MIN_DB);
    let db = |x: f32| {
        if x > 1e-6 {
            (20.0 * x.log10()).max(MIN_DB)
        } else {
            MIN_DB
        }
    };
    let levels: Vec<f32> = (0..f.channels)
        .map(|c| db(f.peaks.get(c).copied().unwrap_or(0.0)))
        .collect();
    for (h, l) in hold.iter_mut().zip(&levels) {
        *h = (*h - 20.0 / 60.0).max(*l).max(MIN_DB);
    }
    let hold = hold.clone();
    if f.live {
        ui.ctx().request_repaint();
    }
    let pairs = f.channels.div_ceil(2);
    // As many pairs to a row as fit beside the label column.
    let per_row = ((ui.available_width() / k - LABEL_W - 8.0) / METER_COL).max(1.0) as usize;
    for row in 0..pairs.div_ceil(per_row) {
        ui.horizontal(|ui| {
            w::row_label(ui, "", LABEL_W, METER.y + 20.0);
            for p in row * per_row..((row + 1) * per_row).min(pairs) {
                let (a, b) = (2 * p, (2 * p + 1).min(f.channels - 1));
                let lv = Levels {
                    level: [levels[a], levels[b]],
                    hold: [hold[a], hold[b]],
                };
                let (cell, resp) = ui
                    .allocate_exact_size(Vec2::new(METER_COL, METER.y + 20.0) * k, Sense::hover());
                let m = Rect::from_center_size(
                    Pos2::new(cell.center().x, cell.top() + METER.y * 0.5 * k),
                    METER * k,
                );
                let label = if a == b {
                    format!("{}", a + 1)
                } else {
                    format!("{}–{}", a + 1, b + 1)
                };
                with_brush(ui.painter(), |br| {
                    meter::paint(br, m, &s, &lv);
                    let lr = Rect::from_min_max(
                        Pos2::new(cell.left(), m.bottom() + 2.0 * k),
                        cell.right_bottom(),
                    );
                    let _ = text::paint(
                        br,
                        lr,
                        &Text::new(&label, Role::Label, s.t.text_dim)
                            .align(Align2::CENTER_CENTER)
                            .policy(Policy::Elide(Elide::End)),
                        s.tk(),
                    );
                });
                if resp.hovered() {
                    let tip = tf("devices.inputs.meter", &[("ch", &label)]);
                    let peak = lv.hold[0].max(lv.hold[1]);
                    crate::kit::hint::set_text(
                        ui.ctx(),
                        &tip,
                        &format!("{peak:.1} dB"),
                        crate::kit::hint::Caps::NONE,
                    );
                }
            }
        });
    }
}

/// The audio lists: drivers, devices, inputs, rates, block sizes.
pub fn lists(app: &mut CantusApp, ctx: &egui::Context) {
    use crate::kit::menu::{Check, Item};
    use crate::kit::widgets::menu_list;
    let Some((list, at)) = app.settings_page.list else {
        return;
    };
    let s = Style::current();
    let mut p = pending(app);
    let scan = scan(app);
    let default = t("settings.default");
    // Each row's label, and what choosing it sets.
    let mut rows: Vec<(String, bool)> = Vec::new();
    match list {
        List::Driver => {
            rows.push((default.clone(), p.driver.is_none()));
            for d in &scan.drivers {
                rows.push((d.name.clone(), p.driver.as_deref() == Some(&d.name)));
            }
        }
        List::Device => {
            rows.push((default.clone(), p.device.is_none()));
            for d in &scan.outputs {
                let l = if d.is_default {
                    format!("{} ({default})", d.name)
                } else {
                    d.name.clone()
                };
                rows.push((l, p.device.as_deref() == Some(&d.name)));
            }
        }
        List::Input => {
            rows.push((default.clone(), p.input_device.is_none()));
            rows.push((
                t("devices.input.none"),
                p.input_device.as_deref() == Some(""),
            ));
            for d in &scan.inputs {
                let mut l = d.name.clone();
                if d.channels > 0 {
                    l += &format!(" · {}", tf("devices.channels", &[("n", &d.channels)]));
                }
                if d.is_default {
                    l += &format!(" ({default})");
                }
                rows.push((l, p.input_device.as_deref() == Some(&d.name)));
            }
        }
        List::Rate => {
            rows.push((default.clone(), p.sample_rate.is_none()));
            for r in SAMPLE_RATES {
                rows.push((format!("{r} Hz"), p.sample_rate == Some(r)));
            }
        }
        List::Buffer => {
            rows.push((default.clone(), p.buffer_frames.is_none()));
            for b in BUFFER_SIZES {
                rows.push((b.to_string(), p.buffer_frames == Some(b)));
            }
        }
    }
    let items: Vec<Item<'_>> = rows
        .iter()
        .map(|(l, on)| Item::Action {
            label: Src::Str(l),
            check: Check::Radio(*on),
            shortcut: None,
            submenu: false,
            enabled: true,
        })
        .collect();
    let mut chosen = None;
    let area = egui::Area::new(egui::Id::new("cantus-settings-list"))
        .order(egui::Order::Foreground)
        .fixed_pos(at)
        .constrain(true)
        .show(ctx, |ui| {
            let (_, c) = menu_list(ui, &items, 320.0 * s.k);
            chosen = c.clicked;
        });
    if let Some(i) = chosen {
        match list {
            List::Driver => {
                let next = i
                    .checked_sub(1)
                    .and_then(|i| scan.drivers.get(i))
                    .map(|d| d.name.clone());
                if next != p.driver {
                    // Device names belong to a driver: start from its defaults.
                    p.driver = next;
                    p.device = None;
                    p.input_device = None;
                }
            }
            List::Device => {
                p.device = i
                    .checked_sub(1)
                    .and_then(|i| scan.outputs.get(i))
                    .map(|d| d.name.clone());
            }
            List::Input => {
                p.input_device = match i {
                    0 => None,
                    1 => Some(String::new()),
                    i => scan.inputs.get(i - 2).map(|d| d.name.clone()),
                };
            }
            List::Rate => p.sample_rate = i.checked_sub(1).map(|i| SAMPLE_RATES[i]),
            List::Buffer => p.buffer_frames = i.checked_sub(1).map(|i| BUFFER_SIZES[i]),
        }
        app.settings_page.pending = Some(p);
    }
    let outside = ctx.input(|i| i.pointer.any_pressed())
        && !ctx
            .pointer_interact_pos()
            .is_some_and(|p| area.response.rect.contains(p));
    if chosen.is_some() || outside || ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
        app.settings_page.list = None;
    }
}

/// Fill the page with a plausible interface for the screenshot example: two drivers, an
/// eight-input interface with signal on its first pair, latencies, a measured offset.
pub fn demo_setup(app: &mut CantusApp) {
    let iface = "USB Audio Interface";
    app.settings.audio.driver = Some("ASIO".into());
    app.settings.audio.device = Some(iface.into());
    app.settings.audio.input_device = None;
    app.settings.audio.sample_rate = Some(48_000);
    app.settings.audio.buffer_frames = Some(256);
    app.settings.audio.record_offset_frames = 38;
    let st = &mut app.settings_page.audio;
    st.scan = Some(Scan {
        driver: Some("ASIO".into()),
        drivers: vec![
            DriverInfo {
                name: "WASAPI".into(),
                is_default: true,
            },
            DriverInfo {
                name: "ASIO".into(),
                is_default: false,
            },
        ],
        outputs: vec![DeviceInfo {
            name: iface.into(),
            is_default: true,
        }],
        inputs: vec![InputDeviceInfo {
            name: iface.into(),
            channels: 8,
            is_default: true,
        }],
    });
    st.demo = Some(Facts {
        driver: "ASIO".into(),
        live: true,
        device: iface.into(),
        rate: 48_000.0,
        input: Some(iface.into()),
        channels: 8,
        peaks: vec![0.42, 0.37, 0.05, 0.0, 0.9, 0.0, 0.0, 0.0],
        latency: DeviceLatency {
            input: 297,
            output: 329,
        },
        dropped: 0,
        input_error: None,
        stalled: false,
    });
    st.hold = vec![-6.0, -7.5, -24.0, MIN_DB, -0.6, MIN_DB, MIN_DB, MIN_DB];
    st.measure = Measure::Done(LoopbackResult {
        round_trip: 664,
        reported: 626,
        queue: 0,
        offset: 38,
        channel: 1,
        confidence: 92.0,
        bursts: 8,
    });
}
