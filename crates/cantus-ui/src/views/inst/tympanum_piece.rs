//! The selected piece's panel, at the editor's right: its name and size with *Hear*; its
//! articulations on their keys (a click plays one); where it is struck (a click on the head
//! plays it struck there: the centre, towards the edge, the rim, a cymbal's bell); its
//! sound (tune, damping, a snare's wires, level — `p{n}.*`); and which mics hear it, with
//! the way to the mixer. Scrolls when the window is shorter than it.

use super::cards::{knob_cell, value_words, Num};
use super::iu::{self, words};
use super::{def, press, Look};
use crate::app::CantusApp;
use crate::i18n::{t, tf};
use crate::kit::color::mix;
use crate::kit::glyph_browser::Glyph as Gb;
use crate::kit::{with_brush, Fill, Style};
use crate::views::mixer::look::{Pal, W};
use cantus_core::inst::tympanum_kit::{KitDef, PieceDef, PieceKind};
use cantus_core::{NativeSpec, TrackId};
use egui::{pos2, vec2, Align2, Pos2, Rect, Sense, Vec2};

const ART_ROW: f32 = 24.0;

/// The mics that hear piece `p`: its own, and every mic that hears the whole kit, less any
/// whose cell for it is off.
pub fn heard_by(def: &KitDef, spec: &NativeSpec, p: u16) -> Vec<String> {
    def.mics
        .iter()
        .filter(|m| m.main_for.contains(&p) || m.main_for.is_empty())
        .filter(|m| {
            let k = format!("b{p}.{}.on", m.output.0);
            spec.kind.param(&k).is_none() || spec.get(&k) >= 0.5
        })
        .map(|m| def::mic_name(def, m))
        .collect()
}

/// The articulation a point of the head plays: `d` is its distance from the centre, 0…1.
fn struck(pd: &PieceDef, d: f32) -> usize {
    let find = |ids: &[&str]| {
        ids.iter()
            .find_map(|id| pd.articulations.iter().position(|a| a.id == *id))
    };
    let at = if def::is_cymbal(pd.kind) {
        if d < 0.25 {
            find(&["bell"])
        } else if d > 0.85 {
            find(&["edge"])
        } else {
            find(&["bow", "edge", "closed"])
        }
    } else if d > 0.92 {
        find(&["rimshot", "rim"])
    } else if d > 0.6 {
        find(&["edge", "center"])
    } else {
        find(&["center"])
    };
    at.unwrap_or(0)
}

/// The panel in `r`.
#[allow(clippy::too_many_arguments)]
pub fn panel(
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
    let Some(pd) = def
        .pieces
        .iter()
        .find(|x| x.index == lk.piece)
        .or(def.pieces.first())
    else {
        return;
    };
    let pad = s.s(14.0);
    let inner = r.shrink2(vec2(pad, s.s(4.0)));
    let mut child = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(r)
            .id_salt(("tymp-piece", track.0)),
    );
    child.set_clip_rect(r.shrink(s.s(1.0)).intersect(ui.clip_rect()));
    if ui.ctx().pointer_hover_pos().is_some_and(|q| r.contains(q)) {
        lk.piece_scroll -= ui.input(|i| i.smooth_scroll_delta.y);
    }
    let mut y = inner.top() + s.s(6.0) - lk.piece_scroll;
    let x = inner.left();
    let w = inner.width();
    // The head: name, size, Hear.
    let name = def::piece_name(pd);
    let hear = t("tympanum.hear");
    let hw = iu::width(ui, s, &hear, 12.0, W::Medium) + s.s(38.0);
    let hr = Rect::from_min_size(pos2(x + w - hw, y + s.s(4.0)), vec2(hw, s.s(28.0)));
    let hresp = iu::button(
        &child,
        s,
        hr,
        child.id().with("hear"),
        &hear,
        iu::Lead::Glyph(Gb::Speaker),
        false,
        true,
    );
    let hresp = iu::hint(hresp, || t("tympanum.hear_tip"));
    if super::pressed(&hresp) {
        if let Some(a) = pd.articulations.first() {
            press(app, track, lk, a.key, pd.index);
        }
    }
    with_brush(child.painter(), |b| {
        let nr = Rect::from_min_max(pos2(x, y), pos2(hr.left() - s.s(8.0), y + s.s(24.0)));
        words(
            b,
            s,
            &name,
            nr,
            Align2::LEFT_CENTER,
            17.0,
            W::SemiBold,
            p.ink1,
        );
        let sr = Rect::from_min_size(pos2(x, y + s.s(24.0)), vec2(w, s.s(16.0)));
        words(
            b,
            s,
            &def::size_text(pd),
            sr,
            Align2::LEFT_CENTER,
            11.0,
            W::Regular,
            p.ink3,
        );
    });
    y += s.s(50.0);
    // The articulations.
    with_brush(child.painter(), |b| {
        let hr = Rect::from_min_size(pos2(x, y), vec2(w, s.s(20.0)));
        iu::head(b, s, hr, &t("tympanum.articulations"), &t("tympanum.key"));
    });
    y += s.s(24.0);
    for (i, a) in pd.articulations.iter().enumerate() {
        let row = Rect::from_min_size(pos2(x - s.s(6.0), y), vec2(w + s.s(12.0), s.s(ART_ROW)));
        let resp = child.interact(row, child.id().with(("art", i)), Sense::click_and_drag());
        let an = def::said(&a.label);
        let kn = crate::controllers::note_name(a.key);
        let resp = iu::hint(resp, || {
            tf("tympanum.art_tip", &[("name", &an), ("key", &kn)])
        });
        if super::pressed(&resp) {
            press(app, track, lk, a.key, pd.index);
        }
        let lit = lk.last_key == a.key
            || (i == 0 && pd.articulations.iter().all(|x| x.key != lk.last_key));
        with_brush(child.painter(), |b| {
            if lit || resp.hovered() {
                let g = if lit {
                    mix(p.surface, p.ink1, 0.06)
                } else {
                    mix(p.surface, p.ink1, 0.03)
                };
                b.rrect(b.snap_rect(row), s.s(4.0), Fill::Solid(g));
            }
            // The play mark.
            let c = pos2(row.left() + s.s(12.0), row.center().y);
            let (h, d) = (s.s(3.5), s.s(3.0));
            let ink = if lit { p.accent } else { p.ink3 };
            b.polygon(
                &[
                    pos2(c.x - d * 0.6, c.y - h),
                    pos2(c.x + d, c.y),
                    pos2(c.x - d * 0.6, c.y + h),
                ],
                ink,
            );
            let nr = Rect::from_min_max(
                pos2(row.left() + s.s(24.0), row.top()),
                pos2(row.right() - s.s(70.0), row.bottom()),
            );
            words(
                b,
                s,
                &an,
                nr,
                Align2::LEFT_CENTER,
                12.0,
                if lit { W::SemiBold } else { W::Regular },
                p.ink1,
            );
            let kr = Rect::from_min_max(
                pos2(row.right() - s.s(70.0), row.top()),
                pos2(row.right() - s.s(30.0), row.bottom()),
            );
            words(
                b,
                s,
                &kn,
                kr,
                Align2::RIGHT_CENTER,
                11.0,
                W::Regular,
                p.ink2,
            );
            let mr = Rect::from_min_max(
                pos2(row.right() - s.s(28.0), row.top()),
                pos2(row.right() - s.s(6.0), row.bottom()),
            );
            words(
                b,
                s,
                &a.key.to_string(),
                mr,
                Align2::RIGHT_CENTER,
                11.0,
                W::Regular,
                p.ink3,
            );
        });
        y += s.s(ART_ROW);
    }
    y += s.s(4.0);
    let note = if def.modelled {
        t("tympanum.modelled_note")
    } else {
        tf("tympanum.recorded_note", &[("n", &8)])
    };
    let used = with_brush(child.painter(), |b| {
        iu::prose(
            b,
            &child,
            s,
            Rect::from_min_size(pos2(x, y), vec2(w, s.s(40.0))),
            &note,
            p.ink3,
            Align2::LEFT_TOP,
        )
    });
    y += used + s.s(14.0);
    // Where it is struck.
    with_brush(child.painter(), |b| {
        let hr = Rect::from_min_size(pos2(x, y), vec2(w, s.s(20.0)));
        iu::head(b, s, hr, &t("tympanum.struck"), &t("tympanum.struck_hint"));
    });
    y += s.s(26.0);
    y = struck_part(app, &child, track, spec, pd, s, pos2(x, y), w, lk);
    // Its sound.
    with_brush(child.painter(), |b| {
        let hr = Rect::from_min_size(pos2(x, y), vec2(w, s.s(20.0)));
        iu::head(b, s, hr, &t("tympanum.sound"), "");
    });
    y += s.s(22.0);
    let n = pd.index;
    let mut cells: Vec<(String, &str, Num)> = vec![
        (
            format!("p{n}.tune"),
            "tympanum.tune",
            Num::new(-12.0, 12.0, 0.0, "st").signed(),
        ),
        (
            format!("p{n}.damp"),
            "tympanum.damp",
            Num::new(0.0, 1.0, 0.0, "%").pct(),
        ),
    ];
    if pd.kind == PieceKind::Snare {
        cells.push((
            format!("p{n}.wires"),
            "tympanum.wires",
            Num::new(0.0, 2.0, 1.0, "%").pct(),
        ));
    }
    cells.push((
        format!("p{n}.level"),
        "tympanum.level",
        Num::new(-96.0, 12.0, 0.0, "dB"),
    ));
    let cw = (w + s.s(12.0)) / cells.len() as f32;
    for (i, (key, label, num)) in cells.iter().enumerate() {
        let cr = Rect::from_min_size(pos2(x - s.s(6.0) + cw * i as f32, y), vec2(cw, s.s(78.0)));
        knob_cell(app, &child, track, spec, s, cr, key, &t(label), *num);
    }
    y += s.s(84.0);
    // Heard by.
    let open = t("tympanum.open_mixer");
    let ow = iu::width(ui, s, &open, 11.0, W::Medium) + s.s(2.0);
    let or = Rect::from_min_size(pos2(x + w - ow, y), vec2(ow, s.s(20.0)));
    let oresp = child.interact(or, child.id().with("open-mixer"), Sense::click());
    // fx/final — the help line names where the link goes (as Chorda's does).
    let oresp = iu::hint(oresp, || t("tympanum.open_mixer_tip"));
    if oresp.clicked() {
        if let Some(m) = def::main_of(def, pd.index) {
            lk.mic = m.output.0;
        }
        app.inst.view = super::super::View::Mixer; // r40:integration — the frame's Mixer
    }
    let mics = heard_by(def, spec, pd.index);
    let list = if mics.is_empty() {
        t("tympanum.heard_by_none")
    } else {
        mics.join(", ")
    };
    let used = with_brush(child.painter(), |b| {
        let hr = Rect::from_min_max(pos2(x, y), pos2(or.left() - s.s(6.0), y + s.s(20.0)));
        words(
            b,
            s,
            &t("tympanum.heard_by"),
            hr,
            Align2::LEFT_CENTER,
            12.0,
            W::SemiBold,
            p.ink1,
        );
        words(
            b,
            s,
            &open,
            or,
            Align2::RIGHT_CENTER,
            11.0,
            W::Medium,
            if oresp.hovered() { p.ink1 } else { p.accent },
        );
        iu::prose(
            b,
            &child,
            s,
            Rect::from_min_size(pos2(x, y + s.s(22.0)), vec2(w, s.s(60.0))),
            &list,
            p.ink2,
            Align2::LEFT_TOP,
        )
    });
    y += s.s(22.0) + used + s.s(10.0);
    let content = y + lk.piece_scroll - r.top();
    lk.piece_scroll = lk.piece_scroll.clamp(0.0, (content - r.height()).max(0.0));
}

/// The head seen from above with the last hit's spot; at its right where hits land (from
/// the centre, how widely) and the last hit's strength. Returns the y under it.
#[allow(clippy::too_many_arguments)]
fn struck_part(
    app: &mut CantusApp,
    ui: &egui::Ui,
    track: TrackId,
    spec: &NativeSpec,
    pd: &PieceDef,
    s: &Style,
    at: Pos2,
    w: f32,
    lk: &mut Look,
) -> f32 {
    let p = Pal::of(s);
    let d = s.s(96.0);
    let c = pos2(at.x + d * 0.5, at.y + d * 0.5);
    let rad = d * 0.5;
    let hit = Rect::from_center_size(c, vec2(d, d));
    let resp = ui.interact(
        hit,
        ui.id().with(("struck", pd.index)),
        Sense::click_and_drag(),
    );
    let resp = iu::hint(resp, || t("tympanum.struck_tip"));
    if super::pressed(&resp) {
        if let Some(q) = resp.interact_pointer_pos() {
            let v = (q - c) / rad;
            let dist = v.length().min(1.0);
            let a = struck(pd, dist);
            lk.piece = pd.index;
            let key = format!("p{}.pos", pd.index);
            if spec.kind.param(&key).is_some() {
                app.set_inst_value(track, &key, (dist * 2.0 - 1.0).clamp(-1.0, 1.0), false);
            }
            if let Some(art) = pd.articulations.get(a) {
                press(app, track, lk, art.key, pd.index);
            }
            if let Some(h) = lk.hit.as_mut() {
                h.2 = v.clamp(Vec2::splat(-1.0), Vec2::splat(1.0));
            }
        }
    }
    let n = pd.index;
    let from = Num::new(-1.0, 1.0, 0.0, "%").pct().signed();
    let spread = Num::new(0.0, 1.0, 0.06, "%").pct().pm();
    let fk = format!("p{n}.pos");
    let sk = format!("p{n}.spread");
    let (fv, sv) = (from.value(spec, &fk), spread.value(spec, &sk));
    let last = lk.hit.filter(|h| h.0 == n);
    with_brush(ui.painter(), |b| {
        let cym = def::is_cymbal(pd.kind);
        b.circle(c, rad, Fill::Solid(p.well));
        b.circle_stroke(c, rad - s.s(1.0), s.s(1.2), mix(p.well, p.ink1, 0.45));
        // The zones: the playing area (dashed rings), a cymbal's bell.
        for f in [0.33, 0.66] {
            let pts: Vec<Pos2> = (0..=48)
                .map(|i| {
                    let a = i as f32 / 48.0 * std::f32::consts::TAU;
                    c + vec2(cantus_dsp::math::cos(a), cantus_dsp::math::sin(a)) * rad * f
                })
                .collect();
            b.dashed(&pts, b.px(), mix(p.well, p.ink1, 0.25), s.s(2.0), s.s(3.0));
        }
        if cym {
            b.circle(c, rad * 0.22, Fill::Solid(mix(p.well, p.ink1, 0.18)));
        }
        // The spot: the last hit's, else where the offset puts it (−1 the centre … +1 the
        // edge; 0 the piece's own spot, a third of the way out).
        let own = 1.0 / 3.0;
        let r_at = if fv < 0.0 {
            own * (1.0 + fv)
        } else {
            own + (1.0 - own) * fv
        };
        let spot = last.map_or(vec2(r_at, 0.0), |h| h.2);
        let q = c + spot * rad * 0.92;
        b.circle(
            q,
            s.s(5.5),
            Fill::Solid(crate::kit::color::with_alpha(p.accent, 0.3)),
        );
        b.circle(q, s.s(3.5), Fill::Solid(p.accent));
        // The figures at its right.
        let fx = c.x + rad + s.s(14.0);
        let fw = at.x + w - fx;
        let rows = [
            (
                t("tympanum.from_centre"),
                from.text(fv),
                Some((fv + 1.0) * 0.5),
            ),
            (t("tympanum.spread"), spread.text(sv), Some(sv)),
            (
                t("tympanum.last_hit"),
                (
                    last.map_or("\u{2014}".to_string(), |h| h.1.to_string()),
                    String::new(),
                ),
                None,
            ),
        ];
        for (i, (label, (num, unit), bar)) in rows.iter().enumerate() {
            let ry = at.y + s.s(4.0) + i as f32 * s.s(32.0);
            let lr = Rect::from_min_size(pos2(fx, ry), vec2(fw, s.s(16.0)));
            words(
                b,
                s,
                label,
                lr,
                Align2::LEFT_CENTER,
                11.0,
                W::Regular,
                p.ink2,
            );
            value_words(b, s, &p, lr, num, unit, Align2::RIGHT_CENTER);
            if let Some(v) = bar {
                let br = Rect::from_min_size(
                    pos2(fx, lr.bottom() + s.s(4.0)),
                    vec2(fw * 0.35, s.s(3.0)),
                );
                b.rrect(b.snap_rect(br), s.s(1.5), Fill::Solid(p.bar_track));
                let fr =
                    Rect::from_min_size(br.min, vec2(br.width() * v.clamp(0.0, 1.0), br.height()));
                b.rrect(b.snap_rect(fr), s.s(1.5), Fill::Solid(p.ink2));
                crate::views::inspector::look::handle(
                    b,
                    &p,
                    pos2(fr.right(), br.center().y),
                    false,
                    1.0,
                );
            }
        }
    });
    at.y + d + s.s(14.0)
}
