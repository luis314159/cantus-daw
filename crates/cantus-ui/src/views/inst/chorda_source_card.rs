//! The card beside the piano in Chorda's Mixer, and the *Outputs* row under them (the
//! design's *Chorda · Mixer* sheets): the selected output — its name, what it is, the piano
//! it comes from and who recorded it — and, for a piano of several images, the images on
//! disk with the rest to get; the model's perspective. The row: *All to Main* · *Each mic to
//! its own strip* (`inst_out::plan`, one undo step), what the outputs do in words, the
//! mix's preset.

use super::mixer::MixLook;
use super::read;
use super::strip::Out;
use crate::app::CantusApp;
use crate::i18n::{t, tf};
use crate::kit::{with_brush, Fill, Style};
use crate::views::inst::inst_mixer::ui::{self as iu, words};
use crate::views::mixer::look::{Pal, W};
use cantus_core::inst::chorda_source::{self as cs, ChordaSource};
use cantus_core::inst_out::{self, OutputPlan, RouteView};
use cantus_core::{NativeSpec, TrackId};
use egui::{pos2, vec2, Align2, Rect};

/// The card.
#[allow(clippy::too_many_arguments)]
pub fn card(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    track: TrackId,
    spec: &NativeSpec,
    s: &Style,
    r: Rect,
    outs: &[Out],
    lk: &MixLook,
) {
    let p = Pal::of(s);
    with_brush(ui.painter(), |b| iu::card(b, s, r));
    let inner = r.shrink2(vec2(s.s(16.0), s.s(12.0)));
    let set = outs.iter().filter(|o| o.image).count() > 1;
    let split = if set { 0.52 } else { 0.56 };
    let left = Rect::from_min_max(
        inner.min,
        pos2(inner.left() + inner.width() * split, inner.bottom()),
    );
    let right = Rect::from_min_max(
        pos2(inner.left() + inner.width() * (split + 0.06), inner.top()),
        inner.max,
    );
    let sel = outs
        .iter()
        .find(|o| Some(o.id.0) == lk.selected)
        .or(outs.first());
    let entry = match ChordaSource::of(spec) {
        ChordaSource::Recorded(src) if cs::has_zones(spec) => {
            read::entry(app.samples.content(), &src)
        }
        _ => None,
    };
    // The left: the output selected, and where it comes from.
    let (title, note) = match (sel, &entry) {
        (Some(o), Some(e)) => (
            o.name.clone(),
            tf(
                "chorda_source.mixer.as_named",
                &[("who", &e.name), ("what", &o.sub)],
            ),
        ),
        (Some(o), None) => (o.name.clone(), o.sub.clone()),
        (None, _) => (t("chorda_source.mixer.model_title"), String::new()),
    };
    let head = Rect::from_min_size(left.min, vec2(left.width(), s.s(22.0)));
    let body = Rect::from_min_max(pos2(left.left(), head.bottom() + s.s(10.0)), left.max);
    with_brush(ui.painter(), |b| {
        iu::head(b, s, head, &title, &note);
        let text = match &entry {
            Some(e) => {
                let who = super::list::licence_word(&e.licence);
                tf("chorda_source.mixer.recorded", &[("who", &who)])
            }
            None => t("chorda_source.mixer.model_note"),
        };
        let used = iu::prose(b, ui, s, body, &text, p.ink2, Align2::LEFT_TOP);
        if let Some(e) = &entry {
            let cr = Rect::from_min_size(
                pos2(body.left(), body.top() + used + s.s(12.0)),
                vec2(body.width(), s.s(16.0)),
            );
            let cw = words(
                b,
                s,
                &t("chorda_source.mixer.credit"),
                cr,
                Align2::LEFT_CENTER,
                11.0,
                W::Regular,
                p.ink3,
            );
            // The credit whole, broken over lines as it needs.
            let nr = Rect::from_min_max(
                pos2(cr.left() + cw + s.s(14.0), cr.top() + s.s(1.0)),
                body.max,
            );
            iu::prose(b, ui, s, nr, &e.author, p.ink1, Align2::LEFT_TOP);
        }
    });
    if set {
        images(app, ui, s, right, outs);
    } else if entry.is_none() {
        perspective(ui, s, right, spec);
    }
    let _ = track;
}

/// A set's images: how many are on disk, what they take, the rest to get in one click.
fn images(app: &mut CantusApp, ui: &mut egui::Ui, s: &Style, r: Rect, outs: &[Out]) {
    let p = Pal::of(s);
    let imgs: Vec<&Out> = outs.iter().filter(|o| o.image).collect();
    let have = imgs.iter().filter(|o| o.on_disk).count();
    let missing: Vec<&&Out> = imgs.iter().filter(|o| !o.on_disk).collect();
    let m = app.samples.content();
    let installed = m.installed();
    let used: u64 = imgs
        .iter()
        .filter(|o| o.on_disk)
        .filter_map(|o| o.pack.as_ref())
        .filter_map(|id| installed.iter().find(|x| x.id == *id).map(|x| x.bytes))
        .sum();
    let more: u64 = missing
        .iter()
        .filter_map(|o| o.get.as_ref())
        .map(|(id, dl)| {
            m.manifest()
                .pack(id)
                .map(|pk| pk.installed_size)
                .filter(|z| *z > 0)
                .unwrap_or(*dl)
        })
        .sum();
    let head = Rect::from_min_size(r.min, vec2(r.width(), s.s(18.0)));
    let bar = Rect::from_min_size(
        pos2(r.left(), head.bottom() + s.s(10.0)),
        vec2(r.width() * 0.62, s.s(4.0)),
    );
    let line = Rect::from_min_size(
        pos2(r.left(), bar.bottom() + s.s(8.0)),
        vec2(r.width(), s.s(16.0)),
    );
    let btn = Rect::from_min_size(
        pos2(r.left(), line.bottom() + s.s(8.0)),
        vec2(s.s(150.0).min(r.width()), s.s(24.0)),
    );
    let said = if missing.is_empty() {
        tf(
            "chorda_source.mixer.images_used_all",
            &[("used", &super::list::size_words(used))],
        )
    } else {
        tf(
            "chorda_source.mixer.images_used",
            &[
                ("used", &super::list::size_words(used)),
                ("n", &missing.len()),
                ("size", &super::list::size_words(more)),
            ],
        )
    };
    with_brush(ui.painter(), |b| {
        iu::head(
            b,
            s,
            head,
            &t("chorda_source.mixer.images_title"),
            &tf(
                "chorda_source.mixer.images_count",
                &[("n", &have), ("of", &imgs.len())],
            ),
        );
        b.rrect(b.snap_rect(bar), s.s(2.0), Fill::Solid(p.well));
        let f = have as f32 / imgs.len().max(1) as f32;
        let done = Rect::from_min_size(bar.min, vec2(bar.width() * f, bar.height()));
        b.rrect(b.snap_rect(done), s.s(2.0), Fill::Solid(p.ink2));
        words(
            b,
            s,
            &said,
            line,
            Align2::LEFT_CENTER,
            11.0,
            W::Regular,
            p.ink2,
        );
    });
    if missing.is_empty() {
        return;
    }
    let label = tf("chorda_source.mixer.get_other", &[("n", &missing.len())]);
    let resp = iu::button(
        ui,
        s,
        btn,
        ui.id().with("chorda-get-rest"),
        &label,
        iu::Lead::Glyph(crate::kit::glyph_browser::Glyph::Download),
        false,
        true,
    );
    let resp = iu::hint(resp, || t("chorda_source.mixer.each_own"));
    if resp.clicked() {
        for o in &missing {
            if let Some((pack, _)) = &o.get {
                app.samples.content().install(pack);
            }
        }
    }
    with_brush(ui.painter(), |b| {
        let note = Rect::from_min_max(pos2(r.left(), btn.bottom() + s.s(10.0)), r.max);
        iu::prose(
            b,
            ui,
            s,
            note,
            &t("chorda_source.mixer.each_own"),
            p.ink3,
            Align2::LEFT_TOP,
        );
    });
}

/// The model: where the listener stands, and what it moves.
fn perspective(ui: &mut egui::Ui, s: &Style, r: Rect, spec: &NativeSpec) {
    let p = Pal::of(s);
    let head = Rect::from_min_size(r.min, vec2(r.width(), s.s(18.0)));
    let pct = format!("{:.0} %", spec.get("persp") * 100.0);
    with_brush(ui.painter(), |b| {
        iu::head(b, s, head, &t("chorda_source.mixer.persp_title"), &pct);
        let body = Rect::from_min_max(pos2(r.left(), head.bottom() + s.s(10.0)), r.max);
        iu::prose(
            b,
            ui,
            s,
            body,
            &t("chorda_source.mixer.persp_note"),
            p.ink2,
            Align2::LEFT_TOP,
        );
    });
}

/// The *Outputs* row: the one-click routes, what they leave in words, the mix's preset.
#[allow(clippy::too_many_arguments)]
pub fn actions(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    track: TrackId,
    spec: &NativeSpec,
    s: &Style,
    r: Rect,
    outs: &[Out],
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
            ui.id().with(("chorda-plan", word)),
            &text,
            iu::Lead::None,
            false,
            true,
        );
        let resp = iu::hint(resp, || t(tip));
        if resp.clicked() {
            route_all(app, track, plan);
        }
        x += w + s.s(6.0);
    }
    // What the outputs do now, in words.
    let loaded: Vec<&Out> = outs.iter().filter(|o| o.on_disk && o.on).collect();
    let own = loaded
        .iter()
        .filter(|o| {
            !matches!(
                inst_out::route_view(app.project(), track, o.id),
                RouteView::Main
            )
        })
        .count();
    let missing = outs.iter().filter(|o| !o.on_disk).count();
    let said = if !cs::has_zones(spec) && !cs::MODEL_RENDERS_OUTPUTS {
        t("chorda_source.mixer.model_main")
    } else if missing > 0 {
        tf(
            "chorda_source.mixer.summary",
            &[
                ("own", &own),
                ("main", &(loaded.len() - own)),
                ("missing", &missing),
            ],
        )
    } else {
        tf(
            "chorda_source.mixer.summary_all",
            &[("own", &own), ("main", &(loaded.len() - own))],
        )
    };
    // The right: the mix's preset (the instrument's presets hold the mic mix).
    let preset = app
        .inst
        .preset
        .get(&track)
        .cloned()
        .unwrap_or_else(|| t("inst_mixer.mix_custom"));
    let mix_w =
        (iu::width(ui, s, &preset, 12.0, W::Regular) + s.s(40.0)).clamp(s.s(150.0), s.s(240.0));
    let field = Rect::from_min_max(
        pos2(r.right() - pad - mix_w, cy - s.s(12.0)),
        pos2(r.right() - pad, cy + s.s(12.0)),
    );
    let fresp = ui.interact(field, ui.id().with("chorda-mix"), egui::Sense::click());
    let fresp = iu::hint(fresp, || t("chorda_source.mixer.mix_tip"));
    if fresp.clicked() {
        app.inst.popup = super::super::Popup::Presets;
        app.inst.popup_pass = ui.ctx().cumulative_pass_nr();
        app.inst.popup_from = Some(field);
    }
    let mixw = t("inst_mixer.mix");
    let mw = iu::width(ui, s, &mixw, 12.0, W::Regular);
    with_brush(ui.painter(), |b| {
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
            iu::note(
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
    egui::Id::new(("chorda-plan-rect", track.0, plan))
}

/// Where the button of `plan` was drawn last frame (tests aim at it).
pub fn plan_rect(ctx: &egui::Context, track: TrackId, plan: OutputPlan) -> Option<Rect> {
    ctx.data(|d| d.get_temp::<Rect>(plan_id(track, plan)))
}

/// *All to Main* · *Each mic to its own strip*: one batch, one undo step (`inst_out::plan`),
/// or why not on the status line.
pub fn route_all(app: &mut CantusApp, track: TrackId, plan: OutputPlan) {
    let view = app.project().clone();
    let name = view
        .track(track)
        .map(|t| t.name.clone())
        .unwrap_or_default();
    let made = inst_out::plan(&view, track, plan, &mut |n| {
        app.bus.new_strip(format!("{name} \u{b7} {n}"))
    });
    match made {
        Ok(cmd) => {
            app.exec(cmd);
        }
        Err(_) => {
            let spec = view.track(track).and_then(|t| t.instrument.native());
            let k = if spec.is_some_and(|n| !cs::has_zones(n)) && !cs::MODEL_RENDERS_OUTPUTS {
                "chorda_source.mixer.model_main"
            } else {
                "chorda_source.mixer.nothing"
            };
            app.set_status(t(k));
        }
    }
}
