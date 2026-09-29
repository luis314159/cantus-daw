//! Gravis's two views as kit cards: the display card first (the engine's picture, and in
//! its title bar what plays now, the engine and the voice mode), then one card per section
//! in its function hue — Source, Tone, Dynamics, Output. **Play** keeps what matters
//! live; **Edit** has every control of the engine, then the voice, the LFO, the sub, the
//! drive, the song's hooks and the level, and scrolls when it is taller than the window.

use super::parts::{self, Card, Fmt, Item};
use super::*;
use crate::kit::knob::KnobSize;

const DRIVE_TYPES: [&str; 4] = [
    "bass.drive.tube",
    "bass.drive.tape",
    "bass.drive.fold",
    "bass.drive.hard",
];
const WAVES: [&str; 4] = [
    "bass.wave.saw",
    "bass.wave.square",
    "bass.wave.pulse",
    "bass.wave.triangle",
];
const FILTERS: [&str; 4] = [
    "bass.filter.ladder",
    "bass.filter.diode",
    "bass.filter.svf_lp",
    "bass.filter.svf_bp",
];
const TECHNIQUES: [&str; 4] = [
    "bass.tech.finger",
    "bass.tech.pick",
    "bass.tech.slap",
    "bass.tech.mute",
];
const TYPES: [&str; 4] = [
    "bass.type.round",
    "bass.type.half",
    "bass.type.flat",
    "bass.type.tape",
];
const FINGERINGS: [&str; 3] = [
    "bass.fingering.lowest",
    "bass.fingering.position",
    "bass.fingering.low",
];
const TUNINGS: [&str; 3] = [
    "bass.tuning.standard",
    "bass.tuning.drop",
    "bass.tuning.half",
];
const STRINGS: [&str; 3] = ["bass.strings.4", "bass.strings.5", "bass.strings.6"];
const PICKUPS: [&str; 2] = ["bass.pu.single", "bass.pu.humbucker"];
const PRIORITIES: [&str; 3] = [
    "bass.priority.last",
    "bass.priority.low",
    "bass.priority.high",
];
const GLIDE_WHEN: [&str; 3] = ["bass.glide.legato", "bass.glide.always", "bass.glide.off"];
const GLIDE_BY: [&str; 2] = ["bass.glide.time", "bass.glide.rate"];
const SHAPES: [&str; 3] = ["bass.shape.exp", "bass.shape.linear", "bass.shape.s"];
const DUCK_SHAPES: [&str; 3] = ["bass.shape.linear", "bass.shape.exp", "bass.shape.s"];
const PHASES: [&str; 3] = ["bass.phase.reset", "bass.phase.random", "bass.phase.free"];
const LFO_SHAPES: [&str; 4] = [
    "bass.lfo.sine",
    "bass.lfo.triangle",
    "bass.lfo.square",
    "bass.lfo.saw",
];
const SUB_WAVES: [&str; 3] = ["bass.lfo.sine", "bass.lfo.triangle", "bass.lfo.square"];
const DUCK_FROM: [&str; 3] = ["bass.duck.none", "bass.duck.kick", "bass.duck.sidechain"];
const SNAPS: [&str; 3] = ["bass.snap.off", "bass.snap.scale", "bass.snap.roots"];

const fn cell(key: &'static str) -> Item<'static> {
    Item::Cell(key, KnobSize::M, Fmt::Unit)
}

const fn cell_as(key: &'static str, fmt: Fmt) -> Item<'static> {
    Item::Cell(key, KnobSize::M, fmt)
}

const fn choice(key: &'static str, labels: &'static [&'static str], w: f32) -> Item<'static> {
    Item::Choice {
        key,
        labels,
        first: 0.0,
        w,
        caption: true,
    }
}

/// A row of cards whose shares follow what they hold.
fn fitted<'a>(
    list: &[(&'a str, Function, &'a [Item<'a>])],
    width: f32,
    s: &Style,
) -> Vec<Card<'a>> {
    let lists: Vec<&[Item]> = list.iter().map(|(_, _, items)| *items).collect();
    let fracs = parts::balanced(&lists, width, s);
    list.iter()
        .zip(fracs)
        .map(|((title, f, items), frac)| Card {
            title,
            f: *f,
            display: None,
            reserve: 0.0,
            items,
            frac,
        })
        .collect()
}

/// The Output card every engine shares in Play: the drive's character, the drive, the sub
/// and the glide.
const OUTPUT_PLAY: [Item<'static>; 5] = [
    choice("drive.type", &DRIVE_TYPES, 200.0),
    Item::Break,
    cell("drive"),
    cell("sub.level"),
    cell("glide"),
];

pub fn play(app: &mut CantusApp, ui: &mut egui::Ui, track: TrackId, spec: &NativeSpec, s: &Style) {
    let area = ui.max_rect();
    let k = s.k;
    let unison = spec.get("syn.unison").round() as u8;
    let e808_pitch = [
        cell("808.pitch"),
        cell("808.pitch_time"),
        cell("808.knock"),
        cell("808.sigh"),
    ];
    let e808_tone = [cell("808.shape"), cell("808.harmonics"), cell("808.click")];
    let e808_env = [cell("808.decay"), cell("808.sustain"), cell("808.release")];
    let syn_osc = [
        choice("syn.wave", &WAVES, 220.0),
        Item::Break,
        cell_as("syn.unison", Fmt::Whole("")),
        cell_as("syn.detune", Fmt::Unison(unison)),
        cell("syn.osc2"),
    ];
    let syn_filter = [
        choice("syn.filter", &FILTERS, 250.0),
        Item::Break,
        cell("syn.cutoff"),
        cell("syn.reso"),
        cell("syn.env"),
        cell("syn.key_track"),
    ];
    let syn_accent = [
        Item::Toggle("syn.accent_sweep", "bass.syn.accent_sweep", 130.0),
        Item::Break,
        cell("syn.f_decay"),
        cell("syn.accent"),
        cell("syn.decay"),
    ];
    let str_play = [
        choice("str.technique", &TECHNIQUES, 240.0),
        Item::Break,
        cell("str.hardness"),
        cell("str.mute"),
        cell("str.noise"),
    ];
    let str_tone = [
        choice("str.type", &TYPES, 240.0),
        Item::Break,
        cell("str.bright"),
        cell("str.tone"),
        cell("str.buzz"),
    ];
    let str_sus = [cell("str.sustain"), cell("str.release")];
    let list: [(&str, Function, &[Item]); 4] = match engine_of(spec) {
        0 => [
            ("bass.card.pitch", Function::Source, &e808_pitch),
            ("bass.card.tone", Function::Tone, &e808_tone),
            ("bass.card.envelope", Function::Dynamics, &e808_env),
            ("bass.card.out", Function::Output, &OUTPUT_PLAY),
        ],
        1 => [
            ("bass.card.osc", Function::Source, &syn_osc),
            ("bass.card.filter", Function::Tone, &syn_filter),
            ("bass.card.accent", Function::Dynamics, &syn_accent),
            ("bass.card.out", Function::Output, &OUTPUT_PLAY),
        ],
        _ => [
            ("bass.card.play", Function::Source, &str_play),
            ("bass.card.tone", Function::Tone, &str_tone),
            ("bass.card.sustain", Function::Dynamics, &str_sus),
            ("bass.card.out", Function::Output, &OUTPUT_PLAY),
        ],
    };
    let row = fitted(&list, area.width(), s);
    let hb = parts::row_height(&row, area.width(), s);
    let yb = area.bottom() - hb;
    let top = Rect::from_min_max(area.min, Pos2::new(area.right(), yb - parts::gutter(s)));
    let disp = (top.height() / k - parts::TITLE - 1.0).max(60.0);
    display_card(app, ui, track, spec, s, top, disp);
    parts::cards(app, ui, track, spec, s, area, yb, hb, &row);
}

pub fn body(app: &mut CantusApp, ui: &mut egui::Ui, track: TrackId, spec: &NativeSpec, s: &Style) {
    let width = ui.available_width();
    let k = s.k;
    let g = parts::gutter(s);
    let unison = spec.get("syn.unison").round() as u8;

    // The engine's own cards.
    let e808_pitch = [
        cell("808.pitch"),
        cell("808.pitch_time"),
        cell("808.knock"),
        cell("808.sigh"),
    ];
    let e808_tone = [
        cell("808.shape"),
        cell("808.harmonics"),
        cell_as("808.tilt", Fmt::Signed("dB/oct")),
        cell("808.even"),
        Item::Break,
        cell("808.click"),
        cell("808.click_tone"),
    ];
    let e808_env = [
        cell("808.attack"),
        cell("808.hold"),
        cell("808.decay"),
        cell("808.sustain"),
        cell("808.release"),
    ];
    let syn_osc = [
        choice("syn.wave", &WAVES, 220.0),
        Item::Break,
        cell("syn.pw"),
        cell_as("syn.unison", Fmt::Whole("")),
        cell_as("syn.detune", Fmt::Unison(unison)),
        cell("syn.spread"),
        cell("syn.width"),
        Item::Break,
        cell("syn.mono_below"),
        cell("syn.osc2"),
        cell_as("syn.osc2_semi", Fmt::Signed("st")),
        cell("syn.osc2_fine"),
    ];
    let syn_filter = [
        choice("syn.filter", &FILTERS, 250.0),
        Item::Break,
        cell("syn.cutoff"),
        cell("syn.reso"),
        cell("syn.bass_comp"),
        cell("syn.drive"),
        cell("syn.env"),
        cell("syn.key_track"),
    ];
    let syn_env = [
        cell("syn.f_attack"),
        cell("syn.f_decay"),
        cell("syn.f_sustain"),
        cell("syn.f_release"),
        Item::Break,
        cell("syn.attack"),
        cell("syn.decay"),
        cell("syn.sustain"),
        cell("syn.release"),
    ];
    let syn_accent = [
        Item::Toggle("syn.accent_sweep", "bass.syn.accent_sweep", 130.0),
        Item::Break,
        cell("syn.accent"),
        cell_as("syn.accent_vel", Fmt::Whole("")),
    ];
    let str_play = [
        choice("str.technique", &TECHNIQUES, 240.0),
        choice("str.fingering", &FINGERINGS, 220.0),
        Item::Break,
        cell("str.hardness"),
        cell_as("str.pop_vel", Fmt::Whole("")),
        cell("str.mute"),
        cell("str.release"),
        cell("str.noise"),
        cell("str.humanize"),
    ];
    let str_elec = [
        choice("str.pu_type", &PICKUPS, 180.0),
        Item::Break,
        cell_as("str.pu1", Fmt::Int("mm")),
        cell("str.pu1_level"),
        cell_as("str.pu2", Fmt::Int("mm")),
        cell("str.pu2_level"),
        cell_as("str.pu_height", Fmt::One("mm")),
        cell("str.piezo"),
        cell("str.tone"),
    ];
    let str_neck = [
        Item::Choice {
            key: "str.strings",
            labels: &STRINGS,
            first: 4.0,
            w: 110.0,
            caption: true,
        },
        choice("str.tuning", &TUNINGS, 230.0),
        choice("str.type", &TYPES, 250.0),
        Item::Toggle("str.fretless", "bass.str.fretless", 100.0),
        Item::Break,
        cell_as("str.scale", Fmt::Int("mm")),
        cell_as("str.frets", Fmt::Whole("")),
        cell("str.stiffness"),
        cell("str.sustain"),
        cell("str.bright"),
        cell("str.age"),
        cell_as("str.action", Fmt::One("mm")),
        cell("str.buzz"),
    ];
    let engine_rows: Vec<Vec<Card>> = match engine_of(spec) {
        0 => vec![fitted(
            &[
                ("bass.card.pitch", Function::Source, &e808_pitch),
                ("bass.card.tone", Function::Tone, &e808_tone),
                ("bass.card.envelope", Function::Dynamics, &e808_env),
            ],
            width,
            s,
        )],
        1 => vec![
            fitted(
                &[
                    ("bass.card.osc", Function::Source, &syn_osc),
                    ("bass.card.filter", Function::Tone, &syn_filter),
                ],
                width,
                s,
            ),
            fitted(
                &[
                    ("bass.card.envelopes", Function::Dynamics, &syn_env),
                    ("bass.card.accent", Function::Dynamics, &syn_accent),
                ],
                width,
                s,
            ),
        ],
        _ => vec![
            fitted(
                &[
                    ("bass.card.play", Function::Source, &str_play),
                    ("bass.card.electronics", Function::Tone, &str_elec),
                ],
                width,
                s,
            ),
            fitted(
                &[("bass.card.string", Function::Source, &str_neck)],
                width,
                s,
            ),
        ],
    };

    // The cards every engine shares.
    let voice = [
        choice("priority", &PRIORITIES, 150.0),
        choice("glide.trigger", &GLIDE_WHEN, 210.0),
        choice("glide.rate", &GLIDE_BY, 170.0),
        Item::Break,
        choice("glide.shape", &SHAPES, 200.0),
        choice("phase", &PHASES, 190.0),
        Item::Break,
        cell("glide"),
        cell("legato_window"),
        cell("declick"),
        cell_as("bend_range", Fmt::Int("st")),
        cell_as("octave", Fmt::Signed("oct")),
        cell_as("transpose", Fmt::Signed("st")),
        cell("fine"),
        cell("a4"),
        cell("velocity"),
    ];
    let lfo = [
        Item::Stepper("lfo.shape", &LFO_SHAPES, 130.0),
        Item::Break,
        cell_as("lfo.rate", Fmt::Hz),
    ];
    let sub = [
        choice("sub.wave", &SUB_WAVES, 190.0),
        Item::Break,
        cell("sub.level"),
        cell_as("sub.octave", Fmt::Int("oct")),
    ];
    let drive = [
        choice("drive.type", &DRIVE_TYPES, 200.0),
        Item::Toggle("drive.lock", "bass.drive_lock", 130.0),
        Item::Toggle("clip", "bass.clip", 96.0),
        Item::Break,
        cell("drive"),
        cell_as("drive.keytrack", Fmt::Signed("dB/oct")),
        cell_as("drive.ref", Fmt::Note),
        cell("subsonic"),
    ];
    let song = [
        choice("duck.source", &DUCK_FROM, 240.0),
        choice("duck.shape", &DUCK_SHAPES, 180.0),
        Item::Break,
        cell("duck.depth"),
        cell("duck.pre"),
        cell("duck.hold"),
        cell("duck.release"),
        Item::Break,
        choice("key.snap", &SNAPS, 240.0),
        Item::Slot(180.0, 24.0),
    ];
    let output = [cell("gain")];
    let shared = [
        fitted(
            &[
                ("bass.card.voice", Function::Output, &voice),
                ("bass.card.lfo", Function::Mod, &lfo),
                ("bass.card.sub", Function::Source, &sub),
            ],
            width,
            s,
        ),
        fitted(
            &[
                ("bass.card.drive", Function::Dynamics, &drive),
                ("bass.card.host", Function::Dynamics, &song),
                ("bass.card.level", Function::Output, &output),
            ],
            width,
            s,
        ),
    ];

    let disp = 190.0;
    let mut heights = vec![(parts::TITLE + disp + 1.0) * k];
    let rows: Vec<&[Card]> = engine_rows
        .iter()
        .map(|r| r.as_slice())
        .chain(shared.iter().map(|r| r.as_slice()))
        .collect();
    heights.extend(rows.iter().map(|r| parts::row_height(r, width, s)));
    let total = heights.iter().sum::<f32>() + g * rows.len() as f32;
    let (area, _) = ui.allocate_exact_size(Vec2::new(width, total), Sense::hover());
    let mut y = area.top();
    let top = Rect::from_min_size(area.min, Vec2::new(width, heights[0]));
    display_card(app, ui, track, spec, s, top, disp);
    y += heights[0] + g;
    for (i, (r, h)) in rows.iter().zip(&heights[1..]).enumerate() {
        let drawn = parts::cards(app, ui, track, spec, s, area, y, *h, r);
        // The song's card: the hook it waits for, said once.
        if i == rows.len() - 1 {
            if let Some(Some((_, slots))) = drawn.get(1) {
                if let Some(slot) = slots.first() {
                    host_note(ui, *slot, &s.function(Function::Dynamics));
                }
            }
        }
        y += h + g;
    }
}

/// What the song's card waits for, in its slot: a lamp off and the words, the whole note
/// in the help line.
fn host_note(ui: &egui::Ui, r: Rect, s: &Style) {
    let resp = ui.interact(r, ui.id().with("gravis-host-note"), Sense::hover());
    if resp.hovered() {
        crate::kit::hint::set_text(
            ui.ctx(),
            &t("bass.card.host"),
            &t("bass.host_note"),
            crate::kit::hint::Caps::NONE,
        );
    }
    kit::with_brush(&ui.painter_at(r), |b| {
        let lamp = Pos2::new(r.left() + 6.0 * s.k, r.center().y);
        kit::band::lamp(b, lamp, s, false, s.hue, 1.0);
        let tr = Rect::from_min_max(Pos2::new(lamp.x + 10.0 * s.k, r.top()), r.max);
        let _ = parts::words(
            b,
            tr,
            &parts::caption_text(Src::Key("bass.host_short"), s),
            s,
        );
    });
}
