//! The channel sheet (ADR 0024 §7): input → inserts as tiles → the strip EQ as a display
//! that is a control (`inspector_eq`) → sends → routing and delay compensation → output
//! (`inspector_sends`). A subject with no strip (a track that goes straight to the
//! master, or nothing selected) shows every section dimmed, never hidden.

use super::eq::{self, EqCurve};
use super::menus::MenuId;
use super::sheet::{self, Count, Sheet};
use super::{mini, sends, Act, MenuOpen, Subject};
use crate::app::CantusApp;
use crate::kit::hint::{self, Caps};
use crate::kit::text::Src;
use crate::kit::{cache, Fill, Function, State, Tok};
use cantus_core::{Command, EffectId, EffectSlot, StripId, MAX_INSERTS};
use egui::{pos2, Align2, Id, Rect, Sense, Vec2};
use std::fmt::Write as _;

/// An insert chain as the sheet shows it: a strip's, or the master's (ADR 0026).
#[derive(Copy, Clone)]
pub struct Chain<'a> {
    pub id: StripId,
    pub effects: &'a [EffectSlot],
    pub fx_enabled: bool,
}

/// What the channel sheet keeps between frames.
#[derive(Default)]
pub struct Chan {
    /// Inserts whose mini view is open.
    pub open: Vec<EffectId>,
    /// The EQ band being dragged in the display.
    pub eq_drag: Option<usize>,
    pub eq: EqCurve,
    pub mini: mini::Curves,
    /// An insert being dragged to a new place: which, and where it started.
    moving: Option<(EffectId, usize)>,
    /// The Level section's slim meter: its ballistics between frames (r39:lo-mixer).
    pub level_meter: super::sections::LevelMeter,
}

pub fn show(
    sh: &mut Sheet<'_>,
    st: &mut Chan,
    app: &CantusApp,
    subject: Subject,
    strip: Option<StripId>,
) {
    let p = app.project();
    if matches!(
        subject,
        Subject::Track(_) | Subject::Clip(..) | Subject::Notes(..)
    ) {
        // Below a track, a clip or notes: where the channel's own sheet begins.
        sh.band("inspector.mixer", Function::Output, None, false, None);
    }
    if subject == Subject::Master {
        // The master (ADR 0026): the mix in, its own chain, out to the device. It has no
        // EQ and no sends: those sections stand dimmed, in their places.
        master_input(sh);
        let m = &p.master;
        let chain = Chain {
            id: StripId::MASTER,
            effects: &m.effects,
            fx_enabled: m.fx_enabled,
        };
        // r39:el-mixer — des-mixer's order: the inserts open, the rest folded with its state.
        inserts(sh, st, app, chain);
        let out = crate::i18n::t("mixer.look.out_main");
        if sh.fold("kit.inspector.routing", false, &out) {
            sends::routing(sh, app, None, Some(chain));
        }
        super::sections::level(sh, app, None, true, &mut st.level_meter);
        return;
    }
    input(sh, app, subject, strip);
    match strip.and_then(|id| p.strip(id)) {
        Some(ms) => {
            // r39:el-mixer — des-mixer's order: the inserts, the EQ and the sends open;
            // routing, sidechain, recording, automation, level and freeze folded to one row
            // each with its state, a click away.
            let chain = Chain {
                id: ms.id,
                effects: &ms.effects,
                fx_enabled: ms.fx_enabled,
            };
            inserts(sh, st, app, chain);
            eq::show(sh, st, Some(ms));
            sends::sends(sh, app, Some(ms));
            let out = sheet::name(sh.fmt.get(
                cache::key(("insp-fold-out", ms.id)),
                app.bus.revision() ^ ((crate::i18n::locale() as u64) << 60),
                |b| match ms.output {
                    Some(o) => super::strip_label(app, o, b),
                    None => b.push_str(&crate::i18n::t("mixer.master")),
                },
            ));
            // Who feeds it (a return's senders), in sight as the sends are; a strip only its
            // own tracks feed folds it to one row (they are its source, above).
            let others = p
                .strips
                .iter()
                .filter(|x| x.send_to(ms.id).is_some())
                .count()
                + p.strips.iter().filter(|x| x.output == Some(ms.id)).count()
                + ms.effects.iter().filter(|e| e.sidechain.is_some()).count();
            if others == 0 && cantus_core::routing::is_fed(p, ms.id) {
                let none = sheet::name(sh.fmt.tr("inspector.none"));
                if sh.fold("routing.receives.band", false, &none) {
                    super::receives::receives(sh, app, Some(ms));
                }
            } else {
                super::receives::receives(sh, app, Some(ms));
            }
            if sh.fold("kit.inspector.routing", false, &out) {
                sends::routing(sh, app, Some(ms), Some(chain));
            }
            super::sections::sidechain(sh, app, Some(ms));
            super::sections::recording(sh, app, subject, strip);
            super::sections::automation(sh, app, Some(ms));
            // r39:lo-mixer — Level stands where the pinned strip stood, for a track too.
            super::sections::level(sh, app, Some(ms), false, &mut st.level_meter);
            super::sections::freeze(sh, app, subject, strip);
        }
        None => {
            // The track's recording input: device channels, monitor, trim, arm (ADR 0025).
            super::record::show(sh, app, subject, strip);
            // Freeze (round 29, ADR 0064): what this track freezes, and whether it is.
            super::record::freeze_row(sh, app, subject, strip);
            // No strip: every section in its place, dimmed.
            sh.band(
                "kit.inspector.inserts",
                Function::Dynamics,
                None,
                false,
                None,
            );
            for i in 0..MAX_INSERTS {
                let _ = empty_tile(sh, i, true);
            }
            eq::show(sh, st, None);
            sends::sends(sh, app, None);
            super::receives::receives(sh, app, None);
            sends::routing(sh, app, None, None);
            // A track straight to the master: the master's level (the pinned strip's).
            super::sections::level(sh, app, None, true, &mut st.level_meter);
        }
    }
}

/// The master's source (des-mixer's well): the whole mix comes in.
fn master_input(sh: &mut Sheet<'_>) {
    use crate::kit::glyph_mixer::MGlyph;
    let s = sh.s;
    let well = source_rect(sh);
    let text = sheet::name(sh.fmt.tr("master.input"));
    super::look::field(
        sh.b,
        &s,
        well,
        State::REST,
        super::look::Lead::Mixer(MGlyph::Into),
        &text,
        "",
        false,
    );
}

/// Allocate the source's row: a 28-pt well inside the margins, 6 below the head.
fn source_rect(sh: &mut Sheet<'_>) -> Rect {
    let s = sh.s;
    let w = sh.width();
    let (r, _) = sh
        .ui
        .allocate_exact_size(Vec2::new(w, s.s(40.0)), Sense::hover());
    let m = s.s(12.0);
    Rect::from_min_max(
        pos2(r.left() + m, r.top() + s.s(6.0)),
        pos2(r.right() - m, r.top() + s.s(34.0)),
    )
}

/// The source (des-mixer's well under the head): what plays into the strip — its track's
/// own icon (else the instrument's or audio's glyph), the instrument's name in Medium 12,
/// the track's name (or how many more tracks) in Regular 10 ink 3, and a chevron. A click
/// on a strip chosen on the desk shows the track that plays into it; the chevron (and a
/// click on a track's own sheet) opens its instrument.
fn input(sh: &mut Sheet<'_>, app: &CantusApp, subject: Subject, strip: Option<StripId>) {
    use crate::views::mixer::look::{self, W};
    let p = app.project();
    let s = sh.s;
    let well = source_rect(sh);
    let first = strip.and_then(|id| p.tracks_on_strip(id).next());
    let (Some(id), Some(tr)) = (strip, first) else {
        // A bus or a return: the strips routed into it (its Receives list them all).
        let stamp = app.bus.revision() ^ ((crate::i18n::locale() as u64) << 60);
        let into = strip.map_or(0, |id| {
            p.strips.iter().filter(|x| x.output == Some(id)).count()
        });
        let text = sheet::name(sh.fmt.get(cache::key(("insp-src-bus", strip)), stamp, |b| {
            let first = strip.and_then(|id| p.strips.iter().find(|x| x.output == Some(id)));
            match first {
                Some(x) => {
                    super::strip_label(app, x.id, b);
                    if into > 1 {
                        let _ = write!(b, " + {}", into - 1);
                    }
                }
                None => b.push_str(&crate::i18n::t("inspector.none")),
            }
        }));
        let detail = if into > 0 {
            sheet::name(sh.fmt.tr("lo_mixer.strips_kind"))
        } else {
            sheet::name("")
        };
        super::look::field(
            sh.b,
            &s,
            well,
            if into > 0 {
                State::REST
            } else {
                State::DISABLED
            },
            super::look::Lead::Mixer(crate::kit::glyph_mixer::MGlyph::Into),
            &text,
            &detail,
            false,
        );
        return;
    };
    let pal = look::Pal::of(&s);
    let audio = crate::views::arrangement::head::is_audio(tr);
    let chev = Rect::from_min_max(pos2(well.right() - s.s(26.0), well.top()), well.max);
    let body_r = Rect::from_min_max(well.min, pos2(chev.left(), well.bottom()));
    let body = sh
        .ui
        .interact(body_r, Id::new(("insp-input", id)), Sense::click());
    let open = sh
        .ui
        .interact(chev, Id::new(("insp-input-open", id)), Sense::click());
    let st = State::of(&body, false);
    let st = State {
        hovered: body.hovered() || open.hovered(),
        ..st
    };
    super::look::field(sh.b, &s, well, st, super::look::Lead::None, "", "", false);
    let cy = well.center().y;
    // Its own icon in its colour, else the kind's glyph.
    let icon = Rect::from_center_size(pos2(well.left() + s.s(17.0), cy), Vec2::splat(s.s(16.0)));
    let hue = crate::theme::track_color(tr.color);
    let own = tr
        .glyph
        .as_deref()
        .and_then(crate::views::track_glyphs::by_id);
    let drawn = own.is_some_and(|g| {
        let inks = crate::views::track_glyphs::inks(
            &s,
            crate::kit::color::mix(hue, pal.ink2, 0.2),
            pal.well,
        );
        crate::views::track_glyphs::paint_with(sh.b, icon, g, &inks)
    });
    if !drawn {
        let g = if audio {
            crate::kit::glyph_browser::Glyph::Audio
        } else {
            crate::kit::glyph_browser::Glyph::Instrument
        };
        crate::kit::glyph_browser::paint(sh.b, icon, g, pal.ink2);
    }
    let x = icon.right() + s.s(8.0);
    let base = cy + 4.0;
    let (name, detail) = {
        let stamp = app.bus.revision() ^ ((crate::i18n::locale() as u64) << 60);
        let name = sheet::name(sh.fmt.get(cache::key(("insp-src", id)), stamp, |b| {
            if audio {
                b.push_str(&crate::i18n::t("lo_mixer.audio"));
            } else {
                match &tr.instrument {
                    cantus_core::InstrumentSpec::External(x) => b.push_str(&x.name),
                    spec => b.push_str(&crate::swap::instrument_name(spec)),
                }
            }
        }));
        let detail = sheet::name(sh.fmt.get(cache::key(("insp-src-d", id)), stamp, |b| {
            let more = p.tracks_on_strip(id).count().saturating_sub(1);
            b.push_str(&tr.name);
            if more > 0 {
                let t = crate::i18n::t("lo_mixer.tracks_more");
                b.push(' ');
                b.push_str(&t.replace("{n}", &more.to_string()));
            }
        }));
        (name, detail)
    };
    let right = chev.left();
    let nw = look::say(
        sh.b,
        &s,
        &name,
        x,
        base,
        Align2::LEFT_BOTTOM,
        (right - x).max(8.0),
        12.0,
        W::Medium,
        pal.ink1,
    );
    let dx = x + nw + s.s(6.0);
    if dx < right - s.s(12.0) {
        look::say(
            sh.b,
            &s,
            &detail,
            dx,
            base,
            Align2::LEFT_BOTTOM,
            right - dx,
            10.0,
            W::Regular,
            pal.ink3,
        );
    }
    // The chevron: a right-pointing one, ink 3 (ink 1 under the hand).
    let c = chev.center();
    let ink = if open.hovered() { pal.ink1 } else { pal.ink3 };
    sh.b.polyline(
        &[
            pos2(c.x - 2.0, c.y - 4.0),
            pos2(c.x + 2.0, c.y),
            pos2(c.x - 2.0, c.y + 4.0),
        ],
        1.2,
        ink,
    );
    let strip_sheet = matches!(subject, Subject::Strip(_));
    if body.hovered() {
        let tip = sheet::name(sh.fmt.tr("lo_mixer.source_tip"));
        hint::set_text(sh.ui.ctx(), &name, &tip, Caps::NONE);
    } else if open.hovered() {
        let tip = sheet::name(sh.fmt.tr("lo_mixer.source_open"));
        hint::set_text(sh.ui.ctx(), &tip, "", Caps::NONE);
    }
    if body.clicked() && strip_sheet {
        sh.acts.push(Act::SelectTrack(tr.id));
    } else if (body.clicked() && !audio) || (open.clicked() && !audio) {
        sh.acts.push(Act::OpenInstrument(tr.id));
    } else if open.clicked() {
        sh.acts.push(Act::SelectTrack(tr.id));
    }
}

/// The inserts: every slot drawn, empty ones as italic placeholders, so nothing jumps
/// when one is filled (FL). ▸ opens the slot's mini view; the name opens its editor; the
/// ring is its dry/wet; the LED its bypass; a drag moves it; right-click has the rest.
fn inserts(sh: &mut Sheet<'_>, st: &mut Chan, app: &CantusApp, ms: Chain<'_>) {
    let s = sh.s;
    let id = ms.id;
    let n = ms.effects.len();
    let count = Count {
        key: cache::key(("insp-fxcount", id)),
        stamp: n as u64,
        write: &|b: &mut String| {
            let _ = write!(b, "{n} / {MAX_INSERTS}");
        },
    };
    let fx = ms.fx_enabled;
    let band = sh.band(
        "kit.inspector.inserts",
        Function::Dynamics,
        Some(fx),
        false,
        Some(count),
    );
    if band.power {
        sh.acts.push(Act::Exec(if id.is_master() {
            Command::SetMasterFxEnabled { enabled: !fx }
        } else {
            Command::SetStripFxEnabled { id, enabled: !fx }
        }));
    }
    // The inserts' switch is the strip's effects switch: learnable here too (round 29,
    // review 4 of ADR 0062, #21).
    if let Some(pr) = band.power_rect {
        let target = cantus_core::controller::MappingTarget::StripFx(id);
        let clip = sh.ui.clip_rect();
        if crate::controllers::learn_spot_view(app, sh.ui, pr, clip, &target) {
            sh.acts.push(Act::Learn(target));
        }
    }
    let mut rects = [Rect::NOTHING; MAX_INSERTS];
    for (i, slot) in ms.effects.iter().enumerate() {
        rects[i] = insert_tile(sh, st, app, ms, i, slot);
        if st.open.contains(&slot.id) {
            mini::show(sh, &mut st.mini, app, ms.id, slot);
        }
    }
    // Round 20: a click on an empty slot opens the device picker on the effects; a
    // right-click, the quick list.
    // r39:el-mixer — one "Add an effect" row, never ten empty slots (des-mixer).
    for i in n..(n + 1).min(MAX_INSERTS) {
        match empty_tile(sh, i, false) {
            Some((anchor, true)) => {
                *sh.menu = Some(MenuOpen {
                    id: MenuId::AddInsert(id),
                    anchor,
                });
            }
            Some((_, false)) => {
                *sh.menu = None;
                sh.acts
                    .push(Act::Pick(crate::picker::PickPurpose::AddEffect {
                        strip: id,
                    }));
            }
            None => {}
        }
    }
    // A tile being dragged: a line in the selection colour where it would land.
    let Some((effect, from)) = st.moving else {
        return;
    };
    let Some(p) = sh.ui.ctx().pointer_interact_pos() else {
        return;
    };
    let to = (0..n).find(|&i| p.y < rects[i].center().y).unwrap_or(n);
    let target = if to > from { to - 1 } else { to };
    if target != from && n > 0 {
        let y = if to < n {
            rects[to].top() - s.s(1.0)
        } else {
            rects[n - 1].bottom() + s.s(1.0)
        };
        // des-mixer: a 2-pt line in the accent where it lands.
        let (x0, x1) = (rects[0].left(), rects[0].right());
        let accent = crate::views::mixer::look::Pal::of(&s).accent;
        sh.b.rrect(
            Rect::from_min_max(pos2(x0, y - s.s(1.0)), pos2(x1, y + s.s(1.0))),
            1.0,
            Fill::Solid(accent),
        );
    }
    if !sh.ui.input(|i| i.pointer.any_down()) {
        st.moving = None;
        if target != from {
            sh.acts.push(Act::Exec(Command::MoveEffect {
                strip: id,
                effect,
                index: target,
            }));
        }
    }
}

/// Allocate an insert row (des-mixer: 26 high, 2 apart) inside the margins.
fn tile_rect(sh: &mut Sheet<'_>) -> Rect {
    let s = sh.s;
    let w = sh.width();
    let (r, _) = sh
        .ui
        .allocate_exact_size(Vec2::new(w, s.s(28.0)), Sense::hover());
    let m = s.s(sheet::MARGIN);
    Rect::from_min_max(
        pos2(r.left() + m, r.top()),
        pos2(r.right() - m, r.bottom() - s.s(2.0)),
    )
}

/// One filled slot (des-mixer's insert row): `surface`, r 4; its number (a grip under the
/// pointer), the unit's name Regular 12 and its kind Regular 10 ink 3, a lamp at the right
/// (`play` with its halo; bypassed, a ring, the name in ink 4 and "· Bypassed"); under the
/// pointer the mini view's key. A click opens its editor, the lamp bypasses it, a drag
/// moves it, right-click has the rest. Returns its rect (for the drop line of a drag).
fn insert_tile(
    sh: &mut Sheet<'_>,
    st: &mut Chan,
    app: &CantusApp,
    ms: Chain<'_>,
    i: usize,
    slot: &EffectSlot,
) -> Rect {
    use crate::kit::glyph_mixer::{Ink, MGlyph};
    use crate::views::mixer::look::{self, W};
    let s = sh.s;
    let p = look::Pal::of(&s);
    let rect = tile_rect(sh);
    let e = slot.id;
    let cy = rect.center().y;
    let led_c = pos2(rect.right() - 10.0, cy);
    let mini_r = Rect::from_center_size(pos2(rect.right() - 30.0, cy), Vec2::splat(16.0));
    // The body first: the smaller parts on top of it take the pointer where they are.
    let body = sh
        .ui
        .interact(rect, Id::new(("insp-tile", e)), Sense::click_and_drag());
    let menu = sh
        .ui
        .interact(mini_r, Id::new(("insp-tile-menu", e)), Sense::click());
    let lamp = sh.ui.interact(
        Rect::from_center_size(led_c, Vec2::splat(14.0)),
        Id::new(("insp-tile-led", e)),
        Sense::click(),
    );
    let clip = sh.ui.clip_rect();
    let target = crate::controllers::slot_learn_target(e, 0);
    if crate::controllers::learn_spot_view(app, sh.ui, lamp.rect, clip, &target) {
        sh.acts.push(Act::Learn(target));
    }
    if menu.clicked() {
        match st.open.iter().position(|x| *x == e) {
            Some(k) => {
                st.open.remove(k);
            }
            None => st.open.push(e),
        }
    }
    if lamp.clicked() {
        sh.acts.push(Act::Exec(Command::SetEffectBypass {
            strip: ms.id,
            effect: e,
            bypass: !slot.bypass,
        }));
    }
    if body.clicked() {
        if slot.spec.kind.has_editor() {
            sh.acts.push(Act::OpenEditor(ms.id, e));
        } else if !st.open.contains(&e) {
            st.open.push(e);
        }
    }
    if body.drag_started() {
        st.moving = Some((e, i));
    }
    if body.secondary_clicked() {
        *sh.menu = Some(MenuOpen {
            id: MenuId::Slot(ms.id, e),
            anchor: rect,
        });
    }
    let hosted = slot.spec.external.as_deref();
    let missing = hosted.is_some() && app.plugins.missing(e).is_some();
    // Stopped working (round 29, ADR 0063): drawn like a missing one, in the danger colour.
    let stopped = hosted.is_some() && app.plugins.crashed(e).is_some();
    let open = st.open.contains(&e);
    let moving = st.moving.is_some_and(|(m, _)| m == e);
    let hot = body.hovered() || menu.hovered() || lamp.hovered();
    let on = !slot.bypass && ms.fx_enabled;
    let ground = if moving {
        p.raised
    } else if hot {
        if p.dark {
            crate::kit::color::mix(p.surface, p.ink1, 0.04)
        } else {
            p.raised
        }
    } else {
        p.surface
    };
    // r39:lo-mixer — des-mixer's dragged row: its place left as a quiet socket, the row
    // lifted to `raised`, 4 to the right and 2 down, over a soft shadow (σ 3).
    let slot_rect = rect;
    let rect = if moving {
        sh.b.rrect(
            sh.b.snap_rect(rect),
            4.0,
            Fill::Solid(crate::kit::color::mix(p.base, p.well, 0.5)),
        );
        let lifted = rect.translate(Vec2::new(4.0, 2.0));
        // The theme's drop shadow (the kit's floating surfaces cast the same).
        let shadow = s.t.shadow;
        sh.b.rrect_soft(
            lifted.translate(Vec2::new(0.0, 2.0)),
            6.0,
            Fill::Solid(shadow),
            6.0,
        );
        lifted
    } else {
        rect
    };
    let cy = rect.center().y;
    let led_c = pos2(rect.right() - 10.0, cy);
    let mini_r = Rect::from_center_size(pos2(rect.right() - 30.0, cy), Vec2::splat(16.0));
    sh.b.rrect(sh.b.snap_rect(rect), 4.0, Fill::Solid(ground));
    let base = cy + 4.0;
    if hot || moving {
        look::glyph(
            sh.b,
            &s,
            pos2(rect.left() + 10.0, cy),
            12.0,
            MGlyph::Grip,
            Ink::Quiet,
            ground,
        );
    } else {
        let n = sh.fmt.get(cache::key(("insp-slot-n", i)), i as u64, |b| {
            let _ = write!(b, "{}", i + 1);
        });
        look::say(
            sh.b,
            &s,
            n,
            rect.left() + 8.0,
            base,
            Align2::LEFT_BOTTOM,
            14.0,
            10.0,
            W::Regular,
            p.ink3,
        );
    }
    // The lamp, and the mini view's key under the pointer (or while it is open).
    if on {
        sh.b.circle(
            led_c,
            4.5,
            Fill::Solid(crate::kit::color::with_alpha(p.play, 0.18)),
        );
        sh.b.circle(led_c, 2.5, Fill::Solid(p.play));
    } else {
        sh.b.circle_stroke(led_c, 2.5, sh.b.px(), p.ink4);
    }
    let mut right = led_c.x - 10.0;
    if hot || open {
        if open || menu.hovered() {
            look::well(sh.b, mini_r, &p, menu.hovered());
        }
        let ink = if open {
            Ink::On(p.accent)
        } else if menu.hovered() {
            Ink::Hover
        } else {
            Ink::Rest
        };
        look::glyph(
            sh.b,
            &s,
            mini_r.center(),
            12.0,
            MGlyph::MiniView,
            ink,
            ground,
        );
        right = mini_r.left() - 4.0;
    }
    // A compressor: its live reduction (des-mixer: the value in the solo colour over a
    // short bar from the right), 0.0 in ink 3 at rest.
    if slot.spec.kind == cantus_core::EffectKind::Compressor && on && !open {
        let gr = app
            .comp_gr_on_strip(ms.id)
            .flatten()
            .unwrap_or(0.0)
            .max(0.0);
        let live = gr > 0.05;
        let text = sh.fmt.get(
            cache::key(("insp-gr", e)),
            sheet::bits((gr * 10.0).round()),
            |b| {
                if live {
                    let _ = write!(b, "\u{2212}{gr:.1} dB");
                } else {
                    b.push_str("0.0 dB");
                }
            },
        );
        let ink = if live { p.solo } else { p.ink3 };
        let w = look::say(
            sh.b,
            &s,
            text,
            right,
            base - 2.0,
            Align2::RIGHT_BOTTOM,
            56.0,
            10.0,
            W::Regular,
            ink,
        );
        let bar = Rect::from_min_max(
            pos2(right - 22.0, rect.bottom() - 6.0),
            pos2(right, rect.bottom() - 4.0),
        );
        sh.b.rect(sh.b.snap_rect(bar), Fill::Solid(p.bar_track));
        if live {
            let fw = (gr / 12.0).min(1.0) * bar.width();
            let lit = Rect::from_min_max(pos2(bar.right() - fw, bar.top()), bar.max);
            sh.b.rect(sh.b.snap_rect(lit), Fill::Solid(p.solo));
        }
        right -= w.max(22.0) + 8.0;
    }
    // A keyed insert: the key glyph and its source's name.
    if let Some(src) = slot.sidechain {
        let name = sh
            .fmt
            .get(cache::key(("insp-key", e)), app.bus.revision(), |b| {
                super::strip_label(app, src, b)
            });
        let w = look::say(
            sh.b,
            &s,
            name,
            right,
            base,
            Align2::RIGHT_BOTTOM,
            48.0,
            10.0,
            W::Regular,
            p.ink2,
        );
        look::glyph(
            sh.b,
            &s,
            pos2(right - w - 9.0, cy),
            12.0,
            MGlyph::Key,
            Ink::Solid(p.ink2),
            ground,
        );
        right -= w + 18.0;
    }
    // The chips a hosted plug-in carries (its format; missing).
    if let Some(hp) = hosted {
        if missing || stopped {
            let w = sheet::chip(
                sh.b,
                pos2(right, cy),
                &s,
                Src::Key(if missing {
                    "host.missing"
                } else {
                    "isolation.stopped"
                }),
                Tok::Danger,
                1.0,
            );
            right -= w + s.s(3.0);
        }
        let w = sheet::chip(
            sh.b,
            pos2(right, cy),
            &s,
            Src::Str(hp.format.badge()),
            Tok::TextDim,
            1.0,
        );
        right -= w + s.s(4.0);
    }
    let name_ink = if missing || stopped {
        s.t.danger
    } else if on {
        p.ink1
    } else {
        p.ink4
    };
    let x = rect.left() + 22.0;
    let unit = match insert_name(slot) {
        Src::Str(n) => n,
        Src::Key(k) => sh.fmt.tr(k),
    };
    // The unit's name without its kind ("Comprimo compressor" → "Comprimo").
    let unit = unit.split(' ').next().unwrap_or(unit);
    let nw = look::say(
        sh.b,
        &s,
        unit,
        x,
        base,
        Align2::LEFT_BOTTOM,
        (right - x).max(10.0),
        12.0,
        W::Regular,
        name_ink,
    );
    let kx = x + nw + 6.0;
    if kx < right - 12.0 {
        let stamp = (crate::i18n::locale() as u64) ^ (u64::from(slot.bypass) << 8);
        let k = sh.fmt.get(cache::key(("insp-kind", e)), stamp, |b| {
            b.push_str(&crate::views::mixer::desk_short_name(slot));
            if slot.bypass {
                b.push_str(" \u{b7} ");
                b.push_str(&crate::i18n::t("inspector.bypassed"));
            }
        });
        look::say(
            sh.b,
            &s,
            k,
            kx,
            base,
            Align2::LEFT_BOTTOM,
            right - kx,
            10.0,
            W::Regular,
            if on { p.ink3 } else { p.ink4 },
        );
    }
    if hot {
        let key = if lamp.hovered() {
            "effect.bypass"
        } else if menu.hovered() {
            "inspector.mini_tip"
        } else if slot.spec.kind.has_editor() {
            "inspector.slot_tip"
        } else {
            "inspector.slot_tip_plain"
        };
        let name = sh.fmt.tr(key);
        // A hosted plug-in's tip says where it runs and what it costs (round 29).
        let body = if hosted.is_some() && body.hovered() {
            crate::views::plugin_notice::tip(app, crate::views::plugin_notice::Target::Insert(e))
        } else {
            String::new()
        };
        hint::set_text(
            sh.ui.ctx(),
            name,
            &body,
            Caps {
                menu: true,
                ..Caps::NONE
            },
        );
    }
    slot_rect
}

/// One insert's name as its tile shows it.
fn insert_name(slot: &EffectSlot) -> Src<'_> {
    match slot.spec.external.as_deref() {
        Some(p) => Src::Str(&p.name),
        None => Src::Key(slot.spec.kind.name_key()),
    }
}

/// The row after the inserts (des-mixer: no empty slot rows): `+` and "Add an effect" in
/// ink 3. A click returns its rect (the add menu hangs from it) and whether it was the
/// right button.
pub fn empty_tile(sh: &mut Sheet<'_>, i: usize, dim: bool) -> Option<(Rect, bool)> {
    use crate::kit::glyph_mixer::{Ink, MGlyph};
    use crate::views::mixer::look::{self, W};
    let s = sh.s;
    let p = look::Pal::of(&s);
    let rect = tile_rect(sh);
    let r = sh
        .ui
        .interact(rect, Id::new(("insp-empty-slot", i)), Sense::click());
    crate::views::device_chrome::publish(
        sh.ui.ctx(),
        crate::views::device_chrome::Spot::InspectorEmptySlot(i),
        rect,
    );
    let hot = r.hovered() && !dim;
    if hot {
        sh.b.rrect(
            sh.b.snap_rect(rect),
            4.0,
            Fill::Solid(crate::kit::color::with_alpha(p.ink1, 0.04)),
        );
    }
    let ink = if dim {
        p.ink4
    } else if hot {
        p.ink1
    } else {
        p.ink3
    };
    look::glyph(
        sh.b,
        &s,
        pos2(rect.left() + 6.0, rect.center().y),
        10.0,
        MGlyph::Add,
        Ink::Solid(ink),
        p.base,
    );
    let label = sh.fmt.tr("inspector.add_effect");
    look::say(
        sh.b,
        &s,
        label,
        rect.left() + 14.0,
        rect.center().y + 4.0,
        Align2::LEFT_BOTTOM,
        rect.width() - 16.0,
        11.0,
        W::Regular,
        ink,
    );
    // r39:ux-a — a dimmed slot says why (ADR 0109).
    if r.hovered() && dim {
        let name = sh.fmt.tr("ux.hint.no_strip");
        hint::set_text(sh.ui.ctx(), name, "", Caps::NONE);
    }
    if hot {
        let name = sh.fmt.tr("inspector.slot_add");
        hint::set_text(
            sh.ui.ctx(),
            name,
            "",
            Caps {
                menu: true,
                ..Caps::NONE
            },
        );
    }
    (!dim && (r.clicked() || r.secondary_clicked())).then_some((rect, r.secondary_clicked()))
}
