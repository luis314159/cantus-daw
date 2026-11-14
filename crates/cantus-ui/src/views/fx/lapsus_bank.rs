//! Lapsus's banks (as Gross Beat's): thirty-six **time** patterns and thirty-six **volume**
//! patterns, played in any combination. The card shows one bank at a time — a switch on
//! top names the pattern each bank plays and chooses which the pads show — as pads in six
//! rows of six, each a small screen with its curve, the playing one lit, the one the knob
//! sets ringed when a note plays another. Under them the keyboard that plays both: one
//! range of 36 notes for each bank, coloured as its curve. A click on a pad chooses that
//! bank's slot (one step of undo, the slot parameter's command). Fx side round, ADR 0132
//! provisional.

use super::super::canvas::{self, Ink};
use super::super::controls;
use super::super::frame;
use super::super::host::FxHost;
use super::super::modfx;
use super::{Playing, State};
use crate::kit::color::with_alpha;
use crate::kit::hint::{self, Caps};
use crate::kit::text::{Role, Src, Text};
use crate::kit::{with_brush, Brush, Fill, Style};
use cantus_core::fx::lapsus::{self as model, Lane};
use cantus_dsp::lapsus::LapsusSettings;
use egui::{pos2, vec2, Align2, Color32, Rect, Sense};

/// The rows' names of each bank (`lapsus.trow.{i}`, `lapsus.vrow.{i}`).
const TROW_KEYS: [&str; 6] = [
    "lapsus.trow.0",
    "lapsus.trow.1",
    "lapsus.trow.2",
    "lapsus.trow.3",
    "lapsus.trow.4",
    "lapsus.trow.5",
];
const VROW_KEYS: [&str; 6] = [
    "lapsus.vrow.0",
    "lapsus.vrow.1",
    "lapsus.vrow.2",
    "lapsus.vrow.3",
    "lapsus.vrow.4",
    "lapsus.vrow.5",
];

/// The rows' names' column at 100 %.
const LABEL_W: f32 = 64.0;
/// The keyboard's height at 100 %.
const KEYS_H: f32 = 30.0;

#[allow(clippy::too_many_arguments)]
pub fn card(
    host: &mut dyn FxHost,
    st: &mut State,
    ui: &mut egui::Ui,
    s: &Style,
    rect: Rect,
    tel: &[f32; 64],
    ls: &LapsusSettings,
    playing: Playing,
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
            let gap = 3.0 * k;
            // The switch: which bank the pads show.
            let sw = Rect::from_min_size(
                pos2(r.left() + 4.0 * k, r.top() + 2.0 * k),
                vec2(r.width() - 8.0 * k, 22.0 * k),
            );
            let mut child = ui.new_child(egui::UiBuilder::new().max_rect(sw));
            let labels = [Src::Key("lapsus.bank.time"), Src::Key("lapsus.bank.vol")];
            let now = if st.bank == Lane::Time { 0 } else { 1 };
            if let Some(i) = controls::segmented(
                &mut child,
                s,
                now,
                &labels,
                frame::tr("lapsus.bank.which"),
                sw.width() / k,
            ) {
                st.bank = if i == 0 { Lane::Time } else { Lane::Vol };
            }
            let lane = st.bank;
            let area = Rect::from_min_max(
                pos2(r.left() + LABEL_W * k, sw.bottom() + 24.0 * k),
                pos2(r.right() - 4.0 * k, r.bottom() - 4.0 * k),
            );
            let pw = ((area.width() - 5.0 * gap) / 6.0).max(8.0);
            let below = KEYS_H * k + 34.0 * k;
            let ph = ((area.height() - below - 5.0 * gap) / 6.0).clamp(8.0, 1.6 * pw);
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
            let (slot_key, _) = lane.keys();
            let knob = match lane {
                Lane::Time => ls.tslot,
                Lane::Vol => ls.vslot,
            };
            // Clicks first: what is drawn is what is now set.
            for i in 0..model::SLOTS {
                let resp = ui.interact(
                    pad(i),
                    ui.id().with(("lapsus-pad", lane as u8, i)),
                    Sense::click(),
                );
                if resp.clicked() && i != knob {
                    host.set(slot_key, i as f32, None);
                }
                if resp.hovered() {
                    hint::set_text(
                        ui.ctx(),
                        super::slot_name(lane, i),
                        frame::tr("lapsus.pad_how"),
                        Caps::NONE,
                    );
                }
            }
            let knob = match lane {
                Lane::Time => cantus_engine::fx::lapsus::settings(host.spec()).tslot,
                Lane::Vol => cantus_engine::fx::lapsus::settings(host.spec()).vslot,
            };
            let ink = Ink::of(s);
            let cyan = modfx::face::second_voice(s, &ink).unwrap_or(ink.faint);
            let colour = |l: Lane| if l == Lane::Time { ink.hue } else { cyan };
            let spec = host.spec();
            with_brush(&ui.painter_at(r), |b| {
                // What each bank plays, under the switch.
                for (j, l) in Lane::ALL.into_iter().enumerate() {
                    let half = sw.width() * 0.5;
                    let cell = Rect::from_min_size(
                        pos2(sw.left() + half * j as f32, sw.bottom() + 3.0 * k),
                        vec2(half, 16.0 * k),
                    );
                    let sl = playing.of(l);
                    b.dot(pos2(cell.left() + 6.0 * k, cell.center().y), 2.5 * k, colour(l));
                    canvas::line_in(
                        b,
                        Rect::from_min_max(pos2(cell.left() + 13.0 * k, cell.top()), cell.max),
                        Role::Micro,
                        if l == lane { s.t.text } else { s.t.text_dim },
                        k,
                        format_args!("{} {}", sl + 1, super::slot_name(l, sl)),
                    );
                }
                let rows = if lane == Lane::Time { TROW_KEYS } else { VROW_KEYS };
                for (row, key) in rows.iter().enumerate() {
                    let y = pads.top() + row as f32 * (ph + gap) + ph * 0.5;
                    let t = Text::key(key, Role::Micro, s.t.text_dim).caps(true);
                    canvas::put(
                        b,
                        pos2(pads.left() - 8.0 * k, y),
                        Align2::RIGHT_CENTER,
                        &t,
                        k,
                        None,
                    );
                }
                let c = colour(lane);
                for i in 0..model::SLOTS {
                    let pr = b.snap_rect(pad(i));
                    let lit = i == playing.of(lane);
                    if lit {
                        b.rrect_soft(pr.expand(1.5 * k), 2.5 * k, Fill::Solid(with_alpha(c, 0.25)), 4.0 * k);
                    }
                    canvas::ground_b(b, pr, &ink, s.hue_tok);
                    let p = super::pattern_of(spec, lane, i);
                    let inner = Rect::from_min_max(
                        pos2(pr.left() + 4.0 * k, pr.top() + 12.0 * k),
                        pos2(pr.right() - 4.0 * k, pr.bottom() - 4.0 * k),
                    );
                    let dim = if lit { c } else { with_alpha(c, 0.62) };
                    super::grid::thumb(b, inner, lane, &p, dim, k * 0.85);
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
                    let (edge, w) = if lit {
                        (c, 1.0 * k)
                    } else if i == knob {
                        (with_alpha(ink.text, 0.7), 1.0 * k)
                    } else {
                        (ink.edge, b.px())
                    };
                    b.rrect_stroke(pr, 2.0 * k, w, edge);
                }
                let kb = Rect::from_min_size(
                    pos2(
                        pads.left(),
                        (area.bottom() - below + 12.0 * k).max(pads.bottom() + 12.0 * k),
                    ),
                    vec2(pads.width(), KEYS_H * k),
                );
                keyboard(b, kb, s, ls, playing, ink.hue, cyan, k);
            });
        },
    );
}

/// The keys that play both banks: the two ranges coloured under the keys, the two playing
/// notes lit, each range named under it.
#[allow(clippy::too_many_arguments)]
fn keyboard(
    b: &mut Brush,
    r: Rect,
    s: &Style,
    ls: &LapsusSettings,
    playing: Playing,
    amber: Color32,
    cyan: Color32,
    k: f32,
) {
    let n = model::SLOTS as u8;
    let lo = ls.tkey.min(ls.vkey);
    let hi = ls.tkey.max(ls.vkey).saturating_add(n - 1).min(127);
    let black = |p: u8| matches!(p % 12, 1 | 3 | 6 | 8 | 10);
    let whites = (lo..=hi).filter(|p| !black(*p)).count().max(1);
    let keys = Rect::from_min_max(r.min, pos2(r.right(), r.bottom() - 5.0 * k));
    let ww = keys.width() / whites as f32;
    let range = |p: u8| {
        if (ls.tkey..ls.tkey.saturating_add(n)).contains(&p) {
            Some(amber)
        } else if (ls.vkey..ls.vkey.saturating_add(n)).contains(&p) {
            Some(cyan)
        } else {
            None
        }
    };
    let lit = |p: u8| {
        if p == ls.tkey.saturating_add(playing.time as u8) {
            Some(amber)
        } else if p == ls.vkey.saturating_add(playing.vol as u8) {
            Some(cyan)
        } else {
            None
        }
    };
    let (white, ink_black) = (s.t.key_white, s.t.key_black);
    let px = b.px();
    let mut x = keys.left();
    let mut xs = [0.0f32; 128];
    for p in lo..=hi {
        if black(p) {
            xs[p as usize] = x;
            continue;
        }
        let kr = b.snap_rect(Rect::from_min_max(pos2(x, keys.top()), pos2(x + ww, keys.bottom())));
        b.rect(kr, Fill::Solid(lit(p).unwrap_or(white)));
        b.vline(kr.right() - px * 0.5, kr.top(), kr.bottom(), px, with_alpha(ink_black, 0.35));
        if let Some(c) = range(p) {
            b.rect(
                Rect::from_min_max(pos2(kr.left(), keys.bottom() + 1.5 * k), pos2(kr.right(), r.bottom())),
                Fill::Solid(c),
            );
        }
        xs[p as usize] = x;
        x += ww;
    }
    for p in lo..=hi {
        if !black(p) {
            continue;
        }
        let bw = ww * 0.62;
        let kr = Rect::from_min_size(
            pos2(xs[p as usize] - bw * 0.5, keys.top()),
            vec2(bw, keys.height() * 0.6),
        );
        b.rect(b.snap_rect(kr), Fill::Solid(lit(p).unwrap_or(ink_black)));
    }
    b.rrect_stroke(b.snap_rect(keys), 0.0, px, with_alpha(ink_black, 0.6));
    // The ranges named under them.
    let mut name = |key: u8, l: Lane, c: Color32| {
        let mut a = String::new();
        let mut z = String::new();
        super::note_pill(f32::from(key), &mut a);
        super::note_pill(f32::from(key.saturating_add(n - 1)), &mut z);
        let first = (lo..key).filter(|p| !black(*p)).count() as f32;
        let at = pos2(keys.left() + first * ww + 8.0 * k, r.bottom() + 3.0 * k);
        b.dot(pos2(at.x - 5.0 * k, at.y + 5.0 * k), 2.5 * k, c);
        let word = frame::tr(if l == Lane::Time {
            "lapsus.bank.time"
        } else {
            "lapsus.bank.vol"
        });
        canvas::putf(
            b,
            at,
            Align2::LEFT_TOP,
            Role::Micro,
            s.t.text_dim,
            k,
            None,
            format_args!("{word} {a}\u{2013}{z}"),
        );
    };
    name(ls.vkey, Lane::Vol, cyan);
    name(ls.tkey, Lane::Time, amber);
}
