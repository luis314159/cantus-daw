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
use crate::kit::light;
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
    /// The latency is the monitored path's (low-latency monitoring, ADR 0064 §4).
    monitored: bool,
}

#[derive(Default)]
struct Cache {
    key: Key,
    text: String,
    /// r37:finish — the line without its middle (name · latency) where the whole does not
    /// fit: whole words, never a name cut in two ("ASIO4ALL v2…6").
    short: String,
    /// The whole line's width in its chip.
    text_w: f32,
    /// The chip's width with the short line.
    short_width: f32,
    /// The interface's name alone, and the chip's width with it: the least on a full bar
    /// (the name whole, its latency in the hint). // r37:finish
    name: String,
    name_w: f32,
    short_w: f32,
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
    // Low-latency monitoring (round 29, ADR 0064 §4): what is monitored, round trip.
    let monitored = super::freeze_monitor::monitored_ms(app);
    let ms = match monitored {
        Some(ms) => ms,
        None if rate > 0.0 => rt as f64 / rate * 1000.0,
        None => 0.0,
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
            && k.tk == s.tk().to_bits()
            && k.monitored == monitored.is_some();
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
        let mut short = String::new(); // r37:finish
        let text = if !live {
            t("devices.chip.none")
        } else if lost {
            tf("devices.chip.lost", &[("name", &name)])
        } else if stalled {
            tf("devices.chip.stalled", &[("name", &name)])
        } else {
            let mut lat = String::new();
            if monitored.is_some() {
                let mut n = String::new();
                latency::fmt_ms(ms, &mut n);
                lat = tf("freeze.chip.monitored", &[("ms", &n)]);
            } else if rt > 0 {
                latency::fmt_ms(ms, &mut lat);
                lat.push_str(" ms");
            } else {
                lat.push('–');
            }
            short.push_str(&name); // r37:finish
            short.push_str(" · ");
            short.push_str(&lat);
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
        c.width = (tw + 42.0 * s.k).min(380.0 * s.k); // r37:finish (was 44)
        c.text_w = tw; // r37:finish
        let sw = text::measure(ctx, &Text::new(&short, Role::Label, s.t.text), s.tk()).x;
        let nw = text::measure(ctx, &Text::new(&name, Role::Label, s.t.text), s.tk()).x;
        c.short_w = sw;
        c.name_w = nw;
        c.name = if short.is_empty() {
            String::new()
        } else {
            name.clone()
        };
        // r37:finish — the least is the name alone whole: at 1920 nothing folds whatever
        // the interface is called ("Realtek(R) Audio" pushed the area keys into ⋯).
        c.short_width = if short.is_empty() {
            c.width
        } else {
            (nw + 42.0 * s.k).min(c.width)
        };
        c.short = short;
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
            monitored: monitored.is_some(),
        };
    });
}

/// The chip's width at the current scale (the status bar makes room for it).
pub fn width(app: &CantusApp, ctx: &egui::Context) -> f32 {
    let s = Style::current();
    refresh(app, ctx, &s);
    CACHE.with(|c| c.borrow().width)
}

/// The chip's width showing its short line (name · latency), the least a running device's
/// chip takes on the bar. // r37:finish
pub fn short_width(app: &CantusApp, ctx: &egui::Context) -> f32 {
    let s = Style::current();
    refresh(app, ctx, &s);
    CACHE.with(|c| c.borrow().short_width)
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
        // r37:finish — a chip of the family (the meter's and the snap's), not a raised slab.
        light::chip(b, rect, r, base, &light::Material::of(&s.t), false);
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
            // r37:finish — the short line where the whole one does not fit.
            let line = if c.text_w <= tr.width() + 0.5 || c.short.is_empty() {
                &c.text
            } else if c.short_w <= tr.width() + 0.5 || c.name.is_empty() {
                &c.short
            } else {
                &c.name
            };
            let _ = text::paint(
                b,
                tr,
                &Text::new(line, Role::Label, ink)
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

// ---- round 39: the top bar's device group (`dir-a`'s phase B) ----------------- r39:el-topbar

/// What the bar's device group says: the interface's name while it runs, else the
/// sentence (no device, lost, not responding) — and whether it is that warning.
fn bar_line<R>(f: impl FnOnce(&str, bool) -> R) -> R {
    CACHE.with(|c| {
        let c = c.borrow();
        let k = &c.key;
        if k.live && !k.lost && !k.stalled && !c.name.is_empty() {
            f(&c.name, false)
        } else if !k.live {
            f(crate::views::clips_chrome::word("topbar.device_none"), true)
        } else {
            f(&c.text, true)
        }
    })
}

/// What the bar's widths depend on (the words it says).
pub fn bar_key(app: &CantusApp, ctx: &egui::Context) -> u64 {
    use std::hash::{Hash, Hasher};
    let s = Style::current();
    refresh(app, ctx, &s);
    let mut h = std::collections::hash_map::DefaultHasher::new();
    bar_line(|w, bad| (w, bad).hash(&mut h));
    h.finish()
}

/// The words' least and most room on the bar (the sheet's 104; a long name or sentence
/// elides in its middle, whole in the hint).
const BAR_MIN: f32 = 104.0;
const BAR_MAX: f32 = 180.0;

/// The device group's widths: compact (the speaker key, 30) and full (the words).
pub fn bar_widths(app: &CantusApp, ctx: &egui::Context, s: &Style) -> (f32, f32) {
    refresh(app, ctx, s);
    let w = bar_line(|w, bad| {
        let role = if bad { Role::Control } else { Role::Label };
        let tk = s.tk() * 11.0 / role.size().max(1.0);
        text::measure(ctx, &Text::new(w, role, s.t.text), tk).x
    });
    (
        30.0 * s.k,
        (w + 43.0 * s.k).clamp(BAR_MIN * s.k, BAR_MAX * s.k),
    )
}

/// The device group in the key row `row`: a well with the speaker, the interface's name
/// (11, ink 2) and a ▾ — or the warning and the words in amber; compact, the speaker key
/// (with the warning at its corner). A click opens the popover.
pub fn show_bar(app: &mut CantusApp, ui: &mut Ui, column: Rect, s: &Style, compact: bool) {
    use crate::views::transport::paint as p;
    let ctx = ui.ctx().clone();
    refresh(app, &ctx, s);
    let (_, full) = bar_widths(app, &ctx, s);
    let w = if compact { 30.0 * s.k } else { full };
    let row = p::row(column, s);
    let rect = Rect::from_min_size(row.min, Vec2::new(w.min(row.width()), row.height()));
    let resp = ui.interact(rect, ui.id().with("audio-chip"), Sense::click());
    // The popover hangs under the whole bar, not under the key (review 2).
    let column = Rect::from_min_max(
        egui::pos2(rect.left(), column.top()),
        egui::pos2(rect.right(), column.bottom()),
    );
    app.audio_setup.hit(&ctx, Hit::Chip, column);
    if resp.clicked() {
        audio_setup::toggle_popover(app);
    }
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
    let st = State::of(&resp, false);
    let k = s.k;
    with_brush(ui.painter(), |b| {
        b.register(
            rect,
            "audio-chip",
            &[Tok::Well, Tok::Text, Tok::TextDim],
            &[Some(Tok::MeterHigh)],
        );
        p::well(b, rect, s, None);
        p::tint(b, rect, s, st, s.t.highlight, 0.2);
        let gst = if open {
            State {
                hovered: true,
                ..st
            }
        } else {
            st
        };
        bar_line(|words, bad| {
            if compact {
                speaker(b, rect, s, gst);
                if bad {
                    let g = Rect::from_center_size(
                        Pos2::new(rect.right() - 6.0 * k, rect.top() + 6.0 * k),
                        Vec2::splat(8.0 * k),
                    );
                    glyph::paint(b, g, Glyph::Warning, s.t.meter_high);
                }
                return;
            }
            let gc = Rect::from_center_size(
                Pos2::new(rect.left() + 13.0 * k, rect.center().y),
                Vec2::splat(20.0 * k),
            );
            if bad {
                let g = Rect::from_center_size(gc.center(), Vec2::splat(11.0 * k));
                glyph::paint(b, g, Glyph::Warning, s.t.meter_high);
            } else {
                speaker(b, gc, s, gst);
            }
            let tr = Rect::from_min_max(
                Pos2::new(rect.left() + 25.0 * k, rect.top()),
                Pos2::new(rect.right() - 18.0 * k, rect.bottom()),
            );
            let (role, ink) = if bad {
                (Role::Control, s.t.meter_high)
            } else if st.hovered || open {
                (Role::Label, s.t.text)
            } else {
                (Role::Label, s.t.text_dim)
            };
            let _ = text::paint(
                b,
                tr,
                &Text::new(words, role, ink)
                    .align(Align2::LEFT_CENTER)
                    .policy(Policy::Elide(Elide::Middle)),
                s.tk() * 11.0 / role.size().max(1.0),
            );
            let ink = if st.hovered || open {
                p::ink1(s)
            } else {
                p::ink3(s)
            };
            p::chevron(
                b,
                Pos2::new(rect.right() - 10.0 * k, rect.center().y),
                2.5 * k,
                ink,
                s,
            );
        });
    });
}

/// The dialogs' foot line (`dir-a`): "Now: …" what the sound goes through, and whether it
/// is well. // r39:el-topbar
pub fn status_line(app: &CantusApp, ctx: &egui::Context) -> (String, bool) {
    let s = Style::current();
    refresh(app, ctx, &s);
    let good = health(app) == Health::Good;
    CACHE.with(|c| (tf("topbar.now", &[("what", &c.borrow().text)]), good))
}

/// The device group's glyph (el-topbar's sheet, r39:lo-mixer): the family's sounding
/// speaker (set 4), drawn for its 16-pt box on the device, in the key's inks; the kit's
/// device glyph where the family has no drawing.
fn speaker(b: &mut crate::kit::Brush, c: Rect, s: &crate::kit::Style, st: State) {
    let inks = crate::kit::icons::inks(s, st, s.t.highlight, s.t.well);
    let r = Rect::from_center_size(c.center(), Vec2::splat(s.s(16.0)));
    let g = crate::kit::glyph_browser::Glyph::Speaker;
    if !crate::kit::glyph_browser::paint_inks(b, r, g, &inks) {
        crate::views::transport::paint::icon(b, c, s, Glyph::Device, st, s.t.highlight, 20.0);
    }
}
