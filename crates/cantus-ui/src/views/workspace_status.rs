//! The status bar, in the kit's look: the status line at the left, the engine's readouts
//! at the right, and at its two corners the toggles for the lower zone and the inspector
//! (ADR 0024 §5: "a toggle at its outer corner").
//!
//! Every word is painted into a box with a policy (§3): the readouts claim their width
//! first, from the right, and the least important drop out when the window is narrow;
//! the status line takes what is left and elides. The readouts' texts are rebuilt only
//! when what they say changes, so a bar at rest allocates nothing.

use super::workspace::Pane;
use crate::app::{CantusApp, CentralView, EditFocus};
use crate::i18n::{t, tf, Locale};
use crate::kit::color::shade;
use crate::kit::glyph::Glyph;
use crate::kit::hint::{self, Caps};
use crate::kit::text::{self, Elide, Policy, Role, Text};
use crate::kit::{cap, with_brush, Fill, State, Style};
use egui::{Align2, Pos2, Rect, Sense, Ui, Vec2};
use std::cell::RefCell;
use std::fmt::Write as _;

/// The bar's height at 100 %.
pub const HEIGHT: f32 = 22.0;

/// One readout at the right: its text, whether it warns, its tooltip, and how soon it
/// leaves when the bar is short of room (lower first).
struct Item {
    text: String,
    warn: bool,
    tip: Option<String>,
    priority: u8,
    width: f32,
}

/// What the readouts say: they are rebuilt when this changes.
#[derive(Copy, Clone, Debug, PartialEq)]
struct Key {
    live: bool,
    rate: u32,
    load: u32,
    overruns: u64,
    voices: usize,
    latency: u64,
    unserved: u64,
    /// The block of the latest dropout (its blame is shown until the next one).
    dropout: Option<u64>,
    midi: usize,
    keyboard: Option<i32>,
    locale: Locale,
    tk: u32,
    ppp: u32,
}

#[derive(Default)]
struct Cache {
    key: Option<Key>,
    items: Vec<Item>,
    stats: String,
}

thread_local! {
    static CACHE: RefCell<Cache> = RefCell::new(Cache::default());
}

fn rebuild(app: &CantusApp, ctx: &egui::Context, key: Key, gap: f32, tk: f32, c: &mut Cache) {
    let dim = Style::current().t.text_dim;
    c.items.clear();
    let mut push = |text: String, warn: bool, tip: Option<String>, priority: u8| {
        let width = text::measure(ctx, &Text::new(&text, Role::Label, dim), tk).x + gap;
        c.items.push(Item {
            text,
            warn,
            tip,
            priority,
            width,
        });
    };
    // The device, its rate and its latency are the audio chip's (round 20); the engine's
    // own numbers stay here.
    if key.live {
        let engine = tf(
            "devices.status.engine",
            &[
                ("load", &key.load),
                ("overruns", &key.overruns),
                ("voices", &key.voices),
            ],
        );
        // The callback's histogram in the engine readout's tooltip (round 20, `perf_status`).
        let tip = Some(crate::perf_status::histogram_text(app.engine.stats()));
        push(engine, key.overruns > 0, tip, 4);
    }
    // The latest dropout and who it is blamed on (round 20, `perf_status`).
    if let Some(d) = app.engine.stats().last_dropout {
        let p = app.project();
        push(
            crate::perf_status::dropout_text(p, &d),
            true,
            Some(crate::perf_status::dropout_tip(p, &d)),
            3,
        );
    }
    // Plug-in latency is compensated, never silent: shown whenever there is some, in the
    // warning colour when a path could not be given a delay line.
    if key.latency > 0 || key.unserved > 0 {
        let ms = key.latency as f64 * 1000.0 / app.engine.sample_rate.max(1.0);
        let tip = if key.unserved > 0 {
            tf(
                "status.latency_unserved",
                &[("samples", &key.latency), ("n", &key.unserved)],
            )
        } else {
            tf("status.latency_tip", &[("samples", &key.latency)])
        };
        push(
            tf("status.latency", &[("ms", &format!("{ms:.1}"))]),
            key.unserved > 0,
            Some(tip),
            3,
        );
    }
    push(tf("status.midi", &[("n", &key.midi)]), false, None, 2);
    let keyboard = match key.keyboard {
        Some(octave) => tf("status.keyboard", &[("octave", &octave)]),
        None => t("status.keyboard_off"),
    };
    push(keyboard, false, None, 1);
}

/// Paint the status bar into `ui`'s whole rect.
pub fn show(app: &mut CantusApp, ui: &mut Ui) {
    let s = Style::current();
    let tok = s.t;
    // The bar takes its height even in a panel that did not size it.
    let max = ui.max_rect();
    let rect = Rect::from_min_size(
        max.min,
        Vec2::new(
            max.width(),
            max.height().max((HEIGHT * tok.density).round()),
        ),
    );
    ui.allocate_rect(rect, Sense::hover());
    let k = s.k;
    let tk = s.tk();
    let pad = 6.0 * k;
    let key_side = 18.0 * k;
    let gap = 14.0 * k;
    let ctx = ui.ctx().clone();

    // The corner toggles: the lower zone at the left, the inspector at the right.
    let lower_r = Rect::from_center_size(
        Pos2::new(rect.left() + pad + key_side * 0.5, rect.center().y),
        Vec2::splat(key_side),
    );
    let insp_r = Rect::from_center_size(
        Pos2::new(rect.right() - pad - key_side * 0.5, rect.center().y),
        Vec2::splat(key_side),
    );
    let mut lr = ui.interact(lower_r, ui.id().with("lower-toggle"), Sense::click());
    let mut ir = ui.interact(insp_r, ui.id().with("inspector-toggle"), Sense::click());
    if lr.hovered() {
        let tip = t("workspace.toggle.lower");
        hint::set_text(&ctx, &tip, "", Caps::NONE);
        lr = lr.on_hover_text(tip);
    }
    if ir.hovered() {
        let tip = t("workspace.toggle.inspector");
        hint::set_text(&ctx, &tip, "I", Caps::NONE);
        ir = ir.on_hover_text(tip);
    }
    if lr.clicked() {
        app.workspace.toggle_lower();
    }
    if ir.clicked() {
        app.toggle_pane(Pane::Inspector);
    }
    let lower_on = app.workspace.lower_shown();
    let insp_on = app.workspace.contains(Pane::Inspector);

    let stats = *app.engine.stats();
    let key = Key {
        live: app.engine.is_live(),
        rate: app.engine.sample_rate as u32,
        load: (stats.load * 100.0).round() as u32,
        overruns: stats.overruns,
        voices: stats.active_voices,
        latency: stats.latency_samples,
        unserved: stats.pdc_unserved,
        dropout: stats.last_dropout.map(|d| d.block),
        midi: app.midi_port_names.len(),
        keyboard: app.keyboard_piano.then_some(app.keyboard.octave),
        locale: crate::i18n::locale(),
        tk: tk.to_bits(),
        ppp: ctx.pixels_per_point().to_bits(),
    };

    // The audio chip, first from the right: always there (round 20).
    let chip_w = super::audio_chip::width(app, &ctx);
    let chip_r = Rect::from_min_max(
        Pos2::new(insp_r.left() - pad - chip_w, rect.top() + 2.0 * k),
        Pos2::new(insp_r.left() - pad, rect.bottom() - 2.0 * k),
    );
    let right_edge = chip_r.left() - gap * 0.5;
    // Round 20: the minimised devices' chips come first; the status line follows them.
    let left_edge = super::device_chrome::tray(ui, rect, lower_r.right() + pad, k);
    let painter = ui.painter_at(rect);
    CACHE.with(|cell| {
        let mut c = cell.borrow_mut();
        if c.key != Some(key) {
            rebuild(app, &ctx, key, gap, tk, &mut c);
            c.key = Some(key);
        }
        let c = &mut *c;
        // The stats readout changes every frame; it has its own reused buffer.
        let stats_w = if app.show_stats {
            c.stats.clear();
            let _ = write!(
                c.stats,
                "ui {:.1} ms · notes drawn {} · block {} · max {:.0} µs",
                app.average_frame_ms(),
                app.piano_roll_notes_drawn,
                stats.block_frames,
                stats.max_block_us
            );
            text::measure(&ctx, &Text::new(&c.stats, Role::Label, tok.text_dim), tk).x + gap
        } else {
            0.0
        };
        // Drop the least important readouts until the status line keeps 160 pt.
        let mut keep = [true; 8];
        let items = &c.items;
        let room = |keep: &[bool; 8], stats: bool| {
            right_edge
                - left_edge
                - items
                    .iter()
                    .zip(keep.iter())
                    .filter(|(_, k)| **k)
                    .map(|(it, _)| it.width)
                    .sum::<f32>()
                - if stats { stats_w } else { 0.0 }
        };
        let stats_on = app.show_stats && room(&keep, true) >= 160.0 * k;
        if !stats_on {
            for p in 0..=4u8 {
                if room(&keep, false) >= 160.0 * k {
                    break;
                }
                for (i, it) in items.iter().enumerate() {
                    if it.priority == p {
                        keep[i] = false;
                    }
                }
            }
        }
        let mut x = right_edge;
        for (i, it) in items.iter().enumerate() {
            if !keep[i] {
                continue;
            }
            let r = Rect::from_min_max(
                Pos2::new(x - it.width, rect.top()),
                Pos2::new(x, rect.bottom()),
            );
            x -= it.width;
            if let Some(tip) = &it.tip {
                let resp = ui.interact(r, ui.id().with(("status-item", i)), Sense::hover());
                if resp.hovered() {
                    let _ = resp.on_hover_text(tip.as_str());
                }
            }
        }
        let stats_r = stats_on.then(|| {
            let r = Rect::from_min_max(
                Pos2::new(x - stats_w, rect.top()),
                Pos2::new(x, rect.bottom()),
            );
            x -= stats_w;
            r
        });
        let line = Rect::from_min_max(
            Pos2::new(left_edge, rect.top()),
            Pos2::new(x - gap * 0.5, rect.bottom()),
        );
        with_brush(&painter, |b| {
            let r = b.snap_rect(rect);
            b.rect(r, Fill::V(shade(tok.panel, 0.02), shade(tok.panel, -0.02)));
            b.hline(
                r.left(),
                r.right(),
                r.top() + b.px() * 0.5,
                b.px(),
                shade(tok.panel, if tok.dark { -0.16 } else { -0.12 }),
            );
            for (on, rr, resp, g) in [
                (lower_on, lower_r, &lr, Glyph::Mixer),
                (insp_on, insp_r, &ir, Glyph::Inspector),
            ] {
                let st = State::of(resp, on);
                cap::toolbar_key(
                    b,
                    rr,
                    &s.scaled(key_side / 28.0),
                    st,
                    cap::KeyFace::Glyph(g),
                );
            }
            let msg = Text::new(
                &app.status.text,
                Role::Label,
                if app.status.error {
                    tok.danger
                } else {
                    tok.text_dim
                },
            )
            .align(Align2::LEFT_CENTER)
            .policy(Policy::Elide(Elide::End));
            let _ = text::paint(b, line, &msg, tk);
            let mut x = right_edge;
            for (i, it) in items.iter().enumerate() {
                if !keep[i] {
                    continue;
                }
                let r = Rect::from_min_max(
                    Pos2::new(x - it.width, rect.top()),
                    Pos2::new(x, rect.bottom()),
                );
                x -= it.width;
                let ink = if it.warn { tok.danger } else { tok.text_dim };
                let tt = Text::new(&it.text, Role::Label, ink)
                    .align(Align2::RIGHT_CENTER)
                    .policy(Policy::Elide(Elide::End));
                let _ = text::paint(b, r.shrink2(Vec2::new(gap * 0.5, 0.0)), &tt, tk);
                divider(b, r.left(), rect, k, tok.panel);
            }
            divider(b, chip_r.left() - gap * 0.25, rect, k, tok.panel);
            if let Some(r) = stats_r {
                let tt = Text::new(&c.stats, Role::Label, tok.text_dim)
                    .align(Align2::RIGHT_CENTER)
                    .policy(Policy::Elide(Elide::End));
                let _ = text::paint(b, r.shrink2(Vec2::new(gap * 0.5, 0.0)), &tt, tk);
                divider(b, r.left(), rect, k, tok.panel);
            }
        });
    });
    // Round 20: the device tray's chips, over the bar's ground.
    super::device_chrome::tray_paint(ui, rect, k);
    super::audio_chip::show(app, ui, chip_r);
}

/// The hint of the editor being worked in — what the hint panel says when nothing is
/// hovered. It follows the half of the window being edited: the piano roll's tool hints
/// while clips are dragged in the arrangement would answer a question nobody asked.
pub fn tool_hint_key(app: &CantusApp) -> Option<&'static str> {
    match app.focus {
        EditFocus::Arrangement => Some("arrangement.hint"),
        EditFocus::Mixer => Some("workspace.mixer.hint"),
        EditFocus::PianoRoll => Some(match app.central {
            CentralView::PianoRoll => app.piano_roll.tool.hint_key(),
            CentralView::ChannelRack => "rack.hint",
            CentralView::Mixer => "workspace.mixer.hint",
        }),
    }
}

/// A 1-px divider between two readouts.
fn divider(b: &mut crate::kit::Brush, x: f32, rect: Rect, k: f32, panel: egui::Color32) {
    b.vline(
        b.snap_mid(x),
        rect.top() + 5.0 * k,
        rect.bottom() - 5.0 * k,
        b.px(),
        shade(panel, -0.14),
    );
}
