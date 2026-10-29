//! The FM matrix as a grid of kit cells (FM8's, made legible): a row per modulating
//! operator, a column per modulated one, the diagonal its feedback, and a last column for
//! what it sends to the output. Every cell prints its value — radians, or dB — lit by its
//! strength in the section's hue, and is dragged up and down; a double-click clears it; a
//! ring marks a cell a modulation source reaches.

use super::parts;
use crate::app::CantusApp;
use crate::i18n::{t, tf};
use crate::kit::color::{mix, shade};
use crate::kit::text::{Role, Src};
use crate::kit::{with_brush, Fill, Style};
use crate::views::inst::source_color;
use cantus_core::inst::fm::{matrix_key, NAMES, OPS, OP_KEYS, OUT_OFF};
use cantus_core::inst::modulation::connections;
use cantus_core::{NativeSpec, TrackId};
use egui::{Align2, Pos2, Rect, Sense, Vec2};

/// The matrix in its own card.
pub(super) fn card(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    track: TrackId,
    spec: &NativeSpec,
    s: &Style,
    rect: Rect,
) {
    let disp = rect.height() / s.k - 24.0 - 1.0;
    let cr = parts::card(
        ui,
        rect,
        s,
        Src::Key("fm.matrix_title"),
        None,
        None,
        Some(disp),
    );
    if let Some(d) = cr.display {
        grid(
            app,
            ui,
            track,
            spec,
            &crate::views::inst::face::on_display(s),
            d,
            "card",
        );
    }
}

/// The grid in `rect` (a display); `salt` tells two grids on one view apart.
#[allow(clippy::too_many_arguments)]
pub(super) fn grid(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    track: TrackId,
    spec: &NativeSpec,
    s: &Style,
    rect: Rect,
    salt: &str,
) {
    let k = s.k;
    let inner = rect.shrink(6.0 * k);
    let head = 22.0 * k;
    let cols = OPS + 1;
    let cw = (inner.width() - head) / cols as f32;
    let ch = ((inner.height() - head) / OPS as f32).min(30.0 * k);
    let h = s.hue;
    let routed: Vec<(String, String)> = connections(spec)
        .filter(|c| c.on)
        .map(|c| (c.dst.to_string(), c.src.to_string()))
        .collect();
    let out_label = t("fm.out_short");
    with_brush(&ui.painter_at(rect), |b| {
        // Headers: targets across, sources down; the corner says which way it reads.
        for c in 0..cols {
            let r = Rect::from_min_size(
                Pos2::new(inner.left() + head + cw * c as f32, inner.top()),
                Vec2::new(cw, head),
            );
            let label = NAMES.get(c).copied().unwrap_or(out_label.as_str());
            parts::say(b, r, s, label, Role::Label, s.t.text, Align2::CENTER_CENTER);
        }
        parts::say(
            b,
            Rect::from_min_size(inner.min, Vec2::splat(head)),
            s,
            "\u{2193}\u{2192}",
            Role::Label,
            s.t.text_dim,
            Align2::CENTER_CENTER,
        );
    });
    let corner = Rect::from_min_size(inner.min, Vec2::splat(head));
    let cresp = ui.interact(corner, ui.id().with(("fm-mcorner", salt)), Sense::hover());
    if cresp.hovered() {
        crate::kit::hint::set_text(
            ui.ctx(),
            &t("fm.matrix.corner"),
            "",
            crate::kit::hint::Caps::NONE,
        );
    }
    for src in 0..OPS {
        let y = inner.top() + head + ch * src as f32;
        let selected = app.inst.fm.selected == src;
        let tile = Rect::from_min_size(
            Pos2::new(inner.left(), y + 2.0 * k),
            Vec2::new(head - 4.0 * k, ch - 4.0 * k),
        );
        let tresp = ui.interact(tile, ui.id().with(("fm-mrow", src, salt)), Sense::click());
        if tresp.clicked() {
            app.inst.fm.selected = src;
        }
        let mut cells: Vec<(Rect, String, bool, f32, bool, Option<egui::Color32>)> =
            Vec::with_capacity(cols);
        for c in 0..cols {
            let cell = Rect::from_min_size(
                Pos2::new(inner.left() + head + cw * c as f32 + 1.0 * k, y + 1.0 * k),
                Vec2::new(cw - 2.0 * k, ch - 2.0 * k),
            );
            let (key, v, text, range, per_px): (&'static str, f32, String, (f32, f32), f32) =
                if c < OPS {
                    let key = matrix_key(src, c);
                    let v = spec.get(key);
                    let text = if v == 0.0 {
                        "\u{00b7}".to_string()
                    } else if v.abs() >= 10.0 {
                        format!("{v:.1}")
                    } else {
                        format!("{v:.2}")
                    };
                    let range = if src == c { (0.0, 4.0) } else { (-16.0, 16.0) };
                    (key, v, text, range, 0.02)
                } else {
                    let key = OP_KEYS[src].out;
                    let v = spec.get(key);
                    let text = if v <= OUT_OFF {
                        t("fm.off_short")
                    } else {
                        format!("{v:+.1}")
                    };
                    (key, v, text, (OUT_OFF, 6.0), 0.1)
                };
            let on = if c < OPS { v != 0.0 } else { v > OUT_OFF };
            let resp = ui.interact(
                cell,
                ui.id().with(("fm-cell", src, c, salt)),
                Sense::click_and_drag(),
            );
            let strength = if c < OPS {
                (v.abs() / 8.0).min(1.0)
            } else {
                ((v - OUT_OFF) / 66.0).clamp(0.0, 1.0)
            };
            if resp.dragged() {
                let fine = if ui.input(|i| i.modifiers.shift) {
                    0.2
                } else {
                    1.0
                };
                let d = resp.drag_delta();
                let nv = (v + (d.x - d.y) / k * per_px * fine).clamp(range.0, range.1);
                app.set_inst_value(track, key, nv, true);
            }
            if resp.drag_stopped() {
                app.end_gesture();
            }
            if resp.double_clicked() {
                let default = spec.kind.param(key).map(|i| i.default).unwrap_or(0.0);
                super::batch(app, track, spec, "cmd.fm_disconnect", &[(key, default)]);
            }
            if let Some(p) = resp.dnd_release_payload::<&'static str>() {
                app.connect_modulation(track, *p, key);
            }
            let hot = resp.hovered() || resp.dragged();
            if hot {
                let to = NAMES.get(c).copied().unwrap_or_default();
                let tip = if c < OPS && src == c {
                    tf("fm.cell.fb", &[("op", &NAMES[src])])
                } else if c < OPS {
                    tf("fm.cell.index", &[("from", &NAMES[src]), ("to", &to)])
                } else {
                    tf("fm.cell.out", &[("op", &NAMES[src])])
                };
                crate::kit::hint::set_text(ui.ctx(), &tip, &text, crate::kit::hint::Caps::PARAM);
            }
            let ring = routed
                .iter()
                .find(|(d, _)| d == key)
                .map(|(_, sc)| source_color(sc));
            cells.push((cell, text, on, strength, hot, ring));
        }
        with_brush(&ui.painter_at(rect), |b| {
            let face = if selected {
                mix(s.t.deck_title, h, 0.35)
            } else {
                s.t.deck_title
            };
            b.rrect(tile, s.r(3.0), Fill::Solid(face));
            parts::say(
                b,
                tile,
                s,
                NAMES[src],
                Role::Section,
                s.t.text,
                Align2::CENTER_CENTER,
            );
            for (i, (cell, text, on, strength, hot, ring)) in cells.iter().enumerate() {
                let base = if i == src {
                    shade(s.t.ink, if s.t.dark { 0.05 } else { -0.04 })
                } else {
                    shade(s.t.ink, if s.t.dark { 0.10 } else { -0.07 })
                };
                let fill = if *on {
                    mix(base, h, 0.18 + 0.5 * strength)
                } else {
                    base
                };
                b.rrect(*cell, s.r(2.0), Fill::Solid(fill));
                if *hot {
                    b.rrect_stroke(*cell, s.r(2.0), s.s(1.0), h);
                }
                if let Some(c) = ring {
                    b.rrect_stroke(cell.shrink(s.s(1.5)), s.r(2.0), s.s(1.5), *c);
                }
                let ink = if *on { s.t.text } else { s.t.text_dim };
                parts::say(
                    b,
                    cell.shrink2(Vec2::new(s.s(2.0), 0.0)),
                    s,
                    text,
                    Role::Value,
                    ink,
                    Align2::CENTER_CENTER,
                );
            }
        });
    }
}
