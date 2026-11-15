//! Chorda's strips on the shared instrument desk (round 40 phase 3, ADR 0123, 0125; the
//! design's *Chorda · Mixer* sheets): what each output is ([`Out`]), its strip as the desk
//! draws it ([`super::super::inst_mixer::Strip`]), and the desk itself — *Piano* pinned
//! left, the outputs in the middle under their families' heads, *Room* pinned right; an
//! image not on disk as a dashed placeholder that offers its download; an image on disk
//! loaded or not from its top cell (*as recorded* · *unloaded*); the Out list.

use super::mixer::MixLook;
use super::read;
use crate::app::CantusApp;
use crate::i18n::{t, tf};
use crate::kit::menu::Item;
use crate::kit::text::Src;
use crate::kit::{with_brush, Style};
use crate::views::inst::inst_mixer::ui::{self as iu, words};
use crate::views::inst::inst_mixer::{self as im, desk, strip, Edit, OutCell, Strip, Top};
use crate::views::mixer::look::{self, Pal, W};
use cantus_core::inst::chorda_source::{self as cs, ChordaSource};
use cantus_core::inst_out::{self, key, OutputField, RouteView};
use cantus_core::{NativeSpec, OutputFamily, OutputId, TrackId};
use egui::{pos2, vec2, Align2, Rect};

/// The Room strip's width at 100 % (pinned right).
pub const ROOM_W: f32 = 128.0;
/// The model's room output (`MODEL_OUTPUTS`' *Room*): pinned right, not in the middle.
pub const MODEL_ROOM: OutputId = OutputId(3);

/// One output of the piano as the Mixer shows it.
#[derive(Clone, Debug, PartialEq)]
pub struct Out {
    pub id: OutputId,
    pub name: String,
    pub sub: String,
    pub family: OutputFamily,
    /// The image is on disk (a recorded set's image may not be).
    pub on_disk: bool,
    /// Not on disk: its pack and what it takes on disk once installed.
    pub get: Option<(String, u64)>,
    /// Loaded (`o{n}.on`, and its zones are in the instrument).
    pub on: bool,
    /// One image of a set (loaded or not from its top cell).
    pub image: bool,
    /// An image's pack.
    pub pack: Option<String>,
}

/// A name the reader's language has a word for (the model's, *Releases*, *Stereo*); a
/// pack's own mic names stay as the pack says them.
pub fn display_name(name: &str) -> String {
    let k = format!("chorda_source.name.{}", slug(name));
    if crate::i18n::has(crate::i18n::Locale::En, &k) {
        t(&k)
    } else {
        name.to_string()
    }
}

fn slug(name: &str) -> String {
    name.to_ascii_lowercase()
        .split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|w| !w.is_empty())
        .collect::<Vec<_>>()
        .join("_")
}

/// The line under an output's name: what the microphone is and where (bitKlavier's images,
/// the model's pairs, the noises), else its family.
pub fn sub_of(name: &str, family: OutputFamily) -> String {
    let k = format!("chorda_source.sub.{}", slug(name));
    if crate::i18n::has(crate::i18n::Locale::En, &k) {
        t(&k)
    } else {
        t(family.label_key())
    }
}

/// The outputs of the Chorda `spec` plays, in output order: a recorded set's every image
/// (on disk or not) and its noises, a pack's outputs, or the model's five.
pub fn outs(app: &mut CantusApp, spec: &NativeSpec) -> Vec<Out> {
    let have = cs::outputs(spec);
    let entry = match ChordaSource::of(spec) {
        ChordaSource::Recorded(r) if cs::has_zones(spec) => read::entry(app.samples.content(), &r),
        _ => None,
    };
    let on = |o: OutputId| spec.get(&key(o, OutputField::On)) >= 0.5;
    // What an image takes on disk once installed (FLAC: the manifest's measured size), else
    // what it downloads.
    let sizes: std::collections::HashMap<String, u64> = app
        .samples
        .content()
        .manifest()
        .packs
        .iter()
        .map(|p| (p.id.clone(), p.installed_size))
        .collect();
    let on_disk_size = |pack: &str, download: u64| {
        sizes
            .get(pack)
            .copied()
            .filter(|z| *z > 0)
            .unwrap_or(download)
    };
    let plain = |o: &cs::SourceOutput| Out {
        id: o.id,
        name: display_name(&o.name),
        sub: sub_of(&o.name, o.family),
        family: o.family,
        on_disk: true,
        get: None,
        on: on(o.id),
        image: false,
        pack: None,
    };
    let mut v: Vec<Out> = Vec::new();
    match entry.as_ref().filter(|e| e.images.len() > 1) {
        Some(e) => {
            for img in &e.images {
                v.push(Out {
                    id: img.output,
                    name: display_name(&img.name),
                    sub: sub_of(&img.name, OutputFamily::Perspective),
                    family: OutputFamily::Perspective,
                    on_disk: img.installed.is_some(),
                    get: img
                        .installed
                        .is_none()
                        .then(|| (img.pack.clone(), on_disk_size(&img.pack, img.download))),
                    on: on(img.output) && have.iter().any(|o| o.id == img.output),
                    image: true,
                    pack: Some(img.pack.clone()),
                });
            }
            v.extend(
                have.iter()
                    .filter(|o| !e.images.iter().any(|i| i.output == o.id))
                    .map(plain),
            );
        }
        None => v.extend(have.iter().map(plain)),
    }
    v.sort_by_key(|o| o.id);
    v
}

/// The strip on the desk of `track`'s mixer, its number in the mixer.
fn track_strip(app: &CantusApp, track: TrackId) -> Option<(cantus_core::StripId, usize)> {
    let p = app.project();
    let s = p.track(track)?.strip?;
    Some((s, p.strip_number(s)?))
}

/// The Out cell of output `id` named `name`.
pub fn out_cell(app: &CantusApp, track: TrackId, name: &str, id: OutputId) -> OutCell {
    let p = app.project();
    let inst = p.track(track).map(|t| t.name.clone()).unwrap_or_default();
    let mine = track_strip(app, track).map_or(0, |m| m.1);
    let (label, number, where_) = match inst_out::route_view(p, track, id) {
        RouteView::Main => (
            t("inst_mixer.out_main"),
            None,
            tf("inst_mixer.out_says_main", &[("n", &mine)]),
        ),
        RouteView::Own(s) => {
            let n = p.strip_number(s).unwrap_or(0);
            (
                t("inst_mixer.out_own"),
                Some(n),
                tf(
                    "inst_mixer.out_says_own",
                    &[("n", &n), ("inst", &inst), ("m", &mine)],
                ),
            )
        }
        RouteView::Chosen(s) => {
            let n = p.strip_number(s).unwrap_or(0);
            let sn = p.strip(s).map(|x| x.name.clone()).unwrap_or_default();
            (
                sn.clone(),
                Some(n),
                tf("inst_mixer.out_says_chosen", &[("name", &sn), ("n", &n)]),
            )
        }
    };
    OutCell {
        label,
        number,
        opens: true,
        tip: tf(
            "inst_mixer.out_tip",
            &[("name", &name.to_string()), ("where", &where_)],
        ),
    }
}

/// An output's meter: its strip's when it plays on one, else the peak Exemplar's unit
/// reports for it (a recorded piano); the model reports none per output yet.
fn meter_of(app: &CantusApp, track: TrackId, id: OutputId) -> Option<[f32; 2]> {
    let p = app.project();
    let lin = match inst_out::route_view(p, track, id) {
        RouteView::Own(s) | RouteView::Chosen(s) => {
            let i = p.strip_index(s)?;
            app.engine.meters().strips.get(i).copied().unwrap_or(0.0)
        }
        RouteView::Main => {
            let slot = cantus_engine::inst::sampler::tel::out_peak(id.0)?;
            app.inst_telemetry().get(slot).copied().unwrap_or(0.0)
        }
    };
    (lin > 1e-5).then(|| {
        let d = cantus_core::linear_to_db(lin);
        [d, d - 0.6]
    })
}

/// Output `o` as the desk draws it.
pub fn as_strip(
    app: &CantusApp,
    track: TrackId,
    spec: &NativeSpec,
    o: &Out,
    lk: &MixLook,
) -> Strip {
    let g = |f| spec.get(&key(o.id, f));
    let top = if o.image {
        Top::Words(if o.on {
            t("chorda_source.strip.as_recorded")
        } else {
            t("inst_mixer.unloaded")
        })
    } else {
        Top::Share {
            word: t("inst_mixer.level_short"),
            value: cantus_core::db_to_linear(g(OutputField::Level)).min(1.0),
            full: 1.0,
        }
    };
    Strip {
        id: Some(o.id),
        name: o.name.clone(),
        sub: o.sub.clone(),
        family: o.family,
        top,
        invert: Some(g(OutputField::Invert) >= 0.5),
        pan: Some(g(OutputField::Pan)),
        mute: Some(g(OutputField::Mute) >= 0.5),
        solo: Some(g(OutputField::Solo) >= 0.5),
        delay: Some(g(OutputField::Delay)),
        level_db: g(OutputField::Level),
        level_max: 12.0,
        meter: if o.on && o.on_disk {
            meter_of(app, track, o.id)
        } else {
            None
        },
        out: out_cell(app, track, &o.name, o.id),
        selected: lk.selected == Some(o.id.0),
        loaded: o.on && o.on_disk,
    }
}

/// The level key of the whole piano: Chorda's `gain`, or the recorded piano's own.
pub fn gain_key(spec: &NativeSpec) -> &'static str {
    if cs::has_zones(spec) {
        "rec.gain"
    } else {
        "gain"
    }
}

/// *Piano*, pinned left: all the mics — the model's width, the piano's level, where the
/// instrument plays (its own strip of the desk).
fn piano_strip(app: &CantusApp, track: TrackId, spec: &NativeSpec) -> Strip {
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
    let top = if cs::has_zones(spec) {
        Top::Words(t("chorda_source.strip.as_recorded"))
    } else {
        Top::Share {
            word: t("inst_mixer.width_short"),
            value: spec.get("width"),
            full: 1.0,
        }
    };
    let piano = t("chorda_source.strip.piano");
    Strip {
        id: None,
        name: piano.clone(),
        sub: t("chorda_source.strip.piano_sub"),
        family: OutputFamily::Other,
        top,
        invert: None,
        pan: None,
        mute: None,
        solo: None,
        delay: None,
        level_db: spec.get(gain_key(spec)),
        level_max: 12.0,
        meter,
        out: OutCell {
            label: name,
            number: mine.map(|m| m.1),
            opens: false,
            tip: tf(
                "inst_mixer.out_tip",
                &[
                    ("name", &piano),
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

/// The strips: *Piano* | the outputs under their heads, scrolling | *Room*.
#[allow(clippy::too_many_arguments)]
pub fn desk(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    track: TrackId,
    spec: &NativeSpec,
    s: &Style,
    r: Rect,
    outs: &[Out],
    lk: &mut MixLook,
) {
    let p = Pal::of(s);
    with_brush(ui.painter(), |b| iu::card(b, s, r));
    let inner = r.shrink(s.s(6.0));
    let pitch = s.s(im::STRIP_W);
    let g = s.s(8.0);
    let left = Rect::from_min_size(inner.min, vec2(pitch, inner.height()));
    let right = Rect::from_min_max(pos2(inner.right() - s.s(ROOM_W), inner.top()), inner.max);
    let heads = Rect::from_min_max(
        pos2(left.right() + g, inner.top()),
        pos2(right.left() - g, inner.top() + s.s(im::HEADS)),
    );
    let mid = Rect::from_min_max(
        pos2(heads.left(), heads.bottom() + s.s(4.0)),
        pos2(heads.right(), inner.bottom()),
    );
    // Piano, pinned left.
    let piano = piano_strip(app, track, spec);
    let gain = gain_key(spec);
    let model = !cs::has_zones(spec);
    for e in strip::show(ui, s, left, &piano, ui.id().with(("chorda-piano", track.0))) {
        match e {
            Edit::Top(f) if model => app.set_inst_value(track, "width", f, true),
            Edit::Level(db) => app.set_inst_value(track, gain, db.clamp(-48.0, 12.0), true),
            Edit::End => app.end_gesture(),
            _ => {}
        }
    }
    // The outputs (the model's room is pinned right).
    let middle: Vec<&Out> = outs
        .iter()
        .filter(|o| !(model && o.id == MODEL_ROOM))
        .collect();
    let strips: Vec<Strip> = middle
        .iter()
        .map(|o| as_strip(app, track, spec, o, lk))
        .collect();
    let salt = ui.id().with(("chorda-outs", track.0));
    let mut scroll = lk.scroll;
    let (edits, shown) = desk::middle(ui, s, heads, mid, &strips, &lk.folded, &mut scroll, salt);
    lk.scroll = scroll;
    if let Some(f) = shown.toggle {
        if let Some(i) = lk.folded.iter().position(|x| *x == f) {
            lk.folded.remove(i);
        } else {
            lk.folded.push(f);
        }
    }
    for (i, e) in edits {
        if middle[i].on_disk {
            edit(app, ui, track, spec, lk, middle[i], e);
        }
    }
    // Over the strips the desk drew: an image not on disk is a placeholder; an image on
    // disk loads or unloads from its top cell.
    let clip = mid.intersect(ui.clip_rect());
    for (i, o) in middle.iter().enumerate() {
        let Some(at) = shown.at[i] else { continue };
        let at = at.shrink2(vec2(s.k, 0.0));
        if !o.on_disk {
            placeholder(app, ui, track, s, at, clip, o);
        } else if o.image {
            load_cell(app, ui, track, s, at, clip, o);
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
    let room_out = outs.iter().find(|o| model && o.id == MODEL_ROOM);
    super::room::room(app, ui, track, spec, s, right, room_out, lk);
    out_list(app, ui, track, s, outs, lk);
}

/// What an edit on output `o`'s strip does: a command on its `o{n}.` keys, or the view's.
fn edit(
    app: &mut CantusApp,
    ui: &egui::Ui,
    track: TrackId,
    spec: &NativeSpec,
    lk: &mut MixLook,
    o: &Out,
    e: Edit,
) {
    let id = o.id;
    let flip = |f: OutputField| {
        if spec.get(&key(id, f)) >= 0.5 {
            0.0
        } else {
            1.0
        }
    };
    match e {
        Edit::Top(f) if !o.image => {
            let db = if f <= 0.001 {
                -96.0
            } else {
                cantus_core::linear_to_db(f).max(-96.0)
            };
            app.set_inst_value(track, &key(id, OutputField::Level), db, true)
        }
        Edit::Top(_) => {}
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
        Edit::Select => lk.selected = Some(id.0),
        Edit::Out(at) => {
            lk.selected = Some(id.0);
            lk.open = Some((id.0, at, ui.ctx().cumulative_pass_nr()));
        }
    }
}

/// An image not on disk: the strip's ground, a dashed box, *Not on disk* and its download.
#[allow(clippy::too_many_arguments)]
fn placeholder(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    track: TrackId,
    s: &Style,
    r: Rect,
    clip: Rect,
    o: &Out,
) {
    let p = Pal::of(s);
    let k = s.k;
    let parts = strip::parts(r, s);
    // The name stays; the rest is the box.
    let body = Rect::from_min_max(
        pos2(r.left() + 6.0 * k, parts.top.top()),
        pos2(r.right() - 6.0 * k, parts.out.bottom()),
    );
    let get = Rect::from_center_size(
        pos2(body.center().x, body.center().y + 16.0 * k),
        vec2(body.width() - 12.0 * k, 34.0 * k),
    );
    let size = o.get.as_ref().map_or(0, |g| g.1);
    let weighs = format!("\u{2248} {}", super::list::size_words(size));
    let busy = o.get.as_ref().is_some_and(|g| {
        matches!(
            app.samples.content().status(&g.0),
            cantus_content::Status::Busy(_)
        )
    });
    let resp = ui.interact(
        get.intersect(clip),
        ui.id().with(("chorda-get", track.0, o.id.0)),
        egui::Sense::click(),
    );
    let name = o.name.clone();
    let tip_size = weighs.clone();
    let resp = iu::hint(resp, || {
        tf(
            "chorda_source.mixer.get_tip",
            &[("name", &name), ("size", &tip_size)],
        )
    });
    if resp.clicked() && !busy {
        if let Some((pack, _)) = &o.get {
            app.samples.content().install(pack);
        }
    }
    with_brush(&ui.painter_at(clip), |b| {
        // Cover what the desk drew under the name.
        b.rect(
            Rect::from_min_max(pos2(r.left(), parts.top.top() - 2.0 * k), r.max),
            crate::kit::Fill::Solid(p.base),
        );
        let bx = b.snap_rect(body);
        let pts = [
            bx.left_top(),
            bx.right_top(),
            bx.right_bottom(),
            bx.left_bottom(),
            bx.left_top(),
        ];
        b.dashed(&pts, b.px(), p.line, 4.0 * k, 3.0 * k);
        let nr = Rect::from_min_size(
            pos2(body.left(), get.top() - 24.0 * k),
            vec2(body.width(), 14.0 * k),
        );
        words(
            b,
            s,
            &t("chorda_source.mixer.not_on_disk"),
            nr,
            Align2::CENTER_CENTER,
            10.0,
            W::Regular,
            p.ink3,
        );
        let st = crate::kit::State::of(&resp, busy);
        let label = if busy {
            t("chorda_source.mixer.getting")
        } else {
            weighs.clone()
        };
        // The key: the download glyph over what it weighs (the design's placeholder).
        iu::paint_button(b, s, get, st, "", iu::Lead::None, !busy);
        let g = Rect::from_center_size(
            pos2(get.center().x, get.top() + 11.0 * k),
            vec2(11.0 * k, 11.0 * k),
        );
        let _ = crate::kit::glyph_browser::paint(
            b,
            g,
            crate::kit::glyph_browser::Glyph::Download,
            p.ink1,
        );
        let lr = Rect::from_min_max(pos2(get.left(), g.bottom()), get.max);
        words(
            b,
            s,
            &label,
            lr,
            Align2::CENTER_CENTER,
            10.0,
            W::Medium,
            p.ink1,
        );
    });
}

/// An image on disk: its top cell loads or unloads it (its samples read or freed), one
/// undo step (`super::set_image_on`).
#[allow(clippy::too_many_arguments)]
fn load_cell(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    track: TrackId,
    s: &Style,
    r: Rect,
    clip: Rect,
    o: &Out,
) {
    let parts = strip::parts(r, s);
    let resp = ui.interact(
        parts.top.intersect(clip),
        ui.id().with(("chorda-load", track.0, o.id.0)),
        egui::Sense::click(),
    );
    let name = o.name.clone();
    let on = o.on;
    let resp = iu::hint(resp, || {
        let k = if on {
            "chorda_source.mixer.unload_tip"
        } else {
            "chorda_source.mixer.load_tip"
        };
        tf(k, &[("name", &name)])
    });
    if resp.hovered() {
        let p = Pal::of(s);
        with_brush(&ui.painter_at(clip), |b| {
            look::well(b, parts.top, &p, true);
            let w = if on {
                t("chorda_source.mixer.unload")
            } else {
                t("chorda_source.mixer.load")
            };
            words(
                b,
                s,
                &w,
                parts.top,
                Align2::CENTER_CENTER,
                10.0,
                W::Medium,
                p.ink1,
            );
        });
    }
    if resp.clicked() {
        super::set_image_on(app, track, o.id, !on);
    }
}

/// The Out list of the output whose cell was clicked: *Main*, the first free strip, the
/// desk's named strips. A choice is one command (`inst_out::set_route`), the slot keeping
/// the output's name (a song whose pack is missing still says it).
fn out_list(
    app: &mut CantusApp,
    ui: &egui::Ui,
    track: TrackId,
    s: &Style,
    outs: &[Out],
    lk: &mut MixLook,
) {
    let Some((n, at, opened)) = lk.open else {
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
        ui.id().with(("chorda-out-list", track.0)),
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
        let name = outs.iter().find(|o| o.id.0 == n).map(|o| o.name.clone());
        if let Ok(mut cmd) = inst_out::set_route(&view, track, OutputId(n), target) {
            if let (cantus_core::Command::SetInstrumentOutput { slot: Some(sl), .. }, Some(nm)) =
                (&mut cmd, name)
            {
                sl.name = nm;
            }
            app.exec(cmd);
        }
        lk.open = None;
    } else if close && ui.ctx().cumulative_pass_nr() > opened {
        lk.open = None;
    }
}

/// Open the Out list of output `id` from the cell at `at` (the pinned Room's own cell).
pub fn open_out(ui: &egui::Ui, lk: &mut MixLook, id: OutputId, at: Rect) {
    lk.selected = Some(id.0);
    lk.open = Some((id.0, at, ui.ctx().cumulative_pass_nr()));
}
