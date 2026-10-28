//! (c) One device in the shell (ADR 0024 §8), drawn by the real `kit::shell::Shell`:
//! Aequalis, display-first (≥ 70 % of the body), with cards of cells; and an instrument,
//! Calor, with the shell's bands — macros, modulation chips, the keyboard docked below.

use crate::eq;
use cantus_ui::kit::band::{self, CardLook};
use cantus_ui::kit::cell::{self, CellLook, Control};
use cantus_ui::kit::hint::{self, Caps};
use cantus_ui::kit::knob::{KnobLook, KnobSize};
use cantus_ui::kit::meter::Levels;
use cantus_ui::kit::shell::{self, Ab, Bands, Chip, Footer, Grid, Header, Macro, Preset, Shell};
use cantus_ui::kit::text::Src;
use cantus_ui::kit::{led, segmented, well, with_brush, Brush, Function, State, Style};
use egui::{Pos2, Rect, Vec2};
use std::fmt::Write as _;

pub const SIZE: Vec2 = Vec2::new(944.0, 600.0);
pub const INSTRUMENT: Vec2 = Vec2::new(1040.0, 680.0);

/// Lay `cells` out left to right in `body`, vertically centred.
fn cells(
    b: &mut Brush,
    body: Rect,
    s: &Style,
    list: &[(KnobSize, f32, f32, &str, Src<'_>)],
    hot: Option<usize>,
) -> f32 {
    let mut x = body.left();
    for (i, (size, v, d, text, label)) in list.iter().enumerate() {
        let sz = cell::size(*size);
        let r = Rect::from_min_size(Pos2::new(x, body.center().y - sz.y * 0.5), sz);
        let look = CellLook {
            control: Control::Knob(KnobLook::new(*size, *v, *d)),
            value: text,
            label: *label,
        };
        let st = State {
            hovered: hot == Some(i),
            ..State::REST
        };
        cell::paint(b, r, s, st, &look);
        x += sz.x + 6.0;
    }
    x
}

pub fn aequalis(ui: &mut egui::Ui, _s: &Style) {
    let header = Header {
        name: "Aequalis",
        context: "(Insert 1 · Keys)",
        hue: Function::Tone,
        preset: Some(Preset {
            name: "Vocal Presence",
            modified: true,
            favourite: true,
        }),
        ab: Some(Ab::A),
        undo: Some((true, true)),
        views: None,
        bypassed: Some(false),
        help: true,
        name_w: 0.0,
        window: None,
    };
    let chips = ["Latency 0.0 ms", "Oversampling 2×"];
    let footer = Footer {
        chips: &chips,
        meter: Some(Levels {
            level: [-9.5, -10.2],
            hold: [-6.0, -6.5],
        }),
    };
    Shell::new("proof-aequalis").show(ui, &header, &footer, |ui, s, body| {
        // The pointer is on the band's frequency: the help line says so.
        hint::set(
            ui.ctx(),
            &cantus_ui::i18n::t("kit.param.frequency"),
            Caps::PARAM,
            false,
            |b| b.write_str("2.40 kHz · drag, Shift for fine, double-click resets"),
        );
        let painter = ui.painter_at(body);
        with_brush(&painter, |b| {
            let mut g = Grid::new(body, s);
            let rows_h = 126.0 * s.k;
            let display = g.full(body.height() - 16.0 * s.k - rows_h - 4.0 * s.k);
            eq::display(b, display, s, &eq::AEQUALIS, Some(3), true);
            b.rrect_stroke(
                b.snap_rect(display),
                0.0,
                b.px(),
                cantus_ui::kit::color::with_alpha(s.t.edge, 0.5),
            );
            let fracs = [0.43, 0.25, 0.32];
            let rects: Vec<Rect> = g.row(rows_h, &fracs).collect();
            // Band 3.
            let look = CardLook {
                title: Src::Str("Band 4"),
                switchable: true,
                selector: Some("Bell"),
                menu: true,
                display: None,
            };
            let cr = band::card(b, rects[0], s, State::LIT, &look);
            let x = cells(
                b,
                cr.body,
                s,
                &[
                    (
                        KnobSize::M,
                        0.62,
                        0.5,
                        "2.40 kHz",
                        Src::Key("kit.param.frequency"),
                    ),
                    (
                        KnobSize::M,
                        0.625,
                        0.5,
                        "+4.5 dB",
                        Src::Key("kit.param.gain"),
                    ),
                    (KnobSize::M, 0.35, 0.3, "0.90", Src::Str("Q")),
                ],
                Some(0),
            );
            let lb = Rect::from_min_size(
                Pos2::new(x + 4.0, cr.body.top() + 12.0),
                Vec2::new(cr.body.right() - x - 6.0, 20.0),
            );
            led::button(b, lb, s, State::REST, Src::Key("kit.mixer.solo"));
            led::button(
                b,
                lb.translate(Vec2::new(0.0, 26.0)),
                s,
                State::LIT,
                Src::Str("Dynamic"),
            );
            let so = &s.function(Function::Output);
            let cr = band::card(
                b,
                rects[1],
                so,
                State::LIT,
                &CardLook {
                    title: Src::Key("kit.param.output"),
                    switchable: false,
                    selector: None,
                    menu: true,
                    display: None,
                },
            );
            cells(
                b,
                cr.body,
                so,
                &[
                    (
                        KnobSize::M,
                        0.49,
                        0.5,
                        "−0.5 dB",
                        Src::Key("kit.param.gain"),
                    ),
                    (KnobSize::M, 1.0, 1.0, "100 %", Src::Key("kit.param.mix")),
                ],
                None,
            );
            let cr = band::card(
                b,
                rects[2],
                s,
                State::LIT,
                &CardLook {
                    title: Src::Str("Analyser"),
                    switchable: true,
                    selector: None,
                    menu: true,
                    display: None,
                },
            );
            let seg = Rect::from_min_size(
                cr.body.min + Vec2::new(0.0, 6.0),
                Vec2::new(cr.body.width(), 22.0),
            );
            let labels = [
                Src::Key("kit.inspector.pre"),
                Src::Key("kit.inspector.post"),
                Src::Str("Off"),
            ];
            segmented::paint(b, seg, s, State::REST, &labels, 1, None);
            let half = (cr.body.width() - 6.0) * 0.5;
            let lb = Rect::from_min_size(
                seg.left_bottom() + Vec2::new(0.0, 10.0),
                Vec2::new(half, 20.0),
            );
            led::button(b, lb, s, State::LIT, Src::Str("Freeze"));
            led::button(
                b,
                lb.translate(Vec2::new(half + 6.0, 0.0)),
                s,
                State::REST,
                Src::Str("Collisions"),
            );
            let mf = Rect::from_min_size(
                lb.left_bottom() + Vec2::new(0.0, 10.0),
                Vec2::new(cr.body.width(), 22.0),
            );
            well::menu_field(b, mf, s, State::REST, "Range 18 dB · 4.5 dB/oct", true);
        });
    });
}

pub fn calor(ui: &mut egui::Ui, _s: &Style) {
    let views = [
        Src::Key("shell.view.play"),
        Src::Key("shell.view.edit"),
        Src::Key("shell.view.matrix"),
    ];
    let header = Header {
        name: "Calor",
        context: "(Keys)",
        hue: Function::Source,
        preset: Some(Preset {
            name: "Glass Pluck",
            modified: false,
            favourite: false,
        }),
        ab: Some(Ab::B),
        undo: Some((true, false)),
        views: Some((&views, 1)),
        bypassed: Some(false),
        help: true,
        name_w: 0.0,
        window: None,
    };
    let chips = ["Voices 5/16", "CPU 3.1 %"];
    let footer = Footer {
        chips: &chips,
        meter: Some(Levels {
            level: [-12.0, -11.0],
            hold: [-8.0, -7.5],
        }),
    };
    Shell::new("proof-calor").show(ui, &header, &footer, |ui, s, body| {
        hint::set(
            ui.ctx(),
            &cantus_ui::i18n::t("kit.param.cutoff"),
            Caps::PARAM,
            true,
            |b| write!(b, "{:.2} kHz", 1.84),
        );
        let painter = ui.painter_at(body);
        with_brush(&painter, |b| {
            let mut g = Grid::new(body, s);
            let bands = Bands {
                macros_open: true,
                mods_open: true,
                keyboard: true,
            };
            let br = shell::layout_bands(&mut g, s, &bands);
            let rest = g.rest();
            let row2 = 126.0 * s.k;
            let row1 = rest.height() - row2 - 4.0 * s.k;
            let r1: Vec<Rect> = g.row(row1, &[0.34, 0.30, 0.36]).collect();
            let r2: Vec<Rect> = g.row(row2, &[0.30, 0.22, 0.26, 0.22]).collect();
            let src = &s.function(Function::Source);
            // The displays take what the cells leave.
            let disp = (row1 / s.k - 24.0 - 1.0 - 8.0 - 86.0 - 16.0).max(96.0);
            for (i, (title, sel)) in [("Osc 1", "Saw"), ("Osc 2", "Pulse")].iter().enumerate() {
                let cr = band::card(
                    b,
                    r1[i],
                    src,
                    State::LIT,
                    &CardLook {
                        title: Src::Str(title),
                        switchable: true,
                        selector: Some(sel),
                        menu: true,
                        display: Some(disp),
                    },
                );
                if let Some(d) = cr.display {
                    wave(b, d, src, i == 1);
                }
                let body = Rect::from_min_max(
                    Pos2::new(
                        cr.body.left(),
                        cr.display.map_or(cr.body.top(), |d| d.bottom() + 8.0),
                    ),
                    cr.body.max,
                );
                cells(
                    b,
                    body,
                    src,
                    &[
                        (KnobSize::M, 0.5, 0.5, "+0 st", Src::Key("kit.param.pitch")),
                        (
                            KnobSize::M,
                            0.3,
                            0.0,
                            "+18 ct",
                            Src::Key("kit.param.detune"),
                        ),
                        (KnobSize::M, 0.8, 0.8, "−4 dB", Src::Key("kit.param.level")),
                    ],
                    None,
                );
            }
            let tone = &s.function(Function::Tone);
            let cr = band::card(
                b,
                r1[2],
                tone,
                State::LIT,
                &CardLook {
                    title: Src::Str("Filter"),
                    switchable: true,
                    selector: Some("Ladder 24"),
                    menu: true,
                    display: Some(disp),
                },
            );
            if let Some(d) = cr.display {
                let lp = [
                    eq::Band {
                        freq: 1840.0,
                        gain: 7.0,
                        q: 2.6,
                        kind: eq::Kind::Bell,
                    },
                    eq::Band {
                        freq: 2400.0,
                        gain: 0.0,
                        q: 1.0,
                        kind: eq::Kind::HighCut,
                    },
                ];
                eq::display_with(b, d, tone, &lp, None, false, false);
            }
            let body = Rect::from_min_max(
                Pos2::new(
                    cr.body.left(),
                    cr.display.map_or(cr.body.top(), |d| d.bottom() + 8.0),
                ),
                cr.body.max,
            );
            cells(
                b,
                body,
                tone,
                &[
                    (
                        KnobSize::M,
                        0.58,
                        1.0,
                        "1.84 kHz",
                        Src::Key("kit.param.cutoff"),
                    ),
                    (
                        KnobSize::M,
                        0.42,
                        0.0,
                        "42 %",
                        Src::Key("kit.param.resonance"),
                    ),
                    (KnobSize::S, 0.2, 0.0, "+3 dB", Src::Key("kit.param.drive")),
                    (
                        KnobSize::M,
                        0.5,
                        0.0,
                        "50 %",
                        Src::Key("kit.param.key_tracking"),
                    ),
                ],
                Some(0),
            );
            let dy = &s.function(Function::Dynamics);
            let cr = band::card(
                b,
                r2[0],
                dy,
                State::LIT,
                &CardLook {
                    title: Src::Str("Amp envelope"),
                    switchable: false,
                    selector: None,
                    menu: true,
                    display: None,
                },
            );
            cells(
                b,
                cr.body,
                dy,
                &[
                    (KnobSize::S, 0.08, 0.0, "4 ms", Src::Key("kit.param.attack")),
                    (
                        KnobSize::S,
                        0.35,
                        0.0,
                        "200 ms",
                        Src::Key("kit.param.decay"),
                    ),
                    (KnobSize::S, 0.8, 0.0, "80 %", Src::Key("kit.param.sustain")),
                    (
                        KnobSize::S,
                        0.4,
                        0.0,
                        "320 ms",
                        Src::Key("kit.param.release"),
                    ),
                ],
                None,
            );
            let md = &s.function(Function::Mod);
            let cr = band::card(
                b,
                r2[1],
                md,
                State::LIT,
                &CardLook {
                    title: Src::Str("LFO 1"),
                    switchable: true,
                    selector: Some("Sine"),
                    menu: false,
                    display: None,
                },
            );
            cells(
                b,
                cr.body,
                md,
                &[
                    (KnobSize::M, 0.45, 0.0, "2.4 Hz", Src::Key("kit.param.rate")),
                    (KnobSize::M, 0.3, 0.0, "30 %", Src::Key("kit.param.depth")),
                ],
                None,
            );
            let sp = &s.function(Function::Space);
            let cr = band::card(
                b,
                r2[2],
                sp,
                State::LIT,
                &CardLook {
                    title: Src::Str("Chorus"),
                    switchable: true,
                    selector: None,
                    menu: true,
                    display: None,
                },
            );
            cells(
                b,
                cr.body,
                sp,
                &[
                    (KnobSize::M, 0.5, 0.0, "0.8 Hz", Src::Key("kit.param.rate")),
                    (KnobSize::M, 0.7, 0.0, "70 %", Src::Key("kit.param.width")),
                    (KnobSize::M, 0.25, 0.0, "25 %", Src::Key("kit.param.mix")),
                ],
                None,
            );
            let out = &s.function(Function::Output);
            let cr = band::card(
                b,
                r2[3],
                out,
                State::LIT,
                &CardLook {
                    title: Src::Str("Voice"),
                    switchable: false,
                    selector: None,
                    menu: true,
                    display: None,
                },
            );
            cells(
                b,
                cr.body,
                out,
                &[
                    (KnobSize::M, 0.15, 0.0, "40 ms", Src::Str("Glide")),
                    (
                        KnobSize::M,
                        0.75,
                        0.75,
                        "−3 dB",
                        Src::Key("kit.param.volume"),
                    ),
                ],
                None,
            );
            let macros = [
                Macro {
                    label: Src::Str("Brightness"),
                    value: 0.62,
                    text: "62 %",
                },
                Macro {
                    label: Src::Str("Motion"),
                    value: 0.3,
                    text: "30 %",
                },
                Macro {
                    label: Src::Str("Space"),
                    value: 0.45,
                    text: "45 %",
                },
                Macro {
                    label: Src::Str("Grit"),
                    value: 0.1,
                    text: "10 %",
                },
                Macro {
                    label: Src::Str("Pluck"),
                    value: 0.8,
                    text: "80 %",
                },
            ];
            shell::macro_card(b, br.macros, s, &macros, true);
            let chips = [
                Chip {
                    tag: "LFO 1",
                    family: Function::Mod,
                    count: 2,
                },
                Chip {
                    tag: "Env 2",
                    family: Function::Mod,
                    count: 1,
                },
                Chip {
                    tag: "Vel",
                    family: Function::Source,
                    count: 3,
                },
                Chip {
                    tag: "Key",
                    family: Function::Source,
                    count: 0,
                },
                Chip {
                    tag: "MW",
                    family: Function::Source,
                    count: 1,
                },
            ];
            shell::mod_card(b, br.mods, s, &chips, true);
            if let Some(k) = br.keyboard {
                shell::keyboard_card(b, k, src, 36, 95, &[60, 64, 67]);
            }
        });
    });
}

/// A mock oscillator display: one cycle of the wave, filled.
fn wave(b: &mut Brush, d: Rect, s: &Style, pulse: bool) {
    let t = &s.t;
    let mut pts = [Pos2::ZERO; 160];
    let mid = d.center().y;
    let amp = d.height() * 0.32;
    for (i, p) in pts.iter_mut().enumerate() {
        let x = i as f32 / 159.0;
        let phase = (x * 2.0).fract();
        let y = if pulse {
            if phase < 0.35 {
                1.0
            } else {
                -1.0
            }
        } else {
            1.0 - 2.0 * phase
        };
        let soft = y * (1.0 - 0.15 * (x * 12.0).sin().abs());
        *p = Pos2::new(d.left() + d.width() * x, mid - soft * amp);
    }
    b.hline(d.left(), d.right(), b.snap_mid(mid), b.px(), t.grid);
    b.area(
        &pts,
        mid,
        cantus_ui::kit::color::with_alpha(s.hue, if t.dark { 0.20 } else { 0.14 }),
    );
    b.polyline(&pts, 2.0, s.hue);
}
