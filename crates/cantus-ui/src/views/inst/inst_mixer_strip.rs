//! One strip of an instrument's mixer, drawn as Cantus's mixer draws a Wide strip (des-mixer:
//! the body lit from above, the groove, ticks and cap on the fader's law, the meter sharing
//! it, the keys, the Out well) and sensed: top to bottom its name and kind, the top cell
//! (bleed, width or level), ø and pan, the delay, M and S, the fader and meter, the value,
//! the Out cell.

use super::ui::{self as iu, words};
use super::{db_text, Edit, Strip, Top};
use crate::kit::color::mix;
use crate::kit::glyph_mixer::{Ink, MGlyph};
use crate::kit::knob::{self, KnobLook, KnobSize};
use crate::kit::{with_brush, Brush, Fill, State, Style};
use crate::views::mixer::look::{self, law, Bars, Pal, W};
use egui::{pos2, vec2, Align2, Rect, Response, Sense, Ui};

/// The parts' tops within a strip, at 100 %.
const NAME: f32 = 6.0;
const TOP_CELL: f32 = 40.0;
const PAN_ROW: f32 = 64.0;
const DELAY_ROW: f32 = 90.0;
const KEYS: f32 = 112.0;
const FADER: f32 = 138.0;
/// The delay's range, ms (`o.delay`'s).
pub const DELAY_MAX: f32 = 50.0;
/// What the foot keeps for the value and the Out cell.
const FOOT: f32 = 64.0;

/// Where a strip's parts are.
#[derive(Copy, Clone, Debug)]
pub struct Parts {
    pub name: Rect,
    pub top: Rect,
    pub invert: Rect,
    pub pan: Rect,
    pub delay: Rect,
    pub mute: Rect,
    pub solo: Rect,
    /// The fader's travel (top and bottom of the groove's run) and the meter's x.
    pub travel: Rect,
    pub value: Rect,
    pub out: Rect,
}

/// The parts of a strip in `r`.
pub fn parts(r: Rect, s: &Style) -> Parts {
    let k = s.k;
    let x = |v: f32| r.left() + v * k;
    let y = |v: f32| r.top() + v * k;
    let w = r.width();
    let inset = 6.0 * k;
    let half = (w - 2.0 * inset - 4.0 * k) * 0.5;
    let foot = r.bottom() - FOOT * k;
    Parts {
        name: Rect::from_min_max(
            pos2(r.left() + 1.0 * k, y(NAME)),
            pos2(r.right() - 1.0 * k, y(34.0)),
        ),
        top: Rect::from_min_max(
            pos2(r.left() + inset, y(TOP_CELL)),
            pos2(r.right() - inset, y(TOP_CELL + 18.0)),
        ),
        invert: Rect::from_center_size(
            pos2(x(w / k * 0.3), y(PAN_ROW + 11.0)),
            vec2(20.0 * k, 20.0 * k),
        ),
        pan: Rect::from_center_size(
            pos2(x(w / k * 0.68), y(PAN_ROW + 11.0)),
            vec2(24.0 * k, 24.0 * k),
        ),
        delay: Rect::from_min_max(
            pos2(r.left() + inset, y(DELAY_ROW)),
            pos2(r.right() - inset, y(DELAY_ROW + 16.0)),
        ),
        mute: Rect::from_min_size(pos2(r.left() + inset, y(KEYS)), vec2(half, 15.0 * k)),
        solo: Rect::from_min_size(
            pos2(r.left() + inset + half + 4.0 * k, y(KEYS)),
            vec2(half, 15.0 * k),
        ),
        travel: Rect::from_min_max(
            pos2(r.left(), y(FADER + 8.0)),
            pos2(r.right(), foot - 22.0 * k),
        ),
        value: Rect::from_min_max(pos2(r.left(), foot - 18.0 * k), pos2(r.right(), foot)),
        out: Rect::from_min_max(
            pos2(r.left() + inset * 0.5, foot + 4.0 * k),
            pos2(r.right() - inset * 0.5, r.bottom() - 4.0 * k),
        ),
    }
}

/// Draw `st` in `r` and sense it; what was done comes back as edits. `salt` keeps its ids
/// apart from every other strip's.
pub fn show(ui: &Ui, s: &Style, r: Rect, st: &Strip, salt: egui::Id) -> Vec<Edit> {
    let mut out = Vec::new();
    let p = Pal::of(s);
    let parts = parts(r, s);
    let body = ui.interact(r, salt.with("body"), Sense::click());
    if body.clicked() {
        out.push(Edit::Select);
    }
    with_brush(ui.painter(), |b| {
        let flat = st.selected.then(|| p.selected(p.accent));
        look::body(b, r, &p, flat);
        if st.selected {
            look::stripe(b, r, p.accent);
        }
    });
    // The name and its kind.
    let dim = !st.loaded;
    with_brush(ui.painter(), |b| {
        let ink = if dim { p.ink3 } else { p.ink1 };
        let (nr, sr) = (
            Rect::from_min_max(
                parts.name.min,
                pos2(parts.name.right(), parts.name.top() + 15.0 * s.k),
            ),
            Rect::from_min_max(
                pos2(parts.name.left(), parts.name.top() + 15.0 * s.k),
                parts.name.max,
            ),
        );
        let size = [12.0, 11.0, 10.0, 9.5]
            .into_iter()
            .find(|z| look::width_of(b.ctx(), s, &st.name, *z, W::SemiBold) <= nr.width() - 1.0)
            .unwrap_or(9.5);
        words(
            b,
            s,
            &st.name,
            nr,
            Align2::CENTER_CENTER,
            size,
            W::SemiBold,
            ink,
        );
        let sub_ink = if st.selected { p.ink2 } else { p.ink3 };
        words(
            b,
            s,
            &st.sub,
            sr,
            Align2::CENTER_CENTER,
            10.0,
            W::Regular,
            sub_ink,
        );
    });
    top_cell(ui, s, &p, parts.top, st, salt, &mut out);
    if let Some(inv) = st.invert {
        let resp = ui.interact(parts.invert, salt.with("inv"), Sense::click());
        if resp.clicked() {
            out.push(Edit::Invert);
        }
        let resp = iu::hint(resp, || crate::i18n::t("inst_mixer.invert_tip"));
        with_brush(ui.painter(), |b| {
            let ground = if inv { p.accent } else { p.surface };
            if inv {
                b.rrect(
                    b.snap_rect(parts.invert.shrink(2.0 * s.k)),
                    3.0 * s.k,
                    Fill::Solid(mix(p.surface, p.accent, 0.35)),
                );
            }
            let ink = if resp.hovered() {
                Ink::Hover
            } else {
                Ink::Rest
            };
            look::glyph(
                b,
                s,
                parts.invert.center(),
                12.0 * s.k,
                MGlyph::Phase,
                ink,
                ground,
            );
        });
    }
    if let Some(pan) = st.pan {
        pan_knob(ui, s, parts.pan, pan, salt, &mut out);
    }
    if let Some(ms) = st.delay {
        if let Some(o) = st.id {
            ui.ctx()
                .data_mut(|d| d.insert_temp(delay_rect_id(o), parts.delay));
        }
        delay_cell(ui, s, &p, parts.delay, ms, salt, &mut out);
    }
    if let Some(on) = st.mute {
        key(
            ui,
            s,
            &p,
            parts.mute,
            "M",
            on,
            p.mute_on,
            salt.with("m"),
            "inst_mixer.mute_tip",
            Edit::Mute,
            &mut out,
        );
    }
    if let Some(on) = st.solo {
        key(
            ui,
            s,
            &p,
            parts.solo,
            "S",
            on,
            p.solo,
            salt.with("s"),
            "inst_mixer.solo_tip",
            Edit::Solo,
            &mut out,
        );
    }
    foot(ui, s, &p, &parts, st, salt, &mut out);
    out
}

/// The fader and meter, the value and the Out cell of a strip whose other parts are drawn
/// by its instrument (Tympanum's *Ambience*): `r` runs from the fader's top to the foot.
pub fn lower(ui: &Ui, s: &Style, r: Rect, st: &Strip, salt: egui::Id) -> Vec<Edit> {
    let mut out = Vec::new();
    let p = Pal::of(s);
    let k = s.k;
    let foot_y = r.bottom() - FOOT * k;
    let parts = Parts {
        travel: Rect::from_min_max(
            pos2(r.left(), r.top() + 8.0 * k),
            pos2(r.right(), foot_y - 22.0 * k),
        ),
        value: Rect::from_min_max(pos2(r.left(), foot_y - 18.0 * k), pos2(r.right(), foot_y)),
        out: Rect::from_min_max(
            pos2(r.left() + 3.0 * k, foot_y + 4.0 * k),
            pos2(r.right() - 3.0 * k, r.bottom() - 4.0 * k),
        ),
        ..parts(r, s)
    };
    foot(ui, s, &p, &parts, st, salt, &mut out);
    out
}

fn foot(
    ui: &Ui,
    s: &Style,
    p: &Pal,
    parts: &Parts,
    st: &Strip,
    salt: egui::Id,
    out: &mut Vec<Edit>,
) {
    fader(ui, s, p, parts.travel, st, salt, out);
    with_brush(ui.painter(), |b| {
        let ink = if st.loaded { p.ink1 } else { p.ink3 };
        words(
            b,
            s,
            &db_text(st.level_db),
            parts.value,
            Align2::CENTER_CENTER,
            11.0,
            W::SemiBold,
            ink,
        );
    });
    out_cell(ui, s, p, parts.out, st, salt, out);
}

fn top_cell(ui: &Ui, s: &Style, p: &Pal, r: Rect, st: &Strip, salt: egui::Id, out: &mut Vec<Edit>) {
    match &st.top {
        Top::None => {}
        Top::Words(w) => {
            with_brush(ui.painter(), |b| {
                look::well(b, r, p, false);
                words(b, s, w, r, Align2::CENTER_CENTER, 10.0, W::Regular, p.ink3);
            });
        }
        Top::Share { word, value, full } => {
            let resp = ui.interact(r, salt.with("top"), Sense::click_and_drag());
            if resp.dragged() || resp.drag_started() {
                if let Some(q) = resp.interact_pointer_pos() {
                    let f = ((q.x - r.left()) / r.width()).clamp(0.0, 1.0);
                    out.push(Edit::Top(f));
                }
            }
            if resp.drag_stopped() {
                out.push(Edit::End);
            }
            let pct = format!("{:.0} %", value * 100.0 / full.max(1e-6));
            let resp = iu::hint(resp, || format!("{word} \u{b7} {pct}"));
            with_brush(ui.painter(), |b| {
                look::well(b, r, p, resp.hovered());
                let inner = r.shrink2(vec2(5.0 * s.k, 0.0));
                // The word and its share when both fit apart ("Anchura 70 %" does not at
                // 100 %): else the share alone, the help line naming it.
                let ww = look::width_of(b.ctx(), s, word, 10.0, W::Regular);
                let pw = look::width_of(b.ctx(), s, &pct, 10.0, W::Medium);
                if ww + 2.0 * s.k + pw <= inner.width() {
                    words(
                        b,
                        s,
                        word,
                        inner,
                        Align2::LEFT_CENTER,
                        10.0,
                        W::Regular,
                        p.ink2,
                    );
                    words(
                        b,
                        s,
                        &pct,
                        inner,
                        Align2::RIGHT_CENTER,
                        10.0,
                        W::Medium,
                        p.ink1,
                    );
                } else {
                    words(
                        b,
                        s,
                        &pct,
                        inner,
                        Align2::CENTER_CENTER,
                        10.0,
                        W::Medium,
                        p.ink1,
                    );
                }
                let f = value / full.max(1e-6);
                let bar = Rect::from_min_size(
                    pos2(r.left() + 3.0 * s.k, r.bottom() - 3.0 * s.k),
                    vec2((r.width() - 6.0 * s.k) * f.clamp(0.0, 1.0), 1.5 * s.k),
                );
                b.rect(b.snap_rect(bar), Fill::Solid(p.ink2));
            });
        }
    }
}

fn pan_knob(ui: &Ui, s: &Style, r: Rect, pan: f32, salt: egui::Id, out: &mut Vec<Edit>) {
    let resp = ui.interact(r, salt.with("pan"), Sense::click_and_drag());
    let mut pos = (pan.clamp(-1.0, 1.0) + 1.0) * 0.5;
    if resp.dragged() {
        let dy = resp.drag_delta().y;
        let fine = if ui.input(|i| i.modifiers.shift) {
            0.2
        } else {
            1.0
        };
        pos = (pos - dy / 200.0 * fine).clamp(0.0, 1.0);
        out.push(Edit::Pan(pos * 2.0 - 1.0));
    }
    if resp.double_clicked() {
        out.push(Edit::Pan(0.0));
        out.push(Edit::End);
    }
    if resp.drag_stopped() {
        out.push(Edit::End);
    }
    let resp = iu::hint(resp, || {
        let mut w = String::new();
        pan_words(&mut w, pos * 2.0 - 1.0);
        crate::i18n::tf("inst_mixer.pan_tip", &[("v", &w)])
    });
    let st = State::of(&resp, false);
    with_brush(ui.painter(), |b| {
        let ks = s.scaled(s.k * 9.0 / 10.0);
        knob::paint(
            b,
            r.center(),
            &ks,
            st,
            &KnobLook::new(KnobSize::Mini, pos, 0.5),
        );
    });
}

fn delay_rect_id(o: cantus_core::OutputId) -> egui::Id {
    egui::Id::new(("inst-mixer-delay", o.0))
}

/// Where output `o`'s delay cell was last drawn (tests aim at it).
pub fn delay_rect(ctx: &egui::Context, o: cantus_core::OutputId) -> Option<Rect> {
    ctx.data(|d| d.get_temp(delay_rect_id(o)))
}

/// "0 ms", "1.5 ms", "12.0 ms": a delay as the cell says it.
pub fn delay_words(ms: f32) -> String {
    if ms.abs() < 0.005 {
        "0 ms".to_string()
    } else {
        format!("{ms:.1} ms")
    }
}

/// A typed delay: "1.5", "1,5", "1.5 ms" → ms within the line, or nothing.
pub fn parse_delay(words: &str) -> Option<f32> {
    let w = words.trim().trim_end_matches("ms").trim().replace(',', ".");
    let v: f32 = w.parse().ok()?;
    v.is_finite().then(|| v.clamp(0.0, DELAY_MAX))
}

/// The delay cell, under ø and pan: a well with *Delay* and its ms. Dragged sideways it
/// moves 0.05 ms a point (Shift 0.01, fine enough for a sample at 48 kHz); a
/// double-click types a value (Enter keeps it, Esc leaves it).
fn delay_cell(ui: &Ui, s: &Style, p: &Pal, r: Rect, ms: f32, salt: egui::Id, out: &mut Vec<Edit>) {
    let id = salt.with("delay");
    let typing_id = id.with("typing");
    let typing: Option<String> = ui.ctx().data(|d| d.get_temp(typing_id));
    if let Some(mut words_) = typing {
        let mut event = None;
        egui::Area::new(id.with("area"))
            .order(egui::Order::Foreground)
            .fixed_pos(r.min)
            .show(ui.ctx(), |ui| {
                let (line, _) = ui.allocate_exact_size(r.size(), Sense::hover());
                let o = crate::kit::field::Field::new(id.with("field"))
                    .scale(s.k)
                    .centered()
                    .opens_when_shown()
                    .show(ui, line, &mut words_);
                event = o.event;
            });
        match event {
            Some(crate::kit::field::Event::Commit) => {
                if let Some(v) = parse_delay(&words_) {
                    out.push(Edit::Delay(v));
                    out.push(Edit::End);
                }
                ui.ctx().data_mut(|d| {
                    d.remove::<String>(typing_id);
                });
            }
            Some(crate::kit::field::Event::Cancel) => {
                ui.ctx().data_mut(|d| {
                    d.remove::<String>(typing_id);
                });
            }
            None => {
                ui.ctx().data_mut(|d| {
                    d.insert_temp(typing_id, words_);
                });
            }
        }
        return;
    }
    let resp = ui.interact(r, id, Sense::click_and_drag());
    let mut v = ms;
    if resp.dragged() {
        let per = if ui.input(|i| i.modifiers.shift) {
            0.01
        } else {
            0.05
        };
        let dx = resp.drag_delta().x / s.k;
        if dx != 0.0 {
            v = (ms + dx * per).clamp(0.0, DELAY_MAX);
            out.push(Edit::Delay(v));
        }
    }
    if resp.drag_stopped() {
        out.push(Edit::End);
    }
    if resp.double_clicked() {
        ui.ctx()
            .data_mut(|d| d.insert_temp(typing_id, format!("{ms:.2}")));
    }
    let value = delay_words(v);
    let resp = iu::hint(resp, || {
        crate::i18n::tf("inst_mixer.delay_tip", &[("v", &value)])
    });
    with_brush(ui.painter(), |b| {
        look::well(b, r, p, resp.hovered() || resp.dragged());
        let inner = r.shrink2(vec2(5.0 * s.k, 0.0));
        let word = crate::i18n::t("inst_mixer.delay");
        let ww = look::width_of(b.ctx(), s, &word, 10.0, W::Regular);
        let vw = look::width_of(b.ctx(), s, &value, 10.0, W::Medium);
        let lit = if v > 0.0 { p.ink1 } else { p.ink2 };
        if ww + 4.0 * s.k + vw <= inner.width() {
            words(
                b,
                s,
                &word,
                inner,
                Align2::LEFT_CENTER,
                10.0,
                W::Regular,
                p.ink3,
            );
            words(
                b,
                s,
                &value,
                inner,
                Align2::RIGHT_CENTER,
                10.0,
                W::Medium,
                lit,
            );
        } else {
            words(
                b,
                s,
                &value,
                inner,
                Align2::CENTER_CENTER,
                10.0,
                W::Medium,
                lit,
            );
        }
    });
}

/// "C", "16 L", "40 R".
pub fn pan_words(w: &mut String, pan: f32) {
    use std::fmt::Write as _;
    let v = (pan * 100.0).round();
    if v.abs() < 0.5 {
        w.push('C');
    } else if v < 0.0 {
        let _ = write!(w, "{:.0} L", -v);
    } else {
        let _ = write!(w, "{v:.0} R");
    }
}

#[allow(clippy::too_many_arguments)]
fn key(
    ui: &Ui,
    s: &Style,
    p: &Pal,
    r: Rect,
    letter: &str,
    on: bool,
    lit: egui::Color32,
    id: egui::Id,
    tip: &'static str,
    edit: Edit,
    out: &mut Vec<Edit>,
) {
    let resp = ui.interact(r, id, Sense::click());
    if resp.clicked() {
        out.push(edit);
    }
    let resp = iu::hint(resp, || crate::i18n::t(tip));
    with_brush(ui.painter(), |b| {
        let rr = b.snap_rect(r);
        if on {
            b.rrect(rr, 2.0 * s.k, Fill::Solid(lit));
        } else {
            look::well(b, rr, p, resp.hovered());
        }
        let ink = if on { p.on_ink } else { p.ink2 };
        words(
            b,
            s,
            letter,
            rr,
            Align2::CENTER_CENTER,
            10.0,
            W::SemiBold,
            ink,
        );
    });
}

fn fader(
    ui: &Ui,
    s: &Style,
    p: &Pal,
    travel: Rect,
    st: &Strip,
    salt: egui::Id,
    out: &mut Vec<Edit>,
) {
    let k = s.k;
    let w = travel.width();
    let gx = travel.left() + w * 0.36;
    let mx = travel.left() + w * 0.6;
    let (top, bot) = (travel.top(), travel.bottom());
    let hit = Rect::from_min_max(
        pos2(travel.left() + 4.0 * k, top - 8.0 * k),
        pos2(mx - 2.0 * k, bot + 8.0 * k),
    );
    let resp: Response = ui.interact(hit, salt.with("fader"), Sense::click_and_drag());
    let mut db = st.level_db;
    if resp.dragged() || resp.drag_started() {
        let f0 = law::frac(db);
        let dy = resp.drag_delta().y;
        let fine = if ui.input(|i| i.modifiers.shift) {
            0.2
        } else {
            1.0
        };
        let f = (f0 + dy * fine / (bot - top).max(1.0)).clamp(0.0, 1.0);
        db = law::db_at(f).min(st.level_max);
        if dy != 0.0 {
            out.push(Edit::Level(db));
        }
    }
    if resp.double_clicked() {
        db = 0.0;
        out.push(Edit::Level(0.0));
        out.push(Edit::End);
    }
    if resp.drag_stopped() {
        out.push(Edit::End);
    }
    let resp = iu::hint(resp, || {
        crate::i18n::tf(
            "inst_mixer.fader_tip",
            &[("name", &st.name), ("db", &db_text(db))],
        )
    });
    with_brush(ui.painter(), |b: &mut Brush| {
        look::groove(b, gx, top, bot, p);
        look::ticks(b, gx + 9.0 * k, top, bot, p);
        let y = top + (bot - top) * law::frac(db);
        let centre = if st.selected { p.accent } else { p.ink1 };
        look::fader_cap(
            b,
            gx,
            y,
            vec2(22.0 * k, 13.0 * k),
            p,
            centre,
            resp.hovered() || resp.dragged(),
            crate::kit::fader::Host::InstMixer,
        );
        let bars = st.meter.map(|m| Bars { level: m, hold: m });
        look::meter(
            b,
            mx,
            top,
            bot,
            4.0 * k,
            bars,
            false,
            st.mute == Some(true) || !st.loaded,
            p,
        );
    });
}

fn out_cell(ui: &Ui, s: &Style, p: &Pal, r: Rect, st: &Strip, salt: egui::Id, out: &mut Vec<Edit>) {
    let sense = if st.out.opens {
        Sense::click()
    } else {
        Sense::hover()
    };
    let resp = ui.interact(r, salt.with("out"), sense);
    if resp.clicked() {
        out.push(Edit::Out(r));
    }
    let tip = st.out.tip.clone();
    let resp = iu::hint(resp, || tip);
    with_brush(ui.painter(), |b| {
        look::well(b, r, p, resp.hovered() && st.out.opens);
        let pad = 5.0 * s.k;
        let line1 = Rect::from_min_max(
            pos2(r.left() + pad, r.top() + 2.0 * s.k),
            pos2(r.right() - pad, r.center().y),
        );
        let line2 = Rect::from_min_max(
            pos2(r.left() + pad, r.center().y - 1.0 * s.k),
            pos2(r.right() - pad, r.bottom() - 2.0 * s.k),
        );
        words(
            b,
            s,
            &crate::i18n::t("inst_mixer.out"),
            line1,
            Align2::LEFT_CENTER,
            10.0,
            W::Regular,
            p.ink3,
        );
        let mut right = line1.right();
        if st.out.opens {
            crate::kit::well::chevron(
                b,
                pos2(right - 3.0 * s.k, line1.center().y),
                6.0 * s.k,
                3.5 * s.k,
                false,
                1.2 * s.k,
                p.ink2,
            );
            right -= 10.0 * s.k;
        }
        if let Some(n) = st.out.number {
            let nr =
                Rect::from_min_max(pos2(line1.left(), line1.top()), pos2(right, line1.bottom()));
            words(
                b,
                s,
                &n.to_string(),
                nr,
                Align2::RIGHT_CENTER,
                10.0,
                W::Medium,
                p.ink2,
            );
        }
        words(
            b,
            s,
            &st.out.label,
            line2,
            Align2::LEFT_CENTER,
            12.0,
            W::Medium,
            p.ink1,
        );
    });
}

/// The meters of a folded family, side by side in `r` (each a single bar), on the law.
pub(crate) fn meters(b: &mut Brush, s: &Style, p: &Pal, r: Rect, levels: &[Option<[f32; 2]>]) {
    let n = levels.len().max(1) as f32;
    let bw = (4.0 * s.k)
        .min((r.width() - 2.0 * s.k * n) / (2.0 * n))
        .max(1.0);
    let span = n * (2.0 * bw + 1.0) + (n - 1.0) * 3.0 * s.k;
    let mut x = r.center().x - span * 0.5;
    for m in levels {
        let bars = m.map(|m| Bars { level: m, hold: m });
        look::meter(b, x, r.top(), r.bottom(), bw, bars, false, false, p);
        x += 2.0 * bw + 1.0 + 3.0 * s.k;
    }
}
