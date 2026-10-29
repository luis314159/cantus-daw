//! The selected pad's cards (ADR 0024 §8): voice and model as menu fields beside the
//! pad's tuning and level, the character of its model, its envelope, filter, shape,
//! velocity, choke and voices, the hi-hat's pedal, its layers, and the kit's own controls
//! — each card in its function's hue, every number a cell in its real unit.

use super::{display, grid, set_field, set_kit_field, upgrade, View};
use crate::app::CantusApp;
use crate::i18n::{t, tf};
use crate::kit::band::{self, CardLook, CardRects};
use crate::kit::knob::KnobSize;
use crate::kit::text::{Elide, Policy, Role, Src};
use crate::kit::{with_brush, Fill, Function, Style};
use crate::views::inst::sampler::parts::{self, write_pan, write_value};
use cantus_core::{
    Command, DrumPad, KitField, PadField, PadLayer, PadVoice, PulsusParams, TrackId, PULSUS_MODEL,
};
use egui::{pos2, vec2, Align2, Rect, Sense};
use std::fmt::Write as _;

/// A pad field's readout in its unit: pans by side, amounts as percentages, voices ∞.
fn write_field(field: PadField, info: &cantus_core::PadParamInfo, v: f32, b: &mut String) {
    match field {
        PadField::Polyphony if v < 0.5 => b.push_str(&t("pulsus.poly.inf")),
        PadField::Pan => write_pan(v * 100.0, b),
        PadField::Curve => {
            let _ = write!(b, "{v:+.2}");
        }
        _ if info.unit.is_empty() && !info.stepped && info.min >= 0.0 && info.max <= 1.0 => {
            let _ = write!(b, "{:.0} %", v * 100.0);
        }
        _ => write_value(v, info.unit, info.stepped, b),
    }
}

/// One field of the pad as a kit cell: one undo step per drag.
#[allow(clippy::too_many_arguments)]
fn cell(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    track: TrackId,
    pad: &DrumPad,
    field: PadField,
    rect: Rect,
    s: &Style,
    size: KnobSize,
) {
    let info = field.info(pad.voice);
    let name = t(info.label_key);
    let (r, v) = parts::num_cell(
        ui,
        rect,
        s,
        egui::Id::new(("pulsus-cell", track.0, pad.id.0, field)),
        field.get(pad),
        info.min..=info.max,
        info.default,
        info.logarithmic,
        info.stepped,
        Some(Src::Key(info.label_key)),
        &name,
        size,
        &[],
        move |v, b| write_field(field, &info, v, b),
    );
    if r.changed() {
        set_field(app, track, pad.id, field, v, true);
    }
    if r.drag_stopped() || r.double_clicked() {
        app.end_gesture();
    }
}

/// A named choice as a menu field with ‹ ›: its list drops on click. Returns the index
/// chosen, if it changed.
#[allow(clippy::too_many_arguments)]
fn choice(
    ui: &mut egui::Ui,
    rect: Rect,
    s: &Style,
    id: egui::Id,
    names: &[Src<'_>],
    current: usize,
    shown: &str,
    hint: &str,
) -> Option<usize> {
    let (_, c) = parts::field(ui, rect, s, shown, true, false, hint);
    let n = names.len().max(1) as i32;
    if c.prev || c.next {
        let j = (current as i32 + if c.prev { -1 } else { 1 }).rem_euclid(n) as usize;
        return (j != current).then_some(j);
    }
    let items = parts::radio_items(names, Some(current));
    parts::dropdown(ui, id, rect, s, &items, c.open).filter(|i| *i != current)
}

/// A stepped pad field whose values have names, as a menu field.
fn field_choice(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    track: TrackId,
    pad: &DrumPad,
    field: PadField,
    rect: Rect,
    s: &Style,
) {
    let Some(names) = field.choices(pad) else {
        return;
    };
    let current = (field.get(pad) as usize).min(names.len() - 1);
    let labels: Vec<Src<'_>> = names.iter().map(|k| Src::Key(k)).collect();
    let id = egui::Id::new(("pulsus-choice", track.0, pad.id.0, field));
    let hint = t(field.info(pad.voice).label_key);
    if let Some(i) = choice(ui, rect, s, id, &labels, current, &t(names[current]), &hint) {
        set_field(app, track, pad.id, field, i as f32, false);
    }
}

/// The body of a one-row card, trimmed to the cells' height and centred in it.
fn centred(body: Rect, s: &Style, size: KnobSize) -> Rect {
    let h = parts::cell_size(s, size).y;
    let top = body.center().y - h * 0.5;
    Rect::from_min_max(pos2(body.left(), top), pos2(body.right(), top + h))
}

/// The width `n` cells of `size` take, with their gaps.
fn cells_w(s: &Style, n: usize, size: KnobSize) -> f32 {
    if n == 0 {
        return 0.0;
    }
    parts::cell_size(s, size).x * n as f32 + parts::GAP * s.k * (n - 1) as f32
}

/// A column of menu fields: its width at this scale.
fn fields_w(s: &Style) -> f32 {
    150.0 * s.k
}

/// Cells for `fields` laid from `x` along the body's row.
#[allow(clippy::too_many_arguments)]
fn cells_from(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    track: TrackId,
    pad: &DrumPad,
    fields: &[PadField],
    row: Rect,
    x: f32,
    s: &Style,
) {
    let size = KnobSize::M;
    let r = Rect::from_min_max(pos2(x, row.top()), row.max);
    for (f, cr) in fields
        .iter()
        .zip(parts::cells(r, s, size, fields.len(), fields.len()))
    {
        cell(app, ui, track, pad, *f, cr, s, size);
    }
}

fn applying(pad: &DrumPad, fields: &[PadField]) -> Vec<PadField> {
    fields.iter().copied().filter(|f| f.applies(pad)).collect()
}

/// Lay cards of the given needed widths across `row`: their rects.
fn row_rects(row: Rect, s: &Style, needs: &[f32]) -> Vec<Rect> {
    let gut = parts::GAP * s.k;
    let fr = parts::fracs(needs, row.width(), gut);
    let room = row.width() - gut * (needs.len().saturating_sub(1)) as f32;
    let mut x = row.left();
    fr.iter()
        .map(|f| {
            let w = room * f;
            let r = Rect::from_min_size(pos2(x, row.top()), vec2(w, row.height()));
            x += w + gut;
            r
        })
        .collect()
}

fn look<'a>(title: Src<'a>, selector: Option<&'a str>) -> CardLook<'a> {
    CardLook {
        title,
        switchable: false,
        selector,
        menu: false,
        display: None,
    }
}

const CHARACTER: [PadField; 21] = {
    use PadField::*;
    [
        PitchDrop, PitchDecay, Click, Sigh, Phase, NoiseDecay, Tone, Mix, HpCutoff, Wires, Modes,
        Damping, Position, Hardness, Glide, Shell, BandMix, Swell, Bursts, Spacing, Q,
    ]
};
const ENVELOPE: [PadField; 5] = {
    use PadField::*;
    [Attack, Hold, Curve, PitchEnv, PitchEnvTime]
};
const SHAPE: [PadField; 3] = [
    PadField::TransAttack,
    PadField::TransSustain,
    PadField::Drive,
];
const VELOCITY: [PadField; 4] = {
    use PadField::*;
    [VelLevel, VelTone, VelPitch, VelDecay]
};

/// The pads card: the grid is its display; under it, + Pad ▾, the kit's model, and the
/// upgrade when the kit is a model 1 one.
fn pads_card(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    track: TrackId,
    kit: &PulsusParams,
    rect: Rect,
    s: &Style,
) {
    let k = s.k;
    let row_h = parts::ROW * k;
    let disp = (rect.height() - row_h - 16.0 * k) / k - band::TITLE - 1.0;
    let mut lk = look(Src::Key("pulsus.card.pads"), None);
    lk.display = Some(disp.max(40.0));
    let cr = parts::card(ui, rect, s, &lk, true);
    if let Some(d) = cr.display {
        grid::show(app, ui, track, kit, d, s);
    }
    let row = Rect::from_min_max(pos2(cr.body.left(), cr.body.bottom() - row_h), cr.body.max);
    let add = Rect::from_min_size(row.min, vec2(96.0 * k, row_h));
    let (_, c) = parts::field(
        ui,
        add,
        s,
        &t("pulsus.pad.add"),
        false,
        false,
        &t("pulsus.pad.add"),
    );
    let voices: Vec<Src<'_>> = PadVoice::ALL
        .iter()
        .map(|v| Src::Key(v.label_key()))
        .collect();
    let items = parts::radio_items(&voices, None);
    if let Some(i) = parts::dropdown(
        ui,
        egui::Id::new(("pulsus-add", track.0)),
        add,
        s,
        &items,
        c.open,
    ) {
        super::add_pad(app, track, PadVoice::ALL[i]);
    }
    let model1 = kit.version < PULSUS_MODEL;
    let up_w = if model1 { 176.0 * k } else { 0.0 };
    let mr = Rect::from_min_max(
        pos2(add.right() + 6.0 * k, row.top()),
        pos2(
            row.right() - up_w - if model1 { 6.0 * k } else { 0.0 },
            row.bottom(),
        ),
    );
    let model = if model1 {
        t("pulsus.model1")
    } else {
        t("pulsus.model2")
    };
    parts::readout(ui, mr, s, &model, false, false, &t("pulsus.kit.version"));
    if model1 {
        let ur = Rect::from_min_max(pos2(row.right() - up_w, row.top()), row.max);
        if parts::press(
            ui,
            ur,
            s,
            Src::Key("pulsus.upgrade"),
            &t("pulsus.upgrade_tip"),
        )
        .clicked()
        {
            upgrade(app, track);
        }
    }
}

/// The hit card: the selected pad's display, bleeding to the card's edges.
fn hit_card(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    track: TrackId,
    kit: &PulsusParams,
    pad: Option<&DrumPad>,
    rect: Rect,
    s: &Style,
) {
    let mut lk = look(Src::Key("pulsus.card.hit"), None);
    lk.display = Some(rect.height() / s.k - band::TITLE - 1.0);
    let cr = parts::card(ui, rect, s, &lk, true);
    let Some(d) = cr.display else {
        return;
    };
    let s = &crate::views::inst::face::on_display(s);
    match pad {
        Some(pad) => display::show(app, ui, track, kit, pad, d, s),
        None => parts::words(
            ui,
            d,
            s,
            Src::Key("pad.none_yet"),
            Role::Body,
            s.t.text_dim,
            Align2::CENTER_CENTER,
            Policy::Elide(Elide::End),
        ),
    }
}

/// The voice card's width: its menu fields and four cells.
fn voice_need(s: &Style) -> f32 {
    16.0 * s.k + fields_w(s) + 8.0 * s.k + cells_w(s, 4, KnobSize::M)
}

/// Voice and model as menu fields, then tune, decay, level and pan.
fn voice_card(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    track: TrackId,
    pad: &DrumPad,
    rect: Rect,
    s: &Style,
) {
    let s = &s.function(Function::Source);
    let cr = parts::card(
        ui,
        rect,
        s,
        &look(Src::Key("pulsus.section.voice"), None),
        true,
    );
    let k = s.k;
    let row = centred(cr.body, s, KnobSize::M);
    let fw = fields_w(s);
    let h = parts::ROW * k;
    let f0 = Rect::from_min_size(pos2(row.left(), row.center().y - h - 3.0 * k), vec2(fw, h));
    let voices: Vec<Src<'_>> = PadVoice::ALL
        .iter()
        .map(|v| Src::Key(v.label_key()))
        .collect();
    let id = egui::Id::new(("pulsus-voice", track.0, pad.id.0));
    if let Some(i) = choice(
        ui,
        f0,
        s,
        id,
        &voices,
        pad.voice.index(),
        &t(pad.voice.label_key()),
        &t("pad.voice"),
    ) {
        set_field(app, track, pad.id, PadField::Voice, i as f32, false);
    }
    let f1 = f0.translate(vec2(0.0, h + 6.0 * k));
    if PadField::Model.applies(pad) {
        field_choice(app, ui, track, pad, PadField::Model, f1, s);
    }
    let fields = applying(
        pad,
        &[
            PadField::Tune,
            PadField::Decay,
            PadField::Level,
            PadField::Pan,
        ],
    );
    cells_from(
        app,
        ui,
        track,
        pad,
        &fields,
        row,
        row.left() + fw + 8.0 * k,
        s,
    );
}

/// The model's own fields (and a tom's material, a sample pad's file).
fn character_fields(pad: &DrumPad) -> Vec<PadField> {
    applying(pad, &CHARACTER)
}

fn character_need(s: &Style, pad: &DrumPad) -> f32 {
    let n = character_fields(pad).len();
    let extra = if PadField::Material.applies(pad) || pad.voice == PadVoice::Sample {
        fields_w(s) + 8.0 * s.k
    } else {
        0.0
    };
    (16.0 * s.k + extra + cells_w(s, n, KnobSize::M)).max(180.0 * s.k)
}

fn character_card(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    track: TrackId,
    pad: &DrumPad,
    rect: Rect,
    s: &Style,
) {
    let s = &s.function(Function::Source);
    let cr = parts::card(
        ui,
        rect,
        s,
        &look(Src::Key("pulsus.section.character"), None),
        true,
    );
    let k = s.k;
    let row = centred(cr.body, s, KnobSize::M);
    let mut x = row.left();
    let h = parts::ROW * k;
    if PadField::Material.applies(pad) {
        let f = Rect::from_min_size(pos2(x, row.center().y - h * 0.5), vec2(fields_w(s), h));
        field_choice(app, ui, track, pad, PadField::Material, f, s);
        x += fields_w(s) + 8.0 * k;
    }
    if pad.voice == PadVoice::Sample {
        let f = Rect::from_min_size(pos2(x, row.center().y - h - 3.0 * k), vec2(fields_w(s), h));
        let file = pad
            .sample
            .as_deref()
            .and_then(|p| std::path::Path::new(p).file_name())
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| t("pad.sample_none"));
        parts::readout(ui, f, s, &file, false, false, &t("pad.sample"));
        let l = f.translate(vec2(0.0, h + 6.0 * k));
        if parts::press(ui, l, s, Src::Key("pad.sample_load"), &t("pad.sample_load")).clicked() {
            if let Some(i) = super::kit_of(app, track).and_then(|k| k.index_of(pad.id)) {
                app.load_pad_sample(track, i);
            }
        }
        x += fields_w(s) + 8.0 * k;
    }
    let fields = character_fields(pad);
    cells_from(app, ui, track, pad, &fields, row, x, s);
}

/// A card of cells for `fields` (those the pad uses), with a type selector when given.
#[allow(clippy::too_many_arguments)]
fn fields_card(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    track: TrackId,
    pad: &DrumPad,
    rect: Rect,
    s: &Style,
    title: &str,
    function: Function,
    fields: &[PadField],
    selector: Option<PadField>,
) -> CardRects {
    let s = &s.function(function);
    let sel = selector.filter(|f| f.applies(pad));
    let names = sel.and_then(|f| f.choices(pad));
    let current = sel.map(|f| f.get(pad) as usize);
    let shown = names.zip(current).map(|(n, i)| t(n[i.min(n.len() - 1)]));
    let cr = parts::card(ui, rect, s, &look(Src::Key(title), shown.as_deref()), true);
    if let (Some(f), Some(n), Some(i)) = (sel, names, current) {
        let step = parts::selector_step(ui, &cr, s, &t(f.info(pad.voice).label_key));
        if step != 0 {
            let j = (i as i32 + step).rem_euclid(n.len() as i32);
            set_field(app, track, pad.id, f, j as f32, false);
        }
    }
    let fields = applying(pad, fields);
    let row = centred(cr.body, s, KnobSize::M);
    // A filter that is off keeps its cells, dimmed (ADR 0024 §7: dimmed, never hidden).
    let mut sub = ui.new_child(egui::UiBuilder::new().max_rect(cr.body));
    if selector == Some(PadField::Filter) && pad.filter == 0 {
        sub.disable();
    }
    cells_from(app, &mut sub, track, pad, &fields, row, row.left(), s);
    cr
}

fn fields_need(s: &Style, pad: &DrumPad, fields: &[PadField], selector: bool) -> f32 {
    let n = applying(pad, fields).len();
    let w = 16.0 * s.k + cells_w(s, n, KnobSize::M);
    w.max(if selector { 190.0 } else { 120.0 } * s.k)
}

/// The filter's cells (dimmed while its type is Off).
fn filter_fields(_pad: &DrumPad) -> Vec<PadField> {
    vec![PadField::Cutoff, PadField::Resonance, PadField::FilterEnv]
}

/// The velocity card: the velocity → level law drawn, then the four amounts.
fn velocity_card(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    track: TrackId,
    pad: &DrumPad,
    rect: Rect,
    s: &Style,
) {
    let s = &s.function(Function::Mod);
    let cr = parts::card(
        ui,
        rect,
        s,
        &look(Src::Key("pulsus.section.velocity"), None),
        true,
    );
    let k = s.k;
    let row = centred(cr.body, s, KnobSize::M);
    let cw = 64.0 * k;
    let curve = Rect::from_min_size(
        pos2(row.left(), row.top() + 8.0 * k),
        vec2(cw, row.height() - 16.0 * k),
    );
    with_brush(&ui.painter_at(curve), |b| {
        let r = b.snap_rect(curve);
        crate::kit::light::sunken(b, r, s.r(3.0), s.t.ink, 1.0, s.t.depth);
        let exp = 1.5 * (pad.vel_level / 100.0).max(0.0);
        let pts: Vec<egui::Pos2> = (0..=32)
            .map(|i| {
                let v = i as f32 / 32.0;
                let g = if exp == 0.0 { 1.0 } else { v.powf(exp) };
                pos2(
                    r.left() + 3.0 * k + v * (r.width() - 6.0 * k),
                    r.bottom() - 3.0 * k - g * (r.height() - 6.0 * k),
                )
            })
            .collect();
        b.polyline(&pts, 1.5 * k, s.hue);
    });
    cells_from(
        app,
        ui,
        track,
        pad,
        &applying(pad, &VELOCITY),
        row,
        row.left() + cw + 8.0 * k,
        s,
    );
}

fn velocity_need(s: &Style) -> f32 {
    16.0 * s.k + 72.0 * s.k + cells_w(s, 4, KnobSize::M)
}

/// Choke group as a menu field, choke fade and voices as cells, choke on note-off as an LED.
fn choke_card(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    track: TrackId,
    pad: &DrumPad,
    rect: Rect,
    s: &Style,
) {
    let s = &s.function(Function::Output);
    let cr = parts::card(
        ui,
        rect,
        s,
        &look(Src::Key("pulsus.section.voices"), None),
        true,
    );
    let k = s.k;
    let row = centred(cr.body, s, KnobSize::M);
    let fw = fields_w(s);
    let h = parts::ROW * k;
    let f0 = Rect::from_min_size(pos2(row.left(), row.center().y - h - 3.0 * k), vec2(fw, h));
    let names: Vec<String> = (0..=cantus_core::CHOKE_GROUPS)
        .map(|g| {
            if g == 0 {
                t("pulsus.choke.none")
            } else {
                tf("pad.group", &[("n", &grid::choke_letter(g))])
            }
        })
        .collect();
    let labels: Vec<Src<'_>> = names.iter().map(|n| Src::Str(n)).collect();
    let id = egui::Id::new(("pulsus-choke", track.0, pad.id.0));
    let cur = (pad.choke as usize).min(names.len() - 1);
    if let Some(i) = choice(ui, f0, s, id, &labels, cur, &names[cur], &t("pad.choke")) {
        set_field(app, track, pad.id, PadField::Choke, i as f32, false);
    }
    let f1 = f0.translate(vec2(0.0, h + 6.0 * k));
    let on = pad.choke_on_off;
    if parts::led(
        ui,
        f1,
        s,
        on,
        Src::Key("pulsus.f.choke_on_off"),
        &t("pulsus.f.choke_on_off"),
    )
    .clicked()
    {
        set_field(
            app,
            track,
            pad.id,
            PadField::ChokeOnOff,
            if on { 0.0 } else { 1.0 },
            false,
        );
    }
    cells_from(
        app,
        ui,
        track,
        pad,
        &applying(pad, &[PadField::ChokeFade, PadField::Polyphony]),
        row,
        row.left() + fw + 8.0 * k,
        s,
    );
}

fn choke_need(s: &Style) -> f32 {
    16.0 * s.k + fields_w(s) + 8.0 * s.k + cells_w(s, 2, KnobSize::M)
}

/// The hi-hat's pedal: openness and open decay, and whether it follows the pedal.
fn pedal_card(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    track: TrackId,
    pad: &DrumPad,
    rect: Rect,
    s: &Style,
) {
    let s = &s.function(Function::Mod);
    let cr = parts::card(
        ui,
        rect,
        s,
        &look(Src::Key("pulsus.section.pedal"), None),
        true,
    );
    let k = s.k;
    let row = centred(cr.body, s, KnobSize::M);
    let fields = applying(pad, &[PadField::Openness, PadField::OpenDecay]);
    cells_from(app, ui, track, pad, &fields, row, row.left(), s);
    let x = row.left() + cells_w(s, fields.len(), KnobSize::M) + 8.0 * k;
    let lr = Rect::from_min_max(
        pos2(x, row.center().y - parts::ROW * k * 0.5),
        pos2(row.right(), row.center().y + parts::ROW * k * 0.5),
    );
    let on = pad.pedal;
    if PadField::Pedal.applies(pad)
        && parts::led(
            ui,
            lr,
            s,
            on,
            Src::Key("pulsus.f.pedal"),
            &t("pulsus.f.pedal"),
        )
        .clicked()
    {
        set_field(
            app,
            track,
            pad.id,
            PadField::Pedal,
            if on { 0.0 } else { 1.0 },
            false,
        );
    }
}

fn pedal_need(s: &Style) -> f32 {
    16.0 * s.k + cells_w(s, 2, KnobSize::M) + 8.0 * s.k + 120.0 * s.k
}

fn has_pedal(pad: &DrumPad) -> bool {
    pad.voice == PadVoice::HiHat && PadField::Openness.applies(pad)
}

fn set_layers(
    app: &mut CantusApp,
    track: TrackId,
    pad: &DrumPad,
    layers: Vec<PadLayer>,
    gesture: bool,
) {
    let cmd = Command::SetPadLayers {
        track,
        pad: pad.id,
        layers,
    };
    if gesture {
        app.exec_merging(
            cmd,
            cantus_core::MergeKey::of(&("pulsus-layers", track.0, pad.id.0)),
        );
    } else {
        app.exec(cmd);
    }
}

/// A layer table row's height at 100 %.
const LAYER_ROW: f32 = 22.0;

/// The layers card's height for a pad, at this scale.
fn layers_height(s: &Style, pad: &DrumPad) -> f32 {
    let rows = pad.layers.len().max(1) as f32;
    (parts::CARD_CHROME + 16.0 + (LAYER_ROW + 4.0) * rows + parts::ROW + 6.0) * s.k
}

/// The layers as a table: file, velocity range, crossfade, tune and level, each a well you
/// drag; Remove per row, + Layer… below, the round-robin mode as the card's selector.
fn layers_card(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    track: TrackId,
    pad: &DrumPad,
    rect: Rect,
    s: &Style,
) {
    let cr = fields_card(
        app,
        ui,
        track,
        pad,
        rect,
        s,
        "pulsus.section.layers",
        Function::Source,
        &[],
        Some(PadField::RoundRobin),
    );
    let s = &s.function(Function::Source);
    let k = s.k;
    let gap = parts::GAP * k;
    let cols: [(&str, f32); 7] = [
        ("pad.sample", 0.0),
        ("pulsus.layer.vel_lo", 60.0),
        ("pulsus.layer.vel_hi", 60.0),
        ("pulsus.layer.xfade", 60.0),
        ("pad.tune", 72.0),
        ("pad.level", 72.0),
        ("pulsus.layer.remove", 92.0),
    ];
    let fixed: f32 = cols.iter().map(|(_, w)| w * k).sum::<f32>() + gap * (cols.len() - 1) as f32;
    let name_w = (cr.body.width() - fixed).max(80.0 * k);
    let mut x = cr.body.left();
    let xs: Vec<(f32, f32)> = cols
        .iter()
        .map(|(_, w)| {
            let w = if *w == 0.0 { name_w } else { w * k };
            let at = (x, w);
            x += w + gap;
            at
        })
        .collect();
    let legend_h = 14.0 * k;
    for ((key, _), (x, w)) in cols.iter().zip(&xs).take(6) {
        let r = Rect::from_min_size(pos2(*x, cr.body.top()), vec2(*w, legend_h));
        parts::words(
            ui,
            r,
            s,
            Src::Key(key),
            Role::Micro,
            s.t.text_dim,
            Align2::CENTER_CENTER,
            Policy::Abbreviate,
        );
    }
    let row_h = LAYER_ROW * k;
    let top = cr.body.top() + legend_h + 2.0 * k;
    if pad.layers.is_empty() {
        let r = Rect::from_min_size(pos2(cr.body.left(), top), vec2(cr.body.width(), row_h));
        parts::words(
            ui,
            r,
            s,
            Src::Key("pulsus.layer.none"),
            Role::Body,
            s.t.text_dim,
            Align2::LEFT_CENTER,
            Policy::Elide(Elide::End),
        );
    }
    for (i, layer) in pad.layers.iter().enumerate() {
        let y = top + i as f32 * (row_h + 4.0 * k);
        let cell = |c: usize| Rect::from_min_size(pos2(xs[c].0, y), vec2(xs[c].1, row_h));
        let name = std::path::Path::new(&layer.sample)
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();
        parts::readout(ui, cell(0), s, &name, false, false, &layer.sample);
        let mut edited = layer.clone();
        let mut changed = false;
        let mut done = false;
        let specs: [(f32, std::ops::RangeInclusive<f32>, f32, &str, &str); 5] = [
            (
                layer.vel_lo as f32,
                1.0..=127.0,
                1.0,
                "pulsus.layer.vel_lo",
                "",
            ),
            (
                layer.vel_hi as f32,
                1.0..=127.0,
                127.0,
                "pulsus.layer.vel_hi",
                "",
            ),
            (
                layer.xfade as f32,
                0.0..=64.0,
                0.0,
                "pulsus.layer.xfade",
                "",
            ),
            (layer.tune, -24.0..=24.0, 0.0, "pad.tune", "st"),
            (layer.level_db, -48.0..=12.0, 0.0, "pad.level", "dB"),
        ];
        for (c, (v0, range, default, key, unit)) in specs.into_iter().enumerate() {
            let mut v = v0;
            let r = parts::drag_num(
                ui,
                cell(c + 1),
                s,
                &mut v,
                range,
                default,
                false,
                &t(key),
                |v, b| write_value(v, unit, unit.is_empty(), b),
            );
            if r.changed() {
                changed = true;
                match c {
                    0 => edited.vel_lo = v.round() as u8,
                    1 => edited.vel_hi = v.round() as u8,
                    2 => edited.xfade = v.round() as u8,
                    3 => edited.tune = v,
                    _ => edited.level_db = v,
                }
            }
            done |= r.drag_stopped() || r.double_clicked();
        }
        if changed {
            let mut all = pad.layers.clone();
            all[i] = edited;
            set_layers(app, track, pad, all, true);
        }
        if done {
            app.end_gesture();
        }
        if parts::press(
            ui,
            cell(6),
            s,
            Src::Key("pulsus.layer.remove"),
            &t("pulsus.layer.remove"),
        )
        .clicked()
        {
            let mut all = pad.layers.clone();
            all.remove(i);
            set_layers(app, track, pad, all, false);
        }
    }
    let add = Rect::from_min_size(
        pos2(cr.body.left(), cr.body.bottom() - parts::ROW * k),
        vec2(140.0 * k, parts::ROW * k),
    );
    if parts::press(
        ui,
        add,
        s,
        Src::Key("pulsus.layer.add"),
        &t("pulsus.layer.add"),
    )
    .clicked()
    {
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("WAV", &["wav", "WAV"])
            .pick_file()
        {
            let mut layers = pad.layers.clone();
            layers.push(PadLayer::new(path.to_string_lossy().to_string()));
            set_layers(app, track, pad, layers, false);
        }
    }
}

/// The kit's own controls: the noise seed, the pedal's controller and its direction, and
/// what model 1 does differently.
fn kit_card(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    track: TrackId,
    kit: &PulsusParams,
    pad: Option<&DrumPad>,
    rect: Rect,
    s: &Style,
) {
    let s = &s.function(Function::Output);
    let cr = parts::card(ui, rect, s, &look(Src::Key("pulsus.kit"), None), true);
    let k = s.k;
    let h = parts::ROW * k;
    let row = Rect::from_min_size(
        pos2(cr.body.left(), cr.body.center().y - h * 0.5),
        vec2(cr.body.width(), h),
    );
    let pair = |x: f32, lw: f32, vw: f32| {
        (
            Rect::from_min_size(pos2(x, row.top()), vec2(lw, h)),
            Rect::from_min_size(pos2(x + lw + 6.0 * k, row.top()), vec2(vw, h)),
        )
    };
    let (l, v) = pair(row.left(), 70.0 * k, 90.0 * k);
    parts::row_label(ui, l, s, "pulsus.kit.seed");
    let mut seed = kit.kit.seed as f32;
    let r = parts::drag_num(
        ui,
        v,
        s,
        &mut seed,
        0.0..=65_535.0,
        1.0,
        false,
        &t("pulsus.kit.seed"),
        |v, b| {
            let _ = write!(b, "{v:.0}");
        },
    );
    if r.changed() {
        set_kit_field(app, track, KitField::Seed, seed.round());
    }
    let (l, v) = pair(v.right() + 16.0 * k, 90.0 * k, 72.0 * k);
    parts::row_label(ui, l, s, "pulsus.kit.hat_cc");
    let mut cc = kit.kit.hat_cc as f32;
    let r = parts::drag_num(
        ui,
        v,
        s,
        &mut cc,
        0.0..=127.0,
        4.0,
        false,
        &t("pulsus.kit.hat_cc"),
        |v, b| {
            let _ = write!(b, "CC {v:.0}");
        },
    );
    if r.changed() {
        set_kit_field(app, track, KitField::HatCc, cc.round());
    }
    let lr = Rect::from_min_size(pos2(v.right() + 16.0 * k, row.top()), vec2(170.0 * k, h));
    let inv = kit.kit.hat_invert;
    if parts::led(
        ui,
        lr,
        s,
        inv,
        Src::Key("pulsus.kit.hat_invert"),
        &t("pulsus.kit.hat_invert"),
    )
    .clicked()
    {
        set_kit_field(app, track, KitField::HatInvert, if inv { 0.0 } else { 1.0 });
    }
    if kit.version < PULSUS_MODEL && pad.is_some_and(|p| p.voice != PadVoice::Kick) {
        let nr = Rect::from_min_max(pos2(lr.right() + 16.0 * k, row.top()), row.max);
        parts::readout(
            ui,
            nr,
            s,
            &t("pulsus.drive_model1"),
            false,
            false,
            &t("pulsus.drive_model1"),
        );
    }
}

/// The Play view: the pads beside the selected pad's hit, then voice and character, then
/// envelope, filter and choke.
pub(super) fn play(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    track: TrackId,
    kit: &PulsusParams,
    pad: Option<&DrumPad>,
    s: &Style,
) {
    let area = ui.max_rect();
    let k = s.k;
    let gut = parts::GAP * k;
    let row_h = parts::card_for(s, KnobSize::M, 1, 1).y;
    let n_rows = if pad.is_some() { 2.0 } else { 0.0 };
    let top_h = (area.height() - n_rows * (row_h + gut)).max(160.0 * k);
    let top = Rect::from_min_size(area.min, vec2(area.width(), top_h));
    let split = top.left() + (top.width() - gut) * 0.5;
    pads_card(
        app,
        ui,
        track,
        kit,
        Rect::from_min_max(top.min, pos2(split, top.bottom())),
        s,
    );
    hit_card(
        app,
        ui,
        track,
        kit,
        pad,
        Rect::from_min_max(pos2(split + gut, top.top()), top.max),
        s,
    );
    let Some(pad) = pad else {
        return;
    };
    let r2 = Rect::from_min_size(
        pos2(area.left(), top.bottom() + gut),
        vec2(area.width(), row_h),
    );
    let rs = row_rects(r2, s, &[voice_need(s), character_need(s, pad)]);
    voice_card(app, ui, track, pad, rs[0], s);
    character_card(app, ui, track, pad, rs[1], s);
    let r3 = r2.translate(vec2(0.0, row_h + gut));
    let env = [PadField::Attack, PadField::Hold, PadField::Curve];
    let rs = row_rects(
        r3,
        s,
        &[
            fields_need(s, pad, &env, false),
            fields_need(s, pad, &filter_fields(pad), true),
            choke_need(s),
        ],
    );
    fields_card(
        app,
        ui,
        track,
        pad,
        rs[0],
        s,
        "pulsus.section.envelope",
        Function::Dynamics,
        &env,
        None,
    );
    fields_card(
        app,
        ui,
        track,
        pad,
        rs[1],
        s,
        "pulsus.section.filter",
        Function::Tone,
        &filter_fields(pad),
        Some(PadField::Filter),
    );
    choke_card(app, ui, track, pad, rs[2], s);
}

/// The Edit view: every section, as tall as it needs (it scrolls).
pub(super) fn edit(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    track: TrackId,
    kit: &PulsusParams,
    pad: Option<&DrumPad>,
    s: &Style,
) {
    let k = s.k;
    let gut = parts::GAP * k;
    let w = ui.available_width();
    let row_h = parts::card_for(s, KnobSize::M, 1, 1).y;
    let top_h = 300.0 * k;
    let kit_h = (parts::CARD_CHROME + parts::ROW) * k;
    let rows = if pad.is_some() { 3.0 } else { 0.0 };
    let layers_h = pad.map_or(0.0, |p| layers_height(s, p) + gut);
    let total = top_h + gut + rows * (row_h + gut) + layers_h + kit_h;
    let (area, _) = ui.allocate_exact_size(vec2(w, total), Sense::hover());
    let mut y = area.top();
    let mut take = |h: f32| {
        let r = Rect::from_min_size(pos2(area.left(), y), vec2(area.width(), h));
        y += h + gut;
        r
    };
    let top = take(top_h);
    let split = top.left() + (top.width() - gut) * 0.5;
    pads_card(
        app,
        ui,
        track,
        kit,
        Rect::from_min_max(top.min, pos2(split, top.bottom())),
        s,
    );
    hit_card(
        app,
        ui,
        track,
        kit,
        pad,
        Rect::from_min_max(pos2(split + gut, top.top()), top.max),
        s,
    );
    if let Some(pad) = pad {
        let rs = row_rects(take(row_h), s, &[voice_need(s), character_need(s, pad)]);
        voice_card(app, ui, track, pad, rs[0], s);
        character_card(app, ui, track, pad, rs[1], s);
        let filt = filter_fields(pad);
        let rs = row_rects(
            take(row_h),
            s,
            &[
                fields_need(s, pad, &ENVELOPE, false),
                fields_need(s, pad, &filt, true),
                fields_need(s, pad, &SHAPE, true),
            ],
        );
        fields_card(
            app,
            ui,
            track,
            pad,
            rs[0],
            s,
            "pulsus.section.envelope",
            Function::Dynamics,
            &ENVELOPE,
            None,
        );
        fields_card(
            app,
            ui,
            track,
            pad,
            rs[1],
            s,
            "pulsus.section.filter",
            Function::Tone,
            &filt,
            Some(PadField::Filter),
        );
        fields_card(
            app,
            ui,
            track,
            pad,
            rs[2],
            s,
            "pulsus.section.shape",
            Function::Dynamics,
            &SHAPE,
            Some(PadField::DriveMode),
        );
        let r4 = take(row_h);
        if has_pedal(pad) {
            let rs = row_rects(r4, s, &[velocity_need(s), choke_need(s), pedal_need(s)]);
            velocity_card(app, ui, track, pad, rs[0], s);
            choke_card(app, ui, track, pad, rs[1], s);
            pedal_card(app, ui, track, pad, rs[2], s);
        } else {
            let rs = row_rects(r4, s, &[velocity_need(s), choke_need(s)]);
            velocity_card(app, ui, track, pad, rs[0], s);
            choke_card(app, ui, track, pad, rs[1], s);
        }
        let lh = layers_height(s, pad);
        layers_card(app, ui, track, pad, take(lh), s);
    }
    kit_card(app, ui, track, kit, pad, take(kit_h), s);
    let _ = (View::Edit, Fill::Solid(s.t.ink));
}
