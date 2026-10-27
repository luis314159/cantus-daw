//! The inspector's round-31 sections (ADR 0069): a track's **Note effects** — its chain of
//! arpeggiator, chords and scale before the instrument, in the order they play — and a
//! note clip's **Groove** (the template, its amounts, *Commit*, *Extract groove*) with
//! *Bake to notes* and whether the clip plays past the note effects.
//!
//! Each effect is a title row — its name in the notes' hue, earlier and later, on/off,
//! remove — and, unfolded, one label | value row per parameter. Every parameter row is an
//! automation target (`AutoTarget::NoteFx`): its menu creates an automation clip or a
//! modulation, a modulator's handle connects to it, as any knob's. Rows act after painting
//! (`Act::NoteFx`), one command each, a drag merged into one step.

use super::modulators::param_row;
use super::sheet::{self, Range, Sheet};
use super::Act;
use crate::app::CantusApp;
use crate::kit::glyph::Glyph;
use crate::kit::hint::{self, Caps};
use crate::kit::text::{self, Elide, Policy, Role, Text};
use crate::kit::{cache, cap, led, well, Function, State, Tok};
use cantus_core::groove::ClipGroove;
use cantus_core::notefx::{NoteEffect, NoteFxId, NoteFxParam, NoteFxUnit, MAX_NOTE_FX};
use cantus_core::{AutoTarget, ClipId, MergeKey, TrackId};
use egui::{pos2, Align2, Id, Rect, Sense, Vec2};

/// The notes' hue: note effects make notes, as sources make sound.
const HUE: Function = Function::Source;

/// What a row of the sections asks for.
#[derive(Clone, Debug)]
pub enum NfAct {
    AddMenu(TrackId, Rect),
    Remove(TrackId, NoteFxId),
    Toggle(TrackId, NoteFxId),
    Move(TrackId, NoteFxId, i32),
    Fold(NoteFxId),
    Param(TrackId, NoteFxId, &'static str, f32),
    /// One parameter set by a click (a switch), never merged.
    Set(TrackId, NoteFxId, &'static str, f32),
    /// A choice parameter's list, under the rect.
    ChoiceMenu(TrackId, NoteFxId, &'static str, Rect),
    GrooveMenu(TrackId, ClipId, Rect),
    Amounts(TrackId, ClipId, ClipGroove),
    Commit(TrackId, ClipId),
    Extract(TrackId, ClipId),
    Bake(TrackId, ClipId),
    Skip(TrackId, ClipId, bool),
}

/// Run what a row asked for.
pub fn apply(app: &mut CantusApp, act: NfAct) {
    match act {
        NfAct::AddMenu(t, r) => app.notefx.add_menu = Some((t, r)),
        NfAct::Remove(t, id) => {
            app.remove_note_fx(t, id);
        }
        NfAct::Toggle(t, id) => {
            app.toggle_note_fx(t, id);
        }
        NfAct::Move(t, id, by) => {
            app.move_note_fx(t, id, by);
        }
        NfAct::Fold(id) => {
            if !app.notefx.folded.remove(&id) {
                app.notefx.folded.insert(id);
            }
        }
        NfAct::Param(t, id, key, v) => {
            let merge = MergeKey::of(&("notefx-param", t.0, id.0, key));
            app.set_note_fx_param(t, id, key, v, Some(merge));
        }
        NfAct::ChoiceMenu(t, id, key, r) => app.notefx.choice_menu = Some((t, id, key, r)),
        NfAct::Set(t, id, key, v) => {
            app.set_note_fx_param(t, id, key, v, None);
        }
        NfAct::GrooveMenu(t, c, r) => {
            app.notefx.groove_pattern = false;
            app.notefx.groove_menu = Some((t, c, r));
        }
        NfAct::Amounts(t, c, g) => {
            let merge = MergeKey::of(&("notefx-groove", t.0, c.0));
            app.set_clip_groove(t, c, Some(g), Some(merge));
        }
        NfAct::Commit(t, c) => {
            app.commit_groove(t, c);
        }
        NfAct::Extract(t, c) => {
            if app.extract_groove(t, c).is_none() {
                let busy = app.warp_analysing((t, c));
                app.set_error(crate::i18n::t(if busy {
                    "notefx.groove.analysing"
                } else {
                    "notefx.groove.nothing"
                }));
            }
        }
        NfAct::Bake(t, c) => {
            app.bake_note_fx(t, c);
        }
        NfAct::Skip(t, c, on) => {
            app.exec(cantus_core::Command::SetClipSkipNoteFx {
                track: t,
                clip: c,
                skip: on,
            });
        }
    }
}

/// A track's Note effects section: its band (with +), then each effect.
pub fn chain(sh: &mut Sheet<'_>, app: &CantusApp, track: TrackId) {
    let Some(t) = app.project().track(track) else {
        return;
    };
    let list = &t.note_fx;
    let n = list.len();
    let count = sheet::Count {
        key: cache::key(("notefx-count", track)),
        stamp: n as u64,
        write: &|b: &mut String| {
            if n > 0 {
                b.push_str(&n.to_string());
            }
        },
    };
    let band = sh.band("notefx.band", HUE, None, n < MAX_NOTE_FX, Some(count));
    if let (true, Some(r)) = (band.plus, band.plus_rect) {
        sh.acts.push(Act::NoteFx(NfAct::AddMenu(track, r)));
    }
    if list.is_empty() {
        empty_row(sh, track);
        return;
    }
    for (i, e) in list.iter().enumerate() {
        title(sh, app, track, e, i, n);
        if app.notefx.folded.contains(&e.id) {
            continue;
        }
        for p in e.kind.params() {
            // A chord's or a scale's own key shows only when it has one.
            let keyed = matches!(p.key, "root" | "scale");
            if keyed && e.kind.get("own_key") != Some(1.0) {
                continue;
            }
            param(sh, app, track, e, p);
        }
    }
}

/// No effect yet: the empty row says what goes there, and opens the kinds.
fn empty_row(sh: &mut Sheet<'_>, track: TrackId) {
    let s = sh.s.function(HUE);
    let row = sh.row();
    let m = s.s(sheet::MARGIN);
    let r = Rect::from_min_max(
        pos2(row.full.left() + m, row.value.top()),
        pos2(row.full.right() - m, row.value.bottom()),
    );
    let resp = sh
        .ui
        .interact(r, Id::new(("notefx-empty", track)), Sense::click());
    sheet::placeholder_well(sh.b, r, &s, State::of(&resp, false), "notefx.empty");
    if resp.clicked() {
        sh.acts.push(Act::NoteFx(NfAct::AddMenu(track, r)));
    }
}

/// An effect's title row: fold, name, earlier, later, on/off, remove.
fn title(sh: &mut Sheet<'_>, app: &CantusApp, track: TrackId, e: &NoteEffect, i: usize, n: usize) {
    let s = sh.s.function(HUE);
    sh.ui.add_space(s.d(2.0));
    let row = sh.row();
    let mg = s.s(sheet::MARGIN);
    let (x0, x1) = (row.full.left() + mg, row.full.right() - mg);
    let (y0, y1) = (row.value.top(), row.value.bottom());
    let key = s.s(20.0);
    let gap = s.s(4.0);
    let cell = |k: f32| {
        Rect::from_min_max(
            pos2(x1 - k * (key + gap) - key, y0),
            pos2(x1 - k * (key + gap), y1),
        )
    };
    let (remove, power, later, earlier) = (cell(0.0), cell(1.0), cell(2.0), cell(3.0));
    let name_r = Rect::from_min_max(pos2(x0, y0), pos2(earlier.left() - gap, y1));
    let nr = sh
        .ui
        .interact(name_r, Id::new(("notefx-name", e.id)), Sense::click());
    let name = sh.fmt.get(
        cache::key(("notefx-name", e.id)),
        app.bus.revision() ^ ((crate::i18n::locale() as u64) << 60),
        |b| {
            if let Some(t) = app.project().track(track) {
                b.push_str(&crate::notefx_ui::effect_name(&t.note_fx, e.id));
            }
        },
    );
    let name = sheet::name(name);
    let folded = app.notefx.folded.contains(&e.id);
    let chevron = Rect::from_min_size(
        pos2(name_r.left(), name_r.center().y - s.s(5.0)),
        Vec2::splat(s.s(10.0)),
    );
    crate::kit::glyph::paint(
        sh.b,
        chevron,
        if folded { Glyph::Menu } else { Glyph::Down },
        s.t.text_dim,
    );
    let tr = Rect::from_min_max(pos2(chevron.right() + s.s(4.0), y0), name_r.right_bottom());
    let ink = if e.enabled { s.hue } else { s.t.text_dim };
    let t = Text::new(&name, Role::Body, ink)
        .align(Align2::LEFT_CENTER)
        .policy(Policy::Elide(Elide::End));
    let _ = text::paint(sh.b, tr, &t, s.tk());
    if nr.hovered() {
        let tip = sheet::name(sh.fmt.tr(if folded {
            "notefx.expand"
        } else {
            "notefx.collapse"
        }));
        hint::set_text(sh.ui.ctx(), &name, &tip, Caps::NONE);
    }
    if nr.clicked() {
        sh.acts.push(Act::NoteFx(NfAct::Fold(e.id)));
    }
    let keys = [
        (
            earlier,
            Some(Glyph::Prev),
            "notefx.earlier",
            i > 0,
            NfAct::Move(track, e.id, -1),
        ),
        (
            later,
            Some(Glyph::Next),
            "notefx.later",
            i + 1 < n,
            NfAct::Move(track, e.id, 1),
        ),
        (
            power,
            None,
            "notefx.on_off",
            true,
            NfAct::Toggle(track, e.id),
        ),
        (
            remove,
            Some(Glyph::Close),
            "notefx.remove",
            true,
            NfAct::Remove(track, e.id),
        ),
    ];
    for (k, (r, g, tip, enabled, act)) in keys.into_iter().enumerate() {
        let resp = sh
            .ui
            .interact(r, Id::new(("notefx-key", e.id, k)), Sense::click());
        match g {
            None => led::paint(
                sh.b,
                r.center(),
                s.s(11.0),
                e.enabled,
                &s,
                Tok::Led,
                Tok::PanelLight,
                1.0,
            ),
            Some(g) => {
                let st = State {
                    disabled: !enabled,
                    ..State::of(&resp, false)
                };
                sheet::glyph_key(sh.b, r, &s, st, g);
            }
        }
        if resp.hovered() {
            let name = sheet::name(sh.fmt.tr(tip));
            hint::set_text(sh.ui.ctx(), &name, "", Caps::NONE);
        }
        if resp.clicked() && enabled {
            sh.acts.push(Act::NoteFx(act));
        }
    }
}

/// One parameter's row, automatable and modulatable.
fn param(
    sh: &mut Sheet<'_>,
    app: &CantusApp,
    track: TrackId,
    e: &NoteEffect,
    p: &'static NoteFxParam,
) {
    let v = e.kind.get(p.key).unwrap_or(p.default);
    let range = Range {
        stepped: p.stepped,
        ..Range::lin(p.min, p.max, p.default)
    };
    let kind = e.kind.kind();
    // Words, not numbers: a choice opens its list, a switch flips (ADR 0024 §3).
    if matches!(p.unit, NoteFxUnit::Choice | NoteFxUnit::Switch) {
        return word_row(sh, app, track, e, p, v, &range);
    }
    let target = AutoTarget::NoteFx {
        track,
        effect: e.id,
        key: p.key.to_string(),
    };
    let (id, key) = (e.id, p.key);
    param_row(
        sh,
        app,
        p.label_key,
        Id::new(("notefx-param", e.id, p.key)),
        v,
        &range,
        false,
        move |v, b| crate::notefx_ui::write_value(kind, key, v, b),
        HUE,
        target,
        !e.enabled,
        move |v| Act::NoteFx(NfAct::Param(track, id, key, v)),
    );
}

/// A note clip's Groove section: the template (a list), its timing, accent and length
/// amounts, *Commit* and *Extract groove*; then *Bake to notes* and *Past the note
/// effects*. An audio clip offers *Extract groove* from its transients alone.
pub fn clip(sh: &mut Sheet<'_>, app: &CantusApp, track: TrackId, clip: ClipId) {
    let p = app.project();
    let Some(c) = p.track(track).and_then(|t| t.clip(clip)) else {
        return;
    };
    if c.automation.is_some() {
        return;
    }
    sh.band("notefx.groove.band", HUE, None, false, None);
    if c.audio.is_some() {
        let busy = app.warp_analysing((track, clip));
        if button(
            sh,
            ("notefx-extract", clip),
            "notefx.groove.extract_audio",
            false,
            busy,
        ) {
            sh.acts.push(Act::NoteFx(NfAct::Extract(track, clip)));
        }
        return;
    }
    // The template: a list of none, the swings, the pool.
    let row = sh.row();
    sh.label(row.label, "notefx.groove", false);
    let value = sh.fmt.get(
        cache::key(("notefx-groove", clip)),
        app.bus.revision() ^ ((crate::i18n::locale() as u64) << 60),
        |b| match &c.groove {
            Some(g) => b.push_str(&crate::notefx_ui::groove_name(p, &g.source)),
            None => b.push_str(&crate::i18n::t("notefx.groove.none")),
        },
    );
    let value = sheet::name(value);
    let r = sh.ui.interact(
        row.value,
        Id::new(("notefx-groove-field", clip)),
        Sense::click(),
    );
    let open = app.notefx.groove_menu.is_some_and(|(_, c2, _)| c2 == clip);
    if r.hovered() {
        let name = sheet::name(sh.fmt.tr("notefx.groove"));
        hint::set_text(
            sh.ui.ctx(),
            &name,
            &value,
            Caps {
                menu: true,
                ..Caps::NONE
            },
        );
    }
    well::menu_field(sh.b, row.value, &sh.s, State::of(&r, open), &value, false);
    if r.clicked() {
        sh.acts
            .push(Act::NoteFx(NfAct::GrooveMenu(track, clip, row.value)));
    }
    // A pooled groove: its name, to rename it (review: the pool could not be tidied).
    if let Some(cantus_core::GrooveSource::Pool { id }) = c.groove.map(|g| g.source) {
        pool_name_row(sh, app, id);
    }
    if let Some(g) = c.groove {
        let amounts: [Amount; 3] = [
            ("notefx.groove.timing", g.timing, 1.5, |g, v| g.timing = v),
            ("notefx.groove.velocity", g.velocity, 1.0, |g, v| {
                g.velocity = v
            }),
            ("notefx.groove.length", g.length, 1.0, |g, v| g.length = v),
        ];
        for (key, v, max, set) in amounts {
            let row = sh.row();
            sh.label(row.label, key, false);
            let name = sheet::name(sh.fmt.tr(key));
            let d = sh.value(
                row.value,
                Id::new(("notefx-amount", clip, key)),
                v,
                &Range::lin(
                    0.0,
                    max,
                    if key == "notefx.groove.timing" {
                        1.0
                    } else {
                        0.0
                    },
                ),
                false,
                &name,
                sheet::percent,
                HUE,
                false,
            );
            if let Some(nv) = d.value {
                let mut e = g;
                set(&mut e, nv);
                sh.acts.push(Act::NoteFx(NfAct::Amounts(track, clip, e)));
            }
            if d.ended {
                sh.acts.push(Act::EndGesture);
            }
        }
        if button(
            sh,
            ("notefx-commit", clip),
            "notefx.groove.commit",
            false,
            false,
        ) {
            sh.acts.push(Act::NoteFx(NfAct::Commit(track, clip)));
        }
    }
    if button(
        sh,
        ("notefx-extract", clip),
        "notefx.groove.extract",
        false,
        false,
    ) {
        sh.acts.push(Act::NoteFx(NfAct::Extract(track, clip)));
    }
    // The track's note effects, written into the clip.
    let chain = p.track(track).is_some_and(|t| !t.note_fx.is_empty());
    if chain && !c.skip_note_fx && button(sh, ("notefx-bake", clip), "notefx.bake", false, false) {
        sh.acts.push(Act::NoteFx(NfAct::Bake(track, clip)));
    }
    if chain || c.skip_note_fx {
        let on = sh.led_row_of(
            clip,
            "notefx.skip",
            "notefx.skip.key",
            c.skip_note_fx,
            HUE,
            false,
        );
        if on {
            sh.acts
                .push(Act::NoteFx(NfAct::Skip(track, clip, !c.skip_note_fx)));
        }
    }
}

/// A push button across the value column (as the audio section's), its label inside it;
/// `busy` draws it disabled (an analysis on its way).
fn button(
    sh: &mut Sheet<'_>,
    id: impl std::hash::Hash + std::fmt::Debug,
    key: &str,
    strong: bool,
    busy: bool,
) -> bool {
    let s = sh.s;
    let row = sh.row();
    let w = row.value.width().min(s.s(160.0));
    let br = Rect::from_min_size(row.value.min, Vec2::new(w, row.value.height()));
    let r = sh
        .ui
        .interact(br, Id::new(("notefx-btn", id)), Sense::click());
    let st = State {
        disabled: busy,
        ..State::of(&r, strong)
    };
    let look = cap::CapLook {
        lit: Some(if strong { s.t.highlight } else { s.t.selection }),
        ..Default::default()
    };
    cap::paint(sh.b, br, &s, st, &look);
    let ink = crate::kit::color::on_fill(cap::face(&s, st, &look));
    let tx = Text::key(key, Role::Control, ink)
        .align(Align2::CENTER_CENTER)
        .policy(Policy::Elide(Elide::End));
    let _ = text::paint(sh.b, br.shrink2(Vec2::new(s.s(6.0), 0.0)), &tx, s.tk());
    if r.hovered() {
        let name = sh.fmt.tr(key).to_string();
        let tip = format!("{key}.tip");
        let value = sh.fmt.tr(&tip).to_string();
        hint::set_text(sh.ui.ctx(), &name, &value, Caps::NONE);
    }
    r.clicked() && !busy
}

/// A choice's or a switch's row: its word in a field (a choice's opens its list, a switch's
/// flips it), the parameter's menu on a right-click (*Create automation clip*,
/// *Modulate…*), the marks of what drives it.
#[allow(clippy::too_many_arguments)]
fn word_row(
    sh: &mut Sheet<'_>,
    app: &CantusApp,
    track: TrackId,
    e: &NoteEffect,
    p: &'static NoteFxParam,
    v: f32,
    range: &Range,
) {
    let row = sh.row();
    let dim = !e.enabled;
    sh.label(row.label, p.label_key, dim);
    let target = AutoTarget::NoteFx {
        track,
        effect: e.id,
        key: p.key.to_string(),
    };
    let kind = e.kind.kind();
    let value = sh.fmt.get(
        cache::key(("notefx-word", e.id, p.key)),
        sheet::bits(v) ^ ((crate::i18n::locale() as u64) << 60),
        |b| crate::notefx_ui::write_value(kind, p.key, v, b),
    );
    let value = sheet::name(value);
    let rect = row.value;
    let resp = sh.ui.interact(
        rect,
        Id::new(("notefx-word-field", e.id, p.key)),
        Sense::click(),
    );
    let choice = p.unit == NoteFxUnit::Choice;
    let open = app
        .notefx
        .choice_menu
        .is_some_and(|(_, id, k, _)| id == e.id && k == p.key);
    let st = State {
        disabled: dim,
        ..State::of(&resp, open)
    };
    if choice {
        well::menu_field(sh.b, rect, &sh.s.function(HUE), st, &value, false);
    } else {
        well::readout(sh.b, rect, &sh.s.function(HUE), st, &value, false);
    }
    if resp.hovered() {
        let name = sheet::name(sh.fmt.tr(p.label_key));
        hint::set_text(
            sh.ui.ctx(),
            &name,
            &value,
            Caps {
                menu: choice,
                ..Caps::PARAM
            },
        );
    }
    if resp.clicked() {
        let act = if choice {
            NfAct::ChoiceMenu(track, e.id, p.key, rect)
        } else {
            NfAct::Set(track, e.id, p.key, if v >= 0.5 { 0.0 } else { 1.0 })
        };
        sh.acts.push(Act::NoteFx(act));
    }
    // What drives it, and its menu (as `wired_value` gives a value well).
    let mut rings = [(0.0, egui::Color32::TRANSPARENT); crate::views::modulator::MAX_RINGS];
    let (m, n) = app.mod_offer(&target, range.pos(v), &sh.s, &mut rings);
    let offer = crate::kit::automate::Offer {
        automated: app.is_automated(&target),
        typing: false,
        modulation: Some(m),
    };
    if offer.automated {
        crate::kit::automate::mark(sh.b, rect, &sh.s);
    }
    if n > 0 {
        crate::kit::automate::mod_mark(sh.b, rect, &sh.s);
    }
    let c = crate::kit::automate::modulation_choice(sh.ui, &resp, Some(offer))
        .or_else(|| crate::kit::automate::menu(sh.ui, &resp, offer));
    if let Some(c) = c {
        sh.acts
            .push(Act::Mod(super::modulators::ModAct::Param(target, c, rect)));
    }
}

/// One of a groove's amounts as its row edits it: the key, the value, its top, the setter.
type Amount = (&'static str, f32, f32, fn(&mut ClipGroove, f32));

/// A pooled groove's name being typed: its text as the field holds it.
#[derive(Clone, Debug)]
struct Naming {
    groove: cantus_core::GrooveId,
    text: String,
}

/// The pooled groove's name, renamed in place (a click opens the kit's field; Enter or a
/// click elsewhere renames it — one `SetGroove` — Esc leaves it).
fn pool_name_row(sh: &mut Sheet<'_>, app: &CantusApp, gid: cantus_core::GrooveId) {
    let Some(g) = app.project().grooves.iter().find(|g| g.id == gid) else {
        return;
    };
    let row = sh.row();
    sh.label(row.label, "notefx.groove.name", false);
    let id = Id::new("notefx-groove-name");
    let ctx = sh.ui.ctx().clone();
    let open = ctx
        .data(|d| d.get_temp::<Naming>(id))
        .filter(|n| n.groove == gid);
    let Some(Naming { mut text, .. }) = open else {
        ctx.data_mut(|d| d.remove::<Naming>(id));
        let r = sh.ui.interact(row.value, id, Sense::click());
        if r.hovered() {
            hint::set_text(&ctx, &g.name, "", Caps::NONE);
        }
        well::readout(sh.b, row.value, &sh.s, State::of(&r, false), &g.name, false);
        if r.clicked() {
            *sh.menu = None;
            let naming = Naming {
                groove: gid,
                text: g.name.clone(),
            };
            ctx.data_mut(|d| d.insert_temp(id, naming));
            ctx.request_repaint();
        }
        return;
    };
    let out = crate::kit::field::Field::new(id)
        .opens_when_shown()
        .show(sh.ui, row.value, &mut text);
    match out.event {
        Some(crate::kit::field::Event::Cancel) => {
            ctx.data_mut(|d| d.remove::<Naming>(id));
        }
        Some(crate::kit::field::Event::Commit) => {
            ctx.data_mut(|d| d.remove::<Naming>(id));
            let name = text.trim();
            if !name.is_empty() && name != g.name {
                let others: Vec<cantus_core::Groove> = app
                    .project()
                    .grooves
                    .iter()
                    .filter(|x| x.id != gid)
                    .cloned()
                    .collect();
                let renamed = cantus_core::Groove {
                    name: cantus_core::groove::unique_name(&others, name),
                    ..g.clone()
                };
                sh.acts.push(Act::Exec(cantus_core::Command::SetGroove {
                    groove: Box::new(renamed),
                }));
            }
        }
        None => {
            ctx.data_mut(|d| d.insert_temp(id, Naming { groove: gid, text }));
        }
    }
}
