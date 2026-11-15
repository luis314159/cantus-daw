//! The Mixer's band: the kit with its mics (the selected one ringed, its pick-up cone drawn
//! by the scene; a click on a mic selects its strip), and the selected mic's card — its
//! pattern, how far it stands from what it is pointed at, its aim, its bleed — beside what
//! it hears (each piece in it, against the piece in its own mic) or, switched, every mic's
//! matrix (each piece in each mic: a click turns a cell off, Alt-click flips its polarity,
//! a drag sets its level). A recorded kit's mics stay where they were recorded.

use super::cards::{value_words, Num};
use super::iu::{self, words};
use super::{def, stage, strips, Look};
use crate::app::CantusApp;
use crate::i18n::{t, tf};
use crate::kit::color::{mix, with_alpha};
use crate::kit::text::Src;
use crate::kit::{with_brush, Fill, Style};
use crate::scene::Picked;
use crate::views::mixer::look::{Pal, W};
use cantus_core::inst::tympanum::MicPattern;
use cantus_core::inst::tympanum_kit::{KitDef, MicDef, Place};
use cantus_core::{Command, MergeKey, NativeSpec, OutputId, TrackId};
use cantus_dsp::math;
use egui::{pos2, vec2, Align2, Rect, Sense};

/// The band's height at 100 %.
pub const BAND: f32 = 172.0;

/// Where a mic points, as a unit vector in the kit's cm space.
fn aim(p: &Place) -> [f32; 3] {
    let (az, el) = (p.azimuth.to_radians(), p.elevation.to_radians());
    [
        math::sin(az) * math::cos(el),
        math::cos(az) * math::cos(el),
        math::sin(el),
    ]
}

/// How loud piece `piece` is in mic `m`, dB: the engine's own reckoning (1/r, the pattern
/// at its angle, the cell's level — `tymp-engine`'s), so the picture says what is played.
pub fn heard_db(def: &KitDef, spec: &NativeSpec, piece: u16, m: &MicDef) -> f32 {
    cantus_core::inst::tympanum_kit::heard_db(def, spec, piece, m)
}

/// Piece `p` in mic `m`, against the piece in its own mic (or the mic that hears it most).
pub fn relative_db(def: &KitDef, spec: &NativeSpec, piece: u16, m: &MicDef) -> f32 {
    let own = def::main_of(def, piece)
        .map(|o| heard_db(def, spec, piece, o))
        .unwrap_or_else(|| {
            def.mics
                .iter()
                .map(|o| heard_db(def, spec, piece, o))
                .fold(-96.0, f32::max)
        });
    (heard_db(def, spec, piece, m) - own).clamp(-96.0, 12.0)
}

/// What a mic is pointed at: its own piece, else the kit's middle (the kick).
fn target(def: &KitDef, m: &MicDef) -> Place {
    m.main_for
        .first()
        .and_then(|p| def.pieces.iter().find(|x| x.index == *p))
        .or_else(|| def.pieces.first())
        .map(|p| p.place)
        .unwrap_or_default()
}

/// The band in `r`.
#[allow(clippy::too_many_arguments)]
pub fn band(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    track: TrackId,
    spec: &NativeSpec,
    def: &KitDef,
    s: &Style,
    r: Rect,
    lk: &mut Look,
) {
    let g = s.s(iu::GAP);
    let pic_w = (r.width() * 0.32).clamp(s.s(260.0), s.s(470.0));
    let pic = Rect::from_min_size(r.min, vec2(pic_w, r.height()));
    let card = Rect::from_min_max(pos2(pic.right() + g, r.top()), r.max);
    picture(app, ui, track, spec, def, s, pic, lk);
    mic_card(app, ui, track, spec, def, s, card, lk);
}

#[allow(clippy::too_many_arguments)]
fn picture(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    track: TrackId,
    spec: &NativeSpec,
    def: &KitDef,
    s: &Style,
    r: Rect,
    lk: &mut Look,
) {
    let p = Pal::of(s);
    let sel = Some(Picked::Mic(OutputId(lk.mic)));
    let mut v = stage::view(def, spec, lk, true, sel, s);
    let resp = ui.interact(
        r,
        ui.id().with(("tymp-mic-picture", track.0)),
        Sense::click(),
    );
    let under = resp
        .hover_pos()
        .and_then(|q| stage::pick_shown(app, track, 1, r, &v, def, q));
    v.hovered = under;
    let resp = iu::hint(resp, || t("tympanum.mic_select_tip"));
    if resp.clicked() {
        match under {
            Some(Picked::Mic(id)) => lk.mic = id.0,
            Some(Picked::Piece { piece, .. }) => {
                if let Some(m) = def::main_of(def, piece) {
                    lk.mic = m.output.0;
                }
            }
            _ => {}
        }
    }
    with_brush(ui.painter(), |b| {
        b.rrect(b.snap_rect(r), s.s(6.0), Fill::Solid(p.well));
    });
    let painter = ui.painter_at(r);
    if stage::scene_mut(app, track, 1).paint(&painter, r, &v) {
        crate::views::workspace_ticker::moving(
            ui.ctx(),
            crate::views::workspace_ticker::Reason::Live,
        );
    }
    if stage::PLAN_VIEW {
        with_brush(&painter, |b| mic_plan(b, s, r, def, spec, lk));
    }
    // The count, top left; the selected mic's tag.
    let count = tf("tympanum.mics_count", &[("n", &def.mics.len())]);
    with_brush(&ui.painter_at(r), |b| {
        let cw = iu::width(ui, s, &count, 12.0, W::Medium) + s.s(36.0);
        let cr = Rect::from_min_size(
            pos2(r.left() + s.s(10.0), r.top() + s.s(10.0)),
            vec2(cw, s.s(26.0)),
        );
        iu::veil(b, s, cr);
        iu::mic_glyph(
            b,
            s,
            pos2(cr.left() + s.s(14.0), cr.center().y),
            s.s(13.0),
            p.ink2,
        );
        let tr = Rect::from_min_max(pos2(cr.left() + s.s(26.0), cr.top()), cr.max);
        words(
            b,
            s,
            &count,
            tr,
            Align2::LEFT_CENTER,
            12.0,
            W::Medium,
            p.ink1,
        );
    });
}

/// The mics from above, while the scene's stub draws nothing: the kit's plan, each mic a
/// glyph, the selected one ringed with its pick-up cone.
fn mic_plan(
    b: &mut crate::kit::Brush,
    s: &Style,
    r: Rect,
    def: &KitDef,
    spec: &NativeSpec,
    lk: &Look,
) {
    let p = Pal::of(s);
    let moved = super::moved(def, spec);
    let (mut x0, mut x1, mut y0, mut y1) = (f32::MAX, f32::MIN, f32::MAX, f32::MIN);
    let near = |q: &Place| q.y.abs() < 160.0 && q.x.abs() < 200.0;
    for q in moved
        .pieces
        .iter()
        .map(|x| x.place)
        .chain(moved.mics.iter().map(|m| m.place).filter(near))
    {
        x0 = x0.min(q.x);
        x1 = x1.max(q.x);
        y0 = y0.min(q.y);
        y1 = y1.max(q.y);
    }
    let room = r.shrink2(vec2(s.s(30.0), s.s(30.0)));
    let k = (room.width() / (x1 - x0).max(1.0)).min(room.height() / (y1 - y0).max(1.0));
    let c = room.center();
    let inside = r.shrink(s.s(12.0));
    let f = |x: f32, y: f32| {
        let q = pos2(
            c.x + (x - (x0 + x1) * 0.5) * k,
            c.y - (y - (y0 + y1) * 0.5) * k,
        );
        q.clamp(inside.min, inside.max)
    };
    for pd in &moved.pieces {
        let q = f(pd.place.x, pd.place.y);
        let rr = (pd.diameter * 0.5 * k).max(s.s(3.0));
        let ink = if def::is_cymbal(pd.kind) {
            mix(p.surface, s.t.brand, 0.45)
        } else {
            mix(p.raised, p.ink1, 0.18)
        };
        b.circle(q, rr, Fill::Solid(ink));
    }
    for m in &moved.mics {
        let q = f(m.place.x, m.place.y);
        let lit = m.output.0 == lk.mic;
        if lit {
            let a = aim(&m.place);
            let dir = vec2(a[0], -a[1]);
            let len = dir.length();
            if len > 0.05 {
                let d = dir / len;
                let n = vec2(-d.y, d.x);
                let reach = s.s(46.0);
                b.polygon(
                    &[
                        q,
                        q + d * reach + n * reach * 0.45,
                        q + d * reach - n * reach * 0.45,
                    ],
                    with_alpha(p.accent, 0.18),
                );
            }
            b.circle_stroke(q, s.s(10.0), s.s(1.6), p.accent);
        }
        iu::mic_glyph(b, s, q, s.s(12.0), if lit { p.accent } else { p.ink2 });
    }
}

/// The patterns the card offers: Cardioid, Super, Omni — and the mic's own when it is
/// another (a ribbon's figure 8).
fn patterns(now: MicPattern) -> Vec<MicPattern> {
    let mut v = vec![
        MicPattern::Cardioid,
        MicPattern::Supercardioid,
        MicPattern::Omni,
    ];
    if !v.contains(&now) {
        v[1] = now;
    }
    v
}

/// The selected mic's card in `r`: its controls at the left, what it hears at the right.
#[allow(clippy::too_many_arguments)]
fn mic_card(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    track: TrackId,
    spec: &NativeSpec,
    def: &KitDef,
    s: &Style,
    r: Rect,
    lk: &mut Look,
) {
    let p = Pal::of(s);
    with_brush(ui.painter(), |b| iu::card(b, s, r));
    let moved = super::moved(def, spec);
    let Some(m) = moved
        .mics
        .iter()
        .find(|x| x.output.0 == lk.mic)
        .or(moved.mics.first())
    else {
        return;
    };
    let base = def.mics.iter().find(|x| x.output == m.output).unwrap_or(m);
    let n = m.output.0;
    let pad = s.s(16.0);
    let left_w = (r.width() * 0.36).clamp(s.s(270.0), s.s(320.0));
    let col = Rect::from_min_max(
        pos2(r.left() + pad, r.top() + s.s(10.0)),
        pos2(r.left() + left_w, r.bottom() - s.s(10.0)),
    );
    // The name, its kind and pattern.
    let name = def::mic_name(def, m);
    let kind = format!(
        "{} \u{b7} {}",
        def::mic_kind(def, m),
        t(def::pattern_key(m.pattern)).to_lowercase()
    );
    with_brush(ui.painter(), |b| {
        let nr = Rect::from_min_size(col.min, vec2(col.width(), s.s(22.0)));
        let nw = words(
            b,
            s,
            &name,
            nr,
            Align2::LEFT_CENTER,
            15.0,
            W::SemiBold,
            p.ink1,
        );
        let kr = Rect::from_min_max(pos2(nr.left() + nw + s.s(10.0), nr.top()), nr.max);
        words(
            b,
            s,
            &kind,
            kr,
            Align2::LEFT_CENTER,
            11.0,
            W::Regular,
            p.ink3,
        );
    });
    let movable = m.movable && def.modelled;
    // Pattern.
    let row = |i: f32| {
        Rect::from_min_size(
            pos2(col.left(), col.top() + s.s(30.0) + i * s.s(30.0)),
            vec2(col.width(), s.s(26.0)),
        )
    };
    let pr = row(0.0);
    let label_w = s.s(70.0);
    with_brush(ui.painter(), |b| {
        words(
            b,
            s,
            &t("tympanum.mic.pattern"),
            Rect::from_min_size(pr.min, vec2(label_w, pr.height())),
            Align2::LEFT_CENTER,
            12.0,
            W::Regular,
            p.ink2,
        );
    });
    let pats = patterns(m.pattern);
    let labels: Vec<Src<'_>> = pats
        .iter()
        .map(|x| Src::Key(def::pattern_key(*x)))
        .collect();
    let sel = pats.iter().position(|x| *x == m.pattern).unwrap_or(0);
    let seg = Rect::from_min_max(pos2(pr.left() + label_w, pr.top()), pr.max);
    let (_, chosen) = crate::kit::widgets::Segmented::new(sel, &labels)
        .id(ui.id().with(("tymp-pattern", track.0, n)))
        .name(&t("tympanum.mic.pattern"))
        .style(*s)
        .enabled(movable)
        .show_at(ui, seg);
    if let (Some(i), true) = (chosen, movable) {
        let idx = MicPattern::ALL
            .iter()
            .position(|x| *x == pats[i])
            .unwrap_or(0);
        app.set_inst_value(track, &format!("m{n}.pattern"), (idx + 1) as f32, false);
    }
    // Distance, aim, bleed: rows with their bars.
    let tgt = target(def, m);
    let dist = def::dist(&m.place, &tgt);
    let aim_deg = {
        let v = [tgt.x - m.place.x, tgt.y - m.place.y, tgt.z - m.place.z];
        let l = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt().max(1e-3);
        let a = aim(&m.place);
        math::acos(((v[0] * a[0] + v[1] * a[1] + v[2] * a[2]) / l).clamp(-1.0, 1.0)).to_degrees()
    };
    let bleed = strips::bleed_share(def, spec, n);
    let hears_all = m.main_for.is_empty();
    let rows = [
        (
            "tympanum.mic.distance",
            Num::new(1.0, 300.0, dist, "cm"),
            (dist / 300.0).clamp(0.0, 1.0),
            movable,
        ),
        (
            "tympanum.mic.aim",
            Num::new(0.0, 180.0, aim_deg, "tympanum.mic.aim_unit"),
            (aim_deg / 90.0).clamp(0.0, 1.0),
            movable,
        ),
        (
            "tympanum.mic.bleed",
            Num::new(0.0, 1.0, bleed, "%").pct(),
            bleed.clamp(0.0, 1.0),
            !hears_all,
        ),
    ];
    for (i, (label, num, pos, live)) in rows.iter().enumerate() {
        let rr = row(1.0 + i as f32);
        let bar = Rect::from_min_max(
            pos2(rr.left(), rr.bottom() - s.s(5.0)),
            pos2(rr.left() + rr.width() * 0.6, rr.bottom() - s.s(2.0)),
        );
        let resp = ui.interact(
            rr,
            ui.id().with(("tymp-mic-row", track.0, n, i)),
            Sense::click_and_drag(),
        );
        if *live && (resp.dragged() || resp.drag_started()) {
            if let Some(q) = resp.interact_pointer_pos() {
                let f = ((q.x - bar.left()) / bar.width()).clamp(0.0, 1.0);
                match i {
                    0 => move_to(app, track, base, &tgt, m, (f * 300.0).max(1.0), dist),
                    1 => {
                        let off = spec.get(&format!("m{n}.az"));
                        let want = f * 90.0;
                        app.set_inst_value(
                            track,
                            &format!("m{n}.az"),
                            (off + (want - aim_deg)).clamp(-180.0, 180.0),
                            true,
                        );
                    }
                    _ => {
                        let cmds = strips::set_bleed(def, track, n, f);
                        app.exec_merging(
                            Command::Batch {
                                label_key: "cmd.tympanum_bleed".into(),
                                commands: cmds,
                            },
                            MergeKey::of(&("tymp-bleed", track.0, n)),
                        );
                    }
                }
            }
        }
        if resp.drag_stopped() {
            app.end_gesture();
        }
        let (num_s, unit) = num.text(num.default);
        let unit = if unit.starts_with("tympanum.") {
            t(&unit)
        } else {
            unit
        };
        let (num_s, unit) = if i == 2 && hears_all {
            (t("tympanum.mic.hears_all"), String::new())
        } else {
            (num_s, unit)
        };
        let lt = t(label);
        let tip = if !movable && i < 2 {
            t("tympanum.mic.fixed")
        } else {
            format!("{lt} \u{b7} {num_s} {unit}")
        };
        let resp = iu::hint(resp, || tip);
        with_brush(ui.painter(), |b| {
            let lr = Rect::from_min_max(rr.min, pos2(rr.right(), bar.top() - s.s(1.0)));
            words(
                b,
                s,
                &lt,
                lr,
                Align2::LEFT_CENTER,
                12.0,
                W::Regular,
                if *live { p.ink2 } else { p.ink3 },
            );
            value_words(b, s, &p, lr, &num_s, &unit, Align2::RIGHT_CENTER);
            b.rrect(b.snap_rect(bar), s.s(1.5), Fill::Solid(p.bar_track));
            if *live {
                let fill =
                    Rect::from_min_max(bar.min, pos2(bar.left() + bar.width() * pos, bar.bottom()));
                b.rrect(b.snap_rect(fill), s.s(1.5), Fill::Solid(p.ink2));
                crate::views::inspector::look::handle(
                    b,
                    &p,
                    pos2(fill.right(), bar.center().y),
                    resp.hovered() || resp.dragged(),
                    1.0,
                );
            }
        });
    }
    // At the right: what it hears, or every mic's matrix.
    let right = Rect::from_min_max(
        pos2(r.left() + left_w + s.s(28.0), r.top() + s.s(10.0)),
        pos2(r.right() - pad, r.bottom() - s.s(10.0)),
    );
    let tabs = [
        Src::Key("tympanum.mic.hears"),
        Src::Key("tympanum.mic.matrix"),
    ];
    let tw = tabs
        .iter()
        .map(|x| match x {
            Src::Key(k) => iu::width(ui, s, &t(k), 12.0, W::Medium),
            Src::Str(v) => iu::width(ui, s, v, 12.0, W::Medium),
        })
        .fold(0.0f32, f32::max)
        * 2.0
        + s.s(48.0);
    let tr = Rect::from_min_size(pos2(right.left(), right.top()), vec2(tw, s.s(24.0)));
    let (_, chosen) = crate::kit::widgets::Segmented::new(usize::from(lk.matrix), &tabs)
        .id(ui.id().with(("tymp-hears-tab", track.0)))
        .name(&t("tympanum.mic.hears"))
        .style(*s)
        .show_at(ui, tr);
    if let Some(i) = chosen {
        lk.matrix = i == 1;
    }
    let note = t(if lk.matrix {
        "tympanum.mic.matrix_note"
    } else {
        "tympanum.mic.hears_note"
    });
    with_brush(ui.painter(), |b| {
        let nr = Rect::from_min_max(
            pos2(tr.right() + s.s(12.0), tr.top()),
            pos2(right.right(), tr.bottom()),
        );
        words(
            b,
            s,
            &note,
            nr,
            Align2::RIGHT_CENTER,
            11.0,
            W::Regular,
            p.ink3,
        );
    });
    let chart_r = Rect::from_min_max(pos2(right.left(), tr.bottom() + s.s(8.0)), right.max);
    if lk.matrix {
        matrix(app, ui, track, spec, &moved, s, chart_r, lk);
    } else {
        hears(ui, spec, &moved, m, s, chart_r);
    }
}

/// Move mic `m` along the line to what it is pointed at until it stands `want` cm from it
/// (`m{n}.x/y/z`, one gesture).
fn move_to(
    app: &mut CantusApp,
    track: TrackId,
    base: &MicDef,
    tgt: &Place,
    m: &MicDef,
    want: f32,
    now: f32,
) {
    let n = m.output.0;
    let k = want / now.max(0.5);
    let at = [
        tgt.x + (m.place.x - tgt.x) * k,
        tgt.y + (m.place.y - tgt.y) * k,
        tgt.z + (m.place.z - tgt.z) * k,
    ];
    let cmds = [
        ("x", at[0] - base.place.x),
        ("y", at[1] - base.place.y),
        ("z", at[2] - base.place.z),
    ]
    .into_iter()
    .map(|(f, v)| Command::SetInstrumentValue {
        track,
        key: format!("m{n}.{f}"),
        value: v.clamp(-300.0, 300.0),
    })
    .collect();
    app.exec_merging(
        Command::Batch {
            label_key: "cmd.tympanum_mic".into(),
            commands: cmds,
        },
        MergeKey::of(&("tymp-mic", track.0, n)),
    );
}

/// Each piece in the mic, against the piece in its own mic: a bar from −40 dB, its name and
/// its figure under it; the mic's own pieces in the accent.
fn hears(ui: &egui::Ui, spec: &NativeSpec, def: &KitDef, m: &MicDef, s: &Style, r: Rect) {
    let p = Pal::of(s);
    let n = def.pieces.len().max(1);
    let axis = s.s(26.0);
    let plot = Rect::from_min_max(
        pos2(r.left() + axis, r.top() + s.s(4.0)),
        pos2(r.right(), r.bottom() - s.s(30.0)),
    );
    let cw = plot.width() / n as f32;
    let names: Vec<String> = def.pieces.iter().map(def::piece_name).collect();
    let name_size = [10.0, 9.5, 9.0, 8.5]
        .into_iter()
        .find(|z| {
            names
                .iter()
                .all(|n| iu::width(ui, s, n, *z, W::SemiBold) <= cw - s.s(2.0))
        })
        .unwrap_or(8.5);
    with_brush(ui.painter(), |b| {
        for (db, label) in [(0.0, "0"), (-20.0, "\u{2212}20"), (-40.0, "\u{2212}40")] {
            let y = plot.top() + plot.height() * (-db / 40.0);
            b.hline(
                plot.left(),
                plot.right(),
                b.snap(y) + b.px() * 0.5,
                b.px(),
                with_alpha(p.line, 0.6),
            );
            let lr = Rect::from_min_size(
                pos2(r.left(), y - s.s(7.0)),
                vec2(axis - s.s(6.0), s.s(14.0)),
            );
            words(
                b,
                s,
                label,
                lr,
                Align2::RIGHT_CENTER,
                10.0,
                W::Regular,
                p.ink3,
            );
        }
        for (i, pd) in def.pieces.iter().enumerate() {
            let rel = relative_db(def, spec, pd.index, m).clamp(-40.0, 0.0);
            let own = m.main_for.contains(&pd.index);
            let x = plot.left() + cw * (i as f32 + 0.5);
            let bw = (cw * 0.34).clamp(s.s(4.0), s.s(12.0));
            let top = plot.top() + plot.height() * (-rel / 40.0);
            let col = Rect::from_min_max(
                pos2(x - bw * 0.5, plot.top()),
                pos2(x + bw * 0.5, plot.bottom()),
            );
            b.rrect(
                b.snap_rect(col),
                s.s(2.0),
                Fill::Solid(mix(p.well, p.ink4, 0.25)),
            );
            let bar =
                Rect::from_min_max(pos2(x - bw * 0.5, top), pos2(x + bw * 0.5, plot.bottom()));
            b.rrect(
                b.snap_rect(bar),
                s.s(2.0),
                Fill::Solid(if own {
                    p.accent
                } else {
                    mix(p.ink2, p.surface, 0.15)
                }),
            );
            let nr = Rect::from_min_size(
                pos2(x - cw * 0.5, plot.bottom() + s.s(2.0)),
                vec2(cw, s.s(14.0)),
            );
            words(
                b,
                s,
                &names[i],
                nr,
                Align2::CENTER_CENTER,
                name_size,
                if own { W::SemiBold } else { W::Regular },
                if own { p.ink1 } else { p.ink2 },
            );
            let vr = Rect::from_min_size(pos2(x - cw * 0.5, nr.bottom()), vec2(cw, s.s(13.0)));
            let v = if own {
                "0 dB".to_string()
            } else if -rel < 0.5 {
                "0".to_string()
            } else {
                format!("\u{2212}{:.0}", -rel)
            };
            words(
                b,
                s,
                &v,
                vr,
                Align2::CENTER_CENTER,
                10.0,
                W::Regular,
                p.ink3,
            );
        }
    });
}

/// Every mic's matrix: a row per piece, a cell per mic shaded by how loud the piece is in it;
/// the selected mic's column ringed. A click turns a cell off or on (`b{p}.{m}.on`, which
/// unloads its samples), Alt-click flips its polarity, a drag up or down sets its level.
#[allow(clippy::too_many_arguments)]
fn matrix(
    app: &mut CantusApp,
    ui: &egui::Ui,
    track: TrackId,
    spec: &NativeSpec,
    def: &KitDef,
    s: &Style,
    r: Rect,
    lk: &mut Look,
) {
    let p = Pal::of(s);
    let rows = def.pieces.len().max(1);
    let cols = def.mics.len().max(1);
    let label_w = s.s(56.0);
    let rh = (r.height() / rows as f32).min(s.s(16.0));
    let cw = ((r.width() - label_w) / cols as f32).min(s.s(40.0));
    let alt = ui.input(|i| i.modifiers.alt);
    for (i, pd) in def.pieces.iter().enumerate() {
        let y = r.top() + rh * i as f32;
        with_brush(ui.painter(), |b| {
            let lr = Rect::from_min_size(pos2(r.left(), y), vec2(label_w - s.s(4.0), rh));
            words(
                b,
                s,
                &def::piece_name(pd),
                lr,
                Align2::LEFT_CENTER,
                9.0,
                W::Regular,
                p.ink2,
            );
        });
        for (j, m) in def.mics.iter().enumerate() {
            let cell =
                Rect::from_min_size(pos2(r.left() + label_w + cw * j as f32, y), vec2(cw, rh))
                    .shrink(s.s(0.75));
            let id = ui.id().with(("tymp-cell", track.0, pd.index, m.output.0));
            let resp = ui.interact(cell, id, Sense::click_and_drag());
            let base = format!("b{}.{}.", pd.index, m.output.0);
            let on = spec.get(&format!("{base}on")) >= 0.5;
            let inv = spec.get(&format!("{base}inv")) >= 0.5;
            let rel = relative_db(def, spec, pd.index, m);
            // The cell's level: its parameter's menu and *Learn MIDI controller* (ADR 0062);
            // while the learn mode has it, the cell takes no value from the click.
            let learning =
                crate::controllers::stepped_inst(app, ui, track, spec, &format!("{base}db"), &resp);
            if !learning && resp.clicked() {
                if alt {
                    app.set_inst_value(
                        track,
                        &format!("{base}inv"),
                        if inv { 0.0 } else { 1.0 },
                        false,
                    );
                } else {
                    app.set_inst_value(
                        track,
                        &format!("{base}on"),
                        if on { 0.0 } else { 1.0 },
                        false,
                    );
                }
            }
            if !learning && resp.dragged() {
                let k = format!("{base}db");
                let v = (spec.get(&k) - resp.drag_delta().y * 0.25).clamp(-96.0, 12.0);
                app.set_inst_value(track, &k, v, true);
            }
            if resp.drag_stopped() {
                app.end_gesture();
            }
            let db_s = if on {
                format!("{rel:.0}")
            } else {
                t("tympanum.mic.cell_off")
            };
            let piece_name = def::piece_name(pd);
            let mic_name = def::mic_name(def, m);
            let _ = iu::hint(resp, || {
                tf(
                    "tympanum.mic.cell_tip",
                    &[("piece", &piece_name), ("mic", &mic_name), ("db", &db_s)],
                )
            });
            with_brush(ui.painter(), |b| {
                let f = ((rel + 40.0) / 40.0).clamp(0.0, 1.0);
                let fill = if on {
                    mix(p.well, p.accent, 0.12 + 0.75 * f)
                } else {
                    p.well
                };
                b.rect(b.snap_rect(cell), Fill::Solid(fill));
                if inv && on {
                    b.circle(
                        cell.center(),
                        (rh * 0.18).max(s.s(1.5)),
                        Fill::Solid(p.on_ink),
                    );
                }
            });
        }
    }
    // The selected mic's column.
    if let Some(j) = def.mics.iter().position(|m| m.output.0 == lk.mic) {
        with_brush(ui.painter(), |b| {
            let col = Rect::from_min_size(
                pos2(r.left() + label_w + cw * j as f32, r.top()),
                vec2(cw, rh * rows as f32),
            );
            b.rrect_stroke(b.snap_rect(col), s.s(2.0), s.s(1.5), p.accent);
        });
    }
}
