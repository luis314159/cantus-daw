//! The audio popover (round 20): one click on the chip, and everything about the device
//! is here, changed in place — the interface (with its picture), the driver, the input,
//! the rate, and the buffer, each buffer size showing the round trip it would give
//! *before* it is chosen. What is wrong comes first, in words, with the button that fixes
//! it. Then the test tone, the channels and routing, the setup assistant, and Settings.
//!
//! Every choice applies at once (`audio_setup::apply`): the device reopens with it and
//! the choice is remembered for that interface.

use super::audio_parts::{self as parts, choice_key};
use super::workspace_widgets as w;
use crate::app::CantusApp;
use crate::audio_setup::problems::{self, Fix};
use crate::audio_setup::{self as setup, interfaces, latency, Hit, Live, PopList};
use crate::i18n::{t, tf};
use crate::kit::glyph::{self, Glyph};
use crate::kit::text::{self, Elide, Policy, Role, Text};
use crate::kit::widgets::menu_field;
use crate::kit::{led, segmented, with_brush, Fill, Style, Tok};
use egui::{Align2, Id, Pos2, Rect, Sense, Ui, Vec2};

/// The popover's width at 100 %.
pub const WIDTH: f32 = 412.0;
/// The label column.
const LABEL_W: f32 = 74.0;
/// The buffer sizes offered, after *Auto*.
pub const BUFFERS: [u32; 6] = [64, 128, 256, 512, 1024, 2048];
/// The rates offered, after *Auto*.
const RATES: [u32; 4] = [44_100, 48_000, 88_200, 96_000];

/// The driver names on offer: the system's own, and ASIO when this build has it and an
/// ASIO driver is installed (the interface's own, or a universal one: FlexASIO, ASIO4ALL).
fn drivers(app: &CantusApp, _key: Option<&str>) -> Vec<String> {
    let scan = app.audio_setup.scan.as_ref();
    let system = scan
        .and_then(|s| s.drivers.iter().find(|d| d.is_default))
        .map_or_else(|| "WASAPI".to_string(), |d| d.name.clone());
    let mut v = vec![system];
    let asio = scan.is_some_and(|s| s.has_driver("ASIO") && !s.asio.is_empty());
    if asio || app.settings.audio.driver.as_deref() == Some("ASIO") {
        v.push("ASIO".into());
    }
    v
}

/// The driver in use, as the segmented control shows it.
fn driver_now(app: &CantusApp, live: &Live) -> String {
    match app.settings.audio.driver.as_deref() {
        Some(d) => d.to_string(),
        None if live.live => live.driver.clone(),
        None => "WASAPI".into(),
    }
}

/// Choose a driver for the interface in use.
fn set_driver(app: &mut CantusApp, key: Option<&str>, name: &str) {
    let iface = key.and_then(|k| app.audio_setup.scan.as_ref()?.interface(k).cloned());
    let mut a = app.settings.audio.clone();
    if name.eq_ignore_ascii_case("ASIO") {
        a.driver = Some("ASIO".into());
        // The interface's own ASIO driver, else the first installed (FlexASIO, ASIO4ALL).
        let first = app
            .audio_setup
            .scan
            .as_ref()
            .and_then(|s| s.asio.first().cloned());
        a.device = iface
            .as_ref()
            .and_then(|i| i.asio.clone())
            .or(first)
            .or(a.device);
        a.input_device = None;
    } else {
        a.driver = None;
        if let Some(i) = &iface {
            a.device = i.outputs.first().cloned();
            a.input_device = Some(i.inputs.first().cloned().unwrap_or_default());
        } else {
            a.device = None;
            a.input_device = None;
        }
    }
    setup::apply(app, a);
}

/// The popover, when open: anchored above the chip, right-aligned with it.
pub fn show(app: &mut CantusApp, ctx: &egui::Context) {
    if !app.audio_setup.popover {
        return;
    }
    let s = Style::current();
    let k = s.k;
    let screen = ctx.content_rect();
    let chip = app.audio_setup.geo.rect(&Hit::Chip);
    let anchor = chip.map_or(
        Pos2::new(screen.right() - 12.0, screen.bottom() - 30.0),
        |c| Pos2::new(c.right(), c.top() - 6.0 * k),
    );
    let live = setup::live(app, false);
    let key = setup::current_key(app);
    let own = setup::own_input(app);
    let n = app.audio_setup.dropouts.within(
        std::time::Duration::from_secs(30),
        std::time::Instant::now(),
    );
    let found = problems::diagnose(
        &live,
        app.audio_setup.plug.lost.as_deref(),
        own.as_deref(),
        n,
    );
    let area = egui::Area::new(Id::new("cantus-audio-popover"))
        .order(egui::Order::Foreground)
        .pivot(Align2::RIGHT_BOTTOM)
        .fixed_pos(anchor)
        .constrain(true)
        .show(ctx, |ui| {
            let h = app.audio_setup.pop_h.max(200.0 * k);
            let rect = Rect::from_min_size(ui.max_rect().min, Vec2::new(WIDTH * k, h));
            ui.set_min_size(rect.size());
            parts::panel(ui, rect, &s);
            let inner = rect.shrink2(Vec2::new(14.0 * k, 12.0 * k));
            let mut child = ui.new_child(egui::UiBuilder::new().max_rect(inner));
            child.spacing_mut().item_spacing = Vec2::new(6.0 * k, 7.0 * k);
            header(app, &mut child, &live, key.as_deref());
            for (i, (p, fixes)) in found.iter().enumerate() {
                problem(app, &mut child, i, p, fixes);
            }
            rows(app, &mut child, &live, key.as_deref());
            buffers(app, &mut child, &live);
            footer(app, &mut child);
            let used = child.min_rect().bottom() - rect.top() + 12.0 * k;
            if (used - app.audio_setup.pop_h).abs() > 0.5 {
                app.audio_setup.pop_h = used;
                ui.ctx().request_repaint();
            }
        });
    super::audio_popover_lists::show(app, ctx, key.as_deref());
    // A click outside (not on the chip, not in a list) closes it, as Escape does.
    let pop = area.response.rect;
    let list = app.audio_setup.geo.rect(&Hit::List);
    let pressed = ctx.input(|i| i.pointer.any_pressed());
    let at = ctx.input(|i| i.pointer.interact_pos());
    let inside = at.is_some_and(|p| {
        pop.contains(p)
            || chip.is_some_and(|c| c.contains(p))
            || list.is_some_and(|l| l.contains(p))
    });
    if (pressed && !inside) || ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
        app.audio_setup.popover = false;
        app.audio_setup.pop_list = None;
    }
}

/// The interface's picture, its name, what it runs at, and how it is.
fn header(app: &mut CantusApp, ui: &mut Ui, live: &Live, key: Option<&str>) {
    let s = Style::current();
    let k = s.k;
    let (rect, _) =
        ui.allocate_exact_size(Vec2::new(ui.available_width(), 50.0 * k), Sense::hover());
    let kind = key.map_or(interfaces::Kind::BuiltIn, interfaces::kind_of);
    let name = key.map_or_else(|| t("devices.chip.none"), str::to_string);
    let health = super::audio_chip::health(app);
    let mut sub = String::new();
    if live.live {
        sub = tf(
            "devices.pop.running",
            &[
                ("driver", &live.driver),
                ("rate", &latency::short_rate(live.rate as u32)),
                ("buffer", &live.block),
                ("ins", &live.in_channels),
                ("outs", &live.out_channels),
            ],
        );
    }
    let state = match health {
        super::audio_chip::Health::Good => t("devices.state.good"),
        super::audio_chip::Health::Warn => t("devices.state.warn"),
        super::audio_chip::Health::Lost => t("devices.state.lost"),
    };
    with_brush(ui.painter(), |b| {
        let pic = Rect::from_min_size(rect.min, Vec2::new(84.0 * k, rect.height()));
        b.rrect(
            pic,
            s.r(5.0),
            Fill::V(crate::kit::color::shade(s.t.well, 0.02), s.t.well),
        );
        super::audio_art::paint(
            b,
            pic.shrink(6.0 * k),
            &s,
            kind,
            health != super::audio_chip::Health::Lost,
        );
        let x = pic.right() + 12.0 * k;
        let state_w = 96.0 * k;
        let title = Rect::from_min_max(
            Pos2::new(x, rect.top() + 4.0 * k),
            Pos2::new(rect.right() - state_w, rect.center().y + 2.0 * k),
        );
        let _ = text::paint(
            b,
            title,
            &Text::new(&name, Role::Title, s.t.text)
                .align(Align2::LEFT_CENTER)
                .policy(Policy::Elide(Elide::Middle)),
            s.tk(),
        );
        let line = Rect::from_min_max(
            Pos2::new(x, rect.center().y + 2.0 * k),
            Pos2::new(rect.right(), rect.bottom() - 4.0 * k),
        );
        let _ = text::paint(
            b,
            line,
            &Text::new(&sub, Role::Label, s.t.text_dim)
                .align(Align2::LEFT_CENTER)
                .policy(Policy::Elide(Elide::End)),
            s.tk(),
        );
        let sr = Rect::from_min_max(
            Pos2::new(rect.right() - state_w, title.top()),
            Pos2::new(rect.right(), title.bottom()),
        );
        if let Some(p) = text::paint(
            b,
            sr,
            &Text::new(&state, Role::Label, s.t.text)
                .align(Align2::RIGHT_CENTER)
                .policy(Policy::Elide(Elide::End)),
            s.tk(),
        ) {
            led::paint(
                b,
                Pos2::new(p.rect.left() - 9.0 * k, sr.center().y),
                9.0 * k,
                true,
                &s,
                health.tok(),
                Tok::Panel,
                1.0,
            );
        }
    });
}

/// A problem: the warning glyph, its sentence, its fix buttons.
fn problem(app: &mut CantusApp, ui: &mut Ui, i: usize, p: &problems::Problem, fixes: &[Fix]) {
    let s = Style::current();
    let k = s.k;
    let text = p.text();
    let wdt = ui.available_width();
    let g = parts::wrapped(ui.ctx(), &text, Role::Body, s.tk(), wdt - 40.0 * k, 4);
    let h = g.size().y + 16.0 * k + if fixes.is_empty() { 0.0 } else { 30.0 * k };
    let (rect, _) = ui.allocate_exact_size(Vec2::new(wdt, h), Sense::hover());
    let tone = if p.is_silent() {
        Tok::Danger
    } else {
        Tok::MeterHigh
    };
    with_brush(ui.painter(), |b| {
        b.register(
            rect,
            "audio-problem",
            &[Tok::Well, Tok::Text],
            &[Some(tone)],
        );
        b.rrect(
            rect,
            s.r(5.0),
            Fill::Solid(crate::kit::color::mix(s.t.well, s.t.get(tone), 0.10)),
        );
        b.rrect_stroke(
            rect,
            s.r(5.0),
            b.px(),
            crate::kit::color::with_alpha(s.t.get(tone), 0.6),
        );
        let edge = Rect::from_min_size(rect.min, Vec2::new(3.0 * k, rect.height()))
            .shrink2(Vec2::new(0.0, 5.0 * k));
        b.rrect(edge, 1.5 * k, Fill::Solid(s.t.get(tone)));
        let gr = Rect::from_min_size(
            rect.min + Vec2::new(12.0 * k, 9.0 * k),
            Vec2::splat(14.0 * k),
        );
        glyph::paint(b, gr, Glyph::Warning, s.t.get(tone));
        b.text(
            Pos2::new(b.snap(rect.left() + 34.0 * k), b.snap(rect.top() + 8.0 * k)),
            g,
            s.t.text,
            0.0,
        );
    });
    let mut x = rect.left() + 34.0 * k;
    for (j, f) in fixes.iter().enumerate() {
        let label = f.label();
        let tw = text::measure(
            ui.ctx(),
            &Text::new(&label, Role::Control, s.t.text),
            s.tk(),
        )
        .x + 24.0 * k;
        let r = Rect::from_min_size(
            Pos2::new(x, rect.bottom() - 30.0 * k),
            Vec2::new(tw.max(72.0 * k), 22.0 * k),
        );
        x = r.right() + 6.0 * k;
        let resp = ui.interact(r, Id::new(("audio-fix", i, j)), Sense::click());
        parts::paint_button(ui, r, &resp, &label, j == 0, true);
        app.audio_setup.hit(ui.ctx(), Hit::Fix(i, j), r);
        if resp.clicked() {
            setup::fix(app, f);
        }
    }
}

/// A label | value row: the label right-aligned in the column, then `body`.
fn row(ui: &mut Ui, key: &str, height: f32, body: impl FnOnce(&mut Ui)) {
    ui.horizontal(|ui| {
        w::row_label(ui, key, LABEL_W, height);
        body(ui);
    });
}

/// Interface, driver, input, rate.
fn rows(app: &mut CantusApp, ui: &mut Ui, live: &Live, key: Option<&str>) {
    let s = Style::current();
    let k = s.k;
    let value_w = (ui.available_width() / k - LABEL_W - 8.0).max(160.0);
    // The interface.
    let name = match key {
        Some(k) => k.to_string(),
        None => t("settings.default"),
    };
    let open = app.audio_setup.pop_list == Some(PopList::Interface);
    let mut field = None;
    row(ui, "devices.pop.interface", 24.0, |ui| {
        let (resp, c) = menu_field(ui, &name, false, open, &t("devices.pop.interface"), value_w);
        field = Some((resp.rect, c.open));
    });
    if let Some((r, clicked)) = field {
        app.audio_setup.hit(ui.ctx(), Hit::InterfaceField, r);
        app.audio_setup.list_at = r.left_bottom();
        if clicked {
            app.audio_setup.pop_list = if open { None } else { Some(PopList::Interface) };
            app.audio_setup.request_scan();
        }
    }
    // The driver.
    let names = drivers(app, key);
    let now = driver_now(app, live);
    let mut sel = names
        .iter()
        .position(|d| d.eq_ignore_ascii_case(&now))
        .unwrap_or(0);
    let before = sel;
    let mut seg = Rect::NOTHING;
    let (mut get, mut open_drivers) = (None, false);
    row(ui, "devices.driver", 24.0, |ui| {
        let labels: Vec<text::Src<'_>> = names.iter().map(|n| text::Src::Str(n)).collect();
        let w = (90.0 * names.len() as f32).min(value_w);
        seg = crate::kit::widgets::segmented(ui, &mut sel, &labels, &t("devices.driver"), w).rect;
        // Beside it, always: how to get a low-latency (ASIO) driver.
        let r = parts::button(ui, &t("devices.drivers.get_short"), false, true, 0.0);
        get = Some(r.rect);
        if r.clicked() {
            open_drivers = true;
        }
    });
    if let Some(r) = get {
        app.audio_setup.hit(ui.ctx(), Hit::GetDriver, r);
    }
    if open_drivers {
        app.audio_setup.drivers = true;
        app.audio_setup.request_scan();
    }
    for (i, n) in names.iter().enumerate() {
        app.audio_setup.hit(
            ui.ctx(),
            Hit::Driver(n.clone()),
            segmented::segment(seg, names.len(), i),
        );
    }
    if sel != before {
        set_driver(app, key, &names[sel]);
    }
    // On ASIO: which ASIO driver (they are listed by name; inputs are the driver's own).
    let asio = now.eq_ignore_ascii_case("ASIO");
    if asio {
        let name = app
            .settings
            .audio
            .device
            .clone()
            .unwrap_or_else(|| live.device.clone());
        let open = app.audio_setup.pop_list == Some(PopList::Asio);
        let mut field = None;
        row(ui, "devices.pop.asio_driver", 24.0, |ui| {
            let (resp, c) = menu_field(
                ui,
                &name,
                false,
                open,
                &t("devices.pop.asio_driver"),
                value_w,
            );
            field = Some((resp.rect, c.open));
        });
        if let Some((r, clicked)) = field {
            app.audio_setup.hit(ui.ctx(), Hit::AsioField, r);
            if clicked {
                app.audio_setup.list_at = r.left_bottom();
                app.audio_setup.pop_list = if open { None } else { Some(PopList::Asio) };
                app.audio_setup.request_scan();
            }
        }
    }
    // The input.
    let input = match app.settings.audio.input_device.as_deref() {
        _ if asio => tf("devices.pop.asio_input", &[("n", &live.in_channels)]),
        Some("") => t("devices.input.none"),
        Some(n) => n.to_string(),
        None => live.input.clone().unwrap_or_else(|| t("settings.default")),
    };
    let open = app.audio_setup.pop_list == Some(PopList::Input);
    let mut field = None;
    row(ui, "devices.input", 24.0, |ui| {
        let (resp, c) = menu_field(ui, &input, false, open, &t("devices.input"), value_w);
        field = Some((resp.rect, c.open));
    });
    if let Some((r, clicked)) = field {
        app.audio_setup.hit(ui.ctx(), Hit::InputField, r);
        if clicked && !asio {
            app.audio_setup.list_at = r.left_bottom();
            app.audio_setup.pop_list = if open { None } else { Some(PopList::Input) };
            app.audio_setup.request_scan();
        }
    }
    // The rate.
    let rates: Vec<String> = std::iter::once(t("devices.auto"))
        .chain(RATES.iter().map(|r| problems::khz(*r)))
        .collect();
    let chosen = app.settings.audio.sample_rate;
    let mut sel = chosen
        .and_then(|r| RATES.iter().position(|x| *x == r).map(|i| i + 1))
        .unwrap_or(0);
    let before = sel;
    let mut seg = Rect::NOTHING;
    row(ui, "devices.pop.rate", 24.0, |ui| {
        let labels: Vec<text::Src<'_>> = rates.iter().map(|n| text::Src::Str(n)).collect();
        seg =
            crate::kit::widgets::segmented(ui, &mut sel, &labels, &t("devices.pop.rate"), value_w)
                .rect;
    });
    for i in 0..rates.len() {
        let r = if i == 0 { None } else { Some(RATES[i - 1]) };
        app.audio_setup.hit(
            ui.ctx(),
            Hit::Rate(r),
            segmented::segment(seg, rates.len(), i),
        );
    }
    if sel != before {
        let mut a = app.settings.audio.clone();
        a.sample_rate = if sel == 0 { None } else { Some(RATES[sel - 1]) };
        setup::apply(app, a);
    }
}

/// The buffer: a key per size, each with the round trip it would give.
fn buffers(app: &mut CantusApp, ui: &mut Ui, live: &Live) {
    let s = Style::current();
    let k = s.k;
    w::band(ui, text::Src::Key("devices.pop.buffer"));
    let wdt = ui.available_width();
    let n = BUFFERS.len();
    let gap = 5.0 * k;
    let kw = (wdt - gap * (n as f32 - 1.0)) / n as f32;
    let (rect, _) = ui.allocate_exact_size(Vec2::new(wdt, 42.0 * k), Sense::hover());
    let chosen = app.settings.audio.buffer_frames.unwrap_or(live.block);
    let mut pick = None;
    for (i, b) in BUFFERS.iter().enumerate() {
        let r = Rect::from_min_size(
            Pos2::new(rect.left() + i as f32 * (kw + gap), rect.top()),
            Vec2::new(kw, rect.height()),
        );
        let ms = setup::estimate_ms(live, *b);
        let mut cap = String::from("≈ ");
        latency::fmt_ms(ms, &mut cap);
        cap.push_str(" ms");
        let dot = match latency::feel(ms) {
            latency::Feel::Tight => Tok::Led,
            latency::Feel::Fine => Tok::MeterHigh,
            latency::Feel::Loose => Tok::Danger,
        };
        let resp = choice_key(
            ui,
            r,
            Id::new(("audio-buffer", *b)),
            &b.to_string(),
            &cap,
            Some(dot),
            chosen == *b,
        );
        app.audio_setup.hit(ui.ctx(), Hit::Buffer(*b), r);
        if resp.hovered() {
            crate::kit::hint::set_text(
                ui.ctx(),
                &tf("devices.pop.buffer_tip", &[("n", b), ("ms", &cap)]),
                "",
                crate::kit::hint::Caps::NONE,
            );
        }
        if resp.clicked() && chosen != *b {
            pick = Some(*b);
        }
    }
    // The latencies now, measured.
    let mut line = String::new();
    if live.live && live.latency.round_trip() > 0 {
        let rate = live.rate.max(1.0);
        let f = |x: u32| {
            let mut s = String::new();
            latency::fmt_ms(x as f64 / rate * 1000.0, &mut s);
            s
        };
        line = tf(
            "devices.pop.measured",
            &[
                ("in", &f(live.latency.input)),
                ("out", &f(live.latency.output)),
                ("rt", &f(live.latency.round_trip())),
            ],
        );
    }
    if !line.is_empty() {
        super::workspace_settings::note(ui, &line, false);
    }
    if let Some(b) = pick {
        let mut a = app.settings.audio.clone();
        a.buffer_frames = Some(b);
        setup::apply(app, a);
    }
}

/// The test tone, routing, the setup assistant, Settings.
fn footer(app: &mut CantusApp, ui: &mut Ui) {
    let k = Style::current().k;
    ui.add_space(2.0 * k);
    ui.horizontal(|ui| {
        let tone_on = app.audio_setup.tone.is_some();
        let label = if tone_on {
            t("devices.tone.stop")
        } else {
            t("devices.tone.play")
        };
        let r = parts::button(ui, &label, tone_on, true, 84.0);
        app.audio_setup.hit(ui.ctx(), Hit::Tone, r.rect);
        if r.clicked() {
            let main = setup::current_key(app)
                .and_then(|k| app.settings.audio_setup.profile(&k).map(|p| p.main))
                .unwrap_or(0);
            setup::set_tone(app, if tone_on { None } else { Some(main) });
        }
        let r = parts::button(ui, &t("devices.pop.routing"), false, true, 0.0);
        app.audio_setup.hit(ui.ctx(), Hit::Routing, r.rect);
        if r.clicked() {
            app.audio_setup.routing = true;
            app.audio_setup.popover = false;
        }
        let r = parts::button(ui, &t("devices.pop.assistant"), false, true, 0.0);
        app.audio_setup.hit(ui.ctx(), Hit::Assistant, r.rect);
        if r.clicked() {
            setup::open_first_run(app);
        }
        let r = w::glyph_key(ui, Glyph::Gear, false, 24.0, &t("devices.pop.settings"));
        app.audio_setup.hit(ui.ctx(), Hit::Settings, r.rect);
        if r.clicked() {
            app.show_settings = true;
            app.audio_setup.popover = false;
        }
    });
}
