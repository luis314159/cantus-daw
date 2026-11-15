//! Chorda's source list (round 40 phase 3, ADR 0125; the approved design's *Chorda · Play*
//! left panel), drawn in Slate's hand with the instrument views' shared parts
//! (`inst_mixer::ui`, as Tympanum's kits list): *All · Recorded · Modelled*; **Modelled** —
//! Chorda 2, and Chorda 1 under a folded *Previous models*; **Recorded** — the curated
//! pianos (the Salamander first), each *Get · size* until it is on disk (its progress and
//! *Cancel* while it comes), bitKlavier once for its nine images (*Choose mics…*), the
//! *Manual* pianos opening their authors' pages; **Your own pianos** — what was installed
//! from a file, and *Install from a file…*; at the foot, the pianos on disk and *Manage…*.
//! A click on a piano that is there switches to it (one undo step).

use super::read::{self, PianoEntry};
use crate::app::CantusApp;
use crate::i18n::{t, tf};
use crate::kit::color::mix;
use crate::kit::glyph_browser::Glyph as Gb;
use crate::kit::text::Src;
use crate::kit::{with_brush, Fill, Style};
use crate::views::inst::inst_mixer::ui::{self as iu, words};
use crate::views::mixer::look::{Pal, W};
use cantus_content::{Phase, Status};
use cantus_core::inst::chorda_source::{current_choice, SourceChoice};
use cantus_core::{NativeSpec, TrackId};
use egui::{pos2, vec2, Align2, Rect, Sense};

/// The panel's width at 100 %.
pub const COLUMN_W: f32 = 228.0;
/// The least the Play view's side column takes beside the scene, at 100 % (its pedal
/// words whole in every language).
pub const SIDE_MIN: f32 = 280.0;
const ROW: f32 = 40.0;
const PREVIOUS: f32 = 28.0;
const SECTION: f32 = 28.0;
const INSTALL: f32 = 48.0;

/// Pianos one gets from their author by hand (ADR 0125 §4, *Manual*): Cantus opens the
/// page; the file comes back through *Install from a file…*. (name, what, page)
pub const MANUAL: [(&str, &str, &str); 2] = [
    (
        "Piano in 162",
        "Ivy Audio · Steinway B",
        "https://ivyaudio.com/Piano-in-162",
    ),
    (
        "Helsinki 1967 Pro",
        "Pianobook · upright",
        "https://www.pianobook.co.uk/packs/helsinki-1967-pro",
    ),
];

/// "742 MB", "2.1 GB".
pub fn size_words(bytes: u64) -> String {
    if bytes >= 1_000_000_000 {
        format!("{:.1} GB", bytes as f64 / 1e9)
    } else {
        format!("{} MB", (bytes as f64 / 1e6).round().max(1.0))
    }
}

/// What the list remembers: its filter (0 all, 1 recorded, 2 modelled) and its scroll.
#[derive(Clone, Copy, Debug, Default)]
struct ListLook {
    filter: u8,
    scroll: f32,
}

fn look_id(track: TrackId) -> egui::Id {
    egui::Id::new(("chorda-src-list", track.0))
}

/// One row of the list.
#[derive(Clone, Debug)]
enum Row {
    Section(Section),
    Model(u8),
    Previous,
    Piano(Box<PianoEntry>),
    Manual(usize),
    InstallFile,
}

#[derive(Copy, Clone, Debug, PartialEq)]
enum Section {
    Modelled,
    Recorded,
    Own,
}

impl Row {
    fn height(&self) -> f32 {
        match self {
            Row::Section(_) => SECTION,
            Row::Previous => PREVIOUS,
            Row::InstallFile => INSTALL + 8.0,
            _ => ROW,
        }
    }
}

fn rows(app: &mut CantusApp, filter: u8) -> Vec<Row> {
    let mut out = Vec::new();
    if filter != 1 {
        out.push(Row::Section(Section::Modelled));
        out.push(Row::Model(2));
        out.push(Row::Previous);
        if app.inst.chorda_source_open_previous {
            out.push(Row::Model(1));
        }
    }
    if filter != 2 {
        out.push(Row::Section(Section::Recorded));
        let pianos = read::pianos(app.samples.content());
        let (curated, own): (Vec<_>, Vec<_>) = pianos.into_iter().partition(|p| {
            p.images
                .iter()
                .all(|i| !i.pack.starts_with(cantus_content::user_pack::USER_PREFIX))
        });
        out.extend(curated.into_iter().map(|p| Row::Piano(Box::new(p))));
        out.extend((0..MANUAL.len()).map(Row::Manual));
        out.push(Row::Section(Section::Own));
        out.extend(own.into_iter().map(|p| Row::Piano(Box::new(p))));
        out.push(Row::InstallFile);
    }
    out
}

/// The source panel at the left of Play: drawn into `area`'s left edge; returns the rest.
pub fn column(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    track: TrackId,
    spec: &NativeSpec,
    s: &Style,
    area: Rect,
) -> Rect {
    let p = Pal::of(s);
    let g = s.s(iu::GAP);
    let w = s.s(COLUMN_W).min(area.width() * 0.4);
    let r = Rect::from_min_size(area.min, vec2(w, area.height()));
    let rest = Rect::from_min_max(pos2(r.right() + g, area.top()), area.max);
    let mut lk = ui
        .ctx()
        .data(|d| d.get_temp::<ListLook>(look_id(track)))
        .unwrap_or_default();
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
            &t("chorda_source.title"),
            head,
            Align2::LEFT_CENTER,
            14.0,
            W::SemiBold,
            p.ink1,
        );
    });
    let fr = Rect::from_min_size(
        pos2(head.left(), head.bottom() + s.s(6.0)),
        vec2(head.width(), s.s(24.0)),
    );
    let labels = [
        Src::Key("chorda_source.filter_all"),
        Src::Key("chorda_source.recorded"),
        Src::Key("chorda_source.modelled"),
    ];
    let (_, chosen) = crate::kit::widgets::Segmented::new(usize::from(lk.filter.min(2)), &labels)
        .id(ui.id().with(("chorda-src-filter", track.0)))
        .name(&t("chorda_source.title"))
        .style(*s)
        .show_at(ui, fr);
    if let Some(i) = chosen {
        lk.filter = i as u8;
        lk.scroll = 0.0;
    }
    let foot = Rect::from_min_max(
        pos2(r.left() + pad, r.bottom() - s.s(30.0)),
        pos2(r.right() - pad, r.bottom() - s.s(6.0)),
    );
    foot_row(app, ui, s, foot, r, track);
    // The list, scrolling between the filter and the foot.
    let list = Rect::from_min_max(
        pos2(r.left() + s.s(4.0), fr.bottom() + s.s(8.0)),
        pos2(r.right() - s.s(4.0), foot.top() - s.s(8.0)),
    );
    let mut child = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(list)
            .id_salt(("chorda-src-list", track.0)),
    );
    child.set_clip_rect(list.intersect(ui.clip_rect()));
    let over = ui
        .ctx()
        .pointer_hover_pos()
        .is_some_and(|q| list.contains(q));
    if over {
        lk.scroll -= ui.input(|i| i.smooth_scroll_delta.y);
    }
    let choice = current_choice(spec);
    let mut y = list.top() - lk.scroll;
    for row in rows(app, lk.filter) {
        let h = s.s(row.height());
        let rr = Rect::from_min_size(pos2(list.left(), y), vec2(list.width(), h));
        if rr.bottom() > list.top() && rr.top() < list.bottom() {
            draw_row(app, &child, track, s, rr, &row, &choice);
        }
        y += h;
    }
    let content = y + lk.scroll - list.top();
    lk.scroll = lk.scroll.clamp(0.0, (content - list.height()).max(0.0));
    ui.ctx().data_mut(|d| d.insert_temp(look_id(track), lk));
    rest
}

/// The foot: the pianos on disk, and the Content window.
fn foot_row(app: &mut CantusApp, ui: &egui::Ui, s: &Style, foot: Rect, r: Rect, track: TrackId) {
    let p = Pal::of(s);
    let m = app.samples.content();
    let bytes: u64 = read::pianos(m)
        .iter()
        .flat_map(|p| p.images.iter())
        .filter(|i| i.installed.is_some())
        .filter_map(|i| {
            m.installed()
                .iter()
                .find(|x| x.id == i.pack)
                .map(|x| x.bytes)
        })
        .sum();
    let manage = t("chorda_source.manage");
    let mw = iu::width(ui, s, &manage, 12.0, W::Medium) + s.s(4.0);
    let mr = Rect::from_min_max(pos2(foot.right() - mw, foot.top()), foot.max);
    let mresp = ui.interact(
        mr,
        ui.id().with(("chorda-src-manage", track.0)),
        Sense::click(),
    );
    let mresp = iu::hint(mresp, || t("chorda_source.tip.manage"));
    if mresp.clicked() {
        app.samples.window.open = true;
    }
    with_brush(ui.painter(), |b| {
        b.hline(
            r.left() + s.s(12.0),
            r.right() - s.s(12.0),
            b.snap(foot.top() - s.s(4.0)),
            b.px(),
            p.line,
        );
        let lr = Rect::from_min_size(foot.min, vec2(foot.width() - mw - s.s(8.0), foot.height()));
        words(
            b,
            s,
            &tf(
                "chorda_source.on_disk_total",
                &[("size", &size_words(bytes))],
            ),
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
}

fn draw_row(
    app: &mut CantusApp,
    ui: &egui::Ui,
    track: TrackId,
    s: &Style,
    r: Rect,
    row: &Row,
    choice: &SourceChoice,
) {
    match row {
        Row::Section(which) => section(ui, s, r, *which),
        Row::Model(v) => {
            let on = *choice == SourceChoice::Model(*v);
            let sub = if *v == 2 {
                t("chorda_source.model2_sub")
            } else {
                t("chorda_source.model1_sub")
            };
            let (resp, _) = item(ui, s, r, ("m", *v), &format!("Chorda {v}"), &sub, on, 0.0);
            let resp = iu::hint(resp, || t("chorda_source.tip.model"));
            if resp.clicked() && !on {
                app.chorda_switch(track, &SourceChoice::Model(*v));
            }
        }
        Row::Previous => previous(app, ui, s, r),
        Row::Piano(e) => piano(app, ui, track, s, r, e, choice),
        Row::Manual(i) => {
            let (name, what, page) = MANUAL[*i];
            let label = t("chorda_source.open_page");
            let cw = iu::width(ui, s, &label, 12.0, W::Medium) + s.s(16.0);
            let (resp, stacked) = item(ui, s, r, ("manual", *i), name, what, false, cw + s.s(6.0));
            let _ = iu::hint(resp, || t("chorda_source.tip.manual"));
            let cr = control_rect(s, r, cw, stacked);
            let id = ui.id().with(("chorda-src-page", *i));
            let cresp = iu::button(ui, s, cr, id, &label, iu::Lead::None, false, true);
            let cresp = iu::hint(cresp, || t("chorda_source.tip.manual"));
            if cresp.clicked() {
                crate::audio_setup::drivers::open_url(page);
                app.set_status(tf("chorda_source.manual_note", &[("name", &name)]));
            }
        }
        Row::InstallFile => {
            let br = Rect::from_min_size(
                pos2(r.left() + s.s(8.0), r.top() + s.s(4.0)),
                vec2(r.width() - s.s(16.0), s.s(INSTALL)),
            );
            // r40:fix-f — while a piano installs from a file: its progress, a click cancels.
            if let Some((job, ph)) =
                super::install_under_way(app, cantus_content::manifest::Plays::Chorda)
            {
                super::install_progress_box(app, ui, s, br, &job, ph.as_ref());
            } else if install_box(ui, s, br, track).clicked() {
                super::install_from_file(app, track);
            }
        }
    }
}

/// A section's head: its glyph, its name, its note at the right.
fn section(ui: &egui::Ui, s: &Style, r: Rect, which: Section) {
    let p = Pal::of(s);
    let r = r.shrink2(vec2(s.s(8.0), 0.0));
    let (name, note, glyph) = match which {
        Section::Modelled => (
            "chorda_source.modelled",
            "chorda_source.modelled_note",
            Gb::Instrument,
        ),
        Section::Recorded => (
            "chorda_source.recorded",
            "chorda_source.recorded_note",
            Gb::Audio,
        ),
        Section::Own => ("chorda_source.own", "", Gb::File),
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
}

/// A row: its name and its line (the line left out when it does not fit whole); lit, the
/// accent's bar at its left on a raised ground. `keep` is what the control at the right
/// takes: beside the name when the name fits there whole, else (`true` back) on the line
/// under it, the name then taking the row's width.
#[allow(clippy::too_many_arguments)]
fn item(
    ui: &egui::Ui,
    s: &Style,
    r: Rect,
    salt: impl std::hash::Hash + std::fmt::Debug,
    name: &str,
    about: &str,
    lit: bool,
    keep: f32,
) -> (egui::Response, bool) {
    let p = Pal::of(s);
    let resp = ui.interact(r, ui.id().with(("chorda-src-row", salt)), Sense::click());
    let tx = r.left() + s.s(14.0);
    let full = r.right() - s.s(8.0) - tx;
    let beside = (full - keep).max(s.s(20.0));
    let fits = |w: f32| {
        [12.0, 11.0]
            .into_iter()
            .find(|z| iu::width(ui, s, name, *z, W::SemiBold) <= w)
    };
    let (stacked, tw, size) = match fits(beside) {
        Some(z) => (false, beside, z),
        None => (
            keep > 0.0,
            full,
            [12.0, 11.0, 10.0]
                .into_iter()
                .find(|z| iu::width(ui, s, name, *z, W::SemiBold) <= full)
                .unwrap_or(10.0),
        ),
    };
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
        let nr = Rect::from_min_size(pos2(tx, r.top() + s.s(5.0)), vec2(tw, s.s(16.0)));
        words(
            b,
            s,
            name,
            nr,
            Align2::LEFT_CENTER,
            size,
            W::SemiBold,
            p.ink1,
        );
        // The line runs under the control too (beside it when the control moved down).
        let lw = if stacked { full - keep } else { full };
        let lr = Rect::from_min_size(pos2(tx, nr.bottom() + s.s(1.0)), vec2(lw, s.s(14.0)));
        iu::note(
            b,
            s,
            about,
            lr,
            Align2::LEFT_CENTER,
            10.0,
            W::Regular,
            p.ink3,
        );
    });
    (resp, stacked)
}

/// Where a row's control sits: at its right, on the name's line (or the line under it).
fn control_rect(s: &Style, r: Rect, w: f32, stacked: bool) -> Rect {
    if stacked {
        Rect::from_min_size(
            pos2(r.right() - s.s(8.0) - w, r.top() + s.s(21.0)),
            vec2(w, s.s(17.0)),
        )
    } else {
        Rect::from_min_size(
            pos2(r.right() - s.s(8.0) - w, r.top() + s.s(4.0)),
            vec2(w, s.s(22.0)),
        )
    }
}

/// *Previous models*: a chevron, the words, the versions under it at the right.
fn previous(app: &mut CantusApp, ui: &egui::Ui, s: &Style, r: Rect) {
    let p = Pal::of(s);
    let open = app.inst.chorda_source_open_previous;
    let resp = ui.interact(r, ui.id().with("chorda-src-previous"), Sense::click());
    let resp = iu::hint(resp, || t("chorda_source.tip.previous"));
    if resp.clicked() {
        app.inst.chorda_source_open_previous = !open;
    }
    with_brush(ui.painter(), |b| {
        let ink = if resp.hovered() { p.ink1 } else { p.ink2 };
        let cx = r.left() + s.s(18.0);
        let cy = r.center().y;
        if open {
            crate::kit::well::chevron(b, pos2(cx, cy), s.s(7.0), s.s(4.0), false, s.s(1.3), ink);
        } else {
            let (h, d) = (s.s(3.5), s.s(2.0));
            b.polyline(
                &[pos2(cx - d, cy - h), pos2(cx + d, cy), pos2(cx - d, cy + h)],
                s.s(1.3),
                ink,
            );
        }
        let tr = Rect::from_min_max(pos2(cx + s.s(10.0), r.top()), r.max - vec2(s.s(8.0), 0.0));
        let nw = words(
            b,
            s,
            &t("chorda_source.previous"),
            tr,
            Align2::LEFT_CENTER,
            12.0,
            W::Regular,
            ink,
        );
        let nr = Rect::from_min_max(pos2(tr.left() + nw + s.s(8.0), r.top()), tr.max);
        iu::note(
            b,
            s,
            "Chorda 1",
            nr,
            Align2::RIGHT_CENTER,
            10.0,
            W::Regular,
            p.ink3,
        );
    });
}

/// A licence as people say it (`CC BY 4.0`), a user's own pack as *yours*.
pub fn licence_word(spdx: &str) -> String {
    match spdx {
        "CC0-1.0" => "CC0".into(),
        "CC-BY-3.0" => "CC BY 3.0".into(),
        "CC-BY-4.0" => "CC BY 4.0".into(),
        x if x == cantus_content::user_pack::USER_LICENCE => t("chorda_source.yours"),
        x => x.to_string(),
    }
}

/// A recorded piano's row: its name, who recorded it and its licence, and at its right what
/// it needs (*Get · size*, its progress and *Cancel*, *Choose mics…*) or what it is (*On
/// disk*, *Playing*).
fn piano(
    app: &mut CantusApp,
    ui: &egui::Ui,
    track: TrackId,
    s: &Style,
    r: Rect,
    e: &PianoEntry,
    choice: &SourceChoice,
) {
    let p = Pal::of(s);
    let on = matches!(choice, SourceChoice::Recorded(x) if *x == e.source);
    let installed = e.installed();
    let busy = e
        .images
        .iter()
        .find_map(|i| match app.samples.content().status(&i.pack) {
            Status::Busy(ph) => Some((i.pack.clone(), ph)),
            _ => None,
        });
    // The image to get first: a set's default, else the one there is.
    let first = e
        .images
        .iter()
        .find(|i| i.output == super::DEFAULT_IMAGE)
        .or(e.images.first());
    enum Ctl {
        Cancel(String),
        Get(String, String),
        Choose,
        Is(String),
    }
    let ctl = if let Some((pack, _)) = &busy {
        Ctl::Cancel(pack.clone())
    } else if !installed {
        let size = first.map_or(0, |i| i.download);
        let full = tf("chorda_source.get", &[("size", &size_words(size))]);
        // The whole words when they leave the name its room, else the size alone (the
        // download glyph says the rest; the help line says it all).
        let name_w = iu::width(ui, s, &e.name, 12.0, W::SemiBold);
        let room = r.width() - s.s(22.0) - name_w - s.s(6.0);
        let label = if iu::width(ui, s, &full, 12.0, W::Medium) + s.s(36.0) <= room {
            full
        } else {
            size_words(size)
        };
        Ctl::Get(label, first.map(|i| i.pack.clone()).unwrap_or_default())
    } else if e.images.len() > 1 {
        Ctl::Choose
    } else if on {
        Ctl::Is(t("chorda_source.playing"))
    } else {
        Ctl::Is(t("chorda_source.on_disk"))
    };
    let (label, lead) = match &ctl {
        Ctl::Cancel(_) => (String::new(), iu::Lead::Glyph(Gb::Clear)),
        Ctl::Get(l, _) => (l.clone(), iu::Lead::Glyph(Gb::Download)),
        Ctl::Choose => (t("chorda_source.choose_mics"), iu::Lead::None),
        Ctl::Is(l) => (l.clone(), iu::Lead::Glyph(Gb::Check)),
    };
    let cw = match &ctl {
        Ctl::Cancel(_) => s.s(22.0),
        Ctl::Choose => iu::width(ui, s, &label, 12.0, W::Medium) + s.s(16.0),
        Ctl::Get(..) => iu::width(ui, s, &label, 12.0, W::Medium) + s.s(36.0),
        Ctl::Is(_) => iu::width(ui, s, &label, 11.0, W::Regular) + s.s(18.0),
    };
    let full_about = format!("{} \u{b7} {}", e.author, licence_word(&e.licence));
    let about = if busy.is_some() {
        String::new()
    } else if iu::width(ui, s, &full_about, 10.0, W::Regular) <= r.width() - s.s(22.0) {
        full_about
    } else {
        licence_word(&e.licence)
    };
    let (resp, stacked) = item(
        ui,
        s,
        r,
        ("p", &e.source.pack),
        &e.name,
        &about,
        on,
        cw + s.s(6.0),
    );
    let resp = iu::hint(resp, || {
        if installed {
            t("chorda_source.tip.piano")
        } else {
            t("chorda_source.tip.get")
        }
    });
    if resp.clicked() && installed && !on {
        app.chorda_switch(track, &SourceChoice::Recorded(e.source.clone()));
    }
    let cr = control_rect(s, r, cw, stacked);
    match &ctl {
        Ctl::Is(_) => {
            with_brush(ui.painter(), |b| {
                let g = Rect::from_center_size(
                    pos2(cr.left() + s.s(6.0), cr.center().y),
                    vec2(s.s(11.0), s.s(11.0)),
                );
                let _ = crate::kit::glyph_browser::paint(b, g, Gb::Check, p.ink2);
                let tr = Rect::from_min_max(pos2(g.right() + s.s(5.0), cr.top()), cr.max);
                words(
                    b,
                    s,
                    &label,
                    tr,
                    Align2::LEFT_CENTER,
                    11.0,
                    W::Regular,
                    p.ink2,
                );
            });
        }
        _ => {
            let id = ui.id().with(("chorda-src-ctl", &e.source.pack));
            let cresp = iu::button(ui, s, cr, id, &label, lead, false, true);
            let cresp = iu::hint(cresp, || match &ctl {
                Ctl::Cancel(_) => t("chorda_source.tip.cancel"),
                Ctl::Choose => t("chorda_source.tip.choose"),
                _ => t("chorda_source.tip.get"),
            });
            if cresp.clicked() {
                match &ctl {
                    Ctl::Cancel(pack) => app.samples.content().cancel(pack),
                    Ctl::Get(_, pack) => {
                        app.samples.content().install(pack);
                    }
                    Ctl::Choose => {
                        if !on {
                            app.chorda_switch(track, &SourceChoice::Recorded(e.source.clone()));
                        }
                        app.inst.view = crate::views::inst::View::Mixer;
                    }
                    Ctl::Is(_) => {}
                }
            }
        }
    }
    // While it comes: the bar and how far, under the name.
    if let Some((_, ph)) = busy {
        let f = crate::views::samples_content::fraction(ph.as_ref());
        let total = first.map_or(0, |i| i.download);
        let done = match ph {
            Some(Phase::Downloading { done, .. }) => done,
            _ => (total as f64 * f64::from(f)) as u64,
        };
        with_brush(ui.painter(), |b| {
            let tx = r.left() + s.s(14.0);
            let bar = Rect::from_min_size(
                pos2(tx, r.top() + s.s(24.0)),
                vec2(r.width() * 0.48, s.s(3.0)),
            );
            b.rrect(b.snap_rect(bar), s.s(1.5), Fill::Solid(p.bar_track));
            let fill =
                Rect::from_min_size(bar.min, vec2(bar.width() * f.clamp(0.0, 1.0), bar.height()));
            b.rrect(b.snap_rect(fill), s.s(1.5), Fill::Solid(p.accent));
            let said = tf(
                "chorda_source.progress",
                &[
                    ("pct", &format!("{:.0}", f * 100.0)),
                    ("done", &size_words(done)),
                    ("total", &size_words(total)),
                ],
            );
            let wr = Rect::from_min_size(
                pos2(tx, bar.bottom() + s.s(2.0)),
                vec2(r.right() - s.s(8.0) - tx, s.s(12.0)),
            );
            iu::note(
                b,
                s,
                &said,
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
        ui.id().with(("chorda-src-install", track.0)),
        Sense::click(),
    );
    let resp = iu::hint(resp, || t("chorda_source.tip.install"));
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
            &t("chorda_source.install_file"),
            nr,
            Align2::LEFT_CENTER,
            12.0,
            W::Medium,
            p.ink1,
        );
        let fr = Rect::from_min_max(
            pos2(tx + s.s(14.0), nr.bottom()),
            pos2(rr.right() - s.s(8.0), rr.bottom() - s.s(6.0)),
        );
        iu::note(
            b,
            s,
            "SFZ \u{b7} Decent Sampler \u{b7} SF2",
            fr,
            Align2::LEFT_CENTER,
            10.0,
            W::Regular,
            p.ink3,
        );
    });
    resp
}
