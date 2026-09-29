//! The sixteen voice cards: which is sounding (and what), and how each is off — its
//! oscillators' tuning, its cutoff, its envelope speed, its place in the field — computed
//! by the same function the DSP uses, from the seed and the card's re-roll. Drawn with the
//! kit's brush as small raised cards on the display's ground, each with its lamp (lit by
//! the amplifier's envelope, in the hue), its key, and three bars for its oscillators'
//! tuning. Click a card to read its trims; double-click to re-roll it (one undo step).

use super::*;
use crate::kit::color::{shade, with_alpha};
use crate::kit::light::{self, RaisedOpts};
use crate::kit::text::{Elide, Policy, Role, Src, Text};
use crate::kit::{self, band, Brush, Style, Tok};
use cantus_core::inst::analog::{condition_stop, CARDS, CARD_ROLL_KEYS};
use cantus_dsp::va::CardTrims;
use egui::{Align2, Pos2, Rect, Sense, Vec2};

/// The tokens the cards paint with, beside the hue (for inspect mode).
const TOKENS: &[Tok] = &[
    Tok::Panel,
    Tok::Deck,
    Tok::Text,
    Tok::TextDim,
    Tok::GridStrong,
    Tok::KnobTrack,
];

/// Every card's trims as the engine will play them.
pub fn all_trims(spec: &NativeSpec) -> [CardTrims; CARDS] {
    let s = cantus_engine::inst::analog::settings(spec);
    let v = s.card_variance();
    std::array::from_fn(|c| cantus_dsp::va::card_trims(s.seed, c, s.rolls[c], &v))
}

/// The Condition's named stop (Calibrated … Unruly), as an i18n key.
pub fn stop_key(spec: &NativeSpec) -> &'static str {
    condition_stop(spec.get("analog.condition"))
}

fn pan_text(pan: f32) -> String {
    if pan.abs() < 0.005 {
        "C".into()
    } else if pan < 0.0 {
        format!("L{:.0}", -pan * 100.0)
    } else {
        format!("R{:.0}", pan * 100.0)
    }
}

/// A card's trims in words and units, for the help line.
pub fn card_tip(tr: &CardTrims) -> String {
    format!(
        "{} {:+.1} / {:+.1} / {:+.1} ct \u{b7} {} {:+.2} st \u{b7} {} \u{d7}{:.3} \u{b7} {} {:+.1} % \u{b7} {} {}",
        t("calor.card.tuning"),
        tr.osc_cents[0],
        tr.osc_cents[1],
        tr.osc_cents[2],
        t("calor.vcf.cutoff"),
        tr.cutoff_st,
        t("calor.card.env"),
        tr.env_scale,
        t("calor.osc.pw"),
        tr.pw * 100.0,
        t("calor.card.pan"),
        pan_text(tr.pan),
    )
}

/// Where card `c` of the grid sits in `d`: `cols` across.
fn card_rect(d: Rect, c: usize, cols: usize, s: &Style) -> Rect {
    let rows = CARDS.div_ceil(cols);
    let pad = s.s(8.0);
    let gap = s.s(5.0);
    let inner = d.shrink(pad);
    let w = (inner.width() - gap * (cols - 1) as f32) / cols as f32;
    let h = (inner.height() - gap * (rows - 1) as f32) / rows as f32;
    let (col, row) = (c % cols, c / cols);
    Rect::from_min_size(
        Pos2::new(
            inner.left() + (w + gap) * col as f32,
            inner.top() + (h + gap) * row as f32,
        ),
        Vec2::new(w, h),
    )
}

/// The cards in `d` (a card's display): four across in Play, eight across with their trims
/// printed in Edit (`full`). `s` is in the cards' hue.
#[allow(clippy::too_many_arguments)]
pub fn grid(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    track: TrackId,
    spec: &NativeSpec,
    tel: &cantus_engine::Telemetry,
    d: Rect,
    s: &Style,
    full: bool,
) {
    let cols = if full { 8 } else { 4 };
    let detune = (spec.get("analog.detune") * 2.0 * spec.get("analog.condition")).max(0.5);
    let selected = app.inst.analog.selected_card;
    let all = all_trims(spec);
    let mut clicked = None;
    let mut rerolled = None;
    let mut hovered = None;
    for c in 0..CARDS {
        let r = card_rect(d, c, cols, s);
        let resp = ui.interact(r, ui.id().with(("calor-card", track.0, c)), Sense::click());
        if resp.hovered() {
            hovered = Some(c);
        }
        if resp.double_clicked() {
            rerolled = Some(c);
        } else if resp.clicked() {
            clicked = Some(c);
        }
    }
    kit::with_brush(&ui.painter_at(d), |b| {
        b.register(d, "calor.cards", TOKENS, &[s.hue_tok]);
        for (c, tr) in all.iter().enumerate() {
            let r = card_rect(d, c, cols, s);
            let look = Look {
                n: c + 1,
                level: tel[tel::CARD_LEVEL + c],
                key: tel[tel::CARD_KEY + c],
                selected: selected == Some(c),
                hovered: hovered == Some(c),
                detune,
                full,
            };
            paint_card(b, r, s, &look, tr);
        }
    });
    if let Some(c) = hovered {
        crate::kit::hint::set_text(
            ui.ctx(),
            &format!("{} {}", t("calor.card"), c + 1),
            &card_tip(&all[c]),
            crate::kit::hint::Caps::NONE,
        );
    }
    if let Some(c) = rerolled {
        reroll(app, track, spec, c);
    } else if let Some(c) = clicked {
        app.inst.analog.selected_card = if selected == Some(c) { None } else { Some(c) };
    }
}

/// What one card shows.
struct Look {
    n: usize,
    level: f32,
    /// The key it plays, plus one (0: none).
    key: f32,
    selected: bool,
    hovered: bool,
    /// The widest the cards can stray at this Condition, in cents.
    detune: f32,
    full: bool,
}

fn text_in(
    b: &mut Brush,
    r: Rect,
    text: &str,
    role: Role,
    color: egui::Color32,
    align: Align2,
    s: &Style,
) {
    let t = Text {
        src: Src::Str(text),
        role,
        caps: false,
        align,
        policy: Policy::Elide(Elide::End),
        color,
    };
    let _ = kit::text::paint(b, r, &t, s.tk());
}

fn paint_card(b: &mut Brush, r: Rect, s: &Style, look: &Look, tr: &CardTrims) {
    let t = &s.t;
    let lit = look.level > 0.0;
    let base = if t.dark { shade(t.panel, 0.02) } else { t.deck };
    let base = if look.hovered {
        shade(base, if t.dark { 0.04 } else { -0.03 })
    } else {
        base
    };
    light::raised(
        b,
        r,
        s.r(3.0),
        base,
        RaisedOpts {
            k: s.k,
            depth: t.depth,
            ..Default::default()
        },
    );
    if look.selected || lit {
        let a = if look.selected { 1.0 } else { 0.55 };
        b.rrect_stroke(
            b.snap_rect(r).shrink(b.px() * 0.5),
            s.r(3.0),
            if look.selected { s.s(1.5) } else { b.px() },
            with_alpha(s.hue, a),
        );
    }
    let pad = s.s(4.0);
    let lamp = Pos2::new(r.left() + pad + s.s(5.0), r.top() + pad + s.s(5.0));
    band::lamp(
        b,
        lamp,
        s,
        lit,
        s.hue,
        if lit {
            0.45 + 0.55 * look.level.min(1.0)
        } else {
            1.0
        },
    );
    let head = Rect::from_min_max(
        Pos2::new(lamp.x + s.s(8.0), r.top() + s.s(1.0)),
        Pos2::new(r.right() - pad, r.top() + s.s(19.0)),
    );
    let mut n = String::with_capacity(3);
    let _ = std::fmt::Write::write_fmt(&mut n, format_args!("{}", look.n));
    text_in(
        b,
        head,
        &n,
        Role::Micro,
        t.text_dim,
        Align2::RIGHT_CENTER,
        s,
    );
    if look.key > 0.0 {
        let name = cantus_core::pitch_name((look.key - 1.0) as u8);
        text_in(b, head, &name, Role::Label, t.text, Align2::LEFT_CENTER, s);
    }
    // Three bars: each oscillator's tuning offset, up sharp and down flat, against the
    // widest the cards can stray at this Condition.
    let (mid, half) = if look.full {
        (r.top() + s.s(30.0), s.s(8.0))
    } else {
        (
            r.bottom() - s.s(10.0),
            (r.bottom() - head.bottom() - s.s(12.0)).clamp(s.s(3.0), s.s(8.0)),
        )
    };
    let inner = r.shrink2(Vec2::new(s.s(7.0), 0.0));
    b.hline(
        inner.left(),
        inner.right(),
        b.snap_mid(mid),
        b.px(),
        with_alpha(t.grid_strong, 0.8),
    );
    let bw = inner.width() / 3.0;
    for (i, cents) in tr.osc_cents.iter().enumerate() {
        let cx = inner.left() + bw * (i as f32 + 0.5);
        let len = (cents / look.detune).clamp(-1.0, 1.0) * half;
        let (y0, y1) = (mid.min(mid - len), mid.max(mid - len));
        let bar = Rect::from_min_max(
            Pos2::new(cx - s.s(2.5), y0),
            Pos2::new(cx + s.s(2.5), y1.max(y0 + s.s(1.5))),
        );
        b.rect(b.snap_rect(bar), crate::kit::Fill::Solid(s.hue));
    }
    if look.full {
        let lines = [
            format!("{:+.2} st", tr.cutoff_st),
            format!("\u{d7}{:.2}", tr.env_scale),
            pan_text(tr.pan),
        ];
        let lh = s.s(11.0);
        for (i, l) in lines.iter().enumerate() {
            let lr = Rect::from_min_size(
                Pos2::new(r.left() + pad, mid + half + s.s(3.0) + lh * i as f32),
                Vec2::new(r.width() - 2.0 * pad, lh),
            );
            if lr.bottom() <= r.bottom() - s.s(1.0) {
                text_in(b, lr, l, Role::Micro, t.text_dim, Align2::LEFT_CENTER, s);
            }
        }
    }
}

/// Draw card `c` again: its re-roll number moves on (one undo step).
pub fn reroll(app: &mut CantusApp, track: TrackId, spec: &NativeSpec, c: usize) {
    let key = CARD_ROLL_KEYS[c];
    let next = (spec.get(key) + 1.0) % 10_000.0;
    app.set_inst_value(track, key, next.max(1.0), false);
}

/// The Condition's stop and the seed, in `r` (a slot): the stop in the hue, the seed dim.
pub fn condition_words(ui: &egui::Ui, r: Rect, spec: &NativeSpec, s: &Style) {
    let stop = t(stop_key(spec));
    let seed = format!("{} #{:.0}", t("calor.seed"), spec.get("analog.seed"));
    kit::with_brush(&ui.painter_at(r), |b| {
        let h = r.height() * 0.5;
        let top = Rect::from_min_size(r.min, Vec2::new(r.width(), h));
        let t1 = Text {
            src: Src::Str(&stop),
            role: Role::Section,
            caps: true,
            align: Align2::LEFT_BOTTOM,
            policy: Policy::Elide(Elide::End),
            color: s.hue,
        };
        let _ = kit::text::paint(b, top, &t1, s.tk());
        let bottom = Rect::from_min_max(Pos2::new(r.left(), r.top() + h), r.max);
        text_in(
            b,
            bottom,
            &seed,
            Role::Micro,
            s.t.text_dim,
            Align2::LEFT_CENTER,
            s,
        );
    });
}

/// The card tools in `r` (a slot): re-roll all (a new seed), calibrate (Condition to
/// zero), and the selected card's re-roll and restore.
pub fn tools(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    track: TrackId,
    spec: &NativeSpec,
    r: Rect,
    s: &Style,
) {
    let gap = s.s(6.0);
    let selected = app.inst.analog.selected_card;
    let n = if selected.is_some() { 4.0 } else { 2.0 };
    let w = ((r.width() - gap * (n - 1.0)) / n).min(s.s(120.0));
    let at = |i: usize| {
        Rect::from_min_size(
            Pos2::new(r.left() + (w + gap) * i as f32, r.center().y - s.s(10.0)),
            Vec2::new(w, s.s(20.0)),
        )
    };
    if parts::push_at(ui, at(0), s, "calor.reroll_all", "calor.reroll_all_tip").clicked() {
        let seed = spec.get("analog.seed");
        app.set_inst_value(track, "analog.seed", (seed + 1.0) % 65_536.0, false);
    }
    if parts::push_at(ui, at(1), s, "calor.calibrate", "calor.calibrate_tip").clicked() {
        app.set_inst_value(track, "analog.condition", 0.0, false);
    }
    if let Some(c) = selected {
        if parts::push_at(ui, at(2), s, "calor.reroll_card", "calor.reroll_card_tip").clicked() {
            reroll(app, track, spec, c);
        }
        if spec.is_stored(CARD_ROLL_KEYS[c])
            && parts::push_at(ui, at(3), s, "calor.restore_card", "calor.restore_card_tip")
                .clicked()
        {
            app.exec(Command::ClearInstrumentValue {
                track,
                key: CARD_ROLL_KEYS[c].into(),
            });
        }
    }
}
