//! Shaping crossfades and gain envelopes by hand (ADR 0070): the crossfade's square
//! dragged (its length, about its centre), double-clicked (its editor), right-clicked (its
//! menu); an envelope's points dragged, added on the line, deleted with a double-click,
//! bent by their segment's handle, their segment's shape from the menu. Every gesture is
//! one undo step (`SetClipAudio` or the crossfade's batch, merged by the clip's key).

use super::super::Handle;
use super::{apply, HandleDrag};
use crate::app::CantusApp;
use crate::audio_clips as ac;
use crate::clipfx::{self as fx, env_db, env_v};
use crate::clips::ClipRef;
use crate::i18n::t;
use crate::kit;
use crate::kit::hint::{self, Caps};
use crate::kit::menu::{Check, Item};
use crate::kit::text::Src;
use cantus_core::crossfade::XfadeCurve;
use cantus_core::{Shape, Ticks};
use egui::Pos2;
use std::fmt::Write as _;

/// What a crossfade or envelope drag remembers from its press.
#[derive(Clone, Debug, PartialEq)]
pub enum Drag {
    /// The crossfade's length at the press.
    Xfade {
        len0: Ticks,
        base: Box<fx::XfadeBase>,
    },
    /// A point: its tick and height at the press, the area's height, the clip's direction.
    Point {
        i: usize,
        tick0: Ticks,
        v0: f32,
        height: f32,
        reverse: bool,
    },
    /// A bend: the line's height half-way along at the press.
    Bend { i: usize, v0: f32, height: f32 },
}

/// Whether a handle is one of this module's.
pub fn owns(h: Handle) -> bool {
    matches!(
        h,
        Handle::Xfade | Handle::EnvPoint(_) | Handle::EnvBend(_) | Handle::EnvLine(_)
    )
}

/// The pointer over a handle.
pub fn cursor(h: Handle) -> egui::CursorIcon {
    match h {
        Handle::Gain | Handle::EnvBend(_) => egui::CursorIcon::ResizeVertical,
        Handle::EnvPoint(_) => egui::CursorIcon::Move,
        Handle::EnvLine(_) => egui::CursorIcon::Crosshair,
        _ => egui::CursorIcon::ResizeHorizontal,
    }
}

/// The button went down on one of this module's handles: what the drag works on (a press
/// on the line adds a point there and drags it).
pub fn begin(app: &mut CantusApp, r: ClipRef, handle: Handle) -> Option<(Handle, Drag)> {
    let hits = app.arrangement.audio_handles(r);
    match handle {
        Handle::Xfade => {
            let base = fx::xfade_base(app, r)?;
            Some((
                handle,
                Drag::Xfade {
                    len0: base.len(),
                    base: Box::new(base),
                },
            ))
        }
        Handle::EnvLine(t) => {
            let (_, a) = ac::region(app.project(), r)?;
            let (next, i) = fx::add_point(a, Ticks(i64::from(t)), None);
            apply(app, r, next);
            let i16 = i.min(u16::MAX as usize) as u16;
            let d = point_drag(app, r, i, hits.and_then(|h| h.env))?;
            Some((Handle::EnvPoint(i16), d))
        }
        Handle::EnvPoint(i) => {
            let d = point_drag(app, r, usize::from(i), hits.and_then(|h| h.env))?;
            Some((handle, d))
        }
        Handle::EnvBend(i) => {
            let (clip, a) = ac::region(app.project(), r)?;
            let _ = clip;
            let env = fx::envelope_of(a);
            let (p, q) = (
                env.points.get(usize::from(i))?,
                env.points.get(usize::from(i) + 1)?,
            );
            let mid = (p.tick.0 + (q.tick.0 - p.tick.0) / 2) as f64;
            let height = hits.and_then(|h| h.env).map_or(100.0, |e| e.area.height());
            Some((
                handle,
                Drag::Bend {
                    i: usize::from(i),
                    v0: env_v(env.db_at(mid)),
                    height,
                },
            ))
        }
        _ => None,
    }
}

fn point_drag(
    app: &CantusApp,
    r: ClipRef,
    i: usize,
    hits: Option<super::super::clipfx::EnvHits>,
) -> Option<Drag> {
    let (_, a) = ac::region(app.project(), r)?;
    let p = *fx::envelope_of(a).points.get(i)?;
    Some(Drag::Point {
        i,
        tick0: p.tick,
        v0: env_v(p.db),
        height: hits.map_or(100.0, |e| e.area.height()).max(1.0),
        reverse: a.reverse,
    })
}

/// The pointer moved to `pos` while dragging (`fine`: Shift, a tenth of the move).
pub fn update(app: &mut CantusApp, d: &HandleDrag, drag: &Drag, pos: Pos2, fine: bool) {
    let dx = Ticks(((pos.x - d.press.x) / d.px_per_tick).round() as i64);
    let scale = if fine { 0.1 } else { 1.0 };
    match *drag {
        Drag::Xfade { len0, ref base } => {
            let len = Ticks((len0.0 + 2 * dx.0).max(1));
            app.set_crossfade_length_from(d.r, base, len, true);
        }
        Drag::Point {
            i,
            tick0,
            v0,
            height,
            reverse,
        } => {
            let dt = if reverse { -dx } else { dx };
            let v = v0 + (pos.y - d.press.y) / height * scale;
            let next = fx::move_point(&d.region0, i, tick0 + dt, env_db(v));
            apply(app, d.r, next);
        }
        Drag::Bend { i, v0, height } => {
            let v = v0 + (pos.y - d.press.y) / height.max(1.0) * scale;
            let next = fx::bend_to(&d.region0, i, env_db(v));
            apply(app, d.r, next);
        }
    }
}

/// A double-click on one of this module's handles: the crossfade's editor; a point
/// deleted; a bend straightened; a point added on the line. `false` for another handle.
pub fn reset(app: &mut CantusApp, r: ClipRef, handle: Handle) -> bool {
    let Some((_, a)) = ac::region(app.project(), r) else {
        return false;
    };
    let next = match handle {
        Handle::Xfade => {
            let at = app
                .arrangement
                .audio_handles(r)
                .and_then(|h| h.xfade)
                .map_or(Pos2::ZERO, |x| x.square.left_bottom());
            app.arrangement.audio.xfade_editor = Some((r, at));
            return true;
        }
        Handle::EnvPoint(i) => fx::remove_point(a, usize::from(i)),
        Handle::EnvBend(i) => {
            let mut env = fx::envelope_of(a);
            match env.points.get_mut(usize::from(i)) {
                Some(p) => p.tension = 0.0,
                None => return true,
            }
            fx::with_envelope(a, env)
        }
        Handle::EnvLine(t) => fx::add_point(a, Ticks(i64::from(t)), None).0,
        _ => return false,
    };
    apply(app, r, next);
    app.end_gesture();
    true
}

/// The hint (and the value, live while dragging) for one of this module's handles.
pub fn hint(ctx: &egui::Context, app: &CantusApp, r: ClipRef, handle: Handle, live: bool) -> bool {
    let Some((_, a)) = ac::region(app.project(), r) else {
        return false;
    };
    let env = fx::envelope_of(a);
    let menu = Caps {
        menu: true,
        ..Caps::NONE
    };
    match handle {
        Handle::Xfade => {
            let Some((l, rr)) = fx::pair(app, r) else {
                return true;
            };
            let (from, to) = cantus_core::crossfade::overlap(&l, &rr);
            let secs = crate::views::bars::seconds_between(app.project(), from, to);
            let curve = rr.audio.as_ref().and_then(|a| a.cross);
            let name = t("clipfx.xfade");
            let words = curve.map_or_else(|| t("clipfx.summed"), |c| t(c.label_key()));
            hint::set(ctx, &name, menu, live, |b| {
                write_time(b, secs)?;
                write!(b, " · {words}")
            });
        }
        Handle::EnvPoint(i) => {
            let db = env.points.get(usize::from(i)).map_or(0.0, |p| p.db);
            hint::set(ctx, &t("clipfx.env_point"), menu, live, |b| write_db(b, db));
        }
        Handle::EnvBend(_) => {
            hint::set(ctx, &t("clipfx.env_bend"), menu, live, |_| Ok(()));
        }
        Handle::EnvLine(tk) => {
            let db = env.db_at(f64::from(tk));
            hint::set(ctx, &t("clipfx.env_line"), menu, live, |b| write_db(b, db));
        }
        _ => return false,
    }
    true
}

fn write_time(b: &mut String, s: f64) -> std::fmt::Result {
    if s < 1.0 {
        write!(b, "{:.0} ms", s * 1000.0)
    } else {
        write!(b, "{s:.2} s")
    }
}

fn write_db(b: &mut String, db: f32) -> std::fmt::Result {
    if db <= cantus_core::clip_gain::ENV_MIN_DB {
        b.push_str("−∞ dB");
        Ok(())
    } else {
        write!(b, "{db:+.1} dB")
    }
}

fn action<'a>(label: &'static str, check: Check, enabled: bool) -> Item<'a> {
    Item::Action {
        label: Src::Key(label),
        check,
        shortcut: None,
        submenu: false,
        enabled,
    }
}

/// The menu of one of this module's handles (right-click): the crossfade's curves, or the
/// envelope's segment shapes and points. `false` for another handle.
pub fn menu(app: &mut CantusApp, ui: &mut egui::Ui, r: ClipRef, handle: Handle) -> bool {
    match handle {
        Handle::Xfade => xfade_menu(app, ui, r),
        Handle::EnvPoint(_) | Handle::EnvBend(_) | Handle::EnvLine(_) => {
            env_menu(app, ui, r, handle)
        }
        _ => return false,
    }
    true
}

fn xfade_menu(app: &mut CantusApp, ui: &mut egui::Ui, r: ClipRef) {
    let Some((_, right)) = fx::pair(app, r) else {
        return;
    };
    let now = right.audio.as_ref().and_then(|a| a.cross);
    let mut items = vec![Item::Band(Src::Key("clipfx.xfade"))];
    items.push(action("clipfx.summed", Check::Radio(now.is_none()), true));
    for c in XfadeCurve::ALL {
        items.push(action(c.label_key(), Check::Radio(now == Some(c)), true));
    }
    items.push(Item::Separator);
    items.push(action("clipfx.menu.editor", Check::None, now.is_some()));
    let Some(i) = kit::widgets::menu_list(ui, &items, super::super::super::MENU_W)
        .1
        .clicked
    else {
        return;
    };
    match i {
        1 => {
            app.set_crossfade(r, None);
        }
        2..=4 => {
            app.set_crossfade(r, Some(XfadeCurve::ALL[i - 2]));
        }
        6 => {
            reset(app, r, Handle::Xfade);
        }
        _ => return,
    }
    ui.close();
}

fn env_menu(app: &mut CantusApp, ui: &mut egui::Ui, r: ClipRef, handle: Handle) {
    let Some((_, a)) = ac::region(app.project(), r) else {
        return;
    };
    let a = a.clone();
    let env = fx::envelope_of(&a);
    // The segment the menu is about: from the point, or the one the line lies in.
    let seg = match handle {
        Handle::EnvPoint(i) | Handle::EnvBend(i) => Some(usize::from(i)),
        Handle::EnvLine(tk) => env
            .points
            .partition_point(|p| p.tick.0 <= i64::from(tk))
            .checked_sub(1),
        _ => None,
    }
    .filter(|i| *i + 1 < env.points.len());
    let shape = seg.and_then(|i| env.points.get(i)).map(|p| p.shape);
    let mut items = vec![Item::Band(Src::Key("clipfx.env"))];
    for s in Shape::ALL {
        items.push(action(
            s.label_key(),
            Check::Radio(shape == Some(s)),
            seg.is_some(),
        ));
    }
    items.push(Item::Separator);
    let point = matches!(handle, Handle::EnvPoint(_));
    items.push(action("clipfx.menu.delete_point", Check::None, point));
    items.push(action(
        "clipfx.menu.reset_env",
        Check::None,
        a.gain_env.is_some(),
    ));
    items.push(action("clipfx.menu.show_gain", Check::On, true));
    let Some(i) = kit::widgets::menu_list(ui, &items, super::super::super::MENU_W)
        .1
        .clicked
    else {
        return;
    };
    let n = Shape::ALL.len();
    let next = match i {
        k if (1..=n).contains(&k) => match seg {
            Some(sg) => fx::set_shape(&a, sg, Shape::ALL[k - 1]),
            None => return,
        },
        k if k == n + 2 => match handle {
            Handle::EnvPoint(p) => fx::remove_point(&a, usize::from(p)),
            _ => return,
        },
        k if k == n + 3 => fx::cleared(&a),
        k if k == n + 4 => {
            app.toggle_clip_gain();
            ui.close();
            return;
        }
        _ => return,
    };
    apply(app, r, next);
    app.end_gesture();
    ui.close();
}

/// The crossfade's editor (a double-click on its square): its curve, drawn; *Summed* or
/// one of the three curves; its length, ms. Drawn every frame while open; closed by Esc
/// or a press outside it.
pub fn editor_window(app: &mut CantusApp, ctx: &egui::Context) {
    let Some((r, at)) = app.arrangement.audio.xfade_editor else {
        return;
    };
    let Some((l, right)) = fx::pair(app, r) else {
        app.arrangement.audio.xfade_editor = None;
        return;
    };
    let s = kit::Style::current();
    let k = s.k;
    let now = right.audio.as_ref().and_then(|a| a.cross);
    let (from, to) = cantus_core::crossfade::overlap(&l, &right);
    let map = crate::views::bars::map(app.project());
    let secs = map.seconds_between(from, to);
    let mut ms = (secs * 1000.0) as f32;
    let before = ms;
    // The clips as the knob's turn began (review 1: from the resized ones, a clamped side
    // crept), taken again whenever no button is held.
    let held = ctx.input(|i| i.pointer.any_down());
    let stale = app
        .arrangement
        .audio
        .xfade_base
        .as_ref()
        .is_none_or(|b| b.right.id != right.id);
    if !held || stale {
        app.arrangement.audio.xfade_base = fx::xfade_base(app, r);
    }
    let Some(base) = app.arrangement.audio.xfade_base.clone() else {
        return;
    };
    // As long as the two clips' audio lets it be (review 1: no fixed cap).
    let (lo, hi) = base.span();
    let most = ((map.seconds_between(lo, hi) * 1000.0) as f32)
        .max(ms)
        .max(1.0);
    let mut sel = now.map_or(0, |c| {
        1 + XfadeCurve::ALL.iter().position(|x| *x == c).unwrap_or(0)
    });
    let picked = sel;
    let width = 380.0;
    let area = egui::Area::new(egui::Id::new("cantus-xfade-editor"))
        .order(egui::Order::Foreground)
        .fixed_pos(at)
        .constrain(true)
        .show(ctx, |ui| {
            crate::views::clips_chrome::menu_frame().show(ui, |ui| {
                ui.set_width(width * k);
                crate::views::workspace_widgets::band(ui, Src::Key("clipfx.xfade"));
                let (pic, _) =
                    ui.allocate_exact_size(egui::vec2(width * k, 64.0 * k), egui::Sense::hover());
                kit::with_brush(ui.painter(), |b| picture(b, pic, now, &s));
                let labels = [
                    Src::Key("clipfx.seg.summed"),
                    Src::Key("clipfx.seg.equal_power"),
                    Src::Key("clipfx.seg.linear"),
                    Src::Key("clipfx.seg.s_curve"),
                ];
                kit::widgets::segmented(ui, &mut sel, &labels, &t("clipfx.curve"), width);
                ui.horizontal(|ui| {
                    ui.add_space(8.0 * k);
                    let name = t("clipfx.length");
                    kit::widgets::Param::new(&mut ms, 1.0..=most, 10.0_f32.min(most))
                        .format(|v, b| {
                            let _ = write_time(b, f64::from(v) / 1000.0);
                        })
                        .log(true)
                        .size(kit::knob::KnobSize::M)
                        .cell(Src::Key("clipfx.length"))
                        .name(&name)
                        .show(ui);
                });
            });
        });
    if sel != picked {
        let curve = (sel > 0).then(|| XfadeCurve::ALL[sel - 1]);
        app.set_crossfade(r, curve);
    }
    if (ms - before).abs() > 1e-3 {
        // The length as ticks where the overlap is (through the tempo map).
        let centre = from.0 as f64 + (to.0 - from.0) as f64 * 0.5;
        let half = f64::from(ms) / 2000.0;
        let a = map.tick_after_seconds(centre, -half);
        let b = map.tick_after_seconds(centre, half);
        let len = Ticks(((b - a).round() as i64).max(1));
        app.set_crossfade_length_from(r, &base, len, true);
    }
    let dragging = ctx.input(|i| i.pointer.any_down());
    if !dragging {
        app.end_gesture_if_xfade(r);
    }
    let outside = ctx.input(|i| i.pointer.any_pressed())
        && !ctx
            .pointer_interact_pos()
            .is_some_and(|p| area.response.rect.contains(p));
    if outside || ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
        app.arrangement.audio.xfade_editor = None;
    }
}

/// The editor's picture: the two halves of the curve across the overlap, or the two
/// clips summed.
fn picture(b: &mut kit::Brush, r: egui::Rect, curve: Option<XfadeCurve>, s: &kit::Style) {
    use crate::kit::color::with_alpha;
    let k = s.k;
    let r = r.shrink(6.0 * k);
    b.rrect(r, s.r(3.0), kit::Fill::Solid(with_alpha(s.t.shadow, 0.35)));
    let n = 64;
    let (mut up, mut down) = (Vec::with_capacity(n + 1), Vec::with_capacity(n + 1));
    for i in 0..=n {
        let u = i as f32 / n as f32;
        let (gi, go) = curve.map_or((1.0, 1.0), |c| c.gains(u));
        let x = r.left() + r.width() * u;
        up.push(egui::pos2(x, r.bottom() - gi * r.height()));
        down.push(egui::pos2(x, r.bottom() - go * r.height()));
    }
    b.polyline(&down, 1.6 * k, with_alpha(s.t.menu_text, 0.8));
    b.polyline(&up, 1.6 * k, s.t.selection);
}
