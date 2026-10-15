//! The audio chip (round 20): always in the status bar, it says what the sound is going
//! through — `UMC204HD · ASIO · 48 k · 128 · 6.1 ms` — with a lamp that is green while
//! all is well, amber for a few seconds after a dropout (or while something needs
//! attention), and red when the device is lost or nothing could be opened. One click
//! opens the popover (`audio_popover`) to change any of it in place.
//!
//! Its text is rebuilt only when what it says changes, so a bar at rest allocates nothing.

use crate::app::CantusApp;
use crate::audio_setup::{self, interfaces, latency, Hit};
use crate::i18n::{t, tf, Locale};
use crate::kit::color::shade;
use crate::kit::glyph::{self, Glyph};
use crate::kit::hint::{self, Caps};
use crate::kit::light::{self, RaisedOpts};
use crate::kit::text::{self, Elide, Policy, Role, Text};
use crate::kit::{led, with_brush, Fill, State, Style, Tok};
use egui::{Align2, Pos2, Rect, Sense, Ui, Vec2};
use std::cell::RefCell;
use std::time::{Duration, Instant};

/// How the audio is, as the lamp shows it.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum Health {
    /// Running, nothing wrong.
    Good,
    /// Running, but it broke up lately or something needs a look.
    Warn,
    /// No sound: the device is lost or none is open.
    Lost,
    /// No sound: the driver stopped responding (or asked to be restarted) while the device
    /// is still there — not "disconnected" (round 21).
    Stalled,
}

impl Health {
    pub fn tok(self) -> Tok {
        match self {
            Health::Good => Tok::Led,
            Health::Warn => Tok::MeterHigh,
            Health::Lost | Health::Stalled => Tok::Danger,
        }
    }
}

/// The lamp's state now. Reads atomics and counters; allocates nothing.
pub fn health(app: &CantusApp) -> Health {
    let now = Instant::now();
    let (live, state, input_bad) = match &app.audio_setup.demo {
        Some(d) => (d.live, d.device_state(), d.input_error.is_some()),
        None => (
            app.engine.is_live(),
            app.engine.device_state(),
            app.engine.input_error().is_some(),
        ),
    };
    use cantus_engine::DeviceState as S;
    if !live || state == S::Lost || app.audio_setup.plug.lost.is_some() {
        Health::Lost
    } else if matches!(state, S::Stalled | S::RestartRequested) {
        Health::Stalled
    } else if input_bad || app.audio_setup.dropouts.within(Duration::from_secs(5), now) > 0 {
        Health::Warn
    } else {
        Health::Good
    }
}

/// What the chip's text depends on.
#[derive(Clone, Debug, Default, PartialEq)]
struct Key {
    device: String,
    driver: String,
    rate: u32,
    block: u32,
    lat10: u32,
    lost: bool,
    stalled: bool,
    live: bool,
    locale: Option<Locale>,
    tk: u32,
}

#[derive(Default)]
struct Cache {
    key: Key,
    text: String,
    width: f32,
}

thread_local! {
    static CACHE: RefCell<Cache> = RefCell::new(Cache::default());
}

/// The numbers the chip prints, without allocating unless they changed.
fn refresh(app: &CantusApp, ctx: &egui::Context, s: &Style) {
    let (live, device, driver, rate, block, rt, state) = match &app.audio_setup.demo {
        Some(d) => (
            d.live,
            d.device.as_str(),
            d.driver.as_str(),
            d.rate,
            d.block,
            d.latency.round_trip(),
            d.device_state(),
        ),
        None => {
            let e = &app.engine;
            // What the driver runs, read back (an ASIO driver may dictate it).
            let block = e
                .device_buffer()
                .or(match e.stats().block_frames {
                    0 => None,
                    b => Some(b as u32),
                })
                .unwrap_or(0);
            (
                e.is_live(),
                e.device_name.as_str(),
                e.driver_name(),
                e.sample_rate,
                block,
                e.device_latency().round_trip(),
                e.device_state(),
            )
        }
    };
    use cantus_engine::DeviceState as S;
    let lost = state == S::Lost || app.audio_setup.plug.lost.is_some();
    let stalled = !lost && matches!(state, S::Stalled | S::RestartRequested);
    let ms = if rate > 0.0 {
        rt as f64 / rate * 1000.0
    } else {
        0.0
    };
    CACHE.with(|c| {
        let mut c = c.borrow_mut();
        let k = &c.key;
        let same = k.device == device
            && k.driver == driver
            && k.rate == rate as u32
            && k.block == block
            && k.lat10 == (ms * 10.0).round() as u32
            && k.lost == lost
            && k.stalled == stalled
            && k.live == live
            && k.locale == Some(crate::i18n::locale())
            && k.tk == s.tk().to_bits();
        if same {
            return;
        }
        let name = app
            .audio_setup
            .scan
            .as_ref()
            .and_then(|sc| interfaces::current(&sc.interfaces, &app.settings.audio, device))
            .map(|i| i.key.clone())
            .unwrap_or_else(|| interfaces::adapter_of(device));
        let text = if !live {
            t("devices.chip.none")
        } else if lost {
            tf("devices.chip.lost", &[("name", &name)])
        } else if stalled {
            tf("devices.chip.stalled", &[("name", &name)])
        } else {
            let mut lat = String::new();
            if rt > 0 {
                latency::fmt_ms(ms, &mut lat);
                lat.push_str(" ms");
            } else {
                lat.push('–');
            }
            tf(
                "devices.chip.text",
                &[
                    ("name", &name),
                    ("driver", &driver),
                    ("rate", &latency::short_rate(rate as u32)),
                    ("buffer", &block),
                    ("latency", &lat),
                ],
            )
        };
        let tw = text::measure(ctx, &Text::new(&text, Role::Label, s.t.text), s.tk()).x;
        c.width = (tw + 44.0 * s.k).min(380.0 * s.k);
        c.text = text;
        c.key = Key {
            device: device.to_string(),
            driver: driver.to_string(),
            rate: rate as u32,
            block,
            lat10: (ms * 10.0).round() as u32,
            lost,
            stalled,
            live,
            locale: Some(crate::i18n::locale()),
            tk: s.tk().to_bits(),
        };
    });
}

/// The chip's width at the current scale (the status bar makes room for it).
pub fn width(app: &CantusApp, ctx: &egui::Context) -> f32 {
    let s = Style::current();
    refresh(app, ctx, &s);
    CACHE.with(|c| c.borrow().width)
}

/// Whether the chip reads a running device's line (name · driver · rate · buffer ·
/// latency), which may be shortened in its middle (the whole line is in its hint) — not a
/// sentence (no audio, lost, stalled), which is read whole.
pub fn reads_a_device(app: &CantusApp, ctx: &egui::Context) -> bool {
    let s = Style::current();
    refresh(app, ctx, &s);
    CACHE.with(|c| {
        let k = &c.borrow().key;
        k.live && !k.lost && !k.stalled
    })
}

/// Draw the chip in `rect`; a click opens or closes the popover.
pub fn show(app: &mut CantusApp, ui: &mut Ui, rect: Rect) {
    let s = Style::current();
    let ctx = ui.ctx().clone();
    refresh(app, &ctx, &s);
    let resp = ui.interact(rect, ui.id().with("audio-chip"), Sense::click());
    app.audio_setup.hit(&ctx, Hit::Chip, rect);
    if resp.clicked() {
        audio_setup::toggle_popover(app);
    }
    let h = health(app);
    let open = app.audio_setup.popover;
    if resp.hovered() {
        let tip = t("devices.chip.tip");
        CACHE.with(|c| {
            hint::set_text(
                &ctx,
                &tip,
                &c.borrow().text,
                Caps {
                    menu: true,
                    ..Caps::NONE
                },
            )
        });
    }
    let st = State::of(&resp, open);
    with_brush(ui.painter(), |b| {
        b.register(
            rect,
            "audio-chip",
            &[Tok::Panel, Tok::Text, Tok::TextDim],
            &[Some(h.tok())],
        );
        let base = if st.lit {
            shade(s.t.panel_raised, 0.07)
        } else if st.hovered {
            shade(s.t.panel_raised, 0.04)
        } else {
            shade(s.t.panel, 0.03)
        };
        let r = s.r(4.0);
        light::raised(
            b,
            rect,
            r,
            base,
            RaisedOpts {
                drop: false,
                k: s.k,
                depth: s.t.depth * 0.7,
                ..Default::default()
            },
        );
        if h == Health::Lost {
            // A red edge at the left: seen from the corner of the eye.
            let edge = Rect::from_min_size(rect.min, Vec2::new(s.s(3.0), rect.height()));
            b.rrect(
                edge.shrink2(Vec2::new(0.0, s.s(3.0))),
                s.s(1.5),
                Fill::Solid(s.t.danger),
            );
        }
        let lamp = Pos2::new(rect.left() + s.s(12.0), rect.center().y);
        led::paint(b, lamp, s.s(8.0), true, &s, h.tok(), Tok::Panel, 1.0);
        let tr = Rect::from_min_max(
            Pos2::new(rect.left() + s.s(22.0), rect.top()),
            Pos2::new(rect.right() - s.s(18.0), rect.bottom()),
        );
        let ink = if h == Health::Lost {
            s.t.danger
        } else {
            s.t.text
        };
        CACHE.with(|c| {
            let c = c.borrow();
            let _ = text::paint(
                b,
                tr,
                &Text::new(&c.text, Role::Label, ink)
                    .align(Align2::LEFT_CENTER)
                    .policy(Policy::Elide(Elide::Middle)),
                s.tk(),
            );
        });
        let gr = Rect::from_center_size(
            Pos2::new(rect.right() - s.s(10.0), rect.center().y),
            Vec2::splat(s.s(9.0)),
        );
        glyph::paint(b, gr, Glyph::Down, s.t.text_dim);
    });
}
