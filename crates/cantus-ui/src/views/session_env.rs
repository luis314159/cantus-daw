//! A session clip's own envelopes (ADR 0077) — Live's clip envelopes: pick a parameter of
//! the clip's track (its volume and pan, its strip's, its instrument's, its inserts', the
//! last one tweaked) and draw its curve over the clip's loop. A click adds a point (snapped
//! to a sixteenth; Shift: free), a drag moves one, a double-click or a right-click on one
//! deletes it; *Clear* takes the envelope away. The curve is the automation editor's
//! (`auto_curve`), so it reads exactly as what plays.

use super::clip::{card as card_frame, label, menu};
use crate::app::CantusApp;
use crate::i18n::t;
use crate::kit::hint::{self, Caps};
use crate::kit::text::{self, Elide, Policy, Role, Text};
use crate::kit::{with_brush, Fill, Style};
use crate::views::auto_curve::{self, Frame, Ink};
use cantus_core::automation::{AutoPoint, AutoTarget, Automation};
use cantus_core::session::{SessionCommand as S, SlotRef};
use cantus_core::{MergeKey, Ticks, PPQ};
use egui::{pos2, vec2, Align2, Id, Rect, Sense, Ui};

/// The editor's state: the parameter shown, the point being dragged.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct EnvState {
    pub target: Option<AutoTarget>,
    drag: Option<usize>,
}

/// What a track's clip envelopes can drive, in the menu's order.
pub fn targets(app: &CantusApp, track: cantus_core::TrackId) -> Vec<AutoTarget> {
    let p = app.project();
    let Some(tr) = p.track(track) else {
        return Vec::new();
    };
    let mut v = vec![AutoTarget::TrackGain(track), AutoTarget::TrackPan(track)];
    if let Some(sid) = tr.strip {
        v.extend([
            AutoTarget::StripGain(sid),
            AutoTarget::StripPan(sid),
            AutoTarget::StripWidth(sid),
        ]);
        if let Some(fx) = p.effects_of(sid) {
            for e in fx.iter() {
                v.extend(app.effect_params(e.id));
            }
        }
    }
    v.extend(app.instrument_params(track));
    if let Some(last) = app.automation.last_tweaked.clone() {
        if !v.contains(&last) && cantus_core::automation::range::range_of(p, &last).is_some() {
            v.push(last);
        }
    }
    v
}

fn set(app: &mut CantusApp, slot: SlotRef, envelopes: Vec<Automation>, merge: bool) {
    let cmd = cantus_core::Command::Session(S::SetSlotEnvelopes { slot, envelopes });
    if merge {
        app.exec_merging(
            cmd,
            MergeKey::of(&("session-env", slot.track.0, slot.scene.0)),
        );
    } else {
        app.exec(cmd);
    }
}

pub fn card_ui(app: &mut CantusApp, ui: &mut Ui, s: &Style, r: Rect, slot: SlotRef) {
    card_frame(ui, s, r, "session.env.title");
    let k = s.k;
    let Some(c) = app.project().slot_clip(slot).cloned() else {
        return;
    };
    let options = targets(app, slot.track);
    // The parameter shown: the one chosen, else the first the clip has an envelope of.
    let chosen = app
        .session_view
        .env
        .target
        .clone()
        .filter(|t| options.contains(t) || c.envelope(t).is_some())
        .or_else(|| c.envelopes.first().map(|e| e.target.clone()));
    let names: Vec<String> = options
        .iter()
        .map(|t| {
            let n = crate::automation::target_name(app.project(), t);
            if c.envelope(t).is_some() {
                crate::i18n::tf("session.env.drawn", &[("name", &n)])
            } else {
                n
            }
        })
        .collect();
    let top = Rect::from_min_size(
        pos2(r.left() + 6.0 * k, r.top() + 24.0 * k),
        vec2(r.width() - 12.0 * k, 22.0 * k),
    );
    let lr = Rect::from_min_size(top.min, vec2(64.0 * k, top.height()));
    label(ui, s, lr, "session.env.param", false);
    let clear = Rect::from_min_max(
        pos2(top.right() - 56.0 * k, top.top() + 1.0 * k),
        pos2(top.right(), top.bottom() - 1.0 * k),
    );
    let field = Rect::from_min_max(
        pos2(lr.right() + 6.0 * k, top.top() + 1.0 * k),
        pos2(clear.left() - 6.0 * k, top.bottom() - 1.0 * k),
    );
    let value = chosen.as_ref().map_or_else(
        || t("session.env.choose"),
        |tg| crate::automation::target_name(app.project(), tg),
    );
    let at = chosen
        .as_ref()
        .and_then(|tg| options.iter().position(|o| o == tg));
    let id = Id::new(("session-env", slot.track.0, slot.scene.0));
    if let Some(i) = menu(
        ui,
        s,
        field,
        id.with("param"),
        &value,
        &names,
        at,
        "session.env.param_tip",
        false,
    ) {
        app.session_view.env.target = Some(options[i].clone());
    }
    let has = chosen.as_ref().is_some_and(|tg| c.envelope(tg).is_some());
    let cresp = ui.interact(clear, id.with("clear"), Sense::click());
    with_brush(ui.painter(), |b| {
        super::paint::key(
            b,
            clear,
            s,
            &t("session.env.clear"),
            None,
            cresp.hovered() && has,
        )
    });
    if cresp.hovered() {
        hint::set_text(ui.ctx(), &t("session.env.clear_tip"), "", Caps::NONE);
    }
    if cresp.clicked() && has {
        let tg = chosen.clone().expect("has");
        let envs: Vec<Automation> = c
            .envelopes
            .iter()
            .filter(|e| e.target != tg)
            .cloned()
            .collect();
        set(app, slot, envs, false);
    }
    let area = Rect::from_min_max(
        pos2(r.left() + 6.0 * k, top.bottom() + 6.0 * k),
        pos2(r.right() - 6.0 * k, r.bottom() - 6.0 * k),
    );
    if area.height() < 20.0 * k || area.width() < 40.0 * k {
        return;
    }
    let Some(target) = chosen else {
        with_brush(ui.painter(), |b| {
            b.rrect(area, s.r(2.0), Fill::Solid(s.t.well));
            let tx = Text::key("session.env.empty", Role::Body, s.t.placeholder)
                .align(Align2::CENTER_CENTER)
                .policy(Policy::Elide(Elide::End));
            let _ = text::paint(b, area, &tx, s.tk());
        });
        return;
    };
    draw(app, ui, s, area, slot, &c, target);
}

/// The curve over the clip's loop, and its points' gestures.
fn draw(
    app: &mut CantusApp,
    ui: &mut Ui,
    s: &Style,
    area: Rect,
    slot: SlotRef,
    c: &cantus_core::session::SessionClip,
    target: AutoTarget,
) {
    let k = s.k;
    let from = c.clip.offset;
    let span = c.pass().max(Ticks(1));
    // The curve inset by a point's radius: the first and last points show whole.
    let inner = area.shrink2(egui::vec2(6.0 * k, 0.0));
    let f = Frame {
        x0: inner.left() - from.0 as f32 * (inner.width() / span.0 as f32),
        px_per_tick: inner.width() / span.0 as f32,
        top: area.top() + 4.0 * k,
        bottom: area.bottom() - 4.0 * k,
        clip_left: area.left(),
        clip_right: area.right(),
        origin: Ticks::ZERO,
    };
    let env = c.envelope(&target).cloned();
    let colour = s.t.hue_mod;
    let ground = s.t.well;
    let mut pts = Vec::new();
    with_brush(
        &ui.painter().with_clip_rect(area.intersect(ui.clip_rect())),
        |b| {
            b.rrect(area, s.r(2.0), Fill::Solid(ground));
            // A line a beat, stronger a bar.
            let beats = (span.0 / PPQ).max(1);
            for i in 0..=beats {
                let x = f.x((from.0 + i * PPQ) as f64);
                let bar = i % 4 == 0;
                b.vline(
                    x,
                    area.top(),
                    area.bottom(),
                    b.px(),
                    if bar { s.t.grid_bar } else { s.t.grid },
                );
            }
            if let Some(e) = &env {
                auto_curve::trace(e, from, from + span, &f, 3.0 * k, &mut pts);
                auto_curve::paint(b, &pts, f.bottom, Ink::of(colour, 1.5 * k, s));
                for (i, p) in e.points.iter().enumerate() {
                    let lit = app.session_view.env.drag == Some(i);
                    auto_curve::point(b, f.point(p.tick, p.value), 3.0 * k, colour, ground, lit);
                }
            }
        },
    );
    let id = Id::new(("session-env-area", slot.track.0, slot.scene.0));
    let resp = ui.interact(area, id, Sense::click_and_drag());
    if resp.hovered() {
        let v = resp.hover_pos().map(|p| f.value(p.y)).unwrap_or(0.0);
        let mut text = String::new();
        crate::automation::write_value(app.project(), &target, v, &mut text);
        hint::set_text(ui.ctx(), &t("session.env.draw_tip"), &text, Caps::PARAM);
    }
    let fine = ui.input(|i| i.modifiers.shift);
    let snap = |x: f32| {
        let tk = f.tick(x).round() as i64;
        let tk = if fine {
            tk
        } else {
            (tk as f64 / (PPQ / 4) as f64).round() as i64 * (PPQ / 4)
        };
        Ticks(tk.clamp(from.0, from.0 + span.0))
    };
    let under = |e: &Automation, p: egui::Pos2| auto_curve::point_at(e, &f, p, 6.0 * k);
    let mut envs = c.envelopes.clone();
    let index = envs.iter().position(|e| e.target == target);
    // Delete a point: double-click or right-click on it.
    if resp.double_clicked() || resp.secondary_clicked() {
        if let (Some(p), Some(e)) = (resp.interact_pointer_pos(), env.as_ref()) {
            if let Some(i) = under(e, p) {
                let mut e = e.clone();
                e.points.remove(i);
                let ix = index.expect("has");
                if e.points.is_empty() {
                    envs.remove(ix);
                } else {
                    envs[ix] = e;
                }
                set(app, slot, envs, false);
                return;
            }
        }
    }
    if resp.drag_started() || resp.clicked() {
        if let Some(p) = resp.interact_pointer_pos() {
            let hit = env.as_ref().and_then(|e| under(e, p));
            match hit {
                Some(i) => app.session_view.env.drag = Some(i),
                None => {
                    // A new point where it was pressed.
                    let pt = AutoPoint::new(snap(p.x), f.value(p.y));
                    let mut e = env
                        .clone()
                        .unwrap_or_else(|| Automation::new(target.clone(), Vec::new()));
                    let at = e.points.partition_point(|q| q.tick <= pt.tick);
                    e.points.insert(at, pt);
                    match index {
                        Some(ix) => envs[ix] = e,
                        None => envs.push(e),
                    }
                    app.session_view.env.drag = Some(at);
                    set(app, slot, envs.clone(), true);
                }
            }
        }
    }
    if let (true, Some(i), Some(p)) = (
        resp.dragged(),
        app.session_view.env.drag,
        resp.interact_pointer_pos(),
    ) {
        if let Some(ix) = envs.iter().position(|e| e.target == target) {
            let e = &mut envs[ix];
            if i < e.points.len() {
                let lo = if i > 0 { e.points[i - 1].tick } else { from };
                let hi = e.points.get(i + 1).map_or(from + span, |q| q.tick);
                e.points[i].tick = snap(p.x).clamp(lo, hi);
                e.points[i].value = f.value(p.y);
                set(app, slot, envs, true);
            }
        }
    }
    if resp.drag_stopped() || (resp.clicked() && !resp.dragged()) {
        app.session_view.env.drag = None;
        app.end_gesture();
    }
}

/// The card (see the module docs).
pub fn card(app: &mut CantusApp, ui: &mut Ui, s: &Style, r: Rect, slot: SlotRef) {
    card_ui(app, ui, s, r, slot);
}
