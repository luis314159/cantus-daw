//! Under the picture: the row of pieces (each with its key: a click selects and plays it,
//! Alt-click plays its articulations in turn) and the kit's three cards — *Feel* (timing,
//! strength, never the same hit twice), *Hi-hat pedal* (the openness on its controller, the
//! four openings to hear, the foot's chick) and *Kit* (every piece's tune, ring and level).
//! Plus the controls they and the piece's panel are made of: a knob cell, a value row, a
//! switch row — each bound to an instrument key, one command a gesture.

use super::iu::{self, words};
use super::{def, press, Look};
use crate::app::CantusApp;
use crate::i18n::{t, tf};
use crate::kit::color::mix;
use crate::kit::knob::{self, KnobLook, KnobSize};
use crate::kit::{with_brush, Fill, State, Style};
use crate::views::mixer::look::{Pal, W};
use cantus_core::inst::tympanum_kit::{KitDef, PieceKind};
use cantus_core::{NativeSpec, TrackId};
use egui::{pos2, vec2, Align2, Rect, Sense};

/// The cards' height and the pieces row's, at 100 %.
pub const HEIGHT: f32 = 140.0;
pub const ROW: f32 = 40.0;
/// A control's number: an instrument key, or — until the engine has that key — the value
/// the design shows, drawn as it will be (`tymp-engine` adds the key; the integrator was
/// asked). `scale` maps the key's value to what is shown (100 for a share as %).
#[derive(Copy, Clone, Debug)]
pub struct Num {
    pub min: f32,
    pub max: f32,
    pub default: f32,
    pub unit: &'static str,
    pub scale: f32,
    /// Shown with a sign (± for a spread, + for a tune).
    pub signed: bool,
    pub plus_minus: bool,
}

impl Num {
    pub const fn new(min: f32, max: f32, default: f32, unit: &'static str) -> Num {
        Num {
            min,
            max,
            default,
            unit,
            scale: 1.0,
            signed: false,
            plus_minus: false,
        }
    }
    pub const fn pct(mut self) -> Num {
        self.scale = 100.0;
        self
    }
    pub const fn signed(mut self) -> Num {
        self.signed = true;
        self
    }
    pub const fn pm(mut self) -> Num {
        self.plus_minus = true;
        self
    }
    pub fn value(&self, spec: &NativeSpec, key: &str) -> f32 {
        if spec.kind.param(key).is_some() {
            spec.get(key)
        } else {
            self.default
        }
    }
    /// The number and its unit apart ("+0.5", "st").
    pub fn text(&self, v: f32) -> (String, String) {
        let x = v * self.scale;
        let decimals = if self.unit == "st" || self.unit == "dB" {
            1
        } else {
            0
        };
        let mag = format!("{:.*}", decimals, x.abs());
        let shows_zero = mag.chars().all(|c| c == '0' || c == '.');
        let n = if self.plus_minus {
            format!("\u{b1}{mag}")
        } else if x < 0.0 && !shows_zero {
            format!("\u{2212}{mag}")
        } else if self.signed {
            format!("+{mag}")
        } else {
            mag
        };
        (n, self.unit.to_string())
    }
}

/// A knob with its name above and its value under it, `key` bound (`num` says its range
/// and look when the key is not there yet). One undo step a drag; double-click: default.
#[allow(clippy::too_many_arguments)]
pub fn knob_cell(
    app: &mut CantusApp,
    ui: &egui::Ui,
    track: TrackId,
    spec: &NativeSpec,
    s: &Style,
    r: Rect,
    key: &str,
    label: &str,
    num: Num,
) {
    let p = Pal::of(s);
    let bound = spec.kind.param(key).is_some();
    let (min, max, default) = match spec.kind.param(key) {
        Some(i) => (i.min, i.max, i.default),
        None => (num.min, num.max, num.default),
    };
    let v = num.value(spec, key);
    let pos = ((v - min) / (max - min).max(1e-6)).clamp(0.0, 1.0);
    let kc = pos2(r.center().x, r.top() + s.s(36.0));
    let hit = Rect::from_center_size(kc, vec2(s.s(34.0), s.s(34.0)));
    let resp = ui.interact(
        hit,
        ui.id().with(("tymp-knob", track.0, key)),
        Sense::click_and_drag(),
    );
    // The parameter's menu — automate, modulate, *Learn MIDI controller*, reset — and the
    // learn mode's outline, as every instrument's knob has (ADR 0062); while the learn mode
    // has it, the knob takes no value from the click.
    let learning = bound && crate::controllers::stepped_inst(app, ui, track, spec, key, &resp);
    if bound && !learning {
        if resp.dragged() {
            let fine = if ui.input(|i| i.modifiers.shift) {
                0.2
            } else {
                1.0
            };
            let np = (pos - resp.drag_delta().y / 200.0 * fine).clamp(0.0, 1.0);
            app.set_inst_value(track, key, min + np * (max - min), true);
        }
        if resp.drag_stopped() {
            app.end_gesture();
        }
        if resp.double_clicked() {
            app.set_inst_value(track, key, default, false);
        }
    }
    let (n, u) = num.text(v);
    let resp = iu::hint(resp, || format!("{label} \u{b7} {n} {u}"));
    let st = State::of(&resp, false);
    with_brush(ui.painter(), |b| {
        let lr = Rect::from_min_size(r.min, vec2(r.width(), s.s(16.0)));
        words(
            b,
            s,
            label,
            lr,
            Align2::CENTER_CENTER,
            11.0,
            W::Regular,
            p.ink2,
        );
        let origin = ((default - min) / (max - min).max(1e-6)).clamp(0.0, 1.0);
        knob::paint(b, kc, s, st, &KnobLook::new(KnobSize::S, pos, origin));
        let vr = Rect::from_min_size(pos2(r.left(), kc.y + s.s(20.0)), vec2(r.width(), s.s(18.0)));
        value_words(b, s, &p, vr, &n, &u, Align2::CENTER_CENTER);
    });
}

/// A number SemiBold 12 in ink 1 and its unit Regular 10 in ink 3 after it, aligned in `r`.
pub fn value_words(
    b: &mut crate::kit::Brush,
    s: &Style,
    p: &Pal,
    r: Rect,
    n: &str,
    u: &str,
    align: Align2,
) {
    let nw = crate::views::mixer::look::width_of(b.ctx(), s, n, 12.0, W::SemiBold);
    let uw = if u.is_empty() {
        0.0
    } else {
        crate::views::mixer::look::width_of(b.ctx(), s, u, 10.0, W::Regular)
    };
    let total = nw + uw;
    let x0 = match align.x() {
        egui::Align::Min => r.left(),
        egui::Align::Center => r.center().x - total * 0.5,
        egui::Align::Max => r.right() - total,
    };
    let nr = Rect::from_min_max(pos2(x0, r.top()), pos2(x0 + nw + 1.0, r.bottom()));
    words(b, s, n, nr, Align2::LEFT_CENTER, 12.0, W::SemiBold, p.ink1);
    if uw > 0.0 {
        let ur = Rect::from_min_max(
            pos2(nr.right() + s.s(2.0), r.top()),
            pos2(x0 + total + s.s(6.0), r.bottom()),
        );
        words(b, s, u, ur, Align2::LEFT_CENTER, 10.0, W::Regular, p.ink3);
    }
}

/// A row: its name at the left and its value at the right over a slim bar the row's width
/// (des-controls' dense rows), `key` bound; a drag along the bar moves it.
#[allow(clippy::too_many_arguments)]
pub fn bar_row(
    app: &mut CantusApp,
    ui: &egui::Ui,
    track: TrackId,
    spec: &NativeSpec,
    s: &Style,
    r: Rect,
    key: &str,
    label: &str,
    num: Num,
) {
    let p = Pal::of(s);
    let bound = spec.kind.param(key).is_some();
    let (min, max) = match spec.kind.param(key) {
        Some(i) => (i.min, i.max),
        None => (num.min, num.max),
    };
    let v = num.value(spec, key);
    let pos = ((v - min) / (max - min).max(1e-6)).clamp(0.0, 1.0);
    let bar = Rect::from_min_max(
        pos2(r.left(), r.bottom() - s.s(7.0)),
        pos2(r.left() + r.width() * 0.62, r.bottom() - s.s(4.0)),
    );
    let hit = Rect::from_min_max(pos2(r.left(), r.top()), pos2(r.right(), r.bottom()));
    let resp = ui.interact(
        hit,
        ui.id().with(("tymp-bar", track.0, key)),
        Sense::click_and_drag(),
    );
    let learning = bound && crate::controllers::stepped_inst(app, ui, track, spec, key, &resp);
    let bound = bound && !learning;
    if bound && (resp.dragged() || resp.drag_started()) {
        if let Some(q) = resp.interact_pointer_pos() {
            let f = ((q.x - bar.left()) / bar.width()).clamp(0.0, 1.0);
            app.set_inst_value(track, key, min + f * (max - min), true);
        }
    }
    if bound && resp.drag_stopped() {
        app.end_gesture();
    }
    let (n, u) = num.text(v);
    let resp = iu::hint(resp, || format!("{label} \u{b7} {n} {u}"));
    with_brush(ui.painter(), |b| {
        let lr = Rect::from_min_max(r.min, pos2(r.right(), bar.top() - s.s(2.0)));
        words(
            b,
            s,
            label,
            lr,
            Align2::LEFT_CENTER,
            12.0,
            W::Regular,
            p.ink2,
        );
        value_words(b, s, &p, lr, &n, &u, Align2::RIGHT_CENTER);
        b.rrect(b.snap_rect(bar), s.s(1.5), Fill::Solid(p.bar_track));
        let fill = Rect::from_min_max(bar.min, pos2(bar.left() + bar.width() * pos, bar.bottom()));
        let ink = if resp.hovered() || resp.dragged() {
            p.ink1
        } else {
            p.ink2
        };
        b.rrect(b.snap_rect(fill), s.s(1.5), Fill::Solid(ink));
        crate::views::inspector::look::handle(
            b,
            &p,
            pos2(fill.right(), bar.center().y),
            resp.hovered() || resp.dragged(),
            1.0,
        );
    });
}

/// A row with a switch at its right, `key` bound (on at 0.5 and up).
#[allow(clippy::too_many_arguments)]
pub fn switch_row(
    app: &mut CantusApp,
    ui: &egui::Ui,
    track: TrackId,
    spec: &NativeSpec,
    s: &Style,
    r: Rect,
    key: &str,
    label: &str,
    fallback_on: bool,
    tip: &'static str,
) {
    switch_between(
        app,
        ui,
        track,
        spec,
        s,
        r,
        key,
        label,
        fallback_on,
        tip,
        (0.0, 1.0),
    );
}

/// [`switch_row`] between two values of `key`, `(off, on)`: on from their middle up (r42
/// integration: the model's switch, 1 off and 2 on).
#[allow(clippy::too_many_arguments)]
pub fn switch_between(
    app: &mut CantusApp,
    ui: &egui::Ui,
    track: TrackId,
    spec: &NativeSpec,
    s: &Style,
    r: Rect,
    key: &str,
    label: &str,
    fallback_on: bool,
    tip: &'static str,
    (off, on_at): (f32, f32),
) {
    let p = Pal::of(s);
    let bound = spec.kind.param(key).is_some();
    let on = if bound {
        spec.get(key) >= (off + on_at) * 0.5
    } else {
        fallback_on
    };
    let sw = Rect::from_min_max(
        pos2(r.right() - s.s(26.0), r.center().y - s.s(7.0)),
        pos2(r.right(), r.center().y + s.s(7.0)),
    );
    let resp = ui.interact(
        r,
        ui.id().with(("tymp-switch", track.0, key)),
        Sense::click(),
    );
    let learning = bound && crate::controllers::stepped_inst(app, ui, track, spec, key, &resp);
    if bound && !learning && resp.clicked() {
        app.set_inst_value(track, key, if on { off } else { on_at }, false);
    }
    let resp = iu::hint(resp, || t(tip));
    with_brush(ui.painter(), |b| {
        let lr = Rect::from_min_max(r.min, pos2(sw.left() - s.s(8.0), r.bottom()));
        words(
            b,
            s,
            label,
            lr,
            Align2::LEFT_CENTER,
            12.0,
            W::Regular,
            p.ink2,
        );
        crate::views::inspector::look::switch(
            b,
            s,
            sw.right(),
            sw.center().y,
            on,
            State::of(&resp, on),
        );
    });
}

/// The row of pieces: a tile each, its name and its key; the selected one raised with the
/// accent's line, a green dot while it sounds. Each tile is as wide as its widest name and
/// key ask (the row's share when that is more); a kit whose tiles do not fit scrolls
/// sideways (fx/final: at the Kit view's least width Studio Maple's China was cut at the
/// row's end, out of reach).
pub fn pieces_row(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    track: TrackId,
    def: &KitDef,
    s: &Style,
    r: Rect,
    lk: &mut Look,
) {
    let p = Pal::of(s);
    let n = def.pieces.len().max(1);
    // Measured once a kit and a scale (a frame at rest allocates nothing for it).
    let memo = egui::Id::new((
        "tymp-row-need",
        track.0,
        def.name.as_str(),
        s.tk().to_bits(),
    ));
    let need = match ui.ctx().data(|d| d.get_temp::<f32>(memo)) {
        Some(w) => w,
        None => {
            let w = def
                .pieces
                .iter()
                .map(|pd| {
                    let key = pd.articulations.first().map(|a| a.key).unwrap_or(0);
                    let name = iu::width(ui, s, &def::piece_name(pd), 12.0, W::SemiBold);
                    let kt = iu::width(ui, s, &def::key_text(key), 10.0, W::Regular);
                    name.max(kt)
                })
                .fold(0.0f32, f32::max)
                + s.s(14.0);
            ui.ctx().data_mut(|d| d.insert_temp(memo, w));
            w
        }
    };
    let tw = (r.width() / n as f32).max(need);
    with_brush(ui.painter(), |b| iu::card(b, s, r));
    let mut child = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(r)
            .id_salt(("tymp-row", track.0)),
    );
    child.set_clip_rect(r.intersect(ui.clip_rect()));
    let alt = ui.input(|i| i.modifiers.alt);
    let mut tiles = |ui: &mut egui::Ui, row: Rect| {
        for (i, pd) in def.pieces.iter().enumerate() {
            let tr = Rect::from_min_size(
                pos2(row.left() + tw * i as f32, row.top()),
                vec2(tw, row.height()),
            );
            let key = pd.articulations.first().map(|a| a.key).unwrap_or(0);
            let resp = ui.interact(
                tr,
                ui.id().with(("tile", pd.index)),
                Sense::click_and_drag(),
            );
            let name = def::piece_name(pd);
            let kt = def::key_text(key);
            let resp = iu::hint(resp, || {
                tf(
                    "tympanum.piece_tip",
                    &[
                        ("name", &name),
                        ("key", &crate::controllers::note_name(key)),
                    ],
                )
            });
            if super::pressed(&resp) {
                let nart = pd.articulations.len().max(1);
                let a = if alt {
                    let a = if lk.piece == pd.index {
                        lk.turn % nart
                    } else {
                        0
                    };
                    lk.turn = a + 1;
                    a
                } else {
                    0
                };
                lk.piece = pd.index;
                lk.tag = None;
                if let Some(art) = pd.articulations.get(a) {
                    press(app, track, lk, art.key, pd.index);
                }
            }
            let lit = lk.piece == pd.index;
            let sounding = lk.held.is_some() && lk.hit.is_some_and(|h| h.0 == pd.index);
            with_brush(ui.painter(), |b| {
                let cell = tr.shrink2(vec2(s.s(2.0), s.s(3.0)));
                if lit {
                    b.rrect(
                        b.snap_rect(cell),
                        s.s(4.0),
                        Fill::Solid(mix(p.surface, p.ink1, 0.07)),
                    );
                    let line = Rect::from_min_size(
                        pos2(cell.left() + s.s(6.0), cell.top()),
                        vec2(cell.width() - s.s(12.0), s.s(2.0)),
                    );
                    b.rect(b.snap_rect(line), Fill::Solid(p.accent));
                } else if resp.hovered() {
                    b.rrect(
                        b.snap_rect(cell),
                        s.s(4.0),
                        Fill::Solid(mix(p.surface, p.ink1, 0.035)),
                    );
                }
                if i > 0 && !lit && lk.piece != def.pieces[i - 1].index {
                    b.vline(
                        b.snap(tr.left()) + b.px() * 0.5,
                        tr.top() + s.s(10.0),
                        tr.bottom() - s.s(10.0),
                        b.px(),
                        p.line,
                    );
                }
                let nr = Rect::from_min_size(
                    pos2(tr.left(), tr.top() + s.s(5.0)),
                    vec2(tr.width(), s.s(16.0)),
                );
                words(
                    b,
                    s,
                    &name,
                    nr,
                    Align2::CENTER_CENTER,
                    12.0,
                    if lit { W::SemiBold } else { W::Medium },
                    p.ink1,
                );
                let kr =
                    Rect::from_min_size(pos2(tr.left(), nr.bottom()), vec2(tr.width(), s.s(14.0)));
                words(
                    b,
                    s,
                    &kt,
                    kr,
                    Align2::CENTER_CENTER,
                    10.0,
                    W::Regular,
                    if lit { p.ink2 } else { p.ink3 },
                );
                if sounding {
                    b.circle(
                        pos2(cell.right() - s.s(7.0), cell.top() + s.s(7.0)),
                        s.s(3.0),
                        Fill::Solid(p.play),
                    );
                }
            });
        }
    };
    let total = tw * n as f32;
    if total <= r.width() + 0.5 {
        tiles(&mut child, r);
    } else {
        egui::ScrollArea::horizontal()
            .id_salt(("tymp-row-scroll", track.0))
            .auto_shrink([false, false])
            .show(&mut child, |ui| {
                let (row, _) = ui.allocate_exact_size(vec2(total, r.height()), Sense::hover());
                tiles(ui, row);
            });
    }
}

/// The kit's three cards under the row.
#[allow(clippy::too_many_arguments)]
pub fn cards(
    app: &mut CantusApp,
    ui: &egui::Ui,
    track: TrackId,
    spec: &NativeSpec,
    def: &KitDef,
    s: &Style,
    r: Rect,
    lk: &mut Look,
) {
    let g = s.s(iu::GAP);
    let w = r.width() - 2.0 * g;
    let (a, b2) = (w * 0.40, w * 0.32);
    let feel = Rect::from_min_size(r.min, vec2(a, r.height()));
    let pedal = Rect::from_min_size(pos2(feel.right() + g, r.top()), vec2(b2, r.height()));
    let kit = Rect::from_min_max(pos2(pedal.right() + g, r.top()), r.max);
    with_brush(ui.painter(), |b| {
        for c in [feel, pedal, kit] {
            iu::card(b, s, c);
        }
    });
    let pad = s.s(14.0);
    let head = |c: Rect| {
        Rect::from_min_size(
            pos2(c.left() + pad, c.top() + s.s(8.0)),
            vec2(c.width() - 2.0 * pad, s.s(22.0)),
        )
    };
    // Feel.
    with_brush(ui.painter(), |b| {
        iu::head(
            b,
            s,
            head(feel),
            &t("tympanum.feel"),
            &t("tympanum.feel_note"),
        )
    });
    let rows = |c: Rect, i: f32| {
        Rect::from_min_size(
            pos2(c.left() + pad, c.top() + s.s(36.0) + i * s.s(30.0)),
            vec2(c.width() - 2.0 * pad, s.s(28.0)),
        )
    };
    bar_row(
        app,
        ui,
        track,
        spec,
        s,
        rows(feel, 0.0),
        "timing",
        &t("tympanum.timing"),
        Num::new(0.0, 20.0, 4.0, "ms"),
    );
    bar_row(
        app,
        ui,
        track,
        spec,
        s,
        rows(feel, 1.0),
        "humanize",
        &t("tympanum.strength"),
        Num::new(0.0, 1.0, 0.06, "%").pct().pm(),
    );
    let sw = Rect::from_min_size(
        pos2(feel.left() + pad, feel.bottom() - s.s(34.0)),
        vec2(feel.width() - 2.0 * pad, s.s(24.0)),
    );
    switch_row(
        app,
        ui,
        track,
        spec,
        s,
        sw,
        "vary",
        &t("tympanum.never_twice"),
        true,
        "tympanum.never_twice_tip",
    );
    // The hi-hat's pedal.
    let cc = spec.get("hat.cc").round() as u32;
    with_brush(ui.painter(), |b| {
        iu::head(
            b,
            s,
            head(pedal),
            &t("tympanum.pedal"),
            &tf("tympanum.pedal_note", &[("cc", &cc)]),
        )
    });
    pedal_card(app, ui, track, spec, def, s, pedal, pad, lk);
    // Kit: every piece at once.
    with_brush(ui.painter(), |b| {
        iu::head(
            b,
            s,
            head(kit),
            &t("tympanum.kit_card"),
            &t("tympanum.kit_card_note"),
        )
    });
    let cells = Rect::from_min_max(
        pos2(kit.left() + s.s(6.0), kit.top() + s.s(40.0)),
        pos2(kit.right() - s.s(6.0), kit.bottom() - s.s(6.0)),
    );
    let cw = cells.width() / 3.0;
    let specs = [
        (
            "tune",
            "tympanum.kit_tune",
            Num::new(-12.0, 12.0, 0.0, "st").signed(),
        ),
        (
            "ring",
            "tympanum.kit_ring",
            Num::new(0.0, 2.0, 1.0, "%").pct(),
        ),
        (
            "gain",
            "tympanum.kit_level",
            Num::new(-24.0, 12.0, 0.0, "dB"),
        ),
    ];
    for (i, (key, label, num)) in specs.into_iter().enumerate() {
        let cr = Rect::from_min_size(
            pos2(cells.left() + cw * i as f32, cells.top()),
            vec2(cw, cells.height()),
        );
        knob_cell(app, ui, track, spec, s, cr, key, &t(label), num);
    }
}

/// The pedal's card: how open the hat is on its controller, the four openings to hear, and
/// the foot's chick.
#[allow(clippy::too_many_arguments)]
fn pedal_card(
    app: &mut CantusApp,
    ui: &egui::Ui,
    track: TrackId,
    spec: &NativeSpec,
    def: &KitDef,
    s: &Style,
    c: Rect,
    pad: f32,
    lk: &mut Look,
) {
    let p = Pal::of(s);
    let hat = def.pieces.iter().find(|x| x.kind == PieceKind::HiHat);
    let stops = [
        ("tight", "tympanum.open.tight"),
        ("closed", "tympanum.open.closed"),
        ("half", "tympanum.open.half"),
        ("open", "tympanum.open.open"),
    ];
    // How open the hat is: the opening last played on it, else closed.
    let open = hat
        .and_then(|h| h.articulations.iter().find(|a| a.key == lk.last_key))
        .and_then(|a| a.openness)
        .map_or(0.15, |o| o.at);
    let row = Rect::from_min_size(
        pos2(c.left() + pad, c.top() + s.s(36.0)),
        vec2(c.width() - 2.0 * pad, s.s(18.0)),
    );
    let pct = format!("{:.0}", open * 100.0);
    with_brush(ui.painter(), |b| {
        words(
            b,
            s,
            &t("tympanum.openness"),
            row,
            Align2::LEFT_CENTER,
            12.0,
            W::Regular,
            p.ink2,
        );
        value_words(b, s, &p, row, &pct, "%", Align2::RIGHT_CENTER);
        let bar = Rect::from_min_size(
            pos2(row.left(), row.bottom() + s.s(4.0)),
            vec2(row.width() * 0.5, s.s(3.0)),
        );
        b.rrect(b.snap_rect(bar), s.s(1.5), Fill::Solid(p.bar_track));
        let fill = Rect::from_min_size(bar.min, vec2(bar.width() * open, bar.height()));
        b.rrect(b.snap_rect(fill), s.s(1.5), Fill::Solid(p.ink2));
        crate::views::inspector::look::handle(
            b,
            &p,
            pos2(fill.right(), bar.center().y),
            false,
            1.0,
        );
    });
    let sr = Rect::from_min_size(
        pos2(row.left(), row.bottom() + s.s(12.0)),
        vec2(row.width(), s.s(20.0)),
    );
    let sw = sr.width() / 4.0;
    for (i, (id, label)) in stops.iter().enumerate() {
        let r = Rect::from_min_size(
            pos2(sr.left() + sw * i as f32, sr.top()),
            vec2(sw, sr.height()),
        );
        let art = hat.and_then(|h| h.articulations.iter().find(|a| a.id == *id));
        let resp = ui.interact(r, ui.id().with(("tymp-open", track.0, *id)), Sense::click());
        let name = t(label);
        let resp = iu::hint(resp, || match art {
            Some(a) => tf(
                "tympanum.art_tip",
                &[
                    ("name", &name),
                    ("key", &crate::controllers::note_name(a.key)),
                ],
            ),
            None => name.clone(),
        });
        if resp.clicked() {
            if let (Some(a), Some(h)) = (art, hat) {
                lk.piece = h.index;
                press(app, track, lk, a.key, h.index);
            }
        }
        let now = art
            .and_then(|a| a.openness)
            .is_some_and(|o| (o.at - open).abs() < 0.04);
        with_brush(ui.painter(), |b| {
            let ink = if now {
                p.ink1
            } else if resp.hovered() {
                p.ink2
            } else {
                p.ink3
            };
            words(
                b,
                s,
                &name,
                r,
                Align2::CENTER_CENTER,
                11.0,
                if now { W::SemiBold } else { W::Regular },
                ink,
            );
        });
    }
    let sw = Rect::from_min_size(
        pos2(c.left() + pad, c.bottom() - s.s(34.0)),
        vec2(c.width() - 2.0 * pad, s.s(24.0)),
    );
    switch_row(
        app,
        ui,
        track,
        spec,
        s,
        sw,
        "hat.chick",
        &t("tympanum.foot_chick"),
        true,
        "tympanum.foot_chick_tip",
    );
}
