//! Exemplar's two views on the kit (ADR 0024 §8): the display first — the key × velocity
//! map, the selected zone's waveform and seam — then a card per section of the signal
//! flow, each in its function's hue, with cells inside.
//!
//! **Play**: the map beside the zone's waveform, lit by what sounds, over four cards of
//! the parameters that matter live. **Edit**: the files and quality row, the map beside
//! the zone card, the sample and seam cards, the instrument's cards (amplitude, filter,
//! output; modulation envelope, vibrato, LFO 2, voice; the zone's group, keyswitches) and
//! the zone table. Edit is taller than the window: it scrolls, never squeezes.

use super::parts::{self, write_pan};
use super::{drop_files, map, open_instrument, relative, sfz, take_drop, wave};
use crate::app::CantusApp;
use crate::i18n::{t, tf};
use crate::kit::band::{self, CardLook, CardRects};
use crate::kit::knob::KnobSize;
use crate::kit::menu::Item;
use crate::kit::text::{Elide, Policy, Role, Src};
use crate::kit::{with_brush, Function, Style};
use cantus_core::inst::modulation::connections;
use cantus_core::inst::sampler::{self as model, gf, zf, GROUP_PARAMS};
use cantus_core::{pitch_name, NativeSpec, TrackId};
use egui::{pos2, vec2, Align2, Rect, Sense};
use std::path::Path;

/// One of the instrument's own numbers as a kit cell: range, taper and label from its
/// description (the label abbreviates through `<label_key>.short`), the readout in its
/// unit, one undo step per drag, a modulation source dropped on it connects, and what
/// modulates it drawn as rings.
#[allow(clippy::too_many_arguments)]
pub fn cell(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    track: TrackId,
    spec: &NativeSpec,
    key: &str,
    rect: Rect,
    s: &Style,
    size: KnobSize,
) -> egui::Response {
    let Some(info) = spec.kind.param(key) else {
        return ui.allocate_rect(rect, Sense::hover());
    };
    let mut rings: Vec<(f32, egui::Color32)> = connections(spec)
        .filter(|c| c.dst == key && c.on)
        .map(|c| (c.amount, s.t.hue(crate::views::inst::source_family(c.src))))
        .collect();
    let mut value = spec.get(key);
    let mut mod_rings = [(0.0, egui::Color32::TRANSPARENT); crate::views::modulator::MAX_RINGS];
    let w = crate::views::inst::frame::wire(
        app,
        track,
        key,
        &mut value,
        (info.min, info.max, info.logarithmic),
        s,
        &mut mod_rings,
    );
    rings.extend_from_slice(&mod_rings[..w.n]);
    let name = t(info.label_key);
    let (unit, stepped) = (info.unit, info.stepped);
    let pan = key.ends_with("pan");
    let note = key.starts_with("sw.");
    let (r, v, choice) = parts::num_cell_offered(
        ui,
        rect,
        s,
        egui::Id::new(("smp-cell", track.0, key)),
        value,
        info.min..=info.max,
        info.default,
        info.logarithmic,
        stepped,
        Some(Src::Key(info.label_key)),
        &name,
        size,
        &rings,
        Some((w.automated, w.offer)),
        move |v, b| {
            if pan {
                write_pan(v, b);
            } else if note {
                if v < -0.5 {
                    b.push('\u{2014}');
                } else {
                    b.push_str(&pitch_name(v.round() as u8));
                }
            } else {
                parts::write_value(v, unit, stepped, b);
            }
        },
    );
    crate::views::inst::frame::wired(app, &w, choice, r.rect);
    if r.changed() {
        app.set_inst_value(track, key, v, true);
    }
    if r.drag_stopped() || r.double_clicked() {
        app.end_gesture();
    }
    if egui::DragAndDrop::has_payload_of_type::<&'static str>(ui.ctx()) && r.contains_pointer() {
        with_brush(ui.painter(), |b| {
            b.rrect_stroke(r.rect, s.r(6.0), 1.5 * s.k, s.hue)
        });
    }
    if let Some(src) = r.dnd_release_payload::<&'static str>() {
        app.connect_modulation(track, *src, key);
    }
    r
}

/// A card of the instrument's cells: what it shows and how.
struct Section<'a> {
    title: &'a str,
    function: Function,
    keys: &'a [&'a str],
    /// A type selector in the title bar: the parameter and its names.
    selector: Option<(&'a str, &'a [&'a str])>,
}

const FILTER_TYPES: [&str; 4] = [
    "smp.filter.off",
    "smp.filter.lp",
    "smp.filter.bp",
    "smp.filter.hp",
];
const LFO_SHAPES: [&str; 4] = [
    "smp.shape.sine",
    "smp.shape.triangle",
    "smp.shape.square",
    "smp.shape.saw",
];
const PLAY_MODES: [&str; 3] = ["smp.play.poly", "smp.play.mono", "smp.play.legato"];

/// The width a section's card needs, at this style's scale.
fn need(s: &Style, sec: &Section<'_>, size: KnobSize) -> f32 {
    let w = parts::card_for(s, size, sec.keys.len(), 1).x;
    if sec.selector.is_some() {
        w.max(172.0 * s.k)
    } else {
        w.max(96.0 * s.k)
    }
}

/// Paint a section's card in `rect` and its cells; the selector steps with ‹ ›.
#[allow(clippy::too_many_arguments)]
fn section(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    track: TrackId,
    spec: &NativeSpec,
    rect: Rect,
    s: &Style,
    sec: &Section<'_>,
    size: KnobSize,
) -> CardRects {
    let s = &s.function(sec.function);
    let current = sec
        .selector
        .map(|(key, names)| (spec.get(key).round() as usize).min(names.len() - 1));
    let sel_name = sec.selector.zip(current).map(|((_, names), i)| t(names[i]));
    let look = CardLook {
        title: Src::Key(sec.title),
        switchable: false,
        selector: sel_name.as_deref(),
        menu: false,
        display: None,
    };
    let cr = parts::card(ui, rect, s, &look, true);
    if let (Some((key, names)), Some(i)) = (sec.selector, current) {
        let step = parts::selector_step(
            ui,
            &cr,
            s,
            &t(spec.kind.param(key).map_or(key, |p| p.label_key)),
        );
        if step != 0 {
            let n = names.len() as i32;
            let j = (i as i32 + step).rem_euclid(n);
            app.set_inst_value(track, key, j as f32, false);
        }
    }
    let per_row = sec.keys.len();
    let body = centred_row(cr.body, s, size);
    for (key, r) in sec
        .keys
        .iter()
        .zip(parts::cells(body, s, size, sec.keys.len(), per_row))
    {
        cell(app, ui, track, spec, key, r, s, size);
    }
    cr
}

/// The body of a one-row card, trimmed to the cells' height and centred in it.
fn centred_row(body: Rect, s: &Style, size: KnobSize) -> Rect {
    let h = parts::cell_size(s, size).y;
    let top = body.center().y - h * 0.5;
    Rect::from_min_max(pos2(body.left(), top), pos2(body.right(), top + h))
}

/// Lay a row of sections out across `row`, each at least as wide as it needs.
#[allow(clippy::too_many_arguments)]
fn section_row(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    track: TrackId,
    spec: &NativeSpec,
    row: Rect,
    s: &Style,
    secs: &[Section<'_>],
    size: KnobSize,
) {
    let gut = parts::GAP * s.k;
    let needs: Vec<f32> = secs.iter().map(|sec| need(s, sec, size)).collect();
    let fr = parts::fracs(&needs, row.width(), gut);
    let room = row.width() - gut * (secs.len().saturating_sub(1)) as f32;
    let mut x = row.left();
    for (sec, f) in secs.iter().zip(fr) {
        let w = room * f;
        let r = Rect::from_min_size(pos2(x, row.top()), vec2(w, row.height()));
        section(app, ui, track, spec, r, s, sec, size);
        x += w + gut;
    }
}

const AMP: Section<'static> = Section {
    title: "smp.card.amp",
    function: Function::Dynamics,
    keys: &[
        "amp.attack",
        "amp.hold",
        "amp.decay",
        "amp.sustain",
        "amp.release",
        "vel.amp",
    ],
    selector: None,
};
const FILTER: Section<'static> = Section {
    title: "smp.card.filter",
    function: Function::Tone,
    keys: &[
        "filter.cutoff",
        "filter.reso",
        "filter.key",
        "filter.vel",
        "filter.env",
    ],
    selector: Some(("filter.type", &FILTER_TYPES)),
};
const OUTPUT: Section<'static> = Section {
    title: "smp.card.output",
    function: Function::Output,
    keys: &["gain", "pan", "start.vel"],
    selector: None,
};
const MOD_ENV: Section<'static> = Section {
    title: "smp.card.env2",
    function: Function::Dynamics,
    keys: &["env2.attack", "env2.decay", "env2.sustain", "env2.release"],
    selector: None,
};
const VIBRATO: Section<'static> = Section {
    title: "smp.card.lfo1",
    function: Function::Mod,
    keys: &["lfo1.pitch", "lfo1.rate", "lfo1.delay"],
    selector: Some(("lfo1.shape", &LFO_SHAPES)),
};
const LFO2: Section<'static> = Section {
    title: "smp.card.lfo2",
    function: Function::Mod,
    keys: &["lfo2.rate"],
    selector: Some(("lfo2.shape", &LFO_SHAPES)),
};
const VOICE: Section<'static> = Section {
    title: "smp.card.voice",
    function: Function::Output,
    keys: &["voices", "glide", "bend", "transpose", "tune"],
    selector: Some(("mode", &PLAY_MODES)),
};
const KEYSWITCH: Section<'static> = Section {
    title: "smp.card.keyswitch",
    function: Function::Output,
    keys: &["sw.lo", "sw.hi", "sw.default"],
    selector: None,
};

/// The selected zone's group as a card: its level, pan, tune and the envelope that
/// overrides the instrument's, its voices; the LED mutes it.
#[allow(clippy::too_many_arguments)]
fn group_card(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    track: TrackId,
    spec: &NativeSpec,
    rect: Rect,
    s: &Style,
    size: KnobSize,
) {
    let s = &s.function(Function::Source);
    let m = app
        .inst
        .sampler
        .selected
        .and_then(|n| model::Zone::of(spec, n))
        .map(|z| z.get(zf::GROUP) as usize)
        .unwrap_or(0);
    let title = spec
        .text(&format!("g{m}.name"))
        .map(str::to_string)
        .unwrap_or_else(|| tf("smp.group_n", &[("n", &m.to_string())]));
    let muted = spec.get(&format!("g{m}.mute")) >= 0.5;
    let look = CardLook {
        title: Src::Str(&title),
        switchable: true,
        selector: None,
        menu: false,
        display: None,
    };
    let cr = parts::card(ui, rect, s, &look, !muted);
    if parts::card_led(ui, &cr, &t("smp.mute")) {
        app.set_inst_value(
            track,
            &format!("g{m}.mute"),
            if muted { 0.0 } else { 1.0 },
            false,
        );
    }
    let fields = [
        gf::GAIN,
        gf::PAN,
        gf::TUNE,
        gf::ATTACK,
        gf::HOLD,
        gf::DECAY,
        gf::SUSTAIN,
        gf::RELEASE,
        gf::VEL,
        gf::POLY,
    ];
    let keys: Vec<String> = fields
        .iter()
        .map(|f| format!("g{m}.{}", model::group_field_name(*f)))
        .collect();
    let body = centred_row(cr.body, s, size);
    for (key, r) in keys
        .iter()
        .zip(parts::cells(body, s, size, keys.len(), keys.len()))
    {
        cell(app, ui, track, spec, key, r, s, size);
    }
    let _ = GROUP_PARAMS.len();
}

/// The files and quality row: Import ▾ (samples, a folder, an instrument), Export SFZ…,
/// a SoundFont's presets when one waits for its choice, the quality, and the import notes.
fn tools(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    track: TrackId,
    spec: &NativeSpec,
    row: Rect,
    s: &Style,
) {
    let s = &s.function(Function::Source);
    let k = s.k;
    let h = row.height();
    let gap = 6.0 * k;
    let ir = Rect::from_min_size(row.min, vec2(132.0 * k, h));
    let (_, c) = parts::field(ui, ir, s, &t("smp.import"), false, false, &t("smp.import"));
    let items = [
        Item::action(Src::Key("smp.import_samples")),
        Item::action(Src::Key("smp.import_folder")),
        Item::action(Src::Key("smp.import_instrument")),
    ];
    let picked = parts::dropdown(
        ui,
        egui::Id::new(("smp-import", track.0)),
        ir,
        s,
        &items,
        c.open,
    );
    match picked {
        Some(0) => {
            if let Some(files) = rfd::FileDialog::new()
                .add_filter(t("smp.audio_files"), &["wav", "aif", "aiff", "flac", "ogg"])
                .pick_files()
            {
                drop_files(app, track, &files, 60);
            }
        }
        Some(1) => {
            if let Some(dir) = rfd::FileDialog::new().pick_folder() {
                drop_files(app, track, &[dir], 60);
            }
        }
        Some(_) => {
            if let Some(path) = rfd::FileDialog::new()
                .add_filter(
                    t("smp.instrument_files"),
                    &["sfz", "dspreset", "sf2", "sf3"],
                )
                .pick_file()
            {
                open_instrument(app, track, &path);
            }
        }
        None => {}
    }
    let er = Rect::from_min_size(pos2(ir.right() + gap, row.top()), vec2(128.0 * k, h));
    if parts::press(ui, er, s, Src::Key("smp.export_sfz"), &t("smp.export_sfz")).clicked() {
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("SFZ", &["sfz"])
            .set_file_name("exemplar.sfz")
            .save_file()
        {
            let base = path.parent().map(Path::to_path_buf).unwrap_or_default();
            let rel = |p: &str| relative(Path::new(p), &base);
            match std::fs::write(&path, sfz::export(spec, &rel)) {
                Ok(()) => {
                    app.set_status(tf("smp.exported", &[("path", &path.display().to_string())]))
                }
                Err(e) => app.set_error(e.to_string()),
            }
        }
    }
    // A SoundFont with several presets waits here for the one to load.
    let mut x = er.right() + gap;
    if let Some((path, sf)) = app.inst.sampler.pending_sf2.take() {
        let fr = Rect::from_min_size(pos2(x, row.top()), vec2(220.0 * k, h));
        x = fr.right() + gap;
        let names: Vec<String> = sf
            .presets
            .iter()
            .take(256)
            .map(|p| format!("{:03}:{:03} {}", p.bank, p.program, p.name))
            .collect();
        let mut labels: Vec<Src<'_>> = names.iter().map(|n| Src::Str(n)).collect();
        labels.push(Src::Key("smp.sf2_cancel"));
        let items = parts::radio_items(&labels, None);
        let id = egui::Id::new(("smp-sf2", track.0));
        let first = !ui.data(|d| d.get_temp::<bool>(id).unwrap_or(false));
        let (_, c) = parts::field(
            ui,
            fr,
            s,
            &t("smp.sf2_choose"),
            false,
            true,
            &t("smp.sf2_choose"),
        );
        let mut keep = true;
        if let Some(i) = parts::dropdown(ui, id, fr, s, &items, c.open || first) {
            keep = false;
            if i < names.len() {
                match super::sf2::to_import(&sf, i, &super::sf2_dir(&path)) {
                    Ok(imp) => super::apply_import(app, track, imp),
                    Err(e) => app.set_error(e),
                }
            }
        }
        if keep {
            app.inst.sampler.pending_sf2 = Some((path, sf));
        }
    }
    // The quality at the right, and the import notes when there are some.
    let q = (spec.get("quality").round() as usize).min(3);
    let names = [
        Src::Key("smp.q.draft"),
        Src::Key("smp.q.standard"),
        Src::Key("smp.q.high"),
        Src::Key("smp.q.best"),
    ];
    let qw = 300.0 * k;
    let qr = Rect::from_min_max(pos2(row.right() - qw, row.top()), row.max);
    let tips = [
        "smp.q.draft_tip",
        "smp.q.standard_tip",
        "smp.q.high_tip",
        "smp.q.best_tip",
    ];
    let hint = format!("{} \u{b7} {}", t("smp.quality"), t(tips[q]));
    if let Some(i) = parts::seg(ui, qr, s, q, &names, &hint) {
        app.set_inst_value(track, "quality", i as f32, false);
    }
    // Round robin's order (round 28): in order by each note's place in the song, or drawn
    // from the note (Exemplars saved before). Shown when the row has room for it.
    let rw = 190.0 * k;
    let rr = Rect::from_min_max(
        pos2(qr.left() - gap - rw, row.top()),
        pos2(qr.left() - gap, row.bottom()),
    );
    let mut left = qr.left();
    if rr.left() > x {
        let order = (spec.get("rr.order").round() as usize).min(1);
        let names = [Src::Key("smp.rr.by_note"), Src::Key("smp.rr.in_order")];
        let tips = ["smp.rr.by_note_tip", "smp.rr.in_order_tip"];
        let hint = format!("{} \u{b7} {}", t("smp.rr_order"), t(tips[order]));
        if let Some(i) = parts::seg(ui, rr, s, order, &names, &hint) {
            app.set_inst_value(track, "rr.order", i as f32, false);
        }
        left = rr.left();
    }
    // A drum kit (round 28): the piano roll and the channel rack show a row per drum.
    let kw = 120.0 * k;
    let kr = Rect::from_min_max(
        pos2(left - gap - kw, row.top()),
        pos2(left - gap, row.bottom()),
    );
    if kr.left() > x {
        let on = spec.get("kit") >= 0.5;
        let tip = format!("{} \u{b7} {}", t("smp.kit"), t("smp.kit_tip"));
        if parts::led(ui, kr, s, on, Src::Key("smp.kit"), &tip).clicked() {
            app.set_inst_value(track, "kit", if on { 0.0 } else { 1.0 }, false);
        }
        left = kr.left();
    }
    let lr = Rect::from_min_max(
        pos2(left - gap - 132.0 * k, row.top()),
        pos2(left - gap, row.bottom()),
    );
    if !app.inst.sampler.log.is_empty() && lr.left() > x {
        let label = tf(
            "smp.log_n",
            &[("n", &app.inst.sampler.log.len().to_string())],
        );
        if parts::led(
            ui,
            lr,
            s,
            app.inst.sampler.show_log,
            Src::Str(&label),
            &label,
        )
        .clicked()
        {
            app.inst.sampler.show_log = !app.inst.sampler.show_log;
        }
    }
}

/// The import notes as a card of lines.
fn log_card(app: &CantusApp, ui: &mut egui::Ui, rect: Rect, s: &Style) {
    let look = CardLook {
        title: Src::Key("smp.card.log"),
        switchable: false,
        selector: None,
        menu: false,
        display: None,
    };
    let cr = parts::card(ui, rect, s, &look, true);
    let lh = 16.0 * s.k;
    for (i, line) in app.inst.sampler.log.iter().enumerate() {
        let r = Rect::from_min_size(
            pos2(cr.body.left(), cr.body.top() + i as f32 * lh),
            vec2(cr.body.width(), lh),
        );
        parts::words(
            ui,
            r,
            s,
            Src::Str(line),
            Role::Body,
            s.t.text,
            Align2::LEFT_CENTER,
            Policy::Elide(Elide::End),
        );
    }
}

fn log_height(app: &CantusApp, s: &Style) -> f32 {
    (parts::CARD_CHROME + 16.0 * app.inst.sampler.log.len().max(1) as f32) * s.k
}

/// The map's card: the map is its display, bleeding to the edges, the key strip along its
/// bottom. Files dropped on it are mapped where they land.
fn map_card(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    track: TrackId,
    spec: &NativeSpec,
    rect: Rect,
    s: &Style,
    zoomed: bool,
) {
    let s = &s.function(Function::Source);
    let disp = rect.height() / s.k - band::TITLE - 1.0;
    let look = CardLook {
        title: Src::Key("smp.card.map"),
        switchable: false,
        selector: None,
        menu: false,
        display: Some(disp),
    };
    let cr = parts::card(ui, rect, s, &look, true);
    let Some(d) = cr.display else {
        return;
    };
    if zoomed {
        // Fit, in the title bar: show every zone again after a zoom.
        let k = s.k;
        let fr = Rect::from_min_max(
            pos2(cr.title.right() - 88.0 * k, cr.title.top() + 3.0 * k),
            pos2(cr.title.right() - 6.0 * k, cr.title.bottom() - 3.0 * k),
        );
        if parts::press(ui, fr, s, Src::Key("smp.fit"), &t("smp.fit_tip")).clicked() {
            app.inst.sampler.keys = None;
        }
    }
    let geo = map::map(app, ui, track, spec, d, s, zoomed);
    if let Some((files, key)) = take_drop(ui, d, |p| geo.key_at(p)) {
        drop_files(app, track, &files, key);
    }
}

/// The Play view: the map beside the zone's waveform, then the live cards.
pub fn play(app: &mut CantusApp, ui: &mut egui::Ui, track: TrackId, spec: &NativeSpec, s: &Style) {
    let area = ui.max_rect();
    let k = s.k;
    let gut = parts::GAP * k;
    let cards_h = parts::card_for(s, KnobSize::M, 1, 1).y;
    let top_h = (area.height() - cards_h - gut).max(120.0 * k);
    let top = Rect::from_min_size(area.min, vec2(area.width(), top_h));
    let split = top.left() + (top.width() - gut) * 0.62;
    map_card(
        app,
        ui,
        track,
        spec,
        Rect::from_min_max(top.min, pos2(split, top.bottom())),
        s,
        false,
    );
    let src = s.function(Function::Source);
    wave::play_card(
        app,
        ui,
        track,
        spec,
        Rect::from_min_max(pos2(split + gut, top.top()), top.max),
        &src,
    );
    let row = Rect::from_min_size(
        pos2(area.left(), top.bottom() + gut),
        vec2(area.width(), cards_h),
    );
    let secs = [
        Section {
            keys: &["amp.attack", "amp.decay", "amp.sustain", "amp.release"],
            ..AMP
        },
        Section {
            keys: &["filter.cutoff", "filter.reso", "filter.env"],
            ..FILTER
        },
        Section {
            keys: &["lfo1.pitch", "lfo1.rate"],
            selector: None,
            ..VIBRATO
        },
        Section {
            keys: &["glide", "tune", "gain"],
            selector: None,
            ..VOICE
        },
    ];
    section_row(app, ui, track, spec, row, s, &secs, KnobSize::M);
}

/// The height of the Sample and Seam cards' displays, at 100 %.
const WAVE_DISPLAY: f32 = 118.0;

/// The Edit view, as tall as it needs.
pub fn body(app: &mut CantusApp, ui: &mut egui::Ui, track: TrackId, spec: &NativeSpec, s: &Style) {
    let k = s.k;
    let gut = parts::GAP * k;
    let w = ui.available_width();
    let row_h = parts::card_for(s, KnobSize::M, 1, 1).y;
    let tools_h = parts::ROW * k;
    let a_h = map::zone_card_size(s).y;
    let b_h = wave::sample_card_height(s, WAVE_DISPLAY);
    let zones = model::zones(spec).len();
    let t_h = map::table_height(s, zones);
    let show_log = app.inst.sampler.show_log && !app.inst.sampler.log.is_empty();
    let l_h = if show_log {
        log_height(app, s) + gut
    } else {
        0.0
    };
    let total = tools_h + a_h + b_h + 3.0 * row_h + t_h + 6.0 * gut + l_h;
    let (area, _) = ui.allocate_exact_size(vec2(w, total), Sense::hover());
    let mut y = area.top();
    let mut take = |h: f32| {
        let r = Rect::from_min_size(pos2(area.left(), y), vec2(area.width(), h));
        y += h + gut;
        r
    };
    let tools_r = take(tools_h);
    let a = take(a_h);
    let zone_w = map::zone_card_size(s).x;
    let split = a.right() - zone_w;
    map_card(
        app,
        ui,
        track,
        spec,
        Rect::from_min_max(a.min, pos2(split - gut, a.bottom())),
        s,
        true,
    );
    let src = s.function(Function::Source);
    map::zone_card(
        app,
        ui,
        track,
        spec,
        Rect::from_min_max(pos2(split, a.top()), a.max),
        &src,
    );
    let b = take(b_h);
    let bsplit = b.left() + (b.width() - gut) * 0.64;
    wave::sample_card(
        app,
        ui,
        track,
        spec,
        Rect::from_min_max(b.min, pos2(bsplit, b.bottom())),
        &src,
        WAVE_DISPLAY,
    );
    wave::seam_card(
        app,
        ui,
        track,
        spec,
        Rect::from_min_max(pos2(bsplit + gut, b.top()), b.max),
        &src,
        WAVE_DISPLAY,
    );
    section_row(
        app,
        ui,
        track,
        spec,
        take(row_h),
        s,
        &[AMP, FILTER, OUTPUT],
        KnobSize::M,
    );
    section_row(
        app,
        ui,
        track,
        spec,
        take(row_h),
        s,
        &[MOD_ENV, VIBRATO, LFO2, VOICE],
        KnobSize::M,
    );
    let e = take(row_h);
    let ks = need(s, &KEYSWITCH, KnobSize::M);
    let esplit = e.right() - ks;
    group_card(
        app,
        ui,
        track,
        spec,
        Rect::from_min_max(e.min, pos2(esplit - gut, e.bottom())),
        s,
        KnobSize::M,
    );
    section(
        app,
        ui,
        track,
        spec,
        Rect::from_min_max(pos2(esplit, e.top()), e.max),
        s,
        &KEYSWITCH,
        KnobSize::M,
    );
    map::table(app, ui, track, spec, take(t_h), s);
    if show_log {
        let lh = log_height(app, s);
        log_card(app, ui, take(lh), s);
    }
    // Last, so its list drops over the cards below it.
    tools(app, ui, track, spec, tools_r, s);
}
