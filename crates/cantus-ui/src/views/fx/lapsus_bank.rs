//! Lapsus's bank: thirty-six patterns as pads in six rows of six — stops and starts,
//! speed, repeats, reverse, scratch, level — each a small screen with its two curves, the
//! playing one lit, the slot the knob sets ringed when a note plays another; under them
//! the keyboard that plays them (a note per slot from `key`). A click chooses the slot (one
//! step of undo, the `slot` parameter's command). Fx side round, ADR 0132 provisional.

use super::super::canvas::{self, Ink};
use super::super::controls;
use super::super::frame;
use super::super::host::FxHost;
use super::super::modfx;
use super::State;
use crate::kit::color::with_alpha;
use crate::kit::hint::{self, Caps};
use crate::kit::text::{Role, Src, Text};
use crate::kit::{with_brush, Fill, Style};
use cantus_core::fx::lapsus as model;
use cantus_dsp::lapsus::LapsusSettings;
use egui::{pos2, vec2, Align2, Rect, Sense};

/// The rows' names (`lapsus.row.{i}`).
const ROW_KEYS: [&str; 6] = [
    "lapsus.row.0",
    "lapsus.row.1",
    "lapsus.row.2",
    "lapsus.row.3",
    "lapsus.row.4",
    "lapsus.row.5",
];

/// The rows' names' column at 100 %.
const LABEL_W: f32 = 64.0;
/// The keyboard's height at 100 %.
const KEYS_H: f32 = 40.0;

#[allow(clippy::too_many_arguments)]
pub fn card(
    host: &mut dyn FxHost,
    st: &mut State,
    ui: &mut egui::Ui,
    s: &Style,
    rect: Rect,
    tel: &[f32; 64],
    ls: &LapsusSettings,
    playing: usize,
) {
    let _ = tel;
    controls::card(
        ui,
        s,
        rect,
        Src::Key("lapsus.card.bank"),
        None,
        |ui, cr| {
            let r = cr.body;
            let k = s.k;
            let gap = 4.0 * k;
            let area = Rect::from_min_max(
                pos2(r.left() + LABEL_W * k, r.top() + 2.0 * k),
                pos2(r.right() - 4.0 * k, r.bottom() - 4.0 * k),
            );
            let pw = ((area.width() - 5.0 * gap) / 6.0).max(8.0);
            let below = KEYS_H * k + 30.0 * k;
            let ph = ((area.height() - below - 5.0 * gap) / 6.0).clamp(8.0, 1.8 * pw);
            let pads = Rect::from_min_size(area.min, vec2(area.width(), 6.0 * ph + 5.0 * gap));
            st.pads = Some(pads);
            let pad = |i: usize| {
                let (row, col) = (i / 6, i % 6);
                Rect::from_min_size(
                    pos2(
                        pads.left() + col as f32 * (pw + gap),
                        pads.top() + row as f32 * (ph + gap),
                    ),
                    vec2(pw, ph),
                )
            };
            // Clicks first: what is drawn is what is now set.
            for i in 0..model::SLOTS {
                let resp = ui.interact(pad(i), ui.id().with(("lapsus-pad", i)), Sense::click());
                if resp.clicked() && i != ls.slot {
                    host.set("slot", i as f32, None);
                }
                if resp.hovered() {
                    hint::set_text(
                        ui.ctx(),
                        super::slot_name(i),
                        frame::tr("lapsus.pad_how"),
                        Caps::NONE,
                    );
                }
            }
            let knob = cantus_engine::fx::lapsus::settings(host.spec()).slot;
            let ink = Ink::of(s);
            let cyan = modfx::face::second_voice(s, &ink).unwrap_or(ink.faint);
            let spec = host.spec();
            with_brush(&ui.painter_at(r), |b| {
                for (row, key) in ROW_KEYS.iter().enumerate() {
                    let y = pads.top() + row as f32 * (ph + gap) + ph * 0.5;
                    let t = Text::key(key, Role::Micro, s.t.text_dim).caps(true);
                    canvas::put(b, pos2(pads.left() - 8.0 * k, y), Align2::RIGHT_CENTER, &t, k, None);
                }
                for i in 0..model::SLOTS {
                    let pr = b.snap_rect(pad(i));
                    let lit = i == playing;
                    canvas::ground_b(b, pr, &ink, s.hue_tok);
                    if lit {
                        b.rrect_soft(pr.expand(2.0 * k), 3.0 * k, Fill::Solid(with_alpha(ink.hue, 0.30)), 5.0 * k);
                        canvas::ground_b(b, pr, &ink, s.hue_tok);
                    }
                    let p = super::pattern_of(spec, i);
                    // Under the slot's number, so no curve runs through it.
                    let inner = Rect::from_min_max(
                        pos2(pr.left() + 4.0 * k, pr.top() + 13.0 * k),
                        pos2(pr.right() - 4.0 * k, pr.bottom() - 4.0 * k),
                    );
                    let (amber, blue) = if lit {
                        (ink.hue, cyan)
                    } else {
                        (with_alpha(ink.hue, 0.62), with_alpha(cyan, 0.55))
                    };
                    super::grid::thumb(b, inner, &p, amber, blue, k * 0.8);
                    canvas::putf(
                        b,
                        pos2(pr.left() + 2.5 * k, pr.top() + 1.5 * k),
                        Align2::LEFT_TOP,
                        Role::Micro,
                        if lit { ink.text } else { ink.dim },
                        k * 0.85,
                        None,
                        format_args!("{}", i + 1),
                    );
                    let edge = if lit {
                        ink.hue
                    } else if i == knob {
                        with_alpha(ink.text, 0.7)
                    } else {
                        ink.edge
                    };
                    b.rrect_stroke(pr, 2.0 * k, if lit || i == knob { 1.5 * k } else { b.px() }, edge);
                }
                // The keyboard that plays them: a note per slot, from `key`.
                let kb = Rect::from_min_size(
                    pos2(pads.left(), (area.bottom() - below + 10.0 * k).max(pads.bottom() + 10.0 * k)),
                    vec2(pads.width(), KEYS_H * k),
                );
                let lo = ls.key;
                let hi = lo.saturating_add(model::SLOTS as u8 - 1);
                let held = [lo.saturating_add(playing as u8)];
                crate::kit::shell::keyboard_card(b, kb, s, lo, hi, &held);
                let mut a = String::new();
                let mut z = String::new();
                super::note_pill(f32::from(lo), &mut a);
                super::note_pill(f32::from(hi), &mut z);
                let line = Rect::from_min_max(
                    pos2(pads.left(), kb.bottom() + 3.0 * k),
                    pos2(r.right() - 4.0 * k, kb.bottom() + 17.0 * k),
                );
                canvas::line_in(
                    b,
                    line,
                    Role::Micro,
                    s.t.text_dim,
                    k,
                    format_args!("{a} \u{2013} {z}  {}", frame::tr("lapsus.keys_play")),
                );
            });
        },
    );
}
