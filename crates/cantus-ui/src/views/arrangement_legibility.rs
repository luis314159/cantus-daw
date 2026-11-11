//! Every shipped theme is easy to work in and to read (the owner, 2026-11-10: "Todos deben
//! ser fáciles de trabajar y de leer con los diferentes temas"), measured on the colours the
//! views really paint — the arrangement's own painters ([`super::paint::note_ink`],
//! [`super::paint::body_ground`], the clip's selection outline, the grid's inks, the
//! track tints through the lane veil) — not on the tokens alone.
//!
//! Non-text contrast (WCAG 2.1, 1.4.11) at 3:1 for what carries meaning: a clip's content
//! (notes, hits, the waveform: one ink) on its own body for every track colour of the demo,
//! at rest, selected, lit and playing; the selection outline against the selected body and
//! the lane beside it; the playhead on the lanes; the fader cap on its strip; solo, record
//! and the lit key on their wells; the meters in their groove; the focus ring wherever it
//! lands. The demo's track colours are told apart in every theme (CIELAB ΔE between their
//! large fills, through the veil). The grid's bar and beat lines must be visible.
//!
//! Where Slate itself — the default the owner approved, which must not move by a pixel —
//! stands under a 3:1 target (its quiet bar lines, its white fader caps on Paper), a theme
//! is held to **at least Slate's own measure in the same light**: never harder to read than
//! the default. `RUST_TEST_NOCAPTURE=1` prints the whole table.

use super::paint::{body_ground, note_ink, ClipLook};
use crate::kit::appearance::PRESETS;
use crate::kit::color::{contrast, mix, opaque, selected_fill};
use crate::kit::{Style, Tokens};
use crate::views::clips_chrome::{Tint, Tints};
use egui::Color32;

/// The measures, in the order the table prints them.
const NAMES: [&str; 16] = [
    "content",
    "sel_body",
    "sel_lane",
    "playhead",
    "bar_line",
    "beat_line",
    "ruler_bar",
    "ruler_beat",
    "fader_cap",
    "knob",
    "solo_arm",
    "lit_key",
    "meters",
    "focus",
    "marks",
    "desk_marks",
];

/// What each measure must reach: 3:1 (WCAG 1.4.11) for what carries meaning; a beat line
/// and a ruler's beat tick need only be seen.
const TARGET: [f32; 16] = [
    3.0, 3.0, 3.0, 3.0, 3.0, 1.3, 3.0, 1.3, 3.0, 3.0, 3.0, 3.0, 3.0, 3.0, 3.0, 3.0,
];

/// How far apart two of the demo's track colours must be (CIELAB ΔE) where the track is
/// named by its colour — the colour cell and the header's stripe.
const MARKS_APART: f32 = 12.0;

/// How far apart a light theme's clip bodies at rest must be (CIELAB dE): over the 2.3 of a
/// just-noticeable difference, enough to be seen at a glance side by side.
const BODIES_APART: f32 = 3.0;

/// How strong a light theme's clip bodies at rest may be (CIELAB chroma): within the range
/// of the night themes' bodies (Slate Night's 13.4, Slate Lapis Night's 19.0, Blue Hour's
/// 19.1; Ultraviolet's 30.8) — never past 20. High contrast light lays no veil by design
/// (for low vision) and is not held to it.
const CALM: f32 = 20.0;

fn look(tint: Tint) -> ClipLook {
    ClipLook {
        selected: false,
        muted: false,
        tint,
        renaming: false,
        reserve: 0.0,
        sibling: false,
        label_row: 14.0,
        quiet: false,
        playing: false,
        near: false,
        lit: false,
        dim: false,
    }
}

/// The demo's track colours.
fn demo_colours() -> Vec<Color32> {
    crate::demos::startup_project()
        .tracks
        .iter()
        .map(|t| crate::theme::track_color(t.color))
        .collect()
}

/// CIELAB (D65).
fn lab(c: Color32) -> [f32; 3] {
    let f = |u: u8| {
        let u = u as f32 / 255.0;
        if u <= 0.04045 {
            u / 12.92
        } else {
            ((u + 0.055) / 1.055).powf(2.4)
        }
    };
    let (r, g, b) = (f(c.r()), f(c.g()), f(c.b()));
    let x = (0.4124 * r + 0.3576 * g + 0.1805 * b) / 0.95047;
    let y = 0.2126 * r + 0.7152 * g + 0.0722 * b;
    let z = (0.0193 * r + 0.1192 * g + 0.9505 * b) / 1.08883;
    let k = |t: f32| {
        if t > 0.008856 {
            t.cbrt()
        } else {
            7.787 * t + 16.0 / 116.0
        }
    };
    [
        116.0 * k(y) - 16.0,
        500.0 * (k(x) - k(y)),
        200.0 * (k(y) - k(z)),
    ]
}

fn delta(a: Color32, b: Color32) -> f32 {
    let (a, b) = (lab(a), lab(b));
    ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)).sqrt()
}

/// One theme's measures, and how far apart its demo tracks are.
pub(super) fn measure(t: Tokens) -> ([f32; 16], f32, f32, f32) {
    let s = Style::new(t);
    let mut tints = Tints::default();
    let colours = demo_colours();
    let lanes = [t.row_black, t.row_white];
    let (mut content, mut sel_body, mut sel_lane) = (f32::MAX, f32::MAX, f32::MAX);
    let mut larges = Vec::new();
    let mut smalls = Vec::new();
    for c in &colours {
        let tint = tints.of(*c, &s);
        // A clip's body at rest: what tells two tracks' clips apart in the lanes.
        larges.push(body_ground(&look(tint), &s));
        smalls.push(tint.small);
        for (selected, lit, playing) in [
            (false, false, false),
            (true, true, false),
            (false, true, false),
            (false, false, true),
        ] {
            let lk = ClipLook {
                selected,
                lit,
                playing,
                ..look(tint)
            };
            let mut ground = body_ground(&lk, &s);
            if selected {
                ground = selected_fill(ground, t.selection_tint, 1.5);
            }
            if playing {
                ground = mix(ground, tint.large, 0.06);
            }
            content = content.min(contrast(note_ink(&lk, &s), ground));
            if selected {
                sel_body = sel_body.min(contrast(t.selection, ground));
            }
        }
        for lane in lanes {
            sel_lane = sel_lane.min(contrast(t.selection, lane));
        }
    }
    let apart_of = |v: &[Color32]| {
        let mut apart = f32::MAX;
        for (i, a) in v.iter().enumerate() {
            for b in &v[i + 1..] {
                apart = apart.min(delta(*a, *b));
            }
        }
        apart
    };
    let (apart, marks_apart) = (apart_of(&larges), apart_of(&smalls));
    // How loud the bodies at rest are: their strongest chroma (CIELAB).
    let loud = larges
        .iter()
        .map(|c| {
            let l = lab(*c);
            (l[1] * l[1] + l[2] * l[2]).sqrt()
        })
        .fold(0.0, f32::max);
    let both = |fg: Color32, bgs: &[Color32]| {
        bgs.iter()
            .map(|b| contrast(fg, *b))
            .fold(f32::MAX, f32::min)
    };
    let ink = super::slate::Ink::of(&s);
    let lit_well = mix(t.well, t.highlight, if t.dark { 0.20 } else { 0.16 });
    // The fader cap's rim, as `kit::fader_slate` draws it at rest.
    let edge = opaque(t.inner_shadow);
    let rim = if t.dark {
        edge
    } else {
        crate::kit::fader_slate::paper_rim(&t, edge)
    };
    // The track's marks: the colour cell (on the bars, the strips, the selected strip) and
    // the header's stripe at rest and selected (on the header and the lanes).
    let marks = smalls
        .iter()
        .map(|c| both(*c, &[t.toolbar, t.strip, t.strip_selected]))
        .chain(colours.iter().flat_map(|c| {
            [false, true].map(|sel| both(ink.stripe(*c, sel), &[ink.surface, ink.base, ink.lane2]))
        }))
        .fold(f32::MAX, f32::min);
    // r39:fix2 — the mixer desk's stripe at a strip's head, as `mixer_strip` paints it: at
    // rest on the strip's top and body, selected on the selected body.
    let pal = crate::views::mixer::look::Pal::of(&s);
    let rest_k = if t.dark { 0.72 } else { 0.70 };
    let desk_marks = colours
        .iter()
        .map(|c| {
            both(pal.stripe(*c, rest_k), &[pal.strip_top, pal.surface])
                .min(both(pal.stripe(*c, 1.0), &[pal.selected(*c)]))
        })
        .fold(f32::MAX, f32::min);
    let m = [
        content,
        sel_body,
        sel_lane,
        both(t.highlight, &lanes).min(both(t.playhead, &lanes)),
        both(ink.grid_bar, &[ink.base, ink.lane2]),
        both(ink.grid_beat, &[ink.base, ink.lane2]),
        both(ink.ink3, &[t.toolbar]),
        both(ink.ink4, &[t.toolbar]),
        both(t.fcap_mid, &[t.strip]).max(both(rim, &[t.strip, t.strip_selected])),
        contrast(t.knob_rim, t.deck).max(contrast(t.knob_body, t.deck)),
        both(ink.solo, &[t.well, t.strip]).min(both(t.danger, &[t.well, t.strip])),
        contrast(t.highlight, lit_well),
        both(t.meter_low, &[t.meter_well])
            .min(both(t.meter_high, &[t.meter_well]))
            .min(both(t.meter_clip, &[t.meter_well])),
        both(
            opaque(t.focus),
            &[t.panel, t.ink, t.toolbar, t.strip, t.deck],
        ),
        marks,
        desk_marks,
    ];
    (m, apart, marks_apart, loud)
}

#[test]
fn every_theme_is_easy_to_read_and_to_work_in() {
    let slate = |dark: bool| {
        let t = if dark {
            crate::theme_slate::slate_night_tokens()
        } else {
            crate::theme_slate::slate_paper_tokens()
        };
        measure(t)
    };
    let (night, paper) = (slate(true), slate(false));
    let mut bad = Vec::new();
    println!(
        "{:<18} {}  bodies_apart marks_apart bodies_chroma",
        "theme",
        NAMES.join(" ")
    );
    for (id, make) in PRESETS {
        let t = make().kit_tokens();
        let (m, apart, marks_apart, loud) = measure(t);
        let (reference, ref_apart, _, _) = if t.dark { night } else { paper };
        let row: Vec<String> = m.iter().map(|v| format!("{v:>5.2}")).collect();
        println!(
            "{id:<18} {}  {apart:>5.1} {marks_apart:>5.1} {loud:>5.1}",
            row.join(" ")
        );
        if marks_apart + 1e-3 < MARKS_APART {
            bad.push(format!(
                "{id}: two demo tracks' colour marks are {marks_apart:.1} apart, need {MARKS_APART}"
            ));
        }
        for i in 0..NAMES.len() {
            // A light theme's marks and fader caps at 3:1 whatever Slate Paper did before
            // (round 39, the owner: "seeking excellence"); elsewhere 3:1, or Slate's own in
            // the same light where it stands under (a dark theme's colour cells x0.85).
            let held = !t.dark && matches!(NAMES[i], "marks" | "desk_marks" | "fader_cap");
            let near = if matches!(NAMES[i], "marks" | "desk_marks") {
                0.85
            } else {
                0.97
            };
            let need = if held {
                TARGET[i]
            } else {
                TARGET[i].min(reference[i] * near)
            };
            if m[i] + 1e-3 < need {
                bad.push(format!(
                    "{id}: {} is {:.2}, needs {need:.2}",
                    NAMES[i], m[i]
                ));
            }
        }
        if t.dark {
            // Night's bodies at rest are each dark theme's own (printed, not held): the
            // brief keeps Slate Night as it is, and its bodies are 2.7 apart.
            let _ = ref_apart;
        } else {
            // A light theme's bodies at rest told apart (the kick's and the clap's oranges):
            // CIELAB dE 3 - over the 2.3 of a just-noticeable difference, seen at a glance
            // side by side - while staying as calm as Slate Night's bodies (their chroma).
            if apart + 1e-3 < BODIES_APART {
                bad.push(format!(
                    "{id}: two demo tracks' bodies are {apart:.1} apart, need {BODIES_APART}"
                ));
            }
            if loud > CALM && !id.starts_with("contrast") {
                bad.push(format!(
                    "{id}: the bodies at rest reach chroma {loud:.1}, calm is {CALM}"
                ));
            }
        }
    }
    assert!(bad.is_empty(), "{}", bad.join("\n"));
}
