//! **Quantise audio…** (round 29, ADR 0061): the transients of the chosen audio clips
//! pulled toward the grid — which grid, how far (the strength) and which transients (those
//! at least so strong, a share of the clip's strongest). A small floating panel, opened
//! from the clip menu, the warp editor's bar and the inspector; *Quantise* applies it as
//! one undo step (`CantusApp::quantise_audio`), Esc or a click outside closes it.

use crate::app::CantusApp;
use crate::clips::ClipRef;
use crate::i18n::t;
use crate::kit::glyph::Glyph;
use crate::kit::hint::{self, Caps};
use crate::kit::text::{self, Role, Src, Text};
use crate::kit::{self, well, State, Style};
use crate::views::workspace_settings as ws;
use crate::views::GRIDS;
use cantus_core::Ticks;
use egui::{pos2, vec2, Align2, Context, Id, Key, Rect, Sense};

/// The panel's size at 100 %.
pub const WIDTH: f32 = 420.0;
const HEIGHT: f32 = 196.0;
const TITLE: f32 = 30.0;
const ROW: f32 = 30.0;
const PAD: f32 = 12.0;
/// The grids offered: the arrangement's, without the bar and the half note.
const OFFERED: [usize; 6] = [2, 3, 4, 5, 6, 7];

impl CantusApp {
    /// Open *Quantise audio…* on `clips` (the audio ones among them).
    pub fn open_quantise(&mut self, clips: Vec<ClipRef>) {
        let audio: Vec<ClipRef> = clips
            .into_iter()
            .filter(|r| crate::audio_clips::region(self.project(), *r).is_some())
            .collect();
        if audio.is_empty() {
            return;
        }
        // Ask for the analyses now: they are ready when *Quantise* is pressed.
        for r in &audio {
            let _ = self.warp_analysis(*r);
        }
        self.warp.quantise_open = Some(audio);
    }
}

/// Every frame: the panel, while open.
pub fn frame(app: &mut CantusApp, ctx: &Context) {
    let Some(clips) = app.warp.quantise_open.clone() else {
        return;
    };
    let s = Style::current();
    let k = s.k;
    let screen = ctx.content_rect();
    let size = vec2((WIDTH * k).min(screen.width() - 16.0), HEIGHT * k);
    let at = pos2(
        screen.center().x - size.x * 0.5,
        screen.top() + screen.height() * 0.22,
    );
    let rect = Rect::from_min_size(at, size);
    if ctx.input(|i| i.key_pressed(Key::Escape)) {
        app.warp.quantise_open = None;
        return;
    }
    let mut close = false;
    let mut apply = ctx.input(|i| i.key_pressed(Key::Enter));
    egui::Area::new(Id::new("cantus-warp-quantise"))
        .order(egui::Order::Foreground)
        .fixed_pos(at)
        .show(ctx, |ui| {
            ui.set_min_size(rect.size());
            ws::frame(ui, rect, &s);
            let title = Rect::from_min_size(rect.min, vec2(size.x, TITLE * k));
            if ws::title(ui, title, &s, Glyph::Warp, "warp.quantise.title") {
                close = true;
            }
            let inner = Rect::from_min_max(
                pos2(rect.left() + PAD * k, title.bottom() + PAD * 0.5 * k),
                pos2(rect.right() - PAD * k, rect.bottom() - PAD * k),
            );
            let label_w = 110.0 * k;
            let row = |i: usize| {
                Rect::from_min_size(
                    pos2(inner.left(), inner.top() + i as f32 * ROW * k),
                    vec2(inner.width(), ROW * k - 6.0 * k),
                )
            };
            let q = &mut app.warp.quantise;
            // The grid.
            let r0 = row(0);
            label(ui, r0, "warp.quantise.grid", label_w, &s);
            let mut child = ui.new_child(egui::UiBuilder::new().max_rect(Rect::from_min_max(
                pos2(r0.left() + label_w, r0.top()),
                r0.max,
            )));
            let labels: Vec<Src> = OFFERED.iter().map(|&i| Src::Key(GRIDS[i].0)).collect();
            let mut sel = OFFERED
                .iter()
                .position(|&i| GRIDS[i].1 == q.grid.0)
                .unwrap_or(2);
            if kit::widgets::segmented(
                &mut child,
                &mut sel,
                &labels,
                &t("warp.quantise.grid"),
                (r0.width() - label_w) / k,
            )
            .changed()
            {
                q.grid = Ticks(GRIDS[OFFERED[sel]].1);
            }
            // The strength and which transients.
            let strength = q.strength;
            if let Some(v) = percent(ui, row(1), "warp.quantise.strength", strength, label_w, &s) {
                q.strength = v;
            }
            let threshold = q.threshold;
            if let Some(v) = percent(
                ui,
                row(2),
                "warp.quantise.threshold",
                threshold,
                label_w,
                &s,
            ) {
                q.threshold = v;
            }
            // What it will do.
            let note = crate::i18n::tf_count("warp.quantise.note", clips.len(), &[]);
            let tx = Text::new(&note, Role::Body, s.t.text_dim).align(Align2::LEFT_CENTER);
            kit::with_brush(ui.painter(), |b| {
                let _ = text::paint(b, row(3), &tx, s.tk());
            });
            // The keys.
            let r4 = row(4);
            let mut keys = ui.new_child(egui::UiBuilder::new().max_rect(r4));
            keys.horizontal(|ui| {
                let mut on = false;
                if kit::widgets::led_button(
                    ui,
                    &mut on,
                    Src::Key("warp.quantise.apply"),
                    &t("warp.quantise.apply"),
                    120.0,
                )
                .clicked()
                {
                    apply = true;
                }
                let mut off = false;
                if kit::widgets::led_button(
                    ui,
                    &mut off,
                    Src::Key("warp.quantise.cancel"),
                    &t("warp.quantise.cancel"),
                    100.0,
                )
                .clicked()
                {
                    close = true;
                }
            });
        });
    let (pressed, pointer) = ctx.input(|i| (i.pointer.any_pressed(), i.pointer.interact_pos()));
    if pressed && !pointer.is_some_and(|p| rect.contains(p)) {
        close = true;
    }
    if apply {
        let q = app.warp.quantise;
        let n = app.quantise_audio(&clips, q);
        // Said only when something was quantised: a clip still analysed keeps its
        // "try again in a moment", and the dialog stays open for it (round 29's review).
        if n > 0 {
            app.set_status(crate::i18n::tf_count("warp.status.quantised", n, &[]));
            close = true;
        } else if !clips.iter().any(|r| app.warp_analysing(*r)) {
            close = true;
        }
    }
    if close {
        app.warp.quantise_open = None;
    }
}

fn label(ui: &egui::Ui, r: Rect, key: &str, w: f32, s: &Style) {
    let words = t(key);
    let tx = Text::new(&words, Role::Label, s.t.text_dim).align(Align2::LEFT_CENTER);
    kit::with_brush(ui.painter(), |b| {
        let _ = text::paint(
            b,
            Rect::from_min_size(r.min, vec2(w, r.height())),
            &tx,
            s.tk(),
        );
    });
}

/// A labelled 0 … 100 % value dragged sideways; the new value while dragged.
fn percent(ui: &mut egui::Ui, r: Rect, key: &str, v: f32, label_w: f32, s: &Style) -> Option<f32> {
    label(ui, r, key, label_w, s);
    let well_r = Rect::from_min_max(
        pos2(r.left() + label_w, r.top()),
        pos2(r.left() + label_w + 120.0 * s.k, r.bottom()),
    );
    let resp = ui.interact(well_r, Id::new(("warp-q", key)), Sense::click_and_drag());
    let face = format!("{:.0} %", v * 100.0);
    kit::with_brush(ui.painter(), |b| {
        well::readout(b, well_r, s, State::of(&resp, false), &face, true);
    });
    if resp.hovered() || resp.dragged() {
        hint::set_text(ui.ctx(), &t(key), &face, Caps::PARAM);
    }
    if resp.dragged() {
        let d = resp.drag_delta().x / (200.0 * s.k);
        return Some((v + d).clamp(0.0, 1.0));
    }
    None
}
