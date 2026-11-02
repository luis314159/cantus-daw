//! Saltator's move map as a strip of pads (round 35, ADR 0087), like Pulsus's: one pad per
//! key from C3, each naming its key and the move it plays, the black keys' pads a shade
//! deeper. The move dancing now is lit; pressing a pad plays its key on the dancer's track
//! (a live note, as the shell's keyboard plays one), so the stage dances it at once.

use crate::app::CantusApp;
use crate::i18n::t;
use crate::kit::color::{mix, with_alpha};
use crate::kit::hint::{self, Caps};
use crate::kit::text::{self as ktext, Policy, Role, Text};
use crate::kit::{with_brush, Fill, State as KitState, Style};
use cantus_core::inst::dancer::{Move, BASE_KEY};
use cantus_core::TrackId;
use egui::{Align2, Pos2, Rect, Sense, Vec2};

/// Whether a key is a black one.
fn black(key: u8) -> bool {
    matches!(key % 12, 1 | 3 | 6 | 8 | 10)
}

/// The pads' rects across `rect`: one row of sixteen, or two of eight when narrow.
pub fn pads(rect: Rect, gap: f32) -> Vec<Rect> {
    let n = Move::ALL.len();
    let rows = if rect.width() / n as f32 > rect.height() * 0.9 {
        1
    } else {
        2
    };
    let cols = n / rows;
    let w = (rect.width() - gap * (cols - 1) as f32) / cols as f32;
    let h = (rect.height() - gap * (rows - 1) as f32) / rows as f32;
    (0..n)
        .map(|i| {
            let (c, r) = (i % cols, i / cols);
            Rect::from_min_size(
                Pos2::new(
                    rect.left() + c as f32 * (w + gap),
                    rect.top() + r as f32 * (h + gap),
                ),
                Vec2::new(w, h),
            )
        })
        .collect()
}

/// The strip in `rect`. `playing`: the move dancing now (lit).
pub fn strip(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    rect: Rect,
    track: TrackId,
    playing: Option<Move>,
    s: &Style,
) {
    let rects = pads(rect, s.s(3.0));
    let held = app.inst.dancer.pad;
    let mut pressed = None;
    for (i, (m, r)) in Move::ALL.iter().zip(&rects).enumerate() {
        let key = BASE_KEY + i as u8;
        let resp = ui.interact(*r, ui.id().with(("dance-pad", i)), Sense::click_and_drag());
        let name = t(m.label_key());
        let pitch = cantus_core::model::pitch_name(key);
        if resp.hovered() {
            hint::set_text(
                ui.ctx(),
                &format!("{pitch} \u{b7} {name}"),
                &t("dance.pad.tip"),
                Caps::NONE,
            );
        }
        if resp.is_pointer_button_down_on() {
            pressed = Some(key);
        }
        let lit = playing == Some(*m) || held == Some(key);
        let st = KitState::of(&resp, lit);
        with_brush(&ui.painter_at(r.expand(s.s(2.0))), |b| {
            let base = if black(key) {
                mix(s.t.deck, s.t.ink, 0.55)
            } else {
                mix(s.t.deck, s.t.ink, 0.25)
            };
            let fill = if lit {
                mix(base, s.hue, 0.55)
            } else if st.hovered {
                mix(base, s.hue, 0.15)
            } else {
                base
            };
            b.rrect(*r, s.r(4.0), Fill::V(mix(fill, s.t.light, 0.06), fill));
            if lit {
                b.rrect_stroke(*r, s.r(4.0), s.s(1.0), s.hue);
                b.radial(
                    *r,
                    r.center(),
                    r.size() * 0.6,
                    &[
                        (0.0, with_alpha(s.hue, 0.35)),
                        (1.0, with_alpha(s.hue, 0.0)),
                    ],
                    6,
                );
            }
            let top = Rect::from_min_size(
                r.min + Vec2::new(s.s(4.0), s.s(1.0)),
                Vec2::new(r.width() - s.s(8.0), r.height() * 0.32),
            );
            let _ = ktext::paint(
                b,
                top,
                &Text::new(
                    &pitch,
                    Role::Label,
                    if lit { s.t.text } else { s.t.text_dim },
                )
                .caps(false)
                .align(Align2::LEFT_CENTER)
                .policy(Policy::Elide(ktext::Elide::End)),
                s.tk(),
            );
            // Its name, over two lines when it needs them ("Paso a la / izquierda").
            let bottom = Rect::from_min_max(
                Pos2::new(r.left() + s.s(3.0), r.top() + r.height() * 0.32),
                Pos2::new(r.right() - s.s(3.0), r.bottom() - s.s(2.0)),
            );
            let mut lines =
                (*crate::kit::prose::wrap(b.ctx(), &name, Role::Label, s.tk(), bottom.width()))
                    .clone();
            lines.lines.truncate(2);
            let n = lines.lines.len().max(1) as f32;
            lines.line_h = (bottom.height() / n).min(lines.line_h.max(1.0) * 1.05);
            let h = lines.line_h * n;
            let block = Rect::from_center_size(bottom.center(), Vec2::new(bottom.width(), h));
            let _ = crate::kit::prose::paint(
                b,
                block,
                &lines,
                Role::Label,
                s.tk(),
                if lit { s.t.text } else { s.t.label },
                Align2::CENTER_CENTER,
            );
        });
    }
    app.inst.dancer.placed.insert("pads", rect);
    // A live note while a pad is held down, as the shell's keyboard plays one.
    if pressed != held {
        if let Some(k) = held {
            app.engine.note_off(track, k);
        }
        if let Some(k) = pressed {
            app.engine.note_on(track, k, 100);
        }
        app.inst.dancer.pad = pressed;
    }
}
