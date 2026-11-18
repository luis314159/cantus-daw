//! Chorda's Play view as the approved design draws it (round 40 phase 3, `design-p3`
//! *Chorda · Play*, delivery 6; `chorda-src`): the source list at the left; the piano in
//! its room (the scene's card: its pickers over the top, the room card over the picture);
//! the keyboard under it; *Hammer · Resonance · Output*; at the right, *The piano* — the
//! lid, the perspective ("where you listen") with the mixer's mics it moves and *Open the
//! mixer*, the pedals with the dampers against the pedal. No macro or modulation rows here
//! (the Edit view keeps them, the Matrix view routes them).
//!
//! Every control of the old Play view still has a place: *Room* and *Width* moved to the
//! Mixer (the Room strip's fader, the Piano strip's top cell); *Dampers* is a row of the
//! pedals.

use super::{card, card_h, designing, model2, now_of, view};
use crate::app::CantusApp;
use crate::i18n::t;
use crate::kit::{with_brush, Fill, Function, Style};
use crate::views::inst::inst_mixer::ui::{self as iu, words};
use crate::views::mixer::look::{Pal, W};
use cantus_core::inst::chorda_source::{self as cs};
use cantus_core::inst_out::{key, OutputField};
use cantus_core::{NativeSpec, TrackId};
use cantus_dsp::chorda::voice::pedal_contact;
use cantus_engine::inst::chorda::{settings_from, values_of};
use egui::{pos2, vec2, Align2, Rect, Sense};

/// The right panel's width at 100 % (the design's 260 pt).
pub const SIDE_W: f32 = 260.0;
/// The keyboard's height at 100 %.
const KEYS_H: f32 = 40.0;

/// The Play view.
pub fn play(app: &mut CantusApp, ui: &mut egui::Ui, track: TrackId, spec: &NativeSpec, s: &Style) {
    let busy = designing(app, spec);
    moved_once(app);
    // The player's cards run along the foot, under the source list as well as the piano
    // (fx/final: under the piano alone their eight knobs had 520 points and ran out of
    // the Hammer card); the list above them scrolls.
    let full = ui.max_rect();
    let g = s.s(iu::GAP);
    let h2 = s.s(card_h(0.0, 86.0));
    let upper = Rect::from_min_max(full.min, pos2(full.right(), full.bottom() - h2 - g));
    let top = super::super::chorda_source::list::column(app, ui, track, spec, s, upper);
    let area = Rect::from_min_max(top.min, pos2(top.right(), full.bottom()));
    app.inst.chorda.area = Some(area);
    app.inst.chorda.view = Some(ui.clip_rect());
    let side_w = s.s(SIDE_W).max(area.width() * 0.25);
    let side = Rect::from_min_max(pos2(area.right() - side_w, area.top()), area.max);
    let center = Rect::from_min_max(area.min, pos2(side.left() - g, upper.bottom()));
    let cards = Rect::from_min_max(
        pos2(full.left(), full.bottom() - h2),
        pos2(side.left() - g, full.bottom()),
    );
    let keys = Rect::from_min_max(
        pos2(center.left(), center.bottom() - s.s(KEYS_H)),
        center.max,
    );
    let picture = Rect::from_min_max(center.min, pos2(center.right(), keys.top() - g));
    view::scene_card(app, ui, track, spec, picture, s, busy.as_deref());
    super::super::frame::keyboard_band(app, ui, track, s, keys);
    // Hammer · Resonance · Output (model 1 keeps its width at the output).
    let m2 = model2(spec);
    let output: &[&str] = if m2 {
        &["tilt", "gain"]
    } else {
        &["tilt", "width", "gain"]
    };
    let row: [(&str, Function, &[&str]); 3] = [
        (
            "chorda.hammer",
            Function::Dynamics,
            &["hardness", "dynamics", "vel.curve"],
        ),
        (
            "chorda.resonance",
            Function::Space,
            &["sympathy", "body", "decay"],
        ),
        ("chorda.output", Function::Output, output),
    ];
    // Each card as wide as its share of the knobs (fx/final: at fixed fractions the
    // Hammer's three knobs ran out of its card); a card too narrow for its knobs draws
    // them a size smaller (`organ::parts::fitting`).
    let total: usize = row.iter().map(|(_, _, k)| k.len()).sum();
    let mut x = cards.left();
    let w = cards.width() - 2.0 * g;
    for (title, f, keys) in row {
        let f_w = keys.len() as f32 / total.max(1) as f32;
        let r = Rect::from_min_size(pos2(x, cards.top()), vec2(w * f_w, cards.height()));
        card(
            app,
            ui,
            track,
            spec,
            r,
            s,
            title,
            f,
            0.0,
            keys,
            |_, _, _| {},
        );
        x = r.right() + g;
    }
    panel(app, ui, track, spec, s, side);
}

/// The key of the one-time notice that says where the old Play view's controls went.
pub const MOVED_NOTICE: &str = "chorda.play.moved.r40";

/// The first time this machine shows the new Play view: say on the status line where the
/// old one's controls went (Room level, Width, Dampers, the macros), once.
fn moved_once(app: &mut CantusApp) {
    if app.settings.notices_seen.iter().any(|k| k == MOVED_NOTICE) {
        return;
    }
    app.settings.notices_seen.push(MOVED_NOTICE.to_string());
    app.store_settings();
    app.set_status(t("chorda_source.play.moved_notice"));
}

/// A row: its label at the left, its value at the right, a bar under them that drags the
/// key (0 … 1), double-click for its default; one undo step a gesture.
#[allow(clippy::too_many_arguments)]
fn slider(
    app: &mut CantusApp,
    ui: &egui::Ui,
    track: TrackId,
    spec: &NativeSpec,
    s: &Style,
    r: Rect,
    k: &str,
    label: &str,
    value: &str,
) {
    let p = Pal::of(s);
    let v = spec.get(k).clamp(0.0, 1.0);
    let bar = Rect::from_min_max(
        pos2(r.left(), r.bottom() - s.s(6.0)),
        pos2(r.right(), r.bottom() - s.s(3.0)),
    );
    let resp = ui.interact(
        r,
        ui.id().with(("chorda-play-slider", track.0, k)),
        Sense::click_and_drag(),
    );
    // The row's words, and what it says beside them (where a moved control went).
    let more = match k {
        "persp" => t("chorda_source.play.tip_persp"),
        "lid" => t("chorda_source.play.tip_lid"),
        "pedal" => t("chorda_source.play.tip_sustain"),
        "una" => t("chorda_source.play.tip_soft"),
        "damper" => t("chorda_source.play.tip_dampers"),
        _ => String::new(),
    };
    let tip = if more.is_empty() {
        format!("{label} \u{b7} {value}")
    } else {
        format!("{label} \u{b7} {value}. {more}")
    };
    let resp = iu::hint(resp, || tip);
    if resp.dragged() || resp.drag_started() {
        if let Some(q) = resp.interact_pointer_pos() {
            let f = ((q.x - bar.left()) / bar.width()).clamp(0.0, 1.0);
            app.set_inst_value(track, k, f, true);
        }
    }
    if resp.drag_stopped() {
        app.end_gesture();
    }
    if resp.double_clicked() {
        if let Some(d) = cantus_core::InstrumentKind::Chorda
            .param(k)
            .map(|i| i.default)
        {
            app.set_inst_value(track, k, d, false);
        }
    }
    with_brush(ui.painter(), |b| {
        let tr = Rect::from_min_max(r.min, pos2(r.right(), bar.top() - s.s(2.0)));
        words(
            b,
            s,
            label,
            tr,
            Align2::LEFT_CENTER,
            11.0,
            W::Regular,
            p.ink2,
        );
        words(
            b,
            s,
            value,
            tr,
            Align2::RIGHT_CENTER,
            12.0,
            W::SemiBold,
            p.ink1,
        );
        b.rrect(b.snap_rect(bar), s.s(1.5), Fill::Solid(p.bar_track));
        let fill = Rect::from_min_size(bar.min, vec2(bar.width() * v, bar.height()));
        b.rrect(b.snap_rect(fill), s.s(1.5), Fill::Solid(p.ink2));
        let c = pos2(bar.left() + bar.width() * v, bar.center().y);
        let ink = if resp.hovered() || resp.dragged() {
            p.ink1
        } else {
            p.ink2
        };
        b.circle(c, s.s(4.0), Fill::Solid(ink));
    });
}

/// Small words under a bar, spread across `r` (left, middle, right).
fn ticks(ui: &egui::Ui, s: &Style, r: Rect, words3: [&str; 3]) {
    let p = Pal::of(s);
    with_brush(ui.painter(), |b| {
        for (w, a) in words3.iter().zip([
            Align2::LEFT_CENTER,
            Align2::CENTER_CENTER,
            Align2::RIGHT_CENTER,
        ]) {
            iu::note(b, s, w, r, a, 10.0, W::Regular, p.ink3);
        }
    });
}

fn pct(v: f32) -> String {
    format!("{:.0} %", v * 100.0)
}

/// A section's head inside the panel: its title and a quiet note.
fn head(ui: &egui::Ui, s: &Style, r: Rect, title: &str, note: &str) {
    let p = Pal::of(s);
    with_brush(ui.painter(), |b| {
        words(
            b,
            s,
            title,
            r,
            Align2::LEFT_CENTER,
            12.0,
            W::SemiBold,
            p.ink1,
        );
        iu::note(
            b,
            s,
            note,
            r,
            Align2::RIGHT_CENTER,
            10.0,
            W::Regular,
            p.ink3,
        );
    });
}

/// *The piano*: the lid, where you listen and the mics it reaches, the pedals.
fn panel(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    track: TrackId,
    spec: &NativeSpec,
    s: &Style,
    r: Rect,
) {
    let p = Pal::of(s);
    with_brush(ui.painter(), |b| iu::card(b, s, r));
    let inner = r.shrink2(vec2(s.s(14.0), s.s(12.0)));
    let x0 = inner.left();
    let w = inner.width();
    let mut y = inner.top();
    let line = |y: f32, h: f32| Rect::from_min_size(pos2(x0, y), vec2(w, s.s(h)));
    // The title and what the picker offers.
    with_brush(ui.painter(), |b| {
        words(
            b,
            s,
            &t("chorda_source.play.title"),
            line(y, 20.0),
            Align2::LEFT_CENTER,
            14.0,
            W::SemiBold,
            p.ink1,
        );
        iu::note(
            b,
            s,
            &t("chorda_source.play.finishes"),
            line(y + s.s(20.0), 14.0),
            Align2::LEFT_CENTER,
            10.0,
            W::Regular,
            p.ink3,
        );
    });
    y += s.s(44.0);
    // The lid.
    let lid = spec.get("lid");
    let lid_word = if lid < 0.04 {
        t("chorda_source.play.lid_closed")
    } else if lid < 0.75 {
        t("chorda_source.play.lid_short")
    } else {
        t("chorda_source.play.lid_full")
    };
    slider(
        app,
        ui,
        track,
        spec,
        s,
        line(y, 26.0),
        "lid",
        &t("chorda_source.play.lid"),
        &lid_word,
    );
    y += s.s(28.0);
    ticks(
        ui,
        s,
        line(y, 12.0),
        [
            &t("chorda_source.play.lid_closed_tick"),
            &t("chorda_source.play.lid_short_tick"),
            &t("chorda_source.play.lid_full_tick"),
        ],
    );
    y += s.s(24.0);
    // Where you listen, and the mics of the mixer.
    head(
        ui,
        s,
        line(y, 18.0),
        &t("chorda_source.play.persp"),
        &t("chorda_source.play.persp_note"),
    );
    y += s.s(22.0);
    let persp = spec.get("persp");
    slider(
        app,
        ui,
        track,
        spec,
        s,
        line(y, 26.0),
        "persp",
        &t("chorda_source.play.where"),
        &pct(persp),
    );
    y += s.s(28.0);
    ticks(
        ui,
        s,
        line(y, 12.0),
        [
            &t("chorda_source.name.hammers"),
            &t("chorda_source.name.player"),
            &t("chorda_source.name.audience"),
        ],
    );
    y += s.s(18.0);
    if !cs::has_zones(spec) {
        with_brush(ui.painter(), |b| {
            iu::note(
                b,
                s,
                &t("chorda_source.play.in_mixer"),
                line(y, 14.0),
                Align2::LEFT_CENTER,
                10.0,
                W::Regular,
                p.ink3,
            );
        });
        y += s.s(18.0);
        for o in cs::model_outputs() {
            let db = spec.get(&key(o.id, OutputField::Level));
            let rr = line(y, 16.0);
            with_brush(ui.painter(), |b| {
                let name = super::super::chorda_source::strip::display_name(&o.name);
                words(
                    b,
                    s,
                    &name,
                    rr,
                    Align2::LEFT_CENTER,
                    11.0,
                    W::Regular,
                    p.ink2,
                );
                let bar = Rect::from_min_size(
                    pos2(rr.left() + w * 0.36, rr.center().y - s.s(1.5)),
                    vec2(w * 0.42, s.s(3.0)),
                );
                let f = (cantus_core::db_to_linear(db)).clamp(0.0, 1.0);
                b.rrect(b.snap_rect(bar), s.s(1.5), Fill::Solid(p.bar_track));
                let fill = Rect::from_min_size(bar.min, vec2(bar.width() * f, bar.height()));
                b.rrect(b.snap_rect(fill), s.s(1.5), Fill::Solid(p.ink2));
                let v = crate::views::inst::inst_mixer::db_text(db);
                words(
                    b,
                    s,
                    &format!("{v} dB"),
                    rr,
                    Align2::RIGHT_CENTER,
                    11.0,
                    W::Medium,
                    p.ink1,
                );
            });
            y += s.s(18.0);
        }
        with_brush(ui.painter(), |b| {
            iu::note(
                b,
                s,
                &t("chorda_source.play.own_faders"),
                line(y, 14.0),
                Align2::LEFT_CENTER,
                10.0,
                W::Regular,
                p.ink3,
            );
        });
        y += s.s(16.0);
    }
    // Open the mixer.
    let open = t("chorda_source.play.open_mixer");
    let ow = iu::width(ui, s, &open, 11.0, W::Medium) + s.s(4.0);
    let or = Rect::from_min_size(pos2(inner.right() - ow, y), vec2(ow, s.s(18.0)));
    let oresp = ui.interact(
        or,
        ui.id().with(("chorda-open-mixer", track.0)),
        Sense::click(),
    );
    let oresp = iu::hint(oresp, || t("chorda_source.play.open_mixer_tip"));
    if oresp.clicked() {
        app.inst.view = crate::views::inst::View::Mixer;
    }
    with_brush(ui.painter(), |b| {
        let ink = if oresp.hovered() { p.ink1 } else { p.accent };
        words(b, s, &open, or, Align2::RIGHT_CENTER, 11.0, W::Medium, ink);
    });
    y += s.s(28.0);
    // The pedals.
    head(
        ui,
        s,
        line(y, 18.0),
        &t("chorda.pedals"),
        &t("chorda_source.play.pedals_note"),
    );
    y += s.s(22.0);
    // The pedals as they are now: the player's foot (controllers 64 and 67) or the set
    // position, whichever is further down.
    let now = now_of(&app.inst_telemetry());
    for (k, label, live) in [
        ("pedal", "chorda_source.play.sustain", now.pedal),
        ("una", "chorda_source.play.soft", now.soft),
        ("damper", "chorda.damper", 0.0),
    ] {
        let v = spec.get(k).max(live);
        slider(
            app,
            ui,
            track,
            spec,
            s,
            line(y, 26.0),
            k,
            &t(label),
            &pct(v),
        );
        y += s.s(30.0);
    }
    // Dampers against the pedal: the half-pedal law, where the pedal is now.
    let (vs, _) = settings_from(&values_of(spec));
    let grip = vs.voice.damper as f32;
    with_brush(ui.painter(), |b| {
        iu::note(
            b,
            s,
            &t("chorda_source.play.dampers_curve"),
            line(y, 14.0),
            Align2::LEFT_CENTER,
            10.0,
            W::Regular,
            p.ink3,
        );
    });
    y += s.s(18.0);
    let bottom = inner.bottom() - s.s(16.0);
    if bottom - y > s.s(30.0) {
        let gr = Rect::from_min_max(pos2(x0, y), pos2(inner.right(), bottom));
        curve(ui, s, gr, grip, now.pedal.max(spec.get("pedal")));
        ticks(
            ui,
            s,
            Rect::from_min_max(
                pos2(x0, bottom + s.s(2.0)),
                pos2(inner.right(), bottom + s.s(14.0)),
            ),
            [
                &t("chorda_source.play.up"),
                &t("chorda_source.play.half"),
                &t("chorda_source.play.down"),
            ],
        );
    }
}

/// The dampers' contact against the pedal's travel (`pedal_contact` × the grip), the
/// half-pedal band shaded, the pedal's place now marked.
fn curve(ui: &egui::Ui, s: &Style, r: Rect, grip: f32, pedal: f32) {
    let p = Pal::of(s);
    with_brush(ui.painter(), |b| {
        b.rrect(b.snap_rect(r), s.s(3.0), Fill::Solid(p.well));
        let half = Rect::from_min_max(
            pos2(r.left() + r.width() * 0.15, r.top()),
            pos2(r.left() + r.width() * 0.65, r.bottom()),
        );
        b.rect(
            half,
            Fill::Solid(crate::kit::color::mix(p.well, p.ink1, 0.05)),
        );
        let pad = s.s(6.0);
        let pt = |x: f32| {
            let c = pedal_contact(f64::from(x)) as f32 * grip;
            pos2(
                r.left() + r.width() * x,
                r.bottom() - pad - (r.height() - 2.0 * pad) * c,
            )
        };
        let pts: Vec<egui::Pos2> = (0..=48).map(|i| pt(i as f32 / 48.0)).collect();
        b.polyline(&pts, s.s(1.5), p.ink1);
        let m = pt(pedal.clamp(0.0, 1.0));
        b.circle(m, s.s(3.5), Fill::Solid(p.accent));
    });
}
