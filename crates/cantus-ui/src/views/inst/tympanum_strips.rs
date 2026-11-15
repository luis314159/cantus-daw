//! Tympanum's mixer (ADR 0123, ADR 0124): the one-click routes over the strips, and the
//! strips themselves — *Kit* pinned at the left (the whole kit: its bleed, its level, where
//! it plays), every mic under its family's head in the scrolling middle, *Ambience* pinned at
//! the right (the room, its size and decay, one fader for the room mics). Each strip is
//! `super::super::inst_mixer`'s; what a strip's edit does is said here, on Tympanum's keys.

use super::super::inst_mixer::{self as im, desk, strip, Edit, OutCell, Strip, Top};
use super::iu::{self, words};
use super::{def, Look, Open};
use crate::app::CantusApp;
use crate::i18n::{t, tf};
use crate::kit::menu::Item;
use crate::kit::text::Src;
use crate::kit::{with_brush, Style};
use crate::views::mixer::look::{self, Pal, W};
use cantus_core::inst::tympanum_kit::KitDef;
use cantus_core::inst_out::{self, OutputField, OutputPlan, RouteView};
use cantus_core::{Command, MergeKey, NativeSpec, OutputId, TrackId};
use egui::{pos2, vec2, Align2, Rect};

/// The actions row's height at 100 %.
pub const ACTIONS: f32 = 36.0;
/// The Ambience strip's width at 100 %.
const AMB_W: f32 = 128.0;
/// Where the engine reports each mic's peak (linear) in the instrument's telemetry: slot
/// `METER_TEL + n` for `o{n}` (`tymp-engine`'s unit writes them every block).
pub use cantus_engine::inst::tympanum::METER_TEL;

/// The meters a groove leaves, for the shots and tests with no engine.
pub fn demo_levels(tel: &mut cantus_engine::Telemetry) {
    const DB: [f32; 15] = [
        -9.0, -7.5, -14.0, -6.0, -15.0, -18.0, -22.0, -24.0, -21.0, -20.0, -19.0, -10.0, -12.0,
        -15.0, -17.0,
    ];
    for (i, d) in DB.iter().enumerate() {
        if let Some(v) = tel.get_mut(METER_TEL + i) {
            *v = cantus_core::db_to_linear(*d);
        }
    }
}

fn key(id: OutputId, f: OutputField) -> String {
    inst_out::key(id, f)
}

/// A mic's bleed as a share of as-recorded: the mean of its cells for the pieces it is not
/// the main mic of (dB, as gain), 1 at 0 dB.
pub fn bleed_share(def: &KitDef, spec: &NativeSpec, n: u16) -> f32 {
    let Some(m) = def.mics.get(usize::from(n)) else {
        return 1.0;
    };
    let others: Vec<u16> = def
        .pieces
        .iter()
        .map(|p| p.index)
        .filter(|p| !m.main_for.contains(p))
        .collect();
    if others.is_empty() {
        return 1.0;
    }
    let sum: f32 = others
        .iter()
        .map(|p| spec.get(&format!("b{p}.{n}.db")))
        .sum();
    cantus_core::db_to_linear(sum / others.len() as f32)
}

/// The commands that set mic `n`'s bleed to `share` of as-recorded (every cell but its own
/// pieces').
pub fn set_bleed(def: &KitDef, track: TrackId, n: u16, share: f32) -> Vec<Command> {
    let Some(m) = def.mics.get(usize::from(n)) else {
        return Vec::new();
    };
    let db = if share <= 0.0 {
        -96.0
    } else {
        cantus_core::linear_to_db(share).max(-96.0)
    };
    def.pieces
        .iter()
        .filter(|p| !m.main_for.contains(&p.index))
        .map(|p| Command::SetInstrumentValue {
            track,
            key: format!("b{}.{n}.db", p.index),
            value: db,
        })
        .collect()
}

/// Where the track plays: its strip and its number.
fn track_strip(app: &CantusApp, track: TrackId) -> Option<(cantus_core::StripId, usize)> {
    let p = app.project();
    let s = p.track(track)?.strip?;
    Some((s, p.strip_number(s)?))
}

/// The Out cell of mic `id`.
fn out_cell(app: &CantusApp, track: TrackId, name: &str, id: OutputId) -> OutCell {
    let p = app.project();
    let inst = p.track(track).map(|t| t.name.clone()).unwrap_or_default();
    let mine = track_strip(app, track);
    match inst_out::route_view(p, track, id) {
        RouteView::Main => OutCell {
            label: t("inst_mixer.out_main"),
            number: None,
            opens: true,
            tip: tf(
                "inst_mixer.out_tip",
                &[
                    ("name", &name.to_string()),
                    (
                        "where",
                        &tf(
                            "inst_mixer.out_says_main",
                            &[("n", &mine.map_or(0, |m| m.1))],
                        ),
                    ),
                ],
            ),
        },
        RouteView::Own(s) => {
            let n = p.strip_number(s).unwrap_or(0);
            OutCell {
                label: t("inst_mixer.out_own"),
                number: Some(n),
                opens: true,
                tip: tf(
                    "inst_mixer.out_tip",
                    &[
                        ("name", &name.to_string()),
                        (
                            "where",
                            &tf(
                                "inst_mixer.out_says_own",
                                &[("n", &n), ("inst", &inst), ("m", &mine.map_or(0, |m| m.1))],
                            ),
                        ),
                    ],
                ),
            }
        }
        RouteView::Chosen(s) => {
            let n = p.strip_number(s).unwrap_or(0);
            let sname = p.strip(s).map(|x| x.name.clone()).unwrap_or_default();
            OutCell {
                label: sname.clone(),
                number: Some(n),
                opens: true,
                tip: tf(
                    "inst_mixer.out_tip",
                    &[
                        ("name", &name.to_string()),
                        (
                            "where",
                            &tf("inst_mixer.out_says_chosen", &[("name", &sname), ("n", &n)]),
                        ),
                    ],
                ),
            }
        }
    }
}

/// A mic's meter: its own strip's when it plays on one, else what the engine reports.
fn meter_of(app: &CantusApp, track: TrackId, id: OutputId) -> Option<[f32; 2]> {
    let p = app.project();
    let lin = match inst_out::route_view(p, track, id) {
        RouteView::Own(s) | RouteView::Chosen(s) => {
            let i = p.strip_index(s)?;
            app.engine.meters().strips.get(i).copied().unwrap_or(0.0)
        }
        RouteView::Main => app
            .inst_telemetry()
            .get(METER_TEL + usize::from(id.0))
            .copied()
            .unwrap_or(0.0),
    };
    (lin > 1e-5).then(|| {
        let d = cantus_core::linear_to_db(lin);
        [d, d - 0.6]
    })
}

/// The mics as strips, in the kit's order.
pub fn mic_strips(
    app: &CantusApp,
    track: TrackId,
    spec: &NativeSpec,
    def: &KitDef,
    lk: &Look,
) -> Vec<Strip> {
    def.mics
        .iter()
        .map(|m| {
            let id = m.output;
            let g = |f| spec.get(&key(id, f));
            let top = if m.main_for.is_empty() {
                Top::Words(t("tympanum.mic.hears_all"))
            } else {
                Top::Share {
                    word: t("inst_mixer.bleed_short"),
                    value: bleed_share(def, spec, id.0),
                    full: 1.0,
                }
            };
            let name = def::mic_name(def, m);
            Strip {
                id: Some(id),
                out: out_cell(app, track, &name, id),
                sub: def::mic_kind(def, m),
                name,
                family: m.family,
                top,
                invert: Some(g(OutputField::Invert) >= 0.5),
                pan: Some(g(OutputField::Pan)),
                mute: Some(g(OutputField::Mute) >= 0.5),
                solo: Some(g(OutputField::Solo) >= 0.5),
                delay: Some(g(OutputField::Delay)),
                level_db: g(OutputField::Level),
                level_max: 12.0,
                meter: meter_of(app, track, id),
                selected: lk.mic == id.0,
                loaded: g(OutputField::On) >= 0.5,
            }
        })
        .collect()
}

/// The whole kit's strip, pinned at the left: its bleed (DrumGizmo's one fader), its level
/// (`gain`), where the instrument plays.
fn kit_strip(app: &CantusApp, track: TrackId, spec: &NativeSpec) -> Strip {
    let mine = track_strip(app, track);
    let p = app.project();
    let name = p.track(track).map(|t| t.name.clone()).unwrap_or_default();
    let meter = p.tracks.iter().position(|t| t.id == track).and_then(|i| {
        let lin = app.engine.meters().tracks.get(i).copied().unwrap_or(0.0);
        (lin > 1e-5).then(|| {
            let d = cantus_core::linear_to_db(lin);
            [d, d - 0.4]
        })
    });
    Strip {
        id: None,
        name: t("tympanum.strip.kit"),
        sub: t("tympanum.strip.kit_sub"),
        family: cantus_core::OutputFamily::Other,
        top: Top::Share {
            word: t("inst_mixer.bleed_short"),
            value: cantus_core::db_to_linear(spec.get("bleed")),
            full: 1.0,
        },
        invert: None,
        pan: None,
        mute: None,
        solo: None,
        delay: None,
        level_db: spec.get("gain"),
        level_max: 12.0,
        meter,
        out: OutCell {
            label: name.clone(),
            number: mine.map(|m| m.1),
            opens: false,
            tip: tf(
                "inst_mixer.out_tip",
                &[
                    ("name", &t("tympanum.strip.kit")),
                    (
                        "where",
                        &tf(
                            "inst_mixer.out_says_main",
                            &[("n", &mine.map_or(0, |m| m.1))],
                        ),
                    ),
                ],
            ),
        },
        selected: false,
        loaded: true,
    }
}

/// The row over the strips: *Outputs* and the three one-click routes, what they left in
/// words, the mix's preset and the kit's bleed switch.
#[allow(clippy::too_many_arguments)]
pub fn actions(
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
    let pad = s.s(10.0);
    let h = s.s(24.0);
    let cy = r.center().y;
    let mut x = r.left() + pad;
    let label = t("inst_mixer.outputs");
    let lw = iu::width(ui, s, &label, 12.0, W::Regular);
    with_brush(ui.painter(), |b| {
        let lr = Rect::from_min_max(pos2(x, r.top()), pos2(x + lw + 1.0, r.bottom()));
        words(
            b,
            s,
            &label,
            lr,
            Align2::LEFT_CENTER,
            12.0,
            W::Regular,
            p.ink3,
        );
    });
    x += lw + s.s(14.0);
    for (plan, word, tip) in [
        (
            OutputPlan::AllToMain,
            "inst_mixer.all_main",
            "inst_mixer.all_main_tip",
        ),
        (
            OutputPlan::ByFamily,
            "inst_mixer.by_family",
            "inst_mixer.by_family_tip",
        ),
        (
            OutputPlan::EachOwn,
            "inst_mixer.each_own",
            "inst_mixer.each_own_tip",
        ),
    ] {
        let text = t(word);
        let w = iu::width(ui, s, &text, 12.0, W::Medium) + s.s(24.0);
        let br = Rect::from_min_size(pos2(x, cy - h * 0.5), vec2(w, h));
        ui.ctx()
            .data_mut(|d| d.insert_temp(plan_id(track, plan), br));
        let resp = iu::button(
            ui,
            s,
            br,
            ui.id().with(("tymp-plan", word)),
            &text,
            iu::Lead::None,
            false,
            true,
        );
        let resp = iu::hint(resp, || t(tip));
        if resp.clicked() {
            route_all(app, track, plan, lk);
        }
        x += w + s.s(6.0);
    }
    // What the routes leave, in words.
    let slots = spec.outputs.len();
    let strips = inst_out::strips_of(&spec.outputs).len();
    let main = def.mics.len().saturating_sub(slots);
    let said = lk.said.clone().unwrap_or_else(|| match strips {
        0 => t("inst_mixer.result_none"),
        1 => tf("inst_mixer.result_one", &[("main", &main)]),
        n => tf("inst_mixer.result", &[("strips", &n), ("main", &main)]),
    });
    // The right: the mix's preset and the bleed switch.
    let bleed_on = spec.get("bleed") > -95.0;
    let bw = iu::width(ui, s, &t("inst_mixer.bleed"), 12.0, W::Regular);
    let sw_right = r.right() - pad;
    let sw = Rect::from_min_max(
        pos2(sw_right - s.s(26.0), cy - s.s(7.0)),
        pos2(sw_right, cy + s.s(7.0)),
    );
    let sresp = ui.interact(
        sw.expand(s.s(3.0)),
        ui.id().with("tymp-bleed"),
        egui::Sense::click(),
    );
    let sresp = iu::hint(sresp, || t("inst_mixer.bleed_tip"));
    if sresp.clicked() {
        app.set_inst_value(track, "bleed", if bleed_on { -96.0 } else { 0.0 }, false);
    }
    let preset = app
        .inst
        .preset
        .get(&track)
        .cloned()
        .unwrap_or_else(|| t("inst_mixer.mix_custom"));
    let mix_w =
        (iu::width(ui, s, &preset, 12.0, W::Regular) + s.s(40.0)).clamp(s.s(150.0), s.s(240.0));
    let field = Rect::from_min_max(
        pos2(
            sw.left() - s.s(10.0) - bw - s.s(16.0) - mix_w,
            cy - s.s(12.0),
        ),
        pos2(sw.left() - s.s(10.0) - bw - s.s(16.0), cy + s.s(12.0)),
    );
    let fresp = ui.interact(field, ui.id().with("tymp-mix"), egui::Sense::click());
    let fresp = iu::hint(fresp, || t("inst_mixer.mix_tip"));
    if fresp.clicked() {
        app.inst.popup = super::super::Popup::Presets;
        app.inst.popup_pass = ui.ctx().cumulative_pass_nr();
    }
    let mixw = t("inst_mixer.mix");
    let mw = iu::width(ui, s, &mixw, 12.0, W::Regular);
    with_brush(ui.painter(), |b| {
        let st = crate::kit::State::of(&sresp, bleed_on);
        crate::views::inspector::look::switch(b, s, sw.right(), cy, bleed_on, st);
        let br = Rect::from_min_max(
            pos2(sw.left() - s.s(8.0) - bw - 1.0, r.top()),
            pos2(sw.left() - s.s(8.0), r.bottom()),
        );
        words(
            b,
            s,
            &t("inst_mixer.bleed"),
            br,
            Align2::LEFT_CENTER,
            12.0,
            W::Regular,
            p.ink2,
        );
        let st = crate::kit::State::of(&fresp, false);
        crate::views::inspector::look::field(
            b,
            s,
            field,
            st,
            crate::views::inspector::look::Lead::None,
            &preset,
            "",
            true,
        );
        let mr = Rect::from_min_max(
            pos2(field.left() - s.s(8.0) - mw - 1.0, r.top()),
            pos2(field.left() - s.s(8.0), r.bottom()),
        );
        words(
            b,
            s,
            &mixw,
            mr,
            Align2::LEFT_CENTER,
            12.0,
            W::Regular,
            p.ink3,
        );
        let room = Rect::from_min_max(
            pos2(x + s.s(8.0), r.top()),
            pos2(mr.left() - s.s(12.0), r.bottom()),
        );
        if room.width() > s.s(40.0) {
            words(
                b,
                s,
                &said,
                room,
                Align2::LEFT_CENTER,
                12.0,
                W::Regular,
                p.ink2,
            );
        }
    });
}

fn plan_id(track: TrackId, plan: OutputPlan) -> egui::Id {
    egui::Id::new(("tymp-plan-rect", track.0, plan))
}

/// Where the button of `plan` was drawn last frame (tests aim at it).
pub fn plan_rect(ctx: &egui::Context, track: TrackId, plan: OutputPlan) -> Option<Rect> {
    ctx.data(|d| d.get_temp::<Rect>(plan_id(track, plan)))
}

/// *All to Main · Pieces to strips · Each mic to its own strip*: one batch, one undo step
/// (`inst_out::plan`, `tymp-engine`'s), or why not on the status line.
pub fn route_all(app: &mut CantusApp, track: TrackId, plan: OutputPlan, lk: &mut Look) {
    let view = app.project().clone();
    // r40:fix-f — strips named in the interface's language (findings 7 and the owner's
    // follow-up): a family's strip by the family's word, a modelled kit's mic by its
    // translated name; a recorded kit's mics keep their pack's names.
    let kit = view
        .track(track)
        .and_then(|t| t.instrument.native())
        .map(cantus_core::inst::tympanum::KitRef::of)
        .unwrap_or_else(cantus_core::inst::tympanum::KitRef::default_kit);
    let planned = inst_out::plan_with(
        &view,
        track,
        plan,
        &mut |n| app.bus.new_strip(n),
        &|n: &str| strip_said(&kit, plan, n),
    );
    lk.said = None;
    match planned {
        Ok(cmd) => {
            // r40:tymp-ui — the sub-groups by output family (the owner, 2026-11-12): *Each
            // mic to its own strip* gathers each family's strips under a group inside the
            // kit's band; every other plan takes this kit's sub-groups away. One step.
            // r40:fix-f — planned step by step on a copy (review of 40a27bb7, finding 2):
            // the new groups are inserted where the old ones' removal leaves the list.
            let olds = kit_groups(&view, track);
            let mut st = super::kit_change::Steps::new(view.clone());
            for (_, members) in &olds {
                for m in members {
                    st.push(Command::SetStripGroup {
                        id: *m,
                        group: None,
                    });
                }
            }
            if !st.push(cmd.clone()) {
                app.exec(cmd);
                return;
            }
            for (g, _) in &olds {
                st.push(Command::RemoveStripGroup { id: *g });
            }
            if plan == OutputPlan::EachOwn {
                let after = st.view.clone();
                for c in regroup(app, &after, track) {
                    st.push(c);
                }
            }
            app.exec(Command::Batch {
                label_key: "cmd.inst_output_plan".into(),
                commands: st.cmds,
            });
        }
        Err(r) => {
            use cantus_core::padout::Refusal;
            let name = view
                .track(track)
                .map(|t| t.name.clone())
                .unwrap_or_default();
            let k = match r {
                Refusal::NotAKit => "padout.not_a_kit",
                Refusal::NoRoom => "padout.no_room",
                Refusal::Unfit => "pads.unfit",
                Refusal::Nothing | Refusal::NoPad => "inst_mixer.nothing",
            };
            app.set_error(tf(k, &[("name", &name)]));
        }
    }
}

/// A new strip's name for `plan` from what the core names it (a mic's name, a family's
/// English name), in the interface's language.
pub(super) fn strip_said(
    kit: &cantus_core::inst::tympanum::KitRef,
    plan: OutputPlan,
    n: &str,
) -> String {
    if plan == OutputPlan::ByFamily {
        if let Some(f) = cantus_core::OutputFamily::ALL
            .iter()
            .find(|f| inst_out::family_name(**f) == n)
        {
            return t(f.label_key());
        }
    }
    def::mic_said(kit, n)
}

/// The groups inside `track`'s family: groups every member of which is one of its outputs'
/// strips, or a strip made for an output that now carries nothing (a mic sent back to
/// Main) — what a plan takes away and *Each mic to its own strip* makes again.
pub(super) fn kit_groups(
    p: &cantus_core::Project,
    track: TrackId,
) -> Vec<(cantus_core::GroupId, Vec<cantus_core::StripId>)> {
    let outs = p
        .track(track)
        .map(|t| t.instrument.output_strips())
        .unwrap_or_default();
    let left = cantus_core::padout::orphans(p);
    p.strip_groups
        .iter()
        .filter_map(|g| {
            let members: Vec<cantus_core::StripId> = p
                .strips
                .iter()
                .filter(|s| s.group == Some(g.id))
                .map(|s| s.id)
                .collect();
            (members.iter().any(|m| outs.contains(m))
                && members.iter().all(|m| outs.contains(m) || left.contains(m)))
            .then_some((g.id, members))
        })
        .collect()
}

/// One group per output family (Kick, Snare, Hi-hat, Toms, Cymbals, Rooms…) over the strips
/// made for the kit's outputs as `view` has them — each strip of one family, in no group
/// yet — named by the family, in the track's colour; a family of one strip takes no group.
/// Inserted after the groups `view` has (planned on the song as the batch leaves it).
pub(super) fn regroup(
    app: &mut CantusApp,
    view: &cantus_core::Project,
    track: TrackId,
) -> Vec<Command> {
    let Some(tr) = view.track(track) else {
        return Vec::new();
    };
    let Some(n) = tr.instrument.native() else {
        return Vec::new();
    };
    let color = tr.color;
    let mut fams: Vec<(cantus_core::OutputFamily, Vec<cantus_core::StripId>)> = Vec::new();
    for s in &view.strips {
        if !cantus_core::padout::pad_made(s) || s.group.is_some() {
            continue;
        }
        let mut on = n.outputs.iter().filter(|o| o.strip == s.id);
        let Some(first) = on.next() else {
            continue;
        };
        let fam = first.family;
        if on.any(|o| o.family != fam) {
            continue;
        }
        match fams.iter_mut().find(|(f, _)| *f == fam) {
            Some((_, v)) => v.push(s.id),
            None => fams.push((fam, vec![s.id])),
        }
    }
    let mut cmds = Vec::new();
    let wanted = fams.into_iter().filter(|(_, v)| v.len() > 1);
    for (index, (fam, strips)) in (view.strip_groups.len()..).zip(wanted) {
        let id = cantus_core::GroupId(app.bus.alloc_id());
        cmds.push(Command::InsertStripGroup {
            index,
            group: Box::new(cantus_core::StripGroup {
                id,
                name: crate::i18n::t(fam.label_key()),
                color,
                collapsed: false,
            }),
        });
        cmds.extend(strips.into_iter().map(|s| Command::SetStripGroup {
            id: s,
            group: Some(id),
        }));
    }
    cmds
}

/// The strips: *Kit* | the mics under their heads, scrolling | *Ambience*.
#[allow(clippy::too_many_arguments)]
pub fn desk(
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
    let inner = r.shrink(s.s(6.0));
    let pitch = s.s(im::STRIP_W);
    let g = s.s(8.0);
    let left = Rect::from_min_size(inner.min, vec2(pitch, inner.height()));
    let right = Rect::from_min_max(pos2(inner.right() - s.s(AMB_W), inner.top()), inner.max);
    let heads = Rect::from_min_max(
        pos2(left.right() + g, inner.top()),
        pos2(right.left() - g, inner.top() + s.s(im::HEADS)),
    );
    let mid = Rect::from_min_max(
        pos2(heads.left(), heads.bottom() + s.s(4.0)),
        pos2(heads.right(), inner.bottom()),
    );
    // Kit, pinned left.
    let kit = kit_strip(app, track, spec);
    let kit_id = ui.id().with(("tymp-kit-strip", track.0));
    for e in strip::show(ui, s, left, &kit, kit_id) {
        match e {
            Edit::Top(f) => app.set_inst_value(track, "bleed", share_db(f), true),
            Edit::Level(db) => app.set_inst_value(track, "gain", db.clamp(-24.0, 12.0), true),
            Edit::End => app.end_gesture(),
            _ => {}
        }
    }
    // The mics.
    let strips = mic_strips(app, track, spec, def, lk);
    let mut scroll = lk.strips_scroll;
    let salt = ui.id().with(("tymp-mics", track.0));
    let (edits, shown) = desk::middle(ui, s, heads, mid, &strips, &lk.folded, &mut scroll, salt);
    lk.strips_scroll = scroll;
    if let Some(f) = shown.toggle {
        if let Some(i) = lk.folded.iter().position(|x| *x == f) {
            lk.folded.remove(i);
        } else {
            lk.folded.push(f);
        }
    }
    for (i, e) in edits {
        if let Some(id) = strips[i].id {
            mic_edit(app, ui, track, spec, def, lk, id, e);
        }
    }
    // The dividers either side of the middle.
    with_brush(ui.painter(), |b| {
        for x in [left.right() + g * 0.5, right.left() - g * 0.5] {
            b.vline(
                b.snap(x) + b.px() * 0.5,
                inner.top(),
                inner.bottom(),
                b.px(),
                p.line,
            );
        }
    });
    ambience(app, ui, track, spec, s, right);
    out_list(app, ui, track, s, lk);
}

/// A bar's share as dB: 0 is silence (−96), 1 is as recorded (0 dB).
fn share_db(f: f32) -> f32 {
    if f <= 0.001 {
        -96.0
    } else {
        cantus_core::linear_to_db(f).max(-96.0)
    }
}

#[allow(clippy::too_many_arguments)]
fn mic_edit(
    app: &mut CantusApp,
    ui: &egui::Ui,
    track: TrackId,
    spec: &NativeSpec,
    def: &KitDef,
    lk: &mut Look,
    id: OutputId,
    e: Edit,
) {
    let flip = |f: OutputField| {
        if spec.get(&key(id, f)) >= 0.5 {
            0.0
        } else {
            1.0
        }
    };
    match e {
        Edit::Top(f) => {
            let cmds = set_bleed(def, track, id.0, f);
            if !cmds.is_empty() {
                app.exec_merging(
                    Command::Batch {
                        label_key: "cmd.tympanum_bleed".into(),
                        commands: cmds,
                    },
                    MergeKey::of(&("tymp-bleed", track.0, id.0)),
                );
            }
        }
        Edit::Pan(v) => app.set_inst_value(track, &key(id, OutputField::Pan), v, true),
        Edit::Delay(ms) => app.set_inst_value(track, &key(id, OutputField::Delay), ms, true),
        Edit::Level(db) => app.set_inst_value(track, &key(id, OutputField::Level), db, true),
        Edit::Invert => {
            let v = flip(OutputField::Invert);
            app.set_inst_value(track, &key(id, OutputField::Invert), v, false)
        }
        Edit::Mute => {
            let v = flip(OutputField::Mute);
            app.set_inst_value(track, &key(id, OutputField::Mute), v, false)
        }
        Edit::Solo => {
            let v = flip(OutputField::Solo);
            app.set_inst_value(track, &key(id, OutputField::Solo), v, false)
        }
        Edit::End => app.end_gesture(),
        Edit::Select => lk.mic = id.0,
        Edit::Out(at) => {
            lk.mic = id.0;
            lk.open = Some(Open::Out(id.0));
            lk.opened = ui.ctx().cumulative_pass_nr();
            ui.ctx().data_mut(|d| d.insert_temp(out_anchor(track), at));
        }
    }
}

fn out_anchor(track: TrackId) -> egui::Id {
    egui::Id::new(("tymp-out-anchor", track.0))
}

/// The Out list of the mic whose cell was clicked: *Main*, the first free strip, the desk's
/// named strips. A choice is one command (`inst_out::set_route`).
fn out_list(app: &mut CantusApp, ui: &egui::Ui, track: TrackId, s: &Style, lk: &mut Look) {
    let Some(Open::Out(n)) = lk.open else {
        return;
    };
    let Some(at) = ui.ctx().data(|d| d.get_temp::<Rect>(out_anchor(track))) else {
        lk.open = None;
        return;
    };
    let (labels, targets): (Vec<String>, Vec<Option<cantus_core::StripId>>) =
        crate::views::inst::inst_mixer::out_targets(app.project(), track)
            .into_iter()
            .unzip();
    let items: Vec<Item<'_>> = labels.iter().map(|l| Item::action(Src::Str(l))).collect();
    let mut close = false;
    let chosen = crate::views::fx::frame::popup_list(
        ui.ctx(),
        ui.id().with(("tymp-out-list", track.0)),
        pos2(at.left(), at.bottom() + s.s(2.0)),
        at,
        s,
        &items,
        s.s(200.0),
        &mut close,
    );
    if let Some(i) = chosen {
        let target = targets.get(i).copied().flatten();
        let view = app.project().clone();
        if let Ok(cmd) = inst_out::set_route(&view, track, OutputId(n), target) {
            app.exec(cmd);
        }
        lk.open = None;
    } else if close && ui.ctx().cumulative_pass_nr() > lk.opened {
        lk.open = None;
    }
}

/// *Ambience*, pinned at the right: the room the kit plays in, its size and decay, one fader
/// for the room mics (`amb.level`), and where the room mics go.
fn ambience(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    track: TrackId,
    spec: &NativeSpec,
    s: &Style,
    r: Rect,
) {
    let p = Pal::of(s);
    let k = s.k;
    with_brush(ui.painter(), |b| look::body(b, r, &p, None));
    let name = Rect::from_min_size(pos2(r.left(), r.top() + 6.0 * k), vec2(r.width(), 15.0 * k));
    let sub = Rect::from_min_size(pos2(r.left(), name.bottom()), vec2(r.width(), 13.0 * k));
    with_brush(ui.painter(), |b| {
        let amb = t("tympanum.strip.amb");
        words(
            b,
            s,
            &amb,
            name,
            Align2::CENTER_CENTER,
            12.0,
            W::SemiBold,
            p.ink1,
        );
        let sw = t("tympanum.strip.amb_sub");
        words(
            b,
            s,
            &sw,
            sub,
            Align2::CENTER_CENTER,
            10.0,
            W::Regular,
            p.ink3,
        );
    });
    // The room's field: a click steps through the rooms (the room card lists them),
    // picture and sound together.
    let room = super::stage::room_kind(spec);
    let field = Rect::from_min_max(
        pos2(r.left() + 6.0 * k, sub.bottom() + 8.0 * k),
        pos2(r.right() - 6.0 * k, sub.bottom() + 32.0 * k),
    );
    let room_word = t(super::stage::room_name_key(room));
    let fresp = ui.interact(
        field,
        ui.id().with(("tymp-amb-room", track.0)),
        egui::Sense::click(),
    );
    let fresp = iu::hint(fresp, || t("tympanum.room.tip"));
    if fresp.clicked() {
        let rooms = super::stage::ROOMS;
        let at = rooms.iter().position(|k| *k == room).unwrap_or(0);
        super::stage::choose_room(app, track, rooms[(at + 1) % rooms.len()]);
    }
    with_brush(ui.painter(), |b| {
        let st = crate::kit::State::of(&fresp, false);
        let lead = crate::views::inspector::look::Lead::None;
        crate::views::inspector::look::field(b, s, field, st, lead, &room_word, "", true);
    });
    // Size and decay: two cells.
    let cells = Rect::from_min_max(
        pos2(r.left() + 2.0 * k, field.bottom() + 6.0 * k),
        pos2(r.right() - 2.0 * k, field.bottom() + 70.0 * k),
    );
    let half = cells.width() * 0.5;
    for (i, kk) in ["amb.size", "amb.decay"].iter().enumerate() {
        let cr = Rect::from_min_size(
            pos2(cells.left() + half * i as f32, cells.top()),
            vec2(half, cells.height()),
        );
        super::super::organ::parts::at(ui, cr, ("tymp-amb", *kk), |ui| {
            let size = crate::kit::knob::KnobSize::S;
            super::super::organ::parts::cell(app, ui, track, spec, kk, s, size);
        });
    }
    // The caption over its fader, with room to breathe (the owner's fix).
    let cap = Rect::from_min_size(
        pos2(r.left() + 8.0 * k, cells.bottom() + 10.0 * k),
        vec2(r.width() - 16.0 * k, 30.0 * k),
    );
    let used = with_brush(ui.painter(), |b| {
        b.hline(
            r.left() + 10.0 * k,
            r.right() - 10.0 * k,
            b.snap(cap.top() - 5.0 * k),
            b.px(),
            p.line,
        );
        let note = t("tympanum.strip.amb_note");
        iu::prose(b, ui, s, cap, &note, p.ink2, Align2::CENTER_TOP)
    });
    // The fader and meter, and where the rooms go.
    let lower = Rect::from_min_max(
        pos2(r.left() + 24.0 * k, cap.top() + used + 6.0 * k),
        pos2(r.right() - 24.0 * k, r.bottom()),
    );
    let st = Strip {
        id: None,
        name: String::new(),
        sub: String::new(),
        family: cantus_core::OutputFamily::Rooms,
        top: Top::None,
        invert: None,
        pan: None,
        mute: None,
        solo: None,
        delay: None,
        level_db: spec.get("amb.level"),
        level_max: 12.0,
        meter: None,
        out: amb_out(app, track, spec),
        selected: false,
        loaded: true,
    };
    let salt = ui.id().with(("tymp-amb-fader", track.0));
    for e in strip::lower(ui, s, lower, &st, salt) {
        match e {
            Edit::Level(db) => app.set_inst_value(track, "amb.level", db, true),
            Edit::End => app.end_gesture(),
            _ => {}
        }
    }
}

/// The Ambience strip's Out cell: where the kit's room mics really go — *Main*, their own
/// strip, a strip of the user's — and *Several* when they go to different places (review
/// of 40a27bb7, finding 9: it always said *Own strip*).
fn amb_out(app: &CantusApp, track: TrackId, spec: &NativeSpec) -> OutCell {
    let def = def::def_of(spec);
    let rooms: Vec<&cantus_core::inst::tympanum_kit::MicDef> = def
        .mics
        .iter()
        .filter(|m| m.family == cantus_core::OutputFamily::Rooms)
        .collect();
    let p = app.project();
    let routes: Vec<RouteView> = rooms
        .iter()
        .map(|m| inst_out::route_view(p, track, m.output))
        .collect();
    let tip = t("tympanum.strip.amb_out");
    match (rooms.first(), routes.first()) {
        (Some(m), Some(first)) if routes.iter().all(|r| r == first) => OutCell {
            opens: false,
            tip,
            ..out_cell(app, track, &def::mic_name(&def, m), m.output)
        },
        (Some(_), Some(_)) => OutCell {
            label: t("tympanum.strip.amb_out_several"),
            number: None,
            opens: false,
            tip,
        },
        _ => OutCell {
            label: t("inst_mixer.out_main"),
            number: None,
            opens: false,
            tip,
        },
    }
}
