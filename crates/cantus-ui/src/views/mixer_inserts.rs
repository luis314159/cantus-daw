//! The mixer's insert panel (round 37, finish; ADR 0103 §48): FL's panel at the desk's
//! right — the selected strip's ten insert slots, each its number, its on lamp and the
//! effect's whole name, the empty ones named as places. A click on an insert opens its
//! editor; on an empty slot it opens the inspector, where inserts are added. On from the
//! desk's ▾ menu (*Insert panel*). // r37:finish

use crate::app::CantusApp;
use crate::i18n::{t, tf};
use crate::kit::color::{shade, with_alpha};
use crate::kit::text::{self, Elide, Policy, Role, Text};
use crate::kit::{led, light, with_brush, Fill, Style, Tok};
use cantus_core::MAX_INSERTS;
use egui::{pos2, vec2, Align2, Rect, Sense, Ui};

/// The panel's width at 100 %.
pub const WIDTH: f32 = 196.0;
/// A slot's row.
const ROW: f32 = 22.0;

/// The panel in `rect`: the selected strip's inserts (the master's when none is).
pub fn panel(app: &mut CantusApp, ui: &mut Ui, rect: Rect) {
    let s = Style::current();
    let k = s.k;
    let id = app.selected_strip;
    let p = app.project();
    let (title, effects): (String, Vec<(cantus_core::EffectId, String, bool)>) = {
        let strip = id.and_then(|i| p.strip(i));
        let (name, fx) = match strip {
            Some(st) => (
                if st.name.is_empty() {
                    let n = p
                        .strips
                        .iter()
                        .position(|x| x.id == st.id)
                        .map_or(0, |i| i + 1);
                    tf("mixer.inserts.strip", &[("n", &format!("{n}"))])
                } else {
                    st.name.clone()
                },
                &st.effects,
            ),
            None => (t("kit.mixer.master"), &p.master.effects),
        };
        (
            name,
            fx.iter()
                .map(|e| (e.id, crate::history_text::effect_name(e), !e.bypass))
                .collect(),
        )
    };
    let head = Rect::from_min_size(rect.min, vec2(rect.width(), 28.0 * k));
    let list = Rect::from_min_max(pos2(rect.left(), head.bottom() + 4.0 * k), rect.max);
    let mut clicked = None;
    let mut rows = Vec::with_capacity(MAX_INSERTS);
    for i in 0..MAX_INSERTS {
        let r = Rect::from_min_size(
            pos2(
                list.left() + 6.0 * k,
                list.top() + i as f32 * (ROW + 2.0) * k,
            ),
            vec2(list.width() - 12.0 * k, ROW * k),
        );
        if r.bottom() > list.bottom() {
            break;
        }
        let resp = ui.interact(r, egui::Id::new(("mixer-insert-slot", i)), Sense::click());
        if resp.clicked() {
            clicked = Some(i);
        }
        if resp.hovered() {
            let tip = match effects.get(i) {
                Some((_, name, _)) => name.clone(),
                None => t("mixer.inserts.empty_tip"),
            };
            crate::kit::hint::set_text(ui.ctx(), &tip, "", crate::kit::hint::Caps::NONE);
        }
        rows.push((r, resp.hovered()));
    }
    with_brush(ui.painter(), |b| {
        b.register(
            rect,
            "mixer_inserts",
            &[Tok::Panel, Tok::Text, Tok::Led],
            &[],
        );
        let m = light::Material::of(&s.t);
        b.rect(b.snap_rect(rect), Fill::Solid(s.t.panel));
        light::recess(
            b,
            rect.shrink(2.0 * k),
            s.r(4.0),
            shade(s.t.panel, -0.03),
            &m,
            1.0,
            k,
        );
        let _ = text::paint(
            b,
            head.shrink2(vec2(10.0 * k, 0.0)),
            &Text::key("mixer.inserts.title", Role::Section, s.t.label).align(Align2::LEFT_CENTER),
            s.tk(),
        );
        let _ = text::paint(
            b,
            head.shrink2(vec2(10.0 * k, 0.0)),
            &Text::new(&title, Role::Label, s.t.text)
                .align(Align2::RIGHT_CENTER)
                .policy(Policy::Elide(Elide::Middle)),
            s.tk(),
        );
        b.hline(
            rect.left() + 6.0 * k,
            rect.right() - 6.0 * k,
            head.bottom(),
            b.px(),
            shade(s.t.panel, -0.15),
        );
        for (i, (r, hovered)) in rows.iter().enumerate() {
            let filled = effects.get(i);
            let base = if filled.is_some() {
                shade(s.t.panel_raised, if s.t.dark { 0.0 } else { -0.02 })
            } else {
                shade(s.t.panel, -0.015)
            };
            if filled.is_some() || *hovered {
                light::chip(b, *r, s.r(3.0), base, &m, *hovered);
            }
            let num = Rect::from_min_size(r.min, vec2(20.0 * k, r.height()));
            let _ = text::paint(
                b,
                num,
                &Text::new(&format!("{}", i + 1), Role::Label, s.t.text_dim)
                    .align(Align2::CENTER_CENTER),
                s.tk(),
            );
            let name_r = Rect::from_min_max(pos2(num.right() + 4.0 * k, r.top()), r.max);
            match filled {
                Some((_, name, on)) => {
                    let c = pos2(r.right() - 10.0 * k, r.center().y);
                    led::paint(b, c, 8.0 * k, *on, &s, Tok::Led, Tok::Well, 1.0);
                    let _ = text::paint(
                        b,
                        name_r.with_max_x(r.right() - 20.0 * k),
                        &Text::new(name, Role::Body, if *on { s.t.text } else { s.t.text_dim })
                            .align(Align2::LEFT_CENTER)
                            .policy(Policy::Elide(Elide::End)),
                        s.tk(),
                    );
                }
                None => {
                    let _ = text::paint(
                        b,
                        name_r,
                        &Text::new(
                            &tf("mixer.inserts.slot", &[("n", &format!("{}", i + 1))]),
                            Role::Placeholder,
                            with_alpha(s.t.text_dim, 0.7),
                        )
                        .align(Align2::LEFT_CENTER),
                        s.tk(),
                    );
                }
            }
        }
    });
    if let Some(i) = clicked {
        match (id, effects.get(i)) {
            (Some(strip), Some((fx, _, _))) => app.open_effect_editor(strip, *fx),
            _ => app.show_pane(crate::views::workspace::Pane::Inspector),
        }
    }
}
