//! The kits list (ADR 0124 §3, ADR 0125 §4): *Modelled* (Cantus's tables: instant, no
//! download), *Recorded* (the manifest's Tympanum packs: each with its size and *Get*, its
//! progress and cancel while it downloads, *On disk* once there; a pack *from the author's
//! site* says so, a *Manual* one opens the author's page), *Your own kits* (installed from a
//! file). A click on a kit that is there plays it: one command (`kit`), one undo step.

use super::iu::{self, words};
use super::Look;
use crate::app::CantusApp;
use crate::i18n::{t, tf};
use crate::kit::color::mix;
use crate::kit::glyph_browser::Glyph as Gb;
use crate::kit::text::Src;
use crate::kit::{with_brush, Fill, Style};
use crate::views::mixer::look::{Pal, W};
use cantus_content::manifest::{PackMode, Plays};
use cantus_content::{Pack, Phase, Status};
use cantus_core::inst::tympanum::KitRef;
use cantus_core::inst::tympanum_kit::{modelled_kits, KitDef};
use cantus_core::{NativeSpec, TrackId};
use egui::{pos2, vec2, Align2, Rect, Sense};

const ROW: f32 = 40.0;
const RECORDED_ROW: f32 = 46.0;
const SECTION: f32 = 28.0;

/// One recorded kit the list offers.
struct Recorded {
    pack: String,
    kit: String,
    name: String,
    about: String,
    size: u64,
    mode: PackMode,
    author: String,
    page: Option<String>,
    status: Status,
    /// Installed from a file (a user pack): listed under *Your own kits*.
    own: bool, // r40:chorda-src
}

fn recorded(app: &mut CantusApp) -> Vec<Recorded> {
    let lang = crate::i18n::locale().code();
    let m = app.samples.content();
    let packs: Vec<Pack> = m.manifest().packs.clone();
    // r40:chorda-src — kits installed from a file (user packs), after the curated ones.
    let own: Vec<Recorded> = m
        .instruments()
        .into_iter()
        .filter(|i| i.instrument.plays == Plays::Tympanum && m.manifest().pack(&i.pack).is_none())
        .map(|i| Recorded {
            status: m.status(&i.pack),
            pack: i.pack.clone(),
            kit: i.instrument.id.clone(),
            name: i.instrument.name.get(lang).to_string(),
            about: t("chorda_source.yours"),
            size: 0,
            mode: PackMode::Pack,
            author: i.author.clone(),
            page: None,
            own: true,
        })
        .collect();
    packs
        .iter()
        .flat_map(|p| {
            let status = m.status(&p.id);
            p.instruments
                .iter()
                .filter(|i| i.plays == Plays::Tympanum)
                .map(|i| Recorded {
                    pack: p.id.clone(),
                    kit: i.id.clone(),
                    name: i.name.get(lang).to_string(),
                    about: format!("{} \u{b7} {}", p.description.get(lang), p.licence),
                    size: p.download_size().max(p.installed_size),
                    mode: p.mode,
                    author: p
                        .credits
                        .first()
                        .map(|c| c.name.clone())
                        .unwrap_or_default(),
                    page: p.author_page.clone(),
                    status: status.clone(),
                    own: false,
                })
                .collect::<Vec<_>>()
        })
        .chain(own)
        .fold(Vec::new(), |mut v: Vec<Recorded>, k| {
            // One row a kit: a pack listed twice (the bundled manifest and a local copy)
            // never shows its kit twice, nor clashes its rows' ids.
            if !v.iter().any(|x| x.pack == k.pack && x.kit == k.kit) {
                v.push(k);
            }
            v
        })
}

/// "2.8 GB", "620 MB".
pub fn size_text(bytes: u64) -> String {
    let b = bytes as f64;
    if b >= 1e9 {
        format!("{:.1} GB", b / 1e9)
    } else if bytes == 0 {
        "0 MB".to_string()
    } else {
        format!("{:.0} MB", (b / 1e6).max(1.0))
    }
}

/// The list in `r`.
#[allow(clippy::too_many_arguments)]
pub fn list(
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
    let pad = s.s(12.0);
    let head = Rect::from_min_size(
        pos2(r.left() + pad, r.top() + s.s(8.0)),
        vec2(r.width() - 2.0 * pad, s.s(28.0)),
    );
    with_brush(ui.painter(), |b| {
        words(
            b,
            s,
            &t("tympanum.kits"),
            head,
            Align2::LEFT_CENTER,
            14.0,
            W::SemiBold,
            p.ink1,
        );
    });
    let sr = Rect::from_min_size(
        pos2(head.right() - s.s(28.0), head.top()),
        vec2(s.s(28.0), s.s(28.0)),
    );
    let resp = iu::button(
        ui,
        s,
        sr,
        ui.id().with(("tymp-kits-search", track.0)),
        "",
        iu::Lead::Glyph(Gb::Search),
        false,
        true,
    );
    let _ = iu::hint(resp, || t("tympanum.kits.search"));
    // The filter.
    let fr = Rect::from_min_size(
        pos2(head.left(), head.bottom() + s.s(6.0)),
        vec2(head.width(), s.s(24.0)),
    );
    let labels = [
        Src::Key("tympanum.kits.all"),
        Src::Key("tympanum.kits.recorded"),
        Src::Key("tympanum.kits.modelled"),
    ];
    let (_, chosen) = crate::kit::widgets::Segmented::new(usize::from(lk.filter.min(2)), &labels)
        .id(ui.id().with(("tymp-kits-filter", track.0)))
        .name(&t("tympanum.kits"))
        .style(*s)
        .show_at(ui, fr);
    if let Some(i) = chosen {
        lk.filter = i as u8;
        lk.list_scroll = 0.0;
    }
    // The foot: what is on disk, and the Content window.
    let foot = Rect::from_min_max(
        pos2(r.left() + pad, r.bottom() - s.s(30.0)),
        pos2(r.right() - pad, r.bottom() - s.s(6.0)),
    );
    let rec = recorded(app);
    let on_disk: u64 = {
        let m = app.samples.content();
        let ids: Vec<&str> = rec
            .iter()
            .filter(|k| matches!(k.status, Status::Installed { .. } | Status::Update { .. }))
            .map(|k| k.pack.as_str())
            .collect();
        m.manifest()
            .packs
            .iter()
            .filter(|p| ids.contains(&p.id.as_str()))
            .map(|p| p.installed_size)
            .sum()
    };
    let manage = t("tympanum.kits.manage");
    let mw = iu::width(ui, s, &manage, 12.0, W::Medium) + s.s(4.0);
    let mr = Rect::from_min_max(pos2(foot.right() - mw, foot.top()), foot.max);
    let mresp = ui.interact(
        mr,
        ui.id().with(("tymp-kits-manage", track.0)),
        Sense::click(),
    );
    let mresp = iu::hint(mresp, || t("tympanum.kits.tip_manage"));
    if mresp.clicked() {
        app.samples.window.open = true;
    }
    with_brush(ui.painter(), |b| {
        b.hline(
            r.left() + pad,
            r.right() - pad,
            b.snap(foot.top() - s.s(4.0)),
            b.px(),
            p.line,
        );
        let lw = foot.width() - mw - s.s(8.0);
        let lr = Rect::from_min_size(foot.min, vec2(lw, foot.height()));
        words(
            b,
            s,
            &tf("tympanum.kits.on_disk", &[("size", &size_text(on_disk))]),
            lr,
            Align2::LEFT_CENTER,
            11.0,
            W::Regular,
            p.ink3,
        );
        let ink = if mresp.hovered() { p.ink1 } else { p.ink2 };
        words(
            b,
            s,
            &manage,
            mr,
            Align2::RIGHT_CENTER,
            12.0,
            W::Medium,
            ink,
        );
    });
    // The list, scrolling between the filter and the foot.
    let area = Rect::from_min_max(
        pos2(r.left() + s.s(4.0), fr.bottom() + s.s(8.0)),
        pos2(r.right() - s.s(4.0), foot.top() - s.s(8.0)),
    );
    let current = KitRef::of(spec);
    let _ = def;
    let mut child = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(area)
            .id_salt(("tymp-kits-list", track.0)),
    );
    child.set_clip_rect(area.intersect(ui.clip_rect()));
    let over = ui
        .ctx()
        .pointer_hover_pos()
        .is_some_and(|q| area.contains(q));
    if over {
        lk.list_scroll -= ui.input(|i| i.smooth_scroll_delta.y);
    }
    let mut y = area.top() - lk.list_scroll;
    let x0 = area.left();
    let w = area.width();
    if lk.filter != 1 {
        y = section(&child, s, x0, y, w, Section::Modelled);
        for mk in modelled_kits() {
            let kit = KitRef::Modelled(mk.id.to_string());
            let lit = current == kit;
            // Its style in words, its counts from the kit itself (never a stale figure).
            let about = match cantus_core::inst::tympanum_kit::modelled_kit(mk.id) {
                Some(def) => tf(
                    "tympanum.kits.modelled_about",
                    &[
                        ("style", &t(&format!("{}.about", mk.label_key))),
                        ("pieces", &def.pieces.len()),
                        ("mics", &def.mics.len()),
                    ],
                ),
                None => t(&format!("{}.about", mk.label_key)),
            };
            let row = Rect::from_min_size(pos2(x0, y), vec2(w, s.s(ROW)));
            let resp = kit_row(
                &child,
                s,
                row,
                ("m", mk.id),
                &t(mk.label_key),
                &about,
                lit,
                None,
            );
            let resp = iu::hint(resp, || t("tympanum.kits.tip_modelled"));
            if resp.clicked() && !lit {
                super::kit_change::change_kit(app, track, &kit);
            }
            y += s.s(ROW);
            // r40:drums-hyb — the kit it plays: Generated · Hybrid.
            if lit && cantus_core::inst::tympanum::voice::hybrid_pack(mk.id).is_some() {
                let vr = Rect::from_min_size(pos2(x0, y), vec2(w, s.s(super::voice::ROW)));
                super::voice::row(app, &child, track, spec, mk.id, s, vr);
                y += s.s(super::voice::ROW);
            }
        }
        y += s.s(8.0);
    }
    if lk.filter != 2 {
        y = section(&child, s, x0, y, w, Section::Recorded);
        for k in rec.iter().filter(|k| !k.own) {
            let kit = KitRef::Pack {
                pack: k.pack.clone(),
                kit: k.kit.clone(),
            };
            let lit = current == kit;
            let row = Rect::from_min_size(pos2(x0, y), vec2(w, s.s(RECORDED_ROW)));
            recorded_row(app, &child, track, s, row, k, lit, &kit);
            y += s.s(RECORDED_ROW);
        }
        y += s.s(8.0);
        y = section(&child, s, x0, y, w, Section::Own);
        // r40:chorda-src — the kits installed from a file.
        for k in rec.iter().filter(|k| k.own) {
            let kit = KitRef::Pack {
                pack: k.pack.clone(),
                kit: k.kit.clone(),
            };
            let lit = current == kit;
            let row = Rect::from_min_size(pos2(x0, y), vec2(w, s.s(RECORDED_ROW)));
            recorded_row(app, &child, track, s, row, k, lit, &kit);
            y += s.s(RECORDED_ROW);
        }
        let box_r = Rect::from_min_size(
            pos2(x0 + s.s(8.0), y + s.s(2.0)),
            vec2(w - s.s(16.0), s.s(48.0)),
        );
        // r40:fix-f — while a kit installs from a file: its progress, a click cancels.
        if let Some((job, ph)) =
            super::super::chorda_source::install_under_way(app, Plays::Tympanum)
        {
            super::super::chorda_source::install_progress_box(
                app,
                &child,
                s,
                box_r,
                &job,
                ph.as_ref(),
            );
        } else if install_box(&child, s, box_r, track).clicked() {
            install_from_file(app, track);
        }
        y += s.s(56.0);
    }
    let content = y + lk.list_scroll - area.top();
    lk.list_scroll = lk
        .list_scroll
        .clamp(0.0, (content - area.height()).max(0.0));
}

#[derive(Copy, Clone, PartialEq)]
enum Section {
    Modelled,
    Recorded,
    Own,
}

/// A section's head: its glyph, its name, its note at the right. Returns the y under it.
fn section(ui: &egui::Ui, s: &Style, x: f32, y: f32, w: f32, which: Section) -> f32 {
    let p = Pal::of(s);
    let r = Rect::from_min_size(pos2(x + s.s(8.0), y), vec2(w - s.s(16.0), s.s(SECTION)));
    let (name, note, glyph) = match which {
        Section::Modelled => (
            "tympanum.kits.modelled",
            "tympanum.kits.modelled_note",
            Gb::Instrument,
        ),
        Section::Recorded => (
            "tympanum.kits.recorded",
            "tympanum.kits.recorded_note",
            Gb::Audio,
        ),
        Section::Own => ("tympanum.kits.own", "", Gb::File),
    };
    with_brush(ui.painter(), |b| {
        let g = Rect::from_center_size(
            pos2(r.left() + s.s(6.0), r.center().y),
            vec2(s.s(12.0), s.s(12.0)),
        );
        let _ = crate::kit::glyph_browser::paint(b, g, glyph, p.ink2);
        let tr = Rect::from_min_max(pos2(g.right() + s.s(6.0), r.top()), r.max);
        let nw = words(
            b,
            s,
            &t(name),
            tr,
            Align2::LEFT_CENTER,
            12.0,
            W::SemiBold,
            p.ink1,
        );
        if !note.is_empty() {
            let nr = Rect::from_min_max(pos2(tr.left() + nw + s.s(8.0), r.top()), r.max);
            iu::note(
                b,
                s,
                &t(note),
                nr,
                Align2::RIGHT_CENTER,
                10.0,
                W::Regular,
                p.ink3,
            );
        }
    });
    y + s.s(SECTION)
}

/// A kit's row: its name and its line; lit, the accent's bar at its left on a raised
/// ground. `right` keeps that much at the right for a control.
#[allow(clippy::too_many_arguments)]
fn kit_row(
    ui: &egui::Ui,
    s: &Style,
    r: Rect,
    salt: impl std::hash::Hash + std::fmt::Debug,
    name: &str,
    about: &str,
    lit: bool,
    right: Option<f32>,
) -> egui::Response {
    let p = Pal::of(s);
    let resp = ui.interact(r, ui.id().with(("tymp-kit-row", salt)), Sense::click());
    let keep = right.unwrap_or(0.0);
    with_brush(ui.painter(), |b| {
        if lit || resp.hovered() {
            let ground = if lit {
                mix(p.surface, p.ink1, 0.06)
            } else {
                mix(p.surface, p.ink1, 0.03)
            };
            b.rrect(
                b.snap_rect(r.shrink2(vec2(s.s(2.0), s.s(1.0)))),
                s.s(4.0),
                Fill::Solid(ground),
            );
        }
        if lit {
            let bar = Rect::from_min_size(
                pos2(r.left() + s.s(2.0), r.top() + s.s(6.0)),
                vec2(s.s(2.5), r.height() - s.s(12.0)),
            );
            b.rrect(b.snap_rect(bar), s.s(1.0), Fill::Solid(p.accent));
        }
        let tx = r.left() + s.s(14.0);
        let tw = (r.right() - s.s(8.0) - keep - tx).max(s.s(20.0));
        let nr = Rect::from_min_size(pos2(tx, r.top() + s.s(5.0)), vec2(tw, s.s(16.0)));
        let weight = if lit { W::SemiBold } else { W::Medium };
        // A pack's long name ("Gogodze Phu Vol. II") steps down before it is ever cut.
        let size = [12.0, 11.0, 10.0]
            .into_iter()
            .find(|z| crate::views::mixer::look::width_of(b.ctx(), s, name, *z, weight) <= tw)
            .unwrap_or(10.0);
        words(b, s, name, nr, Align2::LEFT_CENTER, size, weight, p.ink1);
        let ar = Rect::from_min_size(
            pos2(tx, nr.bottom() + s.s(1.0)),
            vec2(r.right() - s.s(8.0) - tx, s.s(14.0)),
        );
        words(
            b,
            s,
            about,
            ar,
            Align2::LEFT_CENTER,
            10.0,
            W::Regular,
            if lit { p.ink2 } else { p.ink3 },
        );
    });
    resp
}

/// A recorded kit's row: its name and line, and at the right what can be done — *Get · size*,
/// the progress with *Cancel*, *On disk*, or (Manual) *Open the page*.
#[allow(clippy::too_many_arguments)]
fn recorded_row(
    app: &mut CantusApp,
    ui: &egui::Ui,
    track: TrackId,
    s: &Style,
    r: Rect,
    k: &Recorded,
    lit: bool,
    kit: &KitRef,
) {
    let p = Pal::of(s);
    let busy = match &k.status {
        Status::Busy(ph) => Some(ph.clone()),
        _ => None,
    };
    let there = matches!(k.status, Status::Installed { .. } | Status::Update { .. });
    // The control at the right, and the room it takes.
    let ctl_h = s.s(24.0);
    let (ctl_label, lead) = if busy.is_some() {
        (String::new(), iu::Lead::Glyph(Gb::Clear))
    } else if there {
        (t("tympanum.kits.downloaded"), iu::Lead::Glyph(Gb::Check))
    } else if k.mode == PackMode::Manual {
        (t("tympanum.kits.open_page"), iu::Lead::None)
    } else {
        (
            tf("tympanum.kits.get", &[("size", &size_text(k.size))]),
            iu::Lead::Glyph(Gb::Download),
        )
    };
    let pad_for = |lead: iu::Lead| {
        s.s(if matches!(lead, iu::Lead::None) {
            16.0
        } else {
            36.0
        })
    };
    let mut cw = if busy.is_some() {
        ctl_h
    } else {
        iu::width(ui, s, &ctl_label, 12.0, W::Medium) + pad_for(lead)
    };
    let name_room = |cw: f32| r.width() - s.s(22.0) - cw - s.s(6.0);
    let mut ctl_label = ctl_label;
    if busy.is_none()
        && !there
        && k.mode != PackMode::Manual
        && iu::width(ui, s, &k.name, 10.0, W::Medium) > name_room(cw)
    {
        ctl_label = size_text(k.size);
        cw = iu::width(ui, s, &ctl_label, 12.0, W::Medium) + pad_for(lead);
    }
    // Still too long ("CrocellKit, every microphone"): what follows its comma leads the
    // line under it, the name keeps its head — every word shown, none cut.
    let (name, tail) = match k.name.split_once(", ") {
        Some((head, tail)) if iu::width(ui, s, &k.name, 10.0, W::Medium) > name_room(cw) => {
            (head.to_string(), Some(tail.to_string()))
        }
        _ => (k.name.clone(), None),
    };
    // The line under the name: the pack's words when they fit beside nothing, else its
    // licence; a pack from the author's site says so.
    let line_w = r.width() - s.s(22.0);
    let about = if k.mode == PackMode::AuthorSite {
        tf("tympanum.kits.from_author", &[("author", &k.author)])
    } else if iu::width(ui, s, &k.about, 10.0, W::Regular) <= line_w {
        k.about.clone()
    } else {
        k.about.rsplit(" \u{b7} ").next().unwrap_or("").to_string()
    };
    let about = match tail {
        Some(tail) => {
            let both = format!("{tail} \u{b7} {about}");
            if about.is_empty() || iu::width(ui, s, &both, 10.0, W::Regular) > line_w {
                tail
            } else {
                both
            }
        }
        None => about,
    };
    let resp = kit_row(
        ui,
        s,
        r,
        ("r", &k.pack, &k.kit),
        &name,
        if busy.is_some() { "" } else { &about },
        lit,
        Some(cw + s.s(6.0)),
    );
    let tip = if k.mode == PackMode::Manual {
        t("tympanum.kits.manual")
    } else {
        t("tympanum.kits.tip_recorded")
    };
    let resp = iu::hint(resp, || tip);
    if resp.clicked() && there && !lit {
        super::kit_change::change_kit(app, track, kit);
    }
    let cr = Rect::from_min_size(
        pos2(r.right() - s.s(8.0) - cw, r.top() + s.s(5.0)),
        vec2(cw, ctl_h),
    );
    let id = ui.id().with(("tymp-kit-get", &k.pack, &k.kit));
    let cresp = iu::button(ui, s, cr, id, &ctl_label, lead, false, !there);
    // The help line says what the button does (round 40's close: the sweep found none).
    let cresp = if busy.is_some() {
        cresp
    } else if there {
        iu::hint(cresp, || t("tympanum.kits.tip_on_disk"))
    } else if k.mode == PackMode::Manual {
        iu::hint(cresp, || t("tympanum.kits.manual"))
    } else {
        iu::hint(cresp, || {
            tf(
                "tympanum.kits.tip_get",
                &[("name", &k.name), ("size", &size_text(k.size))],
            )
        })
    };
    if busy.is_some() {
        let cresp = iu::hint(cresp, || t("tympanum.kits.cancel"));
        if cresp.clicked() {
            app.samples.content().cancel(&k.pack);
        }
    } else if cresp.clicked() && !there {
        if k.mode == PackMode::Manual {
            if let Some(url) = &k.page {
                crate::audio_setup::drivers::open_url(url);
            }
        } else {
            app.samples.content().install(&k.pack);
        }
    }
    // While it downloads: the bar and how far, under the name.
    if let Some(ph) = busy {
        let f = crate::views::samples_content::fraction(ph.as_ref());
        let (done, total) = match ph {
            Some(Phase::Downloading { done, total }) => (done, total),
            _ => ((k.size as f64 * f64::from(f)) as u64, k.size),
        };
        with_brush(ui.painter(), |b| {
            let tx = r.left() + s.s(14.0);
            let bar = Rect::from_min_size(
                pos2(tx, r.top() + s.s(25.0)),
                vec2(r.width() * 0.48, s.s(3.0)),
            );
            b.rrect(b.snap_rect(bar), s.s(1.5), Fill::Solid(p.bar_track));
            let fill =
                Rect::from_min_size(bar.min, vec2(bar.width() * f.clamp(0.0, 1.0), bar.height()));
            b.rrect(b.snap_rect(fill), s.s(1.5), Fill::Solid(p.accent));
            let pct = format!("{:.0}", f * 100.0);
            let words_ = tf(
                "tympanum.kits.progress",
                &[
                    ("pct", &pct),
                    ("done", &size_text(done)),
                    ("total", &size_text(total)),
                ],
            );
            let wr = Rect::from_min_size(
                pos2(tx, bar.bottom() + s.s(2.0)),
                vec2(r.right() - s.s(8.0) - tx, s.s(13.0)),
            );
            words(
                b,
                s,
                &words_,
                wr,
                Align2::LEFT_CENTER,
                10.0,
                W::Regular,
                p.ink3,
            );
        });
    }
}

/// *Install from a file…*: a dashed box with the formats it takes.
fn install_box(ui: &egui::Ui, s: &Style, r: Rect, track: TrackId) -> egui::Response {
    let p = Pal::of(s);
    let resp = ui.interact(
        r,
        ui.id().with(("tymp-kits-install", track.0)),
        Sense::click(),
    );
    let resp = iu::hint(resp, || t("tympanum.kits.tip_install"));
    with_brush(ui.painter(), |b| {
        let rr = b.snap_rect(r);
        let ink = if resp.hovered() { p.ink2 } else { p.ink4 };
        let pts = [
            rr.left_top(),
            rr.right_top(),
            rr.right_bottom(),
            rr.left_bottom(),
            rr.left_top(),
        ];
        b.dashed(&pts, b.px(), ink, s.s(4.0), s.s(3.0));
        let tx = rr.left() + s.s(12.0);
        let plus = pos2(tx + s.s(3.0), rr.top() + s.s(15.0));
        b.hline(
            plus.x - s.s(4.0),
            plus.x + s.s(4.0),
            plus.y,
            s.s(1.3),
            p.ink2,
        );
        b.vline(
            plus.x,
            plus.y - s.s(4.0),
            plus.y + s.s(4.0),
            s.s(1.3),
            p.ink2,
        );
        let nr = Rect::from_min_max(
            pos2(tx + s.s(14.0), rr.top() + s.s(6.0)),
            pos2(rr.right() - s.s(8.0), rr.top() + s.s(24.0)),
        );
        words(
            b,
            s,
            &t("tympanum.kits.install_file"),
            nr,
            Align2::LEFT_CENTER,
            12.0,
            W::Medium,
            p.ink1,
        );
        let fr = Rect::from_min_max(
            pos2(tx, nr.bottom()),
            pos2(rr.right() - s.s(8.0), rr.bottom() - s.s(4.0)),
        );
        words(
            b,
            s,
            &t("tympanum.kits.install_formats"),
            fr,
            Align2::LEFT_CENTER,
            10.0,
            W::Regular,
            p.ink3,
        );
    });
    resp
}

/// Pick a kit on disk. A pack the manifest knows (a *Manual* pack's download, found by its
/// size and hash) installs as that pack; any other kit (an SFZ, a DrumGizmo kit, an archive
/// of one) becomes one of the user's own packs and plays at once — one command (`kit`)
/// (`cantus_content::user_pack` through `chorda_source::install_any`; r40:chorda-src).
fn install_from_file(app: &mut CantusApp, track: TrackId) {
    let Some(path) = rfd::FileDialog::new()
        .add_filter("Kits", &["zip", "xml", "sfz", "tar", "gz", "tgz"])
        .pick_file()
    else {
        return;
    };
    install_kit_path(app, track, &path);
}

/// [`install_from_file`] for a path already chosen (a drop; the tests). A user's kit
/// installs in the background (r40:fix-f); the track takes it once it is in.
pub fn install_kit_path(app: &mut CantusApp, track: TrackId, path: &std::path::Path) {
    use super::super::chorda_source::{install_any, Installed};
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_default();
    match install_any(app, path, Plays::Tympanum, track) {
        Ok(Installed::Pack(_)) => {}
        Ok(Installed::Started(_)) => {
            app.set_status(tf("inst.install.installing", &[("name", &name)]));
        }
        Err(why) => {
            // A file Cantus does not know says so; anything else says why.
            let unknown = cantus_content::user_pack::UserPackError::Unknown.to_string();
            if why == unknown {
                app.set_error(tf("tympanum.kits.unknown_file", &[("name", &name)]));
            } else {
                app.set_error(tf(
                    "tympanum.kits.failed",
                    &[("name", &name), ("why", &why)],
                ));
            }
        }
    }
}
